# Sensor Agent Injection Plan

## Overview

The sensor service is positioned similarly to the worker service: it is a long-running process that dispatches sensor commands into underlying runtimes rather than containing runtime-specific logic in the service binary itself. The worker side now supports injected, statically-linked agent binaries that run inside arbitrary container images. This plan extends the same model to sensors.

Goal:

- Replace the pre-built `attune-sensor` container image in default deployments with an injected sensor agent binary running inside stock runtime images
- Reuse the existing runtime auto-detection and capability reporting model
- Preserve current sensor behavior, including runtime-based execution, registration, heartbeat, and graceful shutdown

Non-goals:

- Converging worker and sensor into a single binary
- Redesigning sensor scheduling or runtime execution semantics
- Removing existing `ATTUNE_SENSOR_RUNTIMES` overrides

## Current State

Relevant implementation points:

- Sensor startup entrypoint: [crates/sensor/src/main.rs](/home/david/Codebase/attune/crates/sensor/src/main.rs)
- Sensor service orchestration: [crates/sensor/src/service.rs](/home/david/Codebase/attune/crates/sensor/src/service.rs)
- Sensor capability registration: [crates/sensor/src/sensor_worker_registration.rs](/home/david/Codebase/attune/crates/sensor/src/sensor_worker_registration.rs)
- Shared runtime detection: [crates/common/src/runtime_detection.rs](/home/david/Codebase/attune/crates/common/src/runtime_detection.rs)
- Current sensor container build: [docker/Dockerfile.sensor.optimized](/home/david/Codebase/attune/docker/Dockerfile.sensor.optimized)
- Existing worker-agent design reference: [docs/plans/universal-worker-agent.md](/home/david/Codebase/attune/docs/plans/universal-worker-agent.md)

Observations:

- Sensors already use the same three-tier capability detection model as workers:
  - `ATTUNE_SENSOR_RUNTIMES`
  - config file capabilities
  - database-driven verification
- The main missing piece is packaging and entrypoint behavior, not capability modeling
- The current sensor Compose service still depends on a pre-built Rust binary baked into the container image
- The sensor manager relies on shared runtime environment assumptions such as interpreter paths and `runtime_envs` compatibility

## Proposed Architecture

Introduce a dedicated injected binary, `attune-sensor-agent`, analogous to the existing `attune-agent` for workers.

Responsibilities of `attune-sensor-agent`:

- Probe the container for available interpreters before the Tokio runtime starts
- Respect `ATTUNE_SENSOR_RUNTIMES` as a hard override
- Populate `ATTUNE_SENSOR_RUNTIMES` automatically when unset
- Support `--detect-only` for diagnostics
- Load config and start `SensorService`

This should remain a separate binary from `attune-agent`.

Reasoning:

- `attune-agent` is worker-specific today and boots `WorkerService`
- Sensor startup and runtime semantics are related but not identical
- A shared bootstrap library is useful; a single polymorphic agent binary is not necessary

## Implementation Phases

### Phase 1: Add Sensor Agent Binary

Add a new binary target under the sensor crate, likely:

- `name = "attune-sensor-agent"`
- `path = "src/agent_main.rs"`

The new binary should mirror the startup shape of [crates/worker/src/agent_main.rs](/home/david/Codebase/attune/crates/worker/src/agent_main.rs), but target sensors instead of workers.

Expected behavior:

1. Install the crypto provider
2. Initialize tracing
3. Parse CLI flags:
   - `--config`
   - `--name`
   - `--detect-only`
4. Detect runtimes synchronously before Tokio starts
5. Set `ATTUNE_SENSOR_RUNTIMES` when auto-detection is used
6. Load config
7. Apply sensor name override if provided
8. Start `SensorService`
9. Handle SIGINT/SIGTERM and call `stop()`

### Phase 2: Reuse and Extract Shared Bootstrap Logic

Avoid duplicating the worker-agent detection/bootstrap code blindly.

Extract shared pieces into a reusable location, likely one of:

- `attune-common`
- a small shared helper module in `crates/common`
- a narrow internal library module used by both worker and sensor crates

Candidate shared logic:

- pre-Tokio runtime detection flow
- override handling
- `--detect-only` reporting
- environment mutation rules

Keep service-specific startup separate:

- worker agent starts `WorkerService`
- sensor agent starts `SensorService`

### Phase 3: Docker Build Support for Injected Sensor Agent

Extend the current agent binary build pipeline so the statically-linked sensor agent can be published into the same shared volume model used for workers.

Options:

- Extend [docker/Dockerfile.agent](/home/david/Codebase/attune/docker/Dockerfile.agent) to build and copy both `attune-agent` and `attune-sensor-agent`
- Or add a sibling Dockerfile if the combined build becomes unclear

Preferred outcome:

- `init-agent` populates `/opt/attune/agent/attune-agent`
- `init-agent` also populates `/opt/attune/agent/attune-sensor-agent`

