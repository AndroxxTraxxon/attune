# Attune Docker Quick Start Guide

Get Attune running in Docker in under 5 minutes!

## Prerequisites

- Docker Engine 20.10+ (with BuildKit support)
- Docker Compose V2 (included with Docker Desktop)
- 4GB+ available RAM
- Ports available: 3000, 5432, 5672, 6379, 8080, 8081, 15672

## TL;DR - Fastest Path

```bash
# Clone and enter directory
cd /path/to/attune

# Stop conflicting system services (if needed)
./scripts/stop-system-services.sh

# Start everything
docker compose up -d

# Check status
docker compose ps

# Access the UI
open http://localhost:3000
```

## Step-by-Step Setup

### 1. Prepare Your Environment

#### Stop System Services (if running)

If you have PostgreSQL, RabbitMQ, or Redis running on your system:

```bash
./scripts/stop-system-services.sh
```

This will:
- Stop PostgreSQL (port 5432)
- Stop RabbitMQ (ports 5672, 15672)
- Stop Redis (port 6379)
- Verify ports are free
- Optionally disable services on boot

**Alternative**: If you want to keep system services, see [PORT_CONFLICTS.md](./PORT_CONFLICTS.md) for changing Docker ports.

### 2. Start Attune

```bash
docker compose up -d
```

**What happens**:
1. Downloads Docker images (first time: ~5-10 min)
2. Creates network and volumes
3. Starts PostgreSQL, RabbitMQ, Redis
4. Runs database migrations automatically (16 migrations)
5. Starts Attune services (API, Executor, Worker, Sensor, Notifier)
6. Starts Web UI

### 3. Verify Services

```bash
# Check all services
docker compose ps

# Expected output - all should be "Up" or "Up (healthy)":
# NAME              STATUS
# attune-api        Up (healthy)
# attune-executor   Up (healthy)
# attune-notifier   Up (healthy)
# attune-sensor     Up (healthy)
# attune-web        Up (healthy)
# attune-worker     Up (healthy)  # May restart if Python missing
# attune-postgres   Up (healthy)
# attune-rabbitmq   Up (healthy)
# attune-redis      Up (healthy)
```

### 4. Access Services

| Service | URL | Purpose |
|---------|-----|---------|
| Web UI | http://localhost:3000 | Main interface |
| API | http://localhost:8080 | REST API |
| API Docs | http://localhost:8080/api-docs | Interactive API documentation |
| Health Check | http://localhost:8080/health | Service status |
| RabbitMQ Management | http://localhost:15672 | Queue monitoring (attune/attune) |

```bash
# Test API
curl http://localhost:8080/health

# Open Web UI
open http://localhost:3000  # macOS
xdg-open http://localhost:3000  # Linux
```

## Next Steps

### Create Your First User

```bash
# Option 1: Use the CLI (recommended)
docker compose exec api attune-service create-admin-user \
  --username admin \
  --email admin@example.com \
  --password changeme

# Option 2: Use the Web UI registration page
open http://localhost:3000/register
```

### Load Core Pack (Optional)

The core pack provides basic actions (HTTP requests, timers, etc.):

```bash
./scripts/load-core-pack.sh
```

### Explore the API

```bash
# Get JWT token
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"changeme"}' \
  | jq -r '.access_token')

# List packs
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/packs

# List actions
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/actions
```

## Common Commands

### Managing Services

```bash
# Start all services
docker compose up -d

# Stop all services
docker compose down

# Restart a specific service
docker compose restart api

# View logs
docker compose logs -f           # All services
docker compose logs -f api       # Single service
docker compose logs --tail 100 api  # Last 100 lines

# Check service status
docker compose ps
```

### Database Operations

```bash
# Access PostgreSQL
docker compose exec postgres psql -U attune -d attune

# View applied migrations
docker compose exec postgres psql -U attune -d attune \
  -c "SELECT * FROM _migrations ORDER BY applied_at;"

# Reset database (WARNING: Deletes all data)
docker compose down -v
docker compose up -d
```

### Troubleshooting

```bash
# View service logs
docker compose logs api --tail 50

# Check migrations
docker compose logs migrations

# Restart everything
docker compose restart

# Full reset (deletes all data)
docker compose down -v
docker compose up -d
```

## Troubleshooting

### Port Already in Use

**Error**: `address already in use`

**Solution**:
```bash
./scripts/stop-system-services.sh
```

Or see [PORT_CONFLICTS.md](./PORT_CONFLICTS.md) for alternatives.

### Services Keep Restarting

**Check logs**:
```bash
docker compose logs api --tail 20
```

