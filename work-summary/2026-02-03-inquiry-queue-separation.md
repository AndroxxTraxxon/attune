# Work Summary: Inquiry Queue Separation Fix

**Date:** 2026-02-03  
**Issues:** 
- Executor deserialization error: "missing field `inquiry_id`"
- Executor deserialization error: "missing field `action_id`"

**Status:** ✅ Both Fixed

## Visual Overview

### Before Fix ❌
```
attune.execution.status.queue
    ├─ Consumer: CompletionListener (expects ExecutionCompletedPayload)
    ├─ Consumer: ExecutionManager (expects ExecutionStatusPayload)
    └─ Consumer: InquiryHandler (expects InquiryRespondedPayload)
    
    Incoming Messages:
    - execution.completed → ExecutionCompletedPayload
    - execution.status.changed → ExecutionStatusChangedPayload
    - inquiry.responded → InquiryRespondedPayload
    
    Problem: Round-robin distribution causes wrong consumer to receive wrong message type!
```

### After Fix ✅
```
attune.execution.completed.queue
    └─ Consumer: CompletionListener (expects ExecutionCompletedPayload)
    └─ Message: execution.completed → ExecutionCompletedPayload ✓

attune.execution.status.queue
    └─ Consumer: ExecutionManager (expects ExecutionStatusPayload)
    └─ Message: execution.status.changed → ExecutionStatusChangedPayload ✓

attune.inquiry.responses.queue
    └─ Consumer: InquiryHandler (expects InquiryRespondedPayload)
    └─ Message: inquiry.responded → InquiryRespondedPayload ✓
    
    Result: Each queue has ONE consumer expecting ONE message type!
```

## Problem Description

The executor service was logging deserialization errors when processing messages from the `execution_status` queue:

```
ERROR ThreadId(13) crates/common/src/mq/consumer.rs:112: Failed to deserialize message: missing field `inquiry_id` at line 1 column 318. Rejecting message.
```

## Root Cause Analysis

The issue was caused by **two different consumers listening to the same RabbitMQ queue** but expecting different message payload types:

### Queue Configuration Issue

The `execution_status` queue (`attune.execution.status.queue`) was bound to the `attune.executions` exchange with routing key `"execution.status.changed"`, but it was receiving messages with two different routing keys:

1. **`execution.completed`** → `ExecutionCompletedPayload` (published by Worker service)
2. **`inquiry.responded`** → `InquiryRespondedPayload` (published by API service)

### Competing Consumers

Two consumers were configured to read from the same `execution_status` queue:

1. **CompletionListener** (`executor.completion` tag)
   - Expected: `ExecutionCompletedPayload` 
   - Fields: `execution_id`, `action_id`, `action_ref`, `status`, `result`, `completed_at`

2. **InquiryHandler** (`executor.inquiry` tag)
   - Expected: `InquiryRespondedPayload`
   - Fields: `inquiry_id`, `execution_id`, `response`, `responded_by`, `responded_at`

### Message Routing Behavior

RabbitMQ distributes messages to consumers on the same queue using **round-robin load balancing**. This meant:

- When an `InquiryRespondedPayload` was delivered to `CompletionListener` → **deserialization failed** (missing `inquiry_id`)
- When an `ExecutionCompletedPayload` was delivered to `InquiryHandler` → **deserialization failed** (missing `action_id`)

The error message specifically mentioned `inquiry_id` because `CompletionListener` tried to deserialize an inquiry response message.

## Solution Implemented

### 1. Created Separate Queue for Inquiry Responses

**File:** `attune/crates/common/src/mq/config.rs`

Added a new queue configuration:

```rust
pub struct QueuesConfig {
    // ... existing queues ...
    
    /// Inquiry responses queue configuration
    pub inquiry_responses: QueueConfig,
}
```

Default configurations:
```rust
execution_completed: QueueConfig {
    name: "attune.execution.completed.queue".to_string(),
    durable: true,
    exclusive: false,
    auto_delete: false,
},
inquiry_responses: QueueConfig {
    name: "attune.inquiry.responses.queue".to_string(),
    durable: true,
    exclusive: false,
    auto_delete: false,
}
```

### 2. Updated Infrastructure Setup

**File:** `attune/crates/common/src/mq/connection.rs`

Added queue declarations and bindings in `setup_infrastructure()`:

```rust
// Declare the new queues with DLX support
self.declare_queue_with_dlx(&config.rabbitmq.queues.execution_completed, dlx).await?;
self.declare_queue_with_dlx(&config.rabbitmq.queues.inquiry_responses, dlx).await?;

// Bind execution_status queue to status changed messages for ExecutionManager
self.bind_queue(
    &config.rabbitmq.queues.execution_status.name,
    &config.rabbitmq.exchanges.executions.name,
    "execution.status.changed",
)
.await?;

// Bind execution_completed queue to completed messages for CompletionListener
self.bind_queue(
    &config.rabbitmq.queues.execution_completed.name,
    &config.rabbitmq.exchanges.executions.name,
    "execution.completed",
)
.await?;

// Bind inquiry_responses queue to inquiry responded messages for InquiryHandler
self.bind_queue(
    &config.rabbitmq.queues.inquiry_responses.name,
    &config.rabbitmq.exchanges.executions.name,
    "inquiry.responded",
)
.await?;
```

