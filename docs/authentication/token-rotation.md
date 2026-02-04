# Token Rotation Guide

**Version:** 1.0  
**Last Updated:** 2025-01-27  
**Audience:** System Administrators, DevOps Engineers

## Overview

This guide provides procedures for rotating service account tokens in Attune to maintain security and prevent token revocation table bloat. All tokens in Attune have expiration times and require periodic rotation.

## Token Expiration Policy

**All tokens MUST expire.** This is a hard requirement to prevent:
- Indefinite growth of the `token_revocation` table
- Long-lived compromised credentials
- Security debt accumulation

### Token Lifetimes

| Token Type | Lifetime | Rotation Frequency | Auto-Cleanup |
|------------|----------|-------------------|--------------|
| Sensor | 24-72 hours | Every 24-72 hours | Yes (on expiration) |
| Action Execution | 5-60 minutes | N/A (single-use) | Yes (on completion) |
| User CLI | 7-30 days | Every 7-30 days | No (manual revocation) |
| Webhook | 90-365 days | Every 90-365 days | No (manual revocation) |

## Sensor Token Rotation

### Why Rotation is Required

Sensor tokens expire after 24-72 hours to:
- Limit the impact of compromised credentials
- Force regular security reviews
- Prevent revocation table bloat
- Align with security best practices

### Rotation Process

#### Manual Rotation (Current)

**Preparation:**
```bash
# Set admin token
export ADMIN_TOKEN="your_admin_token"

# Note the current sensor name
SENSOR_NAME="sensor:core.timer"
```

**Step 1: Create New Service Account**

```bash
# Create new token
curl -X POST http://localhost:8080/service-accounts \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"${SENSOR_NAME}\",
    \"scope\": \"sensor\",
    \"description\": \"Timer sensor (rotated $(date +%Y-%m-%d))\",
    \"ttl_hours\": 72,
    \"metadata\": {
      \"trigger_types\": [\"core.timer\"]
    }
  }"

# Save the response
# {
#   "identity_id": 456,
#   "name": "sensor:core.timer",
#   "token": "eyJhbGci...",  <-- COPY THIS
#   "expires_at": "2025-01-30T12:34:56Z"
# }

export NEW_TOKEN="eyJhbGci..."
```

**Step 2: Update Sensor Configuration**

**For systemd deployments:**
```bash
# Update environment file
sudo nano /etc/attune/sensor-timer.env

# Replace old token with new token
ATTUNE_API_TOKEN=eyJhbGci...  # <-- NEW TOKEN HERE
```

**For Docker/Kubernetes:**
```bash
# Update secret
kubectl create secret generic sensor-timer-token \
  --from-literal=token="${NEW_TOKEN}" \
  --dry-run=client -o yaml | kubectl apply -f -

# Or update Docker environment variable
docker service update attune-core-timer-sensor \
  --env-add ATTUNE_API_TOKEN="${NEW_TOKEN}"
```

**For environment variables:**
```bash
# Update environment variable
export ATTUNE_API_TOKEN="${NEW_TOKEN}"
```

**Step 3: Restart Sensor**

```bash
# systemd
sudo systemctl restart attune-core-timer-sensor

# Docker
docker restart attune-core-timer-sensor

# Kubernetes
kubectl rollout restart deployment/sensor-timer
```

**Step 4: Verify New Token is Working**

```bash
# Check sensor logs
sudo journalctl -u attune-core-timer-sensor -f --since "1 minute ago"

# Look for:
# - "API connectivity verified"
# - "Connected to RabbitMQ"
# - "Started consuming messages"
# - No authentication errors
```

**Step 5: Revoke Old Token (Optional)**

The old token will expire automatically after 72 hours. For immediate revocation:

```bash
# Get old identity_id from previous creation response
OLD_IDENTITY_ID=123

# Revoke old token
curl -X DELETE http://localhost:8080/service-accounts/${OLD_IDENTITY_ID} \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"reason\": \"Token rotation\"
  }"
```

### Rotation Schedule

**Recommended Schedule:**
- **Production:** Every 48 hours (allows 24-hour margin before expiration)
- **Staging:** Every 72 hours
- **Development:** Every 72 hours

**Calendar Reminder:**
Set up recurring calendar events or use cron to remind operators:

```bash
# Add to crontab (runs every 48 hours)
0 */48 * * * /usr/local/bin/rotate-sensor-token.sh
```

### Monitoring Token Expiration

**Check Token Expiration:**

