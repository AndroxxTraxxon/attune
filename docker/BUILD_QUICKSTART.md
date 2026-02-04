# Docker Build Quick Start

## TL;DR - Fastest & Most Reliable Build

```bash
make docker-cache-warm    # ~5-6 minutes (first time only)
make docker-build         # ~15-20 minutes (first time), ~2-5 min (incremental)
make docker-up            # Start all services
```

## Why Two Steps?

Building multiple Rust services in parallel can cause race conditions in the shared Cargo cache. Pre-warming the cache prevents this.

## Build Methods

### Method 1: Cache Warming (Recommended)
**Best for: First-time builds, after dependency updates**

```bash
# Step 1: Pre-load cache
make docker-cache-warm

# Step 2: Build all services
make docker-build

# Step 3: Start
make docker-up
```

⏱️ **Timing**: ~20-25 min first time, ~2-5 min incremental

### Method 2: Direct Build
**Best for: Quick builds, incremental changes**

```bash
docker compose build
make docker-up
```

⏱️ **Timing**: ~25-30 min first time (sequential due to cache locking), ~2-5 min incremental

### Method 3: Single Service
**Best for: Developing one service**

```bash
docker compose build api
docker compose up -d api
```

⏱️ **Timing**: ~5-6 min first time, ~30-60 sec incremental

## Common Commands

| Command | Description | Time |
|---------|-------------|------|
| `make docker-cache-warm` | Pre-load build cache | ~5-6 min |
| `make docker-build` | Build all images | ~2-5 min (warm cache) |
| `make docker-up` | Start all services | ~30 sec |
| `make docker-down` | Stop all services | ~10 sec |
| `make docker-logs` | View logs | - |
| `docker compose build api` | Build single service | ~30-60 sec (warm cache) |

## Troubleshooting

### "File exists (os error 17)" during build

Race condition detected. Solutions:

```bash
# Option 1: Clear cache and retry
docker builder prune -af
make docker-cache-warm
make docker-build

# Option 2: Build sequentially
docker compose build --no-parallel
```

### Builds are very slow

```bash
# Check cache size
docker system df -v | grep buildkit

# Prune if >20GB
docker builder prune --keep-storage 10GB
```

### Service won't start

```bash
# Check logs
docker compose logs api

# Restart single service
docker compose restart api

# Full restart
make docker-down
make docker-up
```

## Development Workflow

### Making Code Changes

```bash
# 1. Edit code
vim crates/api/src/routes/actions.rs

# 2. Rebuild affected service
docker compose build api

# 3. Restart it
docker compose up -d api

# 4. Check logs
docker compose logs -f api
```

### After Pulling Latest Code

```bash
# If dependencies changed (check Cargo.lock)
make docker-cache-warm
make docker-build
make docker-up

# If only code changed
make docker-build
make docker-up
```

## Environment Setup

Ensure BuildKit is enabled:

```bash
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Or add to ~/.bashrc or ~/.zshrc
echo 'export DOCKER_BUILDKIT=1' >> ~/.bashrc
echo 'export COMPOSE_DOCKER_CLI_BUILD=1' >> ~/.bashrc
```

## Architecture

```
Dockerfile (multi-stage)
  ├── Stage 1: Builder
  │   ├── Install Rust + build deps
  │   ├── Copy source code
  │   ├── Build service (with cache mounts)
  │   └── Extract binary
  └── Stage 2: Runtime
      ├── Minimal Debian image
      ├── Copy binary from builder
      ├── Copy configs & migrations
      └── Run service

Services built from same Dockerfile:
  - api (port 8080)
  - executor
  - worker
  - sensor
  - notifier (port 8081)
  
Separate Dockerfile.web for React UI (port 3000)
```

## Cache Mounts Explained

| Mount | Purpose | Size | Sharing |
|-------|---------|------|---------|
| `/usr/local/cargo/registry` | Downloaded crates | ~1-2GB | locked |
| `/usr/local/cargo/git` | Git dependencies | ~100-500MB | locked |
| `/build/target` | Compiled artifacts | ~5-10GB | locked |

**`sharing=locked`** = Only one build at a time (prevents race conditions)

## Next Steps

- 📖 Read full details: [DOCKER_BUILD_RACE_CONDITIONS.md](./DOCKER_BUILD_RACE_CONDITIONS.md)
- 🐳 Docker configuration: [README.md](./README.md)
- 🚀 Quick start guide: [../docs/guides/quick-start.md](../docs/guides/quick-start.md)

## Questions?

- **Why is first build so slow?** Compiling Rust + all dependencies (~200+ crates)
- **Why cache warming?** Prevents multiple builds fighting over the same files
- **Can I build faster?** Yes, but with reliability trade-offs (see full docs)
- **Do I always need cache warming?** No, only for first build or dependency updates