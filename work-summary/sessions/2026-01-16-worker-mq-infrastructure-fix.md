# Worker Service Message Queue Infrastructure Fix

**Date:** 2026-01-16
**Status:** ✅ Completed

## Problem

The worker service was failing to start with the following error:

```
ERROR ThreadId(22) io_loop: Channel closed channel=2 method=Close { 
  reply_code: 404, 
  reply_text: ShortString("NOT_FOUND - no queue 'worker.1.executions' in vhost '/'"), 
  class_id: 60, 
  method_id: 20 
} error=AMQPError { 
  kind: Soft(NOTFOUND), 
  message: ShortString("NOT_FOUND - no queue 'worker.1.executions' in vhost '/'") 
}
```

This occurred because:
1. The worker was trying to consume from a dynamically-named queue `worker.{worker_id}.executions` that didn't exist
2. The RabbitMQ infrastructure (exchanges, queues, bindings) was not being set up by the worker
3. Worker-specific queues need to be created dynamically after worker registration

## Root Cause

The worker service was:
- Assuming the RabbitMQ infrastructure already existed
- Attempting to consume from a worker-specific queue without creating it first
- Not declaring or binding the queue before starting the consumer
- Missing infrastructure setup that other services (executor) had

## Solution

Updated the worker service to:

### 1. Set Up Base Infrastructure on Startup
Added automatic infrastructure setup similar to the executor service:

```rust
// Setup message queue infrastructure (exchanges, queues, bindings)
let mq_config = MqConfig::default();
match mq_connection.setup_infrastructure(&mq_config).await {
    Ok(_) => info!("Message queue infrastructure setup completed"),
    Err(e) => {
        warn!(
            "Failed to setup MQ infrastructure (may already exist): {}",
            e
        );
    }
}
```

This creates the base infrastructure:
- Exchanges: `attune.events`, `attune.executions`, `attune.notifications`
- Base queues: `attune.events.queue`, `attune.executions.queue`, `attune.notifications.queue`
- Dead letter exchange: `attune.dlx`

### 2. Create Worker-Specific Queue Dynamically
Added dynamic queue creation in `start_execution_consumer()`:

```rust
// Create the worker-specific queue
let worker_queue = QueueConfig {
    name: queue_name.clone(),
    durable: false,        // Worker queues are temporary
    exclusive: false,
    auto_delete: true,     // Delete when worker disconnects
};

self.mq_connection
    .declare_queue(&worker_queue)
    .await?;
```

**Key Design Decisions:**
- `durable: false` - Worker queues don't need to survive broker restarts
- `auto_delete: true` - Queues are automatically cleaned up when worker disconnects
- Queue name format: `worker.{worker_id}.executions`

### 3. Bind Queue to Exchange with Worker-Specific Routing Key
Added proper binding so the scheduler can route executions to specific workers:

```rust
// Bind the queue to the executions exchange with worker-specific routing key
self.mq_connection
    .bind_queue(
        &queue_name,
        "attune.executions",
        &format!("worker.{}", worker_id),
    )
    .await?;
```

This allows the executor/scheduler to target specific workers by publishing messages with routing key `worker.{worker_id}`.

### 4. Added Proper Imports
- Added `config::MessageQueueConfig as MqConfig` import
- Added `QueueConfig` import for queue configuration
- Added `warn` import from `tracing` for setup warnings

## Implementation Details

### Files Modified

**`crates/worker/src/service.rs`:**
- Added `MessageQueueConfig` import from `attune_common::mq::config`
- Added `QueueConfig` import
- Added infrastructure setup in `new()` method
- Enhanced `start_execution_consumer()` to:
  - Create worker-specific queue dynamically
  - Bind queue to exchange with worker-specific routing key
  - Add detailed logging for each step
- All operations happen after worker registration (when worker_id is known)

### Infrastructure Created

#### Base Infrastructure (shared with executor)
1. **Exchanges**:
   - `attune.events` (Topic)
   - `attune.executions` (Direct)
   - `attune.notifications` (Fanout)

2. **Base Queues**:
   - `attune.events.queue`
   - `attune.executions.queue`
   - `attune.notifications.queue`

3. **Dead Letter Exchange**: `attune.dlx`

#### Worker-Specific Infrastructure (per worker)
1. **Worker Queue**: `worker.{worker_id}.executions`
   - Non-durable (temporary)
   - Auto-delete when worker disconnects
   - Bound to `attune.executions` exchange

