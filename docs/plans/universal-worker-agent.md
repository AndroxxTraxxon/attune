# Universal Worker Agent Injection

## Overview

This plan describes a new deployment model for Attune workers: a **statically-linked agent binary** that can be injected into any Docker container at runtime, turning arbitrary images into Attune workers. This eliminates the need to build custom worker Docker images for each runtime environment.

### Problem

Today, every Attune worker is a purpose-built Docker image: the same `attune-worker` Rust binary baked into Debian images with specific interpreters installed (see `docker/Dockerfile.worker.optimized`). Adding a new runtime (e.g., Ruby, Go, Java, R) means:

1. Modifying `Dockerfile.worker.optimized` to add a new build stage
2. Installing the interpreter via apt or a package repository
3. Managing the combinatorial explosion of worker variants
4. Rebuilding images (~5 min) for every change
5. Standardizing on `debian:bookworm-slim` as the base (not the runtime's official image)

### Solution

Flip the model: **any Docker image becomes an Attune worker** by injecting a lightweight agent binary at container startup. The agent binary is a statically-linked (musl) Rust executable that connects to MQ/DB, consumes execution messages, spawns subprocesses, and reports results — functionally identical to the current worker, but packaged for universal deployment.

Want Ruby support? Point at `ruby:3.3` and go. Need a GPU runtime? Use `nvidia/cuda:12.3-runtime`. Need a specific Python version with scientific libraries pre-installed? Use any image that has them.

### Industry Precedent

This pattern is battle-tested in major CI/CD and workflow systems:

| System | Pattern | How It Works |
|--------|---------|-------------|
| **Tekton** | InitContainer + shared volume | Copies a static Go `entrypoint` binary into an `emptyDir`; overrides the user container's entrypoint to use it. Steps coordinate via file-based signaling. |
| **Argo Workflows (Emissary)** | InitContainer + sidecar | The `emissary` binary runs as both an init container and a sidecar. Disk-based coordination, no Docker socket, no privileged access. |
| **GitLab CI Runner (Step Runner)** | Binary injection | Newer "Native Step Runner" mode injects a `step-runner` binary into the build container and adjusts `$PATH`. Communicates via gRPC. |
| **Istio** | Mutating webhook | Kubernetes admission controller adds init + sidecar containers transparently. |

The **Tekton/Argo pattern** (static binary + shared volume) is the best fit for Attune because:

- It works with Docker Compose (not K8s-only) via bind mounts / named volumes
- It requires zero dependencies in the user image (just a Linux kernel)
- A static Rust binary (musl-linked) is ~15–25MB and runs anywhere
- No privileged access, no Docker socket needed inside the container

### Compatibility

This plan is **purely additive**. Nothing changes for existing workers:

- `Dockerfile.worker.optimized` and its four targets remain unchanged and functional
- Current `docker-compose.yaml` worker services keep working
- All MQ protocols, DB schemas, and execution flows remain identical
- The agent is just another way to run the same execution engine

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                      Attune Control Plane                        │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │   API    │    │ Executor │    │ RabbitMQ │    │ Postgres │  │
│  └────┬─────┘    └────┬─────┘    └─────┬────┘    └────┬─────┘  │
│       │               │               │               │        │
└───────┼───────────────┼───────────────┼───────────────┼────────┘
        │               │               │               │
  ┌─────┼───────────────┼───────────────┼───────────────┼──────┐
  │     ▼               ▼               ▼               ▼      │
  │  ┌──────────────────────────────────────────────────────┐  │
  │  │            attune-agent (injected binary)            │  │
  │  │  ┌──────────┐ ┌──────────┐ ┌─────────┐ ┌────────┐  │  │
  │  │  │MQ Client │ │DB Client │ │ Process │ │Artifact│  │  │
  │  │  │(lapin)   │ │(sqlx)    │ │Executor │ │Manager │  │  │
  │  │  └──────────┘ └──────────┘ └─────────┘ └────────┘  │  │
  │  └──────────────────────────────────────────────────────┘  │
  │                                                            │
  │  ┌──────────────────────────────────────────────────────┐  │
  │  │     User Container (ANY Docker image)                │  │
  │  │     ruby:3.3, python:3.12, nvidia/cuda, alpine, ...  │  │
  │  └──────────────────────────────────────────────────────┘  │
  │                                                            │
  │  Shared Volumes:                                           │
  │    /opt/attune/agent/       (agent binary, read-only)      │
  │    /opt/attune/packs/       (pack files, read-only)        │
  │    /opt/attune/runtime_envs/(virtualenvs, node_modules)    │
  │    /opt/attune/artifacts/   (artifact files)               │
  └────────────────────────────────────────────────────────────┘
```

### Agent vs. Full Worker Comparison

The agent binary is functionally identical to the current `attune-worker`. The difference is packaging and startup behavior:

| Capability | Full Worker (`attune-worker`) | Agent (`attune-agent`) |
|-----------|------------------------------|------------------------|
| MQ consumption | ✅ | ✅ |
| DB access | ✅ | ✅ |
| Process execution | ✅ | ✅ |
| Artifact management | ✅ | ✅ |
| Secret management | ✅ | ✅ |
| Cancellation / timeout | ✅ | ✅ |
| Heartbeat | ✅ | ✅ |
| Runtime env setup (venvs) | ✅ Proactive at startup | ✅ Lazy on first use |
| Version verification | ✅ Full sweep at startup | ✅ On-demand per-execution |
| Runtime discovery | Manual (`ATTUNE_WORKER_RUNTIMES`) | Auto-detect + optional manual override |
| Linking | Dynamic (glibc) | Static (musl) |
| Base image requirement | `debian:bookworm-slim` | None (any Linux container) |
| Binary size | ~30–50MB | ~15–25MB (stripped, musl) |

### Binary Distribution Methods

Two methods for getting the agent binary into a container:

**Method A: Shared Volume (Docker Compose — recommended)**

An init container copies the agent binary into a Docker named volume. User containers mount this volume read-only and use the binary as their entrypoint.

**Method B: HTTP Download (remote / cloud deployments)**

A new API endpoint (`GET /api/v1/agent/binary`) serves the static binary. A small wrapper script in the container downloads it on first run. Useful for Kubernetes, ECS, or remote Docker hosts where shared volumes are impractical.

## Implementation Phases

### Phase 1: Static Binary Build Infrastructure

**Goal**: Produce a statically-linked `attune-agent` binary that runs in any Linux container.

**Effort**: 3–5 days

**Dependencies**: None

#### 1.1 TLS Backend Audit and Alignment

The agent must link statically with musl. This requires all TLS to use `rustls` (pure Rust) instead of OpenSSL/native-tls.

**Current state** (from `Cargo.toml` workspace dependencies):
- `sqlx`: Already uses `runtime-tokio-rustls` ✅
- `reqwest`: Uses default features (native-tls) — needs `rustls-tls` feature ❌
- `tokio-tungstenite`: Uses `native-tls` feature — needs `rustls` ❌
- `lapin` (v4.3): Uses native-tls by default — needs `rustls` feature ❌

**Changes needed in workspace `Cargo.toml`**:

```toml
# Change reqwest to use rustls
reqwest = { version = "0.13", features = ["json", "rustls-tls"], default-features = false }

# Change tokio-tungstenite to use rustls
tokio-tungstenite = { version = "0.28", features = ["rustls"] }

# Check lapin's TLS features — if using amqps://, need rustls support.
# For plain amqp:// (typical in Docker Compose), no TLS needed.
# For production amqps://, evaluate lapin's rustls support or use a TLS-terminating proxy.
```

**Important**: These changes affect the entire workspace. Test all services (`api`, `executor`, `worker`, `notifier`, `sensor`, `cli`) after switching TLS backends. If switching workspace-wide is too disruptive, use feature flags to conditionally select the TLS backend for the agent build only.

**Alternative**: If workspace-wide rustls migration is too risky, the agent crate can override specific dependencies:

```toml
[dependencies]
reqwest = { workspace = true, default-features = false, features = ["json", "rustls-tls"] }
```

#### 1.2 New Crate or New Binary Target

**Option A (recommended): New binary target in the worker crate**

Add a second binary target to `crates/worker/Cargo.toml`:

```toml
[[bin]]
name = "attune-worker"
path = "src/main.rs"

[[bin]]
name = "attune-agent"
path = "src/agent_main.rs"
```

This reuses all existing code — `ActionExecutor`, `ProcessRuntime`, `WorkerService`, `RuntimeRegistry`, `SecretManager`, `ArtifactManager`, etc. The agent entrypoint is a thin wrapper with different startup behavior (auto-detection instead of manual config).

**Pros**: Zero code duplication. Same test suite covers both binaries.
**Cons**: The agent binary includes unused code paths (e.g., full worker service setup).

**Option B: New crate `crates/agent/`**

```
crates/agent/
├── Cargo.toml          # Depends on attune-common + selected worker modules
├── src/
│   ├── main.rs         # Entry point
│   ├── agent.rs        # Core agent loop
│   ├── detect.rs       # Runtime auto-detection
│   └── health.rs       # Health check (file-based or tiny HTTP)
```

**Pros**: Cleaner separation, can minimize binary size by excluding unused deps.
**Cons**: Requires extracting shared execution code into a library or duplicating it.

**Recommendation**: Start with **Option A** (new binary target in worker crate) for speed. Refactor into a separate crate later if binary size becomes a concern.

#### 1.3 Agent Entrypoint (`src/agent_main.rs`)

The agent entrypoint differs from `main.rs` in:

1. **Runtime auto-detection** instead of relying on `ATTUNE_WORKER_RUNTIMES`
2. **Lazy environment setup** instead of proactive startup sweep
3. **Simplified config loading** — env vars are the primary config source (no config file required, but supported if mounted)
4. **Container-aware defaults** — sensible defaults for paths, timeouts, concurrency

```
src/agent_main.rs responsibilities:
  1. Parse CLI args / env vars for DB URL, MQ URL, worker name
  2. Run runtime auto-detection (Phase 2) to discover available interpreters
  3. Initialize WorkerService with detected capabilities
  4. Start the normal execution consumer loop
  5. Handle SIGTERM/SIGINT for graceful shutdown
```

#### 1.4 Dockerfile for Agent Binary

Create `docker/Dockerfile.agent`:

```dockerfile
# Stage 1: Build the statically-linked agent binary
FROM rust:1.83-bookworm AS builder

RUN apt-get update && apt-get install -y musl-tools
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build
ENV RUST_MIN_STACK=67108864

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY migrations/ ./migrations/
COPY .sqlx/ ./.sqlx/

# Build only the agent binary, statically linked
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=shared \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=shared \
    --mount=type=cache,id=agent-target,target=/build/target \
    SQLX_OFFLINE=true cargo build --release \
      --target x86_64-unknown-linux-musl \
      --bin attune-agent \
    && cp /build/target/x86_64-unknown-linux-musl/release/attune-agent /attune-agent \
    && strip /attune-agent

# Stage 2: Minimal image for volume population
FROM scratch AS agent-binary
COPY --from=builder /attune-agent /attune-agent
```

**Multi-architecture support**: For ARM64 (Apple Silicon, Graviton), add a parallel build stage targeting `aarch64-unknown-linux-musl`. Use Docker buildx multi-platform builds or separate images.

#### 1.5 Makefile Targets

Add to `Makefile`:

```makefile
build-agent:
	SQLX_OFFLINE=true cargo build --release --target x86_64-unknown-linux-musl --bin attune-agent
	strip target/x86_64-unknown-linux-musl/release/attune-agent

docker-build-agent:
	docker buildx build -f docker/Dockerfile.agent -t attune-agent:latest .
```

---

### Phase 2: Runtime Auto-Detection

**Goal**: The agent automatically discovers what interpreters are available in the container, without requiring `ATTUNE_WORKER_RUNTIMES` to be set.

**Effort**: 1–2 days

**Dependencies**: Phase 1 (agent binary exists)

#### 2.1 Interpreter Discovery Module

Create a new module (in `crates/worker/src/` or `crates/common/src/`) that probes the container's filesystem for known interpreters:

```
src/runtime_detect.rs (or extend existing crates/common/src/runtime_detection.rs)

struct DetectedInterpreter {
    runtime_name: String,       // "python", "ruby", "node", etc.
    binary_path: PathBuf,       // "/usr/local/bin/python3"
    version: Option<String>,    // "3.12.1" (parsed from version command output)
}

/// Probe the container for available interpreters.
///
/// For each known runtime, checks common binary names via `which` or
/// direct path existence, then runs the version command to extract
/// the version string.
fn detect_interpreters() -> Vec<DetectedInterpreter> {
    let probes = [
        InterpreterProbe {
            runtime_name: "python",
            binaries: &["python3", "python"],
            version_flag: "--version",
            version_regex: r"Python (\d+\.\d+\.\d+)",
        },
        InterpreterProbe {
            runtime_name: "node",
            binaries: &["node", "nodejs"],
            version_flag: "--version",
            version_regex: r"v(\d+\.\d+\.\d+)",
        },
        InterpreterProbe {
            runtime_name: "ruby",
            binaries: &["ruby"],
            version_flag: "--version",
            version_regex: r"ruby (\d+\.\d+\.\d+)",
        },
        InterpreterProbe {
            runtime_name: "go",
            binaries: &["go"],
            version_flag: "version",
            version_regex: r"go(\d+\.\d+\.\d+)",
        },
        InterpreterProbe {
            runtime_name: "java",
            binaries: &["java"],
            version_flag: "-version",
            version_regex: r#""(\d+[\.\d+]*)""#,
        },
        InterpreterProbe {
            runtime_name: "perl",
            binaries: &["perl"],
            version_flag: "--version",
            version_regex: r"v(\d+\.\d+\.\d+)",
        },
        InterpreterProbe {
            runtime_name: "r",
            binaries: &["Rscript", "R"],
            version_flag: "--version",
            version_regex: r"R.*version (\d+\.\d+\.\d+)",
        },
        InterpreterProbe {
            runtime_name: "shell",
            binaries: &["bash", "sh"],
            version_flag: "--version",
            version_regex: r"(\d+\.\d+\.\d+)",
        },
    ];

    // For each probe:
    // 1. Run `which <binary>` or check known paths
    // 2. If found, run `<binary> <version_flag>` with a short timeout (2s)
    // 3. Parse version from output using the regex
    // 4. Return DetectedInterpreter with the results
}
```

**Integration with existing code**: The existing `crates/common/src/runtime_detection.rs` already has `normalize_runtime_name()` and alias groups. The auto-detection module should use these for matching detected interpreters against DB runtime records.

#### 2.2 Integration with Worker Registration

The agent startup sequence:

1. Run `detect_interpreters()`
2. Match detected interpreters against known runtimes in the `runtime` table (using alias-aware matching from `runtime_detection.rs`)
3. If `ATTUNE_WORKER_RUNTIMES` is set, use it as an override (intersection or union — TBD, probably override wins)
4. Register the worker with the detected/configured capabilities
5. Log what was detected for debugging:
   ```
   [INFO] Detected runtimes: python 3.12.1 (/usr/local/bin/python3), ruby 3.3.0 (/usr/local/bin/ruby), shell 5.2.21 (/bin/bash)
   [INFO] Registering worker with capabilities: [python, ruby, shell]
   ```

#### 2.3 Runtime Hints File (Optional Enhancement)

Allow a `.attune-runtime.yaml` file in the container that declares runtime capabilities and custom configuration. This handles cases where auto-detection isn't sufficient (e.g., custom interpreters, non-standard paths, special environment setup).

```yaml
# /opt/attune/.attune-runtime.yaml (or /.attune-runtime.yaml)
runtimes:
  - name: ruby
    interpreter: /usr/local/bin/ruby
    file_extension: .rb
    version_command: "ruby --version"
    env_setup:
      create_command: "mkdir -p {env_dir}"
      install_command: "cd {env_dir} && bundle install --gemfile {pack_dir}/Gemfile"
  - name: custom-ml
    interpreter: /opt/conda/bin/python
    file_extension: .py
    version_command: "/opt/conda/bin/python --version"
```

The agent checks for this file at startup and merges it with auto-detected runtimes (hints file takes precedence for conflicting runtime names).

**This is a nice-to-have for Phase 2 — implement only if auto-detection proves insufficient for common use cases.**

---

### Phase 3: Refactor Worker for Code Reuse

**Goal**: Ensure the execution engine is cleanly reusable between the full `attune-worker` and the `attune-agent` binary, without code duplication.

**Effort**: 2–3 days

**Dependencies**: Phase 1 (agent entrypoint exists), can be done in parallel with Phase 2

#### 3.1 Identify Shared vs. Agent-Specific Code

Current worker crate modules and their reuse status:

| Module | File(s) | Shared? | Notes |
|--------|---------|---------|-------|
| `ActionExecutor` | `executor.rs` | ✅ Fully shared | Core execution orchestration |
| `ProcessRuntime` | `runtime/process.rs` | ✅ Fully shared | Subprocess spawning, interpreter resolution |
| `process_executor` | `runtime/process_executor.rs` | ✅ Fully shared | Streaming output capture, timeout, cancellation |
| `NativeRuntime` | `runtime/native.rs` | ✅ Fully shared | Direct binary execution |
| `LocalRuntime` | `runtime/local.rs` | ✅ Fully shared | Fallback runtime facade |
| `RuntimeRegistry` | `runtime/mod.rs` | ✅ Fully shared | Runtime selection and registration |
| `ExecutionContext` | `runtime/mod.rs` | ✅ Fully shared | Execution parameters, env vars, secrets |
| `BoundedLogWriter` | `runtime/log_writer.rs` | ✅ Fully shared | Streaming log capture with size limits |
| `parameter_passing` | `runtime/parameter_passing.rs` | ✅ Fully shared | Stdin/file/env parameter delivery |
| `SecretManager` | `secrets.rs` | ✅ Fully shared | Secret decryption via `attune_common::crypto` |
| `ArtifactManager` | `artifacts.rs` | ✅ Fully shared | Artifact finalization (file stat, size update) |
| `HeartbeatManager` | `heartbeat.rs` | ✅ Fully shared | Periodic DB heartbeat |
| `WorkerRegistration` | `registration.rs` | ✅ Shared, extended | Needs auto-detection integration |
| `env_setup` | `env_setup.rs` | ✅ Shared, lazy mode | Agent uses lazy setup instead of proactive |
| `version_verify` | `version_verify.rs` | ✅ Shared, on-demand mode | Agent verifies on-demand instead of full sweep |
| `WorkerService` | `service.rs` | ⚠️ Needs refactoring | Extract reusable `AgentService` or parameterize |

**Conclusion**: Almost everything is already reusable. The main work is in `service.rs`, which needs to be parameterized for the two startup modes (proactive vs. lazy).

#### 3.2 Refactor `WorkerService` for Dual Modes

Instead of duplicating `WorkerService`, add a configuration enum:

```rust
// In service.rs or a new config module

/// Controls how the worker initializes its runtime environment.
pub enum StartupMode {
    /// Full worker mode: proactive environment setup, full version
    /// verification sweep at startup. Used by `attune-worker`.
    Worker,

    /// Agent mode: lazy environment setup (on first use), on-demand
    /// version verification, auto-detected runtimes. Used by `attune-agent`.
    Agent {
        /// Runtimes detected by the auto-detection module.
        detected_runtimes: Vec<DetectedInterpreter>,
    },
}
```

The `WorkerService::start()` method checks this mode:

```rust
match &self.startup_mode {
    StartupMode::Worker => {
        // Existing behavior: full version verification sweep
        self.verify_all_runtime_versions().await?;
        // Existing behavior: proactive environment setup for all packs
        self.setup_all_environments().await?;
    }
    StartupMode::Agent { .. } => {
        // Skip proactive setup — will happen lazily on first execution
        info!("Agent mode: deferring environment setup to first execution");
    }
}
```

#### 3.3 Lazy Environment Setup

In agent mode, the first execution for a given pack+runtime combination triggers environment setup:

```rust
// In executor.rs, within execute_with_cancel()

// Before executing, ensure the runtime environment exists
if !env_dir.exists() {
    info!("Creating runtime environment on first use: {}", env_dir.display());
    self.env_setup.setup_environment(&pack_ref, &runtime_name, &env_dir).await?;
}
```

The current worker already handles this partially — the `ProcessRuntime::execute()` method has auto-repair logic for broken venvs. The lazy setup extends this to handle the case where the env directory doesn't exist at all.

---

### Phase 4: Docker Compose Integration

**Goal**: Make it trivial to add agent-based workers to `docker-compose.yaml`.

**Effort**: 1 day

**Dependencies**: Phase 1 (agent binary and Dockerfile exist)

#### 4.1 Init Service for Agent Volume

Add to `docker-compose.yaml`:

```yaml
services:
  # Populates the agent binary volume (runs once)
  init-agent:
    build:
      context: .
      dockerfile: docker/Dockerfile.agent
    volumes:
      - agent_bin:/opt/attune/agent
    entrypoint: ["/bin/sh", "-c", "cp /attune-agent /opt/attune/agent/attune-agent && chmod +x /opt/attune/agent/attune-agent"]
    restart: "no"
    networks:
      - attune

volumes:
  agent_bin:  # Named volume holding the static agent binary
```

Note: The init-agent service needs a minimal base with `/bin/sh` for the `cp` command. Since the agent Dockerfile's final stage is `FROM scratch`, the init service should use the builder stage or a separate `FROM alpine` stage.

**Revised Dockerfile.agent approach** — use Alpine for the init image so it has a shell:

```dockerfile
# Stage 1: Build
FROM rust:1.83-bookworm AS builder
# ... (build steps from Phase 1.4)

# Stage 2: Init image (has a shell for cp)
FROM alpine:3.20 AS agent-init
COPY --from=builder /attune-agent /attune-agent
# Default command copies the binary into the mounted volume
CMD ["cp", "/attune-agent", "/opt/attune/agent/attune-agent"]

# Stage 3: Bare binary (for HTTP download or direct use)
FROM scratch AS agent-binary
COPY --from=builder /attune-agent /attune-agent
```

#### 4.2 Agent-Based Worker Services

Example services that can be added to `docker-compose.yaml` or a user's `docker-compose.override.yaml`:

```yaml
  # Ruby worker — uses the official Ruby image
  worker-ruby:
    image: ruby:3.3-slim
    depends_on:
      init-agent:
        condition: service_completed_successfully
      postgres:
        condition: service_healthy
      rabbitmq:
        condition: service_healthy
    entrypoint: ["/opt/attune/agent/attune-agent"]
    volumes:
      - agent_bin:/opt/attune/agent:ro
      - packs_data:/opt/attune/packs:ro
      - runtime_envs:/opt/attune/runtime_envs
      - artifacts_data:/opt/attune/artifacts
      - ${ATTUNE_DOCKER_CONFIG_PATH:-./config.docker.yaml}:/opt/attune/config/config.yaml:ro
    environment:
      ATTUNE_WORKER_NAME: worker-ruby-1
      # ATTUNE_WORKER_RUNTIMES omitted — auto-detected as ruby,shell
    networks:
      - attune
    restart: unless-stopped
    stop_grace_period: 45s

  # R worker — uses the official R base image
  worker-r:
    image: r-base:4.4.0
    depends_on:
      init-agent:
        condition: service_completed_successfully
      postgres:
        condition: service_healthy
      rabbitmq:
        condition: service_healthy
    entrypoint: ["/opt/attune/agent/attune-agent"]
    volumes:
      - agent_bin:/opt/attune/agent:ro
      - packs_data:/opt/attune/packs:ro
      - runtime_envs:/opt/attune/runtime_envs
      - artifacts_data:/opt/attune/artifacts
      - ${ATTUNE_DOCKER_CONFIG_PATH:-./config.docker.yaml}:/opt/attune/config/config.yaml:ro
    environment:
      ATTUNE_WORKER_NAME: worker-r-1
    networks:
      - attune
    restart: unless-stopped

  # GPU worker — NVIDIA CUDA image with Python
  worker-gpu:
    image: nvidia/cuda:12.3.1-runtime-ubuntu22.04
    depends_on:
      init-agent:
        condition: service_completed_successfully
      postgres:
        condition: service_healthy
      rabbitmq:
        condition: service_healthy
    entrypoint: ["/opt/attune/agent/attune-agent"]
    runtime: nvidia
    volumes:
      - agent_bin:/opt/attune/agent:ro
      - packs_data:/opt/attune/packs:ro
      - runtime_envs:/opt/attune/runtime_envs
      - artifacts_data:/opt/attune/artifacts
      - ${ATTUNE_DOCKER_CONFIG_PATH:-./config.docker.yaml}:/opt/attune/config/config.yaml:ro
    environment:
      ATTUNE_WORKER_NAME: worker-gpu-1
      ATTUNE_WORKER_RUNTIMES: python,shell  # Manual override (image has python pre-installed)
    networks:
      - attune
    restart: unless-stopped
```

#### 4.3 User Experience Summary

Adding a new runtime to an Attune deployment becomes a ~12 line addition to `docker-compose.override.yaml`:

```yaml
services:
  worker-my-runtime:
    image: my-org/my-custom-image:latest
    depends_on:
      init-agent:
        condition: service_completed_successfully
      postgres:
        condition: service_healthy
      rabbitmq:
        condition: service_healthy
    entrypoint: ["/opt/attune/agent/attune-agent"]
    volumes:
      - agent_bin:/opt/attune/agent:ro
      - packs_data:/opt/attune/packs:ro
      - runtime_envs:/opt/attune/runtime_envs
      - artifacts_data:/opt/attune/artifacts
      - ${ATTUNE_DOCKER_CONFIG_PATH:-./config.docker.yaml}:/opt/attune/config/config.yaml:ro
    networks:
      - attune
```

No Dockerfiles. No rebuilds. No waiting for Rust compilation. Start to finish in seconds.

---

### Phase 5: API Binary Download Endpoint

**Goal**: Support deployments where shared Docker volumes are impractical (Kubernetes, ECS, remote Docker hosts).

**Effort**: 1 day

**Dependencies**: Phase 1 (agent binary exists)

#### 5.1 New API Route

Add to `crates/api/src/routes/`:

```
GET /api/v1/agent/binary
GET /api/v1/agent/binary?arch=x86_64    (default)
GET /api/v1/agent/binary?arch=aarch64

Response: application/octet-stream
Headers: Content-Disposition: attachment; filename="attune-agent"
```

The API serves the binary from a configurable filesystem path (e.g., `/opt/attune/agent/attune-agent`). The binary can be placed there at build time (baked into the API image) or mounted via volume.

**Configuration** (`config.yaml`):

```yaml
agent:
  binary_dir: /opt/attune/agent   # Directory containing agent binaries
  # Files expected: attune-agent-x86_64, attune-agent-aarch64
```

**OpenAPI documentation** via `utoipa`:

```rust
#[utoipa::path(
    get,
    path = "/api/v1/agent/binary",
    params(("arch" = Option<String>, Query, description = "Target architecture (x86_64, aarch64)")),
    responses(
        (status = 200, description = "Agent binary", content_type = "application/octet-stream"),
        (status = 404, description = "Binary not found for requested architecture"),
    ),
    tag = "agent"
)]
```

**Authentication**: This endpoint should be **unauthenticated** or use a simple shared token, since the agent needs to download the binary before it can authenticate. Alternatively, require basic auth or a bootstrap token passed via environment variable.

#### 5.2 Bootstrap Wrapper Script

Provide `scripts/attune-agent-wrapper.sh` for use as a container entrypoint:

```bash
#!/bin/sh
# attune-agent-wrapper.sh — Bootstrap the Attune agent in any container
set -e

