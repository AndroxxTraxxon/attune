# Runtime Version Worker Pipeline Integration

**Date**: 2026-02-26

## Summary

Integrated the runtime version system into the worker execution pipeline, enabling version-aware action execution. When an action declares a `runtime_version_constraint` (e.g., `">=3.12"`), the worker now automatically selects the best matching runtime version and uses its version-specific interpreter, environment commands, and configuration.

## Changes Made

### 1. Version-Aware Execution Context (`crates/worker/src/runtime/mod.rs`)

Added three new fields to `ExecutionContext`:
- `runtime_config_override: Option<RuntimeExecutionConfig>` — version-specific execution config that overrides the parent runtime's config
- `runtime_env_dir_suffix: Option<String>` — directory suffix for per-version environment isolation (e.g., `"python-3.12"`)
- `selected_runtime_version: Option<String>` — selected version string for logging/diagnostics

### 2. Runtime Version Resolution in Executor (`crates/worker/src/executor.rs`)

Added `resolve_runtime_version()` method to `ActionExecutor` that:
- Queries `runtime_version` rows for the action's runtime from the database
- Calls `select_best_version()` with the action's `runtime_version_constraint`
- Returns the selected version's `execution_config`, env dir suffix, and version string
- Gracefully handles missing versions, failed queries, and unmatched constraints with warnings

The method is called during `prepare_execution_context()` and the results are passed through the `ExecutionContext` to the `ProcessRuntime`.

### 3. Version-Aware ProcessRuntime (`crates/worker/src/runtime/process.rs`)

Modified `ProcessRuntime::execute()` to:
- Use `effective_config` (either `context.runtime_config_override` or `self.config`)
- Compute version-specific environment directories when `runtime_env_dir_suffix` is present
- Use version-specific interpreter resolution, environment variables, and interpreter args
- Recreate broken environments using the effective (possibly version-specific) config
- Log the selected version in all execution diagnostics

### 4. Runtime Version Verification (`crates/worker/src/version_verify.rs`)

New module that verifies which runtime versions are available on the system at worker startup:
- Extracts verification commands from `distributions.verification.commands` JSONB
- Runs each command with proper timeout handling (10s per command)
- Matches exit codes and output patterns (regex-based)
- Updates `available` and `verified_at` columns in the database
- Respects `ATTUNE_WORKER_RUNTIMES` filter (alias-aware)
- Falls back to `binary --version` check when no explicit verification commands exist
- Includes 8 unit tests for command extraction and verification logic

### 5. Version-Aware Environment Setup (`crates/worker/src/env_setup.rs`)

Extended the proactive environment setup system to create per-version environments:
- At startup scan: creates environments at `{runtime_envs_dir}/{pack_ref}/{runtime_name}-{version}` for each available version
- On `pack.registered` MQ events: creates per-version environments alongside base environments
- Each version environment uses the version's own `execution_config` (different interpreter binary, venv create command, etc.)
- Base (unversioned) environments are still created for backward compatibility

### 6. Worker Startup Sequence (`crates/worker/src/service.rs`)

Updated the startup sequence to include version verification before environment setup:
1. Connect to DB and MQ
2. Load runtimes → create ProcessRuntime instances
3. Register worker and set up MQ infrastructure
4. **NEW: Verify runtime versions** (run verification commands, update `available` flags)
5. Set up runtime environments (now version-aware)
6. Start heartbeat, execution consumer, pack registration consumer

### 7. Test Updates

Updated all `ExecutionContext` initializations across the workspace to include the new fields:
- `crates/worker/src/runtime/process.rs` — 8 test contexts
- `crates/worker/src/runtime/local.rs` — 2 test contexts
- `crates/worker/src/runtime/python.rs` — 4 test contexts
- `crates/worker/src/runtime/shell.rs` — 8 test contexts
- `crates/worker/tests/dependency_isolation_test.rs` — 1 test context
- `crates/worker/tests/log_truncation_test.rs` — 3 test contexts
- `crates/worker/tests/security_tests.rs` — 8 test contexts

Also fixed pre-existing missing `env_vars` fields in `RuntimeExecutionConfig` initializations in test helper functions.

## Architecture

```
Action with runtime_version_constraint=">=3.12"
    │
    ▼
ActionExecutor::prepare_execution_context()
    │
    ├── Load runtime model (e.g., "Python")
    ├── Query RuntimeVersion rows for that runtime
    ├── select_best_version(versions, ">=3.12")
    │   └── Returns Python 3.13 (highest available matching >=3.12)
    ├── Set runtime_config_override = version's execution_config
    ├── Set runtime_env_dir_suffix = "python-3.13"
    └── Set selected_runtime_version = "3.13"
    │
    ▼
ProcessRuntime::execute(context)
    │
    ├── effective_config = context.runtime_config_override (python3.13 config)
    ├── env_dir = /opt/attune/runtime_envs/{pack}/python-3.13
    ├── interpreter = python3.13 (from version-specific config)
    └── Execute action with version-specific setup
```

## Environment Directory Layout

```
/opt/attune/runtime_envs/
└── my_pack/
    ├── python/          # Base (unversioned) environment
    ├── python-3.11/     # Python 3.11 specific environment
    ├── python-3.12/     # Python 3.12 specific environment
    ├── python-3.13/     # Python 3.13 specific environment
    ├── node/            # Base Node.js environment
    ├── node-18/         # Node.js 18 specific environment
    ├── node-20/         # Node.js 20 specific environment
    └── node-22/         # Node.js 22 specific environment
```

## Dependencies Added

- `regex` (workspace dependency, already in `Cargo.toml`) added to `attune-worker` for verification pattern matching

## Test Results

- All 93 worker unit tests pass
- All 17 dependency isolation tests pass
- All 8 log truncation tests pass
- All 7 security tests pass
- All 33 version matching tests + 2 doc tests pass
- Zero compiler warnings across the workspace

## What's Next

- API/UI endpoints for runtime version management (list, verify, toggle availability)
- Sensor service integration with version-aware runtime selection
- Runtime version auto-detection at pack install time (not just worker startup)