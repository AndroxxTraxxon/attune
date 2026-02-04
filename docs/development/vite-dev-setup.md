# Vite Dev Server Setup for Local Development

## Overview

This guide explains how to run the Vite development server locally while using the Docker containerized backend services (API, database, workers, etc.). This setup provides the best development experience with hot-module reloading and fast iteration on the frontend.

## Architecture

In this development setup:

- **Backend Services**: Run in Docker containers (API, database, RabbitMQ, workers, etc.)
- **Web UI**: Run locally with Vite dev server on port 3001
- **CORS**: Configured to allow cross-origin requests from local Vite dev server

```
┌─────────────────────────────────────────────────────────────┐
│                     Local Machine                            │
│                                                              │
│  ┌─────────────────┐         ┌──────────────────────────┐  │
│  │  Vite Dev Server│◄────────┤   Browser                │  │
│  │  (Port 3001)    │  HMR    │   http://localhost:3001  │  │
│  │  Hot Reload ✨  │         └───────┬──────────────────┘  │
│  └────────┬────────┘                 │                      │
│           │                          │ API Requests         │
│           │ Proxy                    │ /api/* /auth/*       │
│           │ /api → 8080              │                      │
│           │ /auth → 8080             ▼                      │
│           │              ┌─────────────────────────┐        │
│           └─────────────►│  Docker API Service     │        │
│                          │  (Port 8080)            │        │
│                          │  CORS enabled ✓         │        │
│                          └───────┬─────────────────┘        │
│                                  │                          │
│                          ┌───────▼─────────────────┐        │
│                          │  PostgreSQL             │        │
│                          │  RabbitMQ               │        │
│                          │  Workers                │        │
│                          │  Other services...      │        │
│                          └─────────────────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

## Why Port 3001?

The Docker compose setup exposes the production web container (NGINX) on port 3000. When you run Vite dev server, it tries to bind to port 3000 first but will automatically fall back to 3001 if 3000 is taken. We've configured Vite to explicitly use 3001 to avoid conflicts.

## Setup Instructions

### 1. Start Backend Services with Docker

Start all backend services (excluding the web container):

```bash
# Start all backend services
docker compose up -d postgres rabbitmq redis api executor worker-shell worker-python sensor

# Or start everything and then stop the web container
docker compose up -d
docker compose stop web
```

### 2. Verify Backend Services

Check that the API is running:

```bash
# Health check
curl http://localhost:8080/health

# Should return: {"status":"ok"}
```

### 3. Start Vite Dev Server

In a separate terminal:

```bash
cd web
npm install  # If first time or dependencies changed
npm run dev
```

The Vite dev server will start on `http://localhost:3001` (or the next available port).

### 4. Access the Application

Open your browser to:

```
http://localhost:3001
```

You should see the Attune web UI with:
- ✅ Fast hot-module reloading (HMR)
- ✅ API requests proxied to Docker backend
- ✅ No CORS errors
- ✅ Full authentication flow working

## Configuration Details

### Vite Configuration (`web/vite.config.ts`)

```typescript
export default defineConfig({
  server: {
    host: "127.0.0.1",
    port: 3001,
    strictPort: false, // Allow fallback to next port if 3001 is taken
    proxy: {
      "/api": {
        target: "http://localhost:8080",
        changeOrigin: true,
      },
      "/auth": {
        target: "http://localhost:8080",
        changeOrigin: true,
      },
    },
  },
});
```

### CORS Configuration

The API service is configured to allow requests from Vite dev server ports:

**config.docker.yaml:**
```yaml
server:
  cors_origins:
    - http://localhost:3000
    - http://localhost:3001
    - http://localhost:3002
    - http://localhost:5173
    - http://127.0.0.1:3000
    - http://127.0.0.1:3001
    - http://127.0.0.1:3002
    - http://127.0.0.1:5173
```

**config.development.yaml:**
```yaml
server:
  cors_origins:
    - http://localhost:3000
    - http://localhost:3001
    - http://localhost:3002
    - http://localhost:5173
    - http://127.0.0.1:3000
    - http://127.0.0.1:3001
    - http://127.0.0.1:3002
    - http://127.0.0.1:5173
```

Multiple ports are included to support:
- Port 3001: Primary Vite dev server port
- Port 3002: Fallback if 3001 is taken
- Port 5173: Alternative Vite default port
- Port 3000: Docker web container (for comparison)

## Troubleshooting

### CORS Errors

**Symptom:**
```
Access to XMLHttpRequest at 'http://localhost:8080/api/...' from origin 'http://localhost:3001' 
has been blocked by CORS policy
```

