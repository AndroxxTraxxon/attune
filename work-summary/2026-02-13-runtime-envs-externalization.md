# Runtime Environments Externalization

**Date:** 2026-02-13

## Summary

Completed the refactoring to externalize runtime environments (virtualenvs, node_modules, etc.) from pack directories to a dedicated `runtime_envs_dir`. This ensures pack directories remain clean and read-only while isolated runtime environments are managed at a configurable external location.

## Problem

Previously, runtime environments (e.g., Python virtualenvs) were created inside pack directories at `{pack_dir}/.venv`. This had several issues:

1. **Docker incompatibility**: Pack volumes are mounted read-only (`:ro`) in worker containers, preventing environment creation
2. **API service failures**: The API container doesn't have Python installed, so `python3 -m venv` failed silently during pack registration
3. **Dirty pack directories**: Mixing generated environments with pack source files
4. **Missing `runtime_envs_dir` parameter**: `ProcessRuntime::new()` was updated to accept 4 arguments but callers were still passing 3, causing compile errors

## Changes

### Compile Fixes

- **`crates/worker/src/service.rs`**: Added `runtime_envs_dir` from config and passed as 4th argument to `ProcessRuntime::new()`
- **`crates/worker/src/runtime/local.rs`**: Added `PathBuf::from("/opt/attune/runtime_envs")` as 4th argument to `ProcessRuntime::new()` in `LocalRuntime::new()`
- **`crates/worker/src/runtime/process.rs`**: Suppressed `dead_code` warning on `resolve_pack_dir` (tested utility method kept for API completeness)

### Configuration

- **`config.docker.yaml`**: Added `runtime_envs_dir: /opt/attune/runtime_envs`
- **`config.development.yaml`**: Added `runtime_envs_dir: ./runtime_envs`
- **`config.test.yaml`**: Added `runtime_envs_dir: /tmp/attune-test-runtime-envs`
- **`config.example.yaml`**: Added documented `runtime_envs_dir` setting with explanation
- **`crates/common/src/config.rs`**: Added `runtime_envs_dir` field to test `Config` struct initializers

### Docker Compose (`docker-compose.yaml`)

- Added `runtime_envs` named volume
- Mounted `runtime_envs` volume at `/opt/attune/runtime_envs` in:
  - `api` (for best-effort bare-metal env setup)
  - `worker-shell`, `worker-python`, `worker-node`, `worker-full` (for on-demand env creation)

### API Pack Registration (`crates/api/src/routes/packs.rs`)

Updated the best-effort environment setup during pack registration to use external paths:
- Environment directory computed as `{runtime_envs_dir}/{pack_ref}/{runtime_name}` instead of `{pack_dir}/.venv`
- Uses `build_template_vars_with_env()` for proper template variable resolution with external env_dir
- Creates parent directories before attempting environment creation
- Checks `env_dir.exists()` directly instead of legacy `resolve_env_dir()` for dependency installation

### ProcessRuntime `can_execute` Fix (`crates/worker/src/runtime/process.rs`)

Fixed a pre-existing logic issue where `can_execute` would fall through from a non-matching runtime_name to extension-based matching. When an explicit `runtime_name` is specified in the execution context, it is now treated as authoritative — the method returns the result of the name comparison directly without falling through to extension matching.

### Test Updates

- **`crates/worker/tests/dependency_isolation_test.rs`**: Full rewrite to use external `runtime_envs_dir`. All 17 tests pass. Key changes:
  - Separate `packs_base_dir` and `runtime_envs_dir` temp directories
  - `env_dir` computed as `runtime_envs_dir.join(pack_ref).join(runtime_name)`
  - `setup_pack_environment(&pack_dir, &env_dir)` — now takes 2 arguments
  - `environment_exists("pack_ref")` — now takes pack_ref string
  - Assertions verify environments are created at external locations AND that pack directories remain clean
- **`crates/worker/tests/security_tests.rs`**: Added 4th `runtime_envs_dir` argument to all `ProcessRuntime::new()` calls
- **`crates/worker/tests/log_truncation_test.rs`**: Added 4th `runtime_envs_dir` argument to all `ProcessRuntime::new()` calls
- **`crates/worker/src/runtime/process.rs`** (unit test): Added 4th argument to `test_working_dir_set_to_pack_dir`

## Environment Path Pattern

```
{runtime_envs_dir}/{pack_ref}/{runtime_name}
```

Examples:
- `/opt/attune/runtime_envs/python_example/python` (Docker)
- `./runtime_envs/python_example/python` (development)
- `/tmp/attune-test-runtime-envs/testpack/python` (tests)

## Architecture Summary

| Component | Old Behavior | New Behavior |
|-----------|-------------|-------------|
| Env location | `{pack_dir}/.venv` | `{runtime_envs_dir}/{pack_ref}/{runtime}` |
| Pack directory | Modified by venv | Remains clean/read-only |
| API setup | Pack-relative `build_template_vars` | External `build_template_vars_with_env` |
| Worker setup | Did not create venv | Creates venv on-demand before first execution |
| Docker volumes | Only `packs_data` | `packs_data` (ro) + `runtime_envs` (rw) |
| Config | No `runtime_envs_dir` | Configurable with default `/opt/attune/runtime_envs` |

## Test Results

- **attune-common**: 125 passed, 0 failed
- **attune-worker unit tests**: 76 passed, 0 failed, 4 ignored
- **dependency_isolation_test**: 17 passed, 0 failed
- **log_truncation_test**: 8 passed, 0 failed
- **security_tests**: 5 passed, 2 failed (pre-existing, unrelated to this work)
- **Workspace**: Zero compiler warnings

## Pre-existing Issues (Not Addressed)

- `test_shell_secrets_not_in_environ`: Shell secret delivery mechanism issue
- `test_python_secrets_isolated_between_actions`: Python stdin secret reading doesn't match delivery mechanism