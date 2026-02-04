# Service Accounts and Transient API Tokens

**Version:** 1.0  
**Last Updated:** 2025-01-27  
**Status:** Draft

## Overview

Service accounts provide programmatic access to the Attune API for sensors, action executions, and other automated processes. Unlike user accounts, service accounts:

- Have no password (token-based authentication only)
- Have limited scopes (principle of least privilege)
- Can be short-lived or long-lived depending on use case
- Are not tied to a human user
- Can be easily revoked without affecting user access

## Use Cases

1. **Sensors**: Long-lived tokens for sensor daemons to emit events
2. **Action Executions**: Short-lived tokens scoped to a single execution
3. **CLI Tools**: User-scoped tokens for command-line operations
4. **Webhooks**: Tokens for external systems to trigger actions
5. **Monitoring**: Tokens for health checks and metrics collection

## Token Types

### 1. Sensor Tokens

**Purpose**: Authentication for sensor daemon processes

**Characteristics**:
- **Lifetime**: Long-lived (90 days, auto-expires)
- **Scope**: `sensor`
- **Permissions**: Create events, read rules/triggers for specific trigger types
- **Revocable**: Yes (manual revocation via API)
- **Renewable**: Yes (automatic refresh via API, no restart required)
- **Rotation**: Automatic (sensor refreshes token when 80% of TTL elapsed)

**Example Usage**:
```bash
ATTUNE_API_TOKEN=sensor_abc123... ./attune-sensor --sensor-ref core.timer
```

### 2. Action Execution Tokens

**Purpose**: Authentication for action scripts during execution

**Characteristics**:
- **Lifetime**: Short-lived (matches execution timeout, typically 5-60 minutes)
- **Scope**: `action_execution`
- **Permissions**: Read keys, update execution status, limited to specific execution_id
- **Revocable**: Yes (auto-revoked on execution completion or timeout)
- **Renewable**: No (single-use, expires when execution completes or times out)
- **Auto-Cleanup**: Token revocation records are auto-deleted after expiration

**Example Usage**:
```python
# Action script receives token via environment variable
import os
import requests

api_url = os.environ['ATTUNE_API_URL']
api_token = os.environ['ATTUNE_API_TOKEN']
execution_id = os.environ['ATTUNE_EXECUTION_ID']

# Fetch encrypted key
response = requests.get(
    f"{api_url}/keys/myapp.api_key",
    headers={"Authorization": f"Bearer {api_token}"}
)
secret = response.json()['value']
```

### 3. User CLI Tokens

**Purpose**: Authentication for CLI tools on behalf of a user

**Characteristics**:
- **Lifetime**: Medium-lived (7-30 days)
- **Scope**: `user`
- **Permissions**: Full user permissions (RBAC-based)
- **Revocable**: Yes
- **Renewable**: Yes (via refresh token)

**Example Usage**:
```bash
attune auth login  # Stores token in ~/.attune/token
attune action execute core.echo --param message="Hello"
```

### 4. Webhook Tokens

**Purpose**: Authentication for external systems calling Attune webhooks

**Characteristics**:
- **Lifetime**: Long-lived (90-365 days, auto-expires)
- **Scope**: `webhook`
- **Permissions**: Trigger specific actions or create events
- **Revocable**: Yes
- **Renewable**: Yes (generate new token before expiration)
- **Rotation**: Recommended every 90 days

**Example Usage**:
```bash
curl -X POST https://attune.example.com/api/webhooks/deploy \
  -H "Authorization: Bearer webhook_xyz789..." \
  -d '{"status": "deployed"}'
```

## Token Scopes and Permissions

| Scope | Permissions | Use Case |
|-------|-------------|----------|
| `admin` | Full access to all resources | System administrators, web UI |
| `user` | RBAC-based permissions | CLI tools, user sessions |
| `sensor` | Create events, read rules/triggers | Sensor daemons |
| `action_execution` | Read keys, update execution (scoped to execution_id) | Action scripts |
| `webhook` | Create events, trigger actions | External integrations |
| `readonly` | Read-only access to all resources | Monitoring, auditing |

## Database Schema

### Identity Table

Service accounts are stored in the `identity` table with `identity_type = 'service_account'`:

```sql
CREATE TABLE identity (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    identity_type identity_type NOT NULL,  -- 'user' or 'service_account'
    email VARCHAR(255),  -- NULL for service accounts
    password_hash VARCHAR(255),  -- NULL for service accounts
    metadata JSONB DEFAULT '{}',
    created TIMESTAMPTZ DEFAULT NOW(),
    updated TIMESTAMPTZ DEFAULT NOW()
);
```

