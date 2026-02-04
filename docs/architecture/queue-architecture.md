# Queue Architecture and FIFO Execution Ordering

**Status**: Production Ready (v0.1)  
**Last Updated**: 2025-01-27

---

## Overview

Attune implements a **per-action FIFO queue system** to guarantee deterministic execution ordering when policy limits (concurrency, delays) are enforced. This ensures fairness, predictability, and correct workflow execution.

### Why Queue Ordering Matters

**Problem**: Without ordered queuing, when multiple executions are blocked by policies, they proceed in **random order** based on tokio's task scheduling. This causes:

- ❌ **Fairness Violations**: Later requests execute before earlier ones
- ❌ **Non-determinism**: Same workflow produces different orders across runs
- ❌ **Broken Dependencies**: Parent executions may proceed after children
- ❌ **Poor UX**: Unpredictable queue behavior frustrates users

**Solution**: FIFO queues with async notification ensure executions proceed in strict request order.

---

## Architecture Components

### 1. ExecutionQueueManager

**Location**: `crates/executor/src/queue_manager.rs`

The central component managing all execution queues.

```rust
pub struct ExecutionQueueManager {
    queues: DashMap<i64, Arc<Mutex<ActionQueue>>>,  // Key: action_id
    config: QueueConfig,
    db_pool: Option<PgPool>,
}
```

**Key Features**:
- **One queue per action**: Isolated FIFO queues prevent cross-action interference
- **Thread-safe**: Uses `DashMap` for lock-free map access
- **Async-friendly**: Uses `tokio::Notify` for efficient waiting
- **Observable**: Tracks statistics for monitoring

### 2. ActionQueue

Per-action queue structure with FIFO ordering guarantees.

```rust
struct ActionQueue {
    queue: VecDeque<QueueEntry>,  // FIFO queue
    active_count: u32,             // Currently running
    max_concurrent: u32,           // Policy limit
    total_enqueued: u64,           // Lifetime counter
    total_completed: u64,          // Lifetime counter
}
```

### 3. QueueEntry

Individual execution waiting in queue.

```rust
struct QueueEntry {
    execution_id: i64,
    enqueued_at: DateTime<Utc>,
    notifier: Arc<Notify>,  // Async notification
}
```

**Notification Mechanism**:
- Each queued execution gets a `tokio::Notify` handle
- Worker completion triggers `notify.notify_one()` on next waiter
- No polling required - efficient async waiting

---

## Execution Flow

### Normal Flow (With Capacity)

```
┌─────────────────────────────────────────────────────────────┐
│ 1. EnforcementProcessor receives enforcement.created       │
│    └─ Enforcement: rule fired, needs execution             │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. PolicyEnforcer.check_policies(action_id)                │
│    └─ Verify rate limits, quotas                           │
│    └─ Return: None (no violation)                          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. QueueManager.enqueue_and_wait(action_id, exec_id, limit)│
│    └─ Check: active_count < max_concurrent?                │
│    └─ YES: Increment active_count                          │
│    └─ Return immediately (no waiting)                      │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. Create Execution record in database                     │
│    └─ Status: REQUESTED                                    │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. Publish execution.requested to scheduler                │
│    └─ Scheduler selects worker and forwards                │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 6. Worker executes action                                  │
│    └─ Status: RUNNING → SUCCEEDED/FAILED                   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 7. Worker publishes execution.completed                    │
│    └─ Payload: { execution_id, action_id, status, result } │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 8. CompletionListener receives message                     │
│    └─ QueueManager.notify_completion(action_id)            │
│    └─ Decrement active_count                               │
│    └─ Notify next waiter in queue (if any)                 │
└─────────────────────────────────────────────────────────────┘
```

### Queued Flow (At Capacity)

