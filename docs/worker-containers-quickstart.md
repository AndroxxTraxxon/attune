# Containerized Workers Quick Start Guide

This guide shows you how to quickly get started with containerized workers in Attune.

## Overview

Attune workers can run in containers with different runtime capabilities:

- **worker-base**: Shell commands only (~580MB)
- **worker-python**: Shell + Python runtime (~1.2GB)
- **worker-node**: Shell + Node.js runtime (~760MB)
- **worker-full**: All runtimes - shell, python, node, native (~1.6GB)

Workers automatically register their capabilities with the system, and the executor schedules actions to compatible workers.

## Prerequisites

- Docker 20.10+ with BuildKit enabled
- Docker Compose 2.0+
- PostgreSQL and RabbitMQ running (or use docker-compose)

### Default User

When using docker-compose, a default test user is **automatically created** on first startup:
- **Login**: `test@attune.local`
- **Password**: `TestPass123!`

No manual user creation is needed! See [Test User Setup](../testing/test-user-setup.md) for custom users.

## Quick Start

### 1. Build Worker Images

Build all worker variants:

```bash
# Enable BuildKit for faster builds
export DOCKER_BUILDKIT=1

# Build all worker types
docker build --target worker-base -t attune-worker:base -f docker/Dockerfile.worker .
docker build --target worker-python -t attune-worker:python -f docker/Dockerfile.worker .
docker build --target worker-node -t attune-worker:node -f docker/Dockerfile.worker .
docker build --target worker-full -t attune-worker:full -f docker/Dockerfile.worker .
```

Or build with docker-compose:

```bash
docker-compose build worker-shell worker-python worker-node worker-full
```

### 2. Start Workers

Start specific worker types:

```bash
# Start just the Python worker
docker-compose up -d worker-python

# Start multiple worker types
docker-compose up -d worker-shell worker-python worker-full

# Start all workers (includes automatic user creation)
docker-compose up -d worker-shell worker-python worker-node worker-full
```

**Note**: The `init-user` service automatically creates the default test user after migrations complete. Workers depend on this service, so user creation happens before workers start.

### 3. Verify Workers are Running

Check worker status:

```bash
# View running workers
docker-compose ps

# Check logs
docker-compose logs -f worker-python

# Verify registration in database
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, worker_type, status, capabilities->>'runtimes' as runtimes FROM worker;"
```

Expected output:
```
       name        | worker_type | status |        runtimes
-------------------+-------------+--------+-------------------------
 worker-shell-01   | container   | active | shell
 worker-python-01  | container   | active | shell,python
 worker-node-01    | container   | active | shell,node
 worker-full-01    | container   | active | shell,python,node,native
```

### 4. Test Action Execution

First, get an auth token with the default user:

```bash
# Login to get token
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"login":"test@attune.local","password":"TestPass123!"}' \
  | jq -r '.data.access_token')
```

Execute a shell action:

```bash
# Via API
curl -X POST http://localhost:8080/api/v1/executions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "action": "core.echo",
    "parameters": {"message": "Hello from containerized worker!"}
  }'
```

Execute a Python action:

```bash
curl -X POST http://localhost:8080/api/v1/executions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "action": "core.python_hello",
    "parameters": {"name": "Docker"}
  }'
```

## Scaling Workers

### Run Multiple Instances

Scale workers horizontally:

```bash
# Scale Python workers to 3 instances
docker-compose up -d --scale worker-python=3

# Scale multiple worker types
docker-compose up -d --scale worker-python=3 --scale worker-shell=2
```

**Note**: When scaling, you must set unique worker names. Use docker-compose override:

```yaml
# docker-compose.override.yml
services:
  worker-python:
    environment:
      # Use container hostname as worker name for uniqueness
      ATTUNE_WORKER_NAME: worker-python-${HOSTNAME:-01}
```

Or run individual containers:

```bash
docker run -d --name worker-python-01 \
  -e ATTUNE_WORKER_NAME=worker-python-01 \
  -e ATTUNE_WORKER_RUNTIMES=shell,python \
  -e ATTUNE__DATABASE__URL=postgresql://attune:attune@postgres:5432/attune \
  -e ATTUNE__MESSAGE_QUEUE__URL=amqp://attune:attune@rabbitmq:5672 \
  --network attune_attune-network \
  attune-worker:python

docker run -d --name worker-python-02 \
  -e ATTUNE_WORKER_NAME=worker-python-02 \
  -e ATTUNE_WORKER_RUNTIMES=shell,python \
  -e ATTUNE__DATABASE__URL=postgresql://attune:attune@postgres:5432/attune \
  -e ATTUNE__MESSAGE_QUEUE__URL=amqp://attune:attune@rabbitmq:5672 \
  --network attune_attune-network \
  attune-worker:python
```

