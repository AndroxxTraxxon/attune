# Fix: with_items Race Condition Causing Duplicate Task Dispatches

**Date**: 2026-03-04
**Component**: Executor service (`crates/executor/src/scheduler.rs`)
**Issue**: Workflow tasks downstream of `with_items` tasks were being dispatched multiple times

## Problem

When a `with_items` task (e.g., `process_items` with `concurrency: 3`) had multiple items completing nearly simultaneously, the downstream successor task (e.g., `validate`) would be dispatched once per concurrently-completing item instead of once total.

**Root cause**: Workers update execution status in the database to `Completed` *before* publishing the `ExecutionCompleted` MQ message. The completion listener processes MQ messages sequentially, but by the time it processes item N's completion message, items N+1, N+2, etc. may already be marked `Completed` in the database. This means the `siblings_remaining` query (which checks DB status) returns 0 for multiple items, and each one falls through to transition evaluation and dispatches the successor task.

### Concrete Scenario

With `process_items` (5 items, `concurrency: 3`) → `validate`:

1. Items 3 and 4 finish on separate workers nearly simultaneously
2. Worker for item 3 updates DB: status = Completed, then publishes MQ message
3. Worker for item 4 updates DB: status = Completed, then publishes MQ message
4. Completion listener processes item 3's message:
   - `siblings_remaining` query: item 4 is already Completed in DB → **0 remaining**
   - Falls through → dispatches `validate` ✓
5. Completion listener processes item 4's message:
   - `siblings_remaining` query: all items completed → **0 remaining**
   - Falls through → dispatches `validate` **again** ✗

With `concurrency: 3` and tasks of equal duration, up to 3 items could complete simultaneously, causing the successor to be dispatched 3 times.

## Fix

Two-layer defense added to `advance_workflow()`:

### Layer 1: Persisted state check (with_items early return)

After the `siblings_remaining` check passes (all items done), but before evaluating transitions, the fix checks whether `task_name` is already present in the *persisted* `completed_tasks` or `failed_tasks` from the `workflow_execution` record. If so, a previous `advance_workflow` invocation already handled this task's final completion — return early.

This is efficient because it uses data already loaded at the top of the function.

### Layer 2: Already-dispatched DB check (all successor tasks)

Before dispatching ANY successor task, the fix queries the `execution` table for existing child executions with the same `workflow_execution` ID and `task_name`. If any exist, the successor has already been dispatched by a prior call — skip it.

This belt-and-suspenders guard catches edge cases regardless of how the race manifests, including scenarios where the persisted `completed_tasks` list hasn't been updated yet.

## Files Changed

- `crates/executor/src/scheduler.rs` — Added two guards in `advance_workflow()`:
  1. Lines ~1035-1066: Early return for with_items tasks already in persisted completed/failed lists
  2. Lines ~1220-1250: DB existence check before dispatching any successor task

## Testing

- All 601 unit tests pass across the workspace (0 failures, 8 intentionally ignored)
- Zero compiler warnings
- The fix is defensive and backward-compatible — no changes to data models, APIs, or MQ protocols