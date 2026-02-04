# Testing Authentication Endpoints

This guide provides step-by-step instructions for testing the Attune authentication system.

## Prerequisites

1. **Database Running**
   ```bash
   # Start PostgreSQL (if using Docker)
   docker run -d \
     --name postgres \
     -e POSTGRES_PASSWORD=postgres \
     -p 5432:5432 \
     postgres:15
   ```

2. **Database Setup**
   ```bash
   # Create database and user
   psql -U postgres -c "CREATE DATABASE attune;"
   psql -U postgres -c "CREATE USER svc_attune WITH PASSWORD 'attune_password';"
   psql -U postgres -c "GRANT ALL PRIVILEGES ON DATABASE attune TO svc_attune;"
   ```

3. **Run Migrations**
   ```bash
   export DATABASE_URL="postgresql://svc_attune:attune_password@localhost:5432/attune"
   sqlx migrate run
   ```

4. **Set Environment Variables**
   ```bash
   export DATABASE_URL="postgresql://svc_attune:attune_password@localhost:5432/attune"
   export JWT_SECRET="my-super-secret-jwt-key-min-256-bits-please"
   export JWT_ACCESS_EXPIRATION=3600
   export JWT_REFRESH_EXPIRATION=604800
   export RUST_LOG=info
   ```

## Starting the API Server

```bash
cd attune
cargo run --bin attune-api
```

Expected output:
```
 INFO Starting Attune API Service
 INFO Configuration loaded successfully
 INFO Environment: development
 INFO Connecting to database...
 INFO Database connection established
 INFO JWT configuration initialized
 INFO Starting server on 127.0.0.1:8080
 INFO Server listening on 127.0.0.1:8080
 INFO Attune API Service is ready
```

## Testing with cURL

### 1. Health Check (Verify Server is Running)

```bash
curl http://localhost:8080/api/v1/health
```

Expected response:
```json
{
  "status": "healthy"
}
```

### 2. Register a New User

```bash
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "login": "alice",
    "password": "securepass123",
    "display_name": "Alice Smith"
  }'
```

Expected response:
```json
{
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

**Save the access_token for the next steps!**

### 3. Login with Existing User

```bash
curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "login": "alice",
    "password": "securepass123"
  }'
```

Expected response:
```json
{
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

### 4. Get Current User (Protected Endpoint)

Replace `YOUR_ACCESS_TOKEN` with the actual token from step 2 or 3:

```bash
curl http://localhost:8080/auth/me \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

Expected response:
```json
{
  "data": {
    "id": 1,
    "login": "alice",
    "display_name": "Alice Smith"
  }
}
```

### 5. Change Password (Protected Endpoint)

```bash
curl -X POST http://localhost:8080/auth/change-password \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "current_password": "securepass123",
    "new_password": "newsecurepass456"
  }'
```

Expected response:
```json
{
  "data": {
    "success": true,
    "message": "Password changed successfully"
  }
}
```

### 6. Login with New Password

```bash
curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "login": "alice",
    "password": "newsecurepass456"
  }'
```

Should return new tokens.

### 7. Refresh Access Token

Save the refresh_token from a previous login, then:

```bash
curl -X POST http://localhost:8080/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{
    "refresh_token": "YOUR_REFRESH_TOKEN"
  }'