**Common issues**:
- Database not ready → Wait 30 seconds, should auto-recover
- Configuration error → Check environment variables
- Migration failed → Check `docker compose logs migrations`

**Fix**:
```bash
docker compose down
docker compose up -d
```

### Migrations Failed

**View migration logs**:
```bash
docker compose logs migrations
```

**Reset and retry**:
```bash
docker compose down -v  # Deletes database
docker compose up -d
```

### Worker Service Failing

**Error**: `Python validation failed`

**Cause**: Worker container doesn't have Python installed

**Solution**: This is non-critical. Worker will be fixed in future update. Core services work fine without it.

### Can't Access Web UI

**Check**:
```bash
docker compose ps web
docker compose logs web --tail 20
```

**Try**:
```bash
docker compose restart web
open http://localhost:3000
```

## Configuration

### Environment Variables

Create a `.env` file in the project root to customize settings:

```bash
cp env.docker.example .env
```

Edit `.env`:
```bash
# Security (REQUIRED for production)
JWT_SECRET=your-random-secret-here
ENCRYPTION_KEY=your-32-plus-character-encryption-key-here

# Ports (optional)
API_PORT=8080
WEB_PORT=3000
POSTGRES_PORT=5432
```

### Custom Configuration

Override default config with environment variables:

```bash
# Format: ATTUNE__SECTION__KEY=value
export ATTUNE__SECURITY__JWT_SECRET=my-secret
export ATTUNE__DATABASE__URL=postgresql://custom:url@host:5432/db
export ATTUNE__WORKER__WORKER_TYPE=container
```

See [configuration.md](../docs/configuration/configuration.md) for all options.

## Stopping Attune

### Keep Data (Recommended)

```bash
docker compose down
```

Volumes persist, data retained. Next `docker compose up -d` restarts with existing data.

### Delete Everything

```bash
docker compose down -v  # WARNING: Deletes all data
```

Removes all volumes, containers, and networks. Next startup is fresh install.

## Production Deployment

This quick start is for **development only**. For production:

1. **Generate secure secrets**:
   ```bash
   openssl rand -base64 32  # JWT_SECRET
   openssl rand -base64 32  # ENCRYPTION_KEY
   ```

2. **Use proper .env file** with strong credentials

3. **Configure HTTPS** with reverse proxy (nginx, Traefik)

4. **Set up backups** for PostgreSQL volumes

5. **Use external database** (recommended) instead of containerized

6. **Monitor with logging/metrics** (Prometheus, Grafana)

See [production-deployment.md](../docs/deployment/production-deployment.md) for details.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Docker Compose                      │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │   Web UI     │  │   API        │  │  Executor    │ │
│  │  (React)     │  │  (Rust)      │  │  (Rust)      │ │
│  │  Port 3000   │  │  Port 8080   │  │              │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  Worker      │  │  Sensor      │  │  Notifier    │ │
│  │  (Rust)      │  │  (Rust)      │  │  (Rust)      │ │
│  │              │  │              │  │  Port 8081   │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│                                                         │
│  ────────────────  Infrastructure  ─────────────────── │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  PostgreSQL  │  │  RabbitMQ    │  │  Redis       │ │
│  │  Port 5432   │  │  Port 5672   │  │  Port 6379   │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│                                                         │
│  ┌────────────────────────────────────────────────┐   │
│  │  Migrations (runs once at startup)             │   │
│  └────────────────────────────────────────────────┘   │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Resources

- [Docker Documentation](./README.md)
- [Port Conflicts Guide](./PORT_CONFLICTS.md)
- [Build Race Conditions](./DOCKER_BUILD_RACE_CONDITIONS.md)
- [Production Deployment](../docs/deployment/production-deployment.md)
- [Configuration Guide](../docs/configuration/configuration.md)

## Getting Help

- **View logs**: `docker compose logs <service>`
- **Check documentation**: `docs/` directory
- **API documentation**: http://localhost:8080/api-docs (when running)
- **Report issues**: GitHub issues

## Quick Reference

### Essential Commands

```bash
# Start
docker compose up -d

# Stop
docker compose down

# Logs
docker compose logs -f

# Status
docker compose ps

# Restart
docker compose restart

# Reset (deletes data)
docker compose down -v && docker compose up -d
```

### Service URLs

- Web UI: http://localhost:3000
- API: http://localhost:8080
- API Docs: http://localhost:8080/api-docs
- RabbitMQ: http://localhost:15672 (attune/attune)

### Default Credentials

- PostgreSQL: `attune` / `attune`
- RabbitMQ: `attune` / `attune`
- First user: Create via CLI or Web UI

---

**Ready to automate?** Start building workflows in the Web UI at http://localhost:3000! 🚀
