# Attune Worker Containers

This directory contains Docker configurations for building Attune worker containers with different runtime capabilities.

## Overview

Attune workers can run in containers with specialized runtime environments. Workers automatically declare their capabilities when they register with the system, enabling intelligent action scheduling based on runtime requirements.

## Worker Variants

### Base Worker (`worker-base`)
- **Runtimes**: `shell`
- **Base Image**: Debian Bookworm Slim
- **Size**: ~580 MB
- **Use Case**: Lightweight workers for shell scripts and basic automation
- **Build**: `make docker-build-worker-base`

### Python Worker (`worker-python`)
- **Runtimes**: `shell`, `python`
- **Base Image**: Python 3.11 Slim
- **Size**: ~1.2 GB
- **Includes**: pip, virtualenv, common Python libraries (requests, pyyaml, jinja2, python-dateutil)
- **Use Case**: Python actions and scripts with dependencies
- **Build**: `make docker-build-worker-python`

### Node.js Worker (`worker-node`)
- **Runtimes**: `shell`, `node`
- **Base Image**: Node 20 Slim
- **Size**: ~760 MB
- **Includes**: npm, yarn
- **Use Case**: JavaScript/TypeScript actions and npm packages
- **Build**: `make docker-build-worker-node`

### Full Worker (`worker-full`)
- **Runtimes**: `shell`, `python`, `node`, `native`
- **Base Image**: Debian Bookworm
- **Size**: ~1.6 GB
- **Includes**: Python 3.x, Node.js 20, build tools
- **Use Case**: General-purpose automation requiring multiple runtimes
- **Build**: `make docker-build-worker-full`

## Building Worker Images

### Build All Variants
```bash
make docker-build-workers
```

### Build Individual Variants
```bash
# Base worker (shell only)
make docker-build-worker-base

# Python worker
make docker-build-worker-python

# Node.js worker
make docker-build-worker-node

# Full worker (all runtimes)
make docker-build-worker-full
```

### Direct Docker Build
```bash
# Using Docker directly with BuildKit
DOCKER_BUILDKIT=1 docker build \
  --target worker-python \
  -t attune-worker:python \
  -f docker/Dockerfile.worker.optimized \
  .
```

## Running Workers

### Using Docker Compose
```bash
# Start specific worker type
docker-compose up -d worker-python

# Start all workers
docker-compose up -d worker-shell worker-python worker-node worker-full

# Scale workers
docker-compose up -d --scale worker-python=3
```

### Using Docker Run
```bash
docker run -d \
  --name worker-python-01 \
  --network attune_attune-network \
  -e ATTUNE_WORKER_NAME=worker-python-01 \
  -e ATTUNE_WORKER_RUNTIMES=shell,python \
  -e ATTUNE__DATABASE__URL=postgresql://attune:attune@postgres:5432/attune \
  -e ATTUNE__MESSAGE_QUEUE__URL=amqp://attune:attune@rabbitmq:5672 \
  -v $(pwd)/packs:/opt/attune/packs:ro \
  attune-worker:python
```

## Runtime Capability Declaration

Workers declare their capabilities in three ways (in order of precedence):

### 1. Environment Variable (Highest Priority)
```bash
ATTUNE_WORKER_RUNTIMES="shell,python,custom"
```

### 2. Configuration File
```yaml
worker:
  capabilities:
    runtimes: ["shell", "python"]
```

### 3. Auto-Detection (Fallback)
Workers automatically detect available runtimes by checking for binaries:
- `python3` or `python` → adds `python`
- `node` → adds `node`
- Always includes `shell` and `native`

## Configuration

### Key Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `ATTUNE_WORKER_NAME` | Unique worker identifier | `worker-python-01` |
| `ATTUNE_WORKER_RUNTIMES` | Comma-separated runtime list | `shell,python` |
| `ATTUNE_WORKER_TYPE` | Worker type | `container` |
| `ATTUNE__DATABASE__URL` | PostgreSQL connection | `postgresql://...` |
| `ATTUNE__MESSAGE_QUEUE__URL` | RabbitMQ connection | `amqp://...` |
| `RUST_LOG` | Log level | `info`, `debug`, `trace` |

### Resource Limits

Set CPU and memory limits in `docker-compose.override.yml`:

```yaml
services:
  worker-python:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
```

## Custom Worker Images

### Extend Python Worker

Create a custom worker with additional packages:

```dockerfile
# Dockerfile.worker.ml
FROM attune-worker:python

USER root

# Install ML packages
RUN pip install --no-cache-dir \
    pandas \
    numpy \
    scikit-learn \
    torch

USER attune

ENV ATTUNE_WORKER_RUNTIMES="shell,python,ml"
```