AGENT_DIR="${ATTUNE_AGENT_DIR:-/opt/attune/agent}"
AGENT_BIN="$AGENT_DIR/attune-agent"
AGENT_URL="${ATTUNE_AGENT_URL:-http://attune-api:8080/api/v1/agent/binary}"

# Use volume-mounted binary if available, otherwise download
if [ ! -x "$AGENT_BIN" ]; then
  echo "[attune] Agent binary not found at $AGENT_BIN, downloading from $AGENT_URL..."
  mkdir -p "$AGENT_DIR"
  if command -v wget >/dev/null 2>&1; then
    wget -q -O "$AGENT_BIN" "$AGENT_URL"
  elif command -v curl >/dev/null 2>&1; then
    curl -sL "$AGENT_URL" -o "$AGENT_BIN"
  else
    echo "[attune] ERROR: Neither wget nor curl available. Cannot download agent." >&2
    exit 1
  fi
  chmod +x "$AGENT_BIN"
  echo "[attune] Agent binary downloaded successfully."
fi

echo "[attune] Starting agent..."
exec "$AGENT_BIN" "$@"
```

Usage:

```yaml
# In docker-compose or K8s — when volume mount isn't available
worker-remote:
  image: python:3.12-slim
  entrypoint: ["/opt/attune/scripts/attune-agent-wrapper.sh"]
  volumes:
    - ./scripts/attune-agent-wrapper.sh:/opt/attune/scripts/attune-agent-wrapper.sh:ro
  environment:
    ATTUNE_AGENT_URL: http://attune-api:8080/api/v1/agent/binary