```bash
# Decode JWT to check expiration
echo "${ATTUNE_API_TOKEN}" | cut -d'.' -f2 | base64 -d 2>/dev/null | jq -r '.exp'

# Output: 1738886400 (Unix timestamp)

# Convert to human-readable
date -d @1738886400
# Output: 2025-01-30 12:00:00
```

**Set Up Alerts:**

```bash
#!/bin/bash
# check-token-expiration.sh
# Run this hourly via cron

TOKEN="${ATTUNE_API_TOKEN}"
EXP=$(echo "${TOKEN}" | cut -d'.' -f2 | base64 -d 2>/dev/null | jq -r '.exp')
NOW=$(date +%s)
HOURS_REMAINING=$(( ($EXP - $NOW) / 3600 ))

if [ "$HOURS_REMAINING" -lt 6 ]; then
    echo "WARNING: Sensor token expires in ${HOURS_REMAINING} hours!"
    # Send alert to monitoring system
    curl -X POST https://monitoring.example.com/alerts \
      -d "message=Sensor token expires in ${HOURS_REMAINING} hours"
fi
```

**Add to crontab:**
```bash
0 * * * * /usr/local/bin/check-token-expiration.sh
```

## Action Execution Token Lifecycle

Action execution tokens are automatically managed:

**Creation:** Executor service creates token when scheduling execution
```rust
let token = create_execution_token(
    execution_id,
    action_ref,
    ttl_minutes: action_timeout_minutes
)?;
```

**Usage:** Worker injects token into action environment
```bash
ATTUNE_API_TOKEN=eyJhbGci...
ATTUNE_EXECUTION_ID=123
```

**Expiration:** Token expires when execution times out or completes

**Cleanup:** Revocation record (if created) is automatically deleted after expiration

**No manual intervention required.**

## User CLI Token Rotation

### When to Rotate

- Every 7-30 days (based on TTL)
- When user credentials change
- When token is compromised
- When user leaves organization

### Rotation Process

**Step 1: Login Again**

```bash
# User logs in to get new token
attune auth login

# Enter credentials
# New token is stored in ~/.attune/token
```

**Step 2: Verify New Token**

```bash
# Test with simple command
attune pack list

# Should succeed without errors
```

**Old token is automatically revoked during login (if configured).**

## Webhook Token Rotation

### When to Rotate

- Every 90-365 days (based on TTL)
- When webhook is compromised
- When integrating system changes
- During security audits

### Rotation Process

**Step 1: Create New Webhook Token**

```bash
curl -X POST http://localhost:8080/service-accounts \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "webhook:deployment-notifications",
    "scope": "webhook",
    "description": "GitHub deployment webhook",
    "ttl_days": 90,
    "metadata": {
      "allowed_paths": ["/webhooks/deploy"]
    }
  }'

# Save the new token
export NEW_WEBHOOK_TOKEN="eyJhbGci..."
```

**Step 2: Update External System**

Update the webhook configuration in the external system (GitHub, GitLab, etc.) with the new token.

**Step 3: Test Webhook**

```bash
# Send test webhook
curl -X POST https://attune.example.com/webhooks/deploy \
  -H "Authorization: Bearer ${NEW_WEBHOOK_TOKEN}" \
  -d '{"status": "deployed"}'

# Should succeed
```

**Step 4: Revoke Old Token**

After confirming the new token works:

```bash
curl -X DELETE http://localhost:8080/service-accounts/${OLD_IDENTITY_ID} \
  -H "Authorization: Bearer ${ADMIN_TOKEN}"
```

## Automation Scripts

### Sensor Token Rotation Script

```bash
#!/bin/bash
# rotate-sensor-token.sh
# Automated sensor token rotation

set -e

SENSOR_NAME="${1:-sensor:core.timer}"
ADMIN_TOKEN="${ADMIN_TOKEN}"
API_URL="${ATTUNE_API_URL:-http://localhost:8080}"

if [ -z "$ADMIN_TOKEN" ]; then
    echo "Error: ADMIN_TOKEN environment variable not set"
    exit 1
fi

echo "Rotating token for ${SENSOR_NAME}..."

# Create new token
RESPONSE=$(curl -s -X POST "${API_URL}/service-accounts" \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"${SENSOR_NAME}\",
    \"scope\": \"sensor\",
    \"description\": \"Auto-rotated $(date +%Y-%m-%d)\",
    \"ttl_hours\": 72,
    \"metadata\": {
      \"trigger_types\": [\"core.timer\"]
    }
  }")

NEW_TOKEN=$(echo "$RESPONSE" | jq -r '.token')
EXPIRES_AT=$(echo "$RESPONSE" | jq -r '.expires_at')

if [ -z "$NEW_TOKEN" ] || [ "$NEW_TOKEN" = "null" ]; then
    echo "Error: Failed to create new token"
    echo "$RESPONSE"
    exit 1
fi

echo "New token created, expires at: ${EXPIRES_AT}"

# Update configuration file
echo "ATTUNE_API_TOKEN=${NEW_TOKEN}" | sudo tee /etc/attune/sensor-timer.env

# Restart service
echo "Restarting sensor service..."
sudo systemctl restart attune-core-timer-sensor

# Wait for service to start
sleep 5

# Check status
if sudo systemctl is-active --quiet attune-core-timer-sensor; then
    echo "✓ Sensor token rotated successfully"
    echo "  New token expires: ${EXPIRES_AT}"
else
    echo "✗ Sensor failed to start, check logs"
    sudo journalctl -u attune-core-timer-sensor -n 50
    exit 1
fi
```

