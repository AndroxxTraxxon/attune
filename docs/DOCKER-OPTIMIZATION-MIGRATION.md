# Docker Optimization Migration Checklist

This document provides a step-by-step checklist for migrating from the old Dockerfiles to the optimized build strategy.

## Pre-Migration Checklist

- [ ] **Backup current Dockerfiles**
  ```bash
  cp docker/Dockerfile docker/Dockerfile.backup
  cp docker/Dockerfile.worker docker/Dockerfile.worker.backup
  ```

- [ ] **Review current docker-compose.yaml**
  ```bash
  cp docker-compose.yaml docker-compose.yaml.backup
  ```

- [ ] **Document current build times**
  ```bash
  # Time a clean build
  time docker compose build --no-cache api
  
  # Time an incremental build
  echo "// test" >> crates/api/src/main.rs
  time docker compose build api
  git checkout crates/api/src/main.rs
  ```

- [ ] **Ensure Docker BuildKit is enabled**
  ```bash
  docker buildx version  # Should show buildx plugin
  # BuildKit is enabled by default in docker compose
  ```

## Migration Steps

### Step 1: Build Pack Binaries

Pack binaries must be built separately and placed in `./packs/` before starting services.

- [ ] **Build pack binaries**
  ```bash
  ./scripts/build-pack-binaries.sh
  ```

- [ ] **Verify binaries exist**
  ```bash
  ls -lh packs/core/sensors/attune-core-timer-sensor
  file packs/core/sensors/attune-core-timer-sensor
  ```

- [ ] **Make binaries executable**
  ```bash
  chmod +x packs/core/sensors/attune-core-timer-sensor
  ```

### Step 2: Update docker-compose.yaml

You have two options for adopting the optimized Dockerfiles:

#### Option A: Use Optimized Dockerfiles (Non-Destructive)

Update `docker-compose.yaml` to reference the new Dockerfiles:

- [ ] **Update API service**
  ```yaml
  api:
    build:
      context: .
      dockerfile: docker/Dockerfile.optimized  # Add/change this line
      args:
        SERVICE: api
  ```

- [ ] **Update executor service**
  ```yaml
  executor:
    build:
      context: .
      dockerfile: docker/Dockerfile.optimized
      args:
        SERVICE: executor
  ```

- [ ] **Update sensor service**
  ```yaml
  sensor:
    build:
      context: .
      dockerfile: docker/Dockerfile.optimized
      args:
        SERVICE: sensor
  ```

- [ ] **Update notifier service**
  ```yaml
  notifier:
    build:
      context: .
      dockerfile: docker/Dockerfile.optimized
      args:
        SERVICE: notifier
  ```

- [ ] **Update worker services**
  ```yaml
  worker-shell:
    build:
      context: .
      dockerfile: docker/Dockerfile.worker.optimized
      target: worker-base
      
  worker-python:
    build:
      context: .
      dockerfile: docker/Dockerfile.worker.optimized
      target: worker-python
      
  worker-node:
    build:
      context: .
      dockerfile: docker/Dockerfile.worker.optimized
      target: worker-node
      
  worker-full:
    build:
      context: .
      dockerfile: docker/Dockerfile.worker.optimized
      target: worker-full
  ```

#### Option B: Replace Existing Dockerfiles

- [ ] **Replace main Dockerfile**
  ```bash
  mv docker/Dockerfile.optimized docker/Dockerfile
  ```

- [ ] **Replace worker Dockerfile**
  ```bash
  mv docker/Dockerfile.worker.optimized docker/Dockerfile.worker
  ```

- [ ] **No docker-compose.yaml changes needed** (already references `docker/Dockerfile`)

### Step 3: Clean Old Images

- [ ] **Stop running containers**
  ```bash
  docker compose down
  ```

- [ ] **Remove old images** (optional but recommended)
  ```bash
  docker compose rm -f
  docker images | grep attune | awk '{print $3}' | xargs docker rmi -f
  ```

- [ ] **Remove packs_data volume** (will be recreated)
  ```bash
  docker volume rm attune_packs_data
  ```

### Step 4: Build New Images

- [ ] **Build all services with optimized Dockerfiles**
  ```bash
  docker compose build --no-cache
  ```

- [ ] **Note build time** (should be similar to old clean build)
  ```bash
  # Expected: ~5-6 minutes for all services
  ```

### Step 5: Start Services

- [ ] **Start all services**
  ```bash
  docker compose up -d
  ```

- [ ] **Wait for init-packs to complete**
  ```bash
  docker compose logs -f init-packs
  # Should see: "Packs loaded successfully"
  ```

