# Workflow Template Resolution Implementation

**Date**: 2026-02-27

## Problem

Workflow task parameters containing `{{ }}` template expressions were being passed to workers verbatim without resolution. For example, a workflow task with `seconds: "{{item}}"` would send the literal string `"{{item}}"` to `core.sleep`, which rejected it with `"ERROR: seconds must be a positive integer"`.

Three interconnected features were missing from the executor's workflow orchestration:

1. **Template resolution** — `{{ item }}`, `{{ parameters.x }}`, `{{ result().data.items }}`, etc. in task inputs were never rendered through the `WorkflowContext` before dispatching child executions.
2. **`with_items` expansion** — Tasks declaring `with_items: "{{ number_list }}"` were not expanded into multiple parallel child executions (one per item).
3. **`publish` variable processing** — Transition `publish` directives like `number_list: "{{ result().data.items }}"` were ignored, so variables never propagated between tasks.

A secondary issue was **type coercion**: `render_json` stringified all template results, so `"{{ item }}"` resolving to integer `5` became the string `"5"`, causing type validation failures in downstream actions.

## Root Cause

The `ExecutionScheduler::dispatch_workflow_task()` method passed `task_node.input` directly into the child execution's config without any template rendering. Neither `process_workflow_execution` (entry-point dispatch) nor `advance_workflow` (successor dispatch) constructed or used a `WorkflowContext`. The `publish` directives on transitions were completely ignored in `advance_workflow`.

## Changes

### `crates/executor/src/workflow/context.rs`

- **Function-call expressions**: Added support for `result()`, `result().path.to.field`, `succeeded()`, `failed()`, and `timed_out()` in the expression evaluator via `try_evaluate_function_call()`.
- **`TaskOutcome` enum**: New enum (`Succeeded`, `Failed`, `TimedOut`) to track the last completed task's status for function expressions.
- **`set_last_task_outcome()`**: Records the result and outcome of the most recently completed task.
- **Type-preserving `render_json`**: When a JSON string value is a pure template expression (the entire string is `{{ expr }}`), `render_json` now returns the raw `JsonValue` from the expression instead of stringifying it. Added `try_evaluate_pure_expression()` helper. This means `"{{ item }}"` resolving to `5` stays as integer `5`, not string `"5"`.
- **`rebuild()` constructor**: Reconstructs a `WorkflowContext` from persisted workflow state (stored variables, parameters, and completed task results). Used by the scheduler when advancing a workflow.
- **`export_variables()`**: Exports workflow variables as a JSON object for persisting back to the `workflow_execution.variables` column.
- **Updated `publish_from_result()`**: Uses type-preserving `render_json` for publish expressions so arrays/numbers/booleans retain their types.
- **18 unit tests**: All passing, including new tests for type preservation, `result()` function, `succeeded()`/`failed()`, publish with result function, rebuild, and the exact `with_items` integer scenario from the failing workflow.

### `crates/executor/src/scheduler.rs`

- **Template resolution in `dispatch_workflow_task()`**: Now accepts a `WorkflowContext` parameter and renders `task_node.input` through `wf_ctx.render_json()` before wrapping in the execution config.
- **Initial context in `process_workflow_execution()`**: Builds a `WorkflowContext` from the parent execution's parameters and workflow-level vars, passes it to entry-point task dispatch.
- **Context reconstruction in `advance_workflow()`**: Rebuilds the `WorkflowContext` from the `workflow_execution.variables` column plus results of all completed child executions. Sets `last_task_outcome` from the just-completed execution.
- **`publish` processing**: Iterates transition `publish` directives when a transition fires, evaluates expressions through the context, and persists updated variables back to the `workflow_execution` record.
- **`with_items` expansion**: New `dispatch_with_items_task()` method resolves the `with_items` expression to a JSON array, then creates one child execution per item with `item`/`index` set on the context. Each child gets `task_index` set in its `WorkflowTaskMetadata`.
- **`with_items` completion tracking**: In `advance_workflow()`, tasks with `task_index` (indicating `with_items`) are only marked completed/failed when ALL sibling items for that task name are done.

### `packs/examples/actions/list_example.sh` & `list_example.yaml`

- Rewrote shell script from `bash`+`jq` (unavailable in worker containers) to pure POSIX shell with DOTENV parameter parsing, matching the core pack pattern.
- Changed `parameter_format` from `json` to `dotenv`.

### `packs.external/python_example/actions/list_numbers.py` & `list_numbers.yaml`

- New action `python_example.list_numbers` that returns `{"items": list(range(start, n+start))}`.
- Parameters: `n` (default 10), `start` (default 0). JSON output format, Python ≥3.9.

## Workflow Flow (After Fix)

For the `examples.hello_workflow`:

```
1. generate_numbers task dispatched with rendered input {count: 5, n: 5}
2. python_example.list_numbers returns {items: [0, 1, 2, 3, 4]}
3. Transition publish: number_list = result().data.items → [0,1,2,3,4]
   Variables persisted to workflow_execution record
4. sleep_2 dispatched with with_items: "{{ number_list }}"
   → 5 child executions created, each with item/index context
   → seconds: "{{item}}" renders to 0, 1, 2, 3, 4 (integers, not strings)
5. All sleep items complete → task marked done → echo_3 dispatched
6. Workflow completes
```

## Testing

- All 96 executor unit tests pass (0 failures)
- All 18 workflow context tests pass (including 8 new tests)
- Full workspace compiles with no new warnings (30 pre-existing)