```

---

### Phase 6: Database & Runtime Registry Extensions

**Goal**: Support arbitrary runtimes without requiring every possible runtime to be pre-registered in the DB.

**Effort**: 1–2 days

**Dependencies**: Phase 2 (auto-detection working)

#### 6.1 Extended Runtime Detection Metadata

Add a migration to support auto-detected runtimes:

```sql
-- Migration: NNNNNN_agent_runtime_detection.sql

-- Track whether a runtime was auto-registered by an agent
ALTER TABLE runtime ADD COLUMN IF NOT EXISTS auto_detected BOOLEAN NOT NULL DEFAULT FALSE;

-- Store detection configuration for auto-discovered runtimes
-- Example: { "binaries": ["ruby", "ruby3.2"], "version_command": "--version",
--            "version_regex": "ruby (\\d+\\.\\d+\\.\\d+)" }
ALTER TABLE runtime ADD COLUMN IF NOT EXISTS detection_config JSONB NOT NULL DEFAULT '{}';
```

#### 6.2 Runtime Template Packs

Ship pre-configured runtime definitions for common languages in the `core` pack (or a new `runtimes` pack). These are registered during pack loading and provide the `execution_config` that auto-detected interpreters need.

Add runtime YAML files for new languages:

```
packs/core/runtimes/ruby.yaml
packs/core/runtimes/go.yaml
packs/core/runtimes/java.yaml
packs/core/runtimes/perl.yaml
packs/core/runtimes/r.yaml
```

Example `ruby.yaml`:

```yaml
ref: core.ruby
name: Ruby
label: Ruby Runtime
description: Execute Ruby scripts
execution_config:
  interpreter:
    binary: ruby
    file_extension: .rb
  env_vars:
    GEM_HOME: "{env_dir}/gems"
    GEM_PATH: "{env_dir}/gems"
    BUNDLE_PATH: "{env_dir}/gems"
  environment:
    create_command: "mkdir -p {env_dir}/gems"
    install_command: "cd {pack_dir} && GEM_HOME={env_dir}/gems bundle install --quiet 2>/dev/null || true"
    dependency_file: Gemfile
