# Remove `is_workflow` from Action Table & Add Workflow Edit Button

**Date**: 2026-02

## Summary

Removed the redundant `is_workflow` boolean column from the `action` table throughout the entire stack. An action being a workflow is fully determined by having a non-null `workflow_def` FK — the boolean was unnecessary. Also added a workflow edit button and visual indicator to the Actions page UI.

## Changes

### Backend — Drop `is_workflow` from Action

**`crates/common/src/models.rs`**
- Removed `is_workflow: bool` field from the `Action` struct

**`crates/common/src/repositories/action.rs`**
- Removed `is_workflow` from all SELECT column lists (9 queries)
- Updated `find_workflows()` to use `WHERE workflow_def IS NOT NULL` instead of `WHERE is_workflow = true`
- Updated `link_workflow_def()` to only `SET workflow_def = $2` (no longer sets `is_workflow = true`)

**`crates/api/src/dto/action.rs`**
- Removed `is_workflow` field from `ActionResponse` and `ActionSummary` DTOs
- Added `workflow_def: Option<i64>` field to both DTOs (non-null means this action is a workflow)
- Updated `From<Action>` impls accordingly

**`crates/api/src/validation/params.rs`**
- Removed `is_workflow` from test fixture `make_action()`

**Comments updated in:**
- `crates/api/src/routes/workflows.rs` — companion action helper functions
- `crates/common/src/workflow/registrar.rs` — companion action creation
- `crates/executor/src/workflow/registrar.rs` — companion action creation

### Database Migration

**`migrations/20250101000006_workflow_system.sql`** (modified in-place, no production deployments)
- Removed the `ADD COLUMN is_workflow` step from the ALTER TABLE
- Removed `idx_action_is_workflow` partial index
- Updated `workflow_action_link` view to use `LEFT JOIN action a ON a.workflow_def = wd.id` (dropped `AND a.is_workflow = true` filter)
- Updated column comment on `workflow_def`

> Note: At the time of this action-table cleanup, the execution table still had separate DB-level workflow notification plumbing. That follow-up cleanup is handled independently.

### Frontend — Workflow Edit Button & Indicator

**TypeScript types updated** (4 files):
- `web/src/api/models/ActionResponse.ts` — added `workflow_def?: number | null`
- `web/src/api/models/ActionSummary.ts` — added `workflow_def?: number | null`
- `web/src/api/models/PaginatedResponse_ActionSummary.ts` — added `workflow_def?: number | null`
- `web/src/api/models/ApiResponse_ActionResponse.ts` — added `workflow_def?: number | null`

**`web/src/pages/actions/ActionsPage.tsx`**
- **Action list sidebar**: Workflow actions now show a purple `GitBranch` icon next to their label
- **Action detail view**: Workflow actions show a purple "Edit Workflow" button (with `Pencil` icon) that navigates to `/actions/workflows/:ref/edit`

### Prior Fix — Workflow Save Upsert (same session)

**`web/src/pages/actions/WorkflowBuilderPage.tsx`**
- Fixed workflow save from "new" page when workflow already exists
- On 409 CONFLICT from POST, automatically falls back to PUT (update) with the same data
- Constructs the workflow ref as `{packRef}.{name}` for the fallback PUT call

## Design Rationale

The `is_workflow` boolean on the action table was fully redundant:
- A workflow action always has `workflow_def IS NOT NULL`
- A workflow action's entrypoint always ends in `.workflow.yaml`
- The executor detects workflows by looking up `workflow_definition` by ref, not by checking `is_workflow`
- No runtime code path depended on the boolean that couldn't use `workflow_def IS NOT NULL` instead
