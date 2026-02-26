# Workflow Companion Action Fix

**Date**: 2026-02-25
**Issue**: Workflows created via the web Workflow Builder did not appear in the action list or action palette.

## Root Cause

When a workflow was saved via the Workflow Builder UI (`POST /api/v1/packs/{pack_ref}/workflow-files`), the API handler:

1. ✅ Wrote the workflow YAML file to disk
2. ✅ Created a `workflow_definition` record in the database
3. ❌ Did **not** create a corresponding `action` record

The action palette and action list both query the `action` table. Since no `action` record was created for workflows, they were invisible in those views despite existing in the `workflow_definition` table.

The same gap existed in:
- `POST /api/v1/workflows` (create workflow via API)
- `PUT /api/v1/workflows/{ref}` (update workflow via API)
- `PUT /api/v1/workflows/{ref}/file` (update workflow file)
- `WorkflowRegistrar` in both `crates/common` and `crates/executor` (pack-based workflow loading)

## Fix

### Companion Action Records

Every workflow definition now gets a **companion action record** with:
- `is_workflow = true`
- `workflow_def` FK pointing to the workflow definition
- Same `ref`, `label`, `description`, `param_schema`, `out_schema` as the workflow
- `entrypoint` set to the workflow YAML file path (e.g., `workflows/{name}.workflow.yaml`)
- `runtime = NULL` (workflows don't use a runtime — the executor reads the definition from DB)

### Files Modified

**`crates/api/src/routes/workflows.rs`**:
- `save_workflow_file()`: Now creates a companion action after creating the workflow definition
- `create_workflow()`: Now creates a companion action after creating the workflow definition
- `update_workflow()`: Now updates the companion action's metadata to stay in sync
- `update_workflow_file()`: Uses `ensure_companion_action()` to update or backfill the action
- `delete_workflow()`: No change needed — the `action.workflow_def` FK has `ON DELETE CASCADE`, so deleting the workflow definition automatically deletes the companion action
- Added three helper functions: `create_companion_action()`, `update_companion_action()`, `ensure_companion_action()`

**`crates/common/src/workflow/registrar.rs`**:
- `register_workflow()`: Creates companion action on new workflows, ensures/updates on existing
- Added `create_companion_action()` and `ensure_companion_action()` methods

**`crates/executor/src/workflow/registrar.rs`**:
- Same changes as the common crate registrar (this is a duplicate used by the executor service)

### Backfill Support

The `ensure_companion_action()` function handles workflows that were created before this fix. When updating such a workflow, if no companion action exists, it creates one. This means existing workflows will get their companion action on the next update/re-registration.

## Testing

- All workspace lib tests pass (86 API, 160 common, 85 executor, 82 worker)
- Zero compiler warnings across the workspace
- Compilation clean after `cargo clean` + full rebuild