# Fix: Python Sensor "Permission Denied" — Stale Runtime Assignment

**Date**: 2026-02-20

## Problem

Python-based sensors failed to start with `Permission denied (os error 13)`.

## Root Cause

The sensor's runtime in the database pointed to `core.builtin` (empty `execution_config`) instead of `core.python`. This caused `is_native=true`, making the sensor manager try to execute the `.py` script directly — which fails without the execute bit.

The stale assignment persisted because the pack component loader **skipped** existing sensors on re-registration instead of updating them. Once a sensor was created with the wrong runtime, there was no way to correct it short of deleting the pack entirely.

**DB evidence**: `SELECT runtime, runtime_ref FROM sensor` → `runtime=4, runtime_ref=core.builtin` (should be `runtime=3, runtime_ref=core.python`).

## Changes

### 1. Sensor upsert on re-registration (`crates/common/src/pack_registry/loader.rs`)
- Changed `load_sensors` from skip-if-exists to upsert: existing sensors are updated with fresh metadata from the YAML (runtime, entrypoint, trigger, config, etc.)
- Re-registering a pack now corrects stale runtime assignments

### 2. `UpdateSensorInput` extended (`crates/common/src/repositories/trigger.rs`)
- Added `runtime`, `runtime_ref`, `trigger`, `trigger_ref`, and `config` fields so the update path can correct all sensor metadata
- Updated all callsites in `crates/api/src/routes/triggers.rs` and tests

### 3. Registration-time validation (`crates/common/src/pack_registry/loader.rs`)
- Warns if a non-native `runner_type` (e.g., `python`) resolves to runtime ID 0 (not found)
- Warns if the resolved runtime has empty/missing `execution_config`

### 4. Sensor manager diagnostics (`crates/sensor/src/sensor_manager.rs`)
- Logs full runtime details (id, ref, name, raw `execution_config` JSON)
- Logs `env_dir_exists` status and resolved interpreter path
- Pre-flight check: verifies binary exists and has execute permission before spawn
- Error message includes binary path, `is_native` flag, and runtime ref

### 5. Python loader consistency (`scripts/load_core_pack.py`)
- Added `runtime` and `runtime_ref` to sensor `ON CONFLICT DO UPDATE` clause

## Verification

After rebuilding and re-registering the pack with `force=true`:
- Sensor runtime corrected: `core.builtin` → `core.python`
- Sensor started successfully with venv interpreter at `/opt/attune/runtime_envs/python_example/python/bin/python3`
- Counter sensor fully operational (RabbitMQ connected, rules bootstrapped)