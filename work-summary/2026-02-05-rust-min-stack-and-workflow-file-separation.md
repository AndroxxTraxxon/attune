# RUST_MIN_STACK Fix & Workflow File Metadata Separation

**Date**: 2026-02-05

## Summary

Three related changes: (1) fixed `rustc` SIGSEGV crashes during Docker release builds by increasing the compiler stack size, (2) enforced the separation of concerns between action YAML and workflow YAML files across the parser, loaders, and registrars, and (3) updated the workflow builder UI and API save endpoints to produce the correct two-file layout.

## Problem 1: rustc SIGSEGV in Docker Builds

Docker Compose builds were failing with `rustc interrupted by SIGSEGV` during release compilation. The error message suggested increasing `RUST_MIN_STACK` to 16 MiB.

### Fix

Added `ENV RUST_MIN_STACK=67108864` to the build stage of all 7 Rust Dockerfiles:

- `docker/Dockerfile` (both build stages)
- `docker/Dockerfile.optimized`
- `docker/Dockerfile.worker`
- `docker/Dockerfile.worker.optimized`
- `docker/Dockerfile.sensor.optimized`
- `docker/Dockerfile.pack-binaries`
- `docker/Dockerfile.pack-builder`

Also added `export RUST_MIN_STACK := 16777216` to the `Makefile` for local builds.

## Problem 2: Workflow File Metadata Duplication

The `timeline_demo.yaml` workflow file (in `actions/workflows/`) redundantly defined `ref`, `label`, `description`, `parameters`, `output`, and `tags` — all of which are action-level concerns that belong exclusively in the companion action YAML (`actions/timeline_demo.yaml`). This violated the design principle that action YAML owns the interface and workflow YAML owns the execution graph.

The root cause was that `WorkflowDefinition` required `ref` and `label` as mandatory fields, forcing even action-linked workflow files to include them.

### Backend Parser & Loader Changes

**`crates/common/src/workflow/parser.rs`**:
- Made `ref` and `label` optional with `#[serde(default)]` and removed `min = 1` validators
- Added two new tests: `test_parse_action_linked_workflow_without_ref_and_label` and `test_parse_standalone_workflow_still_works_with_ref_and_label`

**`crates/common/src/pack_registry/loader.rs`**:
- `load_workflow_for_action()` now fills in `ref`/`label`/`description`/`tags` from the action YAML when the workflow file omits them (action YAML is authoritative)

**`crates/common/src/workflow/registrar.rs`** and **`crates/executor/src/workflow/registrar.rs`**:
- Added `effective_ref()` and `effective_label()` helper methods that fall back to `WorkflowFile.ref_name` / `WorkflowFile.name` (derived from filename) when the workflow YAML omits them
- Threaded effective values through `create_workflow`, `update_workflow`, `create_companion_action`, and `ensure_companion_action`

**`scripts/load_core_pack.py`**:
- `upsert_workflow_definition()` now derives `ref`/`label`/`description`/`tags` from the action YAML when the workflow file omits them

**`packs.external/python_example/actions/workflows/timeline_demo.yaml`**:
- Stripped `ref`, `label`, `description`, `parameters`, `output`, and `tags` — file now contains only `version`, `vars`, `tasks`, and `output_map`

## Problem 3: Workflow Builder Wrote Full Definition to Disk

The visual workflow builder's save endpoints (`POST /api/v1/packs/{pack_ref}/workflow-files` and `PUT /api/v1/workflows/{ref}/file`) were writing the full `WorkflowYamlDefinition` — including action-level metadata — to the `.workflow.yaml` file on disk. The YAML viewer also showed a single monolithic preview.

### API Save Endpoint Changes

**`crates/api/src/routes/workflows.rs`** — `write_workflow_yaml()`:
- Now writes **two files** per save:
  1. **Workflow YAML** (`actions/workflows/{name}.workflow.yaml`) — graph-only via `strip_action_level_fields()` which removes `ref`, `label`, `description`, `parameters`, `output`, `tags`
  2. **Action YAML** (`actions/{name}.yaml`) — action-level metadata via `build_action_yaml()` which produces `ref`, `label`, `description`, `enabled`, `workflow_file`, `parameters`, `output`, `tags`
