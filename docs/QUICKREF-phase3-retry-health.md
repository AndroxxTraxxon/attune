# Quick Reference: Phase 3 - Intelligent Retry & Worker Health

## Overview

Phase 3 adds intelligent retry logic and proactive worker health monitoring to automatically recover from transient failures and optimize worker selection.

**Key Features:**
- **Automatic Retry:** Failed executions automatically retry with exponential backoff
- **Health-Aware Scheduling:** Prefer healthy workers with low queue depth
- **Per-Action Configuration:** Custom timeouts and retry limits per action
- **Failure Classification:** Distinguish retriable vs non-retriable failures

## Quick Start

### Enable Retry for an Action

```yaml
# packs/mypack/actions/flaky-api.yaml
name: flaky_api_call
runtime: python
entrypoint: actions/flaky_api.py
timeout_seconds: 120      # Custom timeout (overrides global 5 min)
max_retries: 3            # Retry up to 3 times on failure
parameters:
  url:
    type: string
    required: true
```

### Database Migration

```bash
# Apply Phase 3 schema changes
sqlx migrate run

# Or via Docker Compose
docker compose exec postgres psql -U attune -d attune -f /migrations/20260209000000_phase3_retry_and_health.sql
```

### Check Worker Health

```bash
# View healthy workers
psql -c "SELECT * FROM healthy_workers;"

# Check specific worker health
psql -c "
SELECT 
    name,
    capabilities->'health'->>'status' as health_status,
    capabilities->'health'->>'queue_depth' as queue_depth,
    capabilities->'health'->>'consecutive_failures' as failures
FROM worker 
WHERE id = 1;
"
```

## Retry Behavior

### Retriable Failures

Executions are automatically retried for:
- ✓ Worker unavailable (`worker_unavailable`)
- ✓ Queue timeout/TTL expired (`queue_timeout`)
- ✓ Worker heartbeat stale (`worker_heartbeat_stale`)
- ✓ Transient errors (`transient_error`)
- ✓ Manual retry requested (`manual_retry`)

### Non-Retriable Failures

These failures are NOT retried:
- ✗ Validation errors
- ✗ Permission denied
- ✗ Action not found
- ✗ Invalid parameters
- ✗ Explicit action failure

### Retry Backoff

**Strategy:** Exponential backoff with jitter

```
Attempt 0: ~1 second
Attempt 1: ~2 seconds
Attempt 2: ~4 seconds
Attempt 3: ~8 seconds
Attempt N: min(base * 2^N, 300 seconds)
```

**Jitter:** ±20% randomization to avoid thundering herd

### Retry Configuration

```rust
// Default retry configuration
RetryConfig {
    enabled: true,
    base_backoff_secs: 1,
    max_backoff_secs: 300,       // 5 minutes max
    backoff_multiplier: 2.0,
    jitter_factor: 0.2,          // 20% jitter
}
```

## Worker Health

### Health States

**Healthy:**
- Heartbeat < 30 seconds old
- Consecutive failures < 3
- Queue depth < 50
- Failure rate < 30%

**Degraded:**
- Consecutive failures: 3-9
- Queue depth: 50-99
- Failure rate: 30-69%
- Still receives tasks but deprioritized

**Unhealthy:**
- Heartbeat > 30 seconds old
- Consecutive failures ≥ 10
- Queue depth ≥ 100
- Failure rate ≥ 70%
- Does NOT receive new tasks

### Health Metrics

Workers self-report health in capabilities:

```json
{
  "runtimes": ["shell", "python"],
  "health": {
    "status": "healthy",
    "last_check": "2026-02-09T12:00:00Z",
    "consecutive_failures": 0,
    "total_executions": 1000,
    "failed_executions": 20,
    "average_execution_time_ms": 1500,
    "queue_depth": 5
  }
}
```

### Worker Selection

**Selection Priority:**
1. Healthy workers (queue depth ascending)
2. Degraded workers (queue depth ascending)
3. Skip unhealthy workers

**Example:**
```
Worker A: Healthy, queue=5    ← Selected first
Worker B: Healthy, queue=20   ← Selected second
Worker C: Degraded, queue=10  ← Selected third
Worker D: Unhealthy, queue=0  ← Never selected
```

## Database Schema

### Execution Retry Fields

