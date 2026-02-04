# Quick Reference: Unified Runtime Detection System

**Last Updated:** 2024-02-03  
**Status:** Production-ready

---

## Overview

Both worker and sensor services use a **unified runtime detection system** from `attune-common` that:
- Queries a single `runtime` table (no action/sensor distinction)
- Verifies runtime availability using database-stored verification metadata
- Supports three-tier configuration: env var → config file → database detection

---

## Key Changes from Previous System

### What Changed
- ❌ **Removed:** `runtime_type` field (was `'action'` or `'sensor'`)
- ❌ **Removed:** Duplicate runtime records for actions vs sensors
- ✅ **Added:** Unified `RuntimeDetector` in `attune-common`
- ✅ **Added:** Shared verification logic for all services

### Runtime Refs Changed
```
Before: core.action.python, core.sensor.python
After:  core.python (used by both)
```

---

## Quick Start

### Worker Service

```rust
use attune_common::runtime_detection::RuntimeDetector;

let mut registration = WorkerRegistration::new(pool, &config);
registration.detect_capabilities(&config).await?;
registration.register().await?;
```

### Sensor Service

```rust
use attune_common::runtime_detection::RuntimeDetector;

let mut registration = SensorWorkerRegistration::new(pool, &config);
registration.register(&config).await?;  // Calls detection internally
```

---

## Configuration Priority

### 1. Environment Variable (Highest)
```bash
# Worker
export ATTUNE_WORKER_RUNTIMES="python,shell,node"

# Sensor
export ATTUNE_SENSOR_RUNTIMES="python,shell,builtin"
```

### 2. Config File (Medium)
```yaml
worker:
  capabilities:
    runtimes: ["python", "shell", "native"]

sensor:
  capabilities:
    runtimes: ["python", "shell", "builtin"]
```

### 3. Database Detection (Default)
- Queries all runtimes from `runtime` table
- Verifies each using `distributions->verification` metadata
- Reports only available runtimes

---

## Database Structure

### Runtime Table (Unified)

```sql
CREATE TABLE runtime (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES pack(id),
    pack_ref TEXT,
    description TEXT,
    name TEXT NOT NULL,                    -- e.g., "Python", "Node.js"
    distributions JSONB NOT NULL,          -- Verification metadata
    installation JSONB,
    created TIMESTAMPTZ DEFAULT NOW(),
    updated TIMESTAMPTZ DEFAULT NOW()
);
```

**No `runtime_type` field** - runtimes are shared between actions and sensors.

### Verification Metadata Structure

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
      }
    ],
    "always_available": false,
    "check_required": true
  }
}
```

---

## Common Runtime Refs

| Runtime | Ref | Always Available? |
|---------|-----|-------------------|
| Python | `core.python` | No (requires verification) |
| Node.js | `core.nodejs` | No (requires verification) |
| Shell | `core.shell` | Yes |
| Native | `core.native` | Yes |
| Built-in Sensors | `core.sensor.builtin` | Yes (sensor-only) |

---

## RuntimeDetector API

### Methods

```rust
pub struct RuntimeDetector {
    pool: PgPool,
}

impl RuntimeDetector {
    pub fn new(pool: PgPool) -> Self