- [ ] **Verify services are healthy**
  ```bash
  docker compose ps
  # All services should show "healthy" status
  ```

### Step 6: Verify Packs Are Mounted

- [ ] **Check packs in API service**
  ```bash
  docker compose exec api ls -la /opt/attune/packs/
  # Should see: core/
  ```

- [ ] **Check packs in worker service**
  ```bash
  docker compose exec worker-shell ls -la /opt/attune/packs/
  # Should see: core/
  ```

- [ ] **Check pack binaries**
  ```bash
  docker compose exec sensor ls -la /opt/attune/packs/core/sensors/
  # Should see: attune-core-timer-sensor
  ```

- [ ] **Verify binary is executable**
  ```bash
  docker compose exec sensor /opt/attune/packs/core/sensors/attune-core-timer-sensor --version
  # Should show version or run successfully
  ```

## Verification Tests

### Test 1: Incremental Build Performance

- [ ] **Make a small change to API code**
  ```bash
  echo "// optimization test" >> crates/api/src/main.rs
  ```

- [ ] **Time incremental rebuild**
  ```bash
  time docker compose build api
  # Expected: ~30-60 seconds (vs ~5 minutes before)
  ```

- [ ] **Verify change is reflected**
  ```bash
  docker compose up -d api
  docker compose logs api | grep "optimization test"
  ```

- [ ] **Revert change**
  ```bash
  git checkout crates/api/src/main.rs
  ```

### Test 2: Pack Update Performance

- [ ] **Edit a pack file**
  ```bash
  echo "# test comment" >> packs/core/actions/echo.yaml
  ```

- [ ] **Time pack update**
  ```bash
  time docker compose restart
  # Expected: ~5 seconds (vs ~5 minutes rebuild before)
  ```

- [ ] **Verify pack change visible**
  ```bash
  docker compose exec api cat /opt/attune/packs/core/actions/echo.yaml | grep "test comment"
  ```

- [ ] **Revert change**
  ```bash
  git checkout packs/core/actions/echo.yaml
  ```

### Test 3: Isolated Service Rebuilds

- [ ] **Change worker code only**
  ```bash
  echo "// worker test" >> crates/worker/src/main.rs
  ```

- [ ] **Rebuild worker**
  ```bash
  time docker compose build worker-shell
  # Expected: ~30 seconds
  ```

- [ ] **Verify API not rebuilt**
  ```bash
  docker compose build api
  # Should show: "CACHED" for all layers
  # Expected: ~5 seconds
  ```

- [ ] **Revert change**
  ```bash
  git checkout crates/worker/src/main.rs
  ```

### Test 4: Common Crate Changes

- [ ] **Change common crate**
  ```bash
  echo "// common test" >> crates/common/src/lib.rs
  ```

- [ ] **Rebuild multiple services**
  ```bash
  time docker compose build api executor worker-shell
  # Expected: ~2 minutes per service (all depend on common)
  # Still faster than old ~5 minutes per service
  ```

- [ ] **Revert change**
  ```bash
  git checkout crates/common/src/lib.rs
  ```

## Post-Migration Checklist

### Documentation

- [ ] **Update README or deployment docs** with reference to optimized Dockerfiles

- [ ] **Share optimization docs with team**
  - `docs/docker-layer-optimization.md`
  - `docs/QUICKREF-docker-optimization.md`
  - `docs/QUICKREF-packs-volumes.md`

- [ ] **Document pack binary build process**
  - When to run `./scripts/build-pack-binaries.sh`
  - How to add new pack binaries

### CI/CD Updates

- [ ] **Update CI/CD pipeline** to use optimized Dockerfiles

- [ ] **Add pack binary build step** to CI if needed
  ```yaml
  # Example GitHub Actions
  - name: Build pack binaries
    run: ./scripts/build-pack-binaries.sh
  ```

- [ ] **Update BuildKit cache configuration** in CI
  ```yaml
  # Example: GitHub Actions cache
  - name: Set up Docker Buildx
    uses: docker/setup-buildx-action@v2
  ```

- [ ] **Measure CI build time improvement**
  - Before: ___ minutes
  - After: ___ minutes
  - Improvement: ___%

### Team Training

- [ ] **Train team on new workflows**
  - Code changes: `docker compose build <service>` (30 sec)
  - Pack changes: `docker compose restart` (5 sec)
  - Pack binaries: `./scripts/build-pack-binaries.sh` (2 min)

- [ ] **Update onboarding documentation**
  - Initial setup: run `./scripts/build-pack-binaries.sh`
  - Development: use `packs.dev/` for instant testing

- [ ] **Share troubleshooting guide**
  - `docs/DOCKER-OPTIMIZATION-SUMMARY.md#troubleshooting`