Service account metadata includes:
```json
{
  "scope": "sensor",
  "description": "Timer sensor service account",
  "created_by": 1,  // identity_id of creator
  "expires_at": "2025-04-27T12:34:56Z",
  "trigger_types": ["core.timer"],  // For sensor scope
  "execution_id": 123  // For action_execution scope
}
```

### Token Storage

Tokens are **not** stored in the database (they are stateless JWTs). However, revocation is tracked:

```sql
CREATE TABLE token_revocation (
    id BIGSERIAL PRIMARY KEY,
    identity_id BIGINT NOT NULL REFERENCES identity(id) ON DELETE CASCADE,
    token_jti VARCHAR(255) NOT NULL,  -- JWT ID (jti claim)
    token_exp TIMESTAMPTZ NOT NULL,   -- Token expiration (from exp claim)
    revoked_at TIMESTAMPTZ DEFAULT NOW(),
    revoked_by BIGINT REFERENCES identity(id),
    reason VARCHAR(500),
    UNIQUE(token_jti)
);

CREATE INDEX idx_token_revocation_jti ON token_revocation(token_jti);
CREATE INDEX idx_token_revocation_identity ON token_revocation(identity_id);
CREATE INDEX idx_token_revocation_exp ON token_revocation(token_exp);  -- For cleanup queries
```

## JWT Token Format

### Claims

All service account tokens include these claims:

```json
{
  "sub": "sensor:core.timer",  // Subject: "type:name"
  "jti": "abc123...",  // JWT ID (for revocation)
  "iat": 1706356496,  // Issued at (Unix timestamp)
  "exp": 1714132496,  // Expires at (Unix timestamp)
  "identity_id": 123,
  "identity_type": "service_account",
  "scope": "sensor",
  "metadata": {
    "trigger_types": ["core.timer"]
  }
}
```

### Scope-Specific Claims

**Sensor tokens** (restricted to declared trigger types):
```json
{
  "scope": "sensor",
  "metadata": {
    "trigger_types": ["core.timer", "core.interval"]
  }
}
```

The API enforces that sensors can only create events for trigger types listed in `metadata.trigger_types`. Attempting to create an event for an unauthorized trigger type will result in a `403 Forbidden` error.

**Action execution tokens**:
```json
{
  "scope": "action_execution",
  "metadata": {
    "execution_id": 456,
    "action_ref": "core.echo",
    "workflow_id": 789  // Optional, if part of workflow
  }
}
```

**Webhook tokens**:
```json
{
  "scope": "webhook",
  "metadata": {
    "allowed_paths": ["/webhooks/deploy", "/webhooks/alert"],
    "ip_whitelist": ["203.0.113.0/24"]  // Optional
  }
}
```

## API Endpoints

### Create Service Account

**Admin only**

```http
POST /service-accounts
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "name": "sensor:core.timer",
  "scope": "sensor",
  "description": "Timer sensor service account",
  "ttl_days": 90,  // Sensor tokens: 90 days, auto-refresh before expiration
  "metadata": {
    "trigger_types": ["core.timer"]
  }
}
```

**Response**:
```json
{
  "identity_id": 123,
  "name": "sensor:core.timer",
  "scope": "sensor",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_at": "2025-04-27T12:34:56Z"  // 90 days from now
}
```

**Important**: The token is only shown once. Store it securely.

### List Service Accounts

**Admin only**

```http
GET /service-accounts
Authorization: Bearer {admin_token}
```

**Response**:
```json
{
  "data": [
    {
      "identity_id": 123,
      "name": "sensor:core.timer",
      "scope": "sensor",
      "created_at": "2025-01-27T12:34:56Z",
      "expires_at": "2025-04-27T12:34:56Z",
      "metadata": {
        "trigger_types": ["core.timer"]
      }
    }
  ]
}
```

### Refresh Token (Self-Service)

**Sensor/User tokens can refresh themselves**

```http
POST /auth/refresh
Authorization: Bearer {current_token}
Content-Type: application/json

{}
```

