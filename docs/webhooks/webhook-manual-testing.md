# Webhook System Manual Testing Guide

**Last Updated**: 2026-01-20  
**Status**: Phase 2 Complete

This guide provides step-by-step instructions for manually testing the webhook system functionality.

---

## Prerequisites

1. Attune API service running (default: `http://localhost:8080`)
2. Database with migrations applied
3. Test user account registered
4. `curl` or similar HTTP client
5. `jq` for JSON formatting (optional)

---

## Setup Test Environment

### 1. Start the API Service

```bash
cd crates/api
cargo run
```

### 2. Register a Test User (if not already registered)

```bash
curl -X POST http://localhost:8080/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "webhook_test_user",
    "email": "webhook@example.com",
    "password": "test_password_123"
  }'
```

### 3. Login and Get JWT Token

```bash
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "webhook_test_user",
    "password": "test_password_123"
  }' | jq -r '.data.access_token')

echo "Token: $TOKEN"
```

---

## Test Scenario 1: Enable Webhooks for Existing Trigger

### Step 1: Create a Test Pack

```bash
curl -X POST http://localhost:8080/api/v1/packs \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "webhook_test",
    "label": "Webhook Test Pack",
    "description": "Pack for webhook testing",
    "version": "1.0.0",
    "enabled": true
  }' | jq
```

### Step 2: Create a Test Trigger

```bash
curl -X POST http://localhost:8080/api/v1/triggers \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "webhook_test.github_push",
    "pack_ref": "webhook_test",
    "label": "GitHub Push Event",
    "description": "Trigger for GitHub push webhooks",
    "enabled": true,
    "param_schema": {
      "type": "object",
      "properties": {
        "repository": {"type": "string"},
        "branch": {"type": "string"}
      }
    }
  }' | jq
```

### Step 3: Enable Webhooks

```bash
curl -X POST http://localhost:8080/api/v1/triggers/webhook_test.github_push/webhooks/enable \
  -H "Authorization: Bearer $TOKEN" | jq
```

**Expected Response:**
```json
{
  "data": {
    "id": 1,
    "ref": "webhook_test.github_push",
    "label": "GitHub Push Event",
    "webhook_enabled": true,
    "webhook_key": "wh_abc123...xyz789",
    ...
  }
}
```

**Verify:**
- `webhook_enabled` is `true`
- `webhook_key` starts with `wh_`
- `webhook_key` is 40+ characters long

### Step 4: Save Webhook Key

```bash
WEBHOOK_KEY=$(curl -s -X GET http://localhost:8080/api/v1/triggers/webhook_test.github_push \
  -H "Authorization: Bearer $TOKEN" | jq -r '.data.webhook_key')

echo "Webhook Key: $WEBHOOK_KEY"
```

---

## Test Scenario 2: Send Webhook Events

### Step 1: Send a Basic Webhook

```bash
curl -X POST http://localhost:8080/api/v1/webhooks/$WEBHOOK_KEY \
  -H "Content-Type: application/json" \
  -d '{
    "payload": {
      "repository": "attune/automation-platform",
      "branch": "main",
      "commits": [
        {
          "sha": "abc123",
          "message": "Add webhook support",
          "author": "developer@example.com"
        }
      ]
    }
  }' | jq
```

**Expected Response:**
```json
{
  "data": {
    "event_id": 1,
    "trigger_ref": "webhook_test.github_push",
    "received_at": "2026-01-20T15:30:00Z",
    "message": "Webhook received successfully"
  }
}
```

**Verify:**
- Response status is 200 OK
- `event_id` is a positive integer
- `trigger_ref` matches the trigger
- `received_at` is a valid ISO timestamp

### Step 2: Send Webhook with Metadata

```bash
curl -X POST http://localhost:8080/api/v1/webhooks/$WEBHOOK_KEY \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: push" \
  -H "X-GitHub-Delivery: abc-123-def-456" \
  -d '{
    "payload": {
      "action": "synchronize",
      "number": 42,
      "pull_request": {
        "title": "Add new feature",
        "state": "open"
      }
    },
    "headers": {
      "X-GitHub-Event": "push",
      "X-GitHub-Delivery": "abc-123-def-456"
    },
    "source_ip": "192.30.252.1",
    "user_agent": "GitHub-Hookshot/abc123"
  }' | jq
```

