# Universal Worker Agent — Phase 6: Database & Runtime Registry Extensions

**Date**: 2026-02
**Phase**: 6 of 7 (Universal Worker Agent)
**Plan**: `docs/plans/universal-worker-agent.md`

## Overview

Phase 6 extends the runtime registry so that the universal worker agent (`attune-agent`) can work with arbitrary runtimes — including languages like Ruby, Go, Java, Perl, and R — without requiring every possible runtime to be pre-registered in the database by an administrator.

## Changes Made

### 6.1 Extended Runtime Detection Metadata

**Migration** (`migrations/20250101000012_agent_runtime_detection.sql`):
- Added `auto_detected BOOLEAN NOT NULL DEFAULT FALSE` column to `runtime` table — distinguishes agent-created runtimes from pack-loaded ones
- Added `detection_config JSONB NOT NULL DEFAULT '{}'` column — stores detection metadata (detected binary path, version, runtime name)
- Added index `idx_runtime_auto_detected` for efficient filtering

**Rust Model** (`crates/common/src/models.rs`):
- Added `auto_detected: bool` and `detection_config: JsonDict` fields to the `Runtime` struct

**Repository** (`crates/common/src/repositories/runtime.rs`):
- Added `SELECT_COLUMNS` constant centralising the column list for all runtime queries
- Added `auto_detected` and `detection_config` to `CreateRuntimeInput` and `UpdateRuntimeInput`
- Updated ALL 7 SELECT queries, 2 RETURNING clauses, and the INSERT statement to include the new columns
- Updated the `update` method to support setting `auto_detected` and `detection_config`

**External query sites updated**:
- `crates/common/src/runtime_detection.rs` — `detect_from_database()`
- `crates/common/src/pack_environment.rs` — `get_runtime()`
- `crates/worker/src/executor.rs` — `prepare_execution_context()`

**All `CreateRuntimeInput` construction sites updated** (7 files):
- `crates/api/src/routes/runtimes.rs`
- `crates/common/src/pack_registry/loader.rs`
- `crates/common/tests/helpers.rs`
- `crates/common/tests/repository_runtime_tests.rs`
- `crates/common/tests/repository_worker_tests.rs`
- `crates/executor/tests/fifo_ordering_integration_test.rs`
- `crates/executor/tests/policy_enforcer_tests.rs`

### 6.2 Runtime Template Packs

Added 5 new runtime YAML definitions in `packs/core/runtimes/`:

| File | Ref | Interpreter | Environment | Dependencies |
|------|-----|-------------|-------------|--------------|
| `ruby.yaml` | `core.ruby` | `ruby` (.rb) | GEM_HOME isolation | Gemfile → bundle install |
| `go.yaml` | `core.go` | `go run` (.go) | GOPATH isolation | go.mod → go mod download |
| `java.yaml` | `core.java` | `java` (.java) | None (simple) | None |
| `perl.yaml` | `core.perl` | `perl` (.pl) | local::lib isolation | cpanfile → cpanm |
| `r.yaml` | `core.r` | `Rscript --vanilla` (.R) | renv isolation | renv.lock → renv::restore() |

Each includes verification commands matching the auto-detection module's probe strategy.

### 6.3 Dynamic Runtime Registration

**New module** (`crates/worker/src/dynamic_runtime.rs`):
- `auto_register_detected_runtimes(pool, detected)` — main entry point called from `agent_main.rs` BEFORE `WorkerService::new()`
- For each detected runtime:
  1. Alias-aware lookup in existing DB runtimes (via `normalize_runtime_name`)
  2. If not found, looks for a template runtime by ref pattern `core.<name>`
  3. If template found, clones it with `auto_detected = true` and substitutes the detected binary path
  4. If no template, creates a minimal runtime with just the interpreter binary and file extension
  5. Auto-registered runtimes use ref format `auto.<name>` (e.g., `auto.ruby`)
- Helper functions: `build_detection_config()`, `build_execution_config_from_template()`, `build_minimal_execution_config()`, `build_minimal_distributions()`, `capitalize_runtime_name()`
- 8 unit tests covering all helpers

**Agent entrypoint** (`crates/worker/src/agent_main.rs`):
- Added Phase 2b between config loading and `WorkerService::new()`
- Creates a temporary DB connection and calls `auto_register_detected_runtimes()` for all detected runtimes
- Non-fatal: registration failures are logged as warnings, agent continues

**Runtime name normalization** (`crates/common/src/runtime_detection.rs`):
- Extended `normalize_runtime_name()` with 5 new alias groups:
  - `ruby`/`rb` → `ruby`
  - `go`/`golang` → `go`
  - `java`/`jdk`/`openjdk` → `java`
  - `perl`/`perl5` → `perl`
  - `r`/`rscript` → `r`
- Added 5 new unit tests + 6 new assertions in existing filter tests

## Architecture Decisions

1. **Dynamic registration before WorkerService::new()**: The `WorkerService` constructor loads runtimes from the DB into an immutable `RuntimeRegistry` wrapped in `Arc`. Rather than restructuring this, dynamic registration runs beforehand so the normal loading pipeline picks up the new entries.

2. **Template-based cloning**: Auto-detected runtimes clone their execution config from pack templates (e.g., `core.ruby`) when available, inheriting environment management, dependency installation, and env_vars configuration. Only the interpreter binary path is substituted with the actual detected path.

3. **Minimal fallback**: When no template exists, a bare-minimum runtime entry is created with just the interpreter binary. This enables immediate script execution without environment/dependency management.

4. **`auto.` ref prefix**: Auto-detected runtimes use `auto.<name>` refs to avoid collisions with pack-registered templates (which use `core.<name>` or `<pack>.<name>`).

## Test Results

- **Worker crate**: 114 passed, 0 failed, 3 ignored
- **Common crate**: 321 passed, 0 failed
- **API crate**: 110 passed, 0 failed, 1 ignored
- **Executor crate**: 115 passed, 0 failed, 1 ignored
- **Workspace check**: Zero errors, zero warnings