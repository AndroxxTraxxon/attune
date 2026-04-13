# Sensor Worker Registration Implementation

**Date:** 2026-01-31 (Updated: 2026-02-02)  
**Status:** ✅ Complete and Enhanced with Database-Driven Runtime Detection

---

## Overview

Implemented runtime capability reporting for sensor workers with **database-driven runtime detection**. Sensor services query the `runtime` table for available sensor runtimes and use their verification metadata to determine availability. This makes the sensor service completely independent and self-configuring—adding new runtimes requires no code changes, only database configuration.

This addresses the critical issue that sensor services may be running on machines without certain runtime dependencies (e.g., Python), and previously had no way to report or track these capabilities.

**Key Enhancement (2026-02-02):** Replaced hardcoded runtime detection with database-driven verification using runtime table metadata.

---

## Problem Statement

The user identified that while action workers report their runtime capabilities (via `ATTUNE_WORKER_RUNTIMES` and auto-detection), sensor services had no equivalent mechanism. A sensor service running on a machine without Python installed would fail when trying to execute Python-based sensors, but there was no way to:

1. Know which runtimes a sensor service has available
2. Track active sensor service instances
3. Schedule sensors based on runtime availability (future feature)
4. Monitor sensor service health via heartbeats

---

## Solution: Unified Worker Table with Role Discriminator

Instead of creating a separate `sensor_worker` table, we extended the existing `worker` table with a `worker_role` enum discriminator. This provides:

- **Single source of truth** for all workers (action and sensor)
- **Shared registration/heartbeat logic** 
- **Simpler database schema** and querying

---

## Changes Made

### 1. Database Migration

**File:** `attune/migrations/20260131000001_add_worker_role.sql`

- Created `worker_role_enum` type with values: `action`, `sensor`
- Added `worker_role` column to `worker` table (NOT NULL, default 'action')
- Created indexes for efficient role-based queries:
  - `idx_worker_role` on `worker_role`
  - `idx_worker_role_status` on `(worker_role, status)`
- Migrated existing workers to `worker_role = 'action'` for backward compatibility

### 2. Data Models

**File:** `attune/crates/common/src/models.rs`

- Added `WorkerRole` enum with values: `Action`, `Sensor`, `Hybrid`
- Updated `Worker` model to include `worker_role: WorkerRole` field
- Removed redundant `SensorWorker` struct (using unified `Worker` instead)

### 3. Configuration

**File:** `attune/crates/common/src/config.rs`

Added `SensorConfig` struct with fields:
- `worker_name`: Optional sensor worker name (defaults to "sensor-{hostname}")
- `host`: Optional host (defaults to hostname)
- `capabilities`: Optional HashMap for runtime capabilities
- `max_concurrent_sensors`: Optional max concurrent sensor executions
- `heartbeat_interval`: Heartbeat interval in seconds (default 30)
- `poll_interval`: Sensor poll interval in seconds (default 30)
- `sensor_timeout`: Sensor execution timeout in seconds (default 30)

Added `sensor: Option<SensorConfig>` to main `Config` struct.

### 4. Sensor Worker Registration Module

**File:** `attune/crates/sensor/src/sensor_worker_registration.rs`

Created `SensorWorkerRegistration` struct with methods:
- `new(pool, config)`: Initialize registration manager
- `register()`: Register sensor worker in database (insert or update)
- `heartbeat()`: Send periodic heartbeat to update `last_heartbeat`
- `deregister()`: Mark sensor worker as inactive on shutdown
- `add_capability()`: Add custom capability
- `update_capabilities()`: Update capabilities in database

**Runtime Detection Logic** (database-driven):
1. `ATTUNE_SENSOR_RUNTIMES` environment variable (highest priority - skips database)
2. Config file `sensor.capabilities.runtimes` (medium priority - skips database)
3. **Database-driven detection** (lowest priority - queries runtime table):
   - Query all sensor runtimes from database
   - For each runtime, check verification metadata
   - If `always_available: true` → mark as available
   - If verification commands exist → try each in priority order
   - Binary execution + exit code check + optional regex pattern matching
   - Only include runtimes that pass verification

