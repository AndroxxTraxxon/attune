# Workflow Builder UI Implementation

**Date:** 2026-02-23

## Summary

Implemented a visual workflow builder interface for creating and editing workflow actions. The builder is accessible from the Actions page and provides a node-based canvas for constructing workflows using installed actions as task building blocks.

## Changes

### Frontend (Web UI)

#### New Pages
- **`web/src/pages/actions/WorkflowBuilderPage.tsx`** — Main workflow builder page with:
  - Top toolbar with pack selector, workflow name/label/version inputs, save button
  - Description, tags, and enabled toggle in a secondary row
  - Three-panel layout: action palette (left), canvas (center), task inspector (right)
  - Definition JSON preview panel (toggleable)
  - Validation error display
  - Support for both create (`/actions/workflows/new`) and edit (`/actions/workflows/:ref/edit`) modes

#### New Components (`web/src/components/workflows/`)
- **`ActionPalette.tsx`** — Searchable sidebar listing all available actions grouped by pack. Clicking an action adds it as a task to the canvas with auto-populated input parameters from the action's schema.
- **`WorkflowCanvas.tsx`** — Visual canvas with:
  - Draggable task nodes with absolute positioning
  - SVG edge rendering for task transitions
  - Interactive connection mode: click a port on one node, then click another node to create success/failure transitions
  - Grid background, empty state with guidance
  - Floating "add task" button
- **`TaskNode.tsx`** — Individual task node component showing task name, action reference, input count, badges for conditions/retry/iteration, and connection/configure/delete action buttons
- **`WorkflowEdges.tsx`** — SVG overlay rendering curved bezier edges between connected nodes with color-coded and dash-styled lines per transition type (success=green, failure=red dashed, complete=indigo, timeout=amber, decision=violet). Includes arrow markers and edge labels.
- **`TaskInspector.tsx`** — Right-side property panel with collapsible sections for:
  - Basic settings (name, type, condition)
  - Action selection (dropdown of all actions) with auto-populate from schema
  - Transitions (on_success, on_failure, on_complete, on_timeout dropdowns)
  - Iteration (with_items, batch_size, concurrency)
  - Retry & timeout configuration
  - Publish variables (key=value pairs for workflow variable publishing)

#### New Types & Utilities (`web/src/types/workflow.ts`)
- TypeScript types for workflow builder state, tasks, edges, parameters, YAML definition format
- `builderStateToDefinition()` — Converts builder state to the YAML-compatible definition format
- `definitionToBuilderState()` — Converts existing workflow definitions back to builder state (for edit mode)
- `deriveEdges()` — Extracts visual edges from task transition properties
- `validateWorkflow()` — Client-side validation (name, label, version, pack, task names, action assignments, transition references)
- Utility functions: `generateTaskId()`, `createEmptyTask()`, `generateUniqueTaskName()`

#### New Hooks (`web/src/hooks/useWorkflows.ts`)
- `useWorkflows()` — List workflows with filtering
- `useWorkflow()` — Get single workflow by ref
- `useCreateWorkflow()` / `useUpdateWorkflow()` / `useDeleteWorkflow()` — Standard CRUD mutations
- `useSaveWorkflowFile()` — Calls `POST /api/v1/packs/{pack_ref}/workflow-files` to save workflow file to disk
- `useUpdateWorkflowFile()` — Calls `PUT /api/v1/workflows/{ref}/file` to update workflow file on disk

#### Modified Files
- **`web/src/pages/actions/ActionsPage.tsx`** — Added "Workflow" button in the header that navigates to `/actions/workflows/new`
- **`web/src/App.tsx`** — Added lazy-loaded routes for `WorkflowBuilderPage` at `/actions/workflows/new` and `/actions/workflows/:ref/edit`

### Backend (API)

#### New Endpoints
- **`POST /api/v1/packs/{pack_ref}/workflow-files`** — Saves a new workflow:
  1. Validates the request and checks the pack exists
  2. Checks for duplicate workflow ref
  3. Writes `{name}.workflow.yaml` to `{packs_base_dir}/{pack_ref}/actions/workflows/`
  4. Creates the `workflow_definition` record in the database
  5. Returns the workflow response

- **`PUT /api/v1/workflows/{ref}/file`** — Updates an existing workflow:
  1. Validates the request and finds the existing workflow
  2. Overwrites the YAML file on disk
  3. Updates the database record
  4. Returns the updated workflow response

#### New DTO
- **`SaveWorkflowFileRequest`** in `crates/api/src/dto/workflow.rs` — Request body with name, label, description, version, pack_ref, definition (JSON), param_schema, out_schema, tags, enabled

#### Modified Files
- **`crates/api/src/routes/workflows.rs`** — Added `save_workflow_file`, `update_workflow_file` handlers and helper function `write_workflow_yaml`. Updated routes to include new endpoints. Added unit tests.
- **`crates/api/src/dto/workflow.rs`** — Added `SaveWorkflowFileRequest` DTO

## Workflow File Storage

Workflow files are saved to: `{packs_base_dir}/{pack_ref}/actions/workflows/{name}.workflow.yaml`

This is a new path (`actions/workflows/`) distinct from the existing `workflows/` directory used by the pack sync mechanism. The definition is serialized as YAML and simultaneously persisted to both disk and database.

## Testing

- All 89 existing unit tests pass
- 2 new unit tests added for `SaveWorkflowFileRequest` validation
- TypeScript compilation passes with zero errors from new code
- Rust workspace compilation passes with zero warnings