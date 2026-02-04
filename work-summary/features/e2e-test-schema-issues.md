# E2E Test Schema Mismatch Issues

**Date:** 2026-01-23  
**Status:** 🔴 BLOCKING - E2E tests cannot run due to API schema mismatches  
**Priority:** P0 - Must fix before tests can be used

---

## Overview

The E2E test suite was written based on an expected/older API schema, but the actual Attune API implementation uses different field names and structures. This causes widespread test failures across all tiers.

**Root Cause:** Tests were developed before/alongside the API, and the schemas diverged during implementation.

---

## Issues Discovered

### 1. Pack Registration Endpoint ✅ FIXED

**Problem:**
- Tests called `client.register_pack(pack_dir)` 
- Method sent to `POST /api/v1/packs` (wrong endpoint)
- Actual endpoint is `POST /api/v1/packs/register`

**API Schema:**
```json
{
  "path": "/path/to/pack",
  "skip_tests": true,
  "force": false
}
```

**Fix Applied:**
- Updated `client.register_pack()` to use `/api/v1/packs/register`
- Added `skip_tests` (default: `True`) and `force` (default: `False`) parameters
- Updated `create_test_pack()` to get existing pack before registering

**Status:** ✅ RESOLVED

---

### 2. Trigger Field Names ✅ FIXED (partially)

**Problem:**
- Tests expect: `name`, `type`, `parameters`
- API expects: `ref`, `label`, `description`, `param_schema`, `out_schema`, `enabled`

**Expected by Tests:**
```python
{
  "name": "my_timer",
  "type": "interval_timer",
  "parameters": {"interval_seconds": 5}
}
```

**Actual API Schema (CreateTriggerRequest):**
```json
{
  "ref": "pack.trigger_name",
  "pack_ref": "pack",
  "label": "My Timer",
  "description": "Timer description",
  "param_schema": {...},
  "out_schema": {...},
  "enabled": true
}
```

**API Response:**
```json
{
  "id": 34,
  "ref": "test_pack.interval_5s_12345",
  "label": "interval_5s_12345",
  "pack": 11,
  "pack_ref": "test_pack",
  "enabled": true,
  "webhook_enabled": false,
  ...
}
```

**Fix Applied:**
- Updated `client.create_trigger()` to accept both legacy and new parameters
- Maps `name` → `label`
- Generates `ref` from `pack_ref.name` if not provided
- Ignores `trigger_type` and `parameters` (not used by API)

**Remaining Issues:**
- Tests still reference `trigger['name']` in assertions
- Tests expect timer configuration in `parameters` field
- **Timer triggers don't actually store interval/cron/date config in trigger table**

**Status:** ⚠️ PARTIAL - Client fixed, tests need updates

---

### 3. Timer Architecture Misunderstanding 🔴 CRITICAL

**Problem:**
Tests assume timers work like this:
```
Trigger (with timer config) → Rule → Action
```

Actual Attune architecture:
```
Trigger (event type) ← Sensor (monitors & fires) → Event → Rule → Action
```

**Implications:**
- Creating a trigger alone doesn't create a timer
- Need to create **both** trigger AND sensor for timers to work
- Sensor contains the actual timer configuration (interval_seconds, cron expression, etc.)
- Tests don't create sensors at all

**Example:**
```python
# What tests do:
trigger = client.create_trigger(
    name="interval_timer",
    type="interval_timer",
    parameters={"interval_seconds": 5}
)
# ❌ This creates a trigger but NO sensor → timer never fires

# What's actually needed:
trigger = client.create_trigger(ref="pack.timer", label="Timer")
sensor = client.create_sensor(
    trigger_id=trigger["id"],
    entrypoint="sensors/timer.py",
    runtime="python3",
    config={"interval_seconds": 5}
)
# ✅ Now the sensor will fire events every 5 seconds
```

**Status:** 🔴 BLOCKING - Tests cannot work without sensor creation

---

### 4. Action Field Names 🔴 NEEDS