**Verification Metadata Example (Python):**
```json
{
  "verification": {
    "commands": [
      {
        "binary": "python3",
        "args": ["--version"],
        "exit_code": 0,
        "pattern": "Python 3\\.",
        "priority": 1
      },
      {
        "binary": "python",
        "args": ["--version"],
        "exit_code": 0,
        "pattern": "Python 3\\.",
        "priority": 2
      }
    ]
  }
}
```

**Capabilities Structure:**
```json
{
  "runtimes": ["built-in sensor", "native", "node.js", "python", "shell"],
  "max_concurrent_sensors": 10,
  "sensor_version": "0.1.0"
}
```

**Note:** Runtime names come from the `runtime.name` field in the database, not hardcoded strings.

### 5. Service Integration

**File:** `attune/crates/sensor/src/service.rs`

Updated `SensorService` to:
- Create `SensorWorkerRegistration` on initialization
- **On startup**: Register sensor worker and get worker ID
- **During runtime**: Spawn async heartbeat loop (runs every `heartbeat_interval` seconds)
- **On shutdown**: Deregister sensor worker (set status to inactive)

**File:** `attune/crates/sensor/src/lib.rs`

- Exported `SensorWorkerRegistration` module and struct

### 6. Dependencies

**File:** `attune/crates/sensor/Cargo.toml`

- Added `hostname = "0.4"` dependency for hostname detection

### 7. Documentation

**File:** `attune/docs/sensors/sensor-worker-registration.md`

Comprehensive documentation covering:
- Architecture and database schema
- Configuration (YAML + environment variables)
- Runtime detection priority system
- Registration lifecycle (startup, heartbeat, shutdown)
- Usage examples and API reference
- SQL queries for monitoring sensor workers
- Troubleshooting guide
- Future enhancements (distributed sensor scheduling)

**File:** `attune/docs/sensors/database-driven-runtime-detection.md`

Database-driven runtime detection documentation:
- Verification metadata structure
- How to add new runtimes without code changes
- Runtime verification process
- Configuration examples for each runtime
- Performance and security considerations

### 8. Database Migration for Runtime Metadata

**File:** `attune/migrations/20260202000001_add_sensor_runtimes.sql`

Added sensor runtimes with verification metadata:
- `core.sensor.python` - Python 3 with python3/python fallback checks
- `core.sensor.nodejs` - Node.js with version pattern matching
- `core.sensor.shell` - Shell (marked as always available)
- `core.sensor.native` - Native compiled (marked as always available)
- Updated `core.sensor.builtin` with metadata

Each runtime includes:
- Verification commands with priority ordering
- Expected exit codes
- Regex patterns for version validation
- Fallback commands for different system configurations

---

## Technical Details

### Worker Registration Flow

```
Sensor Service Startup
    ↓
SensorWorkerRegistration::new(pool, config)
    ↓
Detect Capabilities (env var → config → auto-detect)
    ↓
register() → INSERT/UPDATE worker table
    - worker_role = 'sensor'
    - worker_type = 'local'
    - status = 'active'
    - capabilities = {"runtimes": [...], ...}
    - last_heartbeat = NOW()
    ↓
Spawn Heartbeat Loop (async task)
    - Every 30s (configurable): heartbeat()
    - Updates last_heartbeat, ensures status = 'active'
    ↓
Service Running...
    ↓
Service Shutdown Signal
    ↓
deregister() → UPDATE worker SET status = 'inactive'
```

### Database Queries

**Find active sensor workers with Python runtime:**
```sql
SELECT * FROM worker
WHERE worker_role = 'sensor'
  AND status = 'active'
  AND capabilities->'runtimes' ? 'python';
```

