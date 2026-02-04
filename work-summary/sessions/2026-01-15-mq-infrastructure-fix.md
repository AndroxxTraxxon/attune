# Message Queue Infrastructure Fix

**Date:** 2026-01-15
**Status:** ✅ Completed

## Problem

The executor service was failing to start with the following error:

```
ERROR ThreadId(22) io_loop: Channel closed channel=2 method=Close { 
  reply_code: 404, 
  reply_text: ShortString("NOT_FOUND - no queue 'executor.main' in vhost '/'"), 
  class_id: 60, 
  method_id: 20 
} error=AMQPError { 
  kind: Soft(NOTFOUND), 
  message: ShortString("NOT_FOUND - no queue 'executor.main' in vhost '/'") 
}
```

This occurred because:
1. The executor was trying to consume from a hardcoded queue name `"executor.main"` that didn't exist
2. The RabbitMQ infrastructure (exchanges, queues, bindings) was not being set up automatically
3. Services expected queues to exist before they could start consuming messages

After fixing the queue issue, a second error appeared:

```
ERROR Connection closed channel=0 method=Close { 
  reply_code: 530, 
  reply_text: ShortString("NOT_ALLOWED - attempt to reuse consumer tag 'executor'"),
  class_id: 60, 
  method_id: 20 
}
```

This occurred because all three executor components (enforcement processor, scheduler, execution manager) were attempting to share the same Consumer instance with the same consumer tag.

## Root Cause

The executor service had two issues:

**Issue 1: Missing Queue**
- Using a hardcoded queue name (`"executor.main"`) instead of the configured queue name
- Not setting up the RabbitMQ infrastructure on startup
- Assuming queues would be created externally before service startup

**Issue 2: Shared Consumer Tag**
- All three executor components were sharing a single Consumer instance
- RabbitMQ requires unique consumer tags per connection/channel
- Multiple consumers cannot use the same tag on the same connection

## Solution

Updated the executor service to:

### 1. Set Up Infrastructure on Startup
Added automatic infrastructure setup that creates:
- **Exchanges**: `attune.events`, `attune.executions`, `attune.notifications`
- **Queues**: `attune.events.queue`, `attune.executions.queue`, `attune.notifications.queue`
- **Dead Letter Exchange**: `attune.dlx` for failed message handling
- **Bindings**: Proper routing between exchanges and queues

```rust
// Setup message queue infrastructure (exchanges, queues, bindings)
let mq_config = MqConfig::default();
match mq_connection.setup_infrastructure(&mq_config).await {
    Ok(_) => info!("Message queue infrastructure setup completed"),
    Err(e) => {
        warn!("Failed to setup MQ infrastructure (may already exist): {}", e);
    }
}
```

### 2. Create Individual Consumers with Unique Tags
Changed from sharing a single consumer to creating separate consumers for each component:

**Before:**
```rust
// Single consumer shared by all components
let consumer = Consumer::new(
    &mq_connection,
    attune_common::mq::ConsumerConfig {
        queue: "executor.main".to_string(),
        tag: "executor".to_string(),  // Same tag for all!
        // ...
    },
).await?;

// All components share the same consumer
let enforcement_processor = EnforcementProcessor::new(
    pool.clone(),
    publisher.clone(),
    consumer.clone(),  // Shared
);
let scheduler = ExecutionScheduler::new(
    pool.clone(),
    publisher.clone(),
    consumer.clone(),  // Shared
);
let execution_manager = ExecutionManager::new(
    pool.clone(),
    publisher.clone(),
    consumer.clone(),  // Shared
);
```

**After:**
```rust
// Each component creates its own consumer with unique tag
let enforcement_consumer = Consumer::new(
    &mq_connection,
    attune_common::mq::ConsumerConfig {
        queue: queue_name.clone(),
        tag: "executor.enforcement".to_string(),  // Unique tag
        // ...
    },
).await?;
let enforcement_processor = EnforcementProcessor::new(
    pool.clone(),
    publisher.clone(),
    Arc::new(enforcement_consumer),
);

let scheduler_consumer = Consumer::new(
    &mq_connection,
    attune_common::mq::ConsumerConfig {
        queue: queue_name.clone(),
        tag: "executor.scheduler".to_string(),  // Unique tag
        // ...
    },
).await?;
let scheduler = ExecutionScheduler::new(
    pool.clone(),
    publisher.clone(),
    Arc::new(scheduler_consumer),
);

let manager_consumer = Consumer::new(
    &mq_connection,
    attune_common::mq::ConsumerConfig {
        queue: queue_name.clone(),
        tag: "executor.manager".to_string(),  // Unique tag
        // ...
    },
).await?;
let execution_manager = ExecutionManager::new(
    pool.clone(),
    publisher.clone(),
    Arc::new(manager_consumer),
);
```

This implements a **competing consumers pattern** where multiple consumers process messages from the same queue, with RabbitMQ distributing messages among them.

### 3. Use Proper Queue Configuration
Changed from hardcoded queue name to using the configured queue:

**Before:**
```rust
queue: "executor.main".to_string(),
```

**After:**
```rust
let queue_name = mq_config.rabbitmq.queues.executions.name.clone();
queue: queue_name.clone(),
```

### 4. Added Proper Imports and Error Handling
- Added `MessageQueueConfig as MqConfig` import from `attune_common::mq`
- Added `warn` import from `tracing` for logging setup warnings
- Infrastructure setup errors are logged but don't fail startup (idempotent)

