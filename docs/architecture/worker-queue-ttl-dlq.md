# Worker Queue TTL and Dead Letter Queue (Phase 2)

## Overview

Phase 2 of worker availability handling implements message TTL (time-to-live) on worker-specific queues and dead letter queue (DLQ) processing. This ensures that executions sent to unavailable workers are automatically failed instead of remaining stuck indefinitely.

## Architecture

### Message Flow

```
┌─────────────┐
│  Executor   │
│  Scheduler  │
└──────┬──────┘
       │ Publishes ExecutionRequested
       │ routing_key: execution.dispatch.worker.{id}
       │
       ▼
┌──────────────────────────────────┐
│  worker.{id}.executions queue    │
│                                  │
│  Properties:                     │
│  - x-message-ttl: 300000ms (5m)  │
│  - x-dead-letter-exchange: dlx   │
└──────┬───────────────────┬───────┘
       │                   │
       │ Worker consumes   │ TTL expires
       │ (normal flow)     │ (worker unavailable)
       │                   │
       ▼                   ▼
┌──────────────┐    ┌──────────────────┐
│   Worker     │    │  attune.dlx      │
│   Service    │    │  (Dead Letter    │
│              │    │   Exchange)      │
└──────────────┘    └────────┬─────────┘
                             │
                             │ Routes to DLQ
                             │
                             ▼
                    ┌──────────────────────┐
                    │  attune.dlx.queue    │
                    │  (Dead Letter Queue) │
                    └────────┬─────────────┘
                             │
                             │ Consumes
                             │
                             ▼
                    ┌──────────────────────┐
                    │  Dead Letter Handler │
                    │  (in Executor)       │
                    │                      │
                    │  - Identifies exec   │
                    │  - Marks as FAILED   │
                    │  - Logs failure      │
                    └──────────────────────┘
```

### Components

#### 1. Worker Queue TTL

**Configuration:**
- Default: 5 minutes (300,000 milliseconds)
- Configurable via `rabbitmq.worker_queue_ttl_ms`

**Implementation:**
- Applied during queue declaration in `Connection::setup_worker_infrastructure()`
- Uses RabbitMQ's `x-message-ttl` queue argument
- Only applies to worker-specific queues (`worker.{id}.executions`)

**Behavior:**
- When a message remains in the queue longer than TTL
- RabbitMQ automatically moves it to the configured dead letter exchange
- Original message properties and headers are preserved
- Includes `x-death` header with expiration details

#### 2. Dead Letter Exchange (DLX)

**Configuration:**
- Exchange name: `attune.dlx`
- Type: `direct`
- Durable: `true`

**Setup:**
- Created in `Connection::setup_common_infrastructure()`
- Bound to dead letter queue with routing key `#` (all messages)
- Shared across all services

#### 3. Dead Letter Queue

**Configuration:**
- Queue name: `attune.dlx.queue`
- Durable: `true`
- TTL: 24 hours (configurable via `rabbitmq.dead_letter.ttl_ms`)

**Properties:**
- Retains messages for debugging and analysis
- Messages auto-expire after retention period
- No DLX on the DLQ itself (prevents infinite loops)

#### 4. Dead Letter Handler

**Location:** `crates/executor/src/dead_letter_handler.rs`

**Responsibilities:**
1. Consume messages from `attune.dlx.queue`
2. Deserialize message envelope
3. Extract execution ID from payload
4. Verify execution is in non-terminal state
5. Update execution to FAILED status
6. Add descriptive error information
7. Acknowledge message (remove from DLQ)

**Error Handling:**
- Invalid messages: Acknowledged and discarded
- Missing executions: Acknowledged (already processed)
- Terminal state executions: Acknowledged (no action needed)
- Database errors: Nacked with requeue (retry later)

## Configuration

### RabbitMQ Configuration Structure

```yaml
message_queue:
  rabbitmq:
    # Worker queue TTL - how long messages wait before DLX
    worker_queue_ttl_ms: 300000  # 5 minutes (default)
    
    # Dead letter configuration
    dead_letter:
      enabled: true                # Enable DLQ system
      exchange: attune.dlx         # DLX name
      ttl_ms: 86400000            # DLQ retention (24 hours)
```

### Environment-Specific Settings

#### Development (`config.development.yaml`)
```yaml
message_queue:
  rabbitmq:
    worker_queue_ttl_ms: 300000  # 5 minutes
    dead_letter:
      enabled: true
      exchange: attune.dlx
      ttl_ms: 86400000  # 24 hours
```

#### Production (`config.docker.yaml`)
```yaml
message_queue:
  rabbitmq:
    worker_queue_ttl_ms: 300000  # 5 minutes
    dead_letter:
      enabled: true
      exchange: attune.dlx
      ttl_ms: 86400000  # 24 hours
```

### Tuning Guidelines