```

#### 6.3 Dynamic Runtime Registration

When the agent detects an interpreter that matches a runtime template (by name/alias) but the runtime doesn't exist in the DB yet, the agent can auto-register it:

1. Look up the runtime by name in the DB using alias-aware matching
2. If found → use it (existing behavior)
3. If not found → check if a runtime template exists in loaded packs
4. If template found → register the runtime using the template's `execution_config`
5. If no template → register a minimal runtime with just the detected interpreter binary path
6. Mark auto-registered runtimes with `auto_detected = true`

This ensures the agent can work with new runtimes immediately, even if the runtime hasn't been explicitly configured.

---

### Phase 7: Kubernetes Support ✅

**Status**: Complete

**Goal**: Provide Kubernetes manifests and Helm chart support for agent-based workers.

**Effort**: 1–2 days

**Dependencies**: Phase 4 (Docker Compose working), Phase 5 (binary download)

**Implemented**:
- Helm chart `agent-workers.yaml` template — creates a Deployment per `agentWorkers[]` entry
- InitContainer pattern (`agent-loader`) copies the statically-linked binary via `emptyDir` volume
- Full scheduling support: `nodeSelector`, `tolerations`, `runtimeClassName` (GPU/nvidia)
- Runtime auto-detect by default; explicit `runtimes` list override
- Custom env vars, resource limits, log level, termination grace period
- `images.agent` added to `values.yaml` for registry-aware image resolution
- `attune-agent` image added to the Gitea Actions publish workflow (`agent-init` target)
- `NOTES.txt` updated to list enabled agent workers on install
- Quick-reference docs at `docs/QUICKREF-kubernetes-agent-workers.md`

#### 7.1 InitContainer Pattern

The agent maps naturally to Kubernetes using the same Tekton/Argo pattern:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: attune-worker-ruby
spec:
  replicas: 2
  selector:
    matchLabels:
      app: attune-worker-ruby
  template:
    metadata:
      labels:
        app: attune-worker-ruby
    spec:
      initContainers:
        - name: agent-loader
          image: attune/agent:latest    # Built from Dockerfile.agent, agent-init target
          command: ["cp", "/attune-agent", "/opt/attune/agent/attune-agent"]
          volumeMounts:
            - name: agent-bin
              mountPath: /opt/attune/agent
      containers:
        - name: worker
          image: ruby:3.3
          command: ["/opt/attune/agent/attune-agent"]
          env:
            - name: ATTUNE__DATABASE__URL
              valueFrom:
                secretKeyRef:
                  name: attune-secrets
                  key: database-url
            - name: ATTUNE__MESSAGE_QUEUE__URL
              valueFrom:
                secretKeyRef:
                  name: attune-secrets
                  key: mq-url
          volumeMounts:
            - name: agent-bin
              mountPath: /opt/attune/agent
              readOnly: true
            - name: packs
              mountPath: /opt/attune/packs
              readOnly: true
            - name: runtime-envs
              mountPath: /opt/attune/runtime_envs
            - name: artifacts
              mountPath: /opt/attune/artifacts
      volumes:
        - name: agent-bin
          emptyDir: {}
        - name: packs
          persistentVolumeClaim:
            claimName: attune-packs
        - name: runtime-envs
          persistentVolumeClaim:
            claimName: attune-runtime-envs
        - name: artifacts
          persistentVolumeClaim:
            claimName: attune-artifacts
```