```sql
-- Added to execution table
retry_count INTEGER NOT NULL DEFAULT 0,
max_retries INTEGER,
retry_reason TEXT,
original_execution BIGINT REFERENCES execution(id)
```

### Action Configuration Fields

```sql
-- Added to action table
timeout_seconds INTEGER,          -- Per-action timeout override
max_retries INTEGER DEFAULT 0     -- Per-action retry limit
```

### Helper Functions

```sql
-- Check if execution can be retried
SELECT is_execution_retriable(123);

-- Get worker queue depth
SELECT get_worker_queue_depth(1);
```

### Views

```sql
-- Get all healthy workers
SELECT * FROM healthy_workers;
```

## Practical Examples

### Example 1: View Retry Chain

```sql
-- Find all retries for execution 100
WITH RECURSIVE retry_chain AS (
    SELECT id, retry_count, retry_reason, original_execution, status
    FROM execution
    WHERE id = 100
    
    UNION ALL
    
    SELECT e.id, e.retry_count, e.retry_reason, e.original_execution, e.status
    FROM execution e
    JOIN retry_chain rc ON e.original_execution = rc.id
)
SELECT * FROM retry_chain ORDER BY retry_count;
```

### Example 2: Analyze Retry Success Rate

```sql
-- Success rate of retries by reason
SELECT 
    config->>'retry_reason' as reason,
    COUNT(*) as total_retries,
    COUNT(CASE WHEN status = 'completed' THEN 1 END) as succeeded,
    ROUND(100.0 * COUNT(CASE WHEN status = 'completed' THEN 1 END) / COUNT(*), 2) as success_rate
FROM execution
WHERE retry_count > 0
GROUP BY config->>'retry_reason'
ORDER BY total_retries DESC;
```

### Example 3: Find Workers by Health

```sql
-- Workers sorted by health and load
SELECT 
    w.name,
    w.status,
    (w.capabilities->'health'->>'status')::TEXT as health,
    (w.capabilities->'health'->>'queue_depth')::INTEGER as queue,
    (w.capabilities->'health'->>'consecutive_failures')::INTEGER as failures,
    w.last_heartbeat
FROM worker w
WHERE w.status = 'active'
ORDER BY 
    CASE (w.capabilities->'health'->>'status')::TEXT
        WHEN 'healthy' THEN 1
        WHEN 'degraded' THEN 2
        WHEN 'unhealthy' THEN 3
        ELSE 4
    END,
    (w.capabilities->'health'->>'queue_depth')::INTEGER;
```

### Example 4: Manual Retry via API

```bash
# Create retry execution
curl -X POST http://localhost:8080/api/v1/executions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "action_ref": "core.echo",
    "parameters": {"message": "retry test"},
    "config": {
      "retry_of": 123,
      "retry_count": 1,
      "max_retries": 3,
      "retry_reason": "manual_retry",
      "original_execution": 123
    }
  }'
```

## Monitoring

### Key Metrics

**Retry Metrics:**
- Retry rate: % of executions that retry
- Retry success rate: % of retries that succeed
- Average retries per execution
- Retry reason distribution

**Health Metrics:**
- Healthy worker count
- Degraded worker count
- Unhealthy worker count
- Average queue depth per worker
- Average failure rate per worker

### SQL Queries

```sql
-- Retry rate over last hour
SELECT 
    COUNT(DISTINCT CASE WHEN retry_count = 0 THEN id END) as original_executions,
    COUNT(DISTINCT CASE WHEN retry_count > 0 THEN id END) as retry_executions,
    ROUND(100.0 * COUNT(DISTINCT CASE WHEN retry_count > 0 THEN id END) / 
          COUNT(DISTINCT CASE WHEN retry_count = 0 THEN id END), 2) as retry_rate
FROM execution
WHERE created > NOW() - INTERVAL '1 hour';

-- Worker health distribution
SELECT 
    COALESCE((capabilities->'health'->>'status')::TEXT, 'unknown') as health_status,
    COUNT(*) as worker_count,
    AVG((capabilities->'health'->>'queue_depth')::INTEGER) as avg_queue_depth
FROM worker
WHERE status = 'active'
GROUP BY health_status;
```

## Configuration

### Retry Configuration

```rust
// In executor service initialization
let retry_manager = RetryManager::new(pool.clone(), RetryConfig {
    enabled: true,
    base_backoff_secs: 1,
    max_backoff_secs: 300,
    backoff_multiplier: 2.0,
    jitter_factor: 0.2,
});
```