Constraints:

- Keep the binaries statically linked
- Preserve the existing API binary-serving flow from the `agent_bin` volume
- Do not break current worker agent consumers

### Phase 4: Compose Integration for Sensor Agent Injection

Replace the current `sensor` service in [docker-compose.yaml](/home/david/Codebase/attune/docker-compose.yaml) with an agent-injected service.

Target shape:

- stock runtime image instead of `docker/Dockerfile.sensor.optimized`
- `entrypoint: ["/opt/attune/agent/attune-sensor-agent"]`
- `depends_on.init-agent`
- same config, packs, runtime env, and log/artifact mounts as required

Required environment variables must be preserved, especially:

- `ATTUNE_CONFIG`
- `ATTUNE__DATABASE__URL`
- `ATTUNE__MESSAGE_QUEUE__URL`
- `ATTUNE_API_URL`
- `ATTUNE_MQ_URL`
- `ATTUNE_PACKS_BASE_DIR`

Recommended default image strategy:

- Use a stock image that includes the default runtimes the sensor service should expose
- Be conservative about path compatibility with worker-created runtime environments

### Phase 5: Native Capability Handling

Sensors have the same edge case as workers: `native` is a capability but not a discoverable interpreter.

Implication:

- Pure auto-detection can discover Python, Node, Shell, Ruby, etc.
- It cannot infer `native` safely from interpreter probing alone

Plan:

- Keep explicit `ATTUNE_SENSOR_RUNTIMES=...,native` for any default full-capability sensor image
- Revisit later only if native becomes a first-class explicit capability outside interpreter discovery

### Phase 6: Runtime Environment Compatibility

The current sensor image documents an important invariant: sensors and workers share `runtime_envs`, so interpreter paths must remain compatible.

This must remain true after the migration.

Validation criteria:

- Python virtual environments created by workers remain usable by sensors
- Node runtime assumptions remain compatible across images
- No new symlink breakage due to mismatched interpreter installation paths

If necessary, prefer stock images whose paths align with the worker fleet, or explicitly document where sensor and worker images are allowed to diverge.

### Phase 7: Documentation and Examples

After implementation:

- Update [docs/plans/universal-worker-agent.md](/home/david/Codebase/attune/docs/plans/universal-worker-agent.md) with a sensor extension or cross-reference
- Update [docker-compose.yaml](/home/david/Codebase/attune/docker-compose.yaml)
- Update [docker-compose.agent.yaml](/home/david/Codebase/attune/docker-compose.agent.yaml) if it should also include sensor examples
- Add or update quick references for sensor agent injection

The message should be clear:

- Workers and sensors both support injected static agent binaries
- Runtime images are now decoupled from Rust service image builds

## Recommended Implementation Order

1. Add `attune-sensor-agent` binary and make it boot `SensorService`
2. Extract shared bootstrap logic from the worker-agent path
3. Extend the agent Docker build/init path to include the sensor agent binary
4. Replace the Compose `sensor` service with an injected sensor-agent container
5. Validate runtime detection and one end-to-end Python, Node, and native sensor path
6. Update docs and examples

## Risks

### Worker-Agent Coupling

Risk:

- Trying to reuse `attune-agent` directly for sensors will conflate worker and sensor startup semantics

Mitigation:

- Keep separate binaries with shared helper code only where it is truly generic

### Native Capability Loss

Risk:

- Auto-detection does not capture `native`

Mitigation:

- Preserve explicit `ATTUNE_SENSOR_RUNTIMES` where native support is required

### Runtime Path Mismatch

Risk:

- Switching to a stock image may reintroduce broken venv or interpreter path issues

Mitigation:

- Validate image interpreter paths against shared `runtime_envs`
- Prefer images that align with worker path conventions when possible

### Missing Environment Contract

Risk:

- The sensor manager currently depends on env vars such as `ATTUNE_API_URL`, `ATTUNE_MQ_URL`, and `ATTUNE_PACKS_BASE_DIR`

Mitigation:

- Preserve these in the injected sensor container definition
- Avoid relying solely on config fields unless the code is updated accordingly

## Validation Checklist

- `attune-sensor-agent --detect-only` reports detected runtimes correctly
- `ATTUNE_SENSOR_RUNTIMES` override still takes precedence
- Sensor registration records expected runtime capabilities in the `worker` table
- Sensor heartbeat and deregistration still work
- Python-based sensors execute successfully
- Node-based sensors execute successfully
- Native sensors execute successfully when `native` is explicitly enabled
- Shared `runtime_envs` remain usable between workers and sensors
- `docker compose config` validates cleanly after Compose changes

## Deliverables

- New `attune-sensor-agent` binary target
- Shared bootstrap/runtime-detection helpers as needed
- Updated agent build/init pipeline producing a sensor agent binary
- Updated Compose deployment using injected sensor agent containers
- Documentation updates covering the sensor agent model
