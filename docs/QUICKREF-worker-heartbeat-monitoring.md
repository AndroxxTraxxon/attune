# Quick Reference: Worker Heartbeat Monitoring

**Purpose**: Automatically detect and deactivate workers that have stopped sending heartbeats

## Overview

The executor service includes a background task that monitors worker heartbeats and automatically marks stale workers as inactive. This prevents the scheduler from attempting to assign work to workers that are no longer available.

## How It Works

### Background Monitor Task

- **Location**: `crates/executor/src/service.rs` → `worker_heartbeat_monitor_loop()`
- **Check Interval**: Every 60 seconds
- **Staleness Threshold**: 90 seconds (3x the expected 30-second heartbeat interval)

### Detection Logic

The monitor checks all workers with `status = 'active'`:

1. **No Heartbeat**: Workers with `last_heartbeat = NULL` → marked inactive
2. **Stale Heartbeat**: Workers with heartbeat older than 90 seconds → marked inactive
3. **Fresh Heartbeat**: Workers with heartbeat within 90 seconds → remain active

### Automatic Deactivation

When a stale worker is detected:
- Worker status updated to `inactive` in database
- Warning logged with worker name, ID, and heartbeat age
- Summary logged with count of deactivated workers

## Configuration

### Constants (in scheduler.rs and service.rs)

```rust
DEFAULT_HEARTBEAT_INTERVAL: 30 seconds      // Expected worker heartbeat frequency
HEARTBEAT_STALENESS_MULTIPLIER: 3          // Grace period multiplier
MAX_STALENESS: 90 seconds                   // Calculated: 30 * 3
```

### Check Interval

Currently hardcoded to 60 seconds. Configured when spawning the monitor task:

```rust
Self::worker_heartbeat_monitor_loop(worker_pool, 60).await;
```

## Worker Lifecycle

### Normal Operation

```
Worker Starts → Registers → Sends Heartbeats (30s) → Remains Active
```

### Graceful Shutdown

```
Worker Stops → No More Heartbeats → Monitor Detects (60s) → Marked Inactive
```

### Crash/Network Failure

```
Worker Crashes → Heartbeats Stop → Monitor Detects (60s) → Marked Inactive
```

## Monitoring

### Check Active Workers

```sql
SELECT name, worker_role, status, last_heartbeat 
FROM worker 
WHERE status = 'active' 
ORDER BY last_heartbeat DESC;
```

### Check Recent Deactivations

```sql
SELECT name, worker_role, status, last_heartbeat, updated
FROM worker 
WHERE status = 'inactive' 
  AND updated > NOW() - INTERVAL '5 minutes'
ORDER BY updated DESC;
```

### Count Workers by Status

```sql
SELECT status, COUNT(*) 
FROM worker 
GROUP BY status;
```

## Logs

### Monitor Startup

```
INFO: Starting worker heartbeat monitor...
INFO: Worker heartbeat monitor started (check interval: 60s, staleness threshold: 90s)
```

### Worker Deactivation

```
WARN: Worker sensor-77cd23b50478 (ID: 27) heartbeat is stale (1289s old), marking as inactive
INFO: Deactivated 5 worker(s) with stale heartbeats
```

### Error Handling

```
ERROR: Failed to deactivate worker worker-123 (stale heartbeat): <error details>
ERROR: Failed to query active workers for heartbeat check: <error details>
```

## Scheduler Integration

The scheduler already filters out stale workers during worker selection:

```rust
// Filter by heartbeat freshness
let fresh_workers: Vec<_> = active_workers
    .into_iter()
    .filter(|w| Self::is_worker_heartbeat_fresh(w))
    .collect();
```

**Before Heartbeat Monitor**: Scheduler filtered at selection time, but workers stayed "active" in DB
**After Heartbeat Monitor**: Workers marked inactive in DB, scheduler sees accurate state

## Troubleshooting

### Workers Constantly Becoming Inactive

**Symptoms**: Active workers being marked inactive despite running
**Causes**:
- Worker heartbeat interval > 30 seconds
- Network issues preventing heartbeat messages
- Worker service crash loop

**Solutions**:
1. Check worker logs for heartbeat send attempts
2. Verify RabbitMQ connectivity
3. Check worker configuration for heartbeat interval

### Stale Workers Not Being Deactivated

**Symptoms**: Workers with old heartbeats remain active
**Causes**:
- Executor service not running
- Monitor task crashed

**Solutions**:
1. Check executor service logs
2. Verify monitor task started: `grep "heartbeat monitor started" executor.log`
3. Restart executor service

### Too Many Inactive Workers

**Symptoms**: Database has hundreds of inactive workers
**Causes**: Historical workers from development/testing

**Solutions**:
```sql
-- Delete inactive workers older than 7 days
DELETE FROM worker 
WHERE status = 'inactive' 
  AND updated < NOW() - INTERVAL '7 days';
```

## Best Practices

### Worker Registration

Workers should:
- Set appropriate unique name (hostname-based)
- Send heartbeat every 30 seconds
- Handle graceful shutdown (optional: mark self inactive)

### Database Maintenance

- Periodically clean up old inactive workers
- Monitor worker table growth
- Index on `status` and `last_heartbeat` for efficient queries

### Monitoring & Alerts

- Track worker deactivation rate (should be low in production)
- Alert on sudden increase in deactivations (infrastructure issue)
- Monitor active worker count vs. expected

## Related Documentation

- `docs/architecture/worker-service.md` - Worker architecture
- `docs/architecture/executor-service.md` - Executor architecture
- `docs/deployment/ops-runbook-queues.md` - Operational procedures
- `AGENTS.md` - Project rules and conventions

## Implementation Notes

### Why 90 Seconds?

- Worker sends heartbeat every 30 seconds
- 3x multiplier provides grace period for:
  - Network latency
  - Brief load spikes
  - Temporary connectivity issues
- Balances responsiveness vs. false positives

### Why Check Every 60 Seconds?

- Allows 1.5 heartbeat intervals between checks
- Reduces database query frequency
- Adequate response time (stale workers removed within ~2 minutes)

### Thread Safety

- Monitor runs in separate tokio task
- Uses connection pool for database access
- No shared mutable state
- Safe to run multiple executor instances (each monitors independently)