# Universal Worker Agent Phase 2: Runtime Detection ↔ Worker Registration Integration

**Date**: 2026-02-05

## Summary

Integrated the Phase 1 runtime auto-detection module with the worker registration system so that `attune-agent` workers register with rich interpreter metadata (binary paths, versions) in their capabilities, enabling the system to distinguish agents from standard workers and know exactly which interpreters are available and where.

## Changes

### 1. `crates/worker/src/runtime_detect.rs`
- Added `Serialize` and `Deserialize` derives to `DetectedRuntime` so instances can be stored as structured JSON in worker capabilities.

### 2. `crates/worker/src/registration.rs`
- Added `use crate::runtime_detect::DetectedRuntime` import.
- Added `set_detected_runtimes(&mut self, runtimes: Vec<DetectedRuntime>)` method that stores detected interpreter metadata under the `detected_interpreters` capability key as a JSON array of `{name, path, version}` objects.
- Added `set_agent_mode(&mut self, is_agent: bool)` method that sets an `agent_mode` boolean capability to distinguish agent workers from standard workers.
- Both methods are additive — the existing `runtimes` string list capability remains for backward compatibility.

### 3. `crates/worker/src/service.rs`
- Added `detected_runtimes: Option<Vec<DetectedRuntime>>` field to `WorkerService` (initialized to `None` in `new()`).
- Added `pub fn with_detected_runtimes(mut self, runtimes: Vec<DetectedRuntime>) -> Self` builder method that stores agent detection results for use during `start()`.
- Updated `start()` to call `registration.set_detected_runtimes()` and `registration.set_agent_mode(true)` before `register()` when detected runtimes are present.
- Standard `attune-worker` binary is completely unaffected — the field stays `None` and no agent-specific code runs.

### 4. `crates/worker/src/agent_main.rs`
- Added `agent_detected_runtimes: Option<Vec<DetectedRuntime>>` variable to stash detection results.
- After auto-detection runs and sets `ATTUNE_WORKER_RUNTIMES`, the detected `Vec` is saved into `agent_detected_runtimes`.
- After `WorkerService::new()`, calls `.with_detected_runtimes(detected)` if auto-detection ran, so the registration includes full interpreter metadata.

### 5. `crates/worker/src/lib.rs`
- Added `pub use runtime_detect::DetectedRuntime` re-export for convenient access.

## Capability Format

After Phase 2, an agent worker's `capabilities` JSON in the `worker` table looks like:

```json
{
  "runtimes": ["shell", "python", "node"],
  "max_concurrent_executions": 10,
  "worker_version": "0.1.0",
  "agent_mode": true,
  "detected_interpreters": [
    {"name": "shell", "path": "/bin/bash", "version": "5.2.15"},
    {"name": "python", "path": "/usr/bin/python3", "version": "3.12.1"},
    {"name": "node", "path": "/usr/bin/node", "version": "20.11.0"}
  ]
}
```

Standard `attune-worker` instances do NOT have `agent_mode` or `detected_interpreters` keys.

## Design Decisions

- **Builder pattern** (`with_detected_runtimes`) rather than a separate constructor — keeps the API surface minimal and avoids duplicating `new()` logic.
- **Explicit `set_agent_mode`** separate from `set_detected_runtimes` — allows independent control, though in practice they're always called together for agents.
- **JSON serialization via `serde_json::json!()` macro** rather than `serde_json::to_value(&runtimes)` — gives explicit control over the capability shape and avoids coupling the DB format to the Rust struct layout.
- **No changes to the `Worker` model or database schema** — `detected_interpreters` and `agent_mode` are stored inside the existing `capabilities` JSONB column.

## Verification

- `cargo check --workspace` — zero errors, zero warnings
- `cargo test -p attune-worker` — 139 tests pass (105 unit + 17 dependency isolation + 8 log truncation + 7 security + 2 doc-tests)
- Standard `attune-worker` binary path is completely unchanged