**Verify:**
- Event created successfully
- Metadata is stored in event config

### Step 3: Verify Event Was Created

```bash
curl -X GET http://localhost:8080/api/v1/events \
  -H "Authorization: Bearer $TOKEN" | jq
```

**Expected:** Events list includes the webhook-triggered events

---

## Test Scenario 3: Webhook Key Management

### Step 1: Regenerate Webhook Key

```bash
curl -X POST http://localhost:8080/api/v1/triggers/webhook_test.github_push/webhooks/regenerate \
  -H "Authorization: Bearer $TOKEN" | jq
```

**Expected Response:**
```json
{
  "data": {
    "webhook_enabled": true,
    "webhook_key": "wh_new_key_different_from_old",
    ...
  }
}
```

**Verify:**
- New `webhook_key` is different from the old one
- Still starts with `wh_`

### Step 2: Verify Old Key No Longer Works

```bash
# Try to use the old webhook key (should fail)
curl -X POST http://localhost:8080/api/v1/webhooks/$WEBHOOK_KEY \
  -H "Content-Type: application/json" \
  -d '{"payload": {"test": "data"}}' | jq
```

**Expected:** 404 Not Found - "Invalid webhook key"

### Step 3: Get New Key and Test

```bash
NEW_WEBHOOK_KEY=$(curl -s -X GET http://localhost:8080/api/v1/triggers/webhook_test.github_push \
  -H "Authorization: Bearer $TOKEN" | jq -r '.data.webhook_key')

curl -X POST http://localhost:8080/api/v1/webhooks/$NEW_WEBHOOK_KEY \
  -H "Content-Type: application/json" \
  -d '{"payload": {"test": "with new key"}}' | jq
```

**Expected:** 200 OK - Event created successfully

---

## Test Scenario 4: Disable Webhooks

### Step 1: Disable Webhooks

```bash
curl -X POST http://localhost:8080/api/v1/triggers/webhook_test.github_push/webhooks/disable \
  -H "Authorization: Bearer $TOKEN" | jq
```

**Expected Response:**
```json
{
  "data": {
    "webhook_enabled": false,
    "webhook_key": null,
    ...
  }
}
```

**Verify:**
- `webhook_enabled` is `false`
- `webhook_key` is `null`

### Step 2: Verify Webhook No Longer Accepts Events

```bash
curl -X POST http://localhost:8080/api/v1/webhooks/$NEW_WEBHOOK_KEY \
  -H "Content-Type: application/json" \
  -d '{"payload": {"test": "should fail"}}' | jq
```

**Expected:** 404 Not Found - "Invalid webhook key"

---

## Test Scenario 5: Error Handling

### Test 1: Invalid Webhook Key

```bash
curl -X POST http://localhost:8080/api/v1/webhooks/wh_invalid_key_xyz \
  -H "Content-Type: application/json" \
  -d '{"payload": {}}' | jq
```

**Expected:** 404 Not Found

### Test 2: Regenerate Without Enabling First

```bash
# Create new trigger without webhooks
curl -X POST http://localhost:8080/api/v1/triggers \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "webhook_test.no_webhook",
    "pack_ref": "webhook_test",
    "label": "No Webhook Trigger",
    "enabled": true
  }' | jq

# Try to regenerate without enabling
curl -X POST http://localhost:8080/api/v1/triggers/webhook_test.no_webhook/webhooks/regenerate \
  -H "Authorization: Bearer $TOKEN" | jq
```

**Expected:** 400 Bad Request - "Webhooks are not enabled for this trigger"

### Test 3: Management Endpoints Without Auth

```bash
curl -X POST http://localhost:8080/api/v1/triggers/webhook_test.github_push/webhooks/enable | jq
```

**Expected:** 401 Unauthorized

---

## Test Scenario 6: Integration with Rules

### Step 1: Create a Rule for Webhook Trigger

