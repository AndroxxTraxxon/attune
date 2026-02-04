# Token Refresh System - Quick Reference

**Last Updated:** 2025-01-27  
**Component:** Web UI Authentication

## Overview

The web UI implements automatic and proactive JWT token refresh to provide seamless authentication for active users.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  User Activity → API Request                        │
│         ↓                                            │
│  Axios Interceptor (adds JWT)                       │
│         ↓                                            │
│  Server Response                                    │
│    ├─ 200 OK → Continue                            │
│    ├─ 401 Unauthorized → Auto-refresh & retry      │
│    └─ 403 Forbidden → Show permission error        │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│  Background: Token Monitor (every 60s)              │
│         ↓                                            │
│  Token expires in < 5 min?                          │
│    ├─ Yes → Proactive refresh                      │
│    └─ No → Continue monitoring                     │
└─────────────────────────────────────────────────────┘
```

## Key Components

### 1. API Wrapper (`web/src/lib/api-wrapper.ts`)
- **Purpose**: Configure axios with token refresh interceptors
- **Features**:
  - Global axios defaults configuration
  - Request interceptor (adds token)
  - Response interceptor (handles 401/403)
  - Proactive refresh monitor

### 2. ErrorDisplay Component (`web/src/components/common/ErrorDisplay.tsx`)
- **Purpose**: User-friendly error messages
- **Distinguishes**:
  - 401: "Session expired" (handled automatically)
  - 403: "Access denied - insufficient permissions"
  - Other: Generic error with details

### 3. Auth Context (`web/src/contexts/AuthContext.tsx`)
- **Purpose**: Manage authentication state
- **Lifecycle**:
  - `user` set → Start token refresh monitor
  - `user` cleared → Stop token refresh monitor

## Token Lifecycle

### Access Token
- **Duration**: 1 hour (configured on backend)
- **Storage**: `localStorage.getItem('access_token')`
- **Refresh Trigger**: Automatic on 401 response
- **Proactive Refresh**: 5 minutes before expiration

### Refresh Token
- **Duration**: 7 days (configured on backend)
- **Storage**: `localStorage.getItem('refresh_token')`
- **Used**: To obtain new access token
- **Rotation**: Optional (backend can return new refresh token)

## Configuration

### Proactive Refresh Settings
```typescript
// File: web/src/lib/api-wrapper.ts

// Check every 60 seconds
const MONITOR_INTERVAL = 60000; // ms

// Refresh if expiring within 5 minutes
const REFRESH_THRESHOLD = 300; // seconds
```

### API Endpoint
```typescript
// Refresh endpoint
POST /auth/refresh
Content-Type: application/json

{
  "refresh_token": "..."
}

// Response
{
  "data": {
    "access_token": "...",
    "refresh_token": "..." // Optional - for rotation
  }
}
```

## Error Handling

### 401 Unauthorized (Token Expired/Invalid)
```typescript
// Automatic handling:
1. Interceptor detects 401
2. Attempts token refresh with refresh_token
3. On success: Retry original request
4. On failure: Clear tokens, redirect to /login
```

### 403 Forbidden (Insufficient Permissions)
```typescript
// Manual handling in components:
<ErrorDisplay error={error} />
// Shows: "Access Denied - You do not have permission..."
```

### Network/Server Errors
```typescript
// Generic error display:
<ErrorDisplay 
  error={error} 
  showRetry={true}
  onRetry={() => refetch()}
/>
```

## Usage in Components

### Detecting Error Types
```typescript
// In React components using TanStack Query
const { data, error, isLoading } = useActions();

if (error) {
  // ErrorDisplay component handles type detection
  return <ErrorDisplay error={error} />;
}
```

### Custom Error Handling
```typescript
// Check for 403 errors
const is403 = error?.response?.status === 403 || 
              error?.isAuthorizationError;

if (is403) {
  // Show permission-specific UI
}

// Check for 401 errors (rare - usually handled by interceptor)
const is401 = error?.response?.status === 401;
```

## Debugging

### Console Logs
```bash
# Initialization
🔧 Initializing API wrapper
✓ Axios defaults configured with interceptors
✓ API wrapper initialized

# Token Refresh
🔄 Access token expired, attempting refresh...
✓ Token refreshed successfully

# Monitor
🔄 Starting token refresh monitor
✓ Token proactively refreshed
⏹️  Stopping token refresh monitor

# Errors
⚠️ No refresh token available, redirecting to login
Token refresh failed, clearing session and redirecting to login
Access forbidden - insufficient permissions for this resource
```

### Browser DevTools
```bash
# Check tokens
Application → Local Storage → localhost
- access_token: "eyJ..."
- refresh_token: "eyJ..."