Build and run:
```bash
docker build -t attune-worker:ml -f Dockerfile.worker.ml .
docker run -d --name worker-ml-01 ... attune-worker:ml
```

### Add New Runtime

Example: Adding Ruby support

```dockerfile
FROM attune-worker:base

USER root

RUN apt-get update && apt-get install -y \
    ruby-full \
    && rm -rf /var/lib/apt/lists/*

USER attune

ENV ATTUNE_WORKER_RUNTIMES="shell,ruby"
```

## Architecture

### Multi-stage Build

The `Dockerfile.worker.optimized` uses a multi-stage build pattern:

1. **Builder Stage**: Compiles the Rust worker binary
   - Uses BuildKit cache mounts for fast incremental builds
   - Shared across all worker variants

2. **Runtime Stages**: Creates specialized worker images
   - `worker-base`: Minimal shell runtime
   - `worker-python`: Python runtime
   - `worker-node`: Node.js runtime
   - `worker-full`: All runtimes

### Build Cache

BuildKit cache mounts dramatically speed up builds:
- First build: ~5-6 minutes
- Incremental builds: ~30-60 seconds

Cache is shared across builds using `sharing=locked` to prevent race conditions.

## Security

### Non-root Execution
All workers run as user `attune` (UID 1000)

### Read-only Packs
Pack files are mounted read-only to prevent modification:
```yaml
volumes:
  - ./packs:/opt/attune/packs:ro  # :ro = read-only
```

### Network Isolation
Workers run in isolated Docker network with only necessary service access

### Secret Management
Use environment variables for sensitive data; never hardcode in images

## Monitoring

### Check Worker Registration
```bash
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, worker_type, status, capabilities->>'runtimes' as runtimes FROM worker;"
```

### View Logs
```bash
docker-compose logs -f worker-python
```

### Check Resource Usage
```bash
docker stats attune-worker-python
```

### Verify Health
```bash
docker-compose ps | grep worker
```

## Troubleshooting

### Worker Not Registering

**Check database connectivity:**
```bash
docker-compose logs worker-python | grep -i database
```

**Verify environment:**
```bash
docker-compose exec worker-python env | grep ATTUNE
```

### Runtime Not Detected

**Check runtime availability:**
```bash
docker-compose exec worker-python python3 --version
docker-compose exec worker-python node --version
```

**Force runtime declaration:**
```bash
ATTUNE_WORKER_RUNTIMES=shell,python
```

### Actions Not Scheduled

**Verify runtime match:**
```sql
-- Check action runtime requirement
SELECT a.ref, r.name as runtime 
FROM action a 
JOIN runtime r ON a.runtime = r.id 
WHERE a.ref = 'core.my_action';

-- Check worker capabilities
SELECT name, capabilities->>'runtimes' 
FROM worker 
WHERE status = 'active';
```

## Performance

### Image Sizes

| Image | Size | Build Time (Cold) | Build Time (Cached) |
|-------|------|-------------------|---------------------|
| worker-base | ~580 MB | ~5 min | ~30 sec |
| worker-python | ~1.2 GB | ~6 min | ~45 sec |
| worker-node | ~760 MB | ~6 min | ~45 sec |
| worker-full | ~1.6 GB | ~7 min | ~60 sec |

### Optimization Tips

1. **Use specific variants**: Don't use `worker-full` if you only need Python
2. **Enable BuildKit**: Dramatically speeds up builds
3. **Layer caching**: Order Dockerfile commands from least to most frequently changed
4. **Multi-stage builds**: Keeps runtime images small

## Files

- `Dockerfile.worker.optimized` - Multi-stage worker Dockerfile with all variants
- `README.worker.md` - This file
- `../docker-compose.yaml` - Service definitions for all workers

## References

- [Worker Containerization Design](../docs/worker-containerization.md)
- [Quick Start Guide](../docs/worker-containers-quickstart.md)
- [Worker Service Architecture](../docs/architecture/worker-service.md)
- [Production Deployment](../docs/production-deployment.md)

## Quick Commands

```bash
# Build all workers
make docker-build-workers

# Start all workers
docker-compose up -d worker-shell worker-python worker-node worker-full

# Check worker status
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, status, capabilities FROM worker;"

# View Python worker logs
docker-compose logs -f worker-python

# Restart worker
docker-compose restart worker-python

# Scale Python workers
docker-compose up -d --scale worker-python=3

# Stop all workers
docker-compose stop worker-shell worker-python worker-node worker-full
```