**Response**:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_at": "2025-04-27T12:34:56Z"
}
```

**Notes**:
- Current token must be valid (not expired, not revoked)
- New token has same scope and metadata as current token
- New token has same TTL as original token type (e.g., 90 days for sensors)
- Old token remains valid until its original expiration (allows zero-downtime refresh)
- Only `sensor` and `user` scopes can refresh (not `action_execution` or `webhook`)

### Revoke Service Account Token

**Admin only**

```http
DELETE /service-accounts/{identity_id}
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "reason": "Token compromised"
}
```

**Response**:
```json
{
  "message": "Service account revoked",
  "identity_id": 123
}
```

### Create Execution Token (Internal)

**Called by executor service, not exposed in API**

```rust
// In executor service
let execution_timeout_minutes = get_action_timeout(action_ref); // e.g., 30 minutes
let token = create_execution_token(
    execution_id,
    action_ref,
    ttl_minutes: execution_timeout_minutes
)?;
```

This token is passed to the worker service, which injects it into the action's environment.

## Token Creation Workflow

### 1. Sensor Token Creation

```
Admin → POST /service-accounts (scope=sensor) → API
API → Create identity record → Database
API → Generate JWT with sensor scope → Response
Admin → Store token in secure config → Sensor deployment
Sensor → Use token for API calls → Event emission
```

### 2. Execution Token Creation

```
Rule fires → Executor creates enforcement → Executor
Executor → Schedule execution → Database
Executor → Create execution token (internal) → JWT library
Executor → Send execution request to worker → RabbitMQ
Worker → Receive message with token → Action runner
Action → Use token to fetch keys → API
Execution completes → Token expires (TTL) → Automatic cleanup
```

## Token Validation

### Middleware (API Service)

```rust
// In API service
pub async fn validate_token(
    token: &str,
    required_scope: Option<&str>
) -> Result<Claims> {
    // 1. Verify JWT signature
    let claims = decode_jwt(token)?;
    
    // 2. Check expiration (JWT library handles this, but explicit check for clarity)
    if claims.exp < now() {
        return Err(Error::TokenExpired);
    }
    
    // 3. Check revocation (only check non-expired tokens)
    if is_revoked(&claims.jti, claims.exp).await? {
        return Err(Error::TokenRevoked);
    }
    
    // 4. Check scope
    if let Some(scope) = required_scope {
        if claims.scope != scope {
            return Err(Error::InsufficientPermissions);
        }
    }
    
    Ok(claims)
}
```

### Scope-Based Authorization

```rust
// Execution-scoped token can only access its own execution
if claims.scope == "action_execution" {
    let allowed_execution_id = claims.metadata
        .get("execution_id")
        .and_then(|v| v.as_i64())
        .ok_or(Error::InvalidToken)?;
    
    if execution_id != allowed_execution_id {
        return Err(Error::InsufficientPermissions);
    }
}

// Sensor-scoped token can only create events for declared trigger types
if claims.scope == "sensor" {
    let allowed_trigger_types = claims.metadata
        .get("trigger_types")
        .and_then(|v| v.as_array())
        .ok_or(Error::InvalidToken)?;
    
    let allowed_types: Vec<String> = allowed_trigger_types
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    
    if !allowed_types.contains(&trigger_type) {
        return Err(Error::InsufficientPermissions);
    }
}
```

## Security Best Practices

### Token Generation
**Generation:**

1. **Use Strong Secrets**: JWT signing key must be 256+ bits, randomly generated
2. **Include JTI**: Always include `jti` claim for revocation support
3. **REQUIRED Expiration**: All tokens MUST have `exp` claim - no exceptions
   - Sensor tokens: 90 days (auto-refresh before expiration)
   - Action execution tokens: Match execution timeout (5-60 minutes)
   - User CLI tokens: 7-30 days (auto-refresh before expiration)
   - Webhook tokens: 90-365 days (manual rotation)
4. **Minimal Scope**: Grant least privilege necessary
5. **Restrict Trigger Types**: For sensor tokens, only include necessary trigger types in metadata

### Token Storage

1. **Environment Variables**: Preferred method for sensors and actions
2. **Never Log**: Redact tokens from logs (show only last 4 chars)
3. **Never Commit**: Don't commit tokens to version control
4. **Secure Config**: Store in encrypted config management (Vault, k8s secrets)

### Token Transmission

1. **HTTPS Only**: Never send tokens over unencrypted connections
2. **Authorization Header**: Use `Authorization: Bearer {token}` header
3. **No Query Params**: Don't pass tokens in URL query parameters
4. **No Cookies**: For service accounts, avoid cookie-based auth

### Token Revocation

1. **Immediate Revocation**: Check revocation list on every request
2. **Audit Trail**: Log who revoked, when, and why
3. **Cascade Delete**: Revoke all tokens when service account is deleted
4. **Automatic Cleanup**: Delete revocation records for expired tokens (run hourly)
   - Query: `DELETE FROM token_revocation WHERE token_exp < NOW()`
   - Prevents indefinite table bloat
   - Expired tokens are already invalid, no need to track revocation
5. **Validate Permissions**: Enforce trigger type restrictions for sensor tokens on event creation

## Implementation Checklist

- [ ] Add `identity_type` enum to database schema
- [ ] Add `token_revocation` table (with `token_exp` column)
- [ ] Create `POST /service-accounts` endpoint
- [ ] Create `GET /service-accounts` endpoint
- [ ] Create `DELETE /service-accounts/{id}` endpoint
- [ ] Create `POST /auth/refresh` endpoint (for automatic token refresh)
- [ ] Add scope validation middleware
- [ ] Add token revocation check middleware (skip check for expired tokens)
- [ ] Implement execution token creation in executor (TTL = action timeout)
- [ ] Pass execution token to worker via RabbitMQ
- [ ] Inject execution token into action environment
- [ ] Add CLI commands: `attune service-account create/list/revoke`
- [ ] Document token creation for sensor deployment
- [ ] Implement automatic token refresh in sensors (refresh at 80% of TTL)
- [ ] Implement cleanup job for expired token revocations (hourly cron)

## Migration Path

### Phase 1: Database Schema

```sql
-- Add identity_type enum if not exists
DO $$ BEGIN
    CREATE TYPE identity_type AS ENUM ('user', 'service_account');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Add identity_type column to identity table
