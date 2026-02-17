# Worker On-Demand Virtualenv Creation

**Date:** 2026-02-14

## Problem

When installing Python packs via the API in Docker deployments, virtualenvs were never created. The API service attempted to run `python3 -m venv` during pack registration, but the API container (built from `debian:bookworm-slim`) does not have Python installed. The command failed silently (logged as a warning), and pack registration succeeded without a virtualenv.

When the worker later tried to execute a Python action, it fell back to the system Python interpreter instead of using an isolated virtualenv with the pack's dependencies installed.

## Root Cause

The architecture had a fundamental mismatch: environment setup (virtualenv creation, dependency installation) was performed in the **API service** during pack registration, but the API container lacks runtime interpreters like Python. The **worker service** — which has Python installed — had no mechanism to create environments on demand.

## Solution

Moved the primary responsibility for runtime environment creation from the API service to the worker service:

### Worker-Side: On-Demand Environment Creation

**File:** `crates/worker/src/runtime/process.rs`

Added environment setup at the beginning of `ProcessRuntime::execute()`. Before executing any action, the worker now checks if the runtime has environment configuration and ensures the environment exists:

- Calls `setup_pack_environment()` which is idempotent — it creates the virtualenv only if it doesn't already exist
- On failure, logs a warning and falls back to the system interpreter (graceful degradation)
- The virtualenv is created at `{pack_dir}/.venv` inside the shared packs volume, making it accessible across container restarts

### API-Side: Best-Effort with Clear Logging

**File:** `crates/api/src/routes/packs.rs`

Updated the API's environment setup to be explicitly best-effort:

- Changed log levels from `warn` to `info` for expected failures (missing interpreter)
- Added clear messaging that the worker will handle environment creation on first execution
- Added guard to skip dependency installation when the environment directory doesn't exist (i.e., venv creation failed)
- Preserved the setup attempt for non-Docker (bare-metal) deployments where the API host may have Python available

## How It Works

1. **Pack installation** (API): Registers pack in database, loads components, attempts environment setup (best-effort)
2. **First execution** (Worker): Detects missing `.venv`, creates it via `python3 -m venv`, installs dependencies from `requirements.txt`
3. **Subsequent executions** (Worker): `.venv` already exists, skips setup, resolves interpreter to `.venv/bin/python3`

## Files Changed

| File | Change |
|------|--------|
| `crates/worker/src/runtime/process.rs` | Added on-demand environment setup in `execute()` |
| `crates/api/src/routes/packs.rs` | Updated logging and made environment setup explicitly best-effort |

## Testing

- All 75 worker unit tests pass
- All 23 ProcessRuntime tests pass
- Zero compiler warnings