#### 7.2 Helm Chart Values

```yaml
# values.yaml (future Helm chart)
workers:
  - name: ruby
    image: ruby:3.3
    replicas: 2
    runtimes: []  # auto-detect
  - name: python-gpu
    image: nvidia/cuda:12.3.1-runtime-ubuntu22.04
    replicas: 1
    runtimes: [python, shell]
    resources:
      limits:
        nvidia.com/gpu: 1
```

---

## Implementation Order & Effort Summary

| Phase | Description | Effort | Dependencies | Priority |
|-------|------------|--------|-------------|----------|
| **Phase 1** | Static binary build infrastructure | 3–5 days | None | Critical |
| **Phase 3** | Refactor worker for code reuse | 2–3 days | Phase 1 | Critical |
| **Phase 2** | Runtime auto-detection | 1–2 days | Phase 1 | High |
| **Phase 4** | Docker Compose integration | 1 day | Phase 1 | High |
| **Phase 6** | DB runtime registry extensions | 1–2 days | Phase 2 | Medium |
| **Phase 5** | API binary download endpoint | 1 day | Phase 1 | Medium |
| **Phase 7** ✅ | Kubernetes manifests | 1–2 days | Phase 4, 5 | Complete |

**Total estimated effort: 10–16 days**

Phases 2 and 3 can be done in parallel. Phase 4 can start as soon as Phase 1 produces a working binary.

