# Work Summary: CORS Configuration from .env

**Date:** 2026-01-12
**Status:** ✅ Complete

## Overview

Updated the CORS middleware to load allowed origins from the `.env` configuration file instead of being hard-coded, providing flexibility for different deployment environments.

## Problem

The initial CORS implementation had two issues:

1. **Hard-coded Origins:** CORS allowed origins were hard-coded in the middleware:
   ```rust
   .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
   .allow_origin("http://localhost:8080".parse::<HeaderValue>().unwrap())
   // etc.
   ```

2. **CORS Panic:** When using `allow_credentials(true)` (required for JWT authentication), the CORS spec prohibits:
   - `allow_origin(Any)` - Cannot use wildcard with credentials
   - `allow_headers(Any)` - Cannot use wildcard headers with credentials

## Solution

### 1. Updated SecurityConfig

Added separate fields for access and refresh token expiration:
```rust
pub jwt_access_expiration: u64,    // Default: 3600 (1 hour)
pub jwt_refresh_expiration: u64,   // Default: 604800 (7 days)
```

### 2. Updated CORS Middleware

Changed `create_cors_layer()` to accept origins as parameter:
```rust
pub fn create_cors_layer(allowed_origins: Vec<String>) -> CorsLayer
```

Features:
- Accepts list of allowed origins from config
- Falls back to default development origins if empty
- Explicitly specifies headers and methods (no wildcards)
- Maintains credential support for authentication

### 3. Updated AppState

Added `cors_origins` field to store configuration:
```rust
pub struct AppState {
    pub db: PgPool,
    pub jwt_config: Arc<JwtConfig>,
    pub cors_origins: Vec<String>,  // NEW
}
```

### 4. Configuration Flow

```
.env file
  ↓
Config::load()
  ↓
config.server.cors_origins
  ↓
AppState
  ↓
Server::build_router()
  ↓
create_cors_layer(origins)
```

## Configuration

### .env File

```bash
# Leave empty for default development origins
ATTUNE__SERVER__CORS_ORIGINS=

# Or specify custom origins (comma-separated)
ATTUNE__SERVER__CORS_ORIGINS=http://localhost:3000,https://app.example.com,https://admin.example.com
```

### Default Origins (when empty)

If no origins are specified, the API uses these development defaults:
- `http://localhost:3000`
- `http://localhost:8080`
- `http://127.0.0.1:3000`
- `http://127.0.0.1:8080`

### Allowed Headers

Explicitly set to support authentication:
- `Authorization` - For Bearer tokens
- `Content-Type` - For JSON requests
- `Accept` - For content negotiation

### Allowed Methods

- `GET`
- `POST`
- `PUT`
- `DELETE`
- `PATCH`
- `OPTIONS` (for preflight requests)

## Files Modified

1. **`crates/common/src/config.rs`**
   - Split `jwt_expiration` into `jwt_access_expiration` and `jwt_refresh_expiration`

2. **`crates/api/src/middleware/cors.rs`**
   - Made CORS layer accept origins as parameter
   - Added default origins fallback
   - Fixed credentials + wildcard issue

3. **`crates/api/src/state.rs`**
   - Added `cors_origins` field to AppState

4. **`crates/api/src/server.rs`**
   - Pass `cors_origins` from state to middleware

5. **`crates/api/src/main.rs`**
   - Load CORS origins from config
   - Pass to AppState constructor
   - Added logging for CORS configuration

6. **`.env`**
   - Added `ATTUNE__SERVER__CORS_ORIGINS` configuration

7. **`.env.example`**
   - Updated JWT configuration comments
   - Added `ATTUNE__SECURITY__JWT_ACCESS_EXPIRATION`
   - Added `ATTUNE__SECURITY__JWT_REFRESH_EXPIRATION`

8. **`docs/quick-start.md`**
   - Added CORS configuration documentation

## Testing

```bash
# Start server
cargo run --bin attune-api

# Expected logs:
# INFO JWT configuration initialized (access: 3600s, refresh: 604800s)
# INFO CORS configured with default development allowed origin(s)
# INFO Server listening on 127.0.0.1:8080

# Test health check
curl http://localhost:8080/api/v1/health
# Response: {"status":"ok"}
```

## Benefits

1. **Flexibility:** Origins can be changed without recompiling
2. **Environment-specific:** Different origins for dev/staging/prod
3. **Security:** No wildcard origins with credentials
4. **Convenience:** Defaults work for local development
5. **Documentation:** Clear configuration in .env file

## Production Deployment

For production, specify exact origins:

```bash
ATTUNE__SERVER__CORS_ORIGINS=https://app.example.com,https://admin.example.com
```

**Never use** wildcards or overly permissive origins in production!

## Security Notes

- ✅ CORS credentials enabled (required for JWT auth)
- ✅ Explicit origins (no wildcards)
- ✅ Explicit headers (no wildcards)
- ✅ Explicit methods
- ✅ Configurable per environment

## Example Configurations

### Local Development (default)
```bash
ATTUNE__SERVER__CORS_ORIGINS=
```

### Development with Custom Frontend
```bash
ATTUNE__SERVER__CORS_ORIGINS=http://localhost:3000
```

### Staging Environment
```bash
ATTUNE__SERVER__CORS_ORIGINS=https://staging.example.com
```

### Production (Multiple Origins)
```bash
ATTUNE__SERVER__CORS_ORIGINS=https://app.example.com,https://admin.example.com,https://mobile.example.com
```

## Next Steps

- Consider adding origin validation in config loading
- Add metrics for CORS rejections
- Document CORS troubleshooting in API docs
- Consider per-route CORS policies for future features

## References

- CORS Spec: https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS
- Tower HTTP CORS: https://docs.rs/tower-http/latest/tower_http/cors/
- OWASP CORS Security: https://cheatsheetseries.owasp.org/cheatsheets/CORS_Cheat_Sheet.html