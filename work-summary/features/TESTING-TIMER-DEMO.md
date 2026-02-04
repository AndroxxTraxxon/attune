# Testing Guide: Timer Trigger Demo

**Status:** ✅ Ready to Test  
**Date:** 2025-01-18  
**Services Required:** PostgreSQL, RabbitMQ, Valkey (all running)

## Quick Start

The fastest way to test the timer demo:

```bash
# 1. Start all services (in tmux)
./scripts/start_services_test.sh

# 2. Wait 30-60 seconds for compilation and startup

# 3. In a new terminal, create the timer rule
./scripts/setup_timer_echo_rule.sh

# 4. Watch the worker logs - should see "Hello World" every 10 seconds
```

## Prerequisites Checklist

✅ PostgreSQL running on port 5432  
✅ RabbitMQ running on port 5672  
✅ Valkey/Redis running on port 6379  
✅ Database schema migrated  
✅ Core pack loaded  
✅ Admin user created (login: admin, password: admin)  
✅ SQLx query cache prepared

## Detailed Setup (Already Complete)

These steps have already been completed:

### 1. Database Setup ✅
```bash
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
sqlx database create
sqlx migrate run
```

### 2. Load Core Pack ✅
```bash
psql $DATABASE_URL -f scripts/seed_core_pack.sql
```

This created:
- Core pack (ID: 1)
- Shell runtime (ID: 3)
- Timer triggers: `core.timer_10s`, `core.timer_1m`, `core.timer_hourly`
- Actions: `core.echo`, `core.sleep`, `core.noop`

### 3. Admin User ✅
```
Login: admin
Password: admin
```

### 4. SQLx Query Cache ✅
```bash
cd crates/sensor
cargo sqlx prepare
```

## Running the Demo

### Option 1: Using tmux (Recommended)

```bash
# Start all services in one command
./scripts/start_services_test.sh

# This will:
# - Create a tmux session named 'attune'
# - Start 4 services in separate panes:
#   ┌─────────────┬─────────────┐
#   │ API         │ Sensor      │
#   ├─────────────┼─────────────┤
#   │ Executor    │ Worker      │
#   └─────────────┴─────────────┘
# - Auto-attach to the session
```

**Tmux Controls:**
- `Ctrl+b, arrow keys` - Switch between panes
- `Ctrl+b, d` - Detach from session (services keep running)
- `tmux attach -t attune` - Reattach to session
- `tmux kill-session -t attune` - Stop all services

### Option 2: Manual (4 Terminals)

Set environment variables in each terminal:
```bash
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
export ATTUNE__DATABASE__URL="$DATABASE_URL"
export ATTUNE__MESSAGE_QUEUE__URL="amqp://guest:guest@localhost:5672/%2F"
export ATTUNE__JWT__SECRET="dev-secret-not-for-production"
```

**Terminal 1 - API:**
```bash
cargo run --bin attune-api
# Wait for: "Attune API Server listening on 127.0.0.1:8080"
```

**Terminal 2 - Sensor:**
```bash
cargo run --bin attune-sensor
# Wait for: "Started X timer triggers"
```

**Terminal 3 - Executor:**
```bash
cargo run --bin attune-executor
# Wait for: "Executor Service initialized successfully"
```

**Terminal 4 - Worker:**
```bash
cargo run --bin attune-worker
# Wait for: "Attune Worker Service is ready"
```

## Create the Timer Rule

Once all services are running:

```bash
# In a new terminal
./scripts/setup_timer_echo_rule.sh
```

This will:
1. Authenticate as admin
2. Verify core pack, trigger, and action exist
3. Create rule: `core.timer_echo_10s`
4. Configure it to echo "Hello World from timer trigger!" every 10 seconds

## Verify It's Working

### Watch Logs

**Sensor Service (every 10 seconds):**
```
[DEBUG] Interval timer core.timer_10s fired
[INFO] Generated event 123 from timer trigger core.timer_10s
```

**Executor Service:**
```
[INFO] Processing enforcement 456
[INFO] Scheduling execution for action core.echo
[INFO] Execution scheduled: 789
```

**Worker Service (every 10 seconds):**
```
[INFO] Received execution request: 789
[INFO] Executing action core.echo
[INFO] Action completed successfully
```

### Query via API

```bash
# Get auth token
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin"}' | jq -r '.data.access_token')

# List recent executions
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/executions | jq '.data[0:5]'

# Get specific execution
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/executions/789 | jq
```

### Check Database

```bash
psql $DATABASE_URL << 'EOF'
-- Count events from timer
SELECT COUNT(*) as event_count 
FROM attune.event 
WHERE trigger_ref = 'core.timer_10s';

-- Recent executions
SELECT id, status, created 
FROM attune.execution 
ORDER BY created DESC 
LIMIT 5;

-- Rule status
SELECT id, ref, enabled 
FROM attune.rule 
WHERE ref = 'core.timer_echo_10s';
EOF
```

## Expected Output

Every 10 seconds you should see:

1. **Sensor logs:** Timer fires, event generated
2. **Executor logs:** Enforcement processed, execution scheduled
3. **Worker logs:** Action executed, "Hello World from timer trigger!" output
4. **Database:** New event, enforcement, and execution records

