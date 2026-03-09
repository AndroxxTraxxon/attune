# Work Summary: Workflow Cancellation Policy

**Date**: 2026-03-09

## Overview

Added configurable cancellation behavior for workflows. When a workflow is cancelled, the policy controls whether currently running tasks are allowed to finish (default) or are forcefully terminated via SIGTERM.

## Changes

### Backend (Rust)

**`crates/common/src/workflow/parser.rs`**
- Added `CancellationPolicy` enum with two variants: `AllowFinish` (default) and `CancelRunning`
- Added `cancellation_policy` field to `WorkflowDefinition` with `#[serde(default, skip_serializing_if)]` for backward compatibility
- Added 5 unit tests covering default behavior, explicit values, JSON round-trip, and deserialization of legacy definitions without the field

**`crates/common/src/workflow/mod.rs`**
- Re-exported `CancellationPolicy` from the workflow module

**`crates/api/src/routes/executions.rs`**
- Added `resolve_cancellation_policy()` helper that loads the workflow definition from DB and extracts the policy (falls back to `AllowFinish` on any failure)
- Refactored `cancel_workflow_children()` to resolve the policy and delegate to `cancel_workflow_children_with_policy()`
- `cancel_workflow_children_with_policy()` respects the policy:
  - **`AllowFinish`**: Only cancels pre-running children (Requested/Scheduling/Scheduled). Running children are left alone to complete naturally. `advance_workflow` sees the cancelled `workflow_execution` and stops dispatching new tasks.
  - **`CancelRunning`**: Cancels all children including running ones via SIGINT→SIGTERM→SIGKILL MQ messages to workers (previous hard-coded behavior).
- Policy is inherited through recursive cancellation of nested workflows

### Frontend (TypeScript/React)

**`web/src/types/workflow.ts`**
- Added `CancellationPolicy` type and `CANCELLATION_POLICY_LABELS` constant
- Added `cancellationPolicy` field to `WorkflowBuilderState`
- Added `cancellation_policy` field to `WorkflowYamlDefinition` and `WorkflowGraphDefinition`
- Updated `builderStateToDefinition()` and `builderStateToGraph()` to include the field (omitted when default)
- Updated `definitionToBuilderState()` to read the field back

**`web/src/pages/actions/WorkflowBuilderPage.tsx`**
- Added `cancellationPolicy` to `INITIAL_STATE`
- Added a dropdown select in the metadata row for choosing the cancellation policy
- The setting persists into both the full definition and the raw YAML graph view

## Design Decisions

- **Default is `AllowFinish`**: This is the safest behavior — running tasks complete naturally, preventing data corruption from interrupted operations.
- **No migration needed**: The field is stored inside the `workflow_definition.definition` JSONB column. `#[serde(default)]` handles existing definitions that lack the field.
- **Policy inherited by nested workflows**: When a parent workflow is cancelled, its cancellation policy propagates to all descendant workflows rather than each resolving its own.
- **`skip_serializing_if`**: The default value is omitted from serialized JSON to keep stored definitions compact and backward-compatible.

## Testing

- 29/29 workflow parser tests pass (5 new)
- Zero warnings across entire Rust workspace
- Zero TypeScript errors