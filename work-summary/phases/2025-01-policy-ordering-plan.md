# Policy Execution Ordering Implementation Plan

**Date**: 2025-01-XX  
**Status**: Planning  
**Priority**: P0 - BLOCKING (Critical Correctness)

## Problem Statement

Currently, when execution policies (concurrency limits, delays) are enforced, there is **no guaranteed ordering** for which executions proceed when slots become available. This leads to:

1. **Fairness Violations**: Later requests can execute before earlier ones
2. **Non-deterministic Behavior**: Same workflow produces different orders across runs
3. **Workflow Dependencies Break**: Parent executions may proceed after children
4. **Poor User Experience**: Unpredictable queue behavior

### Current Flow (Broken)
```
Request A arrives → Policy blocks (concurrency=1, 1 running)
Request B arrives → Policy blocks (concurrency=1, 1 running)
Request C arrives → Policy blocks (concurrency=1, 1 running)
Running execution completes
→ A, B, or C might proceed (RANDOM, based on tokio scheduling)
```

### Desired Flow (FIFO)
```
Request A arrives → Enqueued at position 0
Request B arrives → Enqueued at position 1
Request C arrives → Enqueued at position 2
Running execution completes → Notify position 0 → A proceeds
A completes → Notify position 1 → B proceeds
B completes → Notify position 2 → C proceeds
```

## Architecture Design

### 1. ExecutionQueueManager

A new component that manages FIFO queues per action and provides slot-based synchronization.

**Key Features:**
- One queue per `action_id` (per-action concurrency control)
- FIFO ordering guarantee using `VecDeque`
- Tokio `Notify` for efficient async waiting
- Thread-safe with `Arc<Mutex<>>` or `DashMap`
- Queue statistics for monitoring

**Data Structures:**
```rust
struct QueueEntry {
    execution_id: i64,
    enqueued_at: DateTime<Utc>,
    notifier: Arc<Notify>,
}

struct ActionQueue {
    queue: VecDeque<QueueEntry>,
    active_count: u32,
    max_concurrent: u32,
}

struct ExecutionQueueManager {
    queues: DashMap<i64, ActionQueue>, // key: action_id
}
```

### 2. Integration Points

#### A. EnforcementProcessor
- **Before**: Directly creates execution and publishes to scheduler
- **After**: Calls `queue_manager.enqueue_and_wait()` before creating execution
- **Change**: Async wait until queue allows execution

#### B. PolicyEnforcer
- **Before**: `wait_for_policy_compliance()` polls every 1 second
- **After**: `enforce_and_wait()` combines policy check + queue wait
- **Change**: More efficient, guaranteed ordering

#### C. ExecutionScheduler
- **No Change**: Receives ExecutionRequested messages as before
- **Note**: Queue happens before scheduling, not during

#### D. Worker → Executor Completion
- **New**: Worker publishes `execution.completed` message
- **New**: Executor's CompletionListener consumes these messages
- **New**: CompletionListener calls `queue_manager.notify_completion(action_id)`

### 3. Message Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ EnforcementProcessor                                             │
│                                                                  │
│  1. Receive enforcement.created                                 │
│  2. queue_manager.enqueue_and_wait(action_id, execution_id)     │
│     ├─ Check policy compliance                                  │
│     ├─ Enqueue to action's FIFO queue                           │
│     ├─ Wait on notifier if queue full                           │
│     └─ Return when slot available                               │
│  3. Create execution record                                     │
│  4. Publish execution.requested                                 │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│ ExecutionScheduler                                              │
│                                                                  │
│  5. Receive execution.requested                                 │
│  6. Select worker                                               │
│  7. Publish to worker queue                                     │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│ Worker                                                          │
│                                                                  │
│  8. Execute action                                              │
│  9. Publish execution.completed (NEW)                           │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│ CompletionListener (NEW)                                        │
│                                                                  │
│ 10. Receive execution.completed                                 │
│ 11. queue_manager.notify_completion(action_id)                  │
│     └─ Notify next waiter in queue                              │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Create ExecutionQueueManager (2 days)

**Files to Create:**
- `crates/executor/src/queue_manager.rs`

