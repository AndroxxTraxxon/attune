# Work Summary: Sensor Lifecycle Management Implementation

**Date**: January 29, 2025  
**Author**: AI Assistant  
**Status**: Partial Implementation (Design Complete, Code Needs Fixes)

## Overview

Implemented intelligent sensor lifecycle management to ensure sensors only run when there are active/enabled rules subscribing to their triggers. When the last rule is disabled or deleted, the sensor is stopped and its API token is revoked. This optimizes resource usage and enhances security.

## Problem Statement

Previously, sensors would run continuously once enabled, regardless of whether any rules were consuming their events. This led to:

- Wasted CPU/memory on sensors without consumers
- Security risk of active API tokens for unused sensors
- Unnecessary cloud infrastructure costs
- Poor resource efficiency

## Solution Design

### Core Concept

**Sensor State = f(Active Rules)**

- If active rules > 0: Sensor SHOULD BE RUNNING
- If active rules = 0: Sensor SHOULD BE STOPPED (token revoked)

### Architecture

1. **SensorManager** - Manages sensor process lifecycle, tracks running sensors
2. **RuleLifecycleListener** - Subscribes to rule events via RabbitMQ
3. **Database Queries** - Tracks active rule counts per trigger/sensor
4. **Token Management** - Issues tokens on start, revokes on stop (to be implemented)

### Data Model

```
Rule ──(trigger_id)──> Trigger <──(trigger_id)── Sensor
                         ↑
                         │
                  Rule subscriptions
                  (COUNT for lifecycle)
```

Query to determine if sensor should run:
```sql
SELECT COUNT(*) FROM rule
WHERE trigger = (SELECT trigger FROM sensor WHERE id = $sensor_id)
  AND enabled = TRUE;
```

## Implementation Details

### Files Modified

#### 1. `attune/crates/sensor/src/sensor_manager.rs`

**Added methods:**

- `has_active_rules(sensor_id)` - Checks if sensor has any enabled rules
- `get_active_rule_count(sensor_id)` - Returns count of active rules
- `stop_sensor(sensor_id, revoke_token)` - Stops sensor and optionally revokes token
- `handle_rule_change(trigger_id)` - Core lifecycle orchestration method

**Modified:**

- `start()` - Now checks active rules before starting each sensor
  - Only starts sensors with active rules
  - Logs skip message for sensors without rules

**Logic in `handle_rule_change()`:**

| Active Rules | Sensor Running | Action                      |
|--------------|----------------|-----------------------------|
| Yes          | Yes            | No action (continue)        |
| Yes          | No             | Start sensor + issue token  |
| No           | Yes            | Stop sensor + revoke token  |
| No           | No             | No action (remain stopped)  |

#### 2. `attune/crates/sensor/src/rule_lifecycle_listener.rs`

**Added:**

- `sensor_manager: Arc<SensorManager>` field
- `get_trigger_id_for_rule()` helper method
- Sensor lifecycle notifications in all event handlers:
  - `handle_rule_created()` - Start sensor if needed
  - `handle_rule_enabled()` - Start sensor if needed
  - `handle_rule_disabled()` - Stop sensor if last rule

**Modified:**

- Constructor now accepts `sensor_manager` parameter
- All event handlers now call `sensor_manager.handle_rule_change(trigger_id)`

**Event Flow:**

```
RabbitMQ Event (rule.created/enabled/disabled)
    ↓
RuleLifecycleListener.handle_message()
    ↓
Get trigger_id for rule (from DB if needed)
    ↓
sensor_manager.handle_rule_change(trigger_id)
    ↓
Query active rule count
    ↓
Start/Stop sensor based on state matrix
```

#### 3. `attune/crates/sensor/src/service.rs`

**Modified:**

- Pass `sensor_manager.clone()` to `RuleLifecycleListener::new()`

### Files Created

#### 1. `attune/docs/sensor-lifecycle-management.md`

Comprehensive documentation covering:

