# Runtime Type Removal, YAML Loading & Ref Format Cleanup

**Date:** 2026-02-13

## Problem

Running a Python action failed with:
```
Runtime not found: No runtime found for action: python_example.hello (available: shell)
```

The worker only had the Shell runtime registered. Investigation revealed four interrelated bugs:

1. **Runtime YAML files were never loaded into the database.** `PackComponentLoader::load_all()` loaded triggers, actions, and sensors but completely ignored the `runtimes/` directory. Files like `packs/core/runtimes/python.yaml` were dead weight.

2. **`load_core_pack.py` only created Shell + Sensor Builtin runtimes** via hardcoded `ensure_shell_runtime()` and `ensure_sensor_runtime()` methods instead of reading from YAML files.

3. **The worker filtered by `runtime_type == "action"`**, but this distinction (action vs sensor) was meaningless — a Python runtime should be usable for both actions and sensors.

4. **Runtime ref naming mismatch.** The Python YAML uses `ref: core.python`, but `resolve_runtime_id("python")` only looked for `core.action.python` and `python` — neither matched.

## Root Cause Analysis: `runtime_type` Was Meaningless

The `runtime_type` column (`action` | `sensor`) conflated *what the runtime is used for* with *what the runtime is*. Analysis of all usages showed:

- **Worker filter**: Only behavioral use — could be replaced by checking if `execution_config` has an interpreter configured.
- **`RuntimeRepository::find_by_type` / `find_action_runtimes`**: Never called anywhere in the codebase. Tests for them were already commented out.
- **`runtime_detection.rs`**: Was filtering by ref pattern (`NOT LIKE '%.sensor.builtin'`), not by `runtime_type`.
- **`DependencyManager::runtime_type()`**: Completely unrelated concept (returns "python"/"nodejs" for language identification).

The real distinction is whether a runtime has an `execution_config` with an interpreter — data that already exists. The column was redundant.

## Changes

### Column Removal: `runtime_type`

| File | Change |
|------|--------|
| `migrations/20250101000002_pack_system.sql` | Removed `runtime_type` column, its CHECK constraint, and its index |
| `crates/common/src/models.rs` | Removed `runtime_type` field from `Runtime` struct |
| `crates/common/src/repositories/runtime.rs` | Removed `runtime_type` from `CreateRuntimeInput`, `UpdateRuntimeInput`, all SELECT/INSERT/UPDATE queries; removed `find_by_type()` and `find_action_runtimes()` |
| `crates/worker/src/service.rs` | Replaced `runtime_type` filter with `execution_config` check (skip runtimes with empty config) |
| `crates/worker/src/executor.rs` | Removed `runtime_type` from runtime SELECT query |
| `crates/common/src/pack_environment.rs` | Removed `runtime_type` from runtime SELECT query |
| `crates/common/src/runtime_detection.rs` | Removed `runtime_type` from runtime SELECT query |
| `crates/common/tests/helpers.rs` | Removed `runtime_type` from `RuntimeFixture` |
| `crates/common/tests/repository_runtime_tests.rs` | Removed `runtime_type` from test fixtures |
| `crates/common/tests/repository_worker_tests.rs` | Removed `runtime_type` from test fixture |
| `crates/common/tests/migration_tests.rs` | Removed stale `runtime_type_enum` from expected enums list |
| `crates/executor/tests/fifo_ordering_integration_test.rs` | Removed `runtime_type` from test fixture |
| `crates/executor/tests/policy_enforcer_tests.rs` | Removed `runtime_type` from test fixture |
| `scripts/load_core_pack.py` | Removed `runtime_type` from INSERT/UPDATE queries |

### Runtime Ref Format Cleanup

Runtime refs now use a clean 2-part `{pack_ref}.{name}` format (e.g., `core.python`, `core.shell`, `core.builtin`). The old 3-part format with `action` or `sensor` segments (e.g., `core.action.shell`, `core.sensor.builtin`) is eliminated.

| File | Change |
|------|--------|
| `packs/core/runtimes/sensor_builtin.yaml` | Renamed ref from `core.sensor.builtin` to `core.builtin` |
| `crates/common/src/schema.rs` | Updated `validate_runtime_ref` to enforce 2-part `pack.name` format; updated tests |
| `crates/common/src/runtime_detection.rs` | Removed `WHERE ref NOT LIKE '%.sensor.builtin'` filter — no ref-based filtering needed |
| `crates/common/src/pack_registry/loader.rs` | Updated hardcoded sensor runtime ref to `core.builtin`; cleaned `resolve_runtime_id()` to use only `core.{name}` patterns (removed legacy `core.action.*` fallbacks) |
| `scripts/load_core_pack.py` | Updated `core.sensor.builtin` references to `core.builtin` |
| `crates/common/tests/repository_runtime_tests.rs` | Updated test refs from 3-part to 2-part format |
| `crates/common/tests/repository_worker_tests.rs` | Updated test ref from 3-part to 2-part format |

### Runtime YAML Loading

| File | Change |
|------|--------|
| `crates/common/src/pack_registry/loader.rs` | Added `load_runtimes()` method to read `runtimes/*.yaml` and insert into DB; added `runtimes_loaded`/`runtimes_skipped` to `PackLoadResult` |
| `crates/api/src/routes/packs.rs` | Updated log message to include runtime count |
| `scripts/load_core_pack.py` | Replaced hardcoded `ensure_shell_runtime()`/`ensure_sensor_runtime()` with `upsert_runtimes()` that reads all YAML files from `runtimes/` directory; added `resolve_action_runtime()` for smart runtime resolution |

### Error Reporting Improvement

| File | Change |
|------|--------|
| `crates/worker/src/executor.rs` | `handle_execution_failure` now accepts an `error_message` parameter. Actual error messages from `prepare_execution_context` and `execute_action` failures are stored in the execution result instead of the generic "Execution failed during preparation". |

## Component Loading Order

`PackComponentLoader::load_all()` now loads in dependency order:
1. **Runtimes** (no dependencies)
2. **Triggers** (no dependencies)
3. **Actions** (depend on runtimes)
4. **Sensors** (depend on triggers and runtimes)

## Deployment

Requires database reset (`docker compose down -v && docker compose up -d`) since the migration changed (column removed).

## Validation

- Zero compiler errors, zero warnings
- All 76 unit tests pass
- Integration test failures are pre-existing (missing `attune_test` database)