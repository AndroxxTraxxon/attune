# Workflow Execution Orchestration & UI Ref-Lock Fix

**Date**: 2026-02-27

## Problem

Two issues were addressed:

### 1. Workflow ref editable during edit mode (UI)
When editing an existing workflow action, the pack selector and workflow name fields were editable, allowing users to change the action's ref — which should be immutable after creation.

### 2. Workflow execution runtime error
Executing a workflow action produced:
```
Action execution failed: Internal error: Runtime not found: No runtime found for action: examples.single_echo (available: node.js, python, shell)
```

**Root cause**: Workflow companion actions are created with `runtime: None` (they aren't scripts — they're orchestration definitions). When the executor's scheduler received an execution request for a workflow action, it dispatched it to a worker like any regular action. The worker then tried to find a runtime to execute it, failed (no runtime matches a `.workflow.yaml` entrypoint), and returned the error.

The `WorkflowCoordinator` in `crates/executor/src/workflow/coordinator.rs` existed as prototype code but was never integrated into the execution pipeline.

## Solution

### UI Fix (`web/src/pages/actions/WorkflowBuilderPage.tsx`)
- Added `disabled={isEditing}` to the `SearchableSelect` pack selector (already supported a `disabled` prop)
- Added `disabled={isEditing}` and conditional disabled styling to the workflow name `<input>`
- Both fields are now locked when editing an existing workflow, preventing ref changes

### Workflow Orchestration (`crates/executor/src/scheduler.rs`)
Added workflow detection and orchestration directly in the `ExecutionScheduler`:

1. **Detection**: `process_execution_requested` checks `action.workflow_def.is_some()` before dispatching to a worker
2. **`process_workflow_execution`**: Loads the workflow definition, parses it into a `WorkflowDefinition`, builds a `TaskGraph`, creates a `workflow_execution` record, and marks the parent execution as Running
3. **`dispatch_workflow_task`**: For each entry-point task in the graph, creates a child execution with the task's actual action ref (e.g., `core.echo` instead of `examples.single_echo`) and publishes an `ExecutionRequested` message. The child execution includes `workflow_task` metadata linking it back to the `workflow_execution` record.
4. **`advance_workflow`** (public): Called by the completion listener when a workflow child task completes. Evaluates transitions from the completed task, schedules successor tasks, checks join barriers, and completes the workflow when all tasks are done.
5. **`complete_workflow`**: Updates both the `workflow_execution` and parent `execution` records to their terminal state.

Key design decisions:
- Child task executions re-enter the normal scheduling pipeline via MQ, so nested workflows (a workflow task that is itself a workflow) are handled recursively
- Transition evaluation supports `succeeded()`, `failed()`, `timed_out()`, `always`, and custom conditions (custom defaults to fire-on-success for now)
- Join barriers are respected — tasks with `join` counts wait for enough predecessors

### Completion Listener (`crates/executor/src/completion_listener.rs`)
- Added workflow advancement: when a completed execution has `workflow_task` metadata, calls `ExecutionScheduler::advance_workflow` to schedule successor tasks or complete the workflow
- Added an `AtomicUsize` round-robin counter for dispatching successor tasks to workers

### Binary Entry Point (`crates/executor/src/main.rs`)
- Added `mod workflow;` so the binary crate can resolve `crate::workflow::graph::*` paths used in the scheduler

## Files Changed

| File | Change |
|------|--------|
| `web/src/pages/actions/WorkflowBuilderPage.tsx` | Disable pack selector and name input when editing |
| `crates/executor/src/scheduler.rs` | Workflow detection, orchestration, task dispatch, advancement |
| `crates/executor/src/completion_listener.rs` | Workflow advancement on child task completion |
| `crates/executor/src/main.rs` | Added `mod workflow;` |

## Architecture Note

This implementation bypasses the prototype `WorkflowCoordinator` (`crates/executor/src/workflow/coordinator.rs`) which had several issues: hardcoded `attune.` schema prefixes, `SELECT *` on the execution table, duplicate parent execution creation, and no integration with the MQ-based scheduling pipeline. The new implementation works directly within the scheduler and completion listener, using the existing repository layer and message queue infrastructure.

## Testing

- Existing executor unit tests pass
- Workspace compiles with zero errors
- No new warnings introduced (pre-existing warnings from unused prototype workflow code remain)