# Quick Reference: Worker Lifecycle & Heartbeat Validation

**Last Updated:** 2026-02-04  
**Status:** Production Ready

## Overview

Workers use graceful shutdown and heartbeat validation to ensure reliable execution scheduling.

## Worker Lifecycle

### Startup
1. Load configuration
2. Connect to database and message queue
3. Detect runtime capabilities
4. Register in database (status = `Active`)
5. Start heartbeat loop
6. Start consuming execution messages

### Normal Operation
- **Heartbeat:** Updates `worker.last_heartbeat` every 30 seconds (default)
- **Status:** Remains `Active`
- **Executions:** Processes messages from worker-specific queue

### Shutdown (Graceful)
1. Receive SIGINT or SIGTERM signal
2. Stop heartbeat loop
3. Mark worker as `Inactive` in database
4. Exit cleanly

### Shutdown (Crash/Kill)
- Worker does not deregister
- Status remains `Active` in database
- Heartbeat stops updating
- **Executor detects as stale after 90 seconds**

## Heartbeat Validation

### Configuration
```yaml
worker:
  heartbeat_interval: 30  # seconds (default)
```

### Staleness Threshold
- **Formula:** `heartbeat_interval * 3 = 90 seconds`
- **Rationale:** Allows 2 missed heartbeats + buffer
- **Detection:** Executor checks on every scheduling attempt

### Worker States

| Last Heartbeat Age | Status | Schedulable |
|-------------------|--------|-------------|
| < 90 seconds      | Fresh  | ✅ Yes      |
| ≥ 90 seconds      | Stale  | ❌ No       |
| None/NULL         | Stale  | ❌ No       |

## Executor Scheduling Flow

```
Execution Requested
    ↓
Find Action Workers
    ↓
Filter by Runtime Compatibility
    ↓
Filter by Active Status
    ↓
Filter by Heartbeat Freshness ← NEW
    ↓
Select Best Worker
    ↓
Queue to Worker
```

## Signal Handling

### Supported Signals
- **SIGINT** (Ctrl+C) - Graceful shutdown
- **SIGTERM** (docker stop, k8s termination) - Graceful shutdown
- **SIGKILL** (force kill) - No cleanup possible

### Docker Example
```bash
# Graceful shutdown (10s grace period)
docker compose stop worker-shell

# Force kill (immediate)
docker compose kill worker-shell
```

### Kubernetes Example
```yaml
spec:
  terminationGracePeriodSeconds: 30  # Time for graceful shutdown
```

## Monitoring & Debugging

### Check Worker Status
```sql
SELECT id, name, status, last_heartbeat,
       EXTRACT(EPOCH FROM (NOW() - last_heartbeat)) as seconds_ago
FROM worker
WHERE worker_role = 'action'
ORDER BY last_heartbeat DESC;
```

### Identify Stale Workers
```sql
SELECT id, name, status,
       EXTRACT(EPOCH FROM (NOW() - last_heartbeat)) as seconds_ago
FROM worker
WHERE worker_role = 'action'
  AND status = 'active'
  AND (last_heartbeat IS NULL OR last_heartbeat < NOW() - INTERVAL '90 seconds');
```

### View Worker Logs
```bash
# Docker Compose
docker compose logs -f worker-shell

# Look for:
# - "Worker registered with ID: X"
# - "Heartbeat sent successfully" (debug level)
# - "Received SIGTERM signal"
# - "Deregistering worker ID: X"
```

### View Executor Logs
```bash
docker compose logs -f executor

# Look for:
# - "Worker X heartbeat is stale: last seen N seconds ago"
# - "No workers with fresh heartbeats available"
```

## Common Issues

### Issue: "No workers with fresh heartbeats available"

**Causes:**
1. All workers crashed/terminated
2. Workers paused/frozen
3. Network partition between workers and database
4. Database connection issues

**Solutions:**
1. Check if workers are running: `docker compose ps`
2. Restart workers: `docker compose restart worker-shell`
3. Check worker logs for errors
4. Verify database connectivity

### Issue: Worker not deregistering on shutdown

**Causes:**
1. SIGKILL used instead of SIGTERM
2. Grace period too short
3. Database connection lost before deregister

**Solutions:**
1. Use `docker compose stop` not `docker compose kill`
2. Increase grace period: `docker compose down -t 30`
3. Check network connectivity

### Issue: Worker stuck in Active status after crash

**Behavior:** Normal - executor will detect as stale after 90s

**Manual Cleanup (if needed):**
```sql
UPDATE worker
SET status = 'inactive'
WHERE last_heartbeat < NOW() - INTERVAL '5 minutes';
```

## Testing

### Test Graceful Shutdown
```bash
# Start worker
docker compose up -d worker-shell

# Wait for registration
sleep 5

# Check status (should be 'active')
docker compose exec postgres psql -U attune -c \
  "SELECT name, status FROM worker WHERE name LIKE 'worker-shell%';"

# Graceful shutdown
docker compose stop worker-shell

# Check status (should be 'inactive')
docker compose exec postgres psql -U attune -c \
  "SELECT name, status FROM worker WHERE name LIKE 'worker-shell%';"
```

### Test Heartbeat Validation
```bash
# Pause worker (simulate freeze)
docker compose pause worker-shell

# Wait for staleness (90+ seconds)
sleep 100

# Try to schedule execution (should fail)
# Use API or CLI to trigger execution
attune execution create --action core.echo --param message="test"

# Should see: "No workers with fresh heartbeats available"
```

## Configuration Reference

### Worker Config
```yaml
worker:
  name: "worker-01"
  heartbeat_interval: 30      # Heartbeat update frequency (seconds)
  max_concurrent_tasks: 10    # Concurrent execution limit
  task_timeout: 300           # Per-task timeout (seconds)
```

### Relevant Constants
```rust
// crates/executor/src/scheduler.rs
const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;
const HEARTBEAT_STALENESS_MULTIPLIER: u64 = 3;
// Max age = 90 seconds
```

## Best Practices

1. **Use Graceful Shutdown:** Always use SIGTERM, not SIGKILL
2. **Monitor Heartbeats:** Alert when workers go stale
3. **Set Grace Periods:** Allow 10-30s for worker shutdown in production
4. **Health Checks:** Implement liveness probes in Kubernetes
5. **Auto-Restart:** Configure restart policies for crashed workers

## Related Documentation

- `work-summary/2026-02-worker-graceful-shutdown-heartbeat-validation.md` - Implementation details
- `docs/architecture/worker-service.md` - Worker architecture
- `docs/architecture/executor-service.md` - Executor architecture
- `AGENTS.md` - Project conventions

## Future Enhancements

- [ ] Configurable staleness multiplier
- [ ] Active health probing
- [ ] Graceful work completion before shutdown
- [ ] Worker reconnection logic
- [ ] Load-based worker selection