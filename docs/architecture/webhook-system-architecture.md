# Webhook System Architecture

**Last Updated**: 2026-01-20  
**Status**: Phase 3 Complete - Advanced Security Features Implemented

---

## Overview

Attune provides built-in webhook support as a first-class feature of the trigger system. Any trigger can be webhook-enabled, allowing external systems to fire events by posting to a unique webhook URL. This eliminates the need for generic webhook triggers and provides better security and traceability.

---

## Core Concepts

### Webhook-Enabled Triggers

Any trigger in Attune can have webhooks enabled:

1. **Pack declares trigger** (e.g., `github.push`, `stripe.payment_succeeded`)
2. **User enables webhooks** via toggle in UI or API
3. **System generates unique webhook key** (secure random token)
4. **Webhook URL is provided** for external system configuration
5. **External system POSTs to webhook URL** with payload
6. **Attune creates event** from webhook payload
7. **Rules evaluate normally** against the event

### Key Benefits

- **Per-Trigger Security**: Each trigger has its own unique webhook key
- **No Generic Triggers**: Webhooks are a feature, not a trigger type
- **Better Traceability**: Clear association between webhook and trigger
- **Flexible Payloads**: Each trigger defines its own payload schema
- **Multi-Tenancy Ready**: Webhook keys can be scoped to identities/organizations

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    External Systems                         │
├─────────────────────────────────────────────────────────────┤
│  GitHub  │  Stripe  │  Slack  │  Custom Apps  │  etc.       │
└────┬──────────┬──────────┬──────────┬───────────────────────┘
     │          │          │          │
     │ POST     │ POST     │ POST     │ POST
     │          │          │          │
     ▼          ▼          ▼          ▼
┌─────────────────────────────────────────────────────────────┐
│         Attune API - Webhook Receiver Endpoint              │
│    POST /api/v1/webhooks/:webhook_key                       │
├─────────────────────────────────────────────────────────────┤
│  1. Validate webhook key                                    │
│  2. Look up associated trigger                              │
│  3. Parse and validate payload                              │
│  4. Create event in database                                │
│  5. Return 200 OK                                           │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              PostgreSQL Database                            │
│  ┌────────┐    ┌───────┐    ┌───────┐                       │
│  │Trigger │───▶│ Event │───▶│ Rule  │                       │
│  │webhook │    │       │    │       │                       │
│  │enabled │    │       │    │       │                       │
│  │webhook │    │       │    │       │                       │
│  │  key   │    │       │    │       │                       │
│  └────────┘    └───────┘    └───────┘                       │
└─────────────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Rule Evaluation                          │
│                    Execution Scheduling                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Database Schema

### Existing `attune.trigger` Table Extensions

Add webhook-related columns to the trigger table:

```sql
ALTER TABLE attune.trigger ADD COLUMN IF NOT EXISTS
    webhook_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE attune.trigger ADD COLUMN IF NOT EXISTS
    webhook_key VARCHAR(64) UNIQUE;

ALTER TABLE attune.trigger ADD COLUMN IF NOT EXISTS
    webhook_secret VARCHAR(128);  -- For HMAC signature verification (optional)

-- Index for fast webhook key lookup
CREATE INDEX IF NOT EXISTS idx_trigger_webhook_key 
    ON attune.trigger(webhook_key) 
    WHERE webhook_key IS NOT NULL;
```

### Webhook Event Metadata

Events created from webhooks include additional metadata:

```json
{
  "source": "webhook",
  "webhook_key": "wh_abc123...",
  "webhook_metadata": {
    "received_at": "2024-01-20T12:00:00Z",
    "source_ip": "192.168.1.100",
    "user_agent": "GitHub-Hookshot/abc123",
    "headers": {
      "X-GitHub-Event": "push",
      "X-GitHub-Delivery": "12345-67890"
    }
  },
  "payload": {
    // Original webhook payload from external system
  }
}
```

---

## API Endpoints

### Webhook Receiver

**Receive Webhook Event**

```http
POST /api/v1/webhooks/:webhook_key
Content-Type: application/json

{
  "ref": "refs/heads/main",
  "commits": [...],
  "repository": {...}
}
```

**Response (Success)**
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "success": true,
  "event_id": 12345,
  "trigger_ref": "github.push",
  "message": "Event created successfully"
}
```

**Response (Invalid Key)**
```http
HTTP/1.1 404 Not Found
Content-Type: application/json

