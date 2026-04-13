# QUICKREF: Sensor Worker Registration

**Quick reference for sensor worker runtime capability reporting**

---

## What It Does

Sensor services now register themselves in the database and report which runtimes (Python, Node.js, Shell, Native) they have available. This enables:

- ✅ Runtime capability tracking per sensor worker
- ✅ Health monitoring via heartbeats
- ✅ Foundation for distributed sensor scheduling (future)

---

## TL;DR

```yaml
# config.yaml (optional - auto-detects if omitted)
sensor:
  worker_name: "sensor-prod-01"
  capabilities:
    runtimes: ["python", "shell", "node"]
  heartbeat_interval: 30
```

```bash
# Override runtime detection
export ATTUNE_SENSOR_RUNTIMES="python,shell"

# Start service (auto-registers on startup)
cargo run --bin attune-sensor
```

---

## Configuration

### Auto-Detection (Recommended)

No configuration needed. Sensor service will:
- Auto-detect Python (checks for `python3`/`python`)
- Auto-detect Node.js (checks for `node`)
- Always include Shell and Native

### Explicit Configuration

```yaml
sensor:
  worker_name: "sensor-{hostname}"     # Default: sensor-{hostname}
  host: "10.0.1.42"                    # Default: hostname
  max_concurrent_sensors: 10            # Default: 10
  heartbeat_interval: 30                # Default: 30 seconds
  capabilities:
    runtimes: ["python", "shell"]       # Override auto-detection
```

### Environment Override

```bash
# Highest priority - overrides config and auto-detection
export ATTUNE_SENSOR_RUNTIMES="shell,native"  # Comma-separated
```

---

## Runtime Detection Priority

1. **ATTUNE_SENSOR_RUNTIMES** env var (highest)
2. **config.sensor.capabilities.runtimes** (medium)
3. **Auto-detection** (lowest)

---

## Database Schema

```sql
-- Sensor workers use the unified worker table
SELECT * FROM worker WHERE worker_role = 'sensor';

-- Columns:
--   id, name, worker_type, worker_role, host, status
--   capabilities JSONB: {"runtimes": ["python", "shell"], ...}
--   last_heartbeat: Updated every 30s (configurable)
```

---

## Querying Sensor Workers

```sql
-- Active sensor workers
SELECT name, host, capabilities->'runtimes' AS runtimes
FROM worker
WHERE worker_role = 'sensor' AND status = 'active';

-- Workers with Python runtime
SELECT name FROM worker
WHERE worker_role = 'sensor'
  AND capabilities->'runtimes' ? 'python';

-- Stale workers (no heartbeat in 5 min)
SELECT name, last_heartbeat
FROM worker
WHERE worker_role = 'sensor'
  AND last_heartbeat < NOW() - INTERVAL '5 minutes';
```

---

## Lifecycle

```
Service Start → Register (INSERT/UPDATE worker)
              ↓
           Heartbeat Loop (every 30s)
              ↓
Service Stop → Deregister (SET status = 'inactive')
```

---

## Monitoring

### Logs

```bash
# Registration
grep "Registering sensor worker" logs
grep "Sensor worker registered with ID" logs

# Heartbeat
grep "Sensor worker heartbeat" logs

# Deregistration
grep "Deregistering sensor worker" logs
```

### Metrics

- Active workers: `SELECT COUNT(*) FROM worker WHERE worker_role = 'sensor' AND status = 'active'`
- Heartbeat lag: `NOW() - last_heartbeat`
- Runtime availability: Count workers per runtime

---

## Troubleshooting

### Runtime Not Detected

```bash
# Check if binary is available
which python3
which node

# Override detection
export ATTUNE_SENSOR_RUNTIMES="python,shell,node"
```

### Worker Not Registering

```bash
# Check migration applied
psql -c "\d worker" attune
# Should see 'worker_role' column

# Apply migration
sqlx migrate run
```

### Heartbeat Not Updating

```bash
# Check sensor service is running
ps aux | grep attune-sensor

# Check logs for errors
journalctl -u attune-sensor -f | grep heartbeat
```

---

## Migration Required

```bash
# Apply migration (adds worker_role column)
sqlx migrate run

# Update SQLx metadata
cargo sqlx prepare --workspace
```

Migration: `20260131000001_add_worker_role.sql`

---

## Files

- Implementation: `crates/sensor/src/sensor_worker_registration.rs`
- Service integration: `crates/sensor/src/service.rs`
- Config: `crates/common/src/config.rs` (SensorConfig struct)
- Migration: `migrations/20260131000001_add_worker_role.sql`
- Docs: `docs/sensors/sensor-worker-registration.md`

---

## Example: Custom Capabilities

```rust
use attune_sensor::SensorWorkerRegistration;

let mut registration = SensorWorkerRegistration::new(pool, &config);
registration.register().await?;

// Add custom capability
registration.add_capability("gpu_enabled".to_string(), json!(true));
registration.update_capabilities().await?;
```

---

## Future: Distributed Scheduling

Once implemented, sensors will be scheduled on workers with required runtime:

```sql
-- Find workers for Python sensor
SELECT * FROM worker
WHERE worker_role = 'sensor'
  AND status = 'active'
  AND capabilities->'runtimes' ? 'python'
ORDER BY last_heartbeat DESC;
```

---

## See Also

- Full docs: `docs/sensors/sensor-worker-registration.md`
- Worker registration: `crates/worker/src/registration.rs` (similar pattern)
- Sensor runtime: `docs/sensors/sensor-runtime.md`