**Worker Queue TTL (`worker_queue_ttl_ms`):**
- **Too short:** Legitimate slow workers may have executions failed prematurely
- **Too long:** Unavailable workers cause delayed failure detection
- **Recommendation:** 2-5x typical execution time, minimum 2 minutes
- **Default (5 min):** Good balance for most workloads

**DLQ Retention (`dead_letter.ttl_ms`):**
- Purpose: Debugging and forensics
- **Too short:** May lose data before analysis
- **Too long:** Accumulates stale data
- **Recommendation:** 24-48 hours in production
- **Default (24 hours):** Adequate for most troubleshooting

## Code Structure

### Queue Declaration with TTL

```rust
// crates/common/src/mq/connection.rs

pub async fn declare_queue_with_dlx_and_ttl(
    &self,
    config: &QueueConfig,
    dlx_exchange: &str,
    ttl_ms: Option<u64>,
) -> MqResult<()> {
    let mut args = FieldTable::default();
    
    // Configure DLX
    args.insert(
        "x-dead-letter-exchange".into(),
        AMQPValue::LongString(dlx_exchange.into()),
    );
    
    // Configure TTL if specified
    if let Some(ttl) = ttl_ms {
        args.insert(
            "x-message-ttl".into(),
            AMQPValue::LongInt(ttl as i64),
        );
    }
    
    // Declare queue with arguments
    channel.queue_declare(&config.name, options, args).await?;
    Ok(())
}
```

### Dead Letter Handler

```rust
// crates/executor/src/dead_letter_handler.rs

pub struct DeadLetterHandler {
    pool: Arc<PgPool>,
    consumer: Consumer,
    running: Arc<Mutex<bool>>,
}

impl DeadLetterHandler {
    pub async fn start(&self) -> Result<(), Error> {
        self.consumer.consume_with_handler(|envelope| {
            match envelope.message_type {
                MessageType::ExecutionRequested => {
                    handle_execution_requested(&pool, &envelope).await
                }
                _ => {
                    // Unexpected message type - acknowledge and discard
                    Ok(())
                }
            }
        }).await
    }
}

async fn handle_execution_requested(
    pool: &PgPool,
    envelope: &MessageEnvelope<Value>,
) -> MqResult<()> {
    // Extract execution ID
    let execution_id = envelope.payload.get("execution_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| /* error */)?;
    
    // Fetch current state
    let execution = ExecutionRepository::find_by_id(pool, execution_id).await?;
    
    // Only fail if in non-terminal state
    if !execution.status.is_terminal() {
        ExecutionRepository::update(pool, execution_id, UpdateExecutionInput {
            status: Some(ExecutionStatus::Failed),
            result: Some(json!({
                "error": "Worker queue TTL expired",
                "message": "Worker did not process execution within configured TTL",
            })),
            ended: Some(Some(Utc::now())),
            ..Default::default()
        }).await?;
    }
    
    Ok(())
}
```

## Integration with Executor Service

The dead letter handler is started automatically by the executor service if DLQ is enabled:

```rust
// crates/executor/src/service.rs

pub async fn start(&self) -> Result<()> {
    // ... other components ...
    
    // Start dead letter handler (if enabled)
    if self.inner.mq_config.rabbitmq.dead_letter.enabled {
        let dlq_name = format!("{}.queue", 
            self.inner.mq_config.rabbitmq.dead_letter.exchange);
        let dlq_consumer = Consumer::new(
            &self.inner.mq_connection,
            create_dlq_consumer_config(&dlq_name, "executor.dlq"),
        ).await?;
        
        let dlq_handler = Arc::new(
            DeadLetterHandler::new(self.inner.pool.clone(), dlq_consumer).await?
        );
        
        handles.push(tokio::spawn(async move {
            dlq_handler.start().await
        }));
    }
    
    // ... wait for completion ...
}
```

## Operational Considerations

### Monitoring

**Key Metrics:**
- DLQ message rate (messages/sec entering DLQ)
- DLQ queue depth (current messages in DLQ)
- DLQ processing latency (time from DLX to handler)
- Failed execution count (executions failed via DLQ)

**Alerting Thresholds:**
- DLQ rate > 10/min: Workers may be unhealthy or TTL too aggressive
- DLQ depth > 100: Handler may be falling behind
- High failure rate: Systematic worker availability issues

### RabbitMQ Management

**View DLQ:**
```bash
# List messages in DLQ
rabbitmqadmin list queues name messages

# Get DLQ details
rabbitmqadmin show queue name=attune.dlx.queue

# Purge DLQ (use with caution)
rabbitmqadmin purge queue name=attune.dlx.queue
```

**View Dead Letters:**
```bash
# Get message from DLQ
rabbitmqadmin get queue=attune.dlx.queue count=1

# Check message death history
# Look for x-death header in message properties
```

### Troubleshooting

#### High DLQ Rate

**Symptoms:** Many executions failing via DLQ