### Token Expiration Check Script

```bash
#!/bin/bash
# check-all-tokens.sh
# Check expiration for all active service accounts

API_URL="${ATTUNE_API_URL:-http://localhost:8080}"
ADMIN_TOKEN="${ADMIN_TOKEN}"
WARN_HOURS=6

# Fetch all service accounts
ACCOUNTS=$(curl -s -X GET "${API_URL}/service-accounts" \
  -H "Authorization: Bearer ${ADMIN_TOKEN}")

echo "$ACCOUNTS" | jq -r '.data[] | "\(.name)\t\(.expires_at)"' | \
while IFS=$'\t' read -r name expires_at; do
    exp_timestamp=$(date -d "$expires_at" +%s)
    now=$(date +%s)
    hours_remaining=$(( ($exp_timestamp - $now) / 3600 ))
    
    if [ "$hours_remaining" -lt "$WARN_HOURS" ]; then
        echo "⚠️  WARNING: ${name} expires in ${hours_remaining} hours (${expires_at})"
    else
        echo "✓  ${name} expires in ${hours_remaining} hours (${expires_at})"
    fi
done
```

## Troubleshooting

### "Token expired" Error

**Symptom:** Sensor logs show "401 Unauthorized" or "Token expired"

**Solution:**
1. Verify current time is correct: `date`
2. Check token expiration: `echo $TOKEN | cut -d'.' -f2 | base64 -d | jq .exp`
3. Create new token and restart sensor (see rotation process above)

### Sensor Won't Start After Rotation

**Symptom:** Sensor fails to start after updating token

**Troubleshooting:**
1. Verify token is correctly formatted (JWT with 3 parts: header.payload.signature)
2. Check token hasn't already expired
3. Verify token has correct scope and metadata
4. Check sensor logs for specific error message

### Token Revocation Table Growing Too Large

**Symptom:** `token_revocation` table has millions of rows

**Solution:**
1. Ensure cleanup job is running (hourly)
2. Manually run cleanup: `DELETE FROM token_revocation WHERE token_exp < NOW()`
3. Verify all tokens have expiration set
4. Check for tokens with very long TTLs

## Best Practices

1. **Set Calendar Reminders:** Don't rely on memory, set recurring calendar events
2. **Automate Where Possible:** Use cron jobs and scripts for rotation
3. **Monitor Expiration:** Set up alerts 6-12 hours before expiration
4. **Test Rotation:** Practice rotation in staging before production
5. **Document Tokens:** Keep inventory of active service accounts and their purposes
6. **Minimal TTL:** Use shortest acceptable TTL for each token type
7. **Rotate on Compromise:** Immediately rotate if token is compromised
8. **Clean Up:** Revoke old tokens after rotation (or let them expire)

## Security Considerations

- **Never commit tokens to version control**
- **Use encrypted storage for tokens** (e.g., Vault, AWS Secrets Manager)
- **Rotate immediately if compromised**
- **Audit token usage regularly**
- **Minimize token scope and permissions**
- **Use separate tokens for each sensor/webhook**
- **Monitor for unauthorized token usage**

## Future Enhancements

1. **Automatic Rotation:** Hot-reload tokens without sensor restart
2. **Token Renewal API:** Extend token TTL without creating new token
3. **Token Rotation Hooks:** Webhook notifications before expiration
4. **Managed Tokens:** Orchestrator handles rotation automatically
5. **Token Rotation Dashboard:** Web UI for monitoring and rotating tokens

## See Also

- [Service Accounts Documentation](./service-accounts.md)
- [Sensor Interface Specification](./sensor-interface.md)
- [Sensor Authentication Overview](./sensor-authentication-overview.md)
- [Timer Sensor README](../crates/core-timer-sensor/README.md)
