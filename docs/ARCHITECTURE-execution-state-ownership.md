# Execution State Ownership Model

**Date**: 2026-02-09  
**Status**: Implemented  
**Related Issues**: Duplicate completion notifications, unnecessary database updates

## Overview

This document defines the **ownership model** for execution state management in Attune. It clarifies which service is responsible for updating execution records at each stage of the lifecycle, eliminating race conditions and redundant database writes.

## The Problem

Prior to this change, both the executor and worker were updating execution state in the database, causing:

1. **Race conditions** - unclear which service's update would happen first
2. **Redundant writes** - both services writing the same status value
3. **Architectural confusion** - no clear ownership boundaries
4. **Warning logs** - duplicate completion notifications

## The Solution: Lifecycle-Based Ownership

Execution state ownership is divided based on **lifecycle stage**, with a clear handoff point:

```
┌─────────────────────────────────────────────────────────────────┐
│                      EXECUTOR OWNERSHIP                         │
│                                                                 │
│  Requested → Scheduling → Scheduled                             │
│                                    │                            │
│  (includes cancellations/failures  │                            │
│   before execution.scheduled       │                            │
│   message is published)            │                            │
│                                    │                            │
│                          Handoff Point:                         │
│                          execution.scheduled message PUBLISHED  │
│                                    ▼                            │
└─────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Worker receives message
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       WORKER OWNERSHIP                          │
│                                                                 │
│  Running → Completed / Failed / Cancelled / Timeout            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Executor Responsibilities

The **Executor Service** owns execution state from creation through scheduling:

- ✅ Creates execution records (`Requested`)
- ✅ Updates status during scheduling (`Scheduling`)
- ✅ Updates status when scheduled to worker (`Scheduled`)
- ✅ Publishes `execution.scheduled` message **← HANDOFF POINT**
- ✅ Handles cancellations/failures BEFORE `execution.scheduled` is published
- ❌ Does NOT update status after `execution.scheduled` is published

**Lifecycle stages**: `Requested` → `Scheduling` → `Scheduled`

**Important**: If an execution is cancelled or fails before the executor publishes `execution.scheduled`, the executor is responsible for updating the status (e.g., to `Cancelled`). The worker never learns about executions that don't reach the handoff point.

### Worker Responsibilities

The **Worker Service** owns execution state after receiving the handoff:

- ✅ Receives `execution.scheduled` message **← TAKES OWNERSHIP**
- ✅ Updates status when execution starts (`Running`)
- ✅ Updates status when execution completes (`Completed`, `Failed`, etc.)
- ✅ Handles cancellations AFTER receiving `execution.scheduled`
- ✅ Updates execution result data
- ✅ Publishes `execution.status_changed` notifications
- ✅ Publishes `execution.completed` notifications
- ❌ Does NOT update status for executions it hasn't received

**Lifecycle stages**: `Running` → `Completed` / `Failed` / `Cancelled` / `Timeout`

**Important**: The worker only owns executions it has received via `execution.scheduled`. If a cancellation happens before this message is sent, the worker is never involved.

## Message Flow

### 1. Executor Creates and Schedules

```
Executor Service
  ├─> Creates execution (status: Requested)
  ├─> Updates status: Scheduling
  ├─> Selects worker
  ├─> Updates status: Scheduled
  └─> Publishes: execution.scheduled → worker-specific queue
```

### 2. Worker Receives and Executes

```
Worker Service
  ├─> Receives: execution.scheduled
  ├─> Updates DB: Scheduled → Running
  ├─> Publishes: execution.status_changed (running)
  ├─> Executes action
  ├─> Updates DB: Running → Completed/Failed
  ├─> Publishes: execution.status_changed (completed/failed)
  └─> Publishes: execution.completed
```

### 3. Executor Handles Orchestration

```
Executor Service (ExecutionManager)
  ├─> Receives: execution.status_changed
  ├─> Does NOT update database
  ├─> Handles orchestration logic:
  │   ├─> Triggers workflow children (if parent completed)
  │   ├─> Updates workflow state
  │   └─> Manages parent-child relationships
  └─> Logs event for monitoring
```

### 4. Queue Management

```
Executor Service (CompletionListener)
  ├─> Receives: execution.completed
  ├─> Releases queue slot
  ├─> Notifies waiting executions
  └─> Updates queue statistics
```

## Database Update Rules

### Executor (Pre-Scheduling)

**File**: `crates/executor/src/scheduler.rs`

```rust
// ✅ Executor updates DB before scheduling
execution.status = ExecutionStatus::Scheduled;
ExecutionRepository::update(pool, execution.id, execution.into()).await?;

// Publish to worker
Self::queue_to_worker(...).await?;
```

### Worker (Post-Scheduling)

**File**: `crates/worker/src/executor.rs`

```rust
// ✅ Worker updates DB when starting
async fn execute(&self, execution_id: i64) -> Result<ExecutionResult> {
    // Update status to running
    self.update_execution_status(execution_id, ExecutionStatus::Running).await?;
    
    // Execute action...
}