## Custom Worker Configuration

### Environment Variables

Key environment variables for worker configuration:

| Variable | Description | Example |
|----------|-------------|---------|
| `ATTUNE_WORKER_NAME` | Unique worker identifier | `worker-python-01` |
| `ATTUNE_WORKER_RUNTIMES` | Comma-separated runtime list | `shell,python` |
| `ATTUNE_WORKER_TYPE` | Worker type (local/remote/container) | `container` |
| `ATTUNE__DATABASE__URL` | PostgreSQL connection string | `postgresql://...` |
| `ATTUNE__MESSAGE_QUEUE__URL` | RabbitMQ connection string | `amqp://...` |
| `RUST_LOG` | Log level | `info`, `debug`, `trace` |

### Custom Runtime Capabilities

Create a worker with custom runtimes:

```bash
docker run -d \
  --name worker-custom \
  -e ATTUNE_WORKER_NAME=worker-custom-01 \
  -e ATTUNE_WORKER_RUNTIMES=shell,python,ruby \
  -e ATTUNE__DATABASE__URL=postgresql://attune:attune@postgres:5432/attune \
  -e ATTUNE__MESSAGE_QUEUE__URL=amqp://attune:attune@rabbitmq:5672 \
  -v ./packs:/opt/attune/packs:ro \
  attune-worker:full
```

### Resource Limits

Set CPU and memory limits:

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

## Building Custom Worker Images

### Extend Python Worker

Create a custom worker with additional Python packages:

```dockerfile
# Dockerfile.worker.custom
FROM attune-worker:python

USER root

# Install additional Python packages
RUN pip install --no-cache-dir \
    pandas \
    numpy \
    scikit-learn \
    boto3

USER attune

# Optionally add custom runtimes
ENV ATTUNE_WORKER_RUNTIMES="shell,python,ml"
```

Build and run:

```bash
docker build -t attune-worker:ml -f Dockerfile.worker.custom .

docker run -d --name worker-ml-01 \
  -e ATTUNE_WORKER_NAME=worker-ml-01 \
  -e ATTUNE__DATABASE__URL=postgresql://attune:attune@postgres:5432/attune \
  -e ATTUNE__MESSAGE_QUEUE__URL=amqp://attune:attune@rabbitmq:5672 \
  --network attune_attune-network \
  attune-worker:ml
```

### Add New Runtime

To add a new runtime (e.g., Ruby):

```dockerfile
FROM attune-worker:base

USER root

# Install Ruby
RUN apt-get update && apt-get install -y ruby-full && \
    rm -rf /var/lib/apt/lists/*

USER attune

ENV ATTUNE_WORKER_RUNTIMES="shell,ruby"
```

## Monitoring Workers

### Check Worker Health

```bash
# View all workers
docker-compose ps | grep worker

# Check specific worker logs
docker-compose logs -f worker-python

# Check worker resource usage
docker stats attune-worker-python

# Verify heartbeat
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, status, last_heartbeat FROM worker ORDER BY last_heartbeat DESC;"
```

### Worker Metrics

Query worker capabilities:

```bash
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, capabilities FROM worker;"
```

Check active executions per worker:

```bash
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT w.name, COUNT(e.id) as active_executions 
   FROM worker w 
   LEFT JOIN execution e ON e.worker = w.id AND e.status = 'running' 
   GROUP BY w.name;"
```

## Troubleshooting

### Worker Not Registering

**Symptom**: Worker starts but doesn't appear in database

**Check**:
1. Database connectivity:
   ```bash
   docker-compose logs worker-python | grep -i database
   ```

2. Environment variables:
   ```bash
   docker-compose exec worker-python env | grep ATTUNE
   ```

3. Manual registration check:
   ```bash
   docker-compose exec postgres psql -U attune -d attune -c \
     "SELECT * FROM worker WHERE name = 'worker-python-01';"
   ```

**Solution**: Verify `ATTUNE__DATABASE__URL` is correct and database is accessible.

### Actions Not Scheduled to Worker

**Symptom**: Actions queued but not executing

**Check**:
1. Worker status:
   ```bash
   docker-compose exec postgres psql -U attune -d attune -c \
     "SELECT name, status, capabilities FROM worker WHERE status = 'active';"
   ```

