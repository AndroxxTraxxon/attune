# Work Summary: CORS Fix and Test User Reset

**Date:** 2026-01-20  
**Session Type:** Bug Fix & User Management

## Issues Addressed

### 1. Forgot Test User Credentials
- User forgot the username/password for local testing instance
- No existing script to reset or create test users

### 2. CORS Error on Login
- Frontend making requests to `http://localhost:8080/auth/login`
- CORS policy blocking cross-origin requests from `localhost:3000` to `localhost:8080`
- Configuration not using Vite proxy correctly

## Solutions Implemented

### 1. Test User Management Script

**Created:** `scripts/create_test_user.sh`

**Features:**
- Creates or resets admin user with default credentials (login: admin, password: admin)
- Supports custom credentials via command-line arguments
- Uses Argon2id password hashing
- Fallback to pre-generated hash for 'admin' password
- Environment variable support for database connection

**Usage:**
```bash
# Create/reset default admin user
./scripts/create_test_user.sh

# Create custom user
./scripts/create_test_user.sh myuser mypassword "My Name"
```

**Test Credentials:**
- **Login:** admin
- **Password:** admin

### 2. CORS Configuration Fix

**Problem:**
- Frontend configured to make direct requests to `localhost:8080`
- Not utilizing Vite's proxy configuration
- `WITH_CREDENTIALS` set to `false`

**Changes Made:**

#### File: `web/src/lib/api-config.ts`
- Changed `API_BASE_URL` from `"http://localhost:8080"` to `""` (empty string)
- This makes requests relative to current origin (uses Vite proxy)
- Changed `WITH_CREDENTIALS` from `false` to `true`
- Enables proper credential handling for CORS

```typescript
// Before
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "http://localhost:8080";
OpenAPI.WITH_CREDENTIALS = false;

// After
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";
OpenAPI.WITH_CREDENTIALS = true;
```

#### File: `web/vite.config.ts`
- Added `/auth` route to proxy configuration
- Previously only `/api` routes were proxied

```typescript
proxy: {
  "/api": {
    target: "http://localhost:8080",
    changeOrigin: true,
  },
  "/auth": {
    target: "http://localhost:8080",
    changeOrigin: true,
  },
}
```

### 3. Documentation

**Created:** `web/CORS-TROUBLESHOOTING.md`

Comprehensive guide covering:
- Architecture overview (proxy-based development)
- Current configuration details
- Common CORS issues and solutions
- Testing procedures
- Development workflow
- Environment variables reference
- Quick fix checklist

## Architecture Explanation

### Development Request Flow

```
Browser (localhost:3000) → Vite Dev Server (proxy) → API Server (localhost:8080)
```

**Why this works:**
- All requests appear to come from `localhost:3000` (same origin)
- Vite proxy forwards `/api/*` and `/auth/*` to backend
- No CORS issues because browser sees same-origin requests
- Backend CORS still configured for direct access if needed

### Backend CORS Configuration

**File:** `crates/api/src/middleware/cors.rs`

**Default allowed origins:**
- `http://localhost:3000` (Vite default)
- `http://localhost:5173` (Vite alternative)
- `http://localhost:8080` (API direct access)
- Plus 127.0.0.1 variants

**Settings:**
- Credentials: Enabled
- Methods: GET, POST, PUT, DELETE, PATCH, OPTIONS
- Headers: Authorization, Content-Type, Accept

## Testing

### Test User Created Successfully
```bash
$ ./scripts/create_test_user.sh
[INFO] Attune Test User Setup
[INFO] ======================
[INFO] Database: attune
[INFO] Host: localhost:5432

[INFO] Creating new user 'admin'...
INSERT 0 1
[INFO] User 'admin' created successfully!

[INFO] ======================================
[INFO] Test User Credentials:
[INFO]   Login:    admin
[INFO]   Password: admin
[INFO] ======================================
```

Verified in database:
```sql
SELECT id, login, display_name, created FROM attune.identity WHERE login='admin';
-- id | login | display_name  | created
--  2 | admin | Administrator | 2026-01-20 19:34:03
```

## Next Steps

1. **Test the CORS fix:**
   - Restart frontend dev server (`cd web && npm run dev`)
   - Browser DevTools Network tab should show requests to `localhost:3000`
   - Login should work without CORS errors

2. **Verify login flow:**
   ```bash
   curl -X POST http://localhost:3000/auth/login \
     -H 'Content-Type: application/json' \
     -d '{"login":"admin","password":"admin"}'
   ```

3. **If still having issues:**
   - Check browser console for request URLs
   - Verify Vite dev server is running on port 3000
   - Review `web/CORS-TROUBLESHOOTING.md` checklist

## Files Created/Modified

**Created:**
- `scripts/create_test_user.sh` - User management script
- `web/CORS-TROUBLESHOOTING.md` - Comprehensive CORS guide

**Modified:**
- `web/src/lib/api-config.ts` - Fixed API base URL and credentials
- `web/vite.config.ts` - Added /auth proxy route

## Impact

**Immediate:**
- Test user credentials now available (admin/admin)
- Script to reset/create users anytime
- CORS errors should be resolved

**Long-term:**
- Better development experience with proxy
- Production-ready architecture (reverse proxy pattern)
- Documentation for troubleshooting

## Notes

- Frontend dev server must be restarted to pick up Vite config changes
- The `create_test_user.sh` script is now the standard way to manage test users
- CORS configuration supports both proxy and direct access patterns
- Production deployments should use reverse proxy (nginx/caddy) for same pattern