2. **Routing Key**: `worker.{worker_id}`
   - Allows targeted message delivery to specific workers
   - Scheduler can route executions based on worker capabilities

## Testing

Verified the fix works correctly:

1. ✅ **Base Infrastructure Setup**: Common exchanges and queues created
   ```
   INFO Setting up RabbitMQ infrastructure
   INFO Queue 'attune.events.queue' declared with dead letter exchange 'attune.dlx'
   INFO Queue 'attune.executions.queue' declared with dead letter exchange 'attune.dlx'
   INFO Queue 'attune.notifications.queue' declared with dead letter exchange 'attune.dlx'
   INFO RabbitMQ infrastructure setup complete
   ```

2. ✅ **Worker Registration**: Worker successfully registers with database
   ```
   INFO Worker registered with ID: 1
   ```

3. ✅ **Dynamic Queue Creation**: Worker-specific queue is created
   ```
   INFO Creating worker-specific queue: worker.1.executions
   INFO Worker queue created: worker.1.executions
   ```

4. ✅ **Queue Binding**: Queue bound to exchange with proper routing key
   ```
   INFO Queue 'worker.1.executions' bound to exchange 'attune.executions' with routing key 'worker.1'
   INFO Queue bound to exchange with routing key 'worker.1'
   ```

5. ✅ **Consumer Start**: Worker successfully starts consuming messages
   ```
   INFO Consumer started for queue: worker.1.executions
   INFO Message queue consumer initialized
   ```

6. ✅ **Service Ready**: Worker service fully operational
   ```
   INFO Worker Service started successfully
   ```

## Impact

- **Automated Setup**: Workers create their own queues automatically on startup
- **Dynamic Infrastructure**: Queue creation happens after worker registration
- **Clean Shutdown**: Auto-delete queues prevent orphaned queues from offline workers
- **Targeted Routing**: Scheduler can route executions to specific workers
- **Idempotent**: Base infrastructure setup is safe to run multiple times
- **Better Logging**: Clear visibility into infrastructure creation steps

## Architecture Notes

### Worker Queue Lifecycle
1. Worker starts and connects to MQ
2. Worker sets up base infrastructure (idempotent)
3. Worker registers with database, receives worker_id
4. Worker creates queue: `worker.{worker_id}.executions`
5. Worker binds queue to `attune.executions` exchange
6. Worker starts consuming with tag `worker-{worker_id}`
7. On shutdown/disconnect, queue is automatically deleted

### Message Routing Flow
```
Scheduler/Executor
    ↓ (publishes to)
attune.executions exchange
    ↓ (routes by key: worker.{id})
worker.{id}.executions queue
    ↓ (consumed by)
Worker Service
```

### Queue Configuration Rationale
- **Non-durable**: Worker queues are ephemeral, tied to worker lifetime
- **Auto-delete**: Prevents accumulation of queues from dead workers
- **Non-exclusive**: Allows monitoring/management tools to inspect queues
- **Named by worker_id**: Enables targeted execution assignment

## Comparison with Other Services

### Executor Service
- Uses persistent queues: `attune.executions.queue`
- Multiple competing consumers on same queue
- Infrastructure setup in service initialization

### Worker Service
- Uses ephemeral per-worker queues: `worker.{id}.executions`
- One consumer per worker queue
- Queue creation after worker registration
- Auto-cleanup when worker disconnects

### Sensor Service
- Uses `MessageQueue` wrapper (publish-only)
- No consumer setup needed
- May need similar updates if consuming in future

## Next Steps

- [ ] Add health check that verifies worker queue exists and is bound
- [ ] Consider adding queue TTL for additional cleanup safety
- [ ] Add metrics for worker queue depth and consumer performance
- [ ] Document worker routing patterns in architecture docs
- [ ] Consider adding worker queue monitoring/alerting
- [ ] Test worker failover scenarios (queue cleanup)

## Related Issues

This fix follows the same pattern as:
- **2026-01-15**: Executor service MQ infrastructure fix
- **2026-01-15**: Configuration URL parsing fix

All services now properly set up their required MQ infrastructure on startup.

## Notes

- Worker queues are intentionally temporary and auto-delete
- Each worker gets its own queue for targeted execution delivery
- The routing key pattern `worker.{id}` allows flexible execution scheduling
- Infrastructure setup is idempotent - safe for multiple workers to run simultaneously
- Queue creation happens after registration to ensure worker_id is available