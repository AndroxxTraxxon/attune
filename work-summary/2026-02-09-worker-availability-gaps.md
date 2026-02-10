# Worker Availability Handling - Gap Analysis

**Date**: 2026-02-09
**Status**: Investigation Complete - Implementation Pending
**Priority**: High
**Impact**: Operational Reliability

## Issue Reported

User reported that when workers are brought down (e.g., `docker compose down worker-shell`), the executor continues attempting to send executions to the unavailable workers, resulting in stuck executions that never complete or fail.

## Investigation Summary

Investigated the executor's worker selection and scheduling logic to understand how worker availability is determined and what happens when workers become unavailable.

### Current Architecture

**Heartbeat-Based Availability:**
- Workers send heartbeats to database every 30 seconds (configurable)
- Scheduler filters workers based on heartbeat freshness
- Workers are considered "stale" if heartbeat is older than 90 seconds (3x heartbeat interval)
- Only workers with fresh heartbeats are eligible for scheduling

**Scheduling Flow:**
```
Execution (REQUESTED) 
  → Scheduler finds worker with fresh heartbeat
  → Execution status updated to SCHEDULED
  → Message published to worker-specific queue
  → Worker consumes and executes
```

### Root Causes Identified

1. **Heartbeat Staleness Window**: Workers can stop within the 90-second staleness window and still appear "available"
   - Worker sends heartbeat at T=0
   - Worker stops at T=30
   - Scheduler can still select this worker until T=90
   - 60-second window where dead worker appears healthy

2. **No Execution Timeout**: Once scheduled, executions have no timeout mechanism
   - Execution remains in SCHEDULED status indefinitely
   - No background process monitors scheduled executions
   - No automatic failure after reasonable time period

3. **Message Queue Accumulation**: Messages sit in worker-specific queues forever
   - Worker-specific queues: `attune.execution.worker.{worker_id}`
   - No TTL configured on these queues
   - No dead letter queue (DLQ) for expired messages
   - Messages never expire even if worker is permanently down

4. **No Graceful Shutdown**: Workers don't update their status when stopping
   - Docker SIGTERM signal not handled
   - Worker status remains "active" in database
   - No notification that worker is shutting down

5. **Retry Logic Issues**: Failed scheduling doesn't trigger meaningful retries
   - Scheduler returns error if no workers available
   - Error triggers message requeue (via nack)
   - But if worker WAS available during scheduling, message is successfully published
   - No mechanism to detect that worker never picked up the message

### Code Locations

**Heartbeat Check:**
```rust
// crates/executor/src/scheduler.rs:226-241
fn is_worker_heartbeat_fresh(worker: &Worker) -> bool {
    let max_age = Duration::from_secs(
        DEFAULT_HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER
    ); // 30 * 3 = 90 seconds
    
    let is_fresh = age.to_std().unwrap_or(Duration::MAX) <= max_age;
    // ...
}
```

**Worker Selection:**
```rust
// crates/executor/src/scheduler.rs:171-246
async fn select_worker(pool: &PgPool, action: &Action) -> Result<Worker> {
    // 1. Find action workers
    // 2. Filter by runtime compatibility
    // 3. Filter by active status
    // 4. Filter by heartbeat freshness ← Gap: 90s window
    // 5. Select first available (no load balancing)
}
```

**Message Queue Consumer:**
```rust
// crates/common/src/mq/consumer.rs:150-175
match handler(envelope.clone()).await {
    Err(e) => {
        let requeue = e.is_retriable(); // Only retries connection errors
        channel.basic_nack(delivery_tag, BasicNackOptions { requeue, .. })
    }
}
```

## Impact Analysis

### User Experience
- **Stuck executions**: Appear to be running but never complete
- **No feedback**: Users don't know execution failed until they check manually
- **Confusion**: Status shows SCHEDULED but nothing happens
- **Lost work**: Executions that could have been routed to healthy workers are stuck

### System Impact
- **Queue buildup**: Messages accumulate in unavailable worker queues
- **Database pollution**: SCHEDULED executions remain in database indefinitely
- **Resource waste**: Memory and disk consumed by stuck state
- **Monitoring gaps**: No clear way to detect this condition

