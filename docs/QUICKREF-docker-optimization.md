# Quick Reference: Docker Build Optimization

## TL;DR

**Problem**: Changing any Rust crate rebuilds all services (~5 minutes each)
**Solution**: Use optimized Dockerfiles that only copy needed crates (~30 seconds)

## Quick Start

### Option 1: Use Optimized Dockerfiles (Recommended)

Update `docker-compose.yaml` to use the new Dockerfiles:

```yaml
# For main services (api, executor, sensor, notifier)
services:
  api:
    build:
      dockerfile: docker/Dockerfile.optimized  # Changed
      
  executor:
    build:
      dockerfile: docker/Dockerfile.optimized  # Changed

  sensor:
    build:
      dockerfile: docker/Dockerfile.optimized  # Changed

  notifier:
    build:
      dockerfile: docker/Dockerfile.optimized  # Changed

# For worker services
  worker-shell:
    build:
      dockerfile: docker/Dockerfile.worker.optimized  # Changed
      
  worker-python:
    build:
      dockerfile: docker/Dockerfile.worker.optimized  # Changed
      
  worker-node:
    build:
      dockerfile: docker/Dockerfile.worker.optimized  # Changed
      
  worker-full:
    build:
      dockerfile: docker/Dockerfile.worker.optimized  # Changed
```

### Option 2: Replace Existing Dockerfiles

```bash
# Backup originals
cp docker/Dockerfile docker/Dockerfile.old
cp docker/Dockerfile.worker docker/Dockerfile.worker.old

# Replace with optimized versions
mv docker/Dockerfile.optimized docker/Dockerfile
mv docker/Dockerfile.worker.optimized docker/Dockerfile.worker

# No docker-compose.yaml changes needed
```

## Performance Comparison

| Scenario | Before | After |
|----------|--------|-------|
| Change API code | ~5 min | ~30 sec |
| Change worker code | ~5 min | ~30 sec |
| Change common crate | ~5 min × 7 services | ~2 min × 7 services |
| Parallel build (4 services) | ~20 min (serialized) | ~5 min (concurrent) |
| Add dependency | ~5 min | ~3 min |
| Clean build | ~5 min | ~5 min |

## How It Works

### Old Dockerfile (Unoptimized)
```dockerfile
COPY crates/ ./crates/              # ❌ Copies ALL crates
RUN cargo build --release           # ❌ Rebuilds everything
```
**Result**: Changing `api/main.rs` invalidates layers for ALL services

### New Dockerfile (Optimized)
```dockerfile
# Stage 1: Cache dependencies
COPY crates/*/Cargo.toml            # ✅ Only manifest files
RUN --mount=type=cache,sharing=shared,... \
    cargo build (with dummy src)    # ✅ Cache dependencies

# Stage 2: Build service
COPY crates/common/ ./crates/common/    # ✅ Shared code
COPY crates/api/ ./crates/api/          # ✅ Only this service
RUN --mount=type=cache,id=target-builder-api,... \
    cargo build --release               # ✅ Only recompile changed code
```
**Result**: Changing `api/main.rs` only rebuilds API service

**Optimized Cache Strategy**:
- Registry/git caches use `sharing=shared` (concurrent-safe)
- Target caches use service-specific IDs (no conflicts)
- **4x faster parallel builds** than old `sharing=locked` strategy
- See `docs/QUICKREF-buildkit-cache-strategy.md` for details

## Testing the Optimization

```bash
# 1. Clean build (first time)
docker compose build --no-cache api
# Expected: ~5-6 minutes

# 2. Change API code
echo "// test" >> crates/api/src/main.rs
docker compose build api
# Expected: ~30 seconds ✅

# 3. Verify worker unaffected
docker compose build worker-shell
# Expected: ~5 seconds (cached) ✅
```

## When to Use Each Dockerfile

### Use Optimized (`Dockerfile.optimized`)
- ✅ Active development with frequent code changes
- ✅ CI/CD pipelines (save time and costs)
- ✅ Multi-service workspaces
- ✅ When you need fast iteration

### Use Original (`Dockerfile`)
- ✅ Simple one-off builds
- ✅ When Dockerfile complexity is a concern
- ✅ Infrequent builds where speed doesn't matter

## Adding New Crates

When you add a new crate to the workspace, update the optimized Dockerfiles:

```dockerfile
# In BOTH Dockerfile.optimized stages (planner AND builder):

# 1. Copy the manifest
COPY crates/new-service/Cargo.toml ./crates/new-service/Cargo.toml

# 2. Create dummy source (planner stage only)
RUN mkdir -p crates/new-service/src && echo "fn main() {}" > crates/new-service/src/main.rs
```

## Common Issues

### "crate not found" during build
**Fix**: Add the crate's `Cargo.toml` to COPY instructions in optimized Dockerfile

### Changes not showing up
**Fix**: Force rebuild: `docker compose build --no-cache <service>`

### Still slow after optimization
**Check**: Are you using the optimized Dockerfile? Verify in `docker-compose.yaml`

## BuildKit Cache Mounts

The optimized Dockerfiles use BuildKit cache mounts for extra speed:

```dockerfile
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build
```

**Automatically enabled** with `docker compose` - no configuration needed!

**Optimized sharing strategy**:
- `sharing=shared` for registry/git (concurrent builds safe)
- Service-specific cache IDs for target directory (no conflicts)
- Result: 4x faster parallel builds

## Summary

**Before**: 
- `COPY crates/ ./crates/` → All services rebuild on any change → 5 min/service
- `sharing=locked` cache mounts → Serialized parallel builds → 4x slower

**After**: 
- `COPY crates/${SERVICE}/` → Only changed service rebuilds → 30 sec/service
- `sharing=shared` + cache IDs → Concurrent parallel builds → 4x faster

**Savings**: 
- 90% faster incremental builds for code changes
- 75% faster parallel builds (4 services concurrently)

## See Also

- Full documentation: `docs/docker-layer-optimization.md`
- Cache strategy: `docs/QUICKREF-buildkit-cache-strategy.md`
- Original Dockerfiles: `docker/Dockerfile.old`, `docker/Dockerfile.worker.old`
- Docker Compose: `docker-compose.yaml`
