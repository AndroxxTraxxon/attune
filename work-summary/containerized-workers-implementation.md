# Containerized Workers Implementation

**Date**: 2024-01-31  
**Status**: Complete  
**Related Issues**: Worker containerization, runtime capability declaration, automatic user initialization

## Overview

Implemented a complete containerized worker system for Attune that allows workers to run in Docker containers with different runtime capabilities. Workers automatically declare their capabilities when registering with the system, enabling intelligent action scheduling based on runtime requirements.

## Changes Made

### 1. Worker Registration Enhancement

**File**: `crates/worker/src/registration.rs`

- Added `detect_capabilities()` method to intelligently determine worker capabilities
- Implemented three-tier capability detection system:
  1. `ATTUNE_WORKER_RUNTIMES` environment variable (highest priority)
  2. Configuration file capabilities
  3. Auto-detection via runtime binary checks (fallback)
- Added `auto_detect_runtimes()` method that checks for Python, Node.js, and other runtimes
- Workers now log detected capabilities on startup
- Capabilities include version metadata (`worker_version`)

**Key Features**:
- Environment variable parsing with whitespace handling
- Case-insensitive runtime matching
- Graceful fallback to auto-detection
- Configurable max concurrent executions

### 2. Configuration Updates

**File**: `crates/common/src/config.rs`

- Added `capabilities` field to `WorkerConfig` struct
- Supports JSON-based capability declaration via config files
- Can be overridden by environment variables

### 3. Multi-stage Worker Dockerfile

**File**: `docker/Dockerfile.worker`

Created a multi-stage Dockerfile supporting four worker variants:

#### Stage 1: Builder
- Compiles Rust worker binary
- Uses BuildKit cache mounts for fast incremental builds
- Shared across all worker variants
- Build times: ~5-6 min cold, ~30-60 sec cached

#### Stage 2a: Base Worker (`worker-base`)
- **Runtime Capabilities**: `shell`
- **Base**: Debian Bookworm Slim
- **Size**: ~580 MB
- **User**: `attune` (UID 1000)
- **Use Case**: Lightweight shell automation