### Severity
**HIGH** - This affects core functionality (execution reliability) and user trust in the system. In production, this would result in:
- Failed automations with no notification
- Debugging difficulties (why didn't my rule execute?)
- Potential data loss (execution intended to process event is lost)

## Proposed Solutions

Comprehensive solution document created at: `docs/architecture/worker-availability-handling.md`

### Phase 1: Immediate Fixes (HIGH PRIORITY)

#### 1. Execution Timeout Monitor
**Purpose**: Fail executions that remain SCHEDULED too long

**Implementation:**
- Background task in executor service
- Checks every 60 seconds for stale scheduled executions
- Fails executions older than 5 minutes
- Updates status to FAILED with descriptive error
- Publishes ExecutionCompleted notification

**Impact**: Prevents indefinitely stuck executions

#### 2. Graceful Worker Shutdown
**Purpose**: Mark workers inactive before they stop

**Implementation:**
- Add SIGTERM handler to worker service
- Update worker status to INACTIVE in database
- Stop consuming from queue
- Wait for in-flight tasks to complete (30s timeout)
- Then exit

**Impact**: Reduces window where dead worker appears available

### Phase 2: Medium-Term Improvements (MEDIUM PRIORITY)

#### 3. Worker Queue TTL + Dead Letter Queue
**Purpose**: Expire messages that sit too long in worker queues

**Implementation:**
- Configure `x-message-ttl: 300000` (5 minutes) on worker queues
- Configure `x-dead-letter-exchange` to route expired messages
- Create DLQ exchange and queue
- Add dead letter handler to fail executions from DLQ

**Impact**: Prevents message queue buildup

#### 4. Reduced Heartbeat Interval
**Purpose**: Detect unavailable workers faster

**Configuration Changes:**
```yaml
worker:
  heartbeat_interval: 10  # Down from 30 seconds

executor:
  # Staleness = 10 * 3 = 30 seconds (down from 90s)
```

**Impact**: 60-second window reduced to 20 seconds

### Phase 3: Long-Term Enhancements (LOW PRIORITY)

#### 5. Active Health Probes
**Purpose**: Verify worker availability beyond heartbeats

**Implementation:**
- Add health endpoint to worker service
- Background health checker in executor
- Pings workers periodically
- Marks workers INACTIVE if unresponsive

**Impact**: More reliable availability detection

#### 6. Intelligent Retry with Worker Affinity
**Purpose**: Reschedule failed executions to different workers

**Implementation:**
- Track which worker was assigned to execution
- On timeout, reschedule to different worker
- Implement exponential backoff
- Maximum retry limit

**Impact**: Better fault tolerance

## Recommended Immediate Actions

1. **Deploy Execution Timeout Monitor** (Week 1)
   - Add timeout check to executor service
   - Configure 5-minute timeout for SCHEDULED executions
   - Monitor timeout rate to tune values

2. **Add Graceful Shutdown to Workers** (Week 1)
   - Implement SIGTERM handler
   - Update Docker Compose `stop_grace_period: 45s`
   - Test worker restart scenarios

3. **Reduce Heartbeat Interval** (Week 1)
   - Update config: `worker.heartbeat_interval: 10`
   - Reduces staleness window from 90s to 30s
   - Low-risk configuration change

4. **Document Known Limitation** (Week 1)
   - Add operational notes about worker restart behavior
   - Document expected timeout duration
   - Provide troubleshooting guide

## Testing Strategy

### Manual Testing
1. Start system with worker running
2. Create execution
3. Immediately stop worker: `docker compose stop worker-shell`
4. Observe execution status over 5 minutes
5. Verify execution fails with timeout error
6. Verify notification sent to user

### Integration Tests
```rust
#[tokio::test]
async fn test_execution_timeout_on_worker_unavailable() {
    // 1. Create worker and start heartbeat
    // 2. Schedule execution
    // 3. Stop worker (no graceful shutdown)
    // 4. Wait > timeout duration
    // 5. Assert execution status = FAILED
    // 6. Assert error message contains "timeout"
}

#[tokio::test]
async fn test_graceful_worker_shutdown() {
    // 1. Create worker with active execution
    // 2. Send SIGTERM
    // 3. Verify worker status → INACTIVE
    // 4. Verify existing execution completes
    // 5. Verify new executions not scheduled to this worker
}
```

### Load Testing
- Test with multiple workers
- Stop workers randomly during execution
- Verify executions redistribute to healthy workers
- Measure timeout detection latency

## Metrics to Monitor Post-Deployment

1. **Execution Timeout Rate**: Track how often executions timeout
2. **Timeout Latency**: Time from worker stop to execution failure
3. **Queue Depth**: Monitor worker-specific queue lengths
4. **Heartbeat Gaps**: Track time between last heartbeat and status change
5. **Worker Restart Impact**: Measure execution disruption during restarts

## Configuration Recommendations

### Development
```yaml
executor:
  scheduled_timeout: 120  # 2 minutes (faster feedback)
  timeout_check_interval: 30  # Check every 30 seconds

worker:
  heartbeat_interval: 10
  shutdown_timeout: 15
```

### Production
```yaml
executor:
  scheduled_timeout: 300  # 5 minutes
  timeout_check_interval: 60  # Check every minute

worker:
  heartbeat_interval: 10
  shutdown_timeout: 30
```

## Related Work

This investigation complements:
- **2026-02-09 DOTENV Parameter Flattening**: Fixes action execution parameters
- **2026-02-09 URL Query Parameter Support**: Improves web UI filtering
- **Worker Heartbeat Monitoring**: Existing heartbeat mechanism (needs enhancement)

Together, these improvements address both execution correctness (parameter passing) and execution reliability (worker availability).

## Documentation Created

1. `docs/architecture/worker-availability-handling.md` - Comprehensive solution guide
   - Problem statement and current architecture
   - Detailed solutions with code examples
   - Implementation priorities and phases
   - Configuration recommendations
   - Testing strategies
   - Migration path

## Next Steps

1. **Review solutions document** with team
2. **Prioritize implementation** based on urgency and resources
3. **Create implementation tickets** for each solution
4. **Schedule deployment** of Phase 1 fixes
5. **Establish monitoring** for new metrics
6. **Document operational procedures** for worker management

## Conclusion

The executor lacks robust handling for worker unavailability, relying solely on heartbeat staleness checks with a wide time window. Multiple complementary solutions are needed:

- **Short-term**: Timeout monitor + graceful shutdown (prevents indefinite stuck state)
- **Medium-term**: Queue TTL + DLQ (prevents message buildup)
- **Long-term**: Health probes + retry logic (improves reliability)

**Priority**: Phase 1 solutions should be implemented immediately as they address critical operational gaps that affect system reliability and user experience.