# Docker Layer Optimization Guide

## Problem Statement

When building Rust workspace projects in Docker, copying the entire `crates/` directory creates a single Docker layer that gets invalidated whenever **any file** in **any crate** changes. This means:

- **Before optimization**: Changing one line in `api/src/main.rs` invalidates layers for ALL services (api, executor, worker, sensor, notifier)
- **Impact**: Every service rebuild takes ~5-6 minutes instead of ~30 seconds
- **Root cause**: Docker's layer caching treats `COPY crates/ ./crates/` as an atomic operation

## Architecture: Packs as Volumes

**Important**: The optimized Dockerfiles do NOT copy the `packs/` directory into service images. Packs are content/configuration that should be decoupled from service binaries.

### Packs Volume Strategy
```yaml
# docker-compose.yaml
volumes:
  packs_data:  # Shared volume for all services

services:
  init-packs:  # Run-once service that populates packs_data
    volumes:
      - ./packs:/source/packs:ro        # Source packs from host
      - packs_data:/opt/attune/packs    # Copy to shared volume
      
  api:
    volumes:
      - packs_data:/opt/attune/packs:ro  # Mount packs as read-only
      
  worker:
    volumes:
      - packs_data:/opt/attune/packs:ro  # All services share same packs
```

**Benefits**:
- ✅ Update packs without rebuilding service images
- ✅ Reduce image size (packs not baked in)
- ✅ Faster builds (no pack copying during image build)
- ✅ Consistent packs across all services

## The Solution: Selective Crate Copying

The optimized Dockerfiles use a multi-stage approach that separates dependency caching from source code compilation:

### Stage 1: Planner (Dependency Caching)
```dockerfile
# Copy only Cargo.toml files (not source code)
COPY Cargo.toml Cargo.lock ./
COPY crates/common/Cargo.toml ./crates/common/Cargo.toml
COPY crates/api/Cargo.toml ./crates/api/Cargo.toml
# ... all other crate manifests

# Create dummy source files
RUN mkdir -p crates/common/src && echo "fn main() {}" > crates/common/src/lib.rs
# ... create dummies for all crates

# Build with dummy source to cache dependencies
RUN cargo build --release --bin attune-${SERVICE}
```

**Result**: This layer is only invalidated when dependencies change (Cargo.toml/Cargo.lock modifications).

### Stage 2: Builder (Selective Source Compilation)
```dockerfile
# Copy common crate (shared dependency)
COPY crates/common/ ./crates/common/

# Copy ONLY the service being built
COPY crates/${SERVICE}/ ./crates/${SERVICE}/

# Build the actual service
RUN cargo build --release --bin attune-${SERVICE}
```

**Result**: This layer is only invalidated when the specific service's code changes (or common crate changes).

### Stage 3: Runtime (No Packs Copying)
```dockerfile
# Create directories for volume mount points
RUN mkdir -p /opt/attune/packs /opt/attune/logs

# Note: Packs are NOT copied here
# They will be mounted as a volume at runtime from packs_data volume
```

**Result**: Service images contain only binaries and configs, not packs. Packs are mounted at runtime.

## Performance Comparison

### Before Optimization (Old Dockerfile)
```
Scenario: Change api/src/routes/actions.rs
- Layer invalidated: COPY crates/ ./crates/
- Rebuilds: All dependencies + all crates
- Time: ~5-6 minutes
- Size: Full dependency rebuild
```

### After Optimization (New Dockerfile)
```
Scenario: Change api/src/routes/actions.rs
- Layer invalidated: COPY crates/api/ ./crates/api/
- Rebuilds: Only attune-api binary
- Time: ~30-60 seconds
- Size: Minimal incremental compilation
```

### Dependency Change Comparison
```
Scenario: Add new dependency to Cargo.toml
- Before: ~5-6 minutes (full rebuild)
- After: ~3-4 minutes (dependency cached separately)
```

## Implementation

### Using Optimized Dockerfiles

The optimized Dockerfiles are available as:
- `docker/Dockerfile.optimized` - For main services (api, executor, sensor, notifier)
- `docker/Dockerfile.worker.optimized` - For worker services

#### Option 1: Switch to Optimized Dockerfiles (Recommended)

Update `docker-compose.yaml`:

```yaml
services:
  api:
    build:
      context: .
      dockerfile: docker/Dockerfile.optimized  # Changed from docker/Dockerfile
      args:
        SERVICE: api
```

#### Option 2: Replace Existing Dockerfiles

```bash
# Backup current Dockerfiles
cp docker/Dockerfile docker/Dockerfile.backup
cp docker/Dockerfile.worker docker/Dockerfile.worker.backup

# Replace with optimized versions
mv docker/Dockerfile.optimized docker/Dockerfile
mv docker/Dockerfile.worker.optimized docker/Dockerfile.worker
```

