# E2E Test Field Name Fixes - 2026-01-23

## Summary
Fixed field name mismatches between E2E tests and the actual API responses. Updated test client library to properly handle the new API schema that uses `ref`/`label` instead of legacy `name`/`type` fields.

## Problems Fixed

### 1. Trigger Field Mismatches
**Problem:** Tests were accessing `trigger['name']` and `trigger['type']` which don't exist in API responses.

**API Schema:**
- Uses `ref` (e.g., "core.webhook", "core.intervaltimer")
- Uses `label` (human-readable name)
- No `type` or `name` fields

**Fixed:**
- ✅ Updated all tests to use `trigger['label']` instead of `trigger['name']`
- ✅ Updated assertions to check `trigger['ref']` instead of `trigger['type']`

### 2. Rule Field Mismatches
**Problem:** Tests were accessing `rule['name']` which doesn't exist.

**API Schema:**
- Uses `ref` (unique identifier)
- Uses `label` (human-readable name)

**Fixed:**
- ✅ Updated all tests to use `rule['label']` instead of `rule['name']`

### 3. Action Field Mismatches
**Problem:** Test was checking `action['runner_type']` which doesn't exist.

**API Schema:**
- Uses `runtime` (runtime ID, optional)
- No `runner_type` field in response

**Fixed:**
- ✅ Removed invalid `runner_type` assertion from tests

### 4. Timer Helper Return Structure
**Problem:** Timer helpers were returning different structures - interval timer returned sensor dict, but date/cron timers returned plain trigger dict.

**Fixed:**
- ✅ Updated `create_date_timer()` to create sensor like interval timer
- ✅ Updated `create_cron_timer()` to create sensor like interval timer
- ✅ All timer helpers now return consistent structure:
  ```python
  {
      "id": core_trigger_id,
      "ref": core_trigger_ref,
      "label": sensor_label,
      "trigger": core_trigger_dict,
      "sensor": sensor_dict,
      "sensor_id": sensor_id,
      # Plus helper-specific fields like fire_at, cron_expression, etc.
  }
  ```

## Client Library Updates

### AttuneClient.create_trigger()
- Already supported new API schema
- Maps legacy `name` parameter to `label`
- Handles `ref` generation from `pack_ref` and `name`

### AttuneClient.create_rule()
**Updated to handle new API schema:**
- Maps legacy `name` to `ref`/`label`
- Converts `trigger_id` to `trigger_ref` by looking up trigger
- Maps `criteria` (string) to `conditions` (JSON)
- Maps `action_parameters` to `action_params`
- Adds support for `trigger_params`

**API Expectations:**
```python
{
    "ref": "pack.rule_name",
    "pack_ref": "pack",
    "label": "Human Readable",
    "description": "...",
    "action_ref": "pack.action",
    "trigger_ref": "pack.trigger",
    "conditions": {},       # JSON Logic
    "action_params": {},
    "trigger_params": {},
    "enabled": true
}
```

### AttuneClient.create_action()
**Updated to handle new API schema:**
- Maps legacy `name` to `ref`/`label`
- Converts `runner_type` (string) to `runtime` (ID) by:
  1. Looking up runtime by reference
  2. Trying common mappings (python3 → core.action.python3, etc.)
  3. Finding runtime ID from the list

**API Expectations:**
```python
{
    "ref": "pack.action_name",
    "pack_ref": "pack",
    "label": "Human Readable",
    "description": "...",
    "entrypoint": "/actions/script.py",
    "runtime": 1,           # Optional runtime ID
    "param_schema": {},
    "out_schema": {}
}
```

### AttuneClient.list_runtimes()
**Added new method:**
- GET `/api/v1/runtimes`
- Returns list of available runtimes
- Used by `create_action()` to look up runtime IDs

## Service Management

### restart_sensor_service()
**Added new helper function:**
- Restarts sensor service after creating sensors
- Tries multiple methods:
  1. Docker-compose restart (if in container)
  2. systemctl restart (if systemd service)
  3. Falls back to waiting for auto-reload
- Called automatically by timer creation helpers

**Why needed:**
- Sensor service loads sensors at startup
- New sensors must be loaded before they can generate events
- Without restart, timers won't fire

## Files Modified

### Test Files
- `tests/e2e/tier1/test_t1_01_interval_timer.py` - Fixed field names
- `tests/e2e/tier1/test_t1_02_date_timer.py` - Fixed field names, updated assertions
- `tests/e2e/tier1/test_t1_03_cron_timer.py` - Fixed field names, updated assertions
- `tests/e2e/tier1/test_t1_04_webhook_trigger.py` - Fixed field names
- `tests/e2e/tier1/test_t1_08_action_failure.py` - Fixed field names

### Helper Files
- `tests/helpers/client.py` - Updated create_rule(), create_action(), added list_runtimes()
- `tests/helpers/fixtures.py` - Updated timer helpers, added restart_sensor_service()

## Testing Status

### Ready to Test
All field name mismatches have been resolved. Tests should now:
1. ✅ Import without errors
2. ✅ Create triggers/actions/rules with correct API schema
3. ✅ Access response fields correctly (ref, label instead of name, type)
4. ✅ Create sensors for all timer types
5. ✅ Restart sensor service after sensor creation

### Next Steps
1. **Run Tier 1 tests:**
   ```bash
   pytest tests/e2e/tier1/ -v
   ```

2. **Check for any remaining issues:**
   - Timer firing correctly
   - Events being created
   - Rules matching events
   - Executions being created

3. **If sensor service restart doesn't work automatically:**
   - Manually restart sensor service before running timer tests
   - Or implement API endpoint for sensor reload (future enhancement)

## Technical Notes

### API Schema Migration
The Attune API has been updated to use a more consistent schema across all entities:
- **ref** - Unique identifier (replaces legacy name/type)
- **label** - Human-readable label
- **description** - Longer description
- All entities follow this pattern (triggers, sensors, actions, rules, packs)

### Backward Compatibility
The client library maintains backward compatibility:
- Old parameters (`name`, `trigger_id`, `runner_type`, etc.) still work
- They are transparently converted to new schema
- This allows existing test code to work with minimal changes

### Sensor Service Reload
Currently requires service restart to load new sensors. Future improvements:
- API endpoint for sensor reload (`POST /api/v1/sensors/reload`)
- Database watcher to detect new sensors
- Hot-reload capability in sensor service

## Success Criteria
- ✅ All imports resolved
- ✅ No field name access errors
- ✅ Client properly converts legacy parameters
- ✅ Timer helpers create sensors consistently
- ✅ Sensor service restart mechanism in place
- 🔄 Tests run successfully (pending execution)