**Causes:**
1. Workers down or restarting frequently
2. Worker queue TTL too aggressive
3. Worker overloaded (not consuming fast enough)
4. Network issues between executor and workers

**Resolution:**
1. Check worker health and logs
2. Verify worker heartbeats in database
3. Consider increasing `worker_queue_ttl_ms`
4. Scale worker fleet if overloaded

#### DLQ Handler Not Processing

**Symptoms:** DLQ depth increasing, executions stuck

**Causes:**
1. Executor service not running
2. DLQ disabled in configuration
3. Database connection issues
4. Handler crashed or deadlocked

**Resolution:**
1. Check executor service logs
2. Verify `dead_letter.enabled = true`
3. Check database connectivity
4. Restart executor service if needed

#### Messages Not Reaching DLQ

**Symptoms:** Executions stuck, DLQ empty

**Causes:**
1. Worker queues not configured with DLX
2. DLX exchange not created
3. DLQ not bound to DLX
4. TTL not configured on worker queues

**Resolution:**
1. Restart services to recreate infrastructure
2. Verify RabbitMQ configuration
3. Check queue properties in RabbitMQ management UI

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_expired_execution_handling() {
    let pool = setup_test_db().await;
    
    // Create execution in SCHEDULED state
    let execution = create_test_execution(&pool, ExecutionStatus::Scheduled).await;
    
    // Simulate DLQ message
    let envelope = MessageEnvelope::new(
        MessageType::ExecutionRequested,
        json!({ "execution_id": execution.id }),
    );
    
    // Process message
    handle_execution_requested(&pool, &envelope).await.unwrap();
    
    // Verify execution failed
    let updated = ExecutionRepository::find_by_id(&pool, execution.id).await.unwrap();
    assert_eq!(updated.status, ExecutionStatus::Failed);
    assert!(updated.result.unwrap()["error"].as_str().unwrap().contains("TTL expired"));
}
```

### Integration Tests

```bash
# 1. Start all services
docker compose up -d

# 2. Create execution targeting stopped worker
curl -X POST http://localhost:8080/api/v1/executions \
  -H "Content-Type: application/json" \
  -d '{
    "action_ref": "core.echo",
    "parameters": {"message": "test"},
    "worker_id": 999  # Non-existent worker
  }'

# 3. Wait for TTL expiration (5+ minutes)
sleep 330

# 4. Verify execution failed
curl http://localhost:8080/api/v1/executions/{id}
# Should show status: "failed", error: "Worker queue TTL expired"

# 5. Check DLQ processed the message
rabbitmqadmin list queues name messages | grep attune.dlx.queue
# Should show 0 messages (processed and removed)
```

## Relationship to Other Phases

### Phase 1 (Completed)
- Execution timeout monitor: Handles executions stuck in SCHEDULED
- Graceful shutdown: Prevents new tasks to stopping workers
- Reduced heartbeat: Faster stale worker detection

**Interaction:** Phase 1 timeout monitor acts as a backstop if DLQ processing fails

### Phase 2 (Current)
- Worker queue TTL: Automatic message expiration
- Dead letter queue: Capture expired messages
- Dead letter handler: Process and fail expired executions

**Benefit:** More precise failure detection at the message queue level

### Phase 3 (Planned)
- Health probes: Proactive worker health checking
- Intelligent retry: Retry transient failures
- Load balancing: Distribute work across healthy workers

**Integration:** Phase 3 will use Phase 2 DLQ data to inform routing decisions

## Benefits

1. **Automatic Failure Detection:** No manual intervention needed for unavailable workers
2. **Precise Timing:** TTL provides exact failure window (vs polling-based Phase 1)
3. **Resource Efficiency:** Prevents message accumulation in worker queues
4. **Debugging Support:** DLQ retains messages for forensic analysis
5. **Graceful Degradation:** System continues functioning even with worker failures

## Limitations

1. **TTL Precision:** RabbitMQ TTL is approximate, not guaranteed to the millisecond
2. **Race Conditions:** Worker may start processing just as TTL expires (rare)
3. **DLQ Capacity:** Very high failure rates may overwhelm DLQ
4. **No Retry Logic:** Phase 2 always fails; Phase 3 will add intelligent retry

## Future Enhancements (Phase 3)

- **Conditional Retry:** Retry messages based on failure reason
- **Priority DLQ:** Prioritize critical execution failures
- **DLQ Analytics:** Aggregate statistics on failure patterns
- **Auto-scaling:** Scale workers based on DLQ rate
- **Custom TTL:** Per-action or per-execution TTL configuration

## References

- RabbitMQ Dead Letter Exchanges: https://www.rabbitmq.com/dlx.html
- RabbitMQ TTL: https://www.rabbitmq.com/ttl.html
- Phase 1 Documentation: `docs/architecture/worker-availability-handling.md`
- Queue Architecture: `docs/architecture/queue-architecture.md`
