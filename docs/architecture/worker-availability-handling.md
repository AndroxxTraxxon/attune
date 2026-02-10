# Worker Availability Handling

**Status**: Implementation Gap Identified  
**Priority**: High  
**Date**: 2026-02-09

## Problem Statement

When workers are stopped or become unavailable, the executor continues attempting to schedule executions to them, resulting in:

1. **Stuck executions**: Executions remain in `SCHEDULING` or `SCHEDULED` status indefinitely
2. **Queue buildup**: Messages accumulate in worker-specific RabbitMQ queues
3. **No failure notification**: Users don't know their executions are stuck
4. **Resource waste**: System resources consumed by queued messages and database records

## Current Architecture

### Heartbeat Mechanism

Workers send heartbeat updates to the database periodically (default: 30 seconds).

```rust
// From crates/executor/src/scheduler.rs
const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;
const HEARTBEAT_STALENESS_MULTIPLIER: u64 = 3;

fn is_worker_heartbeat_fresh(worker: &Worker) -> bool {
    // Worker is fresh if heartbeat < 90 seconds old
    let max_age = Duration::from_secs(
        DEFAULT_HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER
    );
    // ...
}
```

### Scheduling Flow

```
Execution Created (REQUESTED)
    ↓
Scheduler receives message
    ↓
Find compatible worker with fresh heartbeat
    ↓
Update execution to SCHEDULED
    ↓
Publish message to worker-specific queue
    ↓
Worker consumes and executes
```

### Failure Points

1. **Worker stops after heartbeat**: Worker has fresh heartbeat but is actually down
2. **Worker crashes**: No graceful shutdown, heartbeat appears fresh temporarily
3. **Network partition**: Worker isolated but appears healthy
4. **Queue accumulation**: Messages sit in worker-specific queues indefinitely

## Current Mitigations (Insufficient)

### 1. Heartbeat Staleness Check

```rust
fn select_worker(pool: &PgPool, action: &Action) -> Result<Worker> {
    // Filter by active workers
    let active_workers: Vec<_> = workers
        .into_iter()
        .filter(|w| w.status == WorkerStatus::Active)
        .collect();

    // Filter by heartbeat freshness
    let fresh_workers: Vec<_> = active_workers
        .into_iter()
        .filter(|w| is_worker_heartbeat_fresh(w))
        .collect();

    if fresh_workers.is_empty() {
        return Err(anyhow!("No workers with fresh heartbeats"));
    }

    // Select first available worker
    Ok(fresh_workers.into_iter().next().unwrap())
}
```

**Gap**: Workers can stop within the 90-second staleness window.

### 2. Message Requeue on Error

```rust
// From crates/common/src/mq/consumer.rs
match handler(envelope.clone()).await {
    Err(e) => {
        let requeue = e.is_retriable();
        channel.basic_nack(delivery_tag, BasicNackOptions {
            requeue,
            multiple: false,
        }).await?;
    }
}
```

**Gap**: Only requeues on retriable errors (connection/timeout), not worker unavailability.

### 3. Message TTL Configuration

```rust
// From crates/common/src/config.rs
pub struct MessageQueueConfig {
    #[serde(default = "default_message_ttl")]
    pub message_ttl: u64,
}

fn default_message_ttl() -> u64 {
    3600 // 1 hour
}
```

**Gap**: TTL not currently applied to worker queues, and 1 hour is too long.

## Proposed Solutions

### Solution 1: Execution Timeout Mechanism (HIGH PRIORITY)

Add a background task that monitors scheduled executions and fails them if they don't start within a timeout.

**Implementation:**

```rust
// crates/executor/src/execution_timeout_monitor.rs

pub struct ExecutionTimeoutMonitor {
    pool: PgPool,
    publisher: Arc<Publisher>,
    check_interval: Duration,
    scheduled_timeout: Duration,
}

impl ExecutionTimeoutMonitor {
    pub async fn start(&self) -> Result<()> {
        let mut interval = tokio::time::interval(self.check_interval);

        loop {
            interval.tick().await;

            if let Err(e) = self.check_stale_executions().await {
                error!("Error checking stale executions: {}", e);
            }
        }
    }

    async fn check_stale_executions(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::from_std(self.scheduled_timeout)?;

        // Find executions stuck in SCHEDULED status
        let stale_executions = sqlx::query_as::<_, Execution>(
            "SELECT * FROM execution 
             WHERE status = 'scheduled' 
             AND updated < $1"
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;

        for execution in stale_executions {
            warn!(
                "Execution {} has been scheduled for too long, marking as failed",
                execution.id
            );

            self.fail_execution(
                execution.id,
                "Execution timeout: worker did not pick up task within timeout"
            ).await?;
        }

        Ok(())
    }

    async fn fail_execution(&self, execution_id: i64, reason: &str) -> Result<()> {
        // Update execution status
        sqlx::query(
            "UPDATE execution 
             SET status = 'failed', 
                 result = $2,
                 updated = NOW() 
             WHERE id = $1"
        )
        .bind(execution_id)
        .bind(serde_json::json!({
            "error": reason,
            "failed_by": "execution_timeout_monitor"
        }))
        .execute(&self.pool)
        .await?;

        // Publish completion notification
        let payload = ExecutionCompletedPayload {
            execution_id,
            status: ExecutionStatus::Failed,
            result: Some(serde_json::json!({"error": reason})),
        };

        self.publisher
            .publish_envelope(
                MessageType::ExecutionCompleted,
                payload,
                "attune.executions",
            )
            .await?;

        Ok(())
    }
}
```

