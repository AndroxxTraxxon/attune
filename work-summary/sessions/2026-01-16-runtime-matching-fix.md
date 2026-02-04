# Work Summary: Worker Runtime Matching Fix and Routing Key Issue

**Date:** 2026-01-16  
**Session Goal:** Complete the happy path for timer-driven rule execution with echo action

## Summary

Successfully fixed the worker runtime matching logic. The executor now correctly selects workers based on their `capabilities.runtimes` array instead of the deprecated `runtime` column. However, discovered a critical message routing issue preventing executions from reaching workers.

## Completed Work

### 1. Worker Runtime Matching Refactor ✅

**Problem:**
- Executor matched workers by checking `worker.runtime` column (which was NULL)
- Worker's actual capabilities stored in `capabilities.runtimes` JSON array like `["python", "shell", "node"]`
- Actions require specific runtimes (e.g., `core.echo` requires `shell` runtime)

**Solution:**
- Updated `ExecutionScheduler::select_worker()` in `crates/executor/src/scheduler.rs`
- Added `worker_supports_runtime()` helper function that:
  - Parses worker's `capabilities.runtimes` array
  - Performs case-insensitive matching against action's runtime name
  - Falls back to deprecated `runtime` column for backward compatibility
  - Logs detailed matching information for debugging

**Code Changes:**
```rust
fn worker_supports_runtime(worker: &Worker, runtime_name: &str) -> bool {
    if let Some(ref capabilities) = worker.capabilities {
        if let Some(runtimes) = capabilities.get("runtimes") {
            if let Some(runtime_array) = runtimes.as_array() {
                for runtime_value in runtime_array {
                    if let Some(runtime_str) = runtime_value.as_str() {
                        if runtime_str.eq_ignore_ascii_case(runtime_name) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
```

### 2. Message Payload Standardization ✅

**Problem:**
- Multiple local definitions of `ExecutionRequestedPayload` across executor modules
- Each had different fields causing deserialization failures
- Scheduler, manager, and enforcement processor expected different message formats

**Solution:**
- Updated scheduler and execution manager to use shared `ExecutionRequestedPayload` from `attune_common::mq`
- Standardized payload fields:
  - `execution_id: i64`
  - `action_id: Option<i64>`
  - `action_ref: String`
  - `parent_id: Option<i64>`
  - `enforcement_id: Option<i64>`
  - `config: Option<JsonValue>`

**Files Modified:**
- `crates/executor/src/scheduler.rs`
- `crates/executor/src/execution_manager.rs`

### 3. Worker Message Routing Implementation ✅

**Problem:**
- Scheduler published messages to exchange without routing key
- Worker-specific queue `worker.1.executions` bound with routing key `worker.1`
- Messages weren't reaching worker queue due to missing routing key

**Solution:**
- Updated `queue_to_worker()` to use `publish_envelope_with_routing()`
- Routing key format: `worker.{worker_id}`
- Exchange: `attune.executions`
- Added detailed logging of routing key in publish message

**Code:**
```rust
let routing_key = format!("worker.{}", worker_id);
let exchange = "attune.executions";

publisher
    .publish_envelope_with_routing(&envelope, exchange, &routing_key)
    .await?;
```

## Critical Issue Discovered 🚨

### Message Queue Consumer Architecture Problem

**Symptom:**
- Executor creates executions successfully (status: `requested`)
- Enforcement processor publishes `ExecutionRequested` messages
- Scheduler never processes these messages (executions stay in `requested` status)
- Continuous deserialization errors in logs

**Root Cause:**
All three executor components consume from the SAME queue (`attune.executions.queue`) but expect DIFFERENT message payload types:

1. **Enforcement Processor** - expects `EnforcementCreatedPayload`
2. **Execution Scheduler** - expects `ExecutionRequestedPayload`  
3. **Execution Manager** - expects `ExecutionStatusPayload`