**Implementation:**
```rust
pub struct ExecutionQueueManager {
    queues: DashMap<i64, Arc<Mutex<ActionQueue>>>,
}

impl ExecutionQueueManager {
    pub async fn enqueue_and_wait(
        &self,
        action_id: i64,
        execution_id: i64,
        max_concurrent: u32,
    ) -> Result<()>;
    
    pub async fn notify_completion(&self, action_id: i64) -> Result<()>;
    
    pub async fn get_queue_stats(&self, action_id: i64) -> QueueStats;
    
    pub async fn cancel_execution(&self, execution_id: i64) -> Result<()>;
}
```

**Tests:**
- FIFO ordering with 3 concurrent enqueues, limit=1
- 1000 concurrent enqueues maintain order
- Completion notification releases correct waiter
- Multiple actions have independent queues
- Cancel removes from queue correctly

### Step 2: Integrate with PolicyEnforcer (1 day)

**Files to Modify:**
- `crates/executor/src/policy_enforcer.rs`

**Changes:**
- Add `queue_manager: Arc<ExecutionQueueManager>` field
- Create `enforce_and_wait()` method that combines:
  1. Policy compliance check
  2. Queue enqueue and wait
- Keep existing `check_policies()` for validation

**Tests:**
- Policy violation prevents queue entry
- Policy pass allows queue entry
- Queue respects concurrency limits

### Step 3: Update EnforcementProcessor (1 day)

**Files to Modify:**
- `crates/executor/src/enforcement_processor.rs`

**Changes:**
- Add `queue_manager: Arc<ExecutionQueueManager>` field
- In `create_execution()`, before creating execution record:
  ```rust
  // Get action's concurrency limit from policy
  let concurrency_limit = policy_enforcer
      .get_concurrency_limit(rule.action)
      .unwrap_or(u32::MAX);
  
  // Wait for queue slot
  queue_manager
      .enqueue_and_wait(rule.action, enforcement.id, concurrency_limit)
      .await?;
  
  // Now create execution (we have a slot)
  let execution = ExecutionRepository::create(pool, execution_input).await?;
  ```

**Tests:**
- Three executions with limit=1 execute in FIFO order
- Queue blocks until slot available
- Execution created only after queue allows

### Step 4: Create CompletionListener (1 day)

**Files to Create:**
- `crates/executor/src/completion_listener.rs`

**Implementation:**
- New component that consumes `execution.completed` messages
- Calls `queue_manager.notify_completion(action_id)`
- Updates execution status in database (if needed)
- Publishes notifications

**Message Type:**
```rust
// In attune_common/mq/messages.rs
pub struct ExecutionCompletedPayload {
    pub execution_id: i64,
    pub action_id: i64,
    pub status: ExecutionStatus,
    pub result: Option<JsonValue>,
}
```

**Tests:**
- Completion message triggers queue notification
- Correct action_id used for notification
- Database status updated correctly

### Step 5: Update Worker to Publish Completions (0.5 day)

**Files to Modify:**
- `crates/worker/src/executor.rs`

**Changes:**
- After execution completes (success or failure), publish `execution.completed`
- Include action_id in message payload
- Use reliable publishing (ensure message is sent)

**Tests:**
- Worker publishes on success
- Worker publishes on failure
- Worker publishes on timeout
- Worker publishes on cancel

### Step 6: Add Queue Stats API Endpoint (0.5 day)

**Files to Modify:**
- `crates/api/src/routes/actions.rs`

**New Endpoint:**
```
GET /api/v1/actions/:ref/queue-stats

Response:
{
  "action_id": 123,
  "action_ref": "core.echo",
  "queue_length": 5,
  "active_count": 2,
  "max_concurrent": 3,
  "oldest_enqueued_at": "2025-01-15T10:30:00Z"
}
```

**Tests:**
- Endpoint returns correct stats
- Queue stats update in real-time
- Non-existent action returns 404

### Step 7: Integration Testing (1 day)