### Health Probe Configuration

```rust
// In executor service initialization
let health_probe = WorkerHealthProbe::new(pool.clone(), HealthProbeConfig {
    enabled: true,
    heartbeat_max_age_secs: 30,
    degraded_threshold: 3,
    unhealthy_threshold: 10,
    queue_depth_degraded: 50,
    queue_depth_unhealthy: 100,
    failure_rate_degraded: 0.3,
    failure_rate_unhealthy: 0.7,
});
```

## Troubleshooting

### High Retry Rate

**Symptoms:** Many executions retrying repeatedly

**Causes:**
- Workers unstable or frequently restarting
- Network issues causing transient failures
- Actions not idempotent (retry makes things worse)

**Resolution:**
1. Check worker stability: `docker compose ps`
2. Review action idempotency
3. Adjust `max_retries` if retries are unhelpful
4. Investigate root cause of failures

### Retries Not Triggering

**Symptoms:** Failed executions not retrying despite max_retries > 0

**Causes:**
- Action doesn't have `max_retries` set
- Failure is non-retriable (validation error, etc.)
- Global retry disabled

**Resolution:**
1. Check action configuration: `SELECT timeout_seconds, max_retries FROM action WHERE ref = 'action.name';`
2. Check failure message for retriable patterns
3. Verify retry enabled in executor config

### Workers Marked Unhealthy

**Symptoms:** Workers not receiving tasks

**Causes:**
- High queue depth (overloaded)
- Consecutive failures exceed threshold
- Heartbeat stale

**Resolution:**
1. Check worker logs: `docker compose logs -f worker-shell`
2. Verify heartbeat: `SELECT name, last_heartbeat FROM worker;`
3. Check queue depth in capabilities
4. Restart worker if stuck: `docker compose restart worker-shell`

### Retry Loops

**Symptoms:** Execution retries forever or excessive retries

**Causes:**
- Bug in retry reason detection
- Action failure always classified as retriable
- max_retries not being enforced

**Resolution:**
1. Check retry chain: See Example 1 above
2. Verify max_retries: `SELECT config FROM execution WHERE id = 123;`
3. Fix retry reason classification if incorrect
4. Manually fail execution if stuck

## Integration with Previous Phases

### Phase 1 + Phase 2 + Phase 3 Together

**Defense in Depth:**
1. **Phase 1 (Timeout Monitor):** Catches stuck SCHEDULED executions (30s-5min)
2. **Phase 2 (Queue TTL/DLQ):** Expires messages in worker queues (5min)
3. **Phase 3 (Intelligent Retry):** Retries retriable failures (1s-5min backoff)

**Failure Flow:**
```
Execution dispatched → Worker unavailable (Phase 2: 5min TTL)
    → DLQ handler marks FAILED (Phase 2)
    → Retry manager creates retry (Phase 3)
    → Retry dispatched with backoff (Phase 3)
    → Success or exhaust retries
```

**Backup Safety Net:**
If Phase 3 retry fails to create retry, Phase 1 timeout monitor will still catch stuck executions.

## Best Practices

### Action Design for Retries

1. **Make actions idempotent:** Safe to run multiple times
2. **Set realistic timeouts:** Based on typical execution time
3. **Configure appropriate max_retries:**
   - Network calls: 3-5 retries
   - Database operations: 2-3 retries
   - External APIs: 3 retries
   - Local operations: 0-1 retries

### Worker Health Management

1. **Report queue depth regularly:** Update every heartbeat
2. **Track failure metrics:** Consecutive failures, total/failed counts
3. **Implement graceful degradation:** Continue working when degraded
4. **Fail fast when unhealthy:** Stop accepting work if overloaded

### Monitoring Strategy

1. **Alert on high retry rates:** > 20% of executions retrying
2. **Alert on unhealthy workers:** > 50% workers unhealthy
3. **Track retry success rate:** Should be > 70%
4. **Monitor queue depths:** Average should stay < 20

## See Also

- **Architecture:** `docs/architecture/worker-availability-handling.md`
- **Phase 1 Guide:** `docs/QUICKREF-worker-availability-phase1.md`
- **Phase 2 Guide:** `docs/QUICKREF-worker-queue-ttl-dlq.md`
- **Migration:** `migrations/20260209000000_phase3_retry_and_health.sql`
