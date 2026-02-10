# Quick Reference: Execution State Ownership

**Last Updated**: 2026-02-09

## Ownership Model at a Glance

```
┌──────────────────────────────────────────────────────────┐
│  EXECUTOR OWNS                │  WORKER OWNS             │
│  Requested                    │  Running                 │
│  Scheduling                   │  Completed               │
│  Scheduled                    │  Failed                  │
│  (+ pre-handoff Cancelled)    │  (+ post-handoff         │
│                               │     Cancelled/Timeout/   │
│                               │     Abandoned)           │
└───────────────────────────────┴──────────────────────────┘
            │                           │
            └─────── HANDOFF ──────────┘
        execution.scheduled PUBLISHED
```

## Who Updates the Database?

### Executor Updates (Pre-Handoff Only)
- ✅ Creates execution record
- ✅ Updates status: `Requested` → `Scheduling` → `Scheduled`
- ✅ Publishes `execution.scheduled` message **← HANDOFF POINT**
- ✅ Handles cancellations/failures BEFORE handoff (worker never notified)
- ❌ NEVER updates after `execution.scheduled` is published

### Worker Updates (Post-Handoff Only)
- ✅ Receives `execution.scheduled` message (takes ownership)
- ✅ Updates status: `Scheduled` → `Running`
- ✅ Updates status: `Running` → `Completed`/`Failed`/`Cancelled`/etc.
- ✅ Handles cancellations/failures AFTER handoff
- ✅ Updates result data
- ✅ Writes for every status change after receiving handoff

## Who Publishes Messages?

### Executor Publishes
- `enforcement.created` (from rules)
- `execution.requested` (to scheduler)
- `execution.scheduled` (to worker) **← HANDOFF MESSAGE - OWNERSHIP TRANSFER**

### Worker Publishes
- `execution.status_changed` (for each status change after handoff)
- `execution.completed` (when done)

### Executor Receives (But Doesn't Update DB Post-Handoff)
- `execution.status_changed` → triggers orchestration logic (read-only)
- `execution.completed` → releases queue slots

## Code Locations

### Executor Updates DB
```rust
// crates/executor/src/scheduler.rs
execution.status = ExecutionStatus::Scheduled;
ExecutionRepository::update(pool, execution.id, execution.into()).await?;
```

### Worker Updates DB
```rust
// crates/worker/src/executor.rs
self.update_execution_status(execution_id, ExecutionStatus::Running).await?;
// ...
ExecutionRepository::update(&self.pool, execution_id, input).await?;
```

### Executor Orchestrates (Read-Only)
```rust
// crates/executor/src/execution_manager.rs
async fn process_status_change(...) -> Result<()> {
    let execution = ExecutionRepository::find_by_id(pool, execution_id).await?;
    // NO UPDATE - just orchestration logic
    Self::handle_completion(pool, publisher, &execution).await?;
}
```

## Decision Tree: Should I Update the DB?

```
Are you in the Executor?
├─ Have you published execution.scheduled for this execution?
│  ├─ NO → Update DB (you own it)
│  │  └─ Includes: Requested/Scheduling/Scheduled/pre-handoff Cancelled
│  └─ YES → Don't update DB (worker owns it now)
│     └─ Just orchestrate (trigger workflows, etc)
│
Are you in the Worker?
├─ Have you received execution.scheduled for this execution?
│  ├─ YES → Update DB for ALL status changes (you own it)
│  │  └─ Includes: Running/Completed/Failed/post-handoff Cancelled/etc.
│  └─ NO → Don't touch this execution (doesn't exist for you yet)
```

## Common Patterns

### ✅ DO: Worker Updates After Handoff
```rust
// Worker receives execution.scheduled
self.update_execution_status(execution_id, ExecutionStatus::Running).await?;
self.publish_status_update(execution_id, ExecutionStatus::Running).await?;
```

### ✅ DO: Executor Orchestrates Without DB Write
```rust
// Executor receives execution.status_changed
let execution = ExecutionRepository::find_by_id(pool, execution_id).await?;
if status == ExecutionStatus::Completed {
    Self::trigger_child_executions(pool, publisher, &execution).await?;
}
```

### ❌ DON'T: Executor Updates After Handoff
```rust
// Executor receives execution.status_changed
execution.status = status;
ExecutionRepository::update(pool, execution.id, execution).await?; // ❌ WRONG!
```

### ❌ DON'T: Worker Updates Before Handoff
```rust
// Worker updates execution it hasn't received via execution.scheduled
ExecutionRepository::update(&self.pool, execution_id, input).await?; // ❌ WRONG!
```

### ✅ DO: Executor Handles Pre-Handoff Cancellation
```rust
// User cancels execution before it's scheduled to worker
// Execution is still in Requested/Scheduling state
execution.status = ExecutionStatus::Cancelled;
ExecutionRepository::update(pool, execution_id, execution).await?; // ✅ CORRECT!
// Worker never receives execution.scheduled, never knows execution existed
```

### ✅ DO: Worker Handles Post-Handoff Cancellation
```rust
// Worker received execution.scheduled, now owns execution
// User cancels execution while it's running
execution.status = ExecutionStatus::Cancelled;
ExecutionRepository::update(&self.pool, execution_id, execution).await?; // ✅ CORRECT!
self.publish_status_update(execution_id, ExecutionStatus::Cancelled).await?;
```

## Handoff Checklist

When an execution is scheduled:

**Executor Must**:
- [x] Update status to `Scheduled`
- [x] Write to database
- [x] Publish `execution.scheduled` message **← HANDOFF OCCURS HERE**
- [x] Stop updating this execution (ownership transferred)
- [x] Continue to handle orchestration (read-only)

**Worker Must**:
- [x] Receive `execution.scheduled` message **← OWNERSHIP RECEIVED**
- [x] Take ownership of execution state
- [x] Update DB for all future status changes
- [x] Handle any cancellations/failures after this point
- [x] Publish status notifications

**Important**: If execution is cancelled BEFORE executor publishes `execution.scheduled`, the executor updates status to `Cancelled` and worker never learns about it.

## Benefits Summary

| Aspect | Benefit |
|--------|---------|
| **Race Conditions** | Eliminated - only one owner per stage |
| **DB Writes** | Reduced by ~50% - no duplicates |
| **Code Clarity** | Clear boundaries - easy to reason about |
| **Message Traffic** | Reduced - no duplicate completions |
| **Idempotency** | Safe to receive duplicate messages |

## Troubleshooting

### Execution Stuck in "Scheduled"
**Problem**: Worker not updating status to Running  
**Check**: Was execution.scheduled published? Worker received it? Worker healthy?

### Workflow Children Not Triggering
**Problem**: Orchestration not running  
**Check**: Worker published execution.status_changed? Message queue healthy?

### Duplicate Status Updates
**Problem**: Both services updating DB  
**Check**: Executor should NOT update after publishing execution.scheduled

### Execution Cancelled But Status Not Updated
**Problem**: Cancellation not reflected in database  
**Check**: Was it cancelled before or after handoff?  
**Fix**: If before handoff → executor updates; if after handoff → worker updates

### Queue Warnings
**Problem**: Duplicate completion notifications  
**Check**: Only worker should publish execution.completed

## See Also

- **Full Architecture Doc**: `docs/ARCHITECTURE-execution-state-ownership.md`
- **Bug Fix Visualization**: `docs/BUGFIX-duplicate-completion-2026-02-09.md`
- **Work Summary**: `work-summary/2026-02-09-execution-state-ownership.md`
