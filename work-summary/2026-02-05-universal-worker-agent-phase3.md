# Universal Worker Agent — Phase 3: WorkerService Dual-Mode Refactor

**Date**: 2026-02-05

## Overview

Implemented Phase 3 of the Universal Worker Agent plan (`docs/plans/universal-worker-agent.md`): refactoring `WorkerService` for clean code reuse between the full `attune-worker` and the `attune-agent` binary, without code duplication.

## Changes

### 1. `StartupMode` Enum (`crates/worker/src/service.rs`)

Added a `StartupMode` enum that controls how the worker initializes its runtime environment:

- **`Worker`** — Full worker mode with proactive environment setup and full version verification sweep at startup. This is the existing behavior used by `attune-worker`.
- **`Agent { detected_runtimes }`** — Agent mode with lazy (on-demand) environment setup and deferred version verification. Used by `attune-agent`. Carries the auto-detected runtimes from Phase 2.

### 2. `WorkerService` Struct Refactoring (`crates/worker/src/service.rs`)

- Replaced the `detected_runtimes: Option<Vec<DetectedRuntime>>` field with `startup_mode: StartupMode`
- `new()` defaults to `StartupMode::Worker`
- `with_detected_runtimes()` now sets `StartupMode::Agent { detected_runtimes }` — the method signature is unchanged, so `agent_main.rs` requires no modifications

### 3. Conditional Startup in `start()` (`crates/worker/src/service.rs`)

The `start()` method now branches on `self.startup_mode`:

- **Worker mode**: Runs `verify_runtime_versions()` and `scan_and_setup_environments()` proactively (existing behavior, unchanged)
- **Agent mode**: Skips both with an info log — environments will be created lazily on first execution

Agent capability registration (`set_detected_runtimes()`, `set_agent_mode()`) also uses the `StartupMode` match instead of the old `Option` check.

### 4. Lazy Environment Setup (`crates/worker/src/runtime/process.rs`)

Updated `ProcessRuntime::execute()` to perform on-demand environment creation when the env directory is missing. Previously, a missing env dir produced a warning and fell back to the system interpreter. Now it:

1. Logs an info message about lazy setup
2. Creates a temporary `ProcessRuntime` with the effective config
3. Calls `setup_pack_environment()` to create the environment (venv, node_modules, etc.)
4. Falls back to the system interpreter only if creation fails

This is the primary code path for agent mode (where proactive startup setup is skipped) but also serves as a safety net for standard workers.

### 5. Re-export (`crates/worker/src/lib.rs`)

`StartupMode` is re-exported from the `attune_worker` crate root for external use.

## Files Modified

| File | Change |
|------|--------|
| `crates/worker/src/service.rs` | Added `StartupMode` enum, replaced `detected_runtimes` field, conditional startup logic |
| `crates/worker/src/runtime/process.rs` | Lazy on-demand environment creation in `execute()` |
| `crates/worker/src/lib.rs` | Re-export `StartupMode` |
| `AGENTS.md` | Updated development status (Phase 3 complete, Phases 4–7 in progress) |

## Test Results

All 139 tests pass:
- 105 unit tests
- 17 dependency isolation tests
- 8 log truncation tests
- 7 security tests
- 2 doc-tests

Zero compiler warnings across the workspace.

## Design Decisions

- **No code duplication**: The `StartupMode` enum parameterizes `WorkerService` rather than creating a separate `AgentService`. All execution machinery (runtimes, consumers, heartbeat, cancellation) is shared.
- **Lazy setup as safety net**: The on-demand environment creation in `ProcessRuntime::execute()` benefits both modes — agents rely on it as the primary path, while standard workers get it as a fallback if proactive setup missed something.
- **Backward compatible API**: `with_detected_runtimes()` keeps its signature, so `agent_main.rs` needed no changes.