**Minimum viable feature**: Phases 1 + 3 + 4 (~6–9 days) produce a working agent that can be injected into any container via Docker Compose, with manual `ATTUNE_WORKER_RUNTIMES` configuration. Auto-detection (Phase 2) and dynamic registration (Phase 6) add polish.

## Risks & Mitigations

### musl + Crate Compatibility

**Risk**: Some crates may not compile cleanly with `x86_64-unknown-linux-musl` due to C library dependencies.

**Impact**: Build failures or runtime issues.

**Mitigation**:
- SQLx already uses `rustls` (no OpenSSL dependency) ✅
- Switch `reqwest` and `tokio-tungstenite` to `rustls` features (Phase 1.1)
- `lapin` uses pure Rust AMQP — no C dependencies ✅
- Test the musl build early in Phase 1 to surface issues quickly
- If a specific crate is problematic, evaluate alternatives or use `cross` for cross-compilation

### DNS Resolution with musl

**Risk**: musl's DNS resolver behaves differently from glibc (no `/etc/nsswitch.conf`, limited mDNS support). This can cause DNS resolution failures in Docker networks.

**Impact**: Agent can't resolve `postgres`, `rabbitmq`, etc. by Docker service name.

**Mitigation**:
- Use `trust-dns` (now `hickory-dns`) resolver feature in SQLx and reqwest instead of the system resolver
- Test DNS resolution in Docker Compose early
- If issues arise, document the workaround: use IP addresses or add `dns` configuration to the container