```bash
# First, create a test action
curl -X POST http://localhost:8080/api/v1/actions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "webhook_test.log_event",
    "pack_ref": "webhook_test",
    "label": "Log Webhook Event",
    "description": "Logs webhook events",
    "entrypoint": "echo \"Webhook received: {{event.payload}}\"",
    "runtime_ref": "shell",
    "enabled": true
  }' | jq

# Create rule
curl -X POST http://localhost:8080/api/v1/rules \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "webhook_test.on_github_push",
    "pack_ref": "webhook_test",
    "label": "On GitHub Push",
    "description": "Execute action when GitHub push webhook received",
    "trigger_ref": "webhook_test.github_push",
    "action_ref": "webhook_test.log_event",
    "enabled": true
  }' | jq
```

### Step 2: Re-enable Webhooks

```bash
curl -X POST http://localhost:8080/api/v1/triggers/webhook_test.github_push/webhooks/enable \
  -H "Authorization: Bearer $TOKEN" | jq

WEBHOOK_KEY=$(curl -s -X GET http://localhost:8080/api/v1/triggers/webhook_test.github_push \
  -H "Authorization: Bearer $TOKEN" | jq -r '.data.webhook_key')
```

### Step 3: Send Webhook and Verify Rule Execution

```bash
# Send webhook
curl -X POST http://localhost:8080/api/v1/webhooks/$WEBHOOK_KEY \
  -H "Content-Type: application/json" \
  -d '{
    "payload": {
      "repository": "test/repo",
      "branch": "main"
    }
  }' | jq

# Wait a moment for processing, then check executions
sleep 2

curl -X GET http://localhost:8080/api/v1/executions \
  -H "Authorization: Bearer $TOKEN" | jq
```

**Expected:** Execution created for the `webhook_test.log_event` action

---

## Verification Checklist

### Webhook Enablement
- [ ] Can enable webhooks for a trigger
- [ ] Webhook key is generated and returned
- [ ] Webhook key format is correct (starts with `wh_`)
- [ ] `webhook_enabled` field is `true`

### Webhook Receiver
- [ ] Can send webhook with basic payload
- [ ] Can send webhook with metadata
- [ ] Event is created in database
- [ ] Event has webhook metadata in config
- [ ] Response includes event ID and trigger reference

### Webhook Management
- [ ] Can regenerate webhook key
- [ ] Old key stops working after regeneration
- [ ] New key works immediately
- [ ] Can disable webhooks
- [ ] Disabled webhooks return 404

### Error Handling
- [ ] Invalid webhook key returns 404
- [ ] Regenerate without enabling returns 400
- [ ] Management endpoints require authentication
- [ ] Disabled trigger webhooks return 404

### Integration
- [ ] Webhook creates event
- [ ] Event triggers rule evaluation
- [ ] Rule creates execution
- [ ] Execution runs action

---

## Cleanup

```bash
# Delete test rule
curl -X DELETE http://localhost:8080/api/v1/rules/webhook_test.on_github_push \
  -H "Authorization: Bearer $TOKEN"

# Delete test action
curl -X DELETE http://localhost:8080/api/v1/actions/webhook_test.log_event \
  -H "Authorization: Bearer $TOKEN"

# Delete test triggers
curl -X DELETE http://localhost:8080/api/v1/triggers/webhook_test.github_push \
  -H "Authorization: Bearer $TOKEN"

curl -X DELETE http://localhost:8080/api/v1/triggers/webhook_test.no_webhook \
  -H "Authorization: Bearer $TOKEN"

# Delete test pack
curl -X DELETE http://localhost:8080/api/v1/packs/webhook_test \
  -H "Authorization: Bearer $TOKEN"
```

---

## Troubleshooting

### Webhook Key Not Generated
- Verify database migration applied
- Check API logs for errors
- Ensure trigger exists before enabling webhooks

### Webhook Not Creating Event
- Verify webhook key is correct
- Check that trigger has `webhook_enabled = true`
- Ensure payload is valid JSON
- Check API logs for errors

### Rule Not Executing
- Verify rule is enabled
- Check that rule's trigger matches webhook trigger
- Review executor service logs
- Ensure worker service is running

---

## Next Steps

After verifying Phase 2 functionality:
1. Implement Phase 3 features (HMAC, rate limiting)
2. Build Web UI for webhook management
3. Add webhook event history and analytics
4. Create example packs using webhooks (GitHub, Stripe, etc.)