# Sensor Worker Registration

**Version:** 1.0  
**Last Updated:** 2026-01-31

---

## Overview

The Sensor Worker Registration system enables sensor service instances to register themselves in the database, report their runtime capabilities (Python, Node.js, Shell, etc.), and maintain heartbeat status. This mirrors the action worker registration system but is tailored for sensor services.

This feature allows for:
- **Runtime capability reporting**: Each sensor worker reports which runtimes it has available
- **Distributed sensor execution**: Future support for scheduling sensors on workers with required runtimes
- **Service monitoring**: Track active sensor workers and their health status
- **Resource management**: Understand sensor worker capacity and availability

---

## Architecture

### Database Schema

Sensor workers use the unified `worker` table with a `worker_role` discriminator:

```sql
CREATE TABLE worker (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    worker_type worker_type_enum NOT NULL,     -- 'local', 'remote', 'container'
    worker_role worker_role_enum NOT NULL,     -- 'action', 'sensor', 'hybrid'
    runtime BIGINT REFERENCES runtime(id),
    host TEXT,
    port INTEGER,
    status worker_status_enum DEFAULT 'inactive',
    capabilities JSONB,                        -- {"runtimes": ["python", "shell", "node"]}
    meta JSONB,
    last_heartbeat TIMESTAMPTZ,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Worker Role Enum:**
```sql
CREATE TYPE worker_role_enum AS ENUM ('action', 'sensor', 'hybrid');
```

- `action`: Executes actions only
- `sensor`: Monitors triggers and executes sensors only
- `hybrid`: Can execute both actions and sensors (future use)

### Capabilities Structure

The `capabilities` JSONB field contains:

```json
{
  "runtimes": ["python", "shell", "node", "native"],
  "max_concurrent_sensors": 10,
  "sensor_version": "0.1.0"
}
```

---

## Configuration

### YAML Configuration

Add sensor configuration to your `config.yaml`:

```yaml
sensor:
  # Sensor worker name (defaults to "sensor-{hostname}")
  worker_name: "sensor-production-01"
  
  # Sensor worker host (defaults to hostname)
  host: "10.0.1.42"
  
  # Heartbeat interval in seconds
  heartbeat_interval: 30
  
  # Sensor poll interval
  poll_interval: 30
  
  # Sensor execution timeout
  sensor_timeout: 30
  
  # Maximum concurrent sensors
  max_concurrent_sensors: 10
  
  # Capabilities (optional - will auto-detect if not specified)
  capabilities:
    runtimes: ["python", "shell", "node"]
    custom_feature: true
```

### Environment Variables

Override runtime detection with:

```bash
# Specify available runtimes (comma-separated)
export ATTUNE_SENSOR_RUNTIMES="python,shell"

# Or via config override
export ATTUNE__SENSOR__WORKER_NAME="sensor-custom"
export ATTUNE__SENSOR__HEARTBEAT_INTERVAL="60"
```

---

## Runtime Detection

Sensor workers auto-detect available runtimes using a priority system:

### Priority Order

1. **Environment Variable** (highest priority)
   ```bash
   ATTUNE_SENSOR_RUNTIMES="python,shell,node"
   ```

2. **Config File**
   ```yaml
   sensor:
     capabilities:
       runtimes: ["python", "shell"]
   ```

3. **Auto-Detection** (lowest priority)
   - Checks for `python3` or `python` binary
   - Checks for `node` binary
   - Always includes `shell` (bash/sh)
   - Always includes `native` (compiled Rust sensors)

### Auto-Detection Logic

```rust
// Check for Python
if Command::new("python3").arg("--version").output().is_ok() {
    runtimes.push("python".to_string());
}

// Check for Node.js
if Command::new("node").arg("--version").output().is_ok() {
    runtimes.push("node".to_string());
}