{
  "success": false,
  "error": "Invalid webhook key"
}
```

**Response (Disabled)**
```http
HTTP/1.1 403 Forbidden
Content-Type: application/json

{
  "success": false,
  "error": "Webhooks are disabled for this trigger"
}
```

### Webhook Management

**Enable Webhooks for Trigger**

```http
POST /api/v1/triggers/:id/webhook/enable
Authorization: Bearer <token>
```

**Response**
```json
{
  "data": {
    "id": 123,
    "ref": "github.push",
    "webhook_enabled": true,
    "webhook_key": "wh_abc123xyz789...",
    "webhook_url": "https://attune.example.com/api/v1/webhooks/wh_abc123xyz789..."
  }
}
```

**Disable Webhooks for Trigger**

```http
POST /api/v1/triggers/:id/webhook/disable
Authorization: Bearer <token>
```

**Regenerate Webhook Key**

```http
POST /api/v1/triggers/:id/webhook/regenerate
Authorization: Bearer <token>
```

**Response**
```json
{
  "data": {
    "webhook_key": "wh_new_key_here...",
    "webhook_url": "https://attune.example.com/api/v1/webhooks/wh_new_key_here...",
    "previous_key_revoked": true
  }
}
```

**Get Webhook Info**

```http
GET /api/v1/triggers/:id/webhook
Authorization: Bearer <token>
```

**Response**
```json
{
  "data": {
    "enabled": true,
    "webhook_key": "wh_abc123xyz789...",
    "webhook_url": "https://attune.example.com/api/v1/webhooks/wh_abc123xyz789...",
    "created_at": "2024-01-20T10:00:00Z",
    "last_used_at": "2024-01-20T12:30:00Z",
    "total_events": 145
  }
}
```

---

## Webhook Key Format

Webhook keys use a recognizable prefix and secure random suffix:

```
wh_[32 random alphanumeric characters]
```

**Example**: `wh_k7j2n9p4m8q1r5w3x6z0a2b5c8d1e4f7`

**Generation (Rust)**:
```rust
use rand::Rng;
use rand::distributions::Alphanumeric;

fn generate_webhook_key() -> String {
    let random_part: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    
    format!("wh_{}", random_part)
}
```

---

## Security Considerations

### 1. Webhook Key as Bearer Token

The webhook key acts as a bearer token - anyone with the key can post events. Therefore:

- Keys must be long and random (32+ characters)
- Keys must be stored securely
- Keys should be transmitted over HTTPS only
- Keys can be regenerated if compromised

### 2. Optional Signature Verification

For enhanced security, triggers can require HMAC signature verification:

```http
POST /api/v1/webhooks/:webhook_key
X-Webhook-Signature: sha256=abc123...
Content-Type: application/json