### Binary Size

**Risk**: A full statically-linked binary with all worker deps could be 40MB+.

**Impact**: Slow volume population, slow download via API.

**Mitigation**:
- Strip debug symbols (`strip` command) — typically reduces by 50–70%
- Use `opt-level = 'z'` and `lto = true` in release profile
- Consider `upx` compression (trades CPU at startup for smaller binary)
- Feature-gate unused functionality if size is excessive
- Target: <25MB stripped

### Non-root User Conflicts

**Risk**: Different base images run as different UIDs. The agent needs write access to `runtime_envs` and `artifacts` volumes.

**Impact**: Permission denied errors when the container UID doesn't match the volume owner.

**Mitigation**:
- Document the UID requirement (current standard: UID 1000)
- Provide guidance for running the agent as root with privilege drop
- Consider adding a `--user` flag to the agent that drops privileges after setup
- For Kubernetes, use `securityContext.runAsUser` in the Pod spec

### Existing Workers Must Keep Working

**Risk**: Refactoring `WorkerService` (Phase 3) could introduce regressions in existing workers.

**Impact**: Production workers break.

**Mitigation**:
- The refactoring is additive — existing code paths don't change behavior
- Run the full test suite after Phase 3
- Both `attune-worker` and `attune-agent` share the same test infrastructure
- The `StartupMode::Worker` path is the existing code path with no behavioral changes