### 3. Updated Executor Service Configuration

**File:** `attune/crates/executor/src/service.rs`

Changed `InquiryHandler` and `CompletionListener` to consume from dedicated queues:

```rust
// InquiryHandler - Before:
let inquiry_response_queue = self.inner.mq_config.rabbitmq.queues.execution_status.name.clone();

// InquiryHandler - After:
let inquiry_response_queue = self.inner.mq_config.rabbitmq.queues.inquiry_responses.name.clone();

// CompletionListener - Before:
let execution_completed_queue = self.inner.mq_config.rabbitmq.queues.execution_status.name.clone();

// CompletionListener - After:
let execution_completed_queue = self.inner.mq_config.rabbitmq.queues.execution_completed.name.clone();
```

## Message Flow After Fix

### Execution Completion Flow
```
Worker → publishes ExecutionCompletedPayload
       → routing key: "execution.completed"
       → exchange: "attune.executions"
       → queue: "attune.execution.completed.queue"
       → consumer: CompletionListener
       ✅ Correct payload type received
```

### Execution Status Change Flow
```
Worker → publishes ExecutionStatusChangedPayload
       → routing key: "execution.status.changed"
       → exchange: "attune.executions"
       → queue: "attune.execution.status.queue"
       → consumer: ExecutionManager
       ✅ Correct payload type received
```

### Inquiry Response Flow
```
API → publishes InquiryRespondedPayload
    → routing key: "inquiry.responded"
    → exchange: "attune.executions"
    → queue: "attune.inquiry.responses.queue"
    → consumer: InquiryHandler
    ✅ Correct payload type received
```

## Benefits

1. **Type Safety**: Each queue receives only one message type, eliminating deserialization errors
2. **Scalability**: Can scale `CompletionListener`, `ExecutionManager`, and `InquiryHandler` independently
3. **Maintainability**: Clear separation of concerns - each queue has a single purpose
4. **Reliability**: No message rejection due to type mismatches
5. **Performance**: No wasted processing from consumers receiving wrong message types

## Queue Separation Summary

After both fixes, we now have three dedicated queues for execution-related messages:

| Queue | Routing Key | Message Type | Consumer |
|-------|-------------|--------------|----------|
| `attune.execution.status.queue` | `execution.status.changed` | `ExecutionStatusChangedPayload` | ExecutionManager |
| `attune.execution.completed.queue` | `execution.completed` | `ExecutionCompletedPayload` | CompletionListener |
| `attune.inquiry.responses.queue` | `inquiry.responded` | `InquiryRespondedPayload` | InquiryHandler |

**Result:** Each queue now has exactly one consumer expecting exactly one message type. ✅

## Testing Recommendations

1. **Restart all services** to recreate the queue infrastructure with new bindings
2. **Verify queue creation** in RabbitMQ management UI:
   - Check that `attune.inquiry.responses.queue` exists
   - Check that `attune.execution.completed.queue` exists
   - Verify bindings on `attune.executions` exchange:
     - `inquiry.responded` → `attune.inquiry.responses.queue`
     - `execution.completed` → `attune.execution.completed.queue`
     - `execution.status.changed` → `attune.execution.status.queue`
3. **Monitor executor logs** for absence of deserialization errors (`inquiry_id` and `action_id`)
4. **Test inquiry workflow**:
   - Create an action that requests inquiry (`__inquiry` in result)
   - Respond to inquiry via API
   - Verify execution resumes correctly
5. **Test execution completion**:
   - Execute a simple action
   - Verify completion notification processed without errors


### Files Modified

- `attune/crates/common/src/mq/config.rs` - Added `inquiry_responses` and `execution_completed` queues
- `attune/crates/common/src/mq/connection.rs` - Added queue declarations and bindings
- `attune/crates/executor/src/service.rs` - Updated InquiryHandler and CompletionListener to use new queues

## Migration Notes

This is a **breaking change** for existing deployments:

1. Two new queues will be created automatically on service startup:
   - `attune.inquiry.responses.queue`
   - `attune.execution.completed.queue`
2. The `execution_status` queue now has **only one binding** (`execution.status.changed`)
3. Existing messages in queues are unaffected
4. No database migrations required
5. **Action Required**: Restart executor service to apply changes

## Related Issues

- Original implementation assumed a single queue could handle multiple message types
- RabbitMQ round-robin distribution caused non-deterministic deserialization failures
- Errors were intermittent because they depended on which consumer received which message
- `ExecutionManager` uses local payload struct instead of canonical `ExecutionStatusChangedPayload` (not critical but should be unified in future)

## Lessons Learned

1. **One queue, one message type**: RabbitMQ queues should have a single message schema
2. **One queue, one consumer**: Multiple consumers on the same queue creates competition, not cooperation
3. **Use routing keys effectively**: Topic exchanges with specific routing keys provide better message segregation
4. **Consumer tag awareness**: Consumer tags don't prevent round-robin distribution within the same queue
5. **Type-safe patterns**: Rust's strong typing revealed the issue quickly through deserialization errors
6. **Canonical message types**: Use shared message structs from `attune_common::mq::messages`, not local definitions
7. **Incremental fixes**: Sometimes you discover deeper issues while fixing surface-level problems - fix them all at once
8. **Test thoroughly**: Restart services and monitor logs to catch related issues before they reach production