{...}
```

The signature is computed as:
```
HMAC-SHA256(webhook_secret, request_body)
```

This prevents replay attacks and ensures payload integrity.

### 3. IP Whitelisting (Future)

Triggers can optionally restrict webhooks to specific IP ranges:

```json
{
  "webhook_enabled": true,
  "webhook_ip_whitelist": [
    "192.30.252.0/22",  // GitHub
    "185.199.108.0/22"   // GitHub
  ]
}
```

### 4. Rate Limiting

Apply rate limits to prevent abuse:

- Per webhook key: 100 requests per minute
- Per IP address: 1000 requests per minute
- Global: 10,000 requests per minute

### 5. Payload Size Limits

Limit webhook payload sizes:

- Maximum payload size: 1 MB
- Reject larger payloads with 413 Payload Too Large

---

## Event Creation from Webhooks

### Event Structure

```sql
INSERT INTO attune.event (
    trigger,
    trigger_ref,
    payload,
    metadata,
    source
) VALUES (
    <trigger_id>,
    <trigger_ref>,
    <webhook_payload>,
    jsonb_build_object(
        'source', 'webhook',
        'webhook_key', <webhook_key>,
        'received_at', NOW(),
        'source_ip', <client_ip>,
        'user_agent', <user_agent>,
        'headers', <selected_headers>
    ),
    'webhook'
);
```

### Payload Transformation

Webhooks can optionally transform payloads before creating events:

1. **Direct Pass-Through** (default): Entire webhook body becomes event payload
2. **JSONPath Extraction**: Extract specific fields from webhook payload
3. **Template Transformation**: Use templates to reshape payload

**Example (JSONPath)**:
```json
{
  "webhook_payload_mapping": {
    "commit_sha": "$.head_commit.id",
    "branch": "$.ref",
    "author": "$.head_commit.author.name"
  }
}
```

---

## Web UI Integration

### Trigger Detail Page

Display webhook status for each trigger:

```
┌──────────────────────────────────────────────────────────┐
│ Trigger: github.push                                      │
├──────────────────────────────────────────────────────────┤
│                                                           │
│ Webhooks  [Toggle: ● ON ]                                │
│                                                           │
│ Webhook URL:                                              │
│ ┌──────────────────────────────────────────────────────┐ │
│ │ https://attune.example.com/api/v1/webhooks/wh_k7j... │ │
│ └──────────────────────────────────────────────────────┘ │
│ [Copy URL] [Show Key] [Regenerate]                       │
│                                                           │
│ Stats:                                                    │
│   • Events received: 145                                  │
│   • Last event: 2 minutes ago                             │
│   • Created: 2024-01-15 10:30:00                          │
│                                                           │
│ Configuration:                                            │
│   □ Require signature verification                        │
│   □ Enable IP whitelisting                                │
│                                                           │
└──────────────────────────────────────────────────────────┘
```

### Webhook Key Display

Show webhook key with copy button and security warning:

```
┌──────────────────────────────────────────────────────────┐
│ Webhook Key                                               │
├──────────────────────────────────────────────────────────┤
│                                                           │
│ wh_k7j2n9p4m8q1r5w3x6z0a2b5c8d1e4f7g9h2               │
│                                                           │
│ [Copy Key] [Hide]                                         │
│                                                           │
│ ⚠️  Keep this key secret. Anyone with this key can       │
│    trigger events. If compromised, regenerate            │
│    immediately.                                           │
│                                                           │
└──────────────────────────────────────────────────────────┘
```

---

## Implementation Status

### ✅ Phase 1: Database & Core (Complete)

1. ✅ Add webhook columns to `attune.trigger` table
2. ✅ Create migration with indexes
3. ✅ Add webhook key generation function
4. ✅ Update trigger repository with webhook methods
5. ✅ All integration tests passing (6/6)

### ✅ Phase 2: API Endpoints (Complete)

1. ✅ Webhook receiver endpoint: `POST /api/v1/webhooks/:webhook_key`
2. ✅ Webhook management endpoints:
   - `POST /api/v1/triggers/:ref/webhooks/enable`
   - `POST /api/v1/triggers/:ref/webhooks/disable`
   - `POST /api/v1/triggers/:ref/webhooks/regenerate`
3. ✅ Event creation logic with webhook metadata
4. ✅ Error handling and validation
5. ✅ OpenAPI documentation
6. ✅ Integration tests created

**Files Added/Modified:**
- `crates/api/src/routes/webhooks.rs` - Webhook routes implementation
- `crates/api/src/dto/webhook.rs` - Webhook DTOs
- `crates/api/src/dto/trigger.rs` - Added webhook fields to TriggerResponse
- `crates/api/src/openapi.rs` - Added webhook endpoints to OpenAPI spec
- `crates/api/tests/webhook_api_tests.rs` - Comprehensive integration tests

### ✅ Phase 3: Advanced Security Features (Complete)

1. ✅ HMAC signature verification (SHA256, SHA512, SHA1)
2. ✅ Rate limiting per webhook key with configurable windows
3. ✅ IP whitelist support with CIDR notation
4. ✅ Payload size limits (configurable per trigger)
5. ✅ Webhook event logging for audit and analytics
6. ✅ Database functions for security configuration
7. ✅ Repository methods for all Phase 3 features
8. ✅ Enhanced webhook receiver with security checks
9. ✅ Comprehensive error handling and logging

**Database Schema Extensions:**
- `webhook_hmac_enabled`, `webhook_hmac_secret`, `webhook_hmac_algorithm` columns
- `webhook_rate_limit_enabled`, `webhook_rate_limit_requests`, `webhook_rate_limit_window_seconds` columns
- `webhook_ip_whitelist_enabled`, `webhook_ip_whitelist` columns
- `webhook_payload_size_limit_kb` column
- `webhook_event_log` table for audit trail
- `webhook_rate_limit` table for rate limit tracking
- `webhook_stats_detailed` view for analytics

**Repository Methods Added:**
- `enable_webhook_hmac()` - Enable HMAC with secret generation
- `disable_webhook_hmac()` - Disable HMAC verification
- `configure_webhook_rate_limit()` - Configure rate limiting
- `configure_webhook_ip_whitelist()` - Configure IP whitelist
- `check_webhook_rate_limit()` - Check if request within limit
- `check_webhook_ip_whitelist()` - Verify IP against whitelist
- `log_webhook_event()` - Log webhook requests for analytics

**Security Module:**
- HMAC signature verification for SHA256, SHA512, SHA1
- Constant-time comparison for signatures
- CIDR notation support for IP whitelists (IPv4 and IPv6)
- Signature format: `sha256=<hex>` or just `<hex>`
- Headers: `X-Webhook-Signature` or `X-Hub-Signature-256`

**Webhook Receiver Enhancements:**
- Payload size limit enforcement (returns 413 if exceeded)
- IP whitelist validation (returns 403 if not allowed)
- Rate limit enforcement (returns 429 if exceeded)
- HMAC signature verification (returns 401 if invalid)
- Comprehensive event logging for all requests (success and failure)
- Processing time tracking
- Detailed error messages with proper HTTP status codes

**Files Added/Modified:**
- `attune/migrations/20260120000002_webhook_advanced_features.sql` (362 lines)
- `crates/common/src/models.rs` - Added Phase 3 fields and WebhookEventLog model
- `crates/common/src/repositories/trigger.rs` - Added Phase 3 methods (215 lines)
- `crates/api/src/webhook_security.rs` - HMAC and IP validation (274 lines)
- `crates/api/src/routes/webhooks.rs` - Enhanced receiver with security (350+ lines)
- `crates/api/src/middleware/error.rs` - Added TooManyRequests error type
- `crates/api/Cargo.toml` - Added hmac, sha1, sha2, hex dependencies

### 📋 Phase 4: Web UI Integration (In Progress)

1. ✅ Add webhook toggle to trigger detail page
2. ✅ Display webhook URL and key
3. ✅ Add copy-to-clipboard functionality
4. Show webhook statistics from `webhook_stats_detailed` view
5. Add regenerate key button with confirmation
6. HMAC configuration UI (enable/disable, view secret)
7. Rate limit configuration UI
8. IP whitelist management UI
9. Webhook event log viewer
10. Real-time webhook testing tool

### 📋 Phase 5: Additional Features (TODO)

1. Webhook retry on failure with exponential backoff
2. Payload transformation/mapping with JSONPath
3. Multiple webhook keys per trigger
4. Webhook health monitoring and alerts
5. Batch webhook processing
6. Webhook response validation
7. Custom header injection
8. Webhook forwarding/proxying

---

## Example Use Cases

### 1. GitHub Push Events

**Pack Definition:**
```yaml
# packs/github/triggers/push.yaml
name: push
ref: github.push
description: "Triggered when code is pushed to a repository"
type: webhook

