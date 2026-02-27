# `with_items` Concurrency Limiting Implementation

**Date**: 2026-02-27
**Scope**: `crates/executor/src/scheduler.rs`

## Problem

Workflow tasks with `with_items` and a `concurrency` limit dispatched all items simultaneously, ignoring the concurrency setting entirely. For example, a task with `concurrency: 3` and 20 items would dispatch all 20 at once instead of running at most 3 in parallel.

## Root Cause

The `dispatch_with_items_task` method iterated over all items in a single loop, creating a child execution and publishing it to the MQ for every item unconditionally. The `task_node.concurrency` value was logged but never used to gate dispatching.

## Solution

### Approach: DB-Based Sliding Window

All child execution records are created in the database up front (with fully-rendered inputs), but only the first `concurrency` items are published to the message queue. The remaining children stay at `Requested` status in the DB. As each item completes, `advance_workflow` queries for `Requested`-status siblings and publishes enough to refill the concurrency window.

This avoids the need for any auxiliary state in workflow variables — the database itself is the single source of truth for which items are pending vs in-flight.

### Initial Attempt: Workflow Variables (Abandoned)

The first implementation stored pending items as JSON metadata in `workflow_execution.variables` under `__pending_items__{task_name}`. This approach suffered from race conditions: when multiple items completed simultaneously, concurrent `advance_workflow` calls would read stale pending lists, pop the same item, and lose others. The result was that only the initial batch ever executed.

### Key Changes

#### 1. `dispatch_with_items_task` — Two-Phase Dispatch

- **Phase 1**: Creates ALL child execution records in the database. Each row has its input already rendered through the `WorkflowContext`, so no re-rendering is needed later.
- **Phase 2**: Publishes only the first `min(total, concurrency)` to the MQ via `publish_execution_requested`. The rest stay at `Requested` status.

#### 2. `publish_execution_requested` — New Helper

Publishes an `ExecutionRequested` MQ message for an existing execution row. Used both during initial dispatch (Phase 2) and when filling concurrency slots on completion.

#### 3. `publish_pending_with_items_children` — Fill Concurrency Slots

Replaces the old `dispatch_next_pending_with_items`. Queries the database for siblings at `Requested` status (ordered by `task_index`), limited to the number of free slots, and publishes them. No workflow variables involved — the DB query `status = 'requested'` is the authoritative source of undispatched items.

#### 4. `advance_workflow` — Concurrency-Aware Completion

The with_items completion branch now:
1. Counts **in-flight** siblings (`scheduling`, `scheduled`, `running` — NOT `requested`)
2. Reads the `concurrency` limit from the task graph
3. Calculates `free_slots = concurrency - in_flight`
4. Calls `publish_pending_with_items_children(free_slots)` to fill the window
5. Checks **all** non-terminal siblings (including `requested`) to decide whether to advance

## Concurrency Flow Example

For a task with 5 items and `concurrency: 3`:

```
Initial:  Create items 0-4 in DB; publish items 0, 1, 2 to MQ
          Items 3, 4 stay at Requested status in DB

Item 0 ✓: in_flight=2 (items 1,2), free_slots=1 → publish item 3
          siblings_remaining=3 (items 1,2,3,4 minus terminal) → return early

Item 1 ✓: in_flight=2 (items 2,3), free_slots=1 → publish item 4
          siblings_remaining=3 → return early

Item 2 ✓: in_flight=2 (items 3,4), free_slots=1 → no Requested items left
          siblings_remaining=2 → return early

Item 3 ✓: in_flight=1 (item 4), free_slots=2 → no Requested items left
          siblings_remaining=1 → return early

Item 4 ✓: in_flight=0, free_slots=3 → no Requested items left
          siblings_remaining=0 → advance workflow to successor tasks
```

## Race Condition Handling

When multiple items complete simultaneously, concurrent `advance_workflow` calls may both query `status = 'requested'` and find the same pending items. The worst case is a brief over-dispatch (the same execution published to MQ twice). The scheduler handles this gracefully — the second message finds the execution already at `Scheduled`/`Running` status. This is a benign, self-correcting race that never loses items.

## Files Changed

- **`crates/executor/src/scheduler.rs`**:
  - Rewrote `dispatch_with_items_task` with two-phase create-then-publish approach
  - Added `publish_execution_requested` helper for publishing existing execution rows
  - Added `publish_pending_with_items_children` for DB-query-based slot filling
  - Rewrote `advance_workflow` with_items branch with in-flight counting and slot calculation
  - Updated unit tests for the new approach

## Testing

- All 104 executor tests pass (102 + 2 ignored)
- 2 new unit tests for dispatch count and free slots calculations
- Clean workspace build with no new warnings