**Solutions:**

1. **Restart API service** after config changes:
   ```bash
   docker compose restart api
   ```

2. **Verify CORS origins** in API logs:
   ```bash
   docker compose logs api | grep -i cors
   ```

3. **Check your browser's dev tools** Network tab for the actual origin being sent

### Port Already in Use

**Symptom:**
```
Port 3001 is already in use
```

**Solutions:**

1. **Let Vite use next available port:**
   Vite will automatically try 3002, 3003, etc.

2. **Kill process using the port:**
   ```bash
   # Find process
   lsof -i :3001
   
   # Kill it
   kill -9 <PID>
   ```

3. **Use a specific port:**
   ```bash
   npm run dev -- --port 3005
   ```
   
   Make sure this port is in the CORS allowed origins list!

### API Requests Failing

**Symptom:**
API requests return 404 or fail to reach the backend.

**Solutions:**

1. **Verify API is running:**
   ```bash
   curl http://localhost:8080/health
   ```

2. **Check proxy configuration** in `vite.config.ts`

3. **Inspect browser Network tab** to see if requests are being proxied correctly

### Hot Module Reloading Not Working

**Symptom:**
Changes to React components don't auto-refresh.

**Solutions:**

1. **Check Vite dev server output** for errors

2. **Clear browser cache** and hard refresh (Ctrl+Shift+R / Cmd+Shift+R)

3. **Restart Vite dev server:**
   ```bash
   # Stop with Ctrl+C, then restart
   npm run dev
   ```

### WebSocket Connection Issues

**Symptom:**
Real-time updates (execution status, etc.) not working.

**Note:** The notifier service WebSocket endpoint is NOT proxied through Vite. If you need WebSocket functionality, you may need to:

1. Access notifier directly at `ws://localhost:8081`
2. Or add WebSocket proxy configuration to Vite config

## Development Workflow

### Typical Workflow

1. **Start backend once** (usually in the morning):
   ```bash
   docker compose up -d postgres rabbitmq redis api executor worker-shell
   ```

2. **Start Vite dev server** when working on frontend:
   ```bash
   cd web && npm run dev
   ```

3. **Make changes** to React components, TypeScript files, etc.
   - Changes are instantly reflected (HMR)
   - No page reload needed for most changes

4. **Stop Vite** when done (Ctrl+C)
   - Backend services can keep running

5. **Stop backend** when completely done:
   ```bash
   docker compose down
   ```

### Testing API Changes

If you're also developing backend features:

1. Make changes to Rust code
2. Rebuild and restart API:
   ```bash
   docker compose up -d --build api
   ```
3. Vite dev server will continue running
4. Frontend will automatically use new API

### Switching Between Environments

**Use Vite dev server (development):**
- Fastest iteration
- Hot module reloading
- Source maps for debugging
- Best for UI development

**Use Docker web container (production-like):**
```bash
docker compose up -d web
# Access at http://localhost:3000
```
- Tests production build
- Tests NGINX configuration
- No HMR (full page reloads)
- Best for integration testing

## Performance Tips

1. **Keep backend services running** between sessions to avoid startup time

2. **Use `--build` flag selectively** when rebuilding:
   ```bash
   # Only rebuild changed services
   docker compose up -d --build api
   ```

3. **Clear Vite cache** if you encounter weird issues:
   ```bash
   rm -rf web/node_modules/.vite
   ```

## Comparison: Dev Server vs Production Build

| Feature | Vite Dev Server | Docker Web Container |
|---------|----------------|----------------------|
| **Port** | 3001 (local) | 3000 (Docker) |
| **Hot Reload** | ✅ Yes | ❌ No |
| **Build Time** | ⚡ Instant | 🐢 ~30s |
| **Source Maps** | ✅ Yes | ⚠️ Optional |
| **NGINX** | ❌ No | ✅ Yes |
| **Production-like** | ❌ No | ✅ Yes |
| **Best For** | Active development | Testing deployment |

## Additional Resources

- [Vite Documentation](https://vitejs.dev/)
- [Vite Server Options](https://vitejs.dev/config/server-options.html)
- [Docker Compose Documentation](https://docs.docker.com/compose/)
- [Attune Architecture Docs](../architecture/)

## Summary

For rapid frontend development:

```bash
# Terminal 1: Start backend (once)
docker compose up -d postgres rabbitmq redis api executor worker-shell

# Terminal 2: Start Vite dev server (restart as needed)
cd web && npm run dev

# Browser: http://localhost:3001
# Enjoy fast hot-module reloading! ⚡
```