## Troubleshooting

### Timer Not Firing

**Check sensor service logs:**
```
grep "Started.*timer" <sensor-log-file>
```

Expected: `Started X timer triggers`

**Verify trigger in database:**
```bash
psql $DATABASE_URL -c "SELECT id, ref, enabled FROM attune.trigger WHERE ref = 'core.timer_10s';"
```

Should show: `enabled = true`

### No Executions Created

**Check if rule exists:**
```bash
psql $DATABASE_URL -c "SELECT * FROM attune.rule WHERE ref = 'core.timer_echo_10s';"
```

**Check for events:**
```bash
psql $DATABASE_URL -c "SELECT COUNT(*) FROM attune.event WHERE trigger_ref = 'core.timer_10s';"
```

**Check for enforcements:**
```bash
psql $DATABASE_URL -c "SELECT COUNT(*) FROM attune.enforcement WHERE rule_ref = 'core.timer_echo_10s';"
```

### Worker Not Executing

**Verify worker is connected:**
Check worker logs for "Attune Worker Service is ready"

**Check execution status:**
```bash
psql $DATABASE_URL -c "SELECT id, status FROM attune.execution ORDER BY created DESC LIMIT 5;"
```

Should show `status = 'completed'`

**Check runtime exists:**
```bash
psql $DATABASE_URL -c "SELECT id, ref, name FROM attune.runtime WHERE ref = 'core.action.shell';"
```

### Service Connection Issues

**PostgreSQL:**
```bash
psql $DATABASE_URL -c "SELECT 1;"
```

**RabbitMQ:**
```bash
curl -u guest:guest http://localhost:15672/api/overview
```

**Check service logs for connection errors**

## Experimentation

### Change Timer Interval

```bash
psql $DATABASE_URL << 'EOF'
UPDATE attune.trigger
SET param_schema = '{"type": "interval", "seconds": 5}'
WHERE ref = 'core.timer_10s';
EOF

# Restart sensor service to pick up changes
```

### Change Echo Message

```bash
curl -X PUT http://localhost:8080/api/v1/rules/core.timer_echo_10s \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "action_params": {
      "message": "Testing timer automation!"
    }
  }'
```

### Create Hourly Timer Rule

```bash
curl -X POST http://localhost:8080/api/v1/rules \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "core.hourly_test",
    "pack": 1,
    "pack_ref": "core",
    "label": "Hourly Test",
    "description": "Runs every hour",
    "trigger_ref": "core.timer_hourly",
    "action_ref": "core.echo",
    "action_params": {
      "message": "Hourly chime!"
    }
  }'
```

### Disable Rule

```bash
curl -X PUT http://localhost:8080/api/v1/rules/core.timer_echo_10s \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'
```

## Clean Up

### Stop Services

**If using tmux:**
```bash
tmux kill-session -t attune
```

**If using manual terminals:**
Press `Ctrl+C` in each terminal

### Clean Up Test Data

```bash
psql $DATABASE_URL << 'EOF'
-- Remove test executions
DELETE FROM attune.execution WHERE created < NOW() - INTERVAL '1 hour';

-- Remove test events
DELETE FROM attune.event WHERE created < NOW() - INTERVAL '1 hour';

-- Remove test enforcements
DELETE FROM attune.enforcement WHERE created < NOW() - INTERVAL '1 hour';

-- Disable rule
UPDATE attune.rule SET enabled = false WHERE ref = 'core.timer_echo_10s';
EOF
```

### Reset Everything (Optional)

```bash
psql $DATABASE_URL << 'EOF'
DROP SCHEMA attune CASCADE;
EOF

# Then re-run migrations and seed data
sqlx migrate run
psql $DATABASE_URL -f scripts/seed_core_pack.sql
```

## Success Criteria

✅ All services start without errors  
✅ Timer fires every 10 seconds (visible in sensor logs)  
✅ Events created in database  
✅ Rules matched and enforcements created  
✅ Executions scheduled by executor  
✅ Worker executes echo action  
✅ "Hello World" appears in worker logs every 10 seconds  
✅ API queries return execution history  

## Known Issues

1. **Timer drift**: Long-running interval timers may drift slightly over time
2. **Configuration reload**: Changes to timer triggers require sensor service restart
3. **One-shot persistence**: One-shot timers don't persist across service restarts

## Next Steps

After confirming the timer demo works:

1. **Test other timer types**: Try cron and one-shot timers
2. **Create custom actions**: Write Python or Node.js actions
3. **Add rule conditions**: Filter when rules execute
4. **Build workflows**: Chain multiple actions together
5. **Implement policies**: Add concurrency limits, rate limiting
6. **Add monitoring**: Set up metrics and alerting

## Reference

- **Quick Start Guide**: `docs/quickstart-timer-demo.md`
- **Implementation Details**: `work-summary/2025-01-18-timer-triggers.md`
- **API Documentation**: `docs/api-overview.md`
- **Architecture**: `docs/architecture.md`

---

**Last Updated:** 2025-01-18  
**Status:** ✅ All prerequisites complete, ready for testing