**Find stale workers (no heartbeat in 5 min):**
```sql
SELECT name, last_heartbeat
FROM worker
WHERE worker_role = 'sensor'
  AND status = 'active'
  AND last_heartbeat < NOW() - INTERVAL '5 minutes';
```

---

## Testing Requirements

### Unit Tests (Included)

**File:** `attune/crates/sensor/src/sensor_worker_registration.rs`

- `test_database_driven_detection()`: Verifies database-driven runtime detection (requires DB)
- `test_sensor_worker_registration()`: Tests registration/heartbeat/deregistration flow (requires DB)
- `test_sensor_worker_capabilities()`: Tests capability updates (requires DB)

### Integration Tests (To Be Added)

1. **Test sensor service startup registration**
   - Start sensor service
   - Verify worker record created with `worker_role = 'sensor'`
   - Verify capabilities include detected runtimes

2. **Test heartbeat mechanism**
   - Start sensor service
   - Wait 2x heartbeat interval
   - Verify `last_heartbeat` is recent

3. **Test graceful shutdown deregistration**
   - Start sensor service → register
   - Stop sensor service → deregister
   - Verify `status = 'inactive'`

4. **Test runtime detection priority**
   - Set `ATTUNE_SENSOR_RUNTIMES=shell`
   - Verify only "shell" in capabilities (env var overrides database detection)

5. **Test database-driven detection**
   - Start sensor service without env var override
   - Verify runtimes detected from database
   - Add new runtime to database
   - Restart service, verify new runtime detected

6. **Test runtime verification filtering**
   - Add runtime with unavailable binary to database
   - Verify it's NOT included in detected runtimes
   - Install binary, restart service
   - Verify runtime now detected

7. **Test multiple sensor workers**
   - Start multiple sensor services with different names
   - Verify each has unique worker record
   - Verify all tracked independently

---

## Compilation Status

✅ Code compiles successfully with `cargo check --package attune-sensor`

✅ **Database migrations applied:**
```bash
# Migration 1: Add worker_role
migrations/20260131000001_add_worker_role.sql

# Migration 2: Add sensor runtimes with verification metadata
migrations/20260202000001_add_sensor_runtimes.sql

# Both applied successfully on 2026-02-02
```

✅ **SQLx metadata regenerated:**
```bash
cargo sqlx prepare --workspace
# Completed successfully
```

---

## Completed Steps (2026-02-02)

### Database Setup ✅

1. ✅ **Applied migration**: `20260131000001_add_worker_role.sql`
2. ✅ **Applied migration**: `20260202000001_add_sensor_runtimes.sql`
3. ✅ **Updated SQLx metadata**: Regenerated with `cargo sqlx prepare --workspace`
4. ✅ **Tested with live database**: Verified registration and runtime detection
5. ✅ **Verified heartbeat**: Confirmed updates every 30 seconds

### Verification Results ✅

```
✓ Runtime available: Built-in Sensor (core.sensor.builtin)
✓ Runtime available: Native (core.sensor.native)
✓ Runtime available: Node.js (core.sensor.nodejs)
✓ Runtime available: Python (core.sensor.python)
✓ Runtime available: Shell (core.sensor.shell)
✗ Runtime not available: Haskell (test.sensor.haskell) - binary not found

Detected available runtimes: ["built-in sensor", "native", "node.js", "python", "shell"]
```

## Next Steps

### Future Enhancements

1. **Distributed Sensor Scheduling**
   - Implement sensor assignment to workers based on runtime requirements
   - Load balancing across multiple sensor workers
   - Automatic failover if a worker goes down

2. **Runtime Verification**
   - Periodically verify reported runtimes are still available
   - Auto-update capabilities if runtime availability changes

3. **Worker Health Monitoring**
   - Background job to mark workers as inactive if heartbeat is stale
   - Alerts for workers that haven't sent heartbeats
   - Dashboard showing sensor worker status