### Testing the Optimization

1. **Clean build (first time)**:
   ```bash
   docker compose build --no-cache api
   # Time: ~5-6 minutes (expected, building from scratch)
   ```

2. **Incremental build (change API code)**:
   ```bash
   # Edit attune/crates/api/src/routes/actions.rs
   echo "// test comment" >> crates/api/src/routes/actions.rs
   
   docker compose build api
   # Time: ~30-60 seconds (optimized, only rebuilds API)
   ```

3. **Verify other services not affected**:
   ```bash
   # The worker service should still use cached layers
   docker compose build worker-shell
   # Time: ~5 seconds (uses cache, no rebuild needed)
   ```

## How It Works: Docker Layer Caching

Docker builds images in layers, and each instruction (`COPY`, `RUN`, etc.) creates a new layer. Layers are cached and reused if:
1. The instruction hasn't changed
2. The context (files being copied) hasn't changed
3. All previous layers are still valid

### Old Approach (Unoptimized)
```
Layer 1: COPY Cargo.toml Cargo.lock
Layer 2: COPY crates/ ./crates/           ← Invalidated on ANY crate change
Layer 3: RUN cargo build                  ← Always rebuilds everything
```

### New Approach (Optimized)
```
Stage 1 (Planner):
Layer 1: COPY Cargo.toml Cargo.lock       ← Only invalidated on dependency changes
Layer 2: COPY */Cargo.toml                ← Only invalidated on dependency changes
Layer 3: RUN cargo build (dummy)          ← Caches compiled dependencies

Stage 2 (Builder):
Layer 4: COPY crates/common/              ← Invalidated on common changes
Layer 5: COPY crates/${SERVICE}/          ← Invalidated on service-specific changes
Layer 6: RUN cargo build                  ← Only recompiles changed crates
```

## BuildKit Cache Mounts

The optimized Dockerfiles also use BuildKit cache mounts for additional speedup:

```dockerfile
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=shared \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=shared \
    --mount=type=cache,target=/build/target,id=target-builder-${SERVICE} \
    cargo build --release
```

**Benefits**:
- **Cargo registry**: Downloaded crates persist between builds
- **Cargo git**: Git dependencies persist between builds
- **Target directory**: Compilation artifacts persist between builds
- **Optimized sharing**: Registry/git use `sharing=shared` for concurrent access
- **Service-specific caches**: Target directory uses unique cache IDs to prevent conflicts

**Cache Strategy**:
- **`sharing=shared`**: Registry and git caches (cargo handles concurrent access safely)
- **Service-specific IDs**: Target caches use `id=target-builder-${SERVICE}` to prevent conflicts
- **Result**: Safe parallel builds without serialization overhead (4x faster)
- **See**: `docs/QUICKREF-buildkit-cache-strategy.md` for detailed explanation

**Requirements**:
- Enable BuildKit: `export DOCKER_BUILDKIT=1`
- Or use docker-compose which enables it automatically

## Advanced: Parallel Builds

With the optimized Dockerfiles, you can safely build multiple services in parallel:

```bash
# Build all services in parallel (4 workers)
docker compose build --parallel 4

# Or build specific services
docker compose build api executor worker-shell
```

**Optimized for Parallel Builds**:
- ✅ Registry/git caches use `sharing=shared` (concurrent-safe)
- ✅ Target caches use service-specific IDs (no conflicts)
- ✅ **4x faster** than old `sharing=locked` strategy
- ✅ No race conditions or "File exists" errors

**Why it's safe**: Each service compiles different binaries (api vs executor vs worker), so their target caches don't conflict. Cargo's registry and git caches are inherently concurrent-safe.

See `docs/QUICKREF-buildkit-cache-strategy.md` for detailed explanation of the cache strategy.

## Tradeoffs and Considerations

### Advantages
- ✅ **Faster incremental builds**: 30 seconds vs 5 minutes
- ✅ **Better cache utilization**: Only rebuild what changed
- ✅ **Smaller layer diffs**: More efficient CI/CD pipelines
- ✅ **Reduced build costs**: Less CPU time in CI environments

### Disadvantages
- ❌ **More complex Dockerfiles**: Additional planner stage
- ❌ **Slightly longer first build**: Dummy compilation overhead (~30 seconds)
- ❌ **Manual manifest copying**: Need to list all crates explicitly

### When to Use
- ✅ **Active development**: Frequent code changes benefit from fast rebuilds
- ✅ **CI/CD pipelines**: Reduce build times and costs
- ✅ **Monorepo workspaces**: Multiple services sharing common code

### When NOT to Use
- ❌ **Single-crate projects**: No benefit for non-workspace projects
- ❌ **Infrequent builds**: Complexity not worth it for rare builds
- ❌ **Dockerfile simplicity required**: Stick with basic approach

