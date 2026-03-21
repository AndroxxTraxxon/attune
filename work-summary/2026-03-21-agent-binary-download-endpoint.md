# Universal Worker Agent Phase 5: API Binary Download Endpoint

**Date**: 2026-03-21
**Phase**: Universal Worker Agent Phase 5
**Status**: Complete

## Overview

Implemented the API binary download endpoint for the Attune universal worker agent. This enables deployments where shared Docker volumes are impractical (Kubernetes, ECS, remote Docker hosts) by allowing containers to download the agent binary directly from the Attune API at startup.

## Changes

### New Files

- **`crates/api/src/routes/agent.rs`** — Two new unauthenticated API endpoints:
  - `GET /api/v1/agent/binary` — Streams the statically-linked `attune-agent` binary as `application/octet-stream`. Supports `?arch=x86_64|aarch64|arm64` query parameter (defaults to `x86_64`). Tries arch-specific binary (`attune-agent-{arch}`) first, falls back to generic (`attune-agent`). Uses `ReaderStream` for memory-efficient streaming. Optional bootstrap token authentication via `X-Agent-Token` header or `token` query parameter.
  - `GET /api/v1/agent/info` — Returns JSON metadata about available agent binaries (architectures, sizes, availability status, version).

- **`scripts/attune-agent-wrapper.sh`** — Bootstrap entrypoint script for containers without volume-mounted agent binary. Features:
  - Auto-detects host architecture via `uname -m`
  - Checks for volume-mounted binary first (zero-overhead fast path)
  - Downloads from API with retry logic (10 attempts, 5s delay) using `curl` or `wget`
  - Supports bootstrap token via `ATTUNE_AGENT_TOKEN` env var
  - Verifies downloaded binary compatibility
  - Configurable via `ATTUNE_AGENT_DIR`, `ATTUNE_AGENT_URL`, `ATTUNE_AGENT_ARCH` env vars

### Modified Files

- **`crates/common/src/config.rs`** — Added `AgentConfig` struct with `binary_dir` (path to agent binaries) and `bootstrap_token` (optional auth). Added `agent: Option<AgentConfig>` field to `Config`.

- **`crates/api/src/routes/mod.rs`** — Added `pub mod agent` and `pub use agent::routes as agent_routes`.

- **`crates/api/src/server.rs`** — Added `.merge(routes::agent_routes())` to the API v1 router.

- **`crates/api/src/openapi.rs`** — Registered both endpoints in OpenAPI paths, added `AgentBinaryInfo` and `AgentArchInfo` schemas, added `"agent"` tag. Updated endpoint count test assertions (+2 paths, +2 operations).

- **`config.docker.yaml`** — Added `agent.binary_dir: /opt/attune/agent` configuration.

- **`config.development.yaml`** — Added commented-out agent config pointing to local musl build output.

- **`docker-compose.yaml`** — API service now mounts `agent_bin` volume read-only at `/opt/attune/agent` and depends on `init-agent` service completing successfully.

- **`AGENTS.md`** — Updated development status (Phase 5 complete), updated agent_bin volume description, added agent config to Key Settings.

## Architecture Decisions

1. **Unauthenticated endpoint** — The agent needs to download its binary before it can authenticate with JWT. An optional lightweight bootstrap token (`agent.bootstrap_token`) provides security when needed.

2. **Streaming response** — Uses `tokio_util::io::ReaderStream` to stream the ~20MB binary without loading it entirely into memory.

3. **Architecture whitelist** — Only `x86_64`, `aarch64`, and `arm64` (alias) are accepted, preventing path traversal attacks.

4. **Graceful fallback** — Arch-specific binary (`attune-agent-x86_64`) → generic binary (`attune-agent`) → 404. This supports both multi-arch and single-arch deployments.

5. **Volume-first strategy** — The wrapper script checks for a volume-mounted binary before attempting download, so Docker Compose deployments with the `agent_bin` volume pay zero network overhead.

## Testing

- All 4 OpenAPI tests pass (including updated endpoint count: 59 paths, 83 operations)
- All 21 config tests pass (including `AgentConfig` integration)
- API crate compiles with zero warnings
- Common crate compiles with zero warnings