### Volume Mount Ordering

**Risk**: The agent container starts before the `init-agent` service has populated the volume.

**Impact**: Agent binary not found, container crashes.

**Mitigation**:
- Use `depends_on: { init-agent: { condition: service_completed_successfully } }` in Docker Compose
- The wrapper script (Phase 5.2) retries with a short sleep
- For Kubernetes, the initContainer pattern guarantees ordering

## Testing Strategy

### Unit Tests

- Auto-detection module: mock filesystem and process execution to test interpreter discovery
- `StartupMode::Agent` code paths: ensure lazy setup and on-demand verification work correctly
- All existing worker tests continue to pass (regression safety net)

### Integration Tests

- Build the agent binary with musl and run it in various container images:
  - `ruby:3.3-slim` (Ruby + shell)
  - `python:3.12-slim` (Python + shell)
  - `node:20-slim` (Node.js + shell)
  - `alpine:3.20` (shell only)
  - `ubuntu:24.04` (shell only)
  - `debian:bookworm-slim` (shell only, matches current worker)
- Verify: agent starts, auto-detects runtimes, registers with correct capabilities, executes a simple action, reports results
- Verify: DNS resolution works for Docker service names

### Docker Compose Tests

- Spin up the full stack with agent-based workers alongside traditional workers
- Execute actions that target specific runtimes
- Verify the scheduler routes to the correct worker based on capabilities
- Verify graceful shutdown (SIGTERM handling)

### Binary Compatibility Tests

- Test the musl binary on: Alpine, Debian, Ubuntu, CentOS/Rocky, Amazon Linux
- Test on both x86_64 and aarch64 (if multi-arch build is implemented)
- Verify no glibc dependency: `ldd attune-agent` should report "not a dynamic executable"

## Future Enhancements

These are not part of the initial implementation but are natural extensions:

1. **Per-execution container isolation**: Instead of a long-running agent, spawn a fresh container per execution with the agent injected. Provides maximum isolation (each action runs in a clean environment) at the cost of startup latency.

2. **Container image selection in action YAML**: Allow actions to declare `container: ruby:3.3` in their YAML, and have the executor spin up an appropriate container with the agent injected. Similar to GitHub Actions' container actions.

3. **Warm pool**: Pre-start a pool of agent containers for common runtimes to reduce first-execution latency.

4. **Agent self-update**: The agent periodically checks for a newer version of itself (via the API endpoint) and restarts if updated.

5. **Windows support**: Cross-compile the agent for Windows (MSVC static linking) to support Windows containers.

6. **WebAssembly runtime**: Compile actions to WASM and execute them inside the agent using wasmtime, eliminating the need for interpreter binaries entirely.

## References

- Tekton Entrypoint: https://github.com/tektoncd/pipeline/tree/main/cmd/entrypoint
- Argo Emissary Executor: https://argoproj.github.io/argo-workflows/workflow-executors/
- GitLab Runner Docker Executor: https://docs.gitlab.com/runner/executors/docker.html
- Current worker containerization: `docs/worker-containerization.md`
- Current runtime detection: `crates/common/src/runtime_detection.rs`
- Worker service: `crates/worker/src/service.rs`
- Process executor: `crates/worker/src/runtime/process_executor.rs`
- Worker Dockerfile: `docker/Dockerfile.worker.optimized`