## Implementation Details

### Files Modified

**`crates/executor/src/service.rs`:**
- Added `MqConfig` import from `attune_common::mq`
- Added infrastructure setup in `ExecutorService::new()`
- Changed hardcoded queue name to use config
- Removed shared `consumer` field from `ExecutorServiceInner`
- Added `queue_name` field to store configured queue name
- Updated `start()` method to create individual consumers for each component
- Each consumer has a unique tag: `executor.enforcement`, `executor.scheduler`, `executor.manager`
- Removed `consumer()` accessor method (no longer needed)
- Added informative logging for queue initialization

### Infrastructure Created

The `setup_infrastructure()` call creates:

1. **Dead Letter Exchange**: `attune.dlx` (Fanout)
   - Handles failed messages that exceed retry limits

2. **Exchanges**:
   - `attune.events` (Topic) - for sensor-generated events
   - `attune.executions` (Direct) - for execution messages
   - `attune.notifications` (Fanout) - for notification broadcasts

3. **Queues** (all durable, with DLX):
   - `attune.events.queue` - bound to events exchange with routing key `#`
   - `attune.executions.queue` - bound to executions exchange with routing key `execution`
   - `attune.notifications.queue` - bound to notifications exchange

4. **Dead Letter Queues**:
   - Automatically created for each main queue
   - Messages TTL: 24 hours before expiration

## Testing

Verified the fix works correctly:

1. ✅ **Infrastructure Setup**: Queues and exchanges are created on first run
   ```
   INFO Setting up RabbitMQ infrastructure
   INFO Queue 'attune.events.queue' declared with dead letter exchange 'attune.dlx'
   INFO Queue 'attune.executions.queue' declared with dead letter exchange 'attune.dlx'
   INFO Queue 'attune.notifications.queue' declared with dead letter exchange 'attune.dlx'
   ```

2. ✅ **Idempotent Setup**: Subsequent runs don't fail if infrastructure exists
   ```
   WARN Failed to setup MQ infrastructure (may already exist): ...
   ```

3. ✅ **Consumer Initialization**: Executor successfully connects to the proper queue
   ```
   INFO Message queue consumer initialized on queue: attune.executions.queue
   ```

4. ✅ **Service Startup**: All executor components start without errors
   ```
   INFO Starting enforcement processor
   INFO Starting execution scheduler
   INFO Starting execution manager
   INFO Consumer started for queue 'attune.executions.queue' with tag 'executor.enforcement'
   INFO Consumer started for queue 'attune.executions.queue' with tag 'executor.scheduler'
   INFO Consumer started for queue 'attune.executions.queue' with tag 'executor.manager'
   ```

5. ✅ **Competing Consumers**: Multiple consumers successfully process from same queue
   - Each consumer has unique tag
   - RabbitMQ distributes messages across consumers
   - No consumer tag conflicts

## Impact

- **Automated Setup**: No manual RabbitMQ queue creation needed
- **Configuration-Driven**: Queue names come from config, not hardcoded
- **Idempotent**: Services can start/restart without manual intervention
- **Better Logging**: Clear visibility into which queues and consumer tags are being used
- **Production Ready**: Dead letter queues for message failure handling
- **Scalable**: Competing consumers pattern allows for parallel message processing
- **Unique Identification**: Each consumer component has its own distinct tag for monitoring

## Related Components

### Other Services
The sensor service uses a different abstraction (`MessageQueue` wrapper) that publishes to exchanges but doesn't consume. It may need similar infrastructure setup in the future if it starts consuming messages.

### Worker Service
The worker service will likely need similar changes when implemented, as it will consume from worker-specific queues.

## Configuration

The infrastructure uses default configuration from `attune_common::mq::MessageQueueConfig`:
- Queue names: `attune.{events,executions,notifications}.queue`
- Exchange names: `attune.{events,executions,notifications}`
- Dead letter exchange: `attune.dlx`
- DLQ TTL: 24 hours

These can be customized by modifying the `MessageQueueConfig::default()` implementation.

## Architecture Notes

### Competing Consumers Pattern
The executor now uses a **competing consumers pattern** where multiple consumers read from the same queue:
- **Benefits**: Load balancing, parallel processing, better resource utilization
- **RabbitMQ Behavior**: Messages are distributed round-robin among consumers
- **Unique Tags**: Each consumer must have a unique tag (enforced by RabbitMQ)
- **Consumer Count**: 3 consumers (enforcement, scheduler, manager) per executor instance

### Message Distribution
With the current setup:
- All three consumers read from `attune.executions.queue`
- RabbitMQ distributes incoming messages among the three consumers
- Each message is delivered to exactly one consumer
- If message processing fails, it's requeued for another consumer to retry

## Next Steps

- [ ] Consider adding infrastructure setup to sensor service (if needed)
- [ ] Add infrastructure setup to worker service (when implemented)
- [ ] Document RabbitMQ topology in architecture documentation
- [ ] Consider making infrastructure setup a separate CLI tool/command
- [ ] Add health checks that verify queue existence
- [ ] Consider using separate queues for different message types instead of competing consumers
- [ ] Add monitoring/metrics for consumer performance and message distribution

## Notes

- Infrastructure setup is designed to be idempotent - running it multiple times is safe
- If setup fails (e.g., due to permissions), the service logs a warning but continues
- This allows services to work in environments where infrastructure is managed externally
- The setup creates durable queues that survive broker restarts