# Watch refresh requests
Network → Filter: refresh
- POST /auth/refresh
- Status: 200 OK
- Response: { data: { access_token, refresh_token } }

# Monitor console
Console → Filter: Token|refresh|Unauthorized
```

## Common Scenarios

### Scenario 1: Active User
```
User logged in → Using app normally
    ↓
Every 60s: Monitor checks token expiration
    ↓
Token expires in 4 minutes
    ↓
Proactive refresh triggered
    ↓
User continues seamlessly (no interruption)
```

### Scenario 2: Idle User Returns
```
User logged in → Leaves tab idle for 70 minutes
    ↓
Access token expired (after 60 min)
    ↓
User returns, clicks action
    ↓
API returns 401
    ↓
Interceptor attempts refresh
    ↓
If refresh token valid: Success, retry request
If refresh token expired: Redirect to login
```

### Scenario 3: Permission Denied
```
User logged in → Tries restricted action
    ↓
API returns 403 Forbidden
    ↓
ErrorDisplay shows: "Access Denied"
    ↓
User sees clear message (not "Unauthorized")
```

### Scenario 4: Network Failure During Refresh
```
User action → 401 response → Refresh attempt
    ↓
Network error / API down
    ↓
Refresh fails → Tokens cleared
    ↓
Redirect to login
    ↓
SessionStorage saves current path
    ↓
After login → Redirect back to original page
```

## Testing

### Manual Test: Token Expiration
```bash
# 1. Log in to web UI
# 2. Open DevTools → Application → Local Storage
# 3. Copy access_token value
# 4. Decode at jwt.io - note expiration time
# 5. Wait until near expiration
# 6. Perform action (view page, click button)
# 7. Watch Network tab for /auth/refresh call
# 8. Verify action completes successfully
```

### Manual Test: Permission Denied
```bash
# 1. Log in as limited user
# 2. Try to access admin-only resource
# 3. Verify: See "Access Denied" (not "Unauthorized")
# 4. Verify: Amber/yellow UI (not red)
# 5. Verify: Helpful message about permissions
```

### Manual Test: Proactive Refresh
```bash
# 1. Log in
# 2. Open Console
# 3. Look for "🔄 Starting token refresh monitor"
# 4. Wait 60 seconds
# 5. If token expires within 5 min, see:
#    "✓ Token proactively refreshed"
# 6. Logout
# 7. See: "⏹️  Stopping token refresh monitor"
```

## Troubleshooting

### Issue: Redirect loop to /login
**Cause**: Both access_token and refresh_token expired  
**Solution**: Expected behavior - user must log in again

### Issue: Token not refreshing automatically
**Check**:
1. Axios interceptors configured? → See console for init logs
2. Token exists in localStorage?
3. Refresh token valid?
4. Network connectivity?
5. Backend /auth/refresh endpoint working?

### Issue: Monitor not running
**Check**:
1. User authenticated? → Monitor only runs when `user` is set
2. Check console for "Starting token refresh monitor"
3. Verify AuthContext lifecycle in React DevTools

### Issue: Wrong error message (401 vs 403)
**Check**:
1. Using ErrorDisplay component?
2. Error object has `response.status` property?
3. Interceptor properly marking 403 errors?

## Security Notes

1. **Token Storage**: Currently uses localStorage
   - ✅ Works across tabs
   - ⚠️  Vulnerable to XSS
   - 🔒 Consider httpOnly cookies for production

2. **Token Exposure**: Tokens only in Authorization header
   - ✅ Never in URL parameters
   - ✅ Not logged to console

3. **Automatic Cleanup**: Failed refresh clears all tokens
   - ✅ No stale authentication state

4. **Single Sign-Out**: Clearing tokens stops all access
   - ✅ Immediate effect

## API Requirements

The backend must provide:

1. **Login Endpoint**: Returns access_token + refresh_token
2. **Refresh Endpoint**: Accepts refresh_token, returns new access_token
3. **Token Format**: Standard JWT with `exp` claim
4. **Error Codes**: 
   - 401 for expired/invalid tokens
   - 403 for permission denied

## Related Files

- `web/src/lib/api-wrapper.ts` - Core token refresh logic
- `web/src/lib/api-client.ts` - Axios instance configuration
- `web/src/components/common/ErrorDisplay.tsx` - Error UI
- `web/src/contexts/AuthContext.tsx` - Auth state management
- `web/src/pages/auth/LoginPage.tsx` - Login with redirect

## Related Documentation

- Full details: `work-summary/2025-01-token-refresh-improvements.md`
- Authentication: `docs/authentication/authentication.md`
- Token rotation: `docs/authentication/token-rotation.md`