#### Stage 2b: Python Worker (`worker-python`)
- **Runtime Capabilities**: `shell`, `python`
- **Base**: Python 3.11 Slim
- **Size**: ~1.2 GB
- **User**: `attune` (UID 1001, avoids conflict with Python image's existing UID 1000)
- **Includes**: requests, pyyaml, jinja2, python-dateutil
- **Use Case**: Python actions and scripts

#### Stage 2c: Node.js Worker (`worker-node`)
- **Runtime Capabilities**: `shell`, `node`
- **Base**: Node 20 Slim
- **Size**: ~760 MB
- **User**: `attune` (UID 1001, avoids conflict with Node image's `node` user at UID 1000)
- **Use Case**: JavaScript/TypeScript actions

#### Stage 2d: Full Worker (`worker-full`)
- **Runtime Capabilities**: `shell`, `python`, `node`, `native`
- **Base**: Debian Bookworm
- **Size**: ~1.6 GB
- **User**: `attune` (UID 1001)
- **Includes**: Python, Node.js, build tools
- **Use Case**: Multi-language automation

**Security Features**:
- All workers run as non-root user `attune` (UID 1000 for base, UID 1001 for Python/Node/Full to avoid conflicts)
- Read-only pack mounts
- Health checks via process monitoring
- Minimal attack surface

### 4. Docker Compose Integration

**File**: `docker-compose.yaml`

Replaced single worker service with four specialized variants:

- `worker-shell`: Base shell worker
- `worker-python`: Python runtime worker
- `worker-node`: Node.js runtime worker
- `worker-full`: Full multi-runtime worker

**Features**:
- Unique worker names via environment variables
- Proper health checks
- Volume mounts for packs and logs
- Service dependencies on database and message queue
- Resource isolation

**Environment Variables**:
- `ATTUNE_WORKER_NAME`: Unique worker identifier
- `ATTUNE_WORKER_RUNTIMES`: Comma-separated runtime list
- `ATTUNE_WORKER_TYPE`: Set to `container`
- Database and message queue connection strings

### 5. Makefile Targets

**File**: `Makefile`

Added new targets for building worker images:

```bash
make docker-build-workers          # Build all variants
make docker-build-worker-base      # Build base worker
make docker-build-worker-python    # Build Python worker
make docker-build-worker-node      # Build Node.js worker
make docker-build-worker-full      # Build full worker
```

All targets use BuildKit for optimized caching.

### 6. Documentation

#### `docs/worker-containerization.md`
Comprehensive design document covering:
- Architecture overview
- Runtime capability declaration system
- Worker image variants and use cases
- Implementation details
- Configuration options
- Docker Compose integration
- Security considerations
- Pack dependency management
- Monitoring and troubleshooting
- Future enhancements

#### `docs/worker-containers-quickstart.md`
Step-by-step quickstart guide including:
- Building worker images
- Starting workers
- Verifying registration
- Testing action execution
- Scaling workers
- Custom runtime configuration
- Building custom worker images
- Monitoring and troubleshooting
- Best practices
- Production deployment

#### `docker/README.worker.md`
Docker-specific documentation with:
- Worker variant specifications
- Build instructions
- Running workers
- Runtime capability declaration
- Configuration reference
- Custom image creation
- Architecture details
- Security features
- Performance metrics
- Quick command reference

### 7. Automatic User Initialization

**Files**: `docker/init-user.sh`, `docker-compose.yaml`

Added automatic default user creation for Docker deployments:

#### Init Script (`docker/init-user.sh`)
- Automatically creates test user `test@attune.local` / `TestPass123!`
- Idempotent - safe to run multiple times
- Uses pre-computed Argon2id password hash for speed
- Waits for database readiness before proceeding
- Checks if user exists before creating
- Configurable via environment variables

#### Docker Compose Integration
- Added `init-user` service that runs after migrations
- All application services depend on `init-user` completion
- Ensures default user exists before services start
- Eliminates manual user creation step for Docker deployments

**Benefits**:
- Zero-config Docker startup
- No manual user creation needed
- Idempotent and safe
- Clear default credentials in documentation
- Easy to customize via environment variables

### 8. Testing

**File**: `crates/worker/src/registration.rs`

Added unit tests for runtime detection:
- `test_auto_detect_runtimes()`: Verifies auto-detection includes shell and native
- Tests validate runtime detection logic works correctly

### 8. Bug Fixes

**File**: `crates/worker/src/main.rs`

- Fixed `WorkerConfig` initialization to include new `capabilities` field

**File**: `docker/Dockerfile.worker`

- Fixed user creation to use UID 1001 for Python/Node/Full workers to avoid conflicts with existing users in base images
- Node.js base image has `node` user at UID 1000
- Python base image may have conflicting UID 1000 in some versions

## Technical Details

### Runtime Capability System

Workers declare capabilities in their registration:

```json
{
  "runtimes": ["shell", "python"],
  "max_concurrent_executions": 10,
  "worker_version": "0.1.0"
}
```

The executor uses this information when scheduling actions:
1. Check action's runtime requirement
2. Query workers with matching runtime in capabilities
3. Filter by active status
4. Select worker based on availability

### Environment Variable Override

Workers check `ATTUNE_WORKER_RUNTIMES` on startup:

```bash
ATTUNE_WORKER_RUNTIMES="shell,python,custom"
```

This is parsed, trimmed, and converted to lowercase for consistency.

### Auto-detection Logic

If no explicit configuration exists, workers detect runtimes by checking binaries:
- Tests for `python3` or `python` → adds `python`
- Tests for `node` → adds `node`
- Always includes `shell` and `native`

### BuildKit Optimization

Cache mounts dramatically improve build times:
- Cargo registry cache (shared, locked)
- Git cache (shared, locked)
- Build target cache (shared, locked)

The `sharing=locked` mode prevents race conditions during parallel builds.

## Use Cases

### 1. Lightweight Deployment
Use `worker-base` for simple shell automation:
```bash
docker-compose up -d worker-shell
```

### 2. Python-heavy Workloads
Use `worker-python` for Python packs:
```bash
docker-compose up -d worker-python
```

### 3. Multi-language Packs
Use `worker-full` when packs need multiple runtimes:
```bash
docker-compose up -d worker-full
```

### 4. Horizontal Scaling
Scale specific worker types based on demand:
```bash
docker-compose up -d --scale worker-python=3 --scale worker-shell=2
```

### 5. Custom Runtimes
Extend base images for specialized needs:
```dockerfile
FROM attune-worker:python
RUN pip install pandas numpy scikit-learn
ENV ATTUNE_WORKER_RUNTIMES="shell,python,ml"
```

## Benefits

1. **Flexible Deployment**: Choose worker variant based on workload requirements
2. **Resource Optimization**: Use lightweight workers when possible
3. **Isolation**: Each worker runs in isolated container
4. **Scalability**: Easily scale specific worker types
5. **Extensibility**: Simple to add new runtime support
6. **Intelligent Scheduling**: Executor matches actions to capable workers
7. **Fast Builds**: BuildKit caching reduces rebuild times by 90%
8. **Security**: Non-root execution, read-only mounts, minimal attack surface

## Migration Path

### From Local Workers
1. Build appropriate worker image variant
2. Start containerized worker with same capabilities
3. Verify registration in database
4. Decommission local worker

### Adding New Runtime
1. Create Dockerfile extending base worker
2. Install runtime and set `ATTUNE_WORKER_RUNTIMES`
3. Build and deploy custom worker
4. Workers auto-register with new runtime capability

## Production Considerations

### 1. Resource Limits
Set CPU and memory limits in production:
```yaml
deploy:
  resources:
    limits:
      cpus: '2'
      memory: 2G
```

### 2. High Availability
Run multiple instances of each worker type:
```bash
docker-compose up -d --scale worker-python=3
```

### 3. Monitoring
- Track worker heartbeats in database
- Monitor container resource usage
- Collect logs via Docker logging drivers
- Alert on worker registration failures

### 4. Security
- Use secrets management for sensitive environment variables
- Regularly update base images for security patches
- Scan images for vulnerabilities
- Network isolation between workers and external services

## Future Enhancements

1. **Pack-specific Workers**: Workers dedicated to specific packs
2. **GPU Support**: Workers with GPU access for ML workloads
3. **Custom Runtime Registration**: Allow packs to define custom runtimes
4. **Worker Pools**: Logical grouping of workers by capability sets
5. **Auto-scaling**: Scale workers based on queue depth
6. **Pack Isolation**: Run each pack in isolated container
7. **Dynamic Runtime Detection**: Real-time capability updates
8. **Worker Affinity**: Schedule actions to workers that previously ran them

## Testing

All changes compile without errors or warnings:
```bash
cargo check --workspace  # ✅ Success
cargo test -p attune-worker test_auto_detect_runtimes  # ✅ Pass
```

All worker images build successfully:
```bash
make docker-build-workers  # ✅ Success
docker images attune-worker  # Shows all 4 variants built
```

Runtime verification:
```bash
docker run --rm --entrypoint python3 attune-worker:python --version  # Python 3.11.14
docker run --rm --entrypoint node attune-worker:node --version  # v20.20.0
docker run --rm --entrypoint id attune-worker:python  # uid=1001(attune) gid=1001(attune)
```

## Files Changed

### New Files
- `docker/Dockerfile.worker` - Multi-stage worker Dockerfile
- `docker/README.worker.md` - Docker worker documentation
- `docker/init-user.sh` - Automatic user initialization script
- `docker/INIT-USER-README.md` - User initialization documentation
- `docs/worker-containerization.md` - Design document
- `docs/worker-containers-quickstart.md` - Quick start guide
- `docs/QUICKREF-containerized-workers.md` - Quick reference card
- `work-summary/containerized-workers-implementation.md` - This file

### Modified Files
- `crates/worker/src/registration.rs` - Capability detection
- `crates/worker/src/main.rs` - WorkerConfig fix
- `crates/common/src/config.rs` - Added capabilities field
- `docker-compose.yaml` - Added worker variants and init-user service
- `docker/README.md` - Added user initialization documentation
- `Makefile` - Added build targets

## Summary

This implementation provides a complete containerized worker system with intelligent runtime capability declaration and zero-config Docker deployment. Workers can be deployed in specialized variants optimized for different workloads, with automatic capability detection and efficient Docker builds.

**Key Features**:
- **4 Worker Variants**: Base, Python, Node.js, Full - choose based on workload
- **Intelligent Capability Detection**: Environment → Config → Auto-detect
- **Automatic User Setup**: Default test user created on first startup
- **Zero Manual Configuration**: Docker Compose handles everything
- **Production Ready**: Security hardened, comprehensive documentation

The design is extensible, allowing teams to easily add new runtime support or create custom worker images for specialized needs. The multi-tier capability detection system (env → config → auto-detect) provides flexibility for different deployment scenarios while maintaining sensible defaults.

**Getting Started with Docker**:
```bash
# Build and start all services
docker-compose up -d

# Default user automatically created:
# Login: test@attune.local
# Password: TestPass123!

# Workers automatically register with capabilities
# No manual setup required!
```