payload_schema:
  type: object
  properties:
    ref:
      type: string
      description: "Git reference (branch/tag)"
    commits:
      type: array
      description: "Array of commits"
    repository:
      type: object
      description: "Repository information"
```

**User Workflow:**
1. Navigate to trigger `github.push` in UI
2. Enable webhooks (toggle ON)
3. Copy webhook URL
4. Configure in GitHub repository settings:
   - Payload URL: `https://attune.example.com/api/v1/webhooks/wh_abc123...`
   - Content type: `application/json`
   - Events: Just the push event
5. GitHub sends webhook on push
6. Attune creates event
7. Rules evaluate and trigger actions

### 2. Stripe Payment Events

**Pack Definition:**
```yaml
# packs/stripe/triggers/payment_succeeded.yaml
name: payment_succeeded
ref: stripe.payment_succeeded
description: "Triggered when a payment succeeds"
type: webhook

payload_schema:
  type: object
  properties:
    id:
      type: string
    amount:
      type: integer
    currency:
      type: string
    customer:
      type: string
```

**User Workflow:**
1. Enable webhooks for `stripe.payment_succeeded` trigger
2. Copy webhook URL
3. Configure in Stripe dashboard
4. Enable signature verification (recommended for Stripe)
5. Set webhook secret in Attune
6. Stripe sends webhook on successful payment
7. Attune verifies signature and creates event
8. Rules trigger actions (send receipt, update CRM, etc.)

