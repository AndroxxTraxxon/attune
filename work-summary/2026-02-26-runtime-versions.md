# Runtime Versions Feature

**Date**: 2026-02-26
**Scope**: Data model, repositories, version matching, pack loader, API, core pack definitions

## Summary

Added support for multiple versions of the same runtime (e.g., Python 3.11, 3.12, 3.13 or Node.js 18, 20, 22). Actions and sensors can now declare a semver version constraint (e.g., `>=3.12`, `~18.0`, `>=3.12,<4.0`) to specify which runtime version they require. The system selects the best matching available version at execution time.

The design is fully data-driven — no hard-coded handling for any specific runtime. All version detection, constraint matching, and execution configuration is driven by database records and YAML definitions.

## Changes

### New Migration: `20260226000000_runtime_versions.sql`
- **`runtime_version` table**: Stores version-specific execution configurations per runtime
  - `runtime` (FK → runtime.id), `runtime_ref`, `version` (semver string)
  - `version_major`, `version_minor`, `version_patch` (ints for efficient range queries)
  - `execution_config` JSONB — complete standalone config (not a diff) replacing the parent runtime's config when this version is selected
  - `distributions` JSONB — version-specific verification commands
  - `is_default` (at most one per runtime), `available`, `verified_at`
  - `meta` JSONB for arbitrary metadata (EOL dates, LTS codenames, etc.)
  - Unique constraint on `(runtime, version)`
- **`action.runtime_version_constraint`** — new nullable TEXT column for semver constraints
- **`sensor.runtime_version_constraint`** — same as above

### New Module: `crates/common/src/version_matching.rs`
- `parse_version()` — lenient semver parsing (`"3.12"` → `3.12.0`, `"v20.11"` → `20.11.0`)
- `parse_constraint()` — constraint parsing with bare version support (`"3.12"` → `~3.12`)
- `matches_constraint()` — check if a version satisfies a constraint
- `select_best_version()` — pick the highest available version matching a constraint, with default-version preference when no constraint is specified
- `extract_version_components()` — split version string into (major, minor, patch) for DB columns
- 33 unit tests covering all constraint operators, edge cases, and selection logic

### New Repository: `crates/common/src/repositories/runtime_version.rs`
- Full CRUD (FindById, List, Create, Update, Delete)
- `find_by_runtime()` — all versions for a runtime, ordered newest-first
- `find_by_runtime_ref()` — same, by runtime ref string
- `find_available_by_runtime()` — only available versions
- `find_default_by_runtime()` — the default version
- `find_by_runtime_and_version()` — exact version lookup
- `clear_default_for_runtime()` — helper for changing defaults
- `set_availability()` — mark available/unavailable with timestamp
- `delete_by_runtime()` — bulk delete

### Model Changes
- **`RuntimeVersion`** struct in `models::runtime` module with `parsed_execution_config()` method
- **`Action`** — added `runtime_version_constraint: Option<String>`
- **`Sensor`** — added `runtime_version_constraint: Option<String>`

### Pack Loader Updates (`crates/common/src/pack_registry/loader.rs`)
- `load_runtimes()` now calls `load_runtime_versions()` after creating each runtime
- `load_runtime_versions()` parses the `versions` array from runtime YAML and creates `runtime_version` rows
- `load_actions()` reads `runtime_version` from action YAML → stored as `runtime_version_constraint`
- `load_sensors()` reads `runtime_version` from sensor YAML → same

### Repository Updates
- **Action repository**: All SELECT/INSERT/UPDATE/RETURNING queries updated to include `runtime_version_constraint`
- **Sensor repository**: Same — all queries updated
- **Input structs**: `CreateActionInput`, `UpdateActionInput`, `CreateSensorInput`, `UpdateSensorInput` all include the new field

### API Updates
- **Action DTOs**: `CreateActionRequest`, `UpdateActionRequest`, `ActionResponse`, `ActionSummary` include `runtime_version_constraint`
- **Route handlers**: `create_action`, `update_action` pass the field through
- OpenAPI annotations added for the new field

### Core Pack Runtime YAML Updates
- **`packs/core/runtimes/python.yaml`**: Added `versions` array with Python 3.11, 3.12 (default), 3.13 — each with version-specific interpreter binary (`python3.11`, `python3.12`, `python3.13`), venv commands, and verification patterns
- **`packs/core/runtimes/nodejs.yaml`**: Added `versions` array with Node.js 18, 20 (default), 22 — each with version-specific binary, verification commands, and LTS metadata

### Dependency Addition
- **`semver` 1.0** (with serde feature) added to workspace and `attune-common` Cargo.toml

### Test Fixes
- All callsites constructing `CreateActionInput`, `CreateSensorInput`, `UpdateActionInput`, `UpdateSensorInput` across the workspace updated with `runtime_version_constraint: None` (approximately 20 files touched)

## Architecture Decisions

1. **Full execution_config per version, not diffs**: Each `runtime_version` row stores a complete `execution_config` rather than overrides. This avoids merge complexity and makes each version self-contained.

2. **Constraint stored as text, matched at runtime**: The `runtime_version_constraint` column stores the raw semver string. Matching is done in Rust code using the `semver` crate rather than in SQL, because semver range logic is complex and better handled in application code.

3. **Bare version = tilde range**: A constraint like `"3.12"` is interpreted as `~3.12` (>=3.12.0, <3.13.0), which is the most common developer expectation for "compatible with 3.12".

4. **No hard-coded runtime handling**: The entire version system is data-driven through the `runtime_version` table and YAML definitions. Any runtime can define versions with arbitrary verification commands and execution configs.

## What's Next

- **Worker integration**: The worker execution pipeline should query `runtime_version` rows when an action has a `runtime_version_constraint`, use `select_best_version()` to pick the right version, and use that version's `execution_config` instead of the parent runtime's.
- **Runtime version detection**: The `RuntimeDetector` should verify individual version availability by running each version's verification commands and updating the `available`/`verified_at` fields.
- **Environment isolation per version**: The `runtime_envs_dir` path pattern may need to include the version (e.g., `{runtime_envs_dir}/{pack_ref}/{runtime_name}-{version}`) to support multiple Python versions with separate virtualenvs.
- **API endpoints**: CRUD endpoints for `runtime_version` management (list versions for a runtime, register new versions, mark availability).
- **Web UI**: Display version information in runtime/action views, version constraint field in action editor.