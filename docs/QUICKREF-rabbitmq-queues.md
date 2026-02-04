# RabbitMQ Queue Bindings - Quick Reference

**Last Updated:** 2026-02-03  
**Related Fix:** Queue Separation for InquiryHandler, CompletionListener, and ExecutionManager

## Overview

This document provides a quick reference for understanding RabbitMQ queue bindings in Attune after the inquiry queue separation fix.

## Exchange Topology

Attune uses three main exchanges:

1. **`attune.events`** (Topic) - Event messages from sensors
2. **`attune.executions`** (Topic) - Execution and enforcement messages
3. **`attune.notifications`** (Fanout) - System notifications

## Queue → Routing Key Bindings

### Events Exchange (`attune.events`)

| Queue | Routing Key | Message Type | Consumer |
|-------|-------------|--------------|----------|
| `attune.events.queue` | `#` (all) | `EventCreatedPayload` | EventProcessor (executor) |

### Executions Exchange (`attune.executions`)

| Queue | Routing Key | Message Type | Consumer |
|-------|-------------|--------------|----------|
| `attune.enforcements.queue` | `enforcement.#` | `EnforcementCreatedPayload` | EnforcementProcessor (executor) |
| `attune.execution.requests.queue` | `execution.requested` | `ExecutionRequestedPayload` | ExecutionScheduler (executor) |
| `attune.execution.status.queue` | `execution.status.changed` | `ExecutionStatusChangedPayload` | ExecutionManager (executor) |
| `attune.execution.completed.queue` | `execution.completed` | `ExecutionCompletedPayload` | CompletionListener (executor) |
| `attune.inquiry.responses.queue` | `inquiry.responded` | `InquiryRespondedPayload` | InquiryHandler (executor) |

### Notifications Exchange (`attune.notifications`)

| Queue | Routing Key | Message Type | Consumer |
|-------|-------------|--------------|----------|
| `attune.notifications.queue` | (none - fanout) | `NotificationCreatedPayload` | Various |

## Message Publishers

### Worker Service
- **`execution.status.changed`** → Published during execution with status updates
- **`execution.completed`** → Published when execution finishes (success/failure)

### API Service
- **`inquiry.responded`** → Published when user responds to an inquiry
- **`inquiry.created`** → Published when inquiry is created (executor publishes this)

### Executor Service
- **`enforcement.created`** → Published when rule triggers
- **`execution.requested`** → Published to schedule execution
- **`inquiry.created`** → Published when action requests human input

### Sensor Service
- **`event.created`** → Published when trigger condition is met

## Fixed Issues

### ✅ Fixed: inquiry_id Deserialization Error

**Problem:** InquiryHandler was consuming from `execution.status.queue`, receiving wrong message types (ExecutionCompletedPayload instead of InquiryRespondedPayload).

**Solution:** Created dedicated `attune.inquiry.responses.queue` bound to `inquiry.responded`.

**Status:** Fixed in 2026-02-03.

### ✅ Fixed: action_id Deserialization Error

**Problem:** ExecutionManager was consuming from `execution.status.queue` along with CompletionListener, receiving wrong message types (ExecutionCompletedPayload with action_id instead of ExecutionStatusChangedPayload).

**Solution:** Created dedicated `attune.execution.completed.queue` bound to `execution.completed` for CompletionListener. ExecutionManager now exclusively consumes from `attune.execution.status.queue`.

**Status:** Fixed in 2026-02-03.

## Message Flow Examples

### Execution Completion Flow
```
Worker → ExecutionCompletedPayload
       → routing key: "execution.completed"
       → exchange: "attune.executions"
       → queue: "attune.execution.completed.queue"
       → consumer: CompletionListener (executor)
       → action: Release queue slot, notify waiting executions
```

### Inquiry Response Flow
```
API → InquiryRespondedPayload
    → routing key: "inquiry.responded"
    → exchange: "attune.executions"
    → queue: "attune.inquiry.responses.queue"
    → consumer: InquiryHandler (executor)
    → action: Resume paused execution with inquiry response
```

### Execution Status Update Flow
```
Worker → ExecutionStatusChangedPayload
       → routing key: "execution.status.changed"
       → exchange: "attune.executions"
       → queue: "attune.execution.status.queue"
       → consumer: ExecutionManager (executor)
       → action: Update execution record, trigger child executions
```

## Consumer Tags

Each consumer has a unique tag for identification:

- `executor.event` - EventProcessor
- `executor.enforcement` - EnforcementProcessor
- `executor.scheduler` - ExecutionScheduler
- `executor.completion` - CompletionListener
- `executor.manager` - ExecutionManager
- `executor.inquiry` - InquiryHandler

## Debugging Tips

### Check Queue Bindings
```bash
# Via RabbitMQ Management CLI
rabbitmqadmin list bindings

# Via Management UI
http://localhost:15672 → Exchanges → attune.executions → Bindings

# Check specific queue
rabbitmqadmin list bindings source=attune.executions
```

### Monitor Message Routing
```bash
# View messages in queue
rabbitmqadmin get queue=attune.inquiry.responses.queue count=10

# Check consumer connections
rabbitmqadmin list consumers
```

### Check for Deserialization Errors
```bash
# Grep executor logs
grep "Failed to deserialize message" logs/executor.log

# Look for specific field errors
grep "missing field.*inquiry_id" logs/executor.log
grep "missing field.*action_id" logs/executor.log
```

## Configuration

Queue configuration is defined in:
- **File:** `attune/crates/common/src/mq/config.rs`
- **Struct:** `QueuesConfig`

Bindings are established in:
- **File:** `attune/crates/common/src/mq/connection.rs`
- **Function:** `Connection::setup_infrastructure()`

## Best Practices

1. **One queue, one message type** - Avoid multiple message schemas per queue
2. **One queue, one consumer** - Avoid competing consumers on the same queue
3. **Use specific routing keys** - Prefer `execution.completed` over `execution.#`
4. **Canonical message types** - Use structs from `attune_common::mq::messages`
5. **Monitor dead letter queues** - Check DLQ for routing/deserialization failures

## Summary of Queue Architecture

**Before Fix:** Multiple consumers competing on same queue
- `attune.execution.status.queue` had 3 consumers with 3 different message types ❌

**After Fix:** One queue, one consumer, one message type
- `attune.execution.status.queue` → ExecutionManager only ✅
- `attune.execution.completed.queue` → CompletionListener only ✅
- `attune.inquiry.responses.queue` → InquiryHandler only ✅

## Related Documentation

- `attune/work-summary/2026-02-03-inquiry-queue-separation.md` - Complete fix details
- `attune/docs/architecture/queue-architecture.md` - Overall architecture
- `attune/crates/common/src/mq/messages.rs` - Message type definitions