2. Runtime compatibility:
   ```bash
   # Check action runtime requirement
   docker-compose exec postgres psql -U attune -d attune -c \
     "SELECT a.ref, r.name as runtime FROM action a 
      JOIN runtime r ON a.runtime = r.id 
      WHERE a.ref = 'core.my_action';"
   
   # Check worker capabilities
   docker-compose exec postgres psql -U attune -d attune -c \
     "SELECT name, capabilities->>'runtimes' FROM worker WHERE status = 'active';"
   ```

**Solution**: Ensure worker has the required runtime in its capabilities.

### Worker Crashes or Restarts

**Check logs**:
```bash
docker-compose logs --tail=100 worker-python
```

**Common issues**:
- Out of memory: Increase memory limit
- Database connection lost: Check network connectivity
- RabbitMQ connection issues: Verify message queue is running

### Runtime Not Detected

**Symptom**: Worker starts with wrong runtimes

**Check auto-detection**:
```bash
docker-compose exec worker-python python3 --version
docker-compose exec worker-python node --version
```

**Solution**: Set `ATTUNE_WORKER_RUNTIMES` explicitly:
```yaml
environment:
  ATTUNE_WORKER_RUNTIMES: shell,python
```

## Best Practices

### 1. Use Specific Worker Types

Choose the right worker variant for your workload:
- Use `worker-base` for simple shell scripts
- Use `worker-python` for Python-heavy packs
- Use `worker-full` only when you need multiple runtimes

### 2. Set Resource Limits

Always set resource limits to prevent runaway containers:

```yaml
deploy:
  resources:
    limits:
      cpus: '2'
      memory: 2G
```

### 3. Mount Packs Read-Only

Protect pack files from modification:

```yaml
volumes:
  - ./packs:/opt/attune/packs:ro  # :ro = read-only
```

### 4. Use Unique Worker Names

When scaling, ensure each worker has a unique name:

```bash
ATTUNE_WORKER_NAME=worker-python-$(hostname)
```

### 5. Monitor Worker Health

Set up health checks and monitoring:
- Use Docker health checks
- Monitor heartbeat timestamps
- Track execution counts per worker

### 6. Handle Graceful Shutdown

Workers deregister on shutdown. Allow time for graceful shutdown:

```yaml
stop_grace_period: 30s
```

## Production Deployment

### Security Considerations

1. **Run as non-root**: Workers run as user `attune` (UID 1000)
2. **Read-only packs**: Mount pack directories read-only
3. **Network isolation**: Use dedicated Docker networks
4. **Secrets management**: Use environment variables or secrets management
5. **Resource limits**: Set CPU and memory constraints

### High Availability

Deploy multiple workers per runtime:

```bash
docker-compose up -d \
  --scale worker-python=3 \
  --scale worker-shell=2 \
  --scale worker-full=1
```

### Logging

Configure log aggregation:

```yaml
logging:
  driver: "json-file"
  options:
    max-size: "10m"
    max-file: "3"
```

Or use a logging driver:

```yaml
logging:
  driver: "syslog"
  options:
    syslog-address: "tcp://logserver:514"
```

## Next Steps

- Read the full [Worker Containerization Design](worker-containerization.md)
- Learn about [Pack Development](../packs/pack-structure.md)
- Explore [Production Deployment](production-deployment.md)
- Review [Worker Service Architecture](../architecture/worker-service.md)

## Reference

### Worker Image Sizes

| Image | Base | Size (approx) | Runtimes |
|-------|------|---------------|----------|
| `attune-worker:base` | Debian slim | ~580 MB | shell |
| `attune-worker:python` | Python 3.11 slim | ~1.2 GB | shell, python |
| `attune-worker:node` | Node 20 slim | ~760 MB | shell, node |
| `attune-worker:full` | Debian | ~1.6 GB | shell, python, node, native |

### Useful Commands

```bash
# Build workers
docker-compose build worker-python

# Start workers
docker-compose up -d worker-python

# Stop workers
docker-compose stop worker-python

# Remove workers
docker-compose rm -f worker-python

# View logs
docker-compose logs -f worker-python

# Execute shell in worker
docker-compose exec worker-python bash

# Check worker status in DB
docker-compose exec postgres psql -U attune -d attune -c \
  "SELECT name, status, capabilities FROM worker;"

# Restart worker
docker-compose restart worker-python

# Rebuild and restart
docker-compose up -d --build worker-python
```