```
┌─────────────────────────────────────────────────────────────┐
│ 1-2. Same as normal flow (enforcement, policy check)       │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. QueueManager.enqueue_and_wait(action_id, exec_id, limit)│
│    └─ Check: active_count < max_concurrent?                │
│    └─ NO: Queue is at capacity                             │
│    └─ Create QueueEntry with Notify handle                 │
│    └─ Push to VecDeque (FIFO position)                     │
│    └─ await notifier.notified()  ← BLOCKS HERE             │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ (waits for notification)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ WORKER COMPLETES EARLIER EXECUTION                          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ CompletionListener.notify_completion(action_id)            │
│    └─ Lock queue                                            │
│    └─ Pop front QueueEntry (FIFO!)                          │
│    └─ Decrement active_count (was N)                       │
│    └─ entry.notifier.notify_one()  ← WAKES WAITER          │
│    └─ Increment active_count (back to N)                   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. (continued) enqueue_and_wait() resumes                  │
│    └─ Return Ok(()) - slot acquired                        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 4-8. Same as normal flow (create execution, execute, etc.) │
└─────────────────────────────────────────────────────────────┘
```

---

## FIFO Guarantee

### How FIFO is Maintained

1. **Single Queue per Action**: Each action has independent `VecDeque<QueueEntry>`
2. **Push Back, Pop Front**: New entries added to back, next waiter from front
3. **Locked Mutations**: All queue operations protected by `Mutex`
4. **No Reordering**: No priority, no jumping - strict first-in-first-out

### Example Scenario

```
Action: core.http.get (max_concurrent = 2)

T=0: Exec A arrives → active_count=0 → proceeds immediately (active=1)
T=1: Exec B arrives → active_count=1 → proceeds immediately (active=2)
T=2: Exec C arrives → active_count=2 → QUEUED at position 0
T=3: Exec D arrives → active_count=2 → QUEUED at position 1
T=4: Exec E arrives → active_count=2 → QUEUED at position 2

Queue state: [C, D, E]

T=5: A completes → pop C from front → C proceeds (active=2, queue=[D, E])
T=6: B completes → pop D from front → D proceeds (active=2, queue=[E])
T=7: C completes → pop E from front → E proceeds (active=2, queue=[])
T=8: D completes → (queue empty, active=1)
T=9: E completes → (queue empty, active=0)

Result: Executions proceeded in exact order: A, B, C, D, E ✅
```

---

## Queue Statistics

### Data Model

```rust
pub struct QueueStats {
    pub action_id: i64,
    pub queue_length: usize,        // Waiting count
    pub active_count: u32,          // Running count
    pub max_concurrent: u32,        // Policy limit
    pub oldest_enqueued_at: Option<DateTime<Utc>>,
    pub total_enqueued: u64,        // Lifetime counter
    pub total_completed: u64,       // Lifetime counter
}
```

### Persistence

Queue statistics are persisted to the `attune.queue_stats` table for:
- **API visibility**: Real-time queue monitoring
- **Historical tracking**: Execution patterns over time
- **Alerting**: Detect stuck or growing queues

**Update Frequency**: On every queue state change (enqueue, dequeue, complete)

### Accessing Stats

**In-Memory** (Executor service):
```rust
let stats = queue_manager.get_queue_stats(action_id).await;
```

**Database** (Any service):
```rust
let stats = QueueStatsRepository::find_by_action(pool, action_id).await?;
```

**API Endpoint**:
```bash
GET /api/v1/actions/core.http.get/queue-stats
```

---

## Configuration

### Executor Configuration

```yaml
executor:
  queue:
    # Maximum executions per queue (prevents memory exhaustion)
    max_queue_length: 10000
    
    # Maximum time an execution can wait in queue (seconds)
    queue_timeout_seconds: 3600
    
    # Enable/disable queue metrics persistence
    enable_metrics: true
```

### Environment Variables

```bash
# Override via environment
export ATTUNE__EXECUTOR__QUEUE__MAX_QUEUE_LENGTH=5000
export ATTUNE__EXECUTOR__QUEUE__QUEUE_TIMEOUT_SECONDS=1800
```

---

## Performance Characteristics

### Memory Usage

**Per Queue**: ~128 bytes (DashMap entry + Arc + Mutex overhead)  
**Per Queued Execution**: ~80 bytes (QueueEntry + Arc<Notify>)