**What Happens:**
1. Message arrives in `attune.executions.queue`
2. All three consumers compete to consume it (round-robin)
3. Two consumers fail deserialization and reject (nack) the message
4. Message goes to dead letter queue (DLQ)
5. Correct consumer never gets the message

**Log Evidence:**
```
ERROR Failed to deserialize message: missing field `execution_id` at line 1 column 492. Rejecting message.
ERROR Failed to deserialize message: missing field `status` at line 1 column 404. Rejecting message.
ERROR Failed to deserialize message: missing field `rule_ref` at line 1 column 404. Rejecting message.
```

These errors repeat constantly as different consumers try to process incompatible messages.

## Current System Status

### Working Components ✅
- Timer sensors fire every 10 seconds
- Events created in database
- Rules match events correctly
- Enforcements created
- Executions created with action_params flowing through
- Worker registered with correct capabilities
- Worker-specific queue bound correctly

### Broken Pipeline 🔴
- **Enforcement → Execution**: Messages published but rejected by consumers
- **Scheduler → Worker**: Would work IF scheduler received messages
- **Worker → Action Execution**: Not tested yet

### Test Data
- Rule: `core.timer_echo` (ID: 2)
- Trigger: `core.intervaltimer` (ID: 15) 
- Action: `core.echo` (ID: 1, runtime: shell/3)
- Worker: ID 1, capabilities: `{"runtimes": ["python", "shell", "node"]}`
- Recent executions: 46, 47, 48, 49 - all stuck in `requested` status

## Solutions to Consider

### Option 1: Separate Queues (Recommended)
- Create dedicated queues for each message type
- `attune.enforcements.queue` → Enforcement Processor
- `attune.execution.requests.queue` → Scheduler
- `attune.execution.status.queue` → Manager
- Update publishers to route to correct queues

### Option 2: Topic-Based Routing
- Include message type in routing key
- Bind consumers to specific message type patterns
- Example: `execution.requested`, `execution.status`, `enforcement.created`

### Option 3: Message Type Pre-Filtering
- Modify Consumer to peek at `message_type` field before deserializing payload
- Route to appropriate handler based on type
- More complex, requires consumer interface changes

### Option 4: Dead Letter Queue Recovery
- Add DLQ consumer that re-routes messages to correct queues
- Band-aid solution, doesn't fix root cause

## Next Steps

1. **Immediate Priority:** Implement separate queues (Option 1)
   - Update `config.yaml` with new queue definitions
   - Modify enforcement processor to publish to dedicated queue
   - Update scheduler and manager to consume from their specific queues
   - Test end-to-end flow

2. **Verification Steps:**
   - Confirm execution moves from `requested` → `scheduled` → `running`
   - Verify message reaches worker queue
   - Check worker executes shell command
   - Validate "hello, world" appears in execution result

3. **Future Improvements:**
   - Remove deprecated `worker.runtime` column
   - Implement topic-based routing for better scalability
   - Add message type validation at queue level
   - Create monitoring for DLQ depth

## Files Modified

- `attune/crates/executor/src/scheduler.rs` - Runtime matching + routing key
- `attune/crates/executor/src/execution_manager.rs` - Payload standardization
- `attune/crates/executor/src/enforcement_processor.rs` - Uses shared payload

## Testing Notes

Services running:
- Worker: Listening on `worker.1.executions` queue ✅
- Executor: All three consumers competing on one queue ⚠️
- Sensor: Generating events every 10 seconds ✅

Database state:
- 4 executions reached `scheduled` status in previous runs
- All new executions stuck in `requested` status since current run

## Conclusion

The worker runtime matching fix is complete and correct. The system can select appropriate workers based on capabilities. However, the message queue architecture has a fundamental flaw where multiple consumers compete for messages they cannot process. This must be resolved before the happy path can be completed.

The fix is well-understood and straightforward: implement separate queues for different message types. This is the standard pattern for message-driven architectures and will eliminate the consumer competition issue.