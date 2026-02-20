# Work Summary: Native Runtime Refactor, Shell Wrapper Cleanup & Stdin Protocol Fixes (2026-02-20)

## Problem

Python sensors (e.g., `python_example.counter_sensor`) were being executed by `/bin/sh` instead of `python3`, causing `import: not found` errors. Root cause was two-fold:

1. **All sensors hardcoded to `core.builtin` runtime** — `PackComponentLoader::load_sensors()` ignored the sensor YAML's `runner_type` field and assigned every sensor the `core.builtin` runtime.
2. **`core.builtin` runtime defaulted to `/bin/sh`** — The `InterpreterConfig` default was `/bin/sh`, so runtimes with no `execution_config` (like `core.builtin`) got shell as their interpreter, causing Python scripts to be interpreted as shell.

Additionally, the shell wrapper script had a hard dependency on `python3` for JSON secret parsing, and pre-existing security test failures were discovered caused by a stdin protocol conflict between parameters and secrets.

## Changes

### Architecture: Remove "builtin" runtime concept

Replaced the separate `core.builtin` runtime with `core.native`. Runtime detection is now purely data-driven via `execution_config` in the runtime table — no special-cased runtime names.

- **Deleted** `packs/core/runtimes/sensor_builtin.yaml`
- **Fixed** `packs/core/runtimes/native.yaml` — removed `/bin/sh -c` interpreter; empty `execution_config` signals direct binary execution
- **Changed** `InterpreterConfig` default `binary` from `"/bin/sh"` to `""` (empty = native)
- **Updated** `interval_timer_sensor.yaml` — `runner_type: native`

### Sensor runtime resolution

- **Fixed** `PackComponentLoader::load_sensors()` — reads `runner_type` from each sensor's YAML definition and resolves to the correct runtime via `resolve_runtime()`. Defaults to `native`.
- **Added** `resolve_runtime()` method returning `(Id, String)` — both ID and ref
- **Updated** runtime mappings — `builtin`, `standalone` → `core.native`
- **Updated** `load_core_pack.py` — per-sensor runtime resolution from YAML instead of hardcoded `core.builtin`
- **Updated** `seed_core_pack.sql` — references `core.native` instead of `core.builtin`

### Shell wrapper: remove Python dependency

The shell wrapper script (`generate_wrapper_script`) previously used `python3 -c` to parse JSON secrets from stdin into a bash associative array. This created a hard dependency on Python being installed, which violates the principle that core services must operate without supplemental runtimes.

- **Replaced** runtime JSON parsing with Rust-side secret injection — secrets are now embedded directly as `ATTUNE_SECRETS['key']='value'` entries at script generation time
- **Added** `bash_single_quote_escape()` helper for safe bash string embedding
- **Changed** wrapper execution from `bash -c <script>` to writing a temp file and executing it, keeping secrets out of `/proc/<pid>/cmdline`
- **Removed** unused `execute_shell_code()` method (wrapper now uses `execute_shell_file`)
- **Also applied** bash single-quote escaping to parameter values embedded in the wrapper
- **Un-ignored** `test_shell_runtime_with_secrets` — it now passes

### Stdin protocol fixes (pre-existing bugs)

- **Process executor**: Skip writing empty/trivial parameter content (`{}`, `""`, `[]`) to stdin to avoid breaking scripts that read secrets via `readline()`
- **Shell streaming executor**: Same empty-params skip applied
- **Worker env tests**: Replaced flaky env-var-manipulating tests with pure parsing tests to eliminate parallel test interference

## Files Changed

| File | Change |
|------|--------|
| `crates/common/src/models.rs` | Default interpreter binary: `"/bin/sh"` → `""` |
| `crates/common/src/pack_registry/loader.rs` | Sensors read `runner_type` from YAML; added `resolve_runtime()`; removed `builtin` mapping |
| `crates/common/src/runtime_detection.rs` | Comment update |
| `crates/common/src/schema.rs` | Test: `core.builtin` → `core.native` |
| `crates/sensor/src/sensor_manager.rs` | Updated `is_native` comments |
| `crates/worker/src/env_setup.rs` | Fixed flaky env var tests |
| `crates/worker/src/runtime/shell.rs` | Rewrote wrapper to embed secrets from Rust (no Python); temp file execution; removed `execute_shell_code`; un-ignored secrets test |
| `crates/worker/src/runtime/process_executor.rs` | Skip empty params on stdin |
| `crates/worker/src/service.rs` | Comment update |
| `packs/core/runtimes/native.yaml` | Removed `/bin/sh` interpreter; empty execution_config |
| `packs/core/runtimes/sensor_builtin.yaml` | **Deleted** |
| `packs/core/runtimes/README.md` | Removed sensor_builtin reference |
| `packs/core/sensors/interval_timer_sensor.yaml` | `runner_type: native` |
| `scripts/load_core_pack.py` | Per-sensor runtime resolution from YAML |
| `scripts/seed_core_pack.sql` | `core.native` references |
| `AGENTS.md` | Updated runtime documentation |

## Test Results

- All 7 security tests pass (2 previously failing now fixed)
- All 82 worker unit tests pass (1 previously ignored now un-ignored and passing)
- All 17 dependency isolation tests pass
- All 8 log truncation tests pass
- All 145 common unit tests pass
- Zero compiler warnings