ALTER TABLE identity 
    ADD COLUMN IF NOT EXISTS identity_type identity_type DEFAULT 'user';

-- Create token_revocation table
CREATE TABLE IF NOT EXISTS token_revocation (
    id BIGSERIAL PRIMARY KEY,
    identity_id BIGINT NOT NULL REFERENCES identity(id) ON DELETE CASCADE,
    token_jti VARCHAR(255) NOT NULL,
    token_exp TIMESTAMPTZ NOT NULL,  -- For cleanup queries
    revoked_at TIMESTAMPTZ DEFAULT NOW(),
    revoked_by BIGINT REFERENCES identity(id),
    reason VARCHAR(500),
    UNIQUE(token_jti)
);

CREATE INDEX IF NOT EXISTS idx_token_revocation_jti ON token_revocation(token_jti);
CREATE INDEX IF NOT EXISTS idx_token_revocation_exp ON token_revocation(token_exp);
```

### Phase 2: API Implementation

1. Add service account repository
2. Add JWT utilities for scope-based tokens
3. Implement service account CRUD endpoints
4. Add middleware for token validation and revocation

### Phase 3: Integration

1. Update executor to create execution tokens
2. Update worker to receive and use execution tokens
3. Update sensor to accept and use sensor tokens
4. Update CLI to support service account management

## Examples

### Python Action Using Execution Token

```python
#!/usr/bin/env python3
import os
import requests
import sys

# Token is injected by worker
api_url = os.environ['ATTUNE_API_URL']
api_token = os.environ['ATTUNE_API_TOKEN']
execution_id = os.environ['ATTUNE_EXECUTION_ID']

# Fetch encrypted secret
response = requests.get(
    f"{api_url}/keys/myapp.database_password",
    headers={"Authorization": f"Bearer {api_token}"}
)

if response.status_code != 200:
    print(f"Failed to fetch key: {response.text}", file=sys.stderr)
    sys.exit(1)

db_password = response.json()['value']

# Use the secret...
print("Successfully connected to database")
```

### Sensor Using Sensor Token

```rust
// In sensor initialization
let api_token = env::var("ATTUNE_API_TOKEN")?;
let api_url = env::var("ATTUNE_API_URL")?;

let client = reqwest::Client::new();

// Fetch active rules
let response = client
    .get(format!("{}/rules?trigger_type=core.timer", api_url))
    .header("Authorization", format!("Bearer {}", api_token))
    .send()
    .await?;

