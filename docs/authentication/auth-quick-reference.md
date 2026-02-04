# Authentication Quick Reference

## Environment Variables

```bash
JWT_SECRET=your-secret-key-here              # Required in production!
JWT_ACCESS_EXPIRATION=3600                   # Optional (1 hour default)
JWT_REFRESH_EXPIRATION=604800                # Optional (7 days default)
```

## Endpoints

### Register New User
```http
POST /auth/register
Content-Type: application/json

{
  "login": "username",
  "password": "securepass123",
  "display_name": "Full Name"  // optional
}
```

### Login
```http
POST /auth/login
Content-Type: application/json

{
  "login": "username",
  "password": "securepass123"
}
```

### Refresh Token
```http
POST /auth/refresh
Content-Type: application/json

{
  "refresh_token": "eyJhbGc..."
}
```

### Get Current User (Protected)
```http
GET /auth/me
Authorization: Bearer <access_token>
```

### Change Password (Protected)
```http
POST /auth/change-password
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "current_password": "oldpass123",
  "new_password": "newpass456"
}
```

## Response Format

### Success (Register/Login/Refresh)
```json
{
  "data": {
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

### Success (Get Current User)
```json
{
  "data": {
    "id": 1,
    "login": "username",
    "display_name": "Full Name"
  }
}
```

### Error
```json
{
  "error": "Invalid login or password",
  "code": "UNAUTHORIZED"
}
```

## HTTP Status Codes

- `200 OK` - Success
- `400 Bad Request` - Invalid request format
- `401 Unauthorized` - Missing/invalid/expired token or bad credentials
- `403 Forbidden` - Insufficient permissions
- `409 Conflict` - Username already exists
- `422 Unprocessable Entity` - Validation error

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| Missing authentication token | No Authorization header | Add `Authorization: Bearer <token>` |
| Invalid authentication token | Malformed or wrong secret | Verify token format and JWT_SECRET |
| Authentication token expired | Access token expired | Use refresh token to get new one |
| Invalid login or password | Wrong credentials | Check username and password |
| Username already exists | Duplicate registration | Use different username |
| Validation failed | Password too short, etc. | Check validation requirements |

## Validation Rules

- **Login:** 3-255 characters
- **Password:** 8-128 characters
- **Display Name:** 0-255 characters (optional)

## cURL Examples

```bash
# Register
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{"login":"alice","password":"secure123","display_name":"Alice"}'

# Login
curl -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"login":"alice","password":"secure123"}'

# Get Current User (replace TOKEN)
curl http://localhost:8080/auth/me \
  -H "Authorization: Bearer TOKEN"

# Change Password
curl -X POST http://localhost:8080/auth/change-password \
  -H "Authorization: Bearer TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"current_password":"secure123","new_password":"newsecure456"}'

# Refresh Token
curl -X POST http://localhost:8080/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{"refresh_token":"REFRESH_TOKEN"}'
```

## Using in Route Handlers

```rust
use crate::auth::middleware::RequireAuth;

async fn protected_handler(
    RequireAuth(user): RequireAuth,
) -> Result<Json<ApiResponse<Data>>, ApiError> {
    let identity_id = user.identity_id()?;
    let login = user.login();
    
    // Your handler logic
    Ok(Json(ApiResponse::new(data)))
}
```

## Security Checklist

- [ ] Use HTTPS in production
- [ ] Set strong JWT_SECRET (256+ bits)
- [ ] Store tokens securely on client
- [ ] Implement rate limiting
- [ ] Never log tokens
- [ ] Rotate secrets periodically
- [ ] Clear tokens on logout

## Token Lifecycle

1. **Register/Login** → Receive access + refresh tokens
2. **API Call** → Use access token in Authorization header
3. **Token Expires** → Use refresh token to get new access token
4. **Refresh Expires** → User must login again

## Troubleshooting

**Server won't start?**
- Check DATABASE_URL is set
- Verify database is running
- Run migrations: `sqlx migrate run`

**Auth fails with valid credentials?**
- Check password hash in database
- Verify JWT_SECRET matches
- Check token expiration

**Debug logging:**
```bash
RUST_LOG=attune_api=debug cargo run --bin attune-api
```

## Documentation

- Full docs: `docs/authentication.md`
- Testing guide: `docs/testing-authentication.md`
- Implementation: `crates/api/src/routes/auth.rs`
