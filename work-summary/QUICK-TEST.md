# Quick Test: Timer Echo Happy Path

**Goal:** Verify the complete event flow with unified runtime detection in under 2 minutes.

**Status:** ⚠️ **Blocked by sensor binary deployment issue** (see below)

## Prerequisites

✅ Docker containers running (check with `docker ps`)
✅ API service healthy on port 8080

## Known Issue

The timer sensor binary (`attune-core-timer-sensor`) is not deployed in the container, preventing end-to-end testing.
However, all runtime detection components up to the sensor are verified working.

See `work-summary/2026-02-02-unified-runtime-verification.md` for full details.

## Test Steps (Copy-Paste)

### 1. Get Auth Token

```bash
TOKEN=$(curl -s http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"login":"test@attune.local","password":"TestPass123!"}' \
  | jq -r '.data.access_token')

echo "Authenticated: ${TOKEN:0:20}..."
```

### 2. Verify Runtime Detection

```bash
curl -s -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/runtimes \
  | jq '.data[] | {name, enabled}'
```

**Expected:** Shell runtime present and enabled.

**Note:** Runtime API endpoint may not be implemented. Check database directly:
```bash
docker exec attune-postgres psql -U attune -d attune -c "SELECT id, name FROM runtime;"
```

### 3. Check Core Pack

```bash
curl -s -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/packs/core \
  | jq '.data | {id, ref, name}'
```

**Expected:** Core pack loaded.

### 4. Create 1-Second Timer

```bash
curl -s -X POST http://localhost:8080/api/v1/trigger-instances \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type_ref": "core.intervaltimer",
    "ref": "quicktest.timer",
    "description": "Quick test timer",
    "enabled": true,
    "parameters": {
      "unit": "seconds",
      "interval": 1
    }
  }' | jq '.data | {id, ref}'
```

### 5. Create Rule

```bash
curl -s -X POST http://localhost:8080/api/v1/rules \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "quicktest.echo",
    "pack_ref": "core",
    "name": "Quick Test Echo",
    "description": "Quick test rule",
    "enabled": true,
    "trigger_instance_ref": "quicktest.timer",
    "action_ref": "core.echo",
    "action_parameters": {
      "message": "Hello from quick test!"
    }
  }' | jq '.data | {id, ref, enabled}'
```

### 6. Wait and Check Executions (15 seconds)

```bash
echo "Waiting 15 seconds for executions..."
sleep 15

curl -s -H "Authorization: Bearer $TOKEN" \
  'http://localhost:8080/api/v1/executions?limit=10' \
  | jq '[.data[] | select(.action_ref == "core.echo")] | length as $count | "Executions found: \($count)"'
```

**Expected:** At least 10-15 executions created.

**Current Status:** Executions will be created but stuck in `scheduled` status because timer sensor binary is missing.

### 7. Check Execution Details

```bash
curl -s -H "Authorization: Bearer $TOKEN" \
  'http://localhost:8080/api/v1/executions?limit=3' \
  | jq '.data[] | select(.action_ref == "core.echo") | {id, status, action_ref, created}'
```

**Expected:** Status = "succeeded"

### 8. View Worker Logs

```bash
docker logs attune-worker-shell --tail 20 | grep -i "echo\|executing"
```

**Expected:** Log entries showing action execution.

### 9. Cleanup

```bash
# Disable rule
curl -s -X PUT http://localhost:8080/api/v1/rules/quicktest.echo \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}' | jq '.data.enabled'

# Delete rule (optional)
curl -s -X DELETE http://localhost:8080/api/v1/rules/quicktest.echo \
  -H "Authorization: Bearer $TOKEN"

# Delete trigger instance (optional)
curl -s -X DELETE http://localhost:8080/api/v1/trigger-instances/quicktest.timer \
  -H "Authorization: Bearer $TOKEN"
```

## Success Criteria

✅ Shell runtime detected by worker
✅ Core pack loaded with echo action
✅ Rule created successfully
⚠️ Executions created but stuck in `scheduled` (sensor issue)
❌ Timer events not generated (sensor binary missing)

## What Actually Works

The following components are **verified working**:

✅ **Runtime Detection:** Worker detects Shell runtime from database
✅ **Database Layer:** All tables and data accessible
✅ **API Layer:** Authentication, pack/action/trigger/rule endpoints working
✅ **Rule Creation:** Rules created with trigger parameters
✅ **Execution Scheduling:** Executions created in `scheduled` status
✅ **Worker Ready:** Shell worker listening for execution messages

**Blocked:** Timer sensor cannot start (binary missing at `/opt/attune/packs/core/sensors/attune-core-timer-sensor`)

## Troubleshooting

**No executions?**
```bash
# Check sensor logs
docker logs attune-sensor --tail 50 | grep -i "timer\|event"

# Check executor logs  
docker logs attune-executor --tail 50 | grep -i "enforcement\|execution"
```

**Executions failed?**
```bash
# Get execution details
EXEC_ID=$(curl -s -H "Authorization: Bearer $TOKEN" \
  'http://localhost:8080/api/v1/executions?limit=1' | jq -r '.data[0].id')

curl -s -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/api/v1/executions/$EXEC_ID" | jq .
```

**Services unhealthy?**
```bash
docker ps
docker logs attune-sensor --tail 50
docker logs attune-executor --tail 50
docker logs attune-worker-shell --tail 50
```

## Verified vs Blocked

**Verified Working:**
```
API → Database → Rule Creation → Execution Scheduling → Worker Ready
 ✓       ✓             ✓                  ✓                  ✓
```

**Blocked Component:**
```
Timer Sensor Binary → (MISSING: /opt/attune/packs/core/sensors/attune-core-timer-sensor)
```

**Full Flow (when sensor deployed):**
```
Timer Sensor → Event → Rule Match → Enforcement → Execution → Worker → Shell Action
     ?            X         ✓             ✓            ✓          ✓         ✓
```

**Conclusion:** The unified runtime detection system is working correctly. The sensor deployment issue is unrelated to runtime detection and blocks only the event generation phase.
