# Timer Sensor Entrypoint Fix

**Date**: 2026-02-04  
**Issue**: Timer sensor not creating events after re-enabling rule  
**Status**: ✅ Fixed and Verified

## Problem Description

After re-enabling a timer rule, the sensor service appeared to be running and firing timers (based on JSON output in logs), but no events were being created in the database. The executor was not processing any timer-based rules.

## Root Cause Analysis

### Investigation Steps

1. **Sensor Status**: Sensor service was running but marked as "unhealthy"
2. **Log Analysis**: Sensor was outputting JSON like:
   ```json
   {"type":"interval","interval_seconds":1,"fired_at":"2026-02-04T04:43:41.020994946+00:00",
    "execution_count":154,"sensor_ref":"core.interval_timer_sensor",
    "trigger_instance_id":1,"trigger_ref":"default.echo_every_second"}
   ```
3. **Event Verification**: No events with `trigger_ref='core.intervaltimer'` were being created
4. **Binary Investigation**: Discovered two timer sensor binaries existed:
   - `attune-core-timer-sensor` (new version - creates events via API)
   - `attune-timer-sensor` (old version - only logs to stdout)

### Root Cause

The database had the **wrong entrypoint** stored for the timer sensor:
- **YAML file** specified: `entry_point: attune-core-timer-sensor`
- **Database** contained: `entrypoint: 'attune-timer-sensor'`

The sensor manager was launching the old binary (`attune-timer-sensor`) which only output JSON to stdout and did not make API calls to create events. This old binary was outputting informational JSON but was not integrated with the event system.

## Solution

1. **Updated database** to use the correct entrypoint:
   ```sql
   UPDATE public.sensor 
   SET entrypoint='attune-core-timer-sensor' 
   WHERE ref='core.interval_timer_sensor';
   ```

2. **Rebuilt sensor Docker image** to ensure the latest binary was included:
   ```bash
   docker compose build sensor
   ```

3. **Restarted sensor service** to pick up changes:
   ```bash
   docker compose restart sensor
   ```

## Verification

After the fix, the correct binary (`attune-core-timer-sensor`) was launched and immediately started creating events:

### New Log Output (Correct Binary)
```
{"timestamp":"2026-02-04T04:46:57.925938Z","level":"INFO",
 "fields":{"message":"Event created successfully: id=8598, trigger_ref=core.intervaltimer"},
 "threadId":"ThreadId(5)"}
{"timestamp":"2026-02-04T04:46:57.926023Z","level":"INFO",
 "fields":{"message":"Timer fired for rule 1, created event 8598"},
 "threadId":"ThreadId(5)"}
```

### Database Verification
```sql
SELECT id, trigger_ref, rule, created 
FROM public.event 
WHERE trigger_ref='core.intervaltimer' 
ORDER BY created DESC LIMIT 5;

  id  |    trigger_ref     | rule |            created
------+--------------------+------+-------------------------------
 8610 | core.intervaltimer |    1 | 2026-02-04 04:47:10.161621+00
 8609 | core.intervaltimer |    1 | 2026-02-04 04:47:09.137786+00
 8608 | core.intervaltimer |    1 | 2026-02-04 04:47:08.123983+00
```

✅ Events are now being created every second as expected!

## Key Differences Between Binaries

### `attune-timer-sensor` (Old/Incorrect)
- Only outputs JSON to stdout
- Does not create events via API
- Used for debugging/logging only
- Not integrated with the Attune event system

### `attune-core-timer-sensor` (New/Correct)
- Full-featured sensor daemon
- Creates events via REST API
- Connects to RabbitMQ for rule lifecycle events
- Implements token refresh
- Properly integrated with Attune platform

## Impact

- ✅ Timer rules now work correctly when enabled/re-enabled
- ✅ Events are created every interval as configured
- ✅ Executor receives events and creates enforcements
- ✅ Full timer-based automation flow is operational

## Files Changed

- **Database**: Updated `sensor.entrypoint` field for `core.interval_timer_sensor`
- **Docker**: Rebuilt sensor image to ensure latest binary

## Related Components

- **Sensor Service**: `attune/crates/sensor/`
- **Timer Sensor Binary**: `attune/crates/sensor-timer/`
- **Pack Definition**: `attune/packs/core/sensors/interval_timer_sensor.yaml`
- **Database Table**: `public.sensor`

## Lessons Learned

1. **Pack loading matters**: The pack was likely loaded with an old version of the YAML that referenced the old binary name
2. **Database vs. File divergence**: YAML files can become out of sync with database state
3. **Binary naming conventions**: Having two similar binaries (`attune-timer-sensor` vs `attune-core-timer-sensor`) caused confusion
4. **Testing after pack updates**: Always verify that sensors are using the correct entrypoints after pack reloads

## Prevention

To prevent this issue in the future:

1. **Pack reload script**: When reloading packs, verify sensor entrypoints are correct
2. **Remove old binaries**: Delete `attune-timer-sensor` to avoid confusion
3. **Add validation**: Check that sensor entrypoint files exist before starting
4. **Health checks**: Improve sensor health checks to detect non-functioning sensors

## Commands for Future Reference

### Check sensor entrypoint in database
```sql
SELECT id, ref, entrypoint 
FROM public.sensor 
WHERE ref='core.interval_timer_sensor';
```

### Update sensor entrypoint
```sql
UPDATE public.sensor 
SET entrypoint='attune-core-timer-sensor' 
WHERE ref='core.interval_timer_sensor';
```

### Verify timer events are being created
```sql
SELECT id, trigger_ref, rule, created 
FROM public.event 
WHERE trigger_ref='core.intervaltimer' 
ORDER BY created DESC LIMIT 10;
```

### Rebuild and restart sensor service
```bash
docker compose build sensor
docker compose restart sensor
docker compose logs sensor --tail=50
```
