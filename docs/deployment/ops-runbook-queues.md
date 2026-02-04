# Operational Runbook: Queue Management

**Service**: Attune Executor  
**Component**: Execution Queue Manager  
**Audience**: Operations, SRE, DevOps  
**Last Updated**: 2025-01-27

---

## Table of Contents

1. [Overview](#overview)
2. [Quick Reference](#quick-reference)
3. [Monitoring](#monitoring)
4. [Common Issues](#common-issues)
5. [Troubleshooting Procedures](#troubleshooting-procedures)
6. [Maintenance Tasks](#maintenance-tasks)
7. [Emergency Procedures](#emergency-procedures)
8. [Capacity Planning](#capacity-planning)

---

## Overview

The Attune Executor service manages per-action FIFO execution queues to ensure fair, ordered processing when policy limits (concurrency, rate limits) are enforced. This runbook covers operational procedures for monitoring and managing these queues.

### Key Concepts

- **Queue**: Per-action FIFO buffer of waiting executions
- **Active Count**: Number of currently running executions for an action
- **Max Concurrent**: Policy-enforced limit on parallel executions
- **Queue Length**: Number of executions waiting in queue
- **FIFO**: First-In-First-Out ordering guarantee

### System Components

- **ExecutionQueueManager**: Core queue management (in-memory)
- **CompletionListener**: Processes worker completion messages
- **QueueStatsRepository**: Persists statistics to database
- **API Endpoint**: `/api/v1/actions/:ref/queue-stats`

---

## Quick Reference

### Health Check Commands

```bash
# Check executor service status
systemctl status attune-executor

# Check active queues
curl -s http://localhost:8080/api/v1/actions/core.http.get/queue-stats | jq

# Database query for all active queues
psql -U attune -d attune -c "
  SELECT a.ref, qs.queue_length, qs.active_count, qs.max_concurrent,
         qs.oldest_enqueued_at, qs.last_updated
  FROM attune.queue_stats qs
  JOIN attune.action a ON a.id = qs.action_id
  WHERE queue_length > 0 OR active_count > 0
  ORDER BY queue_length DESC;
"

# Check executor logs for queue issues
journalctl -u attune-executor -n 100 --no-pager | grep -i queue
```

### Emergency Actions

```bash
# Restart executor (clears in-memory queues)
sudo systemctl restart attune-executor

# Restart all workers (forces completion messages)
sudo systemctl restart attune-worker@*

# Clear stale queue stats (older than 1 hour, inactive)
psql -U attune -d attune -c "
  DELETE FROM attune.queue_stats
  WHERE last_updated < NOW() - INTERVAL '1 hour'
    AND queue_length = 0
    AND active_count = 0;
"
```

---

## Monitoring

### Key Metrics to Track

| Metric | Threshold | Action |
|--------|-----------|--------|
| Queue Length | > 100 | Investigate load |
| Queue Length | > 500 | Add workers |
| Queue Length | > 1000 | Emergency response |
| Oldest Enqueued | > 10 min | Check workers |
| Oldest Enqueued | > 30 min | Critical issue |
| Active < Max Concurrent | Any | Workers stuck |
| Last Updated | > 10 min | Executor issue |

### Monitoring Queries

#### Active Queues Overview

```sql
SELECT 
  a.ref AS action,
  qs.queue_length,
  qs.active_count,
  qs.max_concurrent,
  ROUND(EXTRACT(EPOCH FROM (NOW() - qs.oldest_enqueued_at)) / 60, 1) AS wait_minutes,
  ROUND(qs.total_completed::float / NULLIF(qs.total_enqueued, 0) * 100, 2) AS completion_pct,
  qs.last_updated
FROM attune.queue_stats qs
JOIN attune.action a ON a.id = qs.action_id
WHERE queue_length > 0 OR active_count > 0
ORDER BY queue_length DESC;
```

#### Top Actions by Throughput

```sql
SELECT 
  a.ref AS action,
  qs.total_enqueued,
  qs.total_completed,
  qs.total_enqueued - qs.total_completed AS pending,
  ROUND(qs.total_completed::float / NULLIF(qs.total_enqueued, 0) * 100, 2) AS completion_pct
FROM attune.queue_stats qs
JOIN attune.action a ON a.id = qs.action_id
WHERE qs.total_enqueued > 0
ORDER BY qs.total_enqueued DESC
LIMIT 20;
```

#### Stuck Queues (Not Progressing)

```sql
SELECT 
  a.ref AS action,
  qs.queue_length,
  qs.active_count,
  ROUND(EXTRACT(EPOCH FROM (NOW() - qs.last_updated)) / 60, 1) AS stale_minutes,
  qs.oldest_enqueued_at
FROM attune.queue_stats qs
JOIN attune.action a ON a.id = qs.action_id
WHERE (queue_length > 0 OR active_count > 0)
  AND last_updated < NOW() - INTERVAL '10 minutes'
ORDER BY stale_minutes DESC;
```

#### Queue Growth Rate

```sql
-- Create a monitoring table for snapshots
CREATE TABLE IF NOT EXISTS attune.queue_snapshots (
  snapshot_time TIMESTAMPTZ DEFAULT NOW(),
  action_id BIGINT,
  queue_length INT,
  active_count INT,
  total_enqueued BIGINT
);

-- Take snapshot (run every 5 minutes)
INSERT INTO attune.queue_snapshots (action_id, queue_length, active_count, total_enqueued)
SELECT action_id, queue_length, active_count, total_enqueued
FROM attune.queue_stats
WHERE queue_length > 0 OR active_count > 0;

-- Analyze growth rate
SELECT 
  a.ref AS action,
  s1.queue_length AS queue_now,
  s2.queue_length AS queue_5min_ago,
  s1.queue_length - s2.queue_length AS growth,
  s1.total_enqueued - s2.total_enqueued AS new_requests
FROM attune.queue_snapshots s1
JOIN attune.queue_snapshots s2 ON s2.action_id = s1.action_id
JOIN attune.action a ON a.id = s1.action_id
WHERE s1.snapshot_time >= NOW() - INTERVAL '1 minute'
  AND s2.snapshot_time >= NOW() - INTERVAL '6 minutes'
  AND s2.snapshot_time < NOW() - INTERVAL '4 minutes'
ORDER BY growth DESC;
```

### Alerting Rules

**Prometheus/Grafana Alerts** (if metrics exported):

```yaml
groups:
  - name: attune_queues
    interval: 30s
    rules:
      - alert: HighQueueDepth
        expr: attune_queue_length > 100
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Queue depth high for {{ $labels.action }}"
          description: "Queue has {{ $value }} waiting executions"

      - alert: CriticalQueueDepth
        expr: attune_queue_length > 500
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Critical queue depth for {{ $labels.action }}"
          description: "Queue has {{ $value }} waiting executions - add workers"

      - alert: StuckQueue
        expr: attune_queue_last_updated < time() - 600
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Queue not progressing for {{ $labels.action }}"
          description: "Queue hasn't updated in 10+ minutes"

      - alert: OldestExecutionAging
        expr: attune_queue_oldest_age_seconds > 1800
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Execution waiting 30+ minutes for {{ $labels.action }}"
```

**Nagios/Icinga Check**:

```bash
#!/bin/bash
# /usr/lib/nagios/plugins/check_attune_queues.sh

WARN_THRESHOLD=${1:-100}
CRIT_THRESHOLD=${2:-500}

MAX_QUEUE=$(psql -U attune -d attune -t -c "
  SELECT COALESCE(MAX(queue_length), 0) FROM attune.queue_stats;
")

if [ "$MAX_QUEUE" -ge "$CRIT_THRESHOLD" ]; then
  echo "CRITICAL: Max queue depth $MAX_QUEUE >= $CRIT_THRESHOLD"
  exit 2
elif [ "$MAX_QUEUE" -ge "$WARN_THRESHOLD" ]; then
  echo "WARNING: Max queue depth $MAX_QUEUE >= $WARN_THRESHOLD"
  exit 1
else
  echo "OK: Max queue depth $MAX_QUEUE"
  exit 0
fi
```

---

## Common Issues

### Issue 1: Queue Growing Continuously

**Symptoms:**
- Queue length increases over time
- Never decreases even when workers are idle
- `oldest_enqueued_at` gets older

**Common Causes:**
1. Workers not processing fast enough
2. Too many incoming requests
3. Concurrency limit too low
4. Worker crashes/restarts

**Quick Diagnosis:**
```bash
# Check worker status
systemctl status attune-worker@*

# Check worker resource usage
ps aux | grep attune-worker
top -p $(pgrep -d',' attune-worker)

# Check recent completions
psql -U attune -d attune -c "
  SELECT COUNT(*), status
  FROM attune.execution
  WHERE updated > NOW() - INTERVAL '5 minutes'
  GROUP BY status;
"
```

**Resolution:** See [Troubleshooting: Growing Queue](#growing-queue)

---

### Issue 2: Queue Not Progressing

**Symptoms:**
- Queue length stays constant
- `last_updated` timestamp doesn't change
- Active executions showing but not completing

**Common Causes:**
1. Workers crashed/hung
2. CompletionListener not running
3. Message queue connection lost
4. Database connection issue

**Quick Diagnosis:**
```bash
# Check executor process
ps aux | grep attune-executor
journalctl -u attune-executor -n 50 --no-pager

# Check message queue
rabbitmqctl list_queues name messages | grep execution.completed

# Check for stuck executions
psql -U attune -d attune -c "
  SELECT id, action, status, created, updated
  FROM attune.execution
  WHERE status = 'running'
    AND updated < NOW() - INTERVAL '10 minutes'
  ORDER BY created DESC
  LIMIT 10;
"
```

**Resolution:** See [Troubleshooting: Stuck Queue](#stuck-queue)

---

### Issue 3: Queue Full Errors

**Symptoms:**
- API returns `Queue full (max length: 10000)` errors
- New executions rejected
- Users report action failures

**Common Causes:**
1. Sudden traffic spike
2. Worker capacity exhausted
3. `max_queue_length` too low
4. Slow action execution

**Quick Diagnosis:**
```bash
# Check current queue stats
curl -s http://localhost:8080/api/v1/actions/AFFECTED_ACTION/queue-stats | jq

# Check configuration
grep -A5 "queue:" /etc/attune/config.yaml

# Check worker count
systemctl list-units attune-worker@* | grep running
```

**Resolution:** See [Troubleshooting: Queue Full](#queue-full)

---

### Issue 4: FIFO Order Violation

**Symptoms:**
- Executions complete out of order
- Later requests finish before earlier ones
- Workflow dependencies break

**Severity:** CRITICAL - This indicates a bug

**Immediate Action:**
1. Capture executor logs immediately
2. Document the violation with timestamps
3. Restart executor service
4. File critical bug report

**Data to Collect:**
```bash
# Capture logs
journalctl -u attune-executor --since "10 minutes ago" > /tmp/executor-fifo-violation.log

# Capture database state
psql -U attune -d attune -c "
  SELECT id, action, status, created, updated
  FROM attune.execution
  WHERE action = <affected_action_id>
    AND created > NOW() - INTERVAL '1 hour'
  ORDER BY created;
" > /tmp/execution-order.txt

# Capture queue stats
curl -s http://localhost:8080/api/v1/actions/AFFECTED_ACTION/queue-stats | jq > /tmp/queue-stats.json
```

---

## Troubleshooting Procedures

### Growing Queue

**Procedure:**

1. **Assess Severity**
   ```bash
   # Get current queue depth
   curl -s http://localhost:8080/api/v1/actions/AFFECTED_ACTION/queue-stats | jq '.data.queue_length'
   ```

2. **Check Worker Health**
   ```bash
   # Active workers
   systemctl list-units attune-worker@* | grep running | wc -l
   
   # Worker resource usage
   ps aux | grep attune-worker | awk '{print $3, $4, $11}'
   
   # Recent worker errors
   journalctl -u attune-worker@* -n 100 --no-pager | grep -i error
   ```

3. **Check Completion Rate**
   ```sql
   SELECT 
     COUNT(*) FILTER (WHERE created > NOW() - INTERVAL '5 minutes') AS recent_created,
     COUNT(*) FILTER (WHERE updated > NOW() - INTERVAL '5 minutes' AND status IN ('succeeded', 'failed')) AS recent_completed
   FROM attune.execution
   WHERE action = <action_id>;
   ```

4. **Solutions (in order of preference)**:

   a. **Scale Workers** (if completion rate too low):
   ```bash
   # Add more worker instances
   sudo systemctl start attune-worker@2
   sudo systemctl start attune-worker@3
   ```

   b. **Increase Concurrency** (if safe):
   ```yaml
   # In config.yaml or via API
   policies:
     actions:
       affected.action:
         concurrency_limit: 10  # Increase from 5
   ```

   c. **Rate Limit at API** (if traffic spike):
   ```yaml
   # In API config
   rate_limits:
     global:
       max_requests_per_minute: 1000
   ```

   d. **Temporary Queue Increase** (emergency only):
   ```yaml
   executor:
     queue:
       max_queue_length: 20000  # Increase from 10000
   ```
   Then restart executor: `sudo systemctl restart attune-executor`

5. **Monitor Results**
   ```bash
   watch -n 5 "curl -s http://localhost:8080/api/v1/actions/AFFECTED_ACTION/queue-stats | jq '.data.queue_length'"
   ```

---

### Stuck Queue

**Procedure:**

1. **Identify Stuck Executions**
   ```sql
   SELECT id, status, created, updated, 
          EXTRACT(EPOCH FROM (NOW() - updated)) / 60 AS stuck_minutes
   FROM attune.execution
   WHERE action = <action_id>
     AND status IN ('running', 'requested')
     AND updated < NOW() - INTERVAL '10 minutes'
   ORDER BY created;
   ```

2. **Check Worker Status**
   ```bash
   # Are workers running?
   systemctl status attune-worker@*
   
   # Are workers processing?
   tail -f /var/log/attune/worker.log | grep execution_id
   ```

3. **Check Message Queue**
   ```bash
   # Completion messages backing up?
   rabbitmqctl list_queues name messages | grep execution.completed
   
   # Connection issues?
   rabbitmqctl list_connections
   ```

4. **Check CompletionListener**
   ```bash
   # Is listener running?
   journalctl -u attune-executor -n 100 --no-pager | grep CompletionListener
   
   # Recent completions processed?
   journalctl -u attune-executor -n 100 --no-pager | grep "notify_completion"
   ```

5. **Solutions**:

   a. **Restart Stuck Workers**:
   ```bash
   # Graceful restart
   sudo systemctl restart attune-worker@1
   ```

   b. **Restart Executor** (if CompletionListener stuck):
   ```bash
   sudo systemctl restart attune-executor
   ```

   c. **Force Complete Stuck Executions** (emergency):
   ```sql
   -- CAUTION: Only for truly stuck executions
   UPDATE attune.execution
   SET status = 'failed', 
       result = '{"error": "Execution stuck, manually failed by operator"}',
       updated = NOW()
   WHERE id IN (<stuck_execution_ids>);
   ```

   d. **Purge and Restart** (nuclear option):
   ```bash
   # Stop services
   sudo systemctl stop attune-executor
   sudo systemctl stop attune-worker@*
   
   # Clear message queues
   rabbitmqctl purge_queue execution.requested
   rabbitmqctl purge_queue execution.completed
   
   # Restart services
   sudo systemctl start attune-executor
   sudo systemctl start attune-worker@1
   ```

---

### Queue Full

**Procedure:**

1. **Immediate Mitigation** (choose one):

   a. **Temporarily Increase Limit**:
   ```yaml
   # config.yaml
   executor:
     queue:
       max_queue_length: 20000
   ```
   ```bash
   sudo systemctl restart attune-executor
   ```

   b. **Add Workers**:
   ```bash
   sudo systemctl start attune-worker@{2..5}
   ```

   c. **Increase Concurrency**:
   ```yaml
   policies:
     actions:
       affected.action:
         concurrency_limit: 20  # Increase
   ```

2. **Analyze Root Cause**
   ```bash
   # Traffic pattern
   psql -U attune -d attune -c "
     SELECT DATE_TRUNC('minute', created) AS minute, COUNT(*)
     FROM attune.execution
     WHERE action = <action_id>
       AND created > NOW() - INTERVAL '1 hour'
     GROUP BY minute
     ORDER BY minute DESC;
   "
   
   # Action performance
   psql -U attune -d attune -c "
     SELECT AVG(EXTRACT(EPOCH FROM (updated - created))) AS avg_duration_seconds
     FROM attune.execution
     WHERE action = <action_id>
       AND status = 'succeeded'
       AND created > NOW() - INTERVAL '1 hour';
   "
   ```

3. **Long-term Solution**:

   - **Traffic spike**: Add API rate limiting
   - **Slow action**: Optimize action code
   - **Under-capacity**: Permanently scale workers
   - **Configuration**: Adjust concurrency limits

---

## Maintenance Tasks

### Daily

```bash
#!/bin/bash
# daily-queue-check.sh

echo "=== Active Queues ==="
psql -U attune -d attune -c "
  SELECT a.ref, qs.queue_length, qs.active_count
  FROM attune.queue_stats qs
  JOIN attune.action a ON a.id = qs.action_id
  WHERE queue_length > 0 OR active_count > 0;
"

echo "=== Stuck Queues ==="
psql -U attune -d attune -c "
  SELECT a.ref, qs.queue_length, 
         ROUND(EXTRACT(EPOCH FROM (NOW() - qs.last_updated)) / 60, 1) AS stale_minutes
  FROM attune.queue_stats qs
  JOIN attune.action a ON a.id = qs.action_id
  WHERE (queue_length > 0 OR active_count > 0)
    AND last_updated < NOW() - INTERVAL '30 minutes';
"

echo "=== Top Actions by Volume ==="
psql -U attune -d attune -c "
  SELECT a.ref, qs.total_enqueued, qs.total_completed
  FROM attune.queue_stats qs
  JOIN attune.action a ON a.id = qs.action_id
  ORDER BY qs.total_enqueued DESC
  LIMIT 10;
"
```

### Weekly

```bash
#!/bin/bash
# weekly-queue-maintenance.sh

echo "=== Cleaning Stale Queue Stats ==="
psql -U attune -d attune -c "
  DELETE FROM attune.queue_stats
  WHERE last_updated < NOW() - INTERVAL '7 days'
    AND queue_length = 0
    AND active_count = 0;
"

echo "=== Queue Snapshots Cleanup ==="
psql -U attune -d attune -c "
  DELETE FROM attune.queue_snapshots
  WHERE snapshot_time < NOW() - INTERVAL '30 days';
"

echo "=== Executor Log Rotation ==="
journalctl --vacuum-time=30d -u attune-executor
```

### Monthly

- Review queue capacity trends
- Analyze high-volume actions
- Plan scaling based on growth
- Update alert thresholds
- Review and test runbook procedures

---

## Emergency Procedures

### Emergency: System-Wide Queue Overload

**Symptoms:**
- Multiple actions with critical queue depths
- System-wide performance degradation
- API response times degraded

**Procedure:**

1. **Enable Emergency Mode**:
   ```yaml
   # config.yaml
   executor:
     emergency_mode: true  # Relaxes limits
     queue:
       max_queue_length: 50000
   ```

2. **Scale Workers Aggressively**:
   ```bash
   for i in {1..10}; do
     sudo systemctl start attune-worker@$i
   done
   ```

3. **Temporarily Disable Non-Critical Actions**:
   ```sql
   -- Disable low-priority actions
   UPDATE attune.action
   SET enabled = false
   WHERE priority < 5 OR tags @> '["low-priority"]';
   ```

4. **Enable API Rate Limiting**:
   ```yaml
   api:
     rate_limits:
       global:
         enabled: true
         max_requests_per_minute: 500
   ```

5. **Monitor Recovery**:
   ```bash
   watch -n 10 "psql -U attune -d attune -t -c 'SELECT SUM(queue_length) FROM attune.queue_stats;'"
   ```

6. **Post-Incident**:
   - Document what happened
   - Analyze root cause
   - Update capacity plan
   - Restore normal configuration

---

### Emergency: Executor Crash Loop

**Symptoms:**
- Executor service repeatedly crashes
- Queues not progressing
- High memory usage before crash

**Procedure:**

1. **Capture Crash Logs**:
   ```bash
   journalctl -u attune-executor --since "30 minutes ago" > /tmp/executor-crash.log
   dmesg | tail -100 > /tmp/dmesg-crash.log
   ```

2. **Check for Memory Issues**:
   ```bash
   # Check OOM kills
   grep -i "out of memory" /var/log/syslog
   grep -i "killed process" /var/log/kern.log
   ```

3. **Emergency Restart with Limited Queues**:
   ```yaml
   # config.yaml
   executor:
     queue:
       max_queue_length: 1000  # Reduce drastically
       enable_metrics: false    # Reduce overhead
   ```

4. **Start in Safe Mode**:
   ```bash
   sudo systemctl start attune-executor
   # Monitor memory
   watch -n 1 "ps aux | grep attune-executor | grep -v grep"
   ```

5. **If Still Crashing**:
   ```bash
   # Disable queue persistence temporarily
   # In code or via feature flag
   export ATTUNE__EXECUTOR__QUEUE__ENABLE_METRICS=false
   sudo systemctl restart attune-executor
   ```

6. **Escalate**:
   - Contact development team
   - Provide crash logs and memory dumps
   - Consider rolling back to previous version

---

## Capacity Planning

### Calculating Required Capacity

**Formula**:
```
Required Workers = (Peak Requests/Hour × Avg Duration) / 3600 / Concurrency Limit
```

**Example**:
- Peak: 10,000 requests/hour
- Avg Duration: 5 seconds
- Concurrency: 10 per worker

```
Workers = (10,000 × 5) / 3600 / 10 = 1.4 → 2 workers minimum
Add 50% buffer → 3 workers recommended
```

### Growth Planning

Monitor monthly trends:

```sql
SELECT 
  DATE_TRUNC('day', created) AS day,
  COUNT(*) AS executions,
  AVG(EXTRACT(EPOCH FROM (updated - created))) AS avg_duration
FROM attune.execution
WHERE created > NOW() - INTERVAL '30 days'
GROUP BY day
ORDER BY day;
```

### Capacity Recommendations

| Queue Depth | Worker Count | Action |
|-------------|--------------|--------|
| < 10 | Current | Maintain |
| 10-50 | +25% | Plan scale-up |
| 50-100 | +50% | Scale soon |
| 100+ | +100% | Scale now |

---

## Related Documentation

- [Queue Architecture](./queue-architecture.md)
- [Executor Service](./executor-service.md)
- [Worker Service](./worker-service.md)
- [API: Actions - Queue Stats](./api-actions.md#get-queue-statistics)

---

**Version**: 1.0  
**Maintained By**: SRE Team  
**Last Updated**: 2025-01-27