    pub async fn detect_capabilities(
        &self,
        config: &Config,
        env_var_name: &str,
        config_capabilities: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<HashMap<String, serde_json::Value>>

    pub async fn detect_from_database(&self) -> Result<Vec<String>>

    pub async fn verify_runtime_available(runtime: &Runtime) -> bool
}
```

### Example Usage

```rust
use attune_common::runtime_detection::RuntimeDetector;

let detector = RuntimeDetector::new(pool.clone());

// For worker service
let capabilities = detector
    .detect_capabilities(
        &config,
        "ATTUNE_WORKER_RUNTIMES",
        config.worker.as_ref().and_then(|w| w.capabilities.as_ref())
    )
    .await?;

// For sensor service
let capabilities = detector
    .detect_capabilities(
        &config,
        "ATTUNE_SENSOR_RUNTIMES",
        config.sensor.as_ref().and_then(|s| s.capabilities.as_ref())
    )
    .await?;
```

---

## Migration

### Apply Migration

```bash
cd attune
sqlx migrate run
```

**Migration:** `20260203000001_unify_runtimes.sql`

**What It Does:**
- Consolidates duplicate runtime records
- Migrates foreign keys in `action` and `sensor` tables
- Drops `runtime_type` column and enum
- Updates indexes

### Verify Migration

```sql
-- Check unified runtimes
SELECT ref, name FROM runtime ORDER BY ref;

-- Expected:
-- core.native
-- core.nodejs
-- core.python
-- core.sensor.builtin
-- core.shell

-- Check worker capabilities
SELECT name, capabilities->'runtimes' FROM worker;
```

---

## Adding New Runtimes

### 1. Add to Database

```sql
INSERT INTO runtime (ref, pack, pack_ref, name, distributions)
VALUES (
    'core.ruby',
    (SELECT id FROM pack WHERE ref = 'core'),
    'core',
    'Ruby',
    jsonb_build_object(
        'verification', jsonb_build_object(
            'commands', jsonb_build_array(
                jsonb_build_object(
                    'binary', 'ruby',
                    'args', jsonb_build_array('--version'),
                    'exit_code', 0,
                    'pattern', 'ruby \d+\.\d+',
                    'priority', 1
                )
            )
        )
    )
);
```

### 2. Restart Services

Services will automatically detect the new runtime on next startup.

### 3. Verify

```sql
SELECT name, capabilities->'runtimes' FROM worker WHERE name = 'worker-hostname';
-- Should include 'ruby' if installed
```

---

## Troubleshooting

### Runtime Not Detected

**Check verification command:**
```bash
python3 --version  # Does this work?
node --version     # Does this work?
```

**Check database metadata:**
```sql
SELECT ref, distributions->'verification' FROM runtime WHERE ref = 'core.python';
```

**Force detection:**
```bash
unset ATTUNE_WORKER_RUNTIMES  # Remove env override
# Restart service - will query database
```

### Wrong Runtimes Reported

**Priority order:**
1. Env var overrides everything
2. Config file if no env var
3. Database detection if neither

**Check env:**
```bash
env | grep ATTUNE.*RUNTIMES
```

**Check config:**
```bash
cat config.development.yaml | grep -A5 capabilities
```

### Update Runtime Verification

```sql
UPDATE runtime
SET distributions = jsonb_set(
    distributions,
    '{verification,commands,0,binary}',
    '"python3.11"'
)
WHERE ref = 'core.python';
```

Restart services to pick up changes.

---

## Code Locations

### Core Module
- `crates/common/src/runtime_detection.rs` - RuntimeDetector implementation
- `crates/common/src/models.rs` - Runtime model (no runtime_type)
- `crates/common/src/repositories/runtime.rs` - Database operations

### Service Integration
- `crates/worker/src/registration.rs` - Worker uses RuntimeDetector
- `crates/sensor/src/sensor_worker_registration.rs` - Sensor uses RuntimeDetector

### Migration
- `migrations/20260203000001_unify_runtimes.sql` - Schema changes

---

## Testing

### Unit Tests
```bash
cargo test -p attune-common runtime_detection
```

### Integration Tests
```bash
cargo test --test repository_runtime_tests
```

### Manual Verification
```bash
# Start worker with debug logging
RUST_LOG=debug cargo run -p attune-worker

# Check logs for:
# - "Detecting worker capabilities..."
# - "✓ Runtime available: Python (core.python)"
# - "Detected available runtimes: ["python", "shell", "native"]"
```

---

## Performance

### Query Optimization
- Runtime detection happens **once at startup**
- Results cached in worker registration
- No runtime queries during action/sensor execution

### Indexing
```sql
CREATE INDEX idx_runtime_name ON runtime(name);
CREATE INDEX idx_runtime_verification ON runtime USING gin ((distributions->'verification'));
```

---

## Security Considerations

### Command Execution
- Verification commands run **at startup only**
- Commands from database (trusted source)
- Output parsed with regex, not eval'd
- Non-zero exit codes handled safely

### Environment Overrides
- Env vars allow operators to restrict runtimes
- Useful for security-sensitive environments
- Can disable verification entirely with explicit list

---

## Future Enhancements

### Planned Features
1. **Version Constraints:** Require Python >=3.9
2. **Capability Matching:** Route work to compatible workers
3. **Health Checks:** Re-verify runtimes periodically
4. **API Endpoints:** GET /api/workers/{id}/capabilities

### Contribution Guide
- Add verification metadata to new runtime records
- Update `RuntimeDetector` for new verification types
- Keep worker and sensor services using shared detector

---

## Summary

✅ **One Runtime Table** - No action/sensor distinction  
✅ **Shared Detection Logic** - In `attune-common`  
✅ **Three-Tier Config** - Env → Config → Database  
✅ **Database-Driven** - Verification metadata in JSONB  
✅ **Extensible** - Add runtimes via SQL inserts  

**Migration Required:** Yes (`20260203000001_unify_runtimes.sql`)  
**Breaking Changes:** Yes (pre-production only)  
**Production Ready:** ✅ Yes