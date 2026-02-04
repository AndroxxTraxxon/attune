# Quick Reference: Timer Echo Happy Path Test

This guide provides a quick reference for testing the core happy-path scenario in Attune: an interval timer running every second to execute `echo "Hello, World!"`.

## Overview

This test verifies the complete event-driven flow with unified runtime detection:

```
Timer Sensor → Event → Rule Match → Enforcement → Execution → Worker → Shell Action
```

## Prerequisites

- Docker and Docker Compose installed
- All Attune services running in containers
- Core pack loaded with timer triggers and echo action

## Quick Test (Automated)

Run the automated test script:

```bash
cd attune
./scripts/test-timer-echo-docker.sh
```

This script will:
1. ✓ Check Docker services are healthy
2. ✓ Authenticate with API
3. ✓ Verify runtime detection (Shell runtime available)
4. ✓ Verify core pack is loaded
5. ✓ Create a 1-second interval timer trigger instance
6. ✓ Create a rule linking timer to echo action
7. ✓ Wait 15 seconds and verify executions
8. ✓ Display results and cleanup

**Expected output:**
```
=== HAPPY PATH TEST PASSED ===

The complete event flow is working:
  Timer Sensor → Event → Rule → Enforcement → Execution → Worker → Shell Action
```

## Manual Test Steps

### 1. Start Services

```bash
docker-compose up -d
docker-compose ps  # Verify all services are running
```

### 2. Check Runtime Detection

```bash
# Get auth token
export TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin"}' | jq -r '.data.access_token')

# Verify runtimes detected
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/runtimes | jq '.data[] | {name, enabled}'
```

**Expected:** Shell runtime should be present and enabled.

### 3. Verify Core Pack

```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/packs/core | jq '.data | {id, ref, name}'
```

**Expected:** Core pack with actions and triggers loaded.

### 4. Create Trigger Instance

```bash
curl -X POST http://localhost:8080/api/v1/trigger-instances \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type_ref": "core.intervaltimer",
    "ref": "test.timer_1s",
    "description": "1-second interval timer",
    "enabled": true,
    "parameters": {
      "unit": "seconds",
      "interval": 1
    }
  }' | jq '.data | {id, ref}'
```

### 5. Create Rule

```bash
curl -X POST http://localhost:8080/api/v1/rules \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "test.timer_echo",
    "pack_ref": "core",
    "name": "Timer Echo Test",
    "description": "Echoes Hello World every second",
    "enabled": true,
    "trigger_instance_ref": "test.timer_1s",
    "action_ref": "core.echo",
    "action_parameters": {
      "message": "Hello, World!"
    }
  }' | jq '.data | {id, ref}'
```

### 6. Monitor Executions

Wait 10-15 seconds, then check for executions:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/executions?limit=10 | \
  jq '.data[] | {id, status, action_ref, created}'
```

**Expected:** Multiple executions with `status: "succeeded"` and `action_ref: "core.echo"`.

### 7. Check Service Logs

```bash
# Sensor service (timer firing)
docker logs attune-sensor --tail 50 | grep -i "timer\|interval"

# Executor service (scheduling)
docker logs attune-executor --tail 50 | grep -i "execution\|schedule"

# Worker service (runtime detection and action execution)
docker logs attune-worker --tail 50 | grep -i "runtime\|shell\|echo"
```

**Expected log entries:**

**Sensor:**
```
Timer trigger fired: core.intervaltimer
Event created: id=123
```

**Executor:**
```
Processing enforcement: id=456
Execution scheduled: id=789
```

**Worker:**
```
Runtime detected: Shell
Executing action: core.echo
Action completed successfully
```

### 8. Cleanup

```bash
# Disable the rule
curl -X PUT http://localhost:8080/api/v1/rules/test.timer_echo \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'

# Delete the rule (optional)
curl -X DELETE http://localhost:8080/api/v1/rules/test.timer_echo \
  -H "Authorization: Bearer $TOKEN"

# Delete trigger instance (optional)
curl -X DELETE http://localhost:8080/api/v1/trigger-instances/test.timer_1s \
  -H "Authorization: Bearer $TOKEN"
```

## Troubleshooting

### No Executions Created

**Check 1: Is the sensor service running?**
```bash
docker logs attune-sensor --tail 100
```
Look for: "Started monitoring trigger instances" or "Timer trigger fired"

**Check 2: Are events being created?**
```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/events?limit=10 | jq '.data | length'
```

**Check 3: Are enforcements being created?**
```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/enforcements?limit=10 | jq '.data | length'
```

**Check 4: Is the rule enabled?**
```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/rules/test.timer_echo | jq '.data.enabled'
```

### Executions Failed

**Check worker logs for errors:**
```bash
docker logs attune-worker --tail 100 | grep -i "error\|failed"
```

**Check execution details:**
```bash
EXEC_ID=$(curl -s -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/executions?limit=1 | jq -r '.data[0].id')

curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/executions/$EXEC_ID | jq '.data'
```

**Common issues:**
- Runtime not detected: Check worker startup logs for "Runtime detected: Shell"
- Action script not found: Verify packs mounted at `/opt/attune/packs` in worker container
- Permission denied: Check file permissions on `packs/core/actions/echo.sh`

### Runtime Not Detected

**Check runtime configuration in database:**
```bash
docker exec -it postgres psql -U attune -d attune \
  -c "SELECT name, enabled, distributions FROM attune.runtime WHERE name ILIKE '%shell%';"
```

**Check worker configuration:**
```bash
docker exec -it attune-worker env | grep ATTUNE
```

**Verify Shell runtime verification:**
```bash
# This should succeed on the worker container
docker exec -it attune-worker /bin/bash -c "echo 'Runtime test'"
```

## Configuration Files

**Docker config:** `config.docker.yaml`
- Database: `postgresql://attune:attune@postgres:5432/attune`
- Message Queue: `amqp://attune:attune@rabbitmq:5672`
- Packs: `/opt/attune/packs`
- Schema: `attune`

**Core pack location (in containers):**
- Actions: `/opt/attune/packs/core/actions/`
- Triggers: `/opt/attune/packs/core/triggers/`
- Sensors: `/opt/attune/packs/core/sensors/`

## Success Criteria

✅ **Shell runtime detected** by worker service
✅ **Core pack loaded** with echo action and timer trigger
✅ **Events generated** by sensor every second
✅ **Enforcements created** by rule matching
✅ **Executions scheduled** by executor service
✅ **Actions executed** by worker service using Shell runtime
✅ **Executions succeed** with "Hello, World!" output

## Next Steps

After verifying the happy path:

1. **Test Python runtime**: Create a Python action and verify runtime detection
2. **Test Node.js runtime**: Create a Node.js action and verify runtime detection
3. **Test workflows**: Chain multiple actions together
4. **Test pack environments**: Verify pack-specific dependency isolation
5. **Test error handling**: Trigger failures and verify retry logic
6. **Test concurrency**: Create multiple rules firing simultaneously

## Related Documentation

- [Unified Runtime Detection](../QUICKREF-unified-runtime-detection.md)
- [Pack Runtime Environments](../pack-runtime-environments.md)
- [Worker Service Architecture](../architecture/worker-service.md)
- [Sensor Service Architecture](../architecture/sensor-service.md)
- [Timer Sensor Quickstart](./timer-sensor-quickstart.md)