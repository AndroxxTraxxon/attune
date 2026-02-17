# Work Summary: Pack Installation Fixes (2026-02-13)

## Problem

The `/packs/install` web UI page was completely non-functional when attempting to install packs from git repositories. Multiple cascading issues prevented successful pack installation via the API.

## Issues Fixed

### 1. `git` binary missing from API container
**Error:** `Failed to execute git clone: No such file or directory (os error 2)`

The `install_from_git` method in `PackInstaller` runs `Command::new("git")` to clone repositories, but the runtime Docker image (`debian:bookworm-slim`) did not include `git`.

**Fix:** Added `git` to the runtime stage's `apt-get install` in `docker/Dockerfile.optimized`.

### 2. Pack tests ran before pack files existed at expected location
**Error:** `Pack directory not found: /opt/attune/packs/python_example`

The `execute_and_store_pack_tests` function always constructed the pack path as `packs_base_dir/pack_ref`, but during installation the pack files were still in a temp directory. The move to permanent storage happened *after* test execution.

**Fix:**
- Added `execute_pack_tests_at(pack_dir, ...)` method to `TestExecutor` that accepts an explicit directory path
- Added `pack_dir_override: Option<&std::path::Path>` parameter to `execute_and_store_pack_tests`
- `register_pack_internal` now passes the actual pack path through to tests

### 3. Missing test config treated as installation failure
**Error:** `No testing configuration found in pack.yaml for pack 'python_example'`

Packs without a `testing` section in `pack.yaml` could not be installed without `force=true`, because the absence of test config was returned as an error.

**Fix:** Changed `execute_and_store_pack_tests` return type from `Result<PackTestResult>` to `Option<Result<PackTestResult>>`. Returns `None` when no testing config exists or testing is disabled, which the caller treats as "no tests to run" (success). All `?` operators were replaced with explicit `match`/`return` to work with the `Option<Result<...>>` return type.

### 4. Packs volume mounted read-only on API container
**Error:** `Read-only file system (os error 30)`

The `packs_data` volume was mounted as `:ro` on the API container, and files were owned by root (written by `init-packs` running as root). The API service (running as user `attune`, uid 1000) could not write.

**Fix:**
- Changed volume mount from `packs_data:/opt/attune/packs:ro` to `:rw` in `docker-compose.yaml`
- Added `chown -R 1000:1000 "$TARGET_PACKS_DIR"` to `docker/init-packs.sh` (runs after initial pack copy and again after all packs loaded)

### 5. Pack components not loaded into database
**Symptom:** Pack installed successfully but actions, triggers, and sensors not visible in the UI.

The `register_pack_internal` function only created the `pack` table record and synced workflows. It never loaded the pack's individual components (actions, triggers, sensors) from their YAML definition files. This was previously only handled by the Python `load_core_pack.py` script during `init-packs`.

**Fix:** Created `PackComponentLoader` in `crates/common/src/pack_registry/loader.rs` — a Rust-native pack component loader that:
- Reads `triggers/*.yaml` and creates trigger records via `TriggerRepository`
- Reads `actions/*.yaml` and creates action records with full field support (parameter_delivery, parameter_format, output_format) via direct SQL
- Reads `sensors/*.yaml` and creates sensor records via `SensorRepository`, resolving trigger and runtime references
- Loads in dependency order: triggers → actions → sensors
- Skips components that already exist (idempotent)
- Resolves runtime IDs by looking up common ref patterns (e.g., `shell` → `core.action.shell`)

Integrated into `register_pack_internal` so both the `/packs/install` and `/packs/register` endpoints load components.

### 6. Pack stored with version suffix in directory name
**Symptom:** Pack stored at `python_example-1.0.0` but workers/sensors look for `python_example`.

`PackStorage::install_pack` was called with `Some(&pack.version)`, creating a versioned directory name. The rest of the system expects `packs_base_dir/pack_ref` without version.

**Fix:** Changed to `install_pack(&installed.path, &pack.r#ref, None)` to match the system convention.

## Files Changed

| File | Change |
|------|--------|
| `docker/Dockerfile.optimized` | Added `git` to runtime dependencies |
| `docker/init-packs.sh` | Added `chown -R 1000:1000` for attune user write access |
| `docker-compose.yaml` | Changed packs volume mount from `:ro` to `:rw` on API |
| `crates/common/src/test_executor.rs` | Added `execute_pack_tests_at` method |
| `crates/common/src/pack_registry/loader.rs` | **New file** — `PackComponentLoader` |
| `crates/common/src/pack_registry/mod.rs` | Added `loader` module and re-exports |
| `crates/api/src/routes/packs.rs` | Fixed test execution path, no-test-config handling, component loading, storage path |