// Always available
runtimes.push("shell".to_string());
runtimes.push("native".to_string());
```

---

## Registration Lifecycle

### 1. Service Startup

When the sensor service starts:

```rust
// Create registration manager
let registration = SensorWorkerRegistration::new(db.clone(), &config);

// Register in database
let worker_id = registration.register().await?;
// Sets status to 'active', records capabilities, sets last_heartbeat
```

**Database Operations:**
- If worker with same name exists: Update to active status
- If new worker: Insert new record with `worker_role = 'sensor'`

### 2. Heartbeat Loop

While running, sends periodic heartbeats:

```rust
// Every 30 seconds (configurable)
registration.heartbeat().await?;
// Updates last_heartbeat, ensures status is 'active'
```

### 3. Service Shutdown

On graceful shutdown:

```rust
// Mark as inactive
registration.deregister().await?;
// Sets status to 'inactive'
```

---

## Usage Example

### Sensor Service Integration

The `SensorService` automatically handles registration:

```rust
use attune_sensor::SensorService;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let service = SensorService::new(config).await?;
    
    // Automatically registers sensor worker on start
    service.start().await?;
    
    // Automatically deregisters on stop
    Ok(())
}
```

### Manual Registration (Advanced)

For custom integrations:

```rust
use attune_sensor::SensorWorkerRegistration;

let mut registration = SensorWorkerRegistration::new(pool, &config);

// Register
let worker_id = registration.register().await?;
println!("Registered as worker ID: {}", worker_id);

// Add custom capability
registration.add_capability("gpu_enabled".to_string(), json!(true));
registration.update_capabilities().await?;

// Send heartbeats
loop {
    tokio::time::sleep(Duration::from_secs(30)).await;
    registration.heartbeat().await?;
}

// Deregister on shutdown
registration.deregister().await?;
```

---

## Querying Sensor Workers

### Find Active Sensor Workers

```sql
SELECT id, name, host, capabilities, last_heartbeat
FROM worker
WHERE worker_role = 'sensor' AND status = 'active';
```

### Find Sensor Workers with Python Runtime

```sql
SELECT id, name, host, capabilities->'runtimes' as runtimes
FROM worker
WHERE worker_role = 'sensor' 
  AND status = 'active'
  AND capabilities->'runtimes' ? 'python';
```

### Find Stale Sensor Workers (No Heartbeat in 5 Minutes)

```sql
SELECT id, name, last_heartbeat
FROM worker
WHERE worker_role = 'sensor'
  AND status = 'active'
  AND last_heartbeat < NOW() - INTERVAL '5 minutes';
```

---

## Monitoring

### Health Checks

Monitor sensor worker health by checking `last_heartbeat`:

```sql
-- Workers that haven't sent heartbeat in 2x heartbeat interval
SELECT 
    name,
    host,
    status,
    last_heartbeat,
    NOW() - last_heartbeat AS time_since_heartbeat
FROM worker
WHERE worker_role = 'sensor'
  AND status = 'active'
  AND last_heartbeat < NOW() - INTERVAL '60 seconds'
ORDER BY last_heartbeat;
```

### Metrics to Track

- **Active sensor workers**: Count of workers with `status = 'active'`
- **Runtime distribution**: Which runtimes are available across workers
- **Heartbeat lag**: Time since last heartbeat for each worker
- **Worker capacity**: Sum of `max_concurrent_sensors` across all active workers

---

## Future Enhancements

### Distributed Sensor Scheduling

Once sensor worker registration is in place, we can implement:

1. **Runtime-based scheduling**: Schedule sensors only on workers with required runtime
2. **Load balancing**: Distribute sensors across multiple workers
3. **Failover**: Automatically reassign sensors if a worker goes down
4. **Geographic distribution**: Run sensors close to monitored resources

### Example: Sensor Scheduling Logic

```rust
// Find sensor workers with required runtime
let workers = sqlx::query_as!(
    Worker,
    r#"
    SELECT * FROM worker
    WHERE worker_role IN ('sensor', 'hybrid')
      AND status = 'active'
      AND capabilities->'runtimes' ? $1
    ORDER BY last_heartbeat DESC
    "#,
    required_runtime
)
.fetch_all(&pool)
.await?;