- Architecture overview and data flow diagrams
- Rule-Sensor-Trigger relationship
- Lifecycle states and transitions
- Implementation details for all methods
- Token management (issuance, revocation, refresh)
- Process management (native vs script sensors)
- Database schema additions (future)
- Monitoring and observability
- API endpoints for token management
- Edge cases and error handling
- Migration strategy (4 phases)
- Testing strategy with example tests
- Future enhancements

## Current State

### What Works (Conceptually)

✅ **Design**: Complete architecture documented  
✅ **Database Queries**: Active rule counting logic implemented  
✅ **Integration Points**: SensorManager integrated with RuleLifecycleListener  
✅ **Logic**: State transition matrix implemented  
✅ **Documentation**: Comprehensive guide created

### What Needs Fixing

❌ **Compilation Errors**: Several errors in sensor crate:
- `sqlx` query macros require `DATABASE_URL` or prepared queries
- `Consumer::clone()` not available (API mismatch)
- Type mismatches in timer manager
- String error type not compatible with `anyhow::Error`

❌ **Token Management**: Not yet implemented
- API endpoint for sensor token issuance
- Token revocation endpoint
- Token cleanup job

❌ **Process Management**: Native sensor spawning not implemented
- PID tracking
- SIGTERM/SIGKILL signal handling
- Process health monitoring

❌ **Testing**: No tests written yet

## Technical Debt

### Immediate Fixes Needed

1. **Fix SQLx Queries**: Run `cargo sqlx prepare` with DATABASE_URL set
2. **Fix Consumer API**: Check correct `lapin` Consumer API for message handling
3. **Fix TimerManager Errors**: Make timer mutable where needed
4. **Error Type Conversions**: Convert String errors to anyhow::Error

### Future Work

1. **Phase 2: Token Management**
   - Implement `/auth/sensor-token` endpoint
   - Implement `/auth/revoke/:token_id` endpoint
   - Add token cleanup job for expired revocations
   - Update sensor startup to use issued tokens

2. **Phase 3: Process Management**
   - Track sensor PIDs in SensorManager
   - Implement graceful shutdown (SIGTERM → SIGKILL)
   - Add process health monitoring loop
   - Implement restart logic with exponential backoff

3. **Phase 4: Observability**
   - Structured logging for lifecycle events
   - Prometheus metrics (sensor count, starts, stops, crashes)
   - Admin API endpoint: `GET /api/v1/sensors/:id/status`
   - Dashboard for sensor management

## Benefits

### When Complete

1. **Resource Efficiency**
   - Sensors only run when needed
   - Estimated 50-80% reduction in idle sensor processes
   - Lower memory/CPU consumption

2. **Security**
   - Tokens revoked when not in use
   - Reduced attack surface
   - Automatic token refresh for running sensors

3. **Cost Optimization**
   - Reduced cloud infrastructure costs
   - Pay only for active sensor compute time

4. **Operational Simplicity**
   - Self-managing sensor lifecycle
   - No manual intervention required
   - Clear observability into sensor state

## Testing Plan

### Unit Tests Needed

- `test_sensor_starts_with_active_rules()`
- `test_sensor_stops_when_last_rule_disabled()`
- `test_sensor_remains_stopped_without_rules()`
- `test_sensor_continues_running_with_rules()`
- `test_multiple_rules_same_trigger()`

### Integration Tests Needed

- `test_end_to_end_lifecycle()`
- `test_rapid_rule_toggling()` (debounce test)
- `test_sensor_crash_handling()`
- `test_token_revocation_failure()`
- `test_database_connectivity_loss()`

### Manual Testing

1. Start system with no rules → Verify sensors NOT running
2. Create enabled rule → Verify sensor STARTS
3. Disable rule → Verify sensor STOPS
4. Check token revocation in DB
5. Re-enable rule → Verify sensor RE-STARTS

## Migration Path

### For Existing Deployments

1. **Deploy code changes** (once compilation fixed)
2. **Run database migrations** (if schema changes needed)
3. **Restart sensor service**
4. Sensors will auto-stop if no active rules
5. Monitor logs for lifecycle events

### Rollback Plan

