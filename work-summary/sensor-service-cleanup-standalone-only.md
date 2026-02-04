# Sensor Service Cleanup: Standalone Sensors Only

**Date**: 2024-02-02  
**Status**: Complete  
**Breaking Change**: Yes - Removes poll-based sensor support

## Overview

Removed the "builtin" poll-based sensor functionality from the sensor service. The sensor service now exclusively manages standalone sensor processes (like `attune-core-timer-sensor`) that run as independent daemons and communicate via the API.

## Motivation

The original architecture had two sensor modes:
1. **Poll-based sensors**: Scripts executed periodically by the sensor service, which would generate events directly
2. **Standalone sensors**: Independent processes that communicate via API

This created architectural confusion where the sensor service was both:
- Managing sensor process lifecycles ✅
- Directly generating events from sensor output ❌

The correct architecture is that **all sensors are independent processes** that use the API to create events. The sensor service should only manage process lifecycles.

## Changes Made

### Files Deleted

1. **`event_generator.rs`** - Only used for poll-based sensors to directly create events
2. **`rule_matcher.rs`** - Unused module for poll-based rule evaluation
3. **`timer_manager.rs`** - Unused (timer functionality now in `attune-core-timer-sensor` standalone)
4. **`sensor_runtime.rs`** - Only used for executing poll-based sensor scripts

### Files Modified

#### `lib.rs`
- Removed exports for deleted modules
- Cleaner public API surface

#### `service.rs`
- Removed `EventGenerator` dependency
- Updated `SensorManager` initialization to not require event generator
- Removed `event_generator()` accessor method
- Updated documentation

#### `sensor_manager.rs`
**Major architectural simplification:**

- **Removed**: `EventGenerator` dependency
- **Removed**: `SensorRuntime` dependency
- **Removed**: Poll-based sensor support entirely
- **Removed**: `start_poll_based_sensor()` method
- **Removed**: `poll_sensor()` method
- **Removed**: `run_loop()` polling loop
- **Kept**: Standalone sensor support with token provisioning
- **Simplified**: `SensorInstance` now only supports standalone mode
- **Updated**: Documentation to clarify standalone-only architecture

Key architectural changes:
```rust
// Before: Constructor needed event_generator
pub fn new(db: PgPool, event_generator: Arc<EventGenerator>) -> Self

// After: No event generation dependencies
pub fn new(db: PgPool) -> Self
```

The sensor manager now focuses on:
- Starting sensor processes when rules become active
- Stopping sensor processes when no rules need them
- Provisioning authentication tokens for sensor processes
- Monitoring sensor health via process lifecycle
- Logging sensor stdout/stderr

#### API Event Creation Fix

Also fixed a bug in `api/src/routes/events.rs` where the `create_event` endpoint wasn't parsing the `trigger_instance_id` parameter to set rule associations on events. Timer sensors send this parameter in format `"rule_{id}"`, and now the API properly:
1. Parses the rule ID from the string
2. Looks up the rule reference from the database
3. Sets `rule` and `rule_ref` fields on the event record

This ensures timer events (and other rule-specific events) properly show their associated rule in the web UI.

## Architecture After Cleanup

### Sensor Service Responsibilities

1. **Lifecycle Management**: Start/stop sensor processes based on active rules
2. **Token Provisioning**: Create API tokens for sensors to authenticate
3. **Process Monitoring**: Track sensor process health (stdout/stderr logging)
4. **Rule Synchronization**: Listen for rule lifecycle events and adjust sensors

### Sensor Process Responsibilities

1. **Event Generation**: Monitor for conditions and create events via API
2. **Configuration**: Read trigger instance configs and act accordingly
3. **Authentication**: Use provisioned tokens to call API endpoints
4. **Independence**: Run as standalone daemons, no coupling to sensor service

### Example: Timer Sensor

The `attune-core-timer-sensor` standalone process:
- Runs independently as a daemon
- Receives API token and configuration via environment variables
- Monitors database for timer trigger instances
- Creates events by calling `POST /api/v1/events` endpoint
- Passes `trigger_instance_id` to associate events with specific rules

## Testing

- ✅ All sensor service tests pass (17 passed, 3 ignored)
- ✅ Workspace compiles cleanly with no warnings
- ✅ No breaking changes to sensor process interface

## Impact

### For Sensor Developers

✅ **No change** - Sensors were already expected to be standalone processes that call the API

### For Sensor Service

✅ **Simplified** - Removed ~400 lines of poll-based sensor code  
✅ **Clearer responsibility** - Only manages process lifecycles, doesn't generate events  
✅ **Better separation of concerns** - Event generation is always through the API

### For Event Creation

✅ **Fixed bug** - Rule associations now properly set on timer events  
✅ **Web UI** - Events now display their associated rule correctly

## Migration Notes

**No migration needed** - The poll-based sensor functionality was never used in production. All current sensors (e.g., `attune-core-timer-sensor`) are already standalone processes.

## Files Changed

```
Deleted:
- crates/sensor/src/event_generator.rs
- crates/sensor/src/rule_matcher.rs
- crates/sensor/src/timer_manager.rs
- crates/sensor/src/sensor_runtime.rs

Modified:
- crates/sensor/src/lib.rs
- crates/sensor/src/service.rs
- crates/sensor/src/sensor_manager.rs
- crates/api/src/routes/events.rs
```

## Related Documentation

- `docs/architecture/sensor-service.md` - Should be updated to reflect standalone-only architecture
- `docs/architecture/webhook-system-architecture.md` - Webhook sensors follow same pattern
- `packs/core/sensors/interval_timer_sensor.yaml` - Example of standalone sensor config

## Future Considerations

1. **Sensor Discovery**: Consider auto-discovery of sensor binaries in pack directories
2. **Health Checks**: Add more sophisticated sensor health monitoring beyond process lifecycle
3. **Graceful Restart**: Implement graceful sensor restart on configuration changes
4. **Resource Limits**: Add CPU/memory limits for sensor processes