- Added `strip_action_level_fields()` helper — extracts only `version`, `vars`, `tasks`, `output_map` from the definition JSON
- Added `build_action_yaml()` helper — constructs the companion action YAML with proper formatting and comments

### Frontend Changes

**`web/src/types/workflow.ts`**:
- Added `WorkflowGraphDefinition` interface (graph-only: `version`, `vars`, `tasks`, `output_map`)
- Added `ActionYamlDefinition` interface (action metadata: `ref`, `label`, `description`, `enabled`, `workflow_file`, `parameters`, `output`, `tags`)
- Added `builderStateToGraph()` — extracts graph-only definition from builder state
- Added `builderStateToActionYaml()` — extracts action metadata from builder state
- Refactored `builderStateToDefinition()` to delegate to `builderStateToGraph()` internally

**`web/src/pages/actions/WorkflowBuilderPage.tsx`**:
- YAML viewer now shows **two side-by-side panels** instead of a single preview:
  - **Left panel (blue, 2/5 width)**: Action YAML — shows `actions/{name}.yaml` content with ref, label, parameters, workflow_file reference
  - **Right panel (green, 3/5 width)**: Workflow YAML — shows `actions/workflows/{name}.workflow.yaml` with graph-only content (version, vars, tasks)
- Each panel has its own copy button, filename label, and description bar explaining the file's role
- Separate `actionYamlPreview` and `workflowYamlPreview` memos replace the old `yamlPreview`

## Design: Two Valid Workflow File Conventions

1. **Standalone workflows** (`workflows/*.yaml`) — no companion action YAML, so they carry their own `ref`, `label`, `parameters`, etc. Loaded by `WorkflowLoader.sync_pack_workflows()`.

2. **Action-linked workflows** (`actions/workflows/*.yaml`) — referenced via `workflow_file` from an action YAML. The action YAML is the single authoritative source for `ref`, `label`, `description`, `parameters`, `output`, and `tags`. The workflow file contains only the execution graph: `version`, `vars`, `tasks`, `output_map`.

The visual workflow builder and API save endpoints now produce the action-linked layout (convention 2) with properly separated files.

## Files Changed

| File | Change |
|------|--------|
| `docker/Dockerfile` | Added `RUST_MIN_STACK=16777216` (both stages) |
| `docker/Dockerfile.optimized` | Added `RUST_MIN_STACK=16777216` |
| `docker/Dockerfile.worker` | Added `RUST_MIN_STACK=16777216` |
| `docker/Dockerfile.worker.optimized` | Added `RUST_MIN_STACK=16777216` |
| `docker/Dockerfile.sensor.optimized` | Added `RUST_MIN_STACK=16777216` |
| `docker/Dockerfile.pack-binaries` | Added `RUST_MIN_STACK=16777216` |
| `docker/Dockerfile.pack-builder` | Added `RUST_MIN_STACK=16777216` |
| `Makefile` | Added `export RUST_MIN_STACK` |
| `crates/common/src/workflow/parser.rs` | Optional `ref`/`label`, 2 new tests |
| `crates/common/src/pack_registry/loader.rs` | Action YAML fallback for metadata |
| `crates/common/src/workflow/registrar.rs` | `effective_ref()`/`effective_label()` |
| `crates/executor/src/workflow/registrar.rs` | `effective_ref()`/`effective_label()` |
| `scripts/load_core_pack.py` | Action YAML fallback for metadata |
| `crates/api/src/routes/workflows.rs` | Two-file write, `strip_action_level_fields()`, `build_action_yaml()` |
| `web/src/types/workflow.ts` | `WorkflowGraphDefinition`, `ActionYamlDefinition`, `builderStateToGraph()`, `builderStateToActionYaml()` |
| `web/src/pages/actions/WorkflowBuilderPage.tsx` | Two-panel YAML viewer |
| `packs.external/python_example/actions/workflows/timeline_demo.yaml` | Stripped action-level metadata |
| `AGENTS.md` | Updated Workflow File Storage, YAML viewer, Docker Build Optimization sections |

## Test Results

- All 23 parser tests pass (including 2 new)
- All 9 loader tests pass
- All 2 registrar tests pass
- All 598 workspace lib tests pass
- Zero TypeScript errors
- Zero compiler warnings
- Zero build errors
