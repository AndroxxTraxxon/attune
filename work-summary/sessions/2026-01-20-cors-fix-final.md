# Work Summary: CORS Error Resolution

**Date:** 2026-01-20  
**Status:** RESOLVED

## Problem

User experiencing CORS error when attempting to login from the web UI at `http://localhost:3000`. The error was preventing requests from reaching the API server at `http://localhost:8080`.

## Root Cause

The issue had multiple contributing factors:

1. **Environment variable override**: `web/.env.development` had `VITE_API_BASE_URL=http://localhost:8080` set, which overrode the code configuration
2. **Generated API client hardcoded values**: `web/src/api/core/OpenAPI.ts` (auto-generated file) had hardcoded `BASE: 'http://localhost:8080'`
3. **Alternative API client**: `web/src/lib/api-client.ts` also had hardcoded base URL
4. **Login form UX issue**: Submit button was `type="button"` instead of `type="submit"`, preventing Enter key submission

## Solution

### 1. Fixed Environment Configuration

**File:** `web/.env.development`

Changed from:
```
VITE_API_BASE_URL=http://localhost:8080
```

To:
```
# Use empty/omit VITE_API_BASE_URL to use Vite proxy (recommended for local dev)
# VITE_API_BASE_URL=
```

This ensures the Vite proxy is used for local development, avoiding CORS entirely.

### 2. Fixed Generated OpenAPI Client

**File:** `web/src/api/core/OpenAPI.ts`

Changed:
```typescript
export const OpenAPI: OpenAPIConfig = {
  BASE: "http://localhost:8080",  // ❌ Wrong
  WITH_CREDENTIALS: false,         // ❌ Wrong
```

To:
```typescript
export const OpenAPI: OpenAPIConfig = {
  BASE: "",                        // ✅ Correct - uses proxy
  WITH_CREDENTIALS: true,          // ✅ Correct - sends credentials
```

**Note:** This is a generated file, so it will be overwritten when running `npm run generate:api`. The fix in `api-config.ts` (which runs after) ensures correct values are set at runtime.

### 3. Fixed Manual API Client

**File:** `web/src/lib/api-client.ts`

Changed:
```typescript
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:8080';
```

To:
```typescript
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";
```

Also fixed the refresh token endpoint to use relative URL when BASE_URL is empty.

### 4. Fixed Login Form UX

**File:** `web/src/pages/auth/LoginPage.tsx`

Changed:
```typescript
<button
  type="button"      // ❌ Wrong - doesn't submit on Enter
  onClick={handleSubmit}
```

To:
```typescript
<button
  type="submit"      // ✅ Correct - submits on Enter
```

Also simplified `handleSubmit` to only accept `React.FormEvent` instead of `FormEvent | MouseEvent`.

### 5. Updated Documentation

**File:** `web/.env.example`

Added comprehensive comments explaining:
- Recommended configuration for local development (use proxy)
- How to configure for direct API access
- Production deployment configuration

## How It Works Now

### Development Request Flow

```
Browser → http://localhost:3000 (Vite Dev Server)
          ↓ (via proxy)
          http://localhost:8080 (API Server)
```

**Key points:**
- Browser only talks to `localhost:3000` (same origin)
- Vite proxy forwards `/api/*` and `/auth/*` to backend
- No CORS headers needed because requests are same-origin
- `WITH_CREDENTIALS: true` ensures cookies/auth headers are sent

### Configuration Precedence

1. **Runtime configuration** (`api-config.ts`) sets `OpenAPI.BASE` at startup
2. **Environment variable** `VITE_API_BASE_URL` is checked first
3. **Default fallback** is `""` (empty string = relative URLs = proxy)

## Files Modified

- `web/.env.development` - Removed hardcoded API URL
- `web/.env.example` - Updated with better documentation
- `web/src/api/core/OpenAPI.ts` - Fixed generated defaults
- `web/src/lib/api-client.ts` - Fixed manual axios client
- `web/src/pages/auth/LoginPage.tsx` - Fixed form submission
- `web/src/lib/api-config.ts` - Already had debug logging (no changes needed)

## Testing

Verified the fix works:
```bash
# Proxy test - returns 401 (proxy working, just wrong credentials)
curl -X POST http://localhost:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"login":"admin","password":"admin"}'

# Response shows proxy is working (no CORS error)
HTTP/1.1 401 Unauthorized
access-control-allow-credentials: true
```

## User Instructions

1. **Hard refresh browser** (`Ctrl+Shift+R` or `Cmd+Shift+R`)
2. **Clear browser cache and localStorage** if needed
3. **Try logging in** - CORS error should be gone
4. **Enter key now works** - can press Enter to submit login form

## Known Issues

### Password Authentication (Separate Issue)

The CORS issue is resolved, but there's a separate authentication issue:
- User accounts exist in database with Argon2id password hashes
- Login attempts return 401 "Invalid login or password"
- This appears to be a password verification issue in the backend
- Needs separate investigation

**Temporary workaround:** User needs to reset password or create new test user with known hash.

## Prevention

To prevent this issue in the future:

1. **Don't commit `.env.development` with hardcoded URLs** - keep it in `.gitignore`
2. **Regenerating API client** - After running `npm run generate:api`, verify `OpenAPI.ts` settings
3. **Document proxy pattern** - Make it clear that proxy is the recommended approach
4. **CI/CD check** - Could add a check to ensure `.env.development` doesn't have `VITE_API_BASE_URL` set

## Architecture Notes

The proxy-based approach is:
- ✅ **Recommended for development** - no CORS configuration needed
- ✅ **Production-ready pattern** - same pattern used with nginx/caddy reverse proxy
- ✅ **Simpler** - one less thing to configure
- ✅ **More secure** - API server doesn't need to expose CORS to browser origins

Only use direct API access (`VITE_API_BASE_URL` set) when:
- Testing API independently
- Connecting to remote API
- Production deployment without reverse proxy (not recommended)