// ✅ Worker updates DB when completing
async fn handle_execution_success(&self, execution_id: i64, result: &ExecutionResult) -> Result<()> {
    let input = UpdateExecutionInput {
        status: Some(ExecutionStatus::Completed),
        result: Some(result_data),
        // ...
    };
    ExecutionRepository::update(&self.pool, execution_id, input).await?;
}
```

### Executor (Post-Scheduling)

**File**: `crates/executor/src/execution_manager.rs`

```rust
// ❌ Executor does NOT update DB after scheduling
async fn process_status_change(...) -> Result<()> {
    // Fetch execution (for orchestration logic only)
    let execution = ExecutionRepository::find_by_id(pool, execution_id).await?;
    
    // Handle orchestration, but do NOT update DB
    match status {
        ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Cancelled => {
            Self::handle_completion(pool, publisher, &execution).await?;
        }
        _ => {}
    }
    
    Ok(())
}
```

## Benefits

### 1. Clear Ownership Boundaries

- No ambiguity about who updates what
- Easy to reason about system behavior
- Reduced cognitive load for developers

### 2. Eliminated Race Conditions

- Only one service updates each lifecycle stage
- No competing writes to same fields
- Predictable state transitions

### 3. Better Performance

- No redundant database writes
- Reduced database contention
- Lower network overhead (fewer queries)

### 4. Cleaner Logs

Before:
```
executor | Updated execution 9061 status: Scheduled -> Running
executor | Updated execution 9061 status: Running -> Running
executor | Updated execution 9061 status: Completed -> Completed
executor | WARN: Completion notification for action 3 but active_count is 0
```

After:
```
executor | Execution 9061 scheduled to worker 29
worker   | Starting execution: 9061
worker   | Execution 9061 completed successfully in 142ms
executor | Execution 9061 reached terminal state: Completed, handling orchestration
```

### 5. Idempotent Message Handling

- Executor can safely receive duplicate status change messages
- Worker updates are authoritative
- No special logic needed for retries

## Edge Cases & Error Handling

### Cancellation Before Handoff

**Scenario**: Execution is queued due to concurrency policy, user cancels before scheduling.

**Handling**:
- Execution in `Requested` or `Scheduling` state
- Executor updates status: → `Cancelled`
- Worker never receives `execution.scheduled`
- No worker resources consumed ✅

### Cancellation After Handoff

**Scenario**: Execution already scheduled to worker, user cancels while running.

**Handling**:
- Worker has received `execution.scheduled` and owns execution
- Worker updates status: `Running` → `Cancelled`
- Worker publishes status change notification
- Executor handles orchestration (e.g., skip workflow children)

### Worker Crashes Before Updating Status

**Scenario**: Worker receives `execution.scheduled` but crashes before updating status to `Running`.

**Handling**:
- Execution remains in `Scheduled` state
- Worker owned the execution but failed to update
- Executor's heartbeat monitoring detects stale scheduled executions
- After timeout, executor can reschedule to another worker or mark as abandoned
- Idempotent: If worker already started, duplicate scheduling is rejected

### Message Delivery Delays

**Scenario**: Worker updates DB but `execution.status_changed` message is delayed.

**Handling**:
- Database reflects correct state (source of truth)
- Executor eventually receives notification and handles orchestration
- Orchestration logic is idempotent (safe to call multiple times)
- Critical: Workflows may have slight delay, but remain consistent

### Partial Failures

**Scenario**: Worker updates DB successfully but fails to publish notification.

**Handling**:
- Database has correct state (worker succeeded)
- Executor won't trigger orchestration until notification arrives
- Future enhancement: Periodic executor polling for stale completions
- Workaround: Worker retries message publishing with exponential backoff

## Migration Notes

### Changes Required

1. **Executor Service** (`execution_manager.rs`):
   - ✅ Removed database updates from `process_status_change()`
   - ✅ Changed to read-only orchestration handler
   - ✅ Updated logs to reflect observer role

2. **Worker Service** (`service.rs`):
   - ✅ Already updates DB directly (no changes needed)
   - ✅ Updated comment: "we'll update the database directly"

3. **Documentation**:
   - ✅ Updated module docs to reflect ownership model
   - ✅ Added ownership boundaries to architecture docs

### Backward Compatibility

- ✅ No breaking changes to external APIs
- ✅ Message formats unchanged
- ✅ Database schema unchanged
- ✅ Workflow behavior unchanged

## Testing Strategy

### Unit Tests

- ✅ Executor tests verify no DB updates after scheduling
- ✅ Worker tests verify DB updates at all lifecycle stages
- ✅ Message handler tests verify orchestration without DB writes

### Integration Tests

- Test full execution lifecycle end-to-end
- Verify status transitions in database
- Confirm orchestration logic (workflow children) still works
- Test failure scenarios (worker crashes, message delays)

### Monitoring

Monitor for:
- Executions stuck in `Scheduled` state (worker not picking up)
- Large delays between status changes (message queue lag)
- Workflow children not triggering (orchestration failure)

## Future Enhancements

### 1. Executor Polling for Stale Completions

If `execution.status_changed` messages are lost, executor could periodically poll for completed executions that haven't triggered orchestration.

### 2. Worker Health Checks

More robust detection of worker failures before scheduled executions time out.

### 3. Explicit Handoff Messages

Consider adding `execution.handoff` message to explicitly mark ownership transfer point.

## References

- **Architecture Doc**: `docs/architecture/executor-service.md`
- **Work Summary**: `work-summary/2026-02-09-duplicate-completion-fix.md`
- **Bug Fix Doc**: `docs/BUGFIX-duplicate-completion-2026-02-09.md`
- **ExecutionManager**: `crates/executor/src/execution_manager.rs`
- **Worker Executor**: `crates/worker/src/executor.rs`
- **Worker Service**: `crates/worker/src/service.rs`

## Summary

The execution state ownership model provides **clear, lifecycle-based boundaries** for who updates execution records:

- **Executor**: Owns state from creation through scheduling (including pre-handoff cancellations)
- **Worker**: Owns state after receiving `execution.scheduled` message
- **Handoff**: Occurs when `execution.scheduled` message is **published to worker**
- **Key Principle**: Worker only knows about executions it receives; pre-handoff cancellations are executor's responsibility

This eliminates race conditions, reduces database load, and provides a clean architectural foundation for future enhancements.