**Example**: 100 actions with 50 queued executions each:
- Queue overhead: 100 × 128 bytes = ~12 KB
- Entry overhead: 5000 × 80 bytes = ~400 KB
- **Total**: ~412 KB (negligible)

### Latency

- **Enqueue (with capacity)**: < 1 μs (just increment counter)
- **Enqueue (at capacity)**: O(1) to queue, then async wait
- **Dequeue (notify)**: < 10 μs (pop + notify)
- **Stats lookup**: < 1 μs (DashMap read)

### Throughput

**Measured Performance** (from stress tests):
- 1,000 executions (concurrency=5): **~200 exec/sec**
- 10,000 executions (concurrency=10): **~500 exec/sec**

**Bottleneck**: Database writes and worker execution time, not queue overhead

---

## Monitoring and Observability

### Health Indicators

**Healthy Queue**:
- ✅ `queue_length` is 0 or low (< 10% of max)
- ✅ `active_count` ≈ `max_concurrent` during load
- ✅ `oldest_enqueued_at` is recent (< 5 minutes)
- ✅ `total_completed` increases steadily

**Unhealthy Queue**:
- ⚠️ `queue_length` consistently high (> 50% of max)
- ⚠️ `oldest_enqueued_at` is old (> 30 minutes)
- 🚨 `queue_length` approaches `max_queue_length`
- 🚨 `active_count` < `max_concurrent` (workers stuck)

### Monitoring Queries

**Active queues**:
```sql
SELECT action_id, queue_length, active_count, max_concurrent,
       oldest_enqueued_at, last_updated
FROM attune.queue_stats
WHERE queue_length > 0 OR active_count > 0
ORDER BY queue_length DESC;
```

**Stuck queues** (not progressing):
```sql
SELECT a.ref, qs.queue_length, qs.active_count,
       qs.oldest_enqueued_at,
       NOW() - qs.last_updated AS stale_duration
FROM attune.queue_stats qs
JOIN attune.action a ON a.id = qs.action_id
WHERE (queue_length > 0 OR active_count > 0)
  AND last_updated < NOW() - INTERVAL '10 minutes';
```

**Queue throughput**:
```sql
SELECT a.ref, qs.total_completed, qs.total_enqueued,
       qs.total_completed::float / NULLIF(qs.total_enqueued, 0) * 100 AS completion_rate
FROM attune.queue_stats qs
JOIN attune.action a ON a.id = qs.action_id
WHERE total_enqueued > 0
ORDER BY total_enqueued DESC;
```

---

## Troubleshooting

### Queue Not Progressing

**Symptom**: `queue_length` stays constant, executions don't proceed

**Possible Causes**:
1. **Workers not completing**: Check worker logs for crashes/hangs
2. **Completion messages not publishing**: Check worker MQ connection
3. **CompletionListener not running**: Check executor service logs
4. **Database deadlock**: Check PostgreSQL logs

**Diagnosis**:
```bash
# Check active executions for this action
psql -c "SELECT id, status, created FROM attune.execution 
         WHERE action = <action_id> AND status IN ('running', 'requested')
         ORDER BY created DESC LIMIT 10;"

# Check worker logs
tail -f /var/log/attune/worker.log | grep "execution_id"

# Check completion messages
rabbitmqctl list_queues name messages
```

### Queue Full Errors

**Symptom**: `Error: Queue full (max length: 10000)`

**Causes**:
- Action is overwhelmed with requests
- Workers are too slow or stuck
- `max_queue_length` is too low

**Solutions**:
1. **Increase limit** (short-term):
   ```yaml
   executor:
     queue:
       max_queue_length: 20000
   ```

2. **Add more workers** (medium-term):
   - Scale worker service horizontally
   - Increase worker concurrency

3. **Increase concurrency limit** (if safe):
   - Adjust action-specific policy
   - Higher `max_concurrent` = more parallel executions

4. **Rate limit at API** (long-term):
   - Add API-level rate limiting
   - Reject requests before they enter system

### Memory Exhaustion

**Symptom**: Executor OOM killed, high memory usage

**Causes**:
- Too many queues with large queue lengths
- Memory leak in queue entries

