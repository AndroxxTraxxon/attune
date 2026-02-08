# Quick Reference: Packs Volume Architecture

## TL;DR

**Packs are NOT copied into Docker images. They are mounted as volumes.**

```bash
# Build pack binaries (one-time or when updated)
./scripts/build-pack-binaries.sh

# Start services - init-packs copies packs to volume
docker compose up -d

# Update pack files - no image rebuild needed!
vim packs/core/actions/my_action.yaml
docker compose restart
```

## Architecture Overview

```
Host Filesystem          Docker Volumes           Service Containers
─────────────────        ───────────────          ──────────────────

./packs/                                          
  ├── core/                                       
  │   ├── actions/                                
  │   ├── sensors/                                
  │   └── pack.yaml                               
  │                                               
  │                      ┌─────────────┐          
  │   (copy during       │ packs_data  │──────────> /opt/attune/packs (api)
  │    init-packs)       │   volume    │          
  │         └────────────>│             │──────────> /opt/attune/packs (executor)
  │                      │             │          
  │                      │             │──────────> /opt/attune/packs (worker)
  │                      │             │          
  │                      │             │──────────> /opt/attune/packs (sensor)
  │                      └─────────────┘          
  │                                               
./packs.dev/                                      
  └── custom-pack/       ┌────────────────────────> /opt/attune/packs.dev (all)
       (bind mount)      │ (read-write for dev)
                         │
                         └─ (mounted directly)
```

## Why Volumes Instead of COPY?

| Aspect | COPY into Image | Volume Mount |
|--------|----------------|--------------|
| **Update packs** | Rebuild image (~5 min) | Restart service (~5 sec) |
| **Image size** | Larger (+packs) | Smaller (no packs) |
| **Development** | Slow iteration | Fast iteration |
| **Consistency** | Each service separate | All services share |
| **Pack binaries** | Baked into image | Updateable |

## docker-compose.yaml Configuration

```yaml
volumes:
  packs_data:
    driver: local

services:
  # Step 1: init-packs runs once to populate packs_data volume
  init-packs:
    image: python:3.11-alpine
    volumes:
      - ./packs:/source/packs:ro              # Host packs (read-only)
      - packs_data:/opt/attune/packs          # Target volume
    command: ["/bin/sh", "/init-packs.sh"]
    restart: on-failure

  # Step 2: Services mount packs_data as read-only
  api:
    volumes:
      - packs_data:/opt/attune/packs:ro       # Production packs (RO)
      - ./packs.dev:/opt/attune/packs.dev:rw  # Dev packs (RW)
    depends_on:
      init-packs:
        condition: service_completed_successfully

  worker-shell:
    volumes:
      - packs_data:/opt/attune/packs:ro       # Same volume
      - ./packs.dev:/opt/attune/packs.dev:rw

  # ... all services follow same pattern
```

## Pack Binaries (Native Code)

Some packs contain compiled binaries (e.g., sensors written in Rust).

### Building Pack Binaries

**Option 1: Use the script (recommended)**
```bash
./scripts/build-pack-binaries.sh
```

**Option 2: Manual build**
```bash
# Build in Docker with GLIBC compatibility
docker build -f docker/Dockerfile.pack-binaries -t attune-pack-builder .

# Extract binaries
docker create --name pack-tmp attune-pack-builder
docker cp pack-tmp:/pack-binaries/. ./packs/
docker rm pack-tmp
```

**Option 3: Native build (if GLIBC matches)**
```bash
cargo build --release --bin attune-core-timer-sensor
cp target/release/attune-core-timer-sensor packs/core/sensors/
```

### When to Rebuild Pack Binaries

- ✅ After `git pull` that updates pack binary source
- ✅ After modifying sensor source code (e.g., `crates/core-timer-sensor`)
- ✅ When setting up development environment for first time
- ❌ NOT needed for YAML/script changes in packs

## Development Workflow

### Editing Pack YAML Files

```bash
# 1. Edit pack files
vim packs/core/actions/echo.yaml

# 2. Restart services (no rebuild!)
docker compose restart

# 3. Test changes
curl -X POST http://localhost:8080/api/v1/executions \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"action_ref": "core.echo", "parameters": {"message": "hello"}}'
```

**Time**: ~5 seconds

### Editing Pack Scripts (Python/Shell)

```bash
# 1. Edit script
vim packs/core/actions/http_request.py

# 2. Restart services
docker compose restart worker-python

# 3. Test
# (run execution)
```

**Time**: ~5 seconds

### Editing Pack Binaries (Native Sensors)

```bash
# 1. Edit source
vim crates/core-timer-sensor/src/main.rs

# 2. Rebuild binary
./scripts/build-pack-binaries.sh

# 3. Restart services
docker compose restart sensor

# 4. Test
# (check sensor registration)
```

**Time**: ~2 minutes (compile + restart)

