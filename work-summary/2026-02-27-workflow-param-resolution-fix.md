# Workflow Parameter Resolution Fix

**Date**: 2026-02-27
**Scope**: `crates/executor/src/scheduler.rs`

## Problem

Workflow executions triggered via the API failed to resolve `{{ parameters.X }}` template expressions in task inputs. Instead of substituting the actual parameter value, the literal string `"{{ parameters.n }}"` was passed to the child action, causing runtime errors like:

```
ValueError: invalid literal for int() with base 10: '{{ parameters.n }}'
```

## Root Cause

The execution scheduler's `process_workflow_execution` and `advance_workflow` methods extracted workflow parameters from the execution's `config` field using:

```rust
execution.config.as_ref()
    .and_then(|c| c.get("parameters").cloned())
    .unwrap_or(json!({}))
```

This only handled the **wrapped** format `{"parameters": {"n": 5}}`, which is how child task executions store their config. However, when a workflow is triggered manually via the API, the config is stored in **flat** format `{"n": 5}` — the API places `request.parameters` directly into the execution's `config` column without wrapping it.

Because `config.get("parameters")` returned `None` for the flat format, `workflow_params` was set to `{}` (empty). The `WorkflowContext` was then built with no parameters, so `{{ parameters.n }}` failed to resolve. The error was silently swallowed by the fallback in `dispatch_workflow_task`, which used the raw (unresolved) input when template rendering failed.

## Fix

Added an `extract_workflow_params` helper function that handles both config formats, matching the existing logic in the worker's `ActionExecutor::prepare_execution_context`:

1. If config contains a `"parameters"` key → use that value (wrapped format)
2. Otherwise, if config is a JSON object → use the entire object as parameters (flat format)
3. Otherwise → return empty object

Replaced both extraction sites in the scheduler (`process_workflow_execution` and `advance_workflow`) with calls to this helper.

## Files Changed

- **`crates/executor/src/scheduler.rs`**:
  - Added `extract_workflow_params()` helper function
  - Updated `process_workflow_execution()` to use the helper
  - Updated `advance_workflow()` to use the helper
  - Added 6 unit tests covering wrapped, flat, None, non-object, empty, and precedence cases

## Testing

- All 104 existing executor tests pass
- 6 new unit tests added and passing
- No new warnings introduced