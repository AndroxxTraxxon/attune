# Trigger and Sensor Architecture

## Overview

Attune uses a two-level architecture for event detection:
- **Triggers** define event types (templates/schemas)
- **Sensors** are configured instances that monitor for those events

This architecture was introduced in migration `20240103000002_restructure_timer_triggers.sql`.

---

## Key Concepts

### Trigger (Event Type Definition)
A **trigger** is a generic event type definition that specifies:
- What parameters are needed to configure monitoring (`param_schema`)
- What data will be in the event payload when it fires (`out_schema`)

**Example:** `core.intervaltimer` is a trigger type that defines how interval-based timers work.

### Sensor (Configured Instance)
A **sensor** is a specific instance of a trigger with actual configuration values:
- References a trigger type
- Provides concrete configuration values (conforming to the trigger's `param_schema`)
- Actually monitors and fires events

**Example:** `core.timer_10s_sensor` is a sensor instance configured to fire `core.intervaltimer` every 10 seconds.

### Rule (Event Handler)
A **rule** connects a trigger type to an action:
- References the trigger type (not the sensor instance)
- Specifies which action to execute when the trigger fires
- Can include parameter mappings from event payload to action parameters

---

## Architecture Flow

```
Sensor Instance (with config)
    ↓ monitors and detects
Trigger Type (fires event)
    ↓ evaluated by
Rule (matches trigger type)
    ↓ creates
Enforcement (rule activation)
    ↓ schedules
Execution (action run)
```

---

## Core Timer Triggers

The core pack provides three generic timer trigger types:

### 1. Interval Timer (`core.intervaltimer`)
Fires at regular intervals.

**Param Schema:**
```json
{
  "unit": "seconds|minutes|hours",
  "interval": <integer>
}
```

**Example Sensor Config:**
```json
{
  "unit": "seconds",
  "interval": 10
}
```

**Event Payload:**
```json
{
  "type": "interval",
  "interval_seconds": 10,
  "fired_at": "2026-01-17T15:30:00Z"
}
```

### 2. Cron Timer (`core.crontimer`)
Fires based on cron schedule expressions.

**Param Schema:**
```json
{
  "expression": "<cron expression>"
}
```

**Example Sensor Config:**
```json
{
  "expression": "0 0 * * * *"
}
```

**Event Payload:**
```json
{
  "type": "cron",
  "fired_at": "2026-01-17T15:00:00Z",
  "scheduled_at": "2026-01-17T15:00:00Z"
}
```

### 3. Datetime Timer (`core.datetimetimer`)
Fires once at a specific date and time.

**Param Schema:**
```json
{
  "fire_at": "<ISO 8601 timestamp>"
}
```

**Example Sensor Config:**
```json
{
  "fire_at": "2026-12-31T23:59:59Z"
}
```

**Event Payload:**
```json
{
  "type": "one_shot",
  "fire_at": "2026-12-31T23:59:59Z",
  "fired_at": "2026-12-31T23:59:59Z"
}
```

---

## Creating a Complete Example

### Step 1: Trigger Type Already Exists
The `core.intervaltimer` trigger type is created by the seed script.

### Step 2: Create a Sensor Instance
```sql
INSERT INTO attune.sensor (
    ref, pack, pack_ref, label, description,
    entrypoint, runtime, runtime_ref,
    trigger, trigger_ref, enabled, config
)
VALUES (
    'mypack.every_30s_sensor',
    <pack_id>,
    'mypack',
    '30 Second Timer',
    'Fires every 30 seconds',
    'builtin:interval_timer',
    <sensor_runtime_id>,
    'core.sensor.builtin',
    <intervaltimer_trigger_id>,
    'core.intervaltimer',
    true,
    '{"unit": "seconds", "interval": 30}'::jsonb
);
```

### Step 3: Create a Rule
```sql
INSERT INTO attune.rule (
    ref, pack, pack_ref, label, description,
    action, action_ref,
    trigger, trigger_ref,
    conditions, action_params, enabled
)
VALUES (
    'mypack.my_rule',
    <pack_id>,
    'mypack',
    'My Rule',
    'Does something every 30 seconds',
    <action_id>,
    'mypack.my_action',
    <intervaltimer_trigger_id>,  -- References the trigger type, not the sensor
    'core.intervaltimer',
    '{}'::jsonb,
    '{"message": "Timer fired!"}'::jsonb,
    true
);
```

**Important:** The rule references the trigger type (`core.intervaltimer`), not the specific sensor instance. Any sensor that fires `core.intervaltimer` events will match this rule.

---

## Why This Architecture?

### Advantages
1. **Reusability:** One trigger type, many sensor instances with different configs
2. **Flexibility:** Multiple sensors can fire the same trigger type
3. **Separation of Concerns:** 
   - Triggers define what events look like
   - Sensors handle how to detect them
   - Rules define what to do when they occur
4. **Consistency:** All events of a type have the same payload schema

### Example Use Cases
- **Multiple timers:** Create multiple sensor instances with different intervals, all using `core.intervaltimer`
- **Webhook triggers:** One webhook trigger type, multiple sensor instances for different endpoints
- **File watchers:** One file change trigger type, multiple sensors watching different directories

---

## Migration from Old Architecture

The old architecture had specific triggers like `core.timer_10s`, `core.timer_1m`, etc. These were removed in migration `20240103000002` and replaced with:
- Generic trigger types: `core.intervaltimer`, `core.crontimer`, `core.datetimetimer`
- Sensor instances: `core.timer_10s_sensor`, etc., configured to use the generic types

If you have old rules referencing specific timer triggers, you'll need to:
1. Update the rule to reference the appropriate generic trigger type
2. Ensure a sensor instance exists with the desired configuration

---

## Database Schema

### Trigger Table
```sql
CREATE TABLE attune.trigger (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES attune.pack(id),
    pack_ref TEXT NOT NULL,
    label TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN DEFAULT true,
    param_schema JSONB NOT NULL,  -- Schema for sensor config
    out_schema JSONB NOT NULL      -- Schema for event payloads
);
```

### Sensor Table
```sql
CREATE TABLE attune.sensor (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES attune.pack(id),
    pack_ref TEXT NOT NULL,
    trigger BIGINT REFERENCES attune.trigger(id),
    trigger_ref TEXT NOT NULL,
    runtime BIGINT REFERENCES attune.runtime(id),
    runtime_ref TEXT NOT NULL,
    config JSONB NOT NULL,         -- Actual config values
    enabled BOOLEAN DEFAULT true
);
```

### Rule Table
```sql
CREATE TABLE attune.rule (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES attune.pack(id),
    pack_ref TEXT NOT NULL,
    trigger BIGINT REFERENCES attune.trigger(id),
    trigger_ref TEXT NOT NULL,     -- References trigger type
    action BIGINT REFERENCES attune.action(id),
    action_ref TEXT NOT NULL,
    action_params JSONB,
    enabled BOOLEAN DEFAULT true
);
```

---

## See Also
- `migrations/20240103000002_restructure_timer_triggers.sql` - Migration that introduced this architecture
- `scripts/seed_core_pack.sql` - Seeds the core trigger types and example sensors
- `docs/examples/rule-parameter-examples.md` - Examples of rules using triggers