**Diagnosis**:
```bash
# Check queue stats in database
psql -c "SELECT SUM(queue_length) as total_queued, 
                COUNT(*) as num_actions,
                MAX(queue_length) as max_queue
         FROM attune.queue_stats;"

# Monitor executor memory
ps aux | grep attune-executor
```

**Solutions**:
- Reduce `max_queue_length`
- Clear old queues: `queue_manager.clear_all_queues()`
- Restart executor service (queues rebuild from DB)

### FIFO Violation (Critical Bug)

**Symptom**: Executions complete out of order

**This should NEVER happen** - indicates a critical bug.

**Diagnosis**:
1. Enable detailed logging:
   ```rust
   // In queue_manager.rs
   tracing::debug!(
       "Enqueued exec {} at position {} for action {}",
       execution_id, queue.len(), action_id
   );
   ```

2. Check for race conditions:
   - Multiple threads modifying same queue
   - Lock not held during entire operation
   - Notify called before entry dequeued

**Report immediately** with:
- Executor logs with timestamps
- Database query showing execution order
- Queue stats at time of violation

---

## Best Practices

### For Operators

1. **Monitor queue depths**: Alert on `queue_length > 100`
2. **Set reasonable limits**: Don't set `max_queue_length` too high
3. **Scale workers**: Add workers when queues consistently fill
4. **Regular cleanup**: Run cleanup jobs to remove stale stats
5. **Test policies**: Validate concurrency limits in staging first

### For Developers

1. **Test with queues**: Always test actions with concurrency limits
2. **Handle timeouts**: Implement proper timeout handling in actions
3. **Idempotent actions**: Design actions to be safely retried
4. **Log execution order**: Log start/end times for debugging
5. **Monitor completion rate**: Track `total_completed / total_enqueued`

### For Action Authors

1. **Know your limits**: Understand action's concurrency safety
2. **Fast completions**: Minimize action execution time
3. **Proper error handling**: Always complete (success or failure)
4. **No indefinite blocking**: Use timeouts on external calls
5. **Test at scale**: Stress test with many concurrent requests

---

## Security Considerations

### Queue Exhaustion DoS

**Attack**: Attacker floods system with action requests to fill queues

**Mitigations**:
- **Rate limiting**: API-level request throttling
- **Authentication**: Require auth for action triggers
- **Queue limits**: `max_queue_length` prevents unbounded growth
- **Queue timeouts**: `queue_timeout_seconds` evicts old entries
- **Monitoring**: Alert on sudden queue growth

### Priority Escalation

**Non-Issue**: FIFO prevents priority jumping - no user can skip the queue

### Information Disclosure

**Concern**: Queue stats reveal system load

**Mitigation**: Restrict `/queue-stats` endpoint to authenticated users with appropriate RBAC

---

## Future Enhancements

### Planned Features

- [ ] **Priority queues**: Allow high-priority executions to jump queue
- [ ] **Queue pausing**: Temporarily stop processing specific actions
- [ ] **Batch notifications**: Notify multiple waiters at once
- [ ] **Queue persistence**: Survive executor restarts
- [ ] **Cross-executor coordination**: Distributed queue management
- [ ] **Advanced metrics**: Latency percentiles, queue age histograms
- [ ] **Auto-scaling**: Automatically adjust `max_concurrent` based on load

---

## Related Documentation

- [Executor Service Architecture](./executor-service.md)
- [Policy Enforcement](./policy-enforcement.md)
- [Worker Service](./worker-service.md)
- [API: Actions - Queue Stats Endpoint](./api-actions.md#queue-statistics)
- [Operational Runbook](./ops-runbook.md)

---

## References

- Implementation: `crates/executor/src/queue_manager.rs`
- Tests: `crates/executor/tests/fifo_ordering_integration_test.rs`
- Implementation Plan: `work-summary/2025-01-policy-ordering-plan.md`
- Status: `work-summary/FIFO-ORDERING-STATUS.md`

---

**Version**: 1.0  
**Status**: Production Ready  
**Last Updated**: 2025-01-27