**Configuration:**

```yaml
# config.yaml
executor:
  scheduled_timeout: 300  # 5 minutes (fail if not running within 5 min)
  timeout_check_interval: 60  # Check every minute
```

### Solution 2: Worker Queue TTL and DLQ (MEDIUM PRIORITY)

Apply message TTL to worker-specific queues with dead letter exchange.

**Implementation:**

```rust
// When declaring worker-specific queues
let mut queue_args = FieldTable::default();

// Set message TTL (5 minutes)
queue_args.insert(
    "x-message-ttl".into(),
    AMQPValue::LongInt(300_000) // 5 minutes in milliseconds
);

// Set dead letter exchange
queue_args.insert(
    "x-dead-letter-exchange".into(),
    AMQPValue::LongString("attune.executions.dlx".into())
);

channel.queue_declare(
    &format!("attune.execution.worker.{}", worker_id),
    QueueDeclareOptions {
        durable: true,
        ..Default::default()
    },
    queue_args,
).await?;
```

**Dead Letter Handler:**

```rust
// crates/executor/src/dead_letter_handler.rs

pub struct DeadLetterHandler {
    pool: PgPool,
    consumer: Arc<Consumer>,
}

impl DeadLetterHandler {
    pub async fn start(&self) -> Result<()> {
        self.consumer
            .consume_with_handler(|envelope: MessageEnvelope<ExecutionScheduledPayload>| {
                let pool = self.pool.clone();
                
                async move {
                    warn!("Received dead letter for execution {}", envelope.payload.execution_id);
                    
                    // Mark execution as failed
                    sqlx::query(
                        "UPDATE execution 
                         SET status = 'failed', 
                             result = $2,
                             updated = NOW() 
                         WHERE id = $1 AND status = 'scheduled'"
                    )
                    .bind(envelope.payload.execution_id)
                    .bind(serde_json::json!({
                        "error": "Message expired in worker queue (worker unavailable)",
                        "failed_by": "dead_letter_handler"
                    }))
                    .execute(&pool)
                    .await?;
                    
                    Ok(())
                }
            })
            .await
    }
}
```

### Solution 3: Worker Health Probes (LOW PRIORITY)

Add active health checking instead of relying solely on heartbeats.

**Implementation:**

```rust
// crates/executor/src/worker_health_checker.rs

pub struct WorkerHealthChecker {
    pool: PgPool,
    check_interval: Duration,
}

impl WorkerHealthChecker {
    pub async fn start(&self) -> Result<()> {
        let mut interval = tokio::time::interval(self.check_interval);

        loop {
            interval.tick().await;

            if let Err(e) = self.check_worker_health().await {
                error!("Error checking worker health: {}", e);
            }
        }
    }

    async fn check_worker_health(&self) -> Result<()> {
        let workers = WorkerRepository::find_action_workers(&self.pool).await?;

        for worker in workers {
            // Skip if heartbeat is very stale (worker is definitely down)
            if !is_heartbeat_recent(&worker) {
                continue;
            }

            // Attempt health check
            match self.ping_worker(&worker).await {
                Ok(true) => {
                    // Worker is healthy, ensure status is Active
                    if worker.status != Some(WorkerStatus::Active) {
                        self.update_worker_status(worker.id, WorkerStatus::Active).await?;
                    }
                }
                Ok(false) | Err(_) => {
                    // Worker is unhealthy, mark as inactive
                    warn!("Worker {} failed health check", worker.name);
                    self.update_worker_status(worker.id, WorkerStatus::Inactive).await?;
                }
            }
        }

        Ok(())
    }

    async fn ping_worker(&self, worker: &Worker) -> Result<bool> {
        // TODO: Implement health endpoint on worker
        // For now, check if worker's queue is being consumed
        Ok(true)
    }
}
```

### Solution 4: Graceful Worker Shutdown (MEDIUM PRIORITY)

Ensure workers mark themselves as inactive before shutdown.

**Implementation:**

