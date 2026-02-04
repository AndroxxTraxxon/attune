# Timer Sensor Fix - Incorrect Entrypoint Binary

**Date:** 2025-01-30  
**Status:** Complete

## Problem

The timer sensor was generating a handful of events immediately after a rule was enabled, but then would halt completely. Investigation revealed that events were being created for a few seconds, then the sensor process would stop.

### Symptoms
- Initial burst of 3-4 events when rule enabled
- Sensor process would die immediately after
- No ongoing event generation
- Zombie process (`defunct`) visible in process list

### Example Timeline
```
08:18:00 - Events 19, 20, 21, 22 created
08:18:06 - Events 23, 24 created  
08:18:07 - Events 25, 26 created
08:18:07 - Sensor stopped (no more events)
```

## Root Cause

The sensor YAML configuration (`packs/core/sensors/interval_timer_sensor.yaml`) was referencing the wrong entrypoint:

```yaml
# INCORRECT - binary with different expectations
entry_point: attune-core-timer-sensor
```

The `attune-core-timer-sensor` binary was a compiled executable that expected different environment variables than what the sensor service provides. Specifically, it required `ATTUNE_API_URL` which was not being set:

```
Error: ATTUNE_API_URL environment variable is required

Caused by:
    environment variable not found
```

This caused the sensor process to exit immediately after startup, but not before emitting a few events that were already buffered.

## Solution

Updated the sensor entrypoint to use the Python implementation which is designed to work with the sensor service's protocol:

```yaml
# CORRECT - Python script compatible with sensor service
entry_point: interval_timer_sensor.py
```

### Files Changed
1. **Configuration**: `attune/packs/core/sensors/interval_timer_sensor.yaml`
   - Changed `entry_point` from `attune-core-timer-sensor` to `interval_timer_sensor.py`

2. **Database**: Updated sensor record directly
   ```sql
   UPDATE sensor 
   SET entrypoint = 'interval_timer_sensor.py' 
   WHERE ref = 'core.interval_timer_sensor';
   ```

## Verification

After the fix, the sensor operates correctly:

1. **Process Running**: Python sensor process visible and active
   ```bash
   $ ps aux | grep interval_timer_sensor.py
   python3 ./packs/core/sensors/interval_timer_sensor.py
   ```

2. **Consistent Event Generation**: Events created every second as configured
   ```
   08:20:57 - Event 32 created
   08:20:58 - Event 33 created
   08:20:59 - Event 34 created
   08:21:00 - Event 35 created
   08:21:01 - Event 36 created
   (continuing indefinitely...)
   ```

3. **No Process Crashes**: No defunct/zombie processes, sensor continues running

4. **Database Verification**: Confirmed ongoing event creation
   ```sql
   SELECT COUNT(*) FROM event WHERE created > NOW() - INTERVAL '1 minute';
   -- Result: 60 events (one per second for 1 minute)
   ```

## Technical Details

### Python Sensor vs Binary Sensor

**Python Sensor (`interval_timer_sensor.py`)**:
- Reads trigger instances from `ATTUNE_SENSOR_TRIGGERS` environment variable
- Emits events as JSON to stdout
- Manages state internally (last fired times, intervals)
- Compatible with sensor service's process management
- No additional environment variables required

**Binary Sensor (`attune-core-timer-sensor`)**:
- Unknown implementation details (compiled binary)
- Required `ATTUNE_API_URL` environment variable
- Not compatible with sensor service's current protocol
- Purpose/origin unclear - possibly from earlier development phase

### Sensor Service Protocol

The sensor service manages long-running sensors by:
1. Starting sensor process with trigger instances in `ATTUNE_SENSOR_TRIGGERS`
2. Reading JSON events from sensor's stdout
3. Parsing events and generating system events
4. Matching events against rules and creating enforcements
5. Monitoring process health and logging stderr

## Lessons Learned

1. **Entrypoint Validation**: Sensor entrypoints should be validated to ensure they're compatible with the sensor service protocol
2. **Error Handling**: Sensor process failures should be more visible - the sensor appeared to "work" initially because buffered events were emitted before the process died
3. **Documentation**: Binary entrypoints should be documented if they have special requirements
4. **Testing**: Integration tests should verify sensors continue running beyond initial startup

## Future Improvements

1. **Health Checks**: Add periodic health checks for long-running sensors
2. **Auto-Restart**: Automatically restart failed sensor processes
3. **Better Logging**: Surface sensor process failures more prominently
4. **Validation**: Validate sensor entrypoints during pack loading
5. **Remove Binary**: Consider removing or documenting the `attune-core-timer-sensor` binary if it's not needed

## Related Files

- `attune/packs/core/sensors/interval_timer_sensor.yaml` - Sensor configuration
- `attune/packs/core/sensors/interval_timer_sensor.py` - Python sensor implementation
- `attune/packs/core/sensors/attune-core-timer-sensor` - Binary (unused/incorrect)
- `attune/crates/sensor/src/sensor_manager.rs` - Sensor process management

## Status: ✅ RESOLVED

The timer sensor now runs continuously and generates events at the configured interval without stopping.
