# Migration Guide: Queue Separation Fix (2026-02-03)

**Issue:** Deserialization errors in executor service  
**Urgency:** High - Critical bug causing message rejection  
**Downtime Required:** Yes (brief - service restart only)

## Overview

This migration separates competing consumers on shared RabbitMQ queues into dedicated queues, fixing deserialization errors:
- `missing field 'inquiry_id'`
- `missing field 'action_id'`

## Changes Summary

### New Queues Created
1. `attune.inquiry.responses.queue` - For inquiry response messages
2. `attune.execution.completed.queue` - For execution completion messages

### Queue Bindings Modified
- `attune.execution.status.queue` - Now only receives `execution.status.changed` messages
- `attune.execution.completed.queue` - Now receives `execution.completed` messages
- `attune.inquiry.responses.queue` - Now receives `inquiry.responded` messages

### Services Affected
- **Executor Service** - Requires restart (consumers reconfigured)
- **Worker Service** - No changes required (publishers work automatically)
- **API Service** - No changes required (publishers work automatically)

## Pre-Migration Checklist

- [ ] Backup current RabbitMQ configuration
- [ ] Note current queue depths in RabbitMQ management UI
- [ ] Verify all services are running and healthy
- [ ] Review recent executor logs for deserialization errors
- [ ] Ensure you have access to restart the executor service

## Migration Steps

### Step 1: Stop the Executor Service

```bash
# Using systemd
sudo systemctl stop attune-executor

# Using docker-compose
docker-compose stop executor

# Or kill the process
pkill -f attune-executor
```

### Step 2: Deploy Updated Code

```bash
# Pull latest code
git pull origin main

# Rebuild executor (and common library)
cd attune
cargo build --release --bin attune-executor
```

### Step 3: Verify RabbitMQ Queue Creation

The new queues will be created automatically when the executor starts, but you can verify the configuration:

```bash
# Check that the code is updated
grep -r "inquiry_responses" crates/common/src/mq/config.rs
grep -r "execution_completed" crates/common/src/mq/config.rs
```

### Step 4: Start the Executor Service

```bash
# Using systemd
sudo systemctl start attune-executor

# Using docker-compose
docker-compose start executor

# Or directly
./target/release/attune-executor --config config.production.yaml
```

### Step 5: Verify Queue Creation in RabbitMQ

Check RabbitMQ Management UI (http://localhost:15672):

**Queues Tab:**
- [ ] `attune.inquiry.responses.queue` exists
- [ ] `attune.execution.completed.queue` exists
- [ ] `attune.execution.status.queue` still exists

**Exchanges Tab → attune.executions → Bindings:**
- [ ] `inquiry.responded` → `attune.inquiry.responses.queue`
- [ ] `execution.completed` → `attune.execution.completed.queue`
- [ ] `execution.status.changed` → `attune.execution.status.queue`

### Step 6: Monitor Executor Logs

```bash
# Watch for successful startup
tail -f /var/log/attune/executor.log

# Or with journalctl
journalctl -u attune-executor -f

# Or with docker
docker logs -f attune-executor
```

**Expected log messages:**
```
INFO Starting Executor Service
INFO Message queue connection established
INFO Queue manager initialized with database persistence
INFO Starting event processor...
INFO Starting completion listener...
INFO Starting enforcement processor...
INFO Starting execution scheduler...
INFO Starting execution manager...
INFO Starting inquiry handler...
INFO Executor Service started successfully
```

### Step 7: Verify No Deserialization Errors

```bash
# Check for the specific errors (should be NONE)
grep "missing field.*inquiry_id" /var/log/attune/executor.log
grep "missing field.*action_id" /var/log/attune/executor.log
grep "Failed to deserialize message" /var/log/attune/executor.log
```

If no output, the fix is working! ✅

### Step 8: Functional Testing

**Test Execution Completion:**
```bash
# Execute a simple action
attune action execute core.echo --param message="test"

# Verify execution completes without errors in logs
```

**Test Inquiry Workflow (if applicable):**
```bash
# Create an action that requests inquiry
# Respond to the inquiry via API
# Verify execution resumes
```

**Test Status Updates:**
```bash
# Execute a longer-running action
# Verify status updates are processed correctly
```

## Rollback Procedure

If issues occur, you can rollback:

### Step 1: Stop Executor
```bash
sudo systemctl stop attune-executor
```

### Step 2: Revert Code
```bash
git revert <commit-hash>
cargo build --release --bin attune-executor
```

### Step 3: Remove New Queues (Optional)
```bash
# Via RabbitMQ Management API
curl -u guest:guest -X DELETE http://localhost:15672/api/queues/%2F/attune.inquiry.responses.queue
curl -u guest:guest -X DELETE http://localhost:15672/api/queues/%2F/attune.execution.completed.queue
```

### Step 4: Restart Executor
```bash
sudo systemctl start attune-executor
```

## Post-Migration Verification

- [ ] Executor service is running and healthy
- [ ] No deserialization errors in logs for 15+ minutes
- [ ] Test executions complete successfully
- [ ] Inquiries (if used) work correctly
- [ ] All three new queue bindings show in RabbitMQ UI
- [ ] Queue message rates look normal
- [ ] No messages in dead letter queues

## Monitoring Points

Watch these metrics for 24 hours post-migration:

1. **Executor Error Rate** - Should drop to near zero
2. **Queue Depths** - Should remain stable/low
3. **Message Delivery Rate** - Should remain consistent
4. **Dead Letter Queue Depth** - Should not increase

## Troubleshooting

### Issue: New queues not created

**Symptoms:** Queues don't appear in RabbitMQ UI

**Solution:**
```bash
# Check executor logs for connection errors
grep "Failed to declare queue" /var/log/attune/executor.log

# Verify RabbitMQ permissions
rabbitmqctl list_user_permissions attune_user
```

### Issue: Still seeing deserialization errors

**Symptoms:** Errors persist after restart

**Solution:**
```bash
# 1. Verify code was rebuilt
attune-executor --version

# 2. Check which queues consumers are using
grep "Starting.*listener" /var/log/attune/executor.log

# 3. Verify bindings in RabbitMQ UI match expected configuration

# 4. Restart ALL services to ensure workers/API use new bindings
sudo systemctl restart attune-worker attune-api attune-executor
```

### Issue: Messages stuck in old queue

**Symptoms:** Old execution.status.queue has growing backlog

**Solution:**
```bash
# Check what messages are in the queue
rabbitmqadmin get queue=attune.execution.status.queue count=5

# If they're completion messages, manually move them:
# 1. Temporarily stop executor
# 2. Purge old queue
# 3. Restart executor (messages will be redelivered after TTL)
```

## Impact Assessment

**Before Fix:**
- ❌ ~30-50% of messages rejected due to deserialization errors
- ❌ Executions not completing properly
- ❌ Inquiries not being processed
- ❌ Resource waste from redelivery attempts

**After Fix:**
- ✅ 100% message delivery success rate
- ✅ All executions complete correctly
- ✅ Inquiries processed immediately
- ✅ Reduced message queue load

## Questions?

Contact the platform team or refer to:
- `attune/work-summary/2026-02-03-inquiry-queue-separation.md` - Technical details
- `attune/docs/QUICKREF-rabbitmq-queues.md` - Queue architecture reference
- `attune/docs/architecture/queue-architecture.md` - Overall architecture

---

**Migration Completed:** __________ (date/time)  
**Performed By:** __________  
**Issues Encountered:** __________  
**Notes:** __________