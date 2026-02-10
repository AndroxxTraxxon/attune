# Quick Reference: Worker Queue TTL and Dead Letter Queue (Phase 2)

## Overview

Phase 2 implements message TTL on worker queues and dead letter queue processing to automatically fail executions when workers are unavailable.

**Key Concept:** If a worker doesn't process an execution within 5 minutes, the message expires and the execution is automatically marked as FAILED.

## How It Works

```
Execution → Worker Queue (TTL: 5 min) → Worker Processing ✓
                    ↓ (if timeout)
              Dead Letter Exchange
                    ↓
              Dead Letter Queue
                    ↓
            DLQ Handler (in Executor)
                    ↓
          Execution marked FAILED
```

## Configuration

### Default Settings (All Environments)

```yaml
message_queue:
  rabbitmq:
    worker_queue_ttl_ms: 300000  # 5 minutes
    dead_letter:
      enabled: true
      exchange: attune.dlx
      ttl_ms: 86400000  # 24 hours DLQ retention
```

### Tuning TTL

**Worker Queue TTL** (`worker_queue_ttl_ms`):
- **Default:** 300000 (5 minutes)
- **Purpose:** How long to wait before declaring worker unavailable
- **Tuning:** Set to 2-5x your typical execution time
- **Too short:** Slow executions fail prematurely
- **Too long:** Delayed failure detection for unavailable workers

**DLQ Retention** (`dead_letter.ttl_ms`):
- **Default:** 86400000 (24 hours)
- **Purpose:** How long to keep expired messages for debugging
- **Tuning:** Based on your debugging/forensics needs

## Components

### 1. Worker Queue TTL

- Applied to all `worker.{id}.executions` queues
- Configured via RabbitMQ queue argument `x-message-ttl`
- Messages expire if not consumed within TTL
- Expired messages routed to dead letter exchange

### 2. Dead Letter Exchange (DLX)

- **Name:** `attune.dlx`
- **Type:** `direct`
- Receives all expired messages from worker queues
- Routes to dead letter queue

### 3. Dead Letter Queue (DLQ)

- **Name:** `attune.dlx.queue`
- Stores expired messages for processing
- Retains messages for 24 hours (configurable)
- Processed by dead letter handler

### 4. Dead Letter Handler

- Runs in executor service
- Consumes messages from DLQ
- Updates executions to FAILED status
- Provides descriptive error messages

## Monitoring

### Key Metrics

```bash
# Check DLQ depth
rabbitmqadmin list queues name messages | grep attune.dlx.queue

# View DLQ rate
# Watch for sustained DLQ message rate > 10/min

# Check failed executions
curl http://localhost:8080/api/v1/executions?status=failed
```

### Health Checks

**Good:**
- DLQ depth: 0-10
- DLQ rate: < 5 messages/min
- Most executions complete successfully

**Warning:**
- DLQ depth: 10-100
- DLQ rate: 5-20 messages/min
- May indicate worker instability

**Critical:**
- DLQ depth: > 100
- DLQ rate: > 20 messages/min
- Workers likely down or overloaded

## Troubleshooting

### High DLQ Rate

**Symptoms:** Many executions failing via DLQ

**Common Causes:**
1. Workers stopped or restarting
2. Workers overloaded (not consuming fast enough)
3. TTL too aggressive for your workload
4. Network connectivity issues

**Resolution:**
```bash
# 1. Check worker status
docker compose ps | grep worker
docker compose logs -f worker-shell

# 2. Verify worker heartbeats
psql -c "SELECT name, status, last_heartbeat FROM worker;"

# 3. Check worker queue depths
rabbitmqadmin list queues name messages | grep "worker\."

# 4. Consider increasing TTL if legitimate slow executions
# Edit config and restart executor:
#   worker_queue_ttl_ms: 600000  # 10 minutes
```

### DLQ Not Processing

**Symptoms:** DLQ depth increasing, executions stuck

**Common Causes:**
1. Executor service not running
2. DLQ disabled in config
3. Database connection issues

**Resolution:**
```bash
# 1. Verify executor is running
docker compose ps executor
docker compose logs -f executor | grep "dead letter"

# 2. Check configuration
grep -A 3 "dead_letter:" config.docker.yaml

# 3. Restart executor if needed
docker compose restart executor
```