// Schedule sensor on least-loaded worker
let target_worker = select_least_loaded_worker(workers)?;
schedule_sensor_on_worker(sensor, target_worker).await?;
```

---

## Troubleshooting

### Worker Not Registering

**Symptom:** Sensor service starts but no worker record in database

**Checks:**
1. Verify database connection: `DATABASE_URL` is correct
2. Check logs for registration errors: `grep "Registering sensor worker" logs`
3. Verify migrations applied: Check for `worker_role` column

**Solution:**
```bash
# Check migration status
sqlx migrate info

# Apply migrations
sqlx migrate run
```

### Runtime Not Detected

**Symptom:** Expected runtime not in `capabilities.runtimes`

**Checks:**
1. Verify binary is in PATH: `which python3`, `which node`
2. Check environment variable: `echo $ATTUNE_SENSOR_RUNTIMES`
3. Review sensor service logs for auto-detection output

**Solution:**
```bash
# Explicitly set runtimes
export ATTUNE_SENSOR_RUNTIMES="python,shell,node"

# Or in config.yaml
sensor:
  capabilities:
    runtimes: ["python", "shell", "node"]
```

### Heartbeat Not Updating

**Symptom:** `last_heartbeat` timestamp is stale

**Checks:**
1. Verify sensor service is running
2. Check for database connection issues in logs
3. Verify heartbeat interval configuration

**Solution:**
```bash
# Check sensor service status
systemctl status attune-sensor

# Review logs
journalctl -u attune-sensor -f | grep heartbeat
```

---

## Migration from Legacy System

If you have existing sensor services without registration:

1. **Apply migration**: `20260131000001_add_worker_role.sql`
2. **Restart sensor services**: They will auto-register on startup
3. **Verify registration**: Query `worker` table for `worker_role = 'sensor'`

Existing action workers are automatically marked as `worker_role = 'action'` by the migration.

---

## Security Considerations

### Worker Naming

- Use hostname-based naming for automatic uniqueness
- Avoid hardcoding credentials in worker names
- Consider using UUIDs for ephemeral/containerized workers

### Capabilities

- Capabilities are self-reported (trust boundary)
- In distributed setups, validate runtime availability before execution
- Consider runtime verification/attestation for high-security environments

### Heartbeat Monitoring

- Stale workers (no heartbeat) should be marked inactive automatically
- Implement worker health checks before scheduling sensors
- Set appropriate heartbeat intervals (too frequent = DB load, too infrequent = slow failover)

---

## API Reference

### SensorWorkerRegistration

```rust
impl SensorWorkerRegistration {
    /// Create new registration manager
    pub fn new(pool: PgPool, config: &Config) -> Self;
    
    /// Register sensor worker in database
    pub async fn register(&mut self) -> Result<i64>;
    
    /// Send heartbeat to update last_heartbeat
    pub async fn heartbeat(&self) -> Result<()>;
    
    /// Mark sensor worker as inactive
    pub async fn deregister(&self) -> Result<()>;
    
    /// Get registered worker ID
    pub fn worker_id(&self) -> Option<i64>;
    
    /// Get worker name
    pub fn worker_name(&self) -> &str;
    
    /// Add custom capability
    pub fn add_capability(&mut self, key: String, value: serde_json::Value);
    
    /// Update capabilities in database
    pub async fn update_capabilities(&self) -> Result<()>;
}
```

---

## See Also

- [Sensor Service Architecture](../architecture/sensor-service.md)
- [Sensor Runtime Execution](sensor-runtime.md)
- [Worker Service Documentation](../architecture/worker-service.md)
- [Configuration Guide](../configuration/configuration.md)

---

**Status:** ✅ Implemented  
**Next Steps:** Implement distributed sensor scheduling based on worker capabilities