# Vite Dev Server CORS Configuration

**Date**: 2026-01-27  
**Status**: ✅ Complete

## Overview

Configured the development environment to support rapid frontend iteration using Vite's development server (port 3001) alongside Docker containerized backend services. This eliminates CORS issues and provides hot-module reloading (HMR) for instant feedback during UI development.

## Problem Statement

The web container in Docker Compose was exposed on port 3000, causing the Vite dev server to fall back to port 3001. This resulted in CORS errors when the frontend tried to communicate with the API service running in Docker, as port 3001 was not in the allowed origins list.

## Solution

### 1. Vite Configuration Updates

**File**: `attune/web/vite.config.ts`

- Set explicit host binding to `127.0.0.1`
- Set primary port to `3001` (avoiding conflict with Docker web container on 3000)
- Enabled `strictPort: false` to allow automatic fallback to 3002, 3003, etc.
- Maintained proxy configuration for `/api` and `/auth` routes to backend (port 8080)

### 2. CORS Configuration Updates

**Files**:
- `attune/config.docker.yaml`
- `attune/config.development.yaml`

Added Vite dev server ports to CORS allowed origins:
- `http://localhost:3001` - Primary Vite dev server port
- `http://localhost:3002` - Fallback port
- `http://127.0.0.1:3001` - IPv4 explicit binding
- `http://127.0.0.1:3002` - IPv4 fallback
- `http://localhost:5173` - Alternative Vite default port
- `http://127.0.0.1:5173` - IPv4 alternative

**Note**: Kept CORS configuration purely in config files, not hardcoded in middleware layer, maintaining configuration-driven approach.

### 3. Documentation Created

Created comprehensive documentation for developers:

**`docs/development/vite-dev-setup.md`** (359 lines):
- Architecture diagram showing local Vite + Docker backend
- Detailed setup instructions
- Configuration explanations
- Troubleshooting guide (CORS, ports, API requests, HMR, WebSockets)
- Development workflow best practices
- Performance tips
- Comparison table: Dev server vs Production build

**`docs/development/QUICKSTART-vite.md`** (238 lines):
- TL;DR quick start commands
- Common commands reference
- Default ports table
- Testing login credentials
- Common issues and fixes
- Development workflow (morning routine, during dev, end of day)
- Architecture diagram
- Pro tips

## Key Features

### Development Workflow Improvements

1. **Hot Module Reloading**: Changes to React components appear instantly without page reload
2. **Fast Iteration**: No need to rebuild Docker web container on every change
3. **Backend Services**: Run once in Docker, keep running between frontend development sessions
4. **No CORS Errors**: Properly configured allowed origins
5. **Flexible Ports**: Automatic fallback if primary port is unavailable

### Port Strategy

| Service | Port | Purpose |
|---------|------|---------|
| Vite Dev Server | 3001 | Frontend development with HMR |
| Docker Web (NGINX) | 3000 | Production-like testing |
| API Service | 8080 | Backend REST API |
| Notifier WebSocket | 8081 | Real-time notifications |

### Developer Experience

**Before**:
```bash
# Had to rebuild entire web container for every change
docker compose up -d --build web
# Wait 30+ seconds for build
# No HMR, full page reloads required
```

**After**:
```bash
# Start backend once
docker compose up -d postgres rabbitmq redis api executor worker-shell sensor

# Start Vite dev server
cd web && npm run dev

# Instant hot-module reloading on every save! ⚡
```

## Technical Details

### Vite Proxy Configuration

The Vite dev server proxies API requests to avoid CORS:

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

### CORS Middleware

The API's CORS middleware (`crates/api/src/middleware/cors.rs`) remains unchanged, using configuration-driven allowed origins from the config files. Default development origins are only used as fallback when no config is provided.

### Configuration Files

Two config files support different deployment scenarios:

- **`config.docker.yaml`**: Used when running all services in Docker (including API)
- **`config.development.yaml`**: Used when running API service locally (native Rust)

Both now include Vite dev server ports in CORS allowed origins.

## Build Verification

✅ Rust compilation successful:
```bash
cargo check --package attune-api
# Finished in 9.60s
```

✅ TypeScript/Vite build successful:
```bash
cd web && npm run build
# Built successfully
```

## Troubleshooting Coverage

Documentation includes solutions for:

1. **CORS Errors**: How to restart API, verify CORS config, check browser dev tools
2. **Port Conflicts**: How to use fallback ports, kill processes, specify custom ports
3. **API Request Failures**: Verify API running, check proxy config, inspect network tab
4. **HMR Not Working**: Check Vite output, clear cache, restart dev server
5. **WebSocket Issues**: Note about direct connection vs proxy configuration

## Benefits

### Development Speed
- **Before**: 30+ second wait for Docker rebuild on every change
- **After**: Instant (<1s) feedback with HMR

### Resource Usage
- Backend services can stay running, only restart when needed
- Frontend development doesn't require full Docker rebuild

### Debugging
- Source maps enabled by default in dev mode
- React DevTools work properly
- Better error messages and stack traces

### Flexibility
- Can switch between Vite dev (development) and Docker web (production testing)
- Easy to test production builds before deployment

## Usage Examples

### Standard Development Session

```bash
# Morning: Start backend services
docker compose up -d postgres rabbitmq redis api executor worker-shell sensor

# Start frontend development
cd web && npm run dev

# Browser: http://localhost:3001
# Make changes → See results instantly

# End of day: Stop Vite (Ctrl+C)
# Optional: docker compose stop (or keep running)
```

### Testing Backend Changes

```bash
# Make changes to Rust code
# Rebuild only API service
docker compose up -d --build api

# Vite dev server continues running
# No frontend restart needed
```

### Switching to Production Build

```bash
# Test production build locally
cd web
npm run build
npm run preview

# Or use Docker web container
docker compose up -d web
# http://localhost:3000
```

## Files Modified

1. `attune/web/vite.config.ts` - Explicit port 3001, host binding
2. `attune/config.docker.yaml` - Added Vite dev server ports to CORS
3. `attune/config.development.yaml` - Added Vite dev server ports to CORS

## Files Created

1. `attune/docs/development/vite-dev-setup.md` - Comprehensive setup guide
2. `attune/docs/development/QUICKSTART-vite.md` - Quick reference guide
3. `attune/work-summary/vite-dev-server-cors-setup.md` - This document

## Next Steps (Optional Enhancements)

1. **WebSocket Proxy**: Add WebSocket proxy to Vite config for notifier service
2. **Environment Detection**: Auto-detect available port and update CORS dynamically
3. **Docker Compose Override**: Create `docker-compose.override.yml` to exclude web service by default for local dev
4. **CLI Helper**: Script to start backend + Vite dev server with one command
5. **Port Persistence**: Store Vite port preference in localStorage or config file

## Testing

Verified the following scenarios:

✅ Vite dev server starts on port 3001  
✅ No CORS errors when accessing API endpoints  
✅ Hot module reloading works correctly  
✅ Authentication flow works end-to-end  
✅ API proxy correctly forwards requests  
✅ Port fallback works (3001 → 3002 → 3003...)  
✅ Both config files have correct CORS origins  
✅ Documentation builds successfully  

## Conclusion

The Vite dev server is now properly configured for local development alongside Docker backend services. Developers can enjoy fast iteration with hot-module reloading while maintaining full access to the containerized backend. The comprehensive documentation ensures smooth onboarding and troubleshooting.

**Recommended workflow**: Use Vite dev server (port 3001) for active frontend development, and Docker web container (port 3000) for production-like integration testing.