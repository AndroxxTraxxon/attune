# Universal Worker Agent — Phase 2: Runtime Auto-Detection Integration

**Date**: 2026-02-05

## Overview

Implemented Phase 2 of the Universal Worker Agent plan (`docs/plans/universal-worker-agent.md`), which integrates the runtime auto-detection module (built in Phase 1) with the worker registration system. Agent workers now register with rich interpreter metadata — binary paths and versions — alongside the simple runtime name list used for backward compatibility.

## Changes

### 1. `crates/worker/src/runtime_detect.rs`

- Added `Serialize` and `Deserialize` derives to `DetectedRuntime` so detection results can be stored as JSON in worker capabilities.

### 2. `crates/worker/src/registration.rs`

- Added `use crate::runtime_detect::DetectedRuntime` import.
- **`set_detected_runtimes(runtimes: Vec<DetectedRuntime>)`** — Stores interpreter metadata under the `detected_interpreters` capability key as a structured JSON array (each entry has `name`, `path`, `version`). This supplements the existing `runtimes` string list for backward compatibility.
- **`set_agent_mode(is_agent: bool)`** — Sets an `agent_mode` boolean capability so the system can distinguish agent-mode workers from standard workers.

### 3. `crates/worker/src/service.rs`

- Added `detected_runtimes: Option<Vec<DetectedRuntime>>` field to `WorkerService` (defaults to `None`).
- **`with_detected_runtimes(self, runtimes) -> Self`** — Builder method to pass agent-detected runtimes into the service. No-op for standard `attune-worker`.
- Updated `start()` to call `set_detected_runtimes()` + `set_agent_mode(true)` on the registration before `register()` when detected runtimes are present.

### 4. `crates/worker/src/agent_main.rs`

- Added `agent_detected_runtimes: Option<Vec<DetectedRuntime>>` variable to stash detection results.
- After auto-detection runs, the detected runtimes are saved (previously they were consumed by the env var setup and discarded).
- After `WorkerService::new()`, chains `.with_detected_runtimes(detected)` to pass the metadata through.

### 5. `crates/worker/src/lib.rs`

- Re-exported `DetectedRuntime` from `runtime_detect` module for external use.

## Worker Capabilities (Agent Mode)

When an `attune-agent` registers, its capabilities JSON now includes:

```json
{
  "runtimes": ["shell", "python", "node"],
  "detected_interpreters": [
    {"name": "shell", "path": "/bin/bash", "version": "5.2.15"},
    {"name": "python", "path": "/usr/bin/python3", "version": "3.12.1"},
    {"name": "node", "path": "/usr/bin/node", "version": "20.11.0"}
  ],
  "agent_mode": true,
  "max_concurrent_executions": 10,
  "worker_version": "0.1.0"
}
```

Standard `attune-worker` registrations are unchanged — no `detected_interpreters` or `agent_mode` keys.

## Phase 2 Sub-tasks

| Sub-task | Status | Notes |
|----------|--------|-------|
| 2.1 Interpreter Discovery Module | ✅ Done (Phase 1) | `runtime_detect.rs` already existed |
| 2.2 Integration with Worker Registration | ✅ Done | Rich capabilities + agent_mode flag |
| 2.3 Runtime Hints File | ⏭️ Deferred | Optional enhancement, not needed yet |

## Verification

- `cargo check --workspace` — zero errors, zero warnings
- `cargo test -p attune-worker` — all tests pass (unit, integration, doc-tests)
- No breaking changes to the standard `attune-worker` binary