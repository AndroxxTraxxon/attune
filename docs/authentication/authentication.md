# Authentication & Authorization

## Overview

Attune uses JWT (JSON Web Token) based authentication for securing API endpoints. The authentication system supports user registration, login, token refresh, and password management.

## Architecture

### Components

1. **JWT Tokens**
   - **Access Tokens**: Short-lived tokens (default: 1 hour) used for API authentication
   - **Refresh Tokens**: Long-lived tokens (default: 7 days) used to obtain new access tokens

2. **Password Security**
   - Passwords are hashed using **Argon2id** (industry-standard, memory-hard algorithm)
   - Password hashes are stored in the `attributes` JSONB field of the `identity` table
   - Minimum password length: 8 characters

3. **Middleware**
   - `require_auth`: Middleware function that validates JWT tokens on protected routes
   - `RequireAuth`: Extractor for accessing authenticated user information in handlers

## Configuration

Authentication is configured via environment variables:

```bash
# JWT Secret Key (REQUIRED in production)
JWT_SECRET=your-secret-key-here

# Token Expiration (in seconds)
JWT_ACCESS_EXPIRATION=3600      # 1 hour (default)
JWT_REFRESH_EXPIRATION=604800   # 7 days (default)
```

**Security Warning**: Always set a strong, random `JWT_SECRET` in production. The default value is insecure and should only be used for development.

## API Endpoints

### Public Endpoints (No Authentication Required)

#### Register a New User

```http
POST /auth/register
Content-Type: application/json

{
  "login": "username",
  "password": "securepassword123",
  "display_name": "John Doe" // optional
}
```

**Response:**
```json
{
  "data": {
    "access_token": "eyJhbGc...",
    "refresh_token": "eyJhbGc...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

#### Login

```http
POST /auth/login
Content-Type: application/json