```rust
// In worker service shutdown handler
impl WorkerService {
    pub async fn shutdown(&self) -> Result<()> {
        info!("Worker shutting down gracefully...");

        // Mark worker as inactive
        sqlx::query(
            "UPDATE worker SET status = 'inactive', updated = NOW() WHERE id = $1"
        )
        .bind(self.worker_id)
        .execute(&self.pool)
        .await?;

        // Stop accepting new tasks
        self.stop_consuming().await?;

        // Wait for in-flight tasks to complete (with timeout)
        let timeout = Duration::from_secs(30);
        tokio::time::timeout(timeout, self.wait_for_completion()).await?;

        info!("Worker shutdown complete");
        Ok(())
    }
}
```

**Docker Signal Handling:**

```yaml
# docker-compose.yaml
services:
  worker-shell:
    stop_grace_period: 45s  # Give worker time to finish tasks
```

## Implementation Priority

### Phase 1: Immediate (Week 1)
1. **Execution Timeout Monitor** - Prevents stuck executions
2. **Graceful Shutdown** - Marks workers inactive on stop

### Phase 2: Short-term (Week 2)
3. **Worker Queue TTL + DLQ** - Prevents message buildup
4. **Dead Letter Handler** - Fails expired executions

### Phase 3: Long-term (Month 1)
5. **Worker Health Probes** - Active availability verification
6. **Retry Logic** - Reschedule to different worker on failure

## Configuration

### Recommended Timeouts

```yaml
executor:
  # How long an execution can stay SCHEDULED before failing
  scheduled_timeout: 300  # 5 minutes

  # How often to check for stale executions
  timeout_check_interval: 60  # 1 minute

  # Message TTL in worker queues
  worker_queue_ttl: 300  # 5 minutes (match scheduled_timeout)

  # Worker health check interval
  health_check_interval: 30  # 30 seconds

worker:
  # How often to send heartbeats
  heartbeat_interval: 10  # 10 seconds (more frequent)

  # Grace period for shutdown
  shutdown_timeout: 30  # 30 seconds
```

### Staleness Calculation

```
Heartbeat Staleness Threshold = heartbeat_interval * 3
                               = 10 * 3 = 30 seconds

This means:
- Worker sends heartbeat every 10s
- If heartbeat is > 30s old, worker is considered stale
- Reduces window where stopped worker appears healthy from 90s to 30s
```

## Monitoring and Observability

### Metrics to Track

1. **Execution timeout rate**: Number of executions failed due to timeout
2. **Worker downtime**: Time between last heartbeat and status change
3. **Dead letter queue depth**: Number of expired messages
4. **Average scheduling latency**: Time from REQUESTED to RUNNING

### Alerts

```yaml
alerts:
  - name: high_execution_timeout_rate
    condition: execution_timeouts > 10 per minute
    severity: warning

  - name: no_active_workers
    condition: active_workers == 0
    severity: critical

  - name: dlq_buildup
    condition: dlq_depth > 100
    severity: warning

  - name: stale_executions
    condition: scheduled_executions_older_than_5min > 0
    severity: warning
```

## Testing

### Test Scenarios

1. **Worker stops mid-execution**: Should timeout and fail
2. **Worker never picks up task**: Should timeout after 5 minutes
3. **All workers down**: Should immediately fail with "no workers available"
4. **Worker stops gracefully**: Should mark inactive and not receive new tasks
5. **Message expires in queue**: Should be moved to DLQ and execution failed

### Integration Test Example

```rust
#[tokio::test]
async fn test_execution_timeout_on_worker_down() {
    let pool = setup_test_db().await;
    let mq = setup_test_mq().await;

    // Create worker and execution
    let worker = create_test_worker(&pool).await;
    let execution = create_test_execution(&pool).await;

    // Schedule execution to worker
    schedule_execution(&pool, &mq, execution.id, worker.id).await;

    // Stop worker (simulate crash - no graceful shutdown)
    stop_worker(worker.id).await;

    // Wait for timeout
    tokio::time::sleep(Duration::from_secs(310)).await;

    // Verify execution is marked as failed
    let execution = get_execution(&pool, execution.id).await;
    assert_eq!(execution.status, ExecutionStatus::Failed);
    assert!(execution.result.unwrap()["error"]
        .as_str()
        .unwrap()
        .contains("timeout"));
}
```

## Migration Path

### Step 1: Add Monitoring (No Breaking Changes)
- Deploy execution timeout monitor
- Monitor logs for timeout events
- Tune timeout values based on actual workload

### Step 2: Add DLQ (Requires Queue Reconfiguration)
- Create dead letter exchange
- Update queue declarations with TTL and DLX
- Deploy dead letter handler
- Monitor DLQ depth

### Step 3: Graceful Shutdown (Worker Update)
- Add shutdown handler to worker
- Update Docker Compose stop_grace_period
- Test worker restarts

### Step 4: Health Probes (Future Enhancement)
- Add health endpoint to worker
- Deploy health checker service
- Transition from heartbeat-only to active probing

## Related Documentation

- [Queue Architecture](./queue-architecture.md)
- [Worker Service](./worker-service.md)
- [Executor Service](./executor-service.md)
- [RabbitMQ Queues Quick Reference](../docs/QUICKREF-rabbitmq-queues.md)