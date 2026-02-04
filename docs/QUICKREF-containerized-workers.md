# Quick Reference: Containerized Workers

## Worker Variants

| Variant | Runtimes | Size | Use Case |
|---------|----------|------|----------|
| `worker-base` | `shell` | ~580MB | Shell scripts, basic automation |
| `worker-python` | `shell`, `python` | ~1.2GB | Python actions, scripts with deps |
| `worker-node` | `shell`, `node` | ~760MB | JavaScript/TypeScript actions |
| `worker-full` | `shell`, `python`, `node`, `native` | ~1.6GB | Multi-language packs |

## Quick Start

```bash
# Build all worker images
make docker-build-workers

# Start specific worker
docker-compose up -d worker-python

# Start all workers
docker-compose up -d worker-shell worker-python worker-node worker-full

# Check worker status
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, worker_type, status, capabilities->>'runtimes' as runtimes FROM worker;"
```

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `ATTUNE_WORKER_NAME` | Unique worker ID | `worker-python-01` |
| `ATTUNE_WORKER_RUNTIMES` | Runtime capabilities | `shell,python` |
| `ATTUNE_WORKER_TYPE` | Worker deployment type | `container` |
| `ATTUNE__DATABASE__URL` | Database connection | `postgresql://...` |
| `ATTUNE__MESSAGE_QUEUE__URL` | RabbitMQ connection | `amqp://...` |
| `RUST_LOG` | Logging level | `info`, `debug` |

## Runtime Capability Declaration

**Priority order:**
1. **Environment variable** (highest): `ATTUNE_WORKER_RUNTIMES="shell,python"`
2. **Config file**: `worker.capabilities.runtimes: ["shell", "python"]`
3. **Auto-detection** (fallback): Checks for runtime binaries

## Build Commands

```bash
# Individual builds
make docker-build-worker-base      # Shell only
make docker-build-worker-python    # Python + shell
make docker-build-worker-node      # Node.js + shell
make docker-build-worker-full      # All runtimes

# Direct Docker build
DOCKER_BUILDKIT=1 docker build \
  --target worker-python \
  -t attune-worker:python \
  -f docker/Dockerfile.worker .
```

## Scaling Workers

```bash
# Scale specific worker type
docker-compose up -d --scale worker-python=3

# Scale multiple types
docker-compose up -d \
  --scale worker-python=3 \
  --scale worker-shell=2
```

## Custom Worker Images

```dockerfile
# Extend Python worker with ML packages
FROM attune-worker:python

USER root
RUN pip install --no-cache-dir pandas numpy scikit-learn
USER attune

ENV ATTUNE_WORKER_RUNTIMES="shell,python,ml"
```

## Monitoring

```bash
# View logs
docker-compose logs -f worker-python

# Check container status
docker-compose ps | grep worker

# Resource usage
docker stats attune-worker-python

# Worker registration
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, status, last_heartbeat, capabilities FROM worker;"

# Active executions per worker
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT w.name, COUNT(e.id) as active 
   FROM worker w 
   LEFT JOIN execution e ON e.worker = w.id AND e.status = 'running' 
   GROUP BY w.name;"
```

## Troubleshooting

### Worker not registering
```bash
# Check database connectivity
docker-compose logs worker-python | grep -i database

# Verify environment
docker-compose exec worker-python env | grep ATTUNE

# Force restart
docker-compose restart worker-python
```

### Runtime not detected
```bash
# Check runtime availability
docker-compose exec worker-python python3 --version

# Force runtime declaration
docker-compose exec worker-python sh -c \
  'echo $ATTUNE_WORKER_RUNTIMES'
```

### Actions not scheduled
```sql
-- Check runtime compatibility
SELECT a.ref, r.name as required_runtime
FROM action a
JOIN runtime r ON a.runtime = r.id
WHERE a.ref = 'core.my_action';

-- Check worker capabilities
SELECT name, capabilities->>'runtimes' as available_runtimes
FROM worker
WHERE status = 'active';
```

## Resource Limits

```yaml
# docker-compose.override.yml
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

## Security

- ✅ Non-root execution (user `attune`, UID 1000)
- ✅ Read-only pack mounts (`:ro` flag)
- ✅ Network isolation (dedicated Docker network)
- ✅ Health checks (process monitoring)
- ✅ Minimal base images (Debian slim variants)

## Common Tasks

```bash
# Stop all workers
docker-compose stop worker-shell worker-python worker-node worker-full

# Remove workers
docker-compose rm -f worker-python

# Rebuild and restart
docker-compose up -d --build worker-python

# Execute shell in worker
docker-compose exec worker-python bash

# View worker config
docker-compose exec worker-python cat /opt/attune/config.yaml
```

## How It Works

1. **Worker starts** → Reads `ATTUNE_WORKER_RUNTIMES` or detects runtimes
2. **Registration** → Stores capabilities in `worker.capabilities` JSON field
3. **Executor schedules action** → Checks action's runtime requirement
4. **Worker selection** → Finds workers with matching runtime in capabilities
5. **Action execution** → Sends to compatible worker via message queue

## Files

- `docker/Dockerfile.worker` - Multi-stage worker Dockerfile
- `docker-compose.yaml` - Worker service definitions
- `crates/worker/src/registration.rs` - Capability detection logic
- `docs/worker-containerization.md` - Full design document
- `docs/worker-containers-quickstart.md` - Detailed guide

## Further Reading

- [Worker Containerization Design](worker-containerization.md)
- [Worker Containers Quick Start](worker-containers-quickstart.md)
- [Worker Service Architecture](architecture/worker-service.md)
- [Docker README](../docker/README.worker.md)