## Development Packs (packs.dev)

For rapid development, use the `packs.dev` directory:

```bash
# Create a dev pack
mkdir -p packs.dev/mypack/actions

# Create action
cat > packs.dev/mypack/actions/test.yaml <<EOF
name: test
description: Test action
runner_type: Shell
entry_point: echo.sh
parameters:
  message:
    type: string
    required: true
EOF

cat > packs.dev/mypack/actions/echo.sh <<'EOF'
#!/bin/bash
echo "Message: $ATTUNE_MESSAGE"
EOF

chmod +x packs.dev/mypack/actions/echo.sh

# Restart to pick up changes
docker compose restart

# Test immediately - no rebuild needed!
```

**Benefits of packs.dev**:
- ✅ Direct bind mount (changes visible immediately)
- ✅ Read-write access (can modify from container)
- ✅ No init-packs step needed
- ✅ Perfect for iteration

## Optimized Dockerfiles and Packs

The optimized Dockerfiles (`docker/Dockerfile.optimized`) do NOT copy packs:

```dockerfile
# ❌ OLD: Packs copied into image
COPY packs/ ./packs/

# ✅ NEW: Only create mount point
RUN mkdir -p /opt/attune/packs /opt/attune/logs

# Packs mounted at runtime from packs_data volume
```

**Result**:
- Service images contain only binaries + configs
- Packs updated independently
- Faster builds (no pack layer invalidation)

## Troubleshooting

### "Pack not found" errors

**Symptom**: API returns 404 for pack/action
**Cause**: Packs not loaded into volume

**Fix**:
```bash
# Check if packs exist in volume
docker compose exec api ls -la /opt/attune/packs/

# If empty, restart init-packs
docker compose restart init-packs
docker compose logs init-packs
```

### Pack changes not visible

**Symptom**: Updated pack.yaml but changes not reflected
**Cause**: Changes made to host `./packs/` after init-packs ran

**Fix**:
```bash
# Option 1: Use packs.dev for development
mv packs/mypack packs.dev/mypack
docker compose restart

# Option 2: Recreate packs_data volume
docker compose down
docker volume rm attune_packs_data
docker compose up -d
```

### Pack binary "exec format error"

**Symptom**: Sensor binary fails with exec format error
**Cause**: Binary compiled for wrong architecture or GLIBC version

**Fix**:
```bash
# Rebuild with Docker (ensures compatibility)
./scripts/build-pack-binaries.sh

# Restart sensor service
docker compose restart sensor
```

### Pack binary "permission denied"

**Symptom**: Binary exists but can't execute
**Cause**: Binary not executable

**Fix**:
```bash
chmod +x packs/core/sensors/attune-core-timer-sensor
docker compose restart init-packs sensor
```

## Best Practices

### DO:
- ✅ Use `./scripts/build-pack-binaries.sh` for pack binaries
- ✅ Put development packs in `packs.dev/`
- ✅ Keep production packs in `packs/`
- ✅ Commit pack YAML/scripts to git
- ✅ Use `.gitignore` for compiled pack binaries
- ✅ Restart services after pack changes
- ✅ Use `init-packs` logs to debug loading issues

### DON'T:
- ❌ Don't copy packs into Dockerfiles
- ❌ Don't edit packs inside running containers
- ❌ Don't commit compiled pack binaries to git
- ❌ Don't expect instant updates to `packs/` (need restart)
- ❌ Don't rebuild service images for pack changes
- ❌ Don't modify packs_data volume directly

## Migration from Old Dockerfiles

If your old Dockerfiles copied packs:

```dockerfile
# OLD Dockerfile
COPY packs/ ./packs/
COPY --from=pack-builder /build/pack-binaries/ ./packs/
```

**Migration steps**:

1. **Build pack binaries separately**:
   ```bash
   ./scripts/build-pack-binaries.sh
   ```

2. **Update to optimized Dockerfile**:
   ```yaml
   # docker-compose.yaml
   api:
     build:
       dockerfile: docker/Dockerfile.optimized
   ```

3. **Rebuild service images**:
   ```bash
   docker compose build --no-cache
   ```

4. **Start services** (init-packs will populate volume):
   ```bash
   docker compose up -d
   ```

## Summary

**Architecture**: Packs → Volume → Services
- Host `./packs/` copied to `packs_data` volume by `init-packs`
- Services mount `packs_data` as read-only
- Dev packs in `packs.dev/` bind-mounted directly

**Benefits**:
- 90% faster pack updates (restart vs rebuild)
- Smaller service images
- Consistent packs across all services
- Clear separation: services = code, packs = content

**Key Commands**:
```bash
./scripts/build-pack-binaries.sh   # Build native pack binaries
docker compose restart              # Pick up pack changes
docker compose logs init-packs      # Debug pack loading
```

**Remember**: Packs are content, not code. Treat them as configuration, not part of the service image.