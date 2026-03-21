# Universal Worker Agent — Phase 1: Static Binary Build Infrastructure

**Date**: 2026-03-21

## Summary

Implemented Phase 1 of the Universal Worker Agent plan (`docs/plans/universal-worker-agent.md`), establishing the build infrastructure for a statically-linked `attune-agent` binary that can be injected into any container to turn it into an Attune worker.

## Problem

Adding support for new runtime environments (Ruby, Go, Java, R, etc.) required building custom Docker images for each combination. This meant modifying `Dockerfile.worker.optimized`, installing interpreters via apt, managing a combinatorial explosion of worker variants, and rebuilding images (~5 min) for every change.

## Solution

Phase 1 lays the groundwork for flipping the model: instead of baking the worker into custom images, a single static binary is injected into **any** container at startup. This phase delivers:

1. **TLS backend audit** — confirmed the worker crate has zero `native-tls` or `openssl` dependencies, making musl static linking viable without any TLS backend changes
2. **New binary target** — `attune-agent` alongside `attune-worker` in the same crate
3. **Runtime auto-detection module** — probes container environments for interpreters
4. **Dockerfile for static builds** — multi-stage musl cross-compilation
5. **Makefile targets** — local and Docker build commands

## Changes

### New Files

- **`crates/worker/src/agent_main.rs`** — Agent entrypoint with three-phase startup: (1) auto-detect runtimes or respect `ATTUNE_WORKER_RUNTIMES` override, (2) load config, (3) run `WorkerService`. Includes `--detect-only` flag for diagnostic probing.

- **`crates/worker/src/runtime_detect.rs`** — Database-free runtime detection module. Probes 8 interpreter families (shell, python, node, ruby, go, java, r, perl) via `which`-style PATH lookup with fallbacks. Captures version strings. 18 unit tests covering version parsing, display formatting, binary lookup, and detection pipeline.

- **`docker/Dockerfile.agent`** — Multi-stage Dockerfile:
  - `builder` stage: cross-compiles with `x86_64-unknown-linux-musl` target, BuildKit cache mounts
  - `agent-binary` stage: `FROM scratch` with just the static binary
  - `agent-init` stage: busybox-based for Docker Compose/K8s init container volume population

### Modified Files

- **`crates/worker/Cargo.toml`** — Added second `[[bin]]` target for `attune-agent`
- **`crates/worker/src/lib.rs`** — Added `pub mod runtime_detect`
- **`Makefile`** — Added targets: `build-agent` (local musl build), `docker-build-agent`, `run-agent`, `run-agent-release`
- **`docker/Dockerfile.worker.optimized`** — Added `agent_main.rs` stub for second binary target
- **`docker/Dockerfile.optimized`** — Added `agent_main.rs` stub
- **`docker/Dockerfile.sensor.optimized`** — Added `agent_main.rs` stub
- **`docker/Dockerfile.pack-binaries`** — Added `agent_main.rs` stub
- **`AGENTS.md`** — Documented agent service, runtime auto-detection, Docker build, Makefile targets

## Key Design Decisions

1. **Same crate, new binary** — The agent lives as a second `[[bin]]` target in `crates/worker` rather than a separate crate. This gives zero code duplication and the same test suite covers both binaries. Can be split into a separate crate later if binary size becomes a concern.

2. **No TLS changes needed** — The plan anticipated needing to switch from `native-tls` to `rustls` workspace-wide. Audit revealed the worker crate already uses `rustls` exclusively (`native-tls` only enters via `tokio-tungstenite` in CLI and `ldap3` in API, neither of which the worker depends on).

3. **Database-free detection** — The `runtime_detect` module is deliberately separate from `attune_common::runtime_detection` (which queries the database). The agent must discover runtimes before any DB connectivity, using pure filesystem probing.

4. **All Dockerfiles updated** — Since the worker crate now has two binary targets, all Dockerfiles that create workspace stubs for `cargo fetch` need a stub for `agent_main.rs`. Missing this would break Docker builds.

## Verification

- `cargo check --all-targets --workspace` — zero warnings ✅
- `cargo test -p attune-worker` — all 37 tests pass (18 new runtime_detect tests + 19 existing) ✅
- `cargo run --bin attune-agent -- --detect-only` — successfully detected 6 runtimes on dev machine ✅
- `cargo run --bin attune-agent -- --help` — correct CLI documentation ✅

## Next Steps (Phases 2–7)

See `docs/plans/universal-worker-agent.md` for the remaining phases:
- **Phase 2**: Integration with worker registration (auto-detected runtimes → DB)
- **Phase 3**: Refactor `WorkerService` for dual modes (lazy env setup)
- **Phase 4**: Docker Compose init service for agent volume
- **Phase 5**: API binary download endpoint
- **Phase 6**: Database runtime registry extensions
- **Phase 7**: Kubernetes support (init containers, Helm chart)