{
  "login": "username",
  "password": "securepassword123"
}
```

**Response:**
```json
{
  "data": {
    "access_token": "eyJhbGc...",
    "refresh_token": "eyJhbGc...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

#### Refresh Access Token

```http
POST /auth/refresh
Content-Type: application/json

{
  "refresh_token": "eyJhbGc..."
}
```

**Response:**
```json
{
  "data": {
    "access_token": "eyJhbGc...",
    "refresh_token": "eyJhbGc...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

### Protected Endpoints (Authentication Required)

All protected endpoints require an `Authorization` header with a valid access token:

```http
Authorization: Bearer <access_token>
```

#### Get Current User

```http
GET /auth/me
Authorization: Bearer eyJhbGc...
```

**Response:**
```json
{
  "data": {
    "id": 1,
    "login": "username",
    "display_name": "John Doe"
  }
}
```

#### Change Password

```http
POST /auth/change-password
Authorization: Bearer eyJhbGc...
Content-Type: application/json

{
  "current_password": "oldpassword123",
  "new_password": "newpassword456"
}
```

**Response:**
```json
{
  "data": {
    "success": true,
    "message": "Password changed successfully"
  }
}
```

## Error Responses

Authentication errors return appropriate HTTP status codes:

- **400 Bad Request**: Invalid request format or validation errors
- **401 Unauthorized**: Missing, invalid, or expired token; invalid credentials
- **403 Forbidden**: Insufficient permissions (future RBAC implementation)
- **409 Conflict**: Username already exists during registration

Example error response:
```json
{
  "error": "Invalid authentication token",
  "code": "UNAUTHORIZED"
}
```

## Usage in Route Handlers

### Protecting Routes

Add the authentication middleware to routes that require authentication:

```rust
use crate::auth::middleware::RequireAuth;

async fn protected_handler(
    RequireAuth(user): RequireAuth,
) -> Result<Json<ApiResponse<MyData>>, ApiError> {
    let identity_id = user.identity_id()?;
    let login = user.login();
    
    // Your handler logic here
    Ok(Json(ApiResponse::new(data)))
}
```

### Accessing User Information

The `RequireAuth` extractor provides access to the authenticated user's claims:

```rust
pub struct AuthenticatedUser {
    pub claims: Claims,
}

impl AuthenticatedUser {
    pub fn identity_id(&self) -> Result<i64, ParseIntError>
    pub fn login(&self) -> &str
}
```

## Database Schema

### Identity Table

The `identity` table stores user authentication information:

```sql
CREATE TABLE attune.identity (
    id BIGSERIAL PRIMARY KEY,
    login TEXT NOT NULL UNIQUE,
    display_name TEXT,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    password_hash TEXT,  -- Added in migration 20240102000001
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Note**: The `password_hash` column is optional to support:
- External authentication providers (OAuth, SAML, etc.)
- Service accounts that don't use password authentication
- Integration-token authentication

## Passwordless Integration Tokens

Administrators can create revokable integration tokens for any identity through the access-control API/UI or CLI. The plaintext token is shown only once, and Attune stores only a hash plus safe display metadata.

Integrations call `POST /auth/token-login` with the opaque token and receive the same `TokenResponse` shape as password, OIDC, and LDAP login. Access JWTs continue to use the identity id as `sub`, so normal RBAC applies. Refresh JWTs created by this flow use the integration-token record id as `sub`; `/auth/refresh` resolves that record and rejects refresh after the token is revoked, expired, deleted, or the owning identity is frozen.

Already-issued access JWTs remain valid until their normal short expiration. Revoke tokens when an integration is decommissioned or a token may have been exposed.

## Security Best Practices

1. **JWT Secret**
   - Use a strong, random secret (minimum 256 bits)
   - Never commit secrets to version control
   - Rotate secrets periodically in production

2. **Token Storage (Client-Side)**
   - Store tokens securely (e.g., httpOnly cookies or secure storage)
   - Never expose tokens in URLs or localStorage (if using web clients)
   - Clear tokens on logout

3. **Password Requirements**
   - Minimum 8 characters (enforced by validation)
   - Consider implementing additional requirements (uppercase, numbers, symbols)
   - Implement rate limiting on login attempts (future enhancement)

4. **HTTPS**
   - Always use HTTPS in production to protect tokens in transit
   - Configure proper TLS/SSL certificates

5. **Token Expiration**
   - Keep access tokens short-lived (1 hour recommended)
   - Use refresh tokens for long-lived sessions
   - Use integration-token revocation to stop future refresh for passwordless integrations

## Future Enhancements

### Planned Features

1. **Role-Based Access Control (RBAC)**
   - Permission set assignments
   - Fine-grained authorization middleware
   - Resource-level permissions

2. **Multi-Factor Authentication (MFA)**
   - TOTP support
   - SMS/Email verification codes

3. **OAuth/OIDC Integration**
   - Support for external identity providers
   - Single Sign-On (SSO)

4. **Token Revocation**
   - Blacklist/whitelist mechanisms
   - Force logout functionality

5. **Account Security**
   - Password reset via email
   - Account lockout after failed attempts
   - Security audit logs

6. **API Keys**
   - Service-to-service authentication
   - Scoped API keys for automation

## Testing

### Manual Testing with cURL

```bash
# Register a new user
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "login": "testuser",
    "password": "testpass123",
    "display_name": "Test User"
  }'

# Login
curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "login": "testuser",
    "password": "testpass123"
  }'

# Get current user (replace TOKEN with actual access token)
curl http://localhost:8080/auth/me \
  -H "Authorization: Bearer TOKEN"

# Change password
curl -X POST http://localhost:8080/auth/change-password \
  -H "Authorization: Bearer TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "current_password": "testpass123",
    "new_password": "newpass456"
  }'

# Refresh token
curl -X POST http://localhost:8080/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{
    "refresh_token": "REFRESH_TOKEN"
  }'
```

### Unit Tests

Password hashing and JWT utilities include comprehensive unit tests:

```bash
# Run auth-related tests
cargo test --package attune-api password
cargo test --package attune-api jwt
cargo test --package attune-api middleware
```

## Troubleshooting

### Common Issues

1. **"Missing authentication token"**
   - Ensure you're including the `Authorization` header
   - Verify the header format: `Bearer <token>`

2. **"Authentication token expired"**
   - Use the refresh token endpoint to get a new access token
   - Check token expiration configuration

3. **"Invalid login or password"**
   - Verify credentials are correct
   - Check if the identity has a password set (some accounts may use external auth)

4. **"JWT_SECRET not set" warning**
   - Set the `JWT_SECRET` environment variable before starting the server
   - Use a strong, random value in production

### Debug Logging

Enable debug logging to troubleshoot authentication issues:

```bash
RUST_LOG=attune_api=debug cargo run --bin attune-api
```

## References

- [RFC 7519: JSON Web Token (JWT)](https://datatracker.ietf.org/doc/html/rfc7519)
- [Argon2 Password Hashing](https://en.wikipedia.org/wiki/Argon2)
- [OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)