let rules: Vec<Rule> = response.json().await?;
```

## Token Lifecycle Management

### Expiration Strategy

**All tokens MUST expire** to prevent indefinite revocation table bloat and reduce attack surface:

| Token Type | Expiration | Rationale |
|------------|------------|-----------|
| Sensor | 90 days | Perpetually running service, auto-refresh before expiration |
| Action Execution | 5-60 minutes | Matches action timeout, auto-cleanup on completion |
| User CLI | 7-30 days | Balance between convenience and security, auto-refresh |
| Webhook | 90-365 days | External integration, manual rotation required |

### Revocation Table Cleanup

Cleanup job runs hourly to prevent table bloat:

```sql
-- Delete revocation records for expired tokens
DELETE FROM token_revocation 
WHERE token_exp < NOW();
```

**Why this works:**
- Expired tokens are already invalid (enforced by JWT `exp` claim)
- No need to track revocation status for invalid tokens
- Keeps revocation table small and queries fast
- Typical size: <1000 rows instead of millions

### Sensor Token Refresh

Sensors automatically refresh their own tokens without human intervention:

**Automatic Process:**
1. Sensor starts with 90-day token
2. Background task monitors token expiration
3. When 80% of TTL elapsed (72 days), sensor requests new token via `POST /auth/refresh`
4. New token is hot-loaded without restart
5. Old token remains valid until original expiration
6. Process repeats indefinitely

**Refresh Timing Example:**
- Token issued: Day 0, expires Day 90
- Refresh trigger: Day 72 (80% of 90 days)
- New token issued: Day 72, expires Day 162
- Old token still valid: Day 72-90 (overlap period)
- Next refresh: Day 144 (80% of new token)

**Zero-Downtime:**
- No service interruption during refresh
- Old token valid during transition
- Graceful fallback on refresh failure

## Cleanup Job Implementation

### Purpose

Prevent indefinite growth of the `token_revocation` table by removing revocation records for expired tokens.

### Why Cleanup Is Safe

- Expired tokens are already invalid (enforced by JWT `exp` claim)
- Token validation checks expiration before checking revocation
- No security risk in deleting expired token revocations
- Significantly reduces table size and improves query performance

### Implementation

**Frequency**: Hourly cron job or background task

**SQL Query**:
```sql
DELETE FROM token_revocation 
WHERE token_exp < NOW();
```

**Expected Impact**:
- Typical table size: <1,000 rows instead of millions over time
- Fast revocation checks (indexed queries on small dataset)
- Reduced storage and backup costs

### Rust Implementation Example

```rust
use tokio::time::{interval, Duration};

/// Background task to clean up expired token revocations
pub async fn start_revocation_cleanup_task(db: PgPool) {
    let mut interval = interval(Duration::from_secs(3600)); // Every hour
    
    loop {
        interval.tick().await;
        
        match cleanup_expired_revocations(&db).await {
            Ok(count) => {
                info!("Cleaned up {} expired token revocations", count);
            }
            Err(e) => {
                error!("Failed to clean up expired token revocations: {}", e);
            }
        }
    }
}

/// Delete token revocation records for expired tokens
async fn cleanup_expired_revocations(db: &PgPool) -> Result<u64> {
    let result = sqlx::query!(
        "DELETE FROM token_revocation WHERE token_exp < NOW()"
    )
    .execute(db)
    .await?;
    
    Ok(result.rows_affected())
}
```

### Monitoring

Track cleanup job metrics:
- Number of records deleted per run
- Job execution time
- Job failures (alert if consecutive failures)

**Prometheus Metrics Example**:
```rust
// Define metrics
lazy_static! {
    static ref REVOCATION_CLEANUP_COUNT: IntCounter = register_int_counter!(
        "attune_revocation_cleanup_total",
        "Total number of expired token revocations cleaned up"
    ).unwrap();
    
    static ref REVOCATION_CLEANUP_DURATION: Histogram = register_histogram!(
        "attune_revocation_cleanup_duration_seconds",
        "Duration of token revocation cleanup job"
    ).unwrap();
}

// In cleanup function
let timer = REVOCATION_CLEANUP_DURATION.start_timer();
let count = cleanup_expired_revocations(&db).await?;
REVOCATION_CLEANUP_COUNT.inc_by(count);
timer.observe_duration();
```

### Alternative: Database Trigger

For automatic cleanup without application code:

```sql
-- Create function to delete old revocations
CREATE OR REPLACE FUNCTION cleanup_expired_token_revocations()
RETURNS trigger AS $$
BEGIN
    DELETE FROM token_revocation WHERE token_exp < NOW() - INTERVAL '1 hour';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Trigger on insert (cleanup when new revocations are added)
CREATE TRIGGER trigger_cleanup_expired_revocations
    AFTER INSERT ON token_revocation
    EXECUTE FUNCTION cleanup_expired_token_revocations();
```

**Note**: Application-level cleanup is preferred for better observability and control.

## Future Enhancements

1. **Rate Limiting**: Per-token rate limits to prevent abuse
2. **Audit Logging**: Comprehensive audit trail of token usage and refresh events
3. **OAuth 2.0**: Support OAuth 2.0 client credentials flow
4. **mTLS**: Mutual TLS authentication for high-security deployments
5. **Token Introspection**: RFC 7662-compliant token introspection endpoint
6. **Scope Hierarchies**: More granular permission scopes
7. **IP Whitelisting**: Restrict token usage to specific IP ranges
8. **Configurable Refresh Timing**: Allow custom refresh thresholds per token type
9. **Token Lineage Tracking**: Track token refresh chains for security audits
8. **Refresh Failure Alerts**: Notify operators when automatic refresh fails
9. **Token Lineage Tracking**: Track token refresh chains for audit purposes