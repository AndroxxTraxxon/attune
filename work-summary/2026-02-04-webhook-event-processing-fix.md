# Webhook Event Processing Fix

**Date**: 2026-02-04  
**Issue**: Webhook events were not triggering rule processing  
**Status**: ✅ Fixed and Verified

## Problem Description

When webhooks were submitted to triggers (e.g., `default.example`), the events were being created in the database successfully, but they were not engaging with any rules. The events didn't specify a specific rule when created (as expected), so they should have matched against all rules subscribing to that trigger. However, the executor service never processed these events.

## Root Cause

The webhook receiver endpoint (`/api/v1/webhooks/{trigger_ref}`) was creating events in the database but **was not publishing `EventCreated` messages** to the RabbitMQ message queue. This meant the executor service had no notification that a new event existed and therefore could not:

1. Find matching rules for the event's trigger
2. Evaluate rule conditions
3. Create enforcements
4. Schedule executions

In contrast, the regular event creation endpoint (`POST /api/v1/events`) was correctly publishing `EventCreated` messages after creating events.

## Solution

Added `EventCreated` message publishing to the webhook receiver endpoint to match the behavior of the regular event creation endpoint.

### Changes Made

**File**: `attune/crates/api/src/routes/webhooks.rs`

1. **Added imports** for message queue types:
   ```rust
   use attune_common::{
       mq::{EventCreatedPayload, MessageEnvelope, MessageType},
       repositories::{
           event::{CreateEventInput, EventRepository},
           trigger::{TriggerRepository, WebhookEventLogInput},
           Create, FindById, FindByRef,
       },
   };
   ```

2. **Added message publishing logic** after event creation (lines 647-676):
   - Construct `EventCreatedPayload` with event details
   - Create `MessageEnvelope` with source "api-webhook-receiver"
   - Publish to message queue via `publisher.publish_envelope()`
   - Log success/failure appropriately
   - Continue processing even if publishing fails (event already recorded)

## Event Flow (After Fix)

```
Webhook Request → API validates request → Event created in DB →
EventCreated message published to RabbitMQ → Executor receives message →
Finds matching rules (event.rule is None, so matches all enabled rules for trigger) →
Creates enforcements → Schedules executions → Workers execute actions
```

## Verification

The executor service properly handles events without a specific rule:
- When `event.rule` is `None`, the `find_matching_rules()` function matches **all enabled rules** with the same `trigger_ref`
- This logic was already correct in `attune/crates/executor/src/event_processor.rs` (lines 145-153)

## Deployment

Since both API and executor services run in Docker:

```bash
# Rebuild API service with fix
docker compose build api

# Restart API service (must use down/up to pick up new image)
docker compose down api
docker compose up -d api
```

**Important**: Using `docker compose restart` alone may not pick up the new image. Use `down` + `up` to ensure the new image is used.

## Testing

To test the fix:

1. Ensure you have a rule that subscribes to a webhook trigger (e.g., `default.example`)
2. Submit a webhook to the trigger endpoint
3. Verify the event is created and the `EventCreated` message is logged
4. Verify the executor processes the event and creates enforcements
5. Verify executions are scheduled and run

### Verified Example

Submitted webhook with correct payload format:
```bash
curl -X POST http://localhost:8080/api/v1/webhooks/wh_kxuvd5ai4hqrzsoog2kzuz3tcskihjpj \
  -H "Content-Type: application/json" \
  -d '{"payload": {"test": "verify_fix", "timestamp": "2026-02-04T04:34:39Z"}}'
```

**Note**: Webhook payload must be wrapped in a `payload` field per the `WebhookReceiverRequest` DTO.

**API logs confirmed**:
```
attune-api | Webhook event 8581 created, attempting to publish EventCreated message
attune-api | Message 11134cae-6c56-4fb9-8395-babf1ae420cd published successfully to 'attune.events'
attune-api | Published EventCreated message for event 8581 (trigger: default.example)
```

**Executor logs confirmed full flow**:
```
attune-executor | Processing EventCreated for event 8581 (trigger: default.example)
attune-executor | Found 1 matching rule(s) for event 8581
attune-executor | Rule default.example_webhook_rule matched event 8581 - creating enforcement
attune-executor | Enforcement 8564 created for rule default.example_webhook_rule (event: 8581)
attune-executor | Creating execution for enforcement: 8564, rule: 3, action: 1
attune-executor | Execution 8564 scheduled to worker 3
attune-executor | Successfully processed completion for execution: 8564 (action: 1)
```

## Impact

- ✅ **Webhooks now properly trigger rule processing** as expected
- ✅ Events without a specific rule correctly match all enabled rules for the trigger
- ✅ No changes to database schema or message formats
- ✅ No breaking changes to existing functionality
- ✅ Webhook events now behave consistently with manually created events
- ✅ Full event → enforcement → execution flow verified working

## Related Files

- `attune/crates/api/src/routes/webhooks.rs` - Fixed file (added EventCreated publishing)
- `attune/crates/api/src/routes/events.rs` - Reference implementation
- `attune/crates/executor/src/event_processor.rs` - Event processing logic (already correct)
- `attune/scripts/test-webhook-event-processing.sh` - Test script for verification

## Lessons Learned

1. **Log level matters**: Initial implementation used `tracing::debug!` for success case, making it invisible in production logs. Changed to `tracing::info!` for visibility.
2. **Docker image updates**: `docker compose restart` doesn't always pick up new images. Use `docker compose down` + `up` to force image reload.
3. **Webhook payload format**: The webhook endpoint expects `{"payload": {...}}` not bare JSON, per `WebhookReceiverRequest` DTO.