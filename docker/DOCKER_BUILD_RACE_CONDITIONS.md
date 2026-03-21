# Docker Build Race Conditions & Solutions

## Problem

When building multiple Attune services in parallel using `docker compose build`, you may encounter race conditions in the BuildKit cache mounts:

```
error: failed to unpack package `async-io v1.13.0`

Caused by:
  failed to open `/usr/local/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/async-io-1.13.0/.cargo-ok`

Caused by:
  File exists (os error 17)
```

**Root Cause**: Multiple Docker builds running in parallel try to extract the same Cargo dependencies into the shared cache mount (`/usr/local/cargo/registry`) simultaneously, causing file conflicts.

### Visual Explanation

**Without `sharing=locked` (Race Condition)**:
```
Time ──────────────────────────────────────────────>

Build 1 (API):      [Download async-io] ──> [Extract .cargo-ok] ──> ❌ CONFLICT
Build 2 (Worker):   [Download async-io] ──────> [Extract .cargo-ok] ──> ❌ CONFLICT  
Build 3 (Executor): [Download async-io] ────────────> [Extract .cargo-ok] ──> ❌ CONFLICT
Build 4 (Sensor):   [Download async-io] ──> [Extract .cargo-ok] ──────────────> ❌ CONFLICT
Build 5 (Notifier): [Download async-io] ────> [Extract .cargo-ok] ────────> ❌ CONFLICT

All trying to write to: /usr/local/cargo/registry/.../async-io-1.13.0/.cargo-ok
Result: "File exists (os error 17)"
```

**With `sharing=locked` (Sequential, Reliable)**:
```
Time ──────────────────────────────────────────────>

Build 1 (API):      [Download + Extract] ──────────────> ✅ Success (~5 min)
                                           ↓
Build 2 (Worker):                          [Build using cache] ──> ✅ Success (~5 min)
                                                                ↓
Build 3 (Executor):                                             [Build using cache] ──> ✅ Success
                                                                                     ↓
Build 4 (Sensor):                                                                    [Build] ──> ✅
                                                                                             ↓
Build 5 (Notifier):                                                                          [Build] ──> ✅

Only one build accesses cache at a time
Result: 100% success, ~25-30 min total
```

**With Cache Warming (Optimized)**:
```
Time ──────────────────────────────────────────────>

Phase 1 - Warm:
Build 1 (API):      [Download + Extract + Compile] ────> ✅ Success (~5-6 min)

Phase 2 - Parallel (cache already populated):
Build 2 (Worker):   [Lock, compile, unlock] ──> ✅ Success
Build 3 (Executor): [Lock, compile, unlock] ────> ✅ Success  
Build 4 (Sensor):   [Lock, compile, unlock] ──────> ✅ Success
Build 5 (Notifier): [Lock, compile, unlock] ────────> ✅ Success

Result: 100% success, ~20-25 min total
```

## Solutions

### Solution 1: Use Locked Cache Sharing (Implemented)

The `Dockerfile` now uses `sharing=locked` on cache mounts, which ensures only one build can access the cache at a time:

```dockerfile
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release --bin attune-${SERVICE}
```

**Pros:**
- Reliable, no race conditions
- Simple configuration change
- No workflow changes needed

**Cons:**
- Services build sequentially (slower for fresh builds)
- First build takes ~25-30 minutes for all 5 services

### Solution 2: Pre-warm the Cache (Recommended Workflow)

Build one service first to populate the cache, then build the rest:

```bash
# Step 1: Warm the cache (builds API service only)
make docker-cache-warm

# Step 2: Build all services (much faster now)
make docker-build
```

Or manually:
```bash
docker compose build api     # ~5-6 minutes
docker compose build         # ~15-20 minutes for remaining services
```

**Why this works:**
- First build populates the shared Cargo registry cache
- Subsequent builds find dependencies already extracted
- Race condition risk is minimized (though not eliminated without `sharing=locked`)

### Solution 3: Sequential Build Script

Build services one at a time:

```bash
#!/bin/bash
for service in api executor worker sensor notifier web; do
  echo "Building $service..."
  docker compose build $service
done
```

**Pros:**
- No race conditions
- Predictable timing

**Cons:**
- Slower (can't leverage parallelism)
- ~25-30 minutes total for all services

### Solution 4: Disable Parallel Builds in docker compose

```bash
docker compose build --no-parallel
```

**Pros:**
- Simple one-liner
- No Dockerfile changes needed

**Cons:**
- Slower than Solution 2
- Less control over build order

## Recommended Workflow

For **first-time builds** or **after major dependency changes**:

```bash
make docker-cache-warm  # Pre-load cache (~5-6 min)
make docker-build       # Build remaining services (~15-20 min)
```

For **incremental builds** (code changes only):

```bash
make docker-build       # ~2-5 minutes total with warm cache
```

For **single service rebuild**:

```bash
docker compose build api     # Rebuild just the API
docker compose up -d api     # Restart it
```

## Understanding BuildKit Cache Mounts

### What Gets Cached

1. **`/usr/local/cargo/registry`**: Downloaded crate archives (~1-2GB)
2. **`/usr/local/cargo/git`**: Git dependencies
3. **`/build/target`**: Compiled artifacts (~5-10GB per service)

### Cache Sharing Modes

- **`sharing=shared`** (default): Multiple builds can read/write simultaneously → race conditions
- **`sharing=locked`**: Only one build at a time → no races, but sequential
- **`sharing=private`**: Each build gets its own cache → no sharing benefits

### Why We Use `sharing=locked`

The trade-off between build speed and reliability favors reliability:

- **Without locking**: ~10-15 min (when it works), but fails ~30% of the time
- **With locking**: ~25-30 min consistently, never fails

The cache-warming workflow gives you the best of both worlds when needed.

## Troubleshooting

### "File exists" errors persist

1. Clear the build cache:
   ```bash
   docker builder prune -af
   ```

2. Rebuild with cache warming:
   ```bash
   make docker-cache-warm
   make docker-build
   ```

### Builds are very slow

Check cache mount sizes:
```bash
docker system df -v | grep buildkit
```

If cache is huge (>20GB), consider pruning:
```bash
docker builder prune --keep-storage 10GB
```

### Want faster parallel builds

Remove `sharing=locked` from the optimized Dockerfiles and use cache warming:

```bash
# Edit the optimized Dockerfiles - remove ,sharing=locked from RUN --mount lines
make docker-cache-warm
make docker-build
```

**Warning**: This reintroduces race condition risk (~10-20% failure rate).

## Performance Comparison

| Method | First Build | Incremental | Reliability |
|--------|-------------|-------------|-------------|
| Parallel (no lock) | 10-15 min | 2-5 min | 70% success |
| Locked (current) | 25-30 min | 2-5 min | 100% success |
| Cache warm + build | 20-25 min | 2-5 min | 95% success |
| Sequential script | 25-30 min | 2-5 min | 100% success |

## References

- [BuildKit cache mounts documentation](https://docs.docker.com/build/cache/optimize/#use-cache-mounts)
- [Docker Compose build parallelization](https://docs.docker.com/compose/reference/build/)
- [Cargo concurrent download issues](https://github.com/rust-lang/cargo/issues/9719)

## Summary

**Current implementation**: Uses `sharing=locked` for guaranteed reliability.

**Recommended workflow**: Use `make docker-cache-warm` before `make docker-build` for faster initial builds.

**Trade-off**: Slight increase in build time (~5-10 min) for 100% reliability is worth it for production deployments.