If issues occur:
1. Revert to previous sensor service version
2. Sensors will resume always-on behavior
3. No data loss (only behavioral change)

## Example Scenarios

### Scenario 1: New Rule Created

```
1. User creates rule: "Send email every 5 minutes"
   - Trigger: core.intervaltimer
   - Action: core.send_email
   - Enabled: true

2. Rule saved to database
   └─> RabbitMQ event: rule.created

3. RuleLifecycleListener receives event
   └─> Gets trigger_id = 1 (core.intervaltimer)

4. sensor_manager.handle_rule_change(1)
   └─> Query: "SELECT COUNT(*) FROM rule WHERE trigger=1 AND enabled=true"
   └─> Result: 1 (this new rule)

5. Find sensors for trigger 1
   └─> Found: core.interval_timer_sensor

6. Check if sensor running
   └─> Not running

7. ACTION: Start sensor
   ├─> Issue API token (90-day TTL)
   ├─> Spawn attune-core-timer-sensor binary
   ├─> Pass token via ATTUNE_API_TOKEN env var
   └─> Register PID in SensorManager

8. Sensor starts emitting events every 5 minutes
9. Rule matcher triggers email action
```

### Scenario 2: Last Rule Disabled

```
1. User disables rule (only rule for timer trigger)

2. Rule.enabled = false in database
   └─> RabbitMQ event: rule.disabled

3. RuleLifecycleListener receives event
   └─> Gets trigger_id = 1

4. sensor_manager.handle_rule_change(1)
   └─> Query: "SELECT COUNT(*) FROM rule WHERE trigger=1 AND enabled=true"
   └─> Result: 0 (no active rules)

5. Find sensors for trigger 1
   └─> Found: core.interval_timer_sensor (currently running)

6. Check if sensor running
   └─> Yes, PID 12345

7. ACTION: Stop sensor
   ├─> Send SIGTERM to PID 12345
   ├─> Wait up to 30s for graceful shutdown
   ├─> Call API: DELETE /auth/token/:token_id
   ├─> Token added to revocation table
   ├─> Remove from running sensors registry
   └─> Log: "Sensor stopped, token revoked"

8. Sensor process exits cleanly
9. Resources freed
```

## Metrics to Track

Once implemented, monitor:

- **sensor.lifecycle.starts** (counter) - Total sensor starts
- **sensor.lifecycle.stops** (counter) - Total sensor stops
- **sensor.lifecycle.crashes** (counter) - Unexpected terminations
- **sensor.active.count** (gauge) - Currently running sensors
- **sensor.token.issued** (counter) - Tokens issued
- **sensor.token.revoked** (counter) - Tokens revoked
- **sensor.restart.attempts** (histogram) - Restart attempt counts

## Conclusion

The sensor lifecycle management feature is well-designed and partially implemented. The core logic is in place, but compilation errors prevent testing. Once the sensor crate is fixed and token management is implemented, this feature will provide significant resource and security benefits with minimal operational overhead.

## Next Steps

1. **Fix sensor crate compilation** (highest priority)
2. **Implement token management API endpoints**
3. **Add process management for native sensors**
4. **Write comprehensive tests**
5. **Deploy to staging for validation**
6. **Monitor metrics and iterate**

## Related Work

- [Native Runtime Support](2025-01-29-native-runtime-support.md) - Enables efficient binary sensor execution
- [Timer Sensor Implementation](../crates/sensor-timer/README.md) - First native sensor
- [Token Security Architecture](../docs/token-security.md) - Token refresh and revocation design

## Files Changed

- `attune/crates/sensor/src/sensor_manager.rs` - Core lifecycle logic
- `attune/crates/sensor/src/rule_lifecycle_listener.rs` - Event integration
- `attune/crates/sensor/src/service.rs` - Dependency wiring

## Files Created

- `attune/docs/sensor-lifecycle-management.md` - Comprehensive documentation (562 lines)

---

**Status**: Implementation paused due to compilation errors in sensor crate. Design is complete and ready for execution once sensor service is fixed.
