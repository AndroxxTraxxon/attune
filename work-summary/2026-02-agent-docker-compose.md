# Universal Worker Agent: Phase 4 — Docker Compose Integration

**Date**: 2026-02-05  
**Phase**: 4 of 7  
**Scope**: Docker Compose integration for agent-based workers

## Summary

Added Docker Compose infrastructure to make it trivial to add agent-based workers to an Attune deployment. Users can now inject the statically-linked `attune-agent` binary into any container image via a shared volume, turning it into a fully functional Attune worker with auto-detected runtimes — no Dockerfiles, no Rust compilation.

## Changes

### docker-compose.yaml
- Added `init-agent` service between `init-packs` and `rabbitmq`
  - Builds from `docker/Dockerfile.agent` (target: `agent-init`)
  - Copies the statically-linked binary to the `agent_bin` volume at `/opt/attune/agent/attune-agent`
  - Runs once (`restart: "no"`) and completes immediately
- Added `agent_bin` named volume to the volumes section

### docker-compose.agent.yaml (new)
- Override file with example agent-based worker services
- **Active (uncommented)**: `worker-ruby` using `ruby:3.3-slim`
- **Commented templates**: Python 3.12, NVIDIA CUDA GPU, and custom image workers
- All workers follow the same pattern: mount `agent_bin` read-only, use `attune-agent` as entrypoint, share standard volumes

### Makefile
- Added `docker-up-agent` target: `docker compose -f docker-compose.yaml -f docker-compose.agent.yaml up -d`
- Added `docker-down-agent` target: corresponding `down` command
- Updated `.PHONY` and help text

### docs/QUICKREF-agent-workers.md (new)
- Quick-reference guide for adding agent-based workers
- Covers: how it works, quick start (override file vs docker-compose.override.yaml), required volumes, required environment variables, runtime auto-detection, testing detection, examples (Ruby, Node.js, GPU, multi-runtime), comparison table (traditional vs agent workers), troubleshooting

## Usage

```bash
# Start everything including the Ruby agent worker
make docker-up-agent

# Or manually
docker compose -f docker-compose.yaml -f docker-compose.agent.yaml up -d

# Stop
make docker-down-agent
```

Adding a new runtime worker is ~12 lines of YAML in `docker-compose.override.yaml`:
```yaml
services:
  worker-my-runtime:
    image: my-org/my-image:latest
    depends_on:
      init-agent:
        condition: service_completed_successfully
      # ... standard health checks
    entrypoint: ["/opt/attune/agent/attune-agent"]
    volumes:
      - agent_bin:/opt/attune/agent:ro
      - packs_data:/opt/attune/packs:ro
      - runtime_envs:/opt/attune/runtime_envs
      - artifacts_data:/opt/attune/artifacts
      - ${ATTUNE_DOCKER_CONFIG_PATH:-./config.docker.yaml}:/opt/attune/config/config.yaml:ro
    networks:
      - attune-network
```

## Dependencies

- **Requires**: Phase 1 (agent binary build infrastructure) — `docker/Dockerfile.agent` must exist
- **Requires**: Phase 3 (WorkerService dual-mode refactor) — agent auto-detection and lazy env setup

## Next Steps

- **Phase 5**: API binary download endpoint (`GET /api/v1/agent/binary`)
- **Phase 6**: Database runtime registry extensions (runtime template packs)
- **Phase 7**: Kubernetes support (InitContainer pattern, Helm chart)