```

Expected response:
```json
{
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

## Error Cases to Test

### Invalid Credentials

```bash
curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "login": "alice",
    "password": "wrongpassword"
  }'
```

Expected response (401):
```json
{
  "error": "Invalid login or password",
  "code": "UNAUTHORIZED"
}
```

### Missing Authentication Token

```bash
curl http://localhost:8080/auth/me
```

Expected response (401):
```json
{
  "error": {
    "code": 401,
    "message": "Missing authentication token"
  }
}
```

### Invalid Token

```bash
curl http://localhost:8080/auth/me \
  -H "Authorization: Bearer invalid.token.here"
```

Expected response (401):
```json
{
  "error": {
    "code": 401,
    "message": "Invalid authentication token"
  }
}
```

### Duplicate Registration

Register the same user twice:

```bash
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "login": "alice",
    "password": "securepass123"
  }'
```

Expected response (409):
```json
{
  "error": "Identity with login 'alice' already exists",
  "code": "CONFLICT"
}
```

### Validation Errors

Short password:

```bash
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "login": "bob",
    "password": "short"
  }'
```

Expected response (422):
```json
{
  "error": "Invalid registration request: ...",
  "code": "VALIDATION_ERROR"
}
```

## Testing with HTTPie (Alternative)

If you prefer HTTPie (more readable):

```bash
# Install HTTPie
pip install httpie

# Register
http POST localhost:8080/auth/register \
  login=alice password=securepass123 display_name="Alice Smith"

# Login
http POST localhost:8080/auth/login \
  login=alice password=securepass123

# Get current user (set TOKEN variable first)
TOKEN="your_access_token_here"
http GET localhost:8080/auth/me \
  "Authorization: Bearer $TOKEN"

# Change password
http POST localhost:8080/auth/change-password \
  "Authorization: Bearer $TOKEN" \
  current_password=securepass123 \
  new_password=newsecurepass456
```

## Testing with Postman

1. **Import Collection**
   - Create a new collection named "Attune Auth"
   - Add base URL variable: `{{baseUrl}}` = `http://localhost:8080`

2. **Setup Environment**
   - Create environment "Attune Local"
   - Variables:
     - `baseUrl`: `http://localhost:8080`
     - `accessToken`: (will be set by tests)
     - `refreshToken`: (will be set by tests)

3. **Add Requests**
   - POST `{{baseUrl}}/auth/register`
   - POST `{{baseUrl}}/auth/login`
   - GET `{{baseUrl}}/auth/me` with header: `Authorization: Bearer {{accessToken}}`
   - POST `{{baseUrl}}/auth/change-password`
   - POST `{{baseUrl}}/auth/refresh`

4. **Test Scripts**
   Add to login/register requests to save tokens:
   ```javascript
   pm.test("Status is 200", function () {
       pm.response.to.have.status(200);
   });
   
   var jsonData = pm.response.json();
   pm.environment.set("accessToken", jsonData.data.access_token);
   pm.environment.set("refreshToken", jsonData.data.refresh_token);
   ```

## Automated Testing

### Unit Tests

Run the authentication unit tests:

```bash
# Test password hashing
cargo test --package attune-api password

# Test JWT utilities
cargo test --package attune-api jwt

# Test middleware
cargo test --package attune-api middleware

# Run all API tests
cargo test --package attune-api
```

### Integration Tests (Future)

Integration tests will be added to test the full authentication flow:

```bash
cargo test --package attune-api --test auth_integration
```

## Troubleshooting

### Server Won't Start

1. **Database Connection Error**
   ```
   Error: error communicating with database
   ```
   - Verify PostgreSQL is running
   - Check DATABASE_URL is correct
   - Verify database exists and user has permissions

2. **Migration Error**
   ```
   Error: migration version not found
   ```
   - Run migrations: `sqlx migrate run`

3. **JWT_SECRET Warning**
   ```
   WARN JWT_SECRET not set, using default
   ```
   - Set JWT_SECRET environment variable

### Authentication Fails

1. **Invalid Credentials**
   - Verify password is correct
   - Check if identity exists in database:
     ```sql
     SELECT * FROM attune.identity WHERE login = 'alice';
     ```

2. **Token Expired**
   - Use the refresh token to get a new access token
   - Check JWT_ACCESS_EXPIRATION setting

3. **Invalid Token Format**
   - Ensure Authorization header format: `Bearer <token>`
   - No extra spaces or quotes

### Database Issues

Check identities in database:
```sql
-- Connect to database
psql -U svc_attune -d attune

-- View all identities
SELECT id, login, display_name, created FROM attune.identity;

-- Check password hash exists
SELECT login, 
       attributes->>'password_hash' IS NOT NULL as has_password 
FROM attune.identity;
```

## Security Notes

- **Never use default JWT_SECRET in production**
- Always use HTTPS in production
- Store tokens securely on the client side
- Implement rate limiting on auth endpoints (future)
- Consider adding MFA for production (future)

## Next Steps

After validating authentication works:

1. Test Pack Management API with authentication
2. Implement additional CRUD APIs
3. Add RBAC permission checking (Phase 2.13)
4. Add integration tests
5. Implement token revocation for logout

## Resources

- [Authentication Documentation](./authentication.md)
- [API Documentation](../README.md)
- [JWT.io](https://jwt.io) - JWT token decoder/debugger