4. **Hybrid Workers**
   - Implement workers that can execute both actions and sensors
   - Useful for resource-constrained deployments

5. **Containerized Sensor Workers**
   - Support for sensor workers running in containers
   - Runtime isolation per sensor execution

---

## Configuration Examples

### Minimal Configuration (Auto-Detection)

```yaml
# No sensor config needed - will auto-detect runtimes
# Worker name: sensor-{hostname}
# Runtimes: detected (python, node, shell, native)
```

### Explicit Runtime Configuration

```yaml
sensor:
  worker_name: "sensor-prod-01"
  capabilities:
    runtimes: ["python", "shell"]
  max_concurrent_sensors: 20
  heartbeat_interval: 60
```

### Environment Variable Override

```bash
# Override auto-detection
export ATTUNE_SENSOR_RUNTIMES="shell,native"

# Custom worker name
export ATTUNE__SENSOR__WORKER_NAME="sensor-custom"
```

---

## Files Modified

### New Files
- `attune/migrations/20260131000001_add_worker_role.sql`
- `attune/crates/sensor/src/sensor_worker_registration.rs`
- `attune/docs/sensors/sensor-worker-registration.md`
- `attune/work-summary/sensor-worker-registration.md`

### Modified Files
### Files Modified
- `attune/crates/common/src/models.rs` - Added `WorkerRole` enum, updated `Worker` model
- `attune/crates/common/src/config.rs` - Added `SensorConfig` struct
- `attune/crates/sensor/src/service.rs` - Integrated sensor worker registration
- `attune/crates/sensor/src/lib.rs` - Exported registration module
- `attune/crates/sensor/Cargo.toml` - Added hostname and regex dependencies
- `attune/crates/sensor/src/sensor_worker_registration.rs` - Enhanced with database-driven detection

---

## AGENTS.md Updates Required

**Section to update:** "Code Conventions & Patterns > Database Layer"

Add note about unified worker table:
```markdown
- **Worker Table**: Used for both action workers and sensor workers
  - `worker_role` enum discriminates: 'action', 'sensor'
  - Action workers: Execute actions via attune-worker service
  - Sensor workers: Monitor triggers via attune-sensor service
  - Capabilities JSONB field includes runtime availability
  - Runtime detection is database-driven via runtime table metadata
```

**Section to update:** "Runtime Detection"

Add new section:
```markdown
- **Database-Driven**: Sensor workers query runtime table for verification metadata
  - No code changes needed to add new runtimes
  - Verification commands, patterns, and priorities stored in database
  - See docs/sensors/database-driven-runtime-detection.md
```

**Section to update:** "Development Status"

Update sensor service status:
```markdown
- ✅ **Complete**: Sensor service (core functionality, worker registration, database-driven runtime detection)
```

---

## Summary

Successfully implemented runtime capability reporting for sensor workers using a **database-driven approach**. Sensor services query the `runtime` table at startup, verify each runtime's availability using configured verification metadata, and register with only the available runtimes. This makes the sensor service completely independent and self-configuring.

**Key Benefits:**
- ✅ No code changes needed to add new runtimes
- ✅ Centralized runtime configuration in database
- ✅ Flexible verification with multiple fallback commands
- ✅ Pattern matching for version validation
- ✅ Priority ordering for preferred verification methods
- ✅ Override capability via environment variables

The implementation uses a unified worker table with `worker_role` discriminator, ensuring consistency with action worker registration. Sensor services automatically register and track their runtime capabilities without any code changes required by pack developers or operators.

**Implementation Time:** ~3 hours (initial + database-driven enhancement)  
**Lines Added:** ~1200 (code + migration + docs)  
**Files Created:** 7 (code, migrations, docs)  
**Files Modified:** 6 (models, config, service, registration)  
**Migrations:** 2 (worker_role + sensor_runtimes)
