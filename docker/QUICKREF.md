# Docker Quick Reference

Quick reference for common Docker commands when working with Attune.

## Table of Contents

- [Quick Start](#quick-start)
- [Service Management](#service-management)
- [Viewing Logs](#viewing-logs)
- [Database Operations](#database-operations)
- [Debugging](#debugging)
- [Maintenance](#maintenance)
- [Troubleshooting](#troubleshooting)
- [BuildKit Cache](#buildkit-cache)

## Quick Start

**Enable BuildKit first (recommended):**
```bash
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1
```

```bash
# One-command setup (generates secrets, builds, starts)
./docker/quickstart.sh

# Manual setup
cp env.docker.example .env
# Edit .env and set JWT_SECRET and ENCRYPTION_KEY
docker compose up -d
```

## Service Management

### Start/Stop Services

```bash
# Start all services
docker compose up -d

# Start specific services
docker compose up -d postgres rabbitmq redis
docker compose up -d api executor worker

# Stop all services
docker compose down

# Stop and remove volumes (WARNING: deletes data)
docker compose down -v

# Restart services
docker compose restart

# Restart specific service
docker compose restart api
```

### Build Images

```bash
# Build all images
docker compose build

# Build specific service
docker compose build api
docker compose build web

# Build without cache (clean build)
docker compose build --no-cache

# Build with BuildKit (faster incremental builds)
DOCKER_BUILDKIT=1 docker compose build

# Pull latest base images and rebuild
docker compose build --pull
```

### Scale Services

```bash
# Run multiple worker instances
docker compose up -d --scale worker=3

# Run multiple executor instances
docker compose up -d --scale executor=2
```

## Viewing Logs

```bash
# View all logs (follow mode)
docker compose logs -f

# View specific service logs
docker compose logs -f api
docker compose logs -f worker
docker compose logs -f postgres

# View last N lines
docker compose logs --tail=100 api

# View logs since timestamp
docker compose logs --since 2024-01-01T10:00:00 api

# View logs without following
docker compose logs api
```

## Database Operations

### Access PostgreSQL

```bash
# Connect to database
docker compose exec postgres psql -U attune

# Run SQL query
docker compose exec postgres psql -U attune -c "SELECT COUNT(*) FROM attune.execution;"

# List tables
docker compose exec postgres psql -U attune -c "\dt attune.*"

# Describe table
docker compose exec postgres psql -U attune -c "\d attune.execution"
```

### Backup and Restore

```bash
# Backup database
docker compose exec postgres pg_dump -U attune > backup.sql

# Restore database
docker compose exec -T postgres psql -U attune < backup.sql

# Backup specific table
docker compose exec postgres pg_dump -U attune -t attune.execution > executions_backup.sql
```

### Run Migrations

```bash
# Check migration status
docker compose exec api sqlx migrate info

# Run pending migrations
docker compose exec api sqlx migrate run

# Revert last migration
docker compose exec api sqlx migrate revert
```

## Debugging

### Access Service Shell

```bash
# API service
docker compose exec api /bin/sh

# Worker service
docker compose exec worker /bin/sh

# Database
docker compose exec postgres /bin/bash
```

### Check Service Status

```bash
# View running services and health
docker compose ps

# View detailed container info
docker inspect attune-api

# View resource usage
docker stats

# View container processes
docker compose top
```

### Test Connections

```bash
# Test API health
curl http://localhost:8080/health

# Test from inside container
docker compose exec worker curl http://api:8080/health

# Test database connection
docker compose exec api sh -c 'psql postgresql://attune:attune@postgres:5432/attune -c "SELECT 1"'

# Test RabbitMQ
docker compose exec rabbitmq rabbitmqctl status
docker compose exec rabbitmq rabbitmqctl list_queues
```

### View Configuration

```bash
# View environment variables
docker compose exec api env

# View config file
docker compose exec api cat /opt/attune/config.docker.yaml

# View generated docker compose config
docker compose config
```

## Maintenance

### Update Images

```bash
# Pull latest base images
docker compose pull

# Rebuild with latest bases
docker compose build --pull

# Restart with new images
docker compose up -d
```

### Clean Up

```bash
# Remove stopped containers
docker compose down

# Remove volumes (deletes data)
docker compose down -v

# Remove images
docker compose down --rmi local

# Prune unused Docker resources
docker system prune -f

# Prune everything including volumes
docker system prune -a --volumes
```

### View Disk Usage

```bash
# Docker disk usage summary
docker system df

# Detailed breakdown
docker system df -v

# Volume sizes
docker volume ls -q | xargs docker volume inspect --format '{{ .Name }}: {{ .Mountpoint }}'
```

## Troubleshooting

### Service Won't Start

```bash
# Check logs for errors
docker compose logs <service-name>

# Check if dependencies are healthy
docker compose ps

# Verify configuration
docker compose config --quiet

# Try rebuilding
docker compose build --no-cache <service-name>
docker compose up -d <service-name>
```

### Database Connection Issues

```bash
# Verify PostgreSQL is running
docker compose ps postgres

# Check PostgreSQL logs
docker compose logs postgres

# Test connection
docker compose exec postgres pg_isready -U attune

# Check network
docker compose exec api ping postgres
```

### RabbitMQ Issues

```bash
# Check RabbitMQ status
docker compose exec rabbitmq rabbitmqctl status

# Check queues
docker compose exec rabbitmq rabbitmqctl list_queues

# Check connections
docker compose exec rabbitmq rabbitmqctl list_connections

# Access management UI
open http://localhost:15672
```

### Permission Errors

```bash
# Fix volume permissions (UID 1000 = attune user)
sudo chown -R 1000:1000 ./packs
sudo chown -R 1000:1000 ./logs

# Check current permissions
ls -la ./packs
ls -la ./logs
```

### Network Issues

```bash
# List networks
docker network ls

# Inspect attune network
docker network inspect attune_attune-network

# Test connectivity between services
docker compose exec api ping postgres
docker compose exec api ping rabbitmq
docker compose exec api ping redis
```

### Reset Everything

```bash
# Nuclear option - complete reset
docker compose down -v --rmi all
docker system prune -a --volumes
rm -rf ./logs/*

# Then rebuild
./docker/quickstart.sh
```

## Environment Variables

Override configuration with environment variables:

```bash
# In .env file or export
export ATTUNE__DATABASE__URL=postgresql://user:pass@host:5432/db
export ATTUNE__LOG__LEVEL=debug
export RUST_LOG=trace

# Then restart services
docker compose up -d
```

## Useful Aliases

Add to `~/.bashrc` or `~/.zshrc`:

```bash
alias dc='docker compose'
alias dcu='docker compose up -d'
alias dcd='docker compose down'
alias dcl='docker compose logs -f'
alias dcp='docker compose ps'
alias dcr='docker compose restart'

# Attune-specific
alias attune-logs='docker compose logs -f api executor worker sensor'
alias attune-db='docker compose exec postgres psql -U attune'
alias attune-shell='docker compose exec api /bin/sh'
```

## Makefile Commands

Project Makefile includes Docker shortcuts:

```bash
make docker-build          # Build all images
make docker-up             # Start services
make docker-down           # Stop services
make docker-logs           # View logs
make docker-ps             # View status
make docker-shell-api      # Access API shell
make docker-shell-db       # Access database
make docker-clean          # Clean up resources
```

## BuildKit Cache

### Enable BuildKit

BuildKit dramatically speeds up incremental builds (5+ minutes → 30-60 seconds):

```bash
# Enable for current session
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Enable globally
./docker/enable-buildkit.sh

# Or manually add to ~/.bashrc or ~/.zshrc
echo 'export DOCKER_BUILDKIT=1' >> ~/.bashrc
echo 'export COMPOSE_DOCKER_CLI_BUILD=1' >> ~/.bashrc
source ~/.bashrc
```

### Manage Build Cache

```bash
# View cache size
docker system df

# View detailed cache info
docker system df -v

# Clear build cache
docker builder prune

# Clear all unused cache
docker builder prune -a

# Clear specific cache type
docker builder prune --filter type=exec.cachemount
```

### Cache Performance

**With BuildKit:**
- First build: ~5-6 minutes
- Code-only changes: ~30-60 seconds
- Dependency changes: ~2-3 minutes
- Cache size: ~5-10GB

**Without BuildKit:**
- Every build: ~5-6 minutes (no incremental compilation)

### Verify BuildKit is Working

```bash
# Check environment
echo $DOCKER_BUILDKIT

# Test BuildKit with cache mounts
cat > /tmp/test.Dockerfile <<EOF
FROM alpine:latest
RUN --mount=type=cache,target=/cache echo "BuildKit works!"
EOF

DOCKER_BUILDKIT=1 docker build -f /tmp/test.Dockerfile /tmp
# If successful, BuildKit is working
```

## Production Checklist

Before deploying to production:

- [ ] Generate secure `JWT_SECRET` and `ENCRYPTION_KEY`
- [ ] Change all default passwords (database, RabbitMQ)
- [ ] Configure proper `CORS_ORIGINS`
- [ ] Set up TLS/SSL with reverse proxy
- [ ] Configure persistent volumes with backups
- [ ] Set up log aggregation
- [ ] Configure monitoring and alerting
- [ ] Review resource limits
- [ ] Test backup/restore procedures
- [ ] Document incident response procedures

## Additional Resources

- [Full Docker Deployment Guide](../docs/docker-deployment.md)
- [Docker Directory README](README.md)
- [Production Deployment](../docs/production-deployment.md)
- [Configuration Guide](../docs/configuration.md)