## Pack Binaries

Pack binaries (like `attune-core-timer-sensor`) need to be built separately and placed in `./packs/` before starting docker-compose.

### Building Pack Binaries

Use the provided script:
```bash
./scripts/build-pack-binaries.sh
```

Or manually:
```bash
# Build pack binaries in Docker with GLIBC compatibility
docker build -f docker/Dockerfile.pack-binaries -t attune-pack-builder .

# Extract binaries
docker create --name pack-tmp attune-pack-builder
docker cp pack-tmp:/pack-binaries/attune-core-timer-sensor ./packs/core/sensors/
docker rm pack-tmp

# Make executable
chmod +x ./packs/core/sensors/attune-core-timer-sensor
```

The `init-packs` service will copy these binaries (along with other pack files) into the `packs_data` volume when docker-compose starts.

### Why Separate Pack Binaries?

- **GLIBC Compatibility**: Built in Debian Bookworm for GLIBC 2.36 compatibility
- **Decoupled Updates**: Update pack binaries without rebuilding service images
- **Smaller Service Images**: Service images don't include pack compilation stages
- **Cleaner Architecture**: Packs are content, services are runtime

## Maintenance

### Adding New Crates

When adding a new crate to the workspace:

1. **Update `Cargo.toml`** workspace members:
   ```toml
   [workspace]
   members = [
       "crates/common",
       "crates/new-service",  # Add this
   ]
   ```

2. **Update optimized Dockerfiles** (both planner and builder stages):
   ```dockerfile
   # In planner stage
   COPY crates/new-service/Cargo.toml ./crates/new-service/Cargo.toml
   RUN mkdir -p crates/new-service/src && echo "fn main() {}" > crates/new-service/src/main.rs
   
   # In builder stage
   COPY crates/new-service/Cargo.toml ./crates/new-service/Cargo.toml
   ```

3. **Test the build**:
   ```bash
   docker compose build new-service
   ```

### Updating Packs

Packs are mounted as volumes, so updating them doesn't require rebuilding service images:

1. **Update pack files** in `./packs/`:
   ```bash
   # Edit pack files
   vim packs/core/actions/my_action.yaml
   ```

2. **Rebuild pack binaries** (if needed):
   ```bash
   ./scripts/build-pack-binaries.sh
   ```

3. **Restart services** to pick up changes:
   ```bash
   docker compose restart
   ```

No image rebuild required!

## Troubleshooting

### Build fails with "crate not found"
**Cause**: Missing crate manifest in COPY instructions
**Fix**: Add the crate's Cargo.toml to both planner and builder stages

### Changes not reflected in build
**Cause**: Docker using stale cached layers
**Fix**: Force rebuild with `docker compose build --no-cache <service>`

### "File exists" errors during parallel builds
**Cause**: Cache mount conflicts
**Fix**: Already handled by `sharing=locked` in optimized Dockerfiles

### Slow builds after dependency changes
**Cause**: Expected behavior - dependencies must be recompiled
**Fix**: This is normal; optimization helps with code changes, not dependency changes

## Alternative Approaches

### cargo-chef (Not Used)
The `cargo-chef` tool provides similar optimization but requires additional tooling:
- Pros: Automatic dependency detection, no manual manifest copying
- Cons: Extra dependency, learning curve, additional maintenance

We opted for the manual approach because:
- Simpler to understand and maintain
- No external dependencies
- Full control over the build process
- Easier to debug issues

### Volume Mounts for Development
For local development, consider mounting the source as a volume:
```yaml
volumes:
  - ./crates/api:/build/crates/api
```
- Pros: Instant code updates without rebuilds
- Cons: Not suitable for production images

## References

- [Docker Build Cache Documentation](https://docs.docker.com/build/cache/)
- [BuildKit Cache Mounts](https://docs.docker.com/build/guide/mounts/)
- [Rust Docker Best Practices](https://docs.docker.com/language/rust/build-images/)
- [cargo-chef Alternative](https://github.com/LukeMathWalker/cargo-chef)

## Summary

The optimized Docker build strategy significantly reduces build times by:
1. **Separating dependency resolution from source compilation**
2. **Only copying the specific crate being built** (plus common dependencies)
3. **Using BuildKit cache mounts** to persist compilation artifacts
4. **Mounting packs as volumes** instead of copying them into images

**Key Architecture Principles**:
- **Service images**: Contain only compiled binaries and configuration
- **Packs**: Mounted as volumes, updated independently of services
- **Pack binaries**: Built separately with GLIBC compatibility
- **Volume strategy**: `init-packs` service populates shared `packs_data` volume

**Result**: 
- Incremental builds drop from 5-6 minutes to 30-60 seconds
- Pack updates don't require image rebuilds
- Service images are smaller and more focused
- Docker-based development workflows are practical for Rust workspaces