### Messages Not Expiring

**Symptoms:** Executions stuck in SCHEDULED, DLQ empty

**Common Causes:**
1. Worker queues not configured with TTL
2. Worker queues not configured with DLX
3. Infrastructure setup failed

**Resolution:**
```bash
# 1. Check queue properties
rabbitmqadmin show queue name=worker.1.executions

# Look for:
# - arguments.x-message-ttl: 300000
# - arguments.x-dead-letter-exchange: attune.dlx

# 2. Recreate infrastructure (safe, idempotent)
docker compose restart executor worker-shell
```

## Testing

### Manual Test: Verify TTL Expiration

```bash
# 1. Stop all workers
docker compose stop worker-shell worker-python worker-node

# 2. Create execution
curl -X POST http://localhost:8080/api/v1/executions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "action_ref": "core.echo",
    "parameters": {"message": "test"}
  }'

# 3. Wait for TTL expiration (5+ minutes)
sleep 330

# 4. Check execution status
curl http://localhost:8080/api/v1/executions/{id} | jq '.data.status'
# Should be "failed"

# 5. Check error message
curl http://localhost:8080/api/v1/executions/{id} | jq '.data.result'
# Should contain "Worker queue TTL expired"

# 6. Verify DLQ processed it
rabbitmqadmin list queues name messages | grep attune.dlx.queue
# Should show 0 messages (processed and removed)
```

## Relationship to Phase 1

**Phase 1 (Timeout Monitor):**
- Monitors executions in SCHEDULED state
- Fails executions after configured timeout
- Acts as backup safety net

**Phase 2 (Queue TTL + DLQ):**
- Expires messages at queue level
- More precise failure detection
- Provides better visibility (DLQ metrics)

**Together:** Provide defense-in-depth for worker unavailability

## Common Operations

### View DLQ Messages

```bash
# Get messages from DLQ (doesn't remove)
rabbitmqadmin get queue=attune.dlx.queue count=10

# View x-death header for expiration details
rabbitmqadmin get queue=attune.dlx.queue count=1 --format=long
```

### Manually Purge DLQ

```bash
# Use with caution - removes all messages
rabbitmqadmin purge queue name=attune.dlx.queue
```

### Temporarily Disable DLQ

```yaml
# config.docker.yaml
message_queue:
  rabbitmq:
    dead_letter:
      enabled: false  # Disables DLQ handler
```

**Note:** Messages will still expire but won't be processed

### Adjust TTL Without Restart

Not possible - queue TTL is set at queue creation time. To change:

```bash
# 1. Stop all services
docker compose down

# 2. Delete worker queues (forces recreation)
rabbitmqadmin delete queue name=worker.1.executions
# Repeat for all worker queues

# 3. Update config
# Edit worker_queue_ttl_ms

# 4. Restart services (queues recreated with new TTL)
docker compose up -d
```

## Key Files

### Configuration
- `config.docker.yaml` - Production settings
- `config.development.yaml` - Development settings

### Implementation
- `crates/common/src/mq/config.rs` - TTL configuration
- `crates/common/src/mq/connection.rs` - Queue setup with TTL
- `crates/executor/src/dead_letter_handler.rs` - DLQ processing
- `crates/executor/src/service.rs` - DLQ handler integration

### Documentation
- `docs/architecture/worker-queue-ttl-dlq.md` - Full architecture
- `docs/architecture/worker-availability-handling.md` - Phase 1 (backup)

## When to Use

**Enable DLQ (default):**
- Production environments
- Development with multiple workers
- Any environment requiring high reliability

**Disable DLQ:**
- Local development with single worker
- Testing scenarios where you want manual control
- Debugging worker behavior

## Next Steps (Phase 3)

- **Health probes:** Proactive worker health checking
- **Intelligent retry:** Retry transient failures
- **Per-action TTL:** Custom timeouts per action type
- **DLQ analytics:** Aggregate failure statistics

## See Also

- Phase 1 Documentation: `docs/architecture/worker-availability-handling.md`
- Queue Architecture: `docs/architecture/queue-architecture.md`
- RabbitMQ Dead Letter Exchanges: https://www.rabbitmq.com/dlx.html