### 3. Custom Application Events

**Pack Definition:**
```yaml
# packs/myapp/triggers/deployment_complete.yaml
name: deployment_complete
ref: myapp.deployment_complete
description: "Triggered when application deployment completes"
type: webhook

payload_schema:
  type: object
  properties:
    environment:
      type: string
      enum: [dev, staging, production]
    version:
      type: string
    deployed_by:
      type: string
    status:
      type: string
      enum: [success, failure]
```

**User Workflow:**
1. Enable webhooks for `myapp.deployment_complete` trigger
2. Get webhook URL
3. Add to CI/CD pipeline:
   ```bash
   curl -X POST https://attune.example.com/api/v1/webhooks/wh_xyz789... \
     -H "Content-Type: application/json" \
     -d '{
       "environment": "production",
       "version": "v2.1.0",
       "deployed_by": "jenkins",
       "status": "success"
     }'
   ```
4. Attune receives webhook and creates event
5. Rules trigger notifications, health checks, etc.

---

## Testing

### Manual Testing

```bash
# Enable webhooks for a trigger
curl -X POST http://localhost:8080/api/v1/triggers/123/webhook/enable \
  -H "Authorization: Bearer $TOKEN"

# Get webhook info
curl http://localhost:8080/api/v1/triggers/123/webhook \
  -H "Authorization: Bearer $TOKEN"

# Send test webhook
WEBHOOK_KEY="wh_k7j2n9p4m8q1r5w3x6z0a2b5c8d1e4f7"
curl -X POST http://localhost:8080/api/v1/webhooks/$WEBHOOK_KEY \
  -H "Content-Type: application/json" \
  -d '{"test": "payload", "value": 123}'

# Verify event was created
curl http://localhost:8080/api/v1/events?limit=1 \
  -H "Authorization: Bearer $TOKEN"
```

### Integration Tests

```rust
#[tokio::test]
async fn test_webhook_enable_disable() {
    // Create trigger
    // Enable webhooks
    // Verify webhook key generated
    // Disable webhooks
    // Verify key removed
}

#[tokio::test]
async fn test_webhook_event_creation() {
    // Enable webhooks for trigger
    // POST to webhook endpoint
    // Verify event created in database
    // Verify event has correct payload and metadata
}

#[tokio::test]
async fn test_webhook_key_regeneration() {
    // Enable webhooks
    // Save original key
    // Regenerate key
    // Verify new key is different
    // Verify old key no longer works
    // Verify new key works
}

#[tokio::test]
async fn test_webhook_invalid_key() {
    // POST to webhook endpoint with invalid key
    // Verify 404 response
    // Verify no event created
}

#[tokio::test]
async fn test_webhook_rate_limiting() {
    // Send 101 requests in 1 minute
    // Verify rate limit exceeded error
}
```

---

## Migration from Generic Webhooks

If Attune previously had generic webhook triggers, migration steps:

1. Create new webhook-enabled triggers for each webhook use case
2. Enable webhooks for new triggers
3. Provide mapping tool in UI to migrate old webhook URLs
4. Run migration script to update external systems
5. Deprecate generic webhook triggers

---

## Performance Considerations

### Webhook Endpoint Optimization

- Async processing: Return 200 OK immediately, process event async
- Connection pooling: Reuse database connections
- Caching: Cache webhook key lookups (with TTL)
- Bulk event creation: Batch multiple webhook events

### Database Indexes

```sql
-- Fast webhook key lookup
CREATE INDEX idx_trigger_webhook_key ON attune.trigger(webhook_key);

-- Webhook event queries
CREATE INDEX idx_event_source ON attune.event(source) WHERE source = 'webhook';
CREATE INDEX idx_event_webhook_key ON attune.event((metadata->>'webhook_key'));
```

---

## Related Documentation

- [Trigger and Sensor Architecture](./trigger-sensor-architecture.md)
- [Event System](./api-events-enforcements.md)
- [Pack Structure](./pack-structure.md)
- [Security Review](./security-review-2024-01-02.md)

---

## Conclusion

Built-in webhook support as a trigger feature provides:

- ✅ Better security with per-trigger webhook keys
- ✅ Clear association between webhooks and triggers
- ✅ Flexible payload handling per trigger type
- ✅ Easy external system integration
- ✅ Full audit trail and traceability

This design eliminates the need for generic webhook triggers while providing a more robust and maintainable webhook system.