## Rollback Plan

If issues arise, you can quickly rollback:

### Rollback to Old Dockerfiles

- [ ] **Restore old docker-compose.yaml**
  ```bash
  cp docker-compose.yaml.backup docker-compose.yaml
  ```

- [ ] **Restore old Dockerfiles** (if replaced)
  ```bash
  cp docker/Dockerfile.backup docker/Dockerfile
  cp docker/Dockerfile.worker.backup docker/Dockerfile.worker
  ```

- [ ] **Rebuild with old Dockerfiles**
  ```bash
  docker compose build --no-cache
  docker compose up -d
  ```

### Keep Both Versions

You can maintain both Dockerfiles and switch between them:

```yaml
# Use optimized for development
services:
  api:
    build:
      dockerfile: docker/Dockerfile.optimized
      
# Use old for production (if needed)
# Just change to: dockerfile: docker/Dockerfile
```

## Performance Metrics Template

Document your actual performance improvements:

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Clean build (all services) | ___ min | ___ min | ___% |
| Incremental build (API) | ___ min | ___ sec | ___% |
| Incremental build (worker) | ___ min | ___ sec | ___% |
| Common crate change | ___ min | ___ min | ___% |
| Pack YAML update | ___ min | ___ sec | ___% |
| Pack binary update | ___ min | ___ min | ___% |
| Image size (API) | ___ MB | ___ MB | ___% |
| CI/CD build time | ___ min | ___ min | ___% |

## Common Issues and Solutions

### Issue: "crate not found" during build

**Cause**: Missing crate manifest in optimized Dockerfile

**Solution**:
```bash
# Add to both planner and builder stages in Dockerfile.optimized
# Planner stage:
COPY crates/missing-crate/Cargo.toml ./crates/missing-crate/Cargo.toml
RUN mkdir -p crates/missing-crate/src && echo "fn main() {}" > crates/missing-crate/src/main.rs

# Builder stage:
COPY crates/missing-crate/Cargo.toml ./crates/missing-crate/Cargo.toml
```

### Issue: Pack binaries "exec format error"

**Cause**: Binary compiled for wrong architecture

**Solution**:
```bash
# Always use Docker to build pack binaries
./scripts/build-pack-binaries.sh

# Restart sensor service
docker compose restart sensor
```

### Issue: Pack changes not visible

**Cause**: Edited `./packs/` after init-packs ran

**Solution**:
```bash
# Use packs.dev for development
mkdir -p packs.dev/mypack
cp -r packs/mypack/* packs.dev/mypack/
vim packs.dev/mypack/actions/my_action.yaml
docker compose restart

# OR recreate packs_data volume
docker compose down
docker volume rm attune_packs_data
docker compose up -d
```

### Issue: Build still slow after optimization

**Cause**: Not using optimized Dockerfile

**Solution**:
```bash
# Verify which Dockerfile is being used
docker compose config | grep dockerfile
# Should show: docker/Dockerfile.optimized

# If not, update docker-compose.yaml
```

## Success Criteria

Migration is successful when:

- ✅ All services start and are healthy
- ✅ Packs are visible in all service containers
- ✅ Pack binaries execute successfully
- ✅ Incremental builds complete in ~30 seconds (vs ~5 minutes)
- ✅ Pack updates complete in ~5 seconds (vs ~5 minutes)
- ✅ API returns pack data correctly
- ✅ Actions execute successfully
- ✅ Sensors register and run correctly
- ✅ Team understands new workflows

## Next Steps

After successful migration:

1. **Monitor build performance** over next few days
2. **Collect team feedback** on new workflows
3. **Update CI/CD metrics** to track improvements
4. **Consider removing old Dockerfiles** after 1-2 weeks of stability
5. **Share results** with team (build time savings, developer experience)

## Additional Resources

- Full Guide: `docs/docker-layer-optimization.md`
- Quick Start: `docs/QUICKREF-docker-optimization.md`
- Packs Architecture: `docs/QUICKREF-packs-volumes.md`
- Summary: `docs/DOCKER-OPTIMIZATION-SUMMARY.md`
- This Checklist: `docs/DOCKER-OPTIMIZATION-MIGRATION.md`

## Questions or Issues?

If you encounter problems during migration:

1. Check troubleshooting sections in optimization docs
2. Review docker compose logs: `docker compose logs <service>`
3. Verify BuildKit is enabled: `docker buildx version`
4. Test with clean build: `docker compose build --no-cache`
5. Rollback if needed using backup Dockerfiles

---

**Migration Date**: _______________

**Performed By**: _______________

**Notes**: _______________