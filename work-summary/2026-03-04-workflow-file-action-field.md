# Workflow Action `workflow_file` Field & Timeline Demo Workflow

**Date**: 2026-03-04
**Scope**: Pack loading architecture, workflow file discovery, demo workflow

## Summary

Introduced a `workflow_file` field for action YAML definitions that separates action-level metadata from workflow graph definitions. This enables a clean conceptual divide: the action YAML controls ref, label, parameters, policies, and tags, while the workflow file contains the execution graph (tasks, transitions, variables). Multiple actions can reference the same workflow file with different configurations, which has implications for policies and parameter mapping.

Also created a comprehensive demo workflow in the `python_example` pack that exercises the Workflow Timeline DAG visualizer.

## Architecture Change

### Before

Workflows could be registered two ways, each with limitations:

1. **`workflows/` directory** (pack root) — scanned by `WorkflowLoader`, registered by `WorkflowRegistrar` which auto-creates a companion action. No separation of action metadata from workflow definition.
2. **API endpoints** (`POST /api/v1/packs/{ref}/workflow-files`) — writes to `actions/workflows/`, creates both `workflow_definition` and companion `action` records. Only available via the visual builder, not during pack file loading.

The `PackComponentLoader` had no awareness of workflow files at all — it only loaded actions, triggers, runtimes, and sensors from their respective directories.

### After

A third path is now supported, bridging both worlds:

3. **Action YAML with `workflow_file` field** — an action YAML in `actions/*.yaml` can include `workflow_file: workflows/timeline_demo.yaml` (path relative to `actions/`). During pack loading, the `PackComponentLoader`:
   - Reads and parses the referenced workflow YAML
   - Creates/updates a `workflow_definition` record
   - Creates the action record with `workflow_def` FK linked
   - Skips runtime resolution (workflow actions have no runner_type)
   - Uses the workflow file path as the entrypoint

This preserves the clean separation the visual builder already uses (action metadata in one place, workflow graph in another) while making it work with the pack file loading pipeline.

### Dual-Directory Workflow Scanning

The `WorkflowLoader` now scans **two** directories:

1. `{pack_dir}/workflows/` — legacy standalone workflow files
2. `{pack_dir}/actions/workflows/` — visual-builder and action-linked workflow files

Files with `.workflow.yaml` suffix have the `.workflow` portion stripped when deriving the name/ref (e.g., `deploy.workflow.yaml` → name `deploy`, ref `pack.deploy`). If the same ref appears in both directories, `actions/workflows/` wins. The `reload_workflow` method searches `actions/workflows/` first with all extension variants.

## Files Changed

### Rust (`crates/common/src/pack_registry/loader.rs`)

- Added imports for `WorkflowDefinitionRepository`, `CreateWorkflowDefinitionInput`, `UpdateWorkflowDefinitionInput`, and `parse_workflow_yaml`
- **`load_actions()`**: When action YAML contains `workflow_file`, calls `load_workflow_for_action()` to create/update the workflow definition, sets entrypoint to the workflow file path, skips runtime resolution, and links the action to the workflow definition after creation/update
- **`load_workflow_for_action()`** (new): Reads and parses the workflow YAML, creates or updates the `workflow_definition` record, respects action YAML schema overrides (action's `parameters`/`output` take precedence over the workflow file's own schemas)

### Rust (`crates/common/src/workflow/loader.rs`)

- **`load_pack_workflows()`**: Now scans both `workflows/` and `actions/workflows/`, with the latter taking precedence on ref collision
- **`reload_workflow()`**: Searches `actions/workflows/` first, trying `.workflow.yaml`, `.yaml`, `.workflow.yml`, and `.yml` extensions before falling back to `workflows/`
- **`scan_workflow_files()`**: Strips `.workflow` suffix from filenames (e.g., `deploy.workflow.yaml` → name `deploy`)
- **3 new tests**: `test_scan_workflow_files_strips_workflow_suffix`, `test_load_pack_workflows_scans_both_directories`, `test_reload_workflow_finds_actions_workflows_dir`

### Python (`scripts/load_core_pack.py`)

- **`upsert_workflow_definition()`** (new): Reads a workflow YAML file, upserts into `workflow_definition` table, returns the ID
- **`upsert_actions()`**: Detects `workflow_file` field, calls `upsert_workflow_definition()`, sets entrypoint to workflow file path, skips runtime resolution for workflow actions, links action to workflow definition via `UPDATE action SET workflow_def = ...`

### Demo Pack Files (`packs.external/python_example/`)

- **`actions/simulate_work.py`** + **`actions/simulate_work.yaml`**: New action that simulates a unit of work with configurable duration, optional failure simulation, and structured JSON output
- **`actions/timeline_demo.yaml`**: Action YAML with `workflow_file: workflows/timeline_demo.yaml` — controls action-level metadata
- **`actions/workflows/timeline_demo.yaml`**: Workflow definition with 11 tasks, 18 transition edges, exercising parallel fan-out/fan-in, `with_items` + concurrency, failure paths, retries, timeouts, publish directives, and custom edge styling via `__chart_meta__`

### Documentation

- **`AGENTS.md`**: Updated Pack Component Loading Order, added Workflow Action YAML (`workflow_file` field) section, added Workflow File Discovery (dual-directory scanning) section, added pitfall #7 (never put workflow content directly in action YAML), renumbered subsequent items
- **`packs.external/python_example/README.md`**: Added docs for `simulate_work`, `timeline_demo` workflow, and usage examples

## Test Results

- **596 unit tests passing**, 0 failures
- **0 compiler warnings** across the workspace
- 3 new tests for the workflow loader changes, all passing
- Integration tests require `attune_test` database (pre-existing infrastructure issue, unrelated)

## Timeline Demo Workflow Features

The `python_example.timeline_demo` workflow creates this execution shape:

```
initialize ─┬─► build_artifacts(6s) ────────────────┐
            ├─► run_linter(3s) ─────┐               ├─► merge_results ─► generate_items ─► process_items(×5, 3∥) ─► validate ─┬─► finalize_success
            └─► security_scan(4s) ──┘               │                                                                        └─► handle_failure ─► finalize_failure
                                    └───────────────┘
```

| Feature | How Exercised |
|---------|--------------|
| Parallel fan-out | `initialize` → 3 branches with different durations |
| Fan-in / join | `merge_results` with `join: 3` |
| `with_items` + concurrency | `process_items` expands to N items, `concurrency: 3` |
| Failure paths | Every task has `{{ failed() }}` transitions |
| Timeout handling | `security_scan` has `timeout: 30` + `{{ timed_out() }}` |
| Retries | `build_artifacts` and `validate` with retry configs |
| Publish directives | Results passed between stages |
| Custom edge colors/labels | Via `__chart_meta__` on transitions |
| Configurable failure | `fail_validation=true` exercises the error path |