**Test Scenarios:**
1. **FIFO Ordering**: 10 executions, limit=1, verify order
2. **Concurrent Actions**: Multiple actions don't interfere
3. **High Concurrency**: 1000 simultaneous enqueues
4. **Completion Handling**: Verify queue progresses on completion
5. **Failure Scenarios**: Worker crash, timeout, cancel
6. **Policy Integration**: Rate limit + queue interaction
7. **API Stats**: Verify queue stats are accurate

**Files:**
- `crates/executor/tests/queue_ordering_test.rs`
- `crates/executor/tests/queue_stress_test.rs`

### Step 8: Documentation (0.5 day)

**Files to Create/Update:**
- `docs/queue-architecture.md` - Queue design and behavior
- `docs/api-actions.md` - Add queue-stats endpoint
- `README.md` - Mention queue ordering guarantees

**Content:**
- How queues work per action
- FIFO guarantees
- Monitoring queue stats
- Performance characteristics
- Troubleshooting queue issues

## API Changes

### New Endpoint
- `GET /api/v1/actions/:ref/queue-stats` - View queue statistics

### Message Types
- `execution.completed` (new) - Worker notifies completion

## Database Changes

**None required** - All queue state is in-memory

## Configuration

Add to `ExecutorConfig`:
```yaml
executor:
  queue:
    max_queue_length: 10000  # Per-action queue limit
    queue_timeout_seconds: 3600  # Max time in queue
    enable_queue_metrics: true
```

## Performance Considerations

1. **Memory Usage**: O(n) per queued execution
   - Mitigation: `max_queue_length` config
   - Typical: 100-1000 queued per action

2. **Lock Contention**: DashMap per action reduces contention
   - Each action has independent lock
   - Notify uses efficient futex-based waiting

3. **Message Overhead**: One additional message per execution
   - `execution.completed` is lightweight
   - Published async, no blocking

## Testing Strategy

### Unit Tests
- QueueManager FIFO behavior
- Notify mechanism correctness
- Queue stats accuracy
- Cancellation handling

### Integration Tests
- End-to-end execution ordering
- Multiple workers, one action
- Concurrent actions independent
- Stress test: 1000 concurrent enqueues

### Performance Tests
- Throughput with queuing enabled
- Latency impact of queuing
- Memory usage under load

## Migration & Rollout

### Phase 1: Deploy with Queue Disabled (Default)
- Deploy code with queue feature
- Queue disabled by default (concurrency_limit = None)
- Monitor for issues

### Phase 2: Enable for Select Actions
- Enable queue for specific high-concurrency actions
- Monitor ordering and performance
- Gather metrics

### Phase 3: Enable Globally
- Set default concurrency limits
- Enable queue for all actions
- Document behavior change

## Success Criteria

- [ ] All tests pass (unit, integration, performance)
- [ ] FIFO ordering guaranteed for same action
- [ ] Completion notification releases queue slot
- [ ] Queue stats API endpoint works
- [ ] Documentation complete
- [ ] No performance regression (< 5% latency increase)
- [ ] Zero race conditions under stress test

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Memory exhaustion | HIGH | max_queue_length config |
| Deadlock in notify | CRITICAL | Timeout on queue wait |
| Worker crash loses completion | MEDIUM | Executor timeout cleanup |
| Race in queue state | HIGH | Careful lock ordering |
| Performance regression | MEDIUM | Benchmark before/after |

## Timeline

- **Total Estimate**: 6-7 days
- **Step 1 (QueueManager)**: 2 days
- **Step 2 (PolicyEnforcer)**: 1 day
- **Step 3 (EnforcementProcessor)**: 1 day
- **Step 4 (CompletionListener)**: 1 day
- **Step 5 (Worker updates)**: 0.5 day
- **Step 6 (API endpoint)**: 0.5 day
- **Step 7 (Integration tests)**: 1 day
- **Step 8 (Documentation)**: 0.5 day

## Next Steps

1. Review plan with team
2. Create `queue_manager.rs` with core data structures
3. Implement `enqueue_and_wait()` with tests
4. Integrate with policy enforcer
5. Continue with remaining steps

---

**Related Documents:**
- `work-summary/TODO.md` - Phase 0.1 task list
- `docs/architecture.md` - Overall system architecture
- `crates/executor/src/policy_enforcer.rs` - Current policy implementation