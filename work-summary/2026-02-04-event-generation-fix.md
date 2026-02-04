# Event Generation Fix and Timer Sensor Cleanup

**Date:** 2026-02-04  
**Status:** Complete ✅

## Problem

Events were not being generated and inserted into the database, despite rules being configured. The timer sensor was outputting JSON to stdout but events were never created in the database.

## Root Cause Analysis

The system had **two timer sensor implementations**:

1. **`attune-timer-sensor`** (669KB) - Old subprocess-based sensor
   - Located in `crates/timer-sensor-subprocess/`
   - Outputs JSON events to stdout
   - Expects sensor manager to parse stdout and create events
   - Does NOT follow the documented sensor protocol

2. **`attune-core-timer-sensor`** (5.8MB) - Correct API-based sensor
   - Located in `crates/sensor-timer/`
   - Makes HTTP POST requests to `/events` API endpoint
   - Follows the documented sensor interface specification
   - Handles rule lifecycle, token refresh, and proper event creation

**The database was configured to use the wrong sensor** (`attune-timer-sensor`), causing events to be logged to stdout but never persisted to the database.

## Investigation Steps

1. **Initial symptom**: Events visible in logs but not in database
   ```
   "fired_at":"2026-02-04T01:21:28.028878792+00:00"
   ```

2. **Discovered sensor manager wasn't processing stdout**: 
   - Sensor manager was just logging stdout, not parsing JSON
   - No event creation or rule matching logic in sensor manager

3. **Found documentation mismatch**:
   - `docs/sensors/sensor-interface.md` specifies sensors should POST to `/events` API
   - Old subprocess sensor was outputting to stdout instead

4. **Identified dual implementations**:
   - Both sensors were being built in Docker
   - Database pointed to wrong one

## Solution

### Part 1: Switch to Correct Sensor Binary

**Database Update:**
```sql
UPDATE sensor 
SET entrypoint = 'attune-core-timer-sensor' 
WHERE ref = 'core.interval_timer_sensor';
```

**Fixed Trigger Parameters:**
The proper sensor expects a tagged enum format:
```json
{
  "type": "interval",
  "interval": 1,
  "unit": "seconds"
}
```

Updated rules to include the missing `"type"` field:
```sql
UPDATE rule 
SET trigger_params = '{"type": "interval", "interval": 1, "unit": "seconds"}' 
WHERE trigger = 3;
```

### Part 2: Remove Old Sensor Implementation

To prevent future confusion, completely removed the old subprocess-based sensor:

**Files Deleted:**
- `crates/timer-sensor-subprocess/` - entire crate directory

**Files Modified:**
- `Cargo.toml` - removed `timer-sensor-subprocess` from workspace members
- `docker/Dockerfile` - removed build steps for `attune-timer-sensor` binary
- `packs/core/sensors/interval_timer_sensor.yaml` - updated `entry_point` to `attune-core-timer-sensor`

## Verification

After the fix, the complete event-driven automation pipeline is working:

```
Timer fires → Event created → Rule evaluated → Enforcement created → Execution runs → Action completes
```

**Database verification:**
```sql
-- Events being created every second
SELECT id, trigger_ref, created FROM event ORDER BY id DESC LIMIT 5;
-- Results: id=965-974, trigger_ref=core.intervaltimer

-- Enforcements created for matching rules
SELECT id, rule_ref, event, status FROM enforcement ORDER BY id DESC LIMIT 5;
-- Results: Both rules (default.echo_every_second, core.echo_every_second) creating enforcements

-- Executions running and completing
SELECT id, action_ref, status, result->'stdout' FROM execution ORDER BY id DESC LIMIT 5;
-- Results: status=completed, stdout="Hello, World!\n"
```

## Technical Details

### Sensor Manager Enhancement

Added `fetch_trigger_instances()` method to query enabled rules and pass their configurations to sensor subprocesses:

**File:** `crates/sensor/src/sensor_manager.rs`

```rust
async fn fetch_trigger_instances(&self, trigger_id: Id) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        r#"
        SELECT *
        FROM rule
        WHERE trigger = $1
          AND enabled = TRUE
        "#,
    )
    .bind(trigger_id)
    .fetch_all(&self.inner.db)
    .await?;

    let trigger_instances: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id").unwrap_or(0);
            let ref_str: String = row.try_get("ref").unwrap_or_default();
            let trigger_params: serde_json::Value = row
                .try_get("trigger_params")
                .unwrap_or(serde_json::json!({}));

            serde_json::json!({
                "id": id,
                "ref": ref_str,
                "config": trigger_params
            })
        })
        .collect();

    Ok(trigger_instances)
}
```

The sensor subprocess receives this via `ATTUNE_SENSOR_TRIGGERS` environment variable.

### Sensor Protocol Compliance

The correct sensor (`attune-core-timer-sensor`) follows the documented protocol:

1. **Initialization:**
   - Reads configuration from environment variables
   - Validates API connectivity
   - Provisions authentication token

2. **Rule Discovery:**
   - Fetches active rules for its trigger type from API
   - Subscribes to RabbitMQ for rule lifecycle events (created, enabled, disabled, deleted)

3. **Event Generation:**
   - Monitors timers based on rule configurations
   - POSTs to `/events` API endpoint when timers fire
   - API handles event creation, rule matching, and enforcement creation

4. **Token Management:**
   - Automatically refreshes tokens at 80% of TTL
   - Handles token expiration gracefully

## Remaining Artifacts

The old binary `attune-timer-sensor` may still exist in:
- Running Docker containers (until rebuilt)
- Local `target/` build directories (until cleaned)
- Existing Docker images (until rebuilt)

These will be naturally cleaned up through normal build/deploy cycles.

## Lessons Learned

1. **Documentation is critical**: The sensor interface spec was correct, but an outdated implementation was still in the codebase
2. **Name similarity caused confusion**: `attune-timer-sensor` vs `attune-core-timer-sensor` are easy to confuse
3. **Remove deprecated code promptly**: Having two implementations led to using the wrong one
4. **Database-driven configuration**: The sensor `entrypoint` in the database must match the correct binary name

## Impact

- ✅ Events are now being created and persisted to database
- ✅ Rules are being evaluated and enforcements created
- ✅ Actions are executing successfully
- ✅ Complete end-to-end automation pipeline functional
- ✅ Codebase cleaned up (removed ~1,200 lines of obsolete code)
- ✅ Documentation and implementation now aligned

## Related Files

**Modified:**
- `attune/crates/sensor/src/sensor_manager.rs` - Added trigger instance fetching
- `attune/Cargo.toml` - Removed old crate from workspace
- `attune/docker/Dockerfile` - Removed old binary build
- `attune/packs/core/sensors/interval_timer_sensor.yaml` - Updated entry_point

**Deleted:**
- `attune/crates/timer-sensor-subprocess/` (entire directory)

**Database:**
- Updated `sensor.entrypoint` for `core.interval_timer_sensor`
- Updated `rule.trigger_params` format to include `"type"` field
