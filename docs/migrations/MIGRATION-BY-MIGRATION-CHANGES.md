# Migration-by-Migration Change List

This document details exactly what needs to change in each migration file during consolidation.

## Files to Keep (with modifications)

### `20250101000001_initial_setup.sql`
**REMOVE**:
- `runtime_type_enum` type (lines ~42-46)

**KEEP**:
- All other enum types
- Extensions
- update_updated_column() function

### `20250101000002_core_tables.sql` → Rename to `20250101000003_pack_system.sql`
**MODIFY runtime table** (lines ~72-93):
```sql
CREATE TABLE runtime (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    pack BIGINT REFERENCES pack(id) ON DELETE CASCADE,
    pack_ref TEXT,
    description TEXT,
    -- REMOVE: runtime_type runtime_type_enum NOT NULL,
    name TEXT NOT NULL,
    distributions JSONB NOT NULL,
    installation JSONB,
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- REMOVE: CONSTRAINT runtime_ref_format CHECK (ref ~ '^[^.]+\.(action|sensor)\.[^.]+$')
    CONSTRAINT runtime_ref_lowercase CHECK (ref = LOWER(ref))
);
```

**REMOVE indexes**:
- `idx_runtime_type`
- `idx_runtime_pack_type`
- `idx_runtime_type_created`

**ADD indexes**:
- `idx_runtime_name` (added in unify migration)
- `idx_runtime_verification` GIN index (added in unify migration)

### `20250101000003_event_system.sql` → Rename to `20250101000005_trigger_event_rule.sql`
**MODIFY trigger table** (add webhook columns from start):
```sql
CREATE TABLE trigger (
    -- ... existing columns ...
    -- ADD FROM START:
    webhook_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    webhook_key VARCHAR(64) UNIQUE,
    webhook_config JSONB DEFAULT '{}'::jsonb,
    -- ... rest of columns ...
);
```

**MODIFY event table** (add rule tracking from start):
```sql
CREATE TABLE event (
    -- ... existing columns ...
    -- ADD FROM START:
    rule BIGINT,
    rule_ref TEXT,
    -- ... rest of columns ...
);

-- ADD constraint:
ALTER TABLE event
    ADD CONSTRAINT event_rule_fkey
    FOREIGN KEY (rule) REFERENCES rule(id) ON DELETE SET NULL;
```

**MODIFY rule table** (add is_adhoc from start):
```sql
CREATE TABLE rule (
    -- ... existing columns ...
    -- ADD FROM START:
    is_adhoc BOOLEAN DEFAULT false NOT NULL,
    -- ... rest of columns ...
);
```

### `20250101000004_execution_system.sql` → Rename to `20250101000006_execution_system.sql`
**MODIFY execution table** (add workflow columns from start):
```sql
CREATE TABLE execution (
    -- ... existing columns ...
    -- ADD FROM START:
    is_workflow BOOLEAN DEFAULT false NOT NULL,
    workflow_def BIGINT REFERENCES workflow_definition(id) ON DELETE CASCADE,
    workflow_task JSONB,
    -- ... rest of columns ...
);
```

**REMOVE**:
- `workflow_task_execution` table (lines ~329-360)
- Don't create it at all

### `20250101000005_supporting_tables.sql` → Rename to `20250101000008_worker_notification.sql`
**MODIFY worker table** (add role from start):

First, ensure `worker_role_enum` is created in `20250101000001_initial_setup.sql`:
```sql
CREATE TYPE worker_role_enum AS ENUM ('action', 'sensor', 'hybrid');
```

Then in worker table:
```sql
CREATE TABLE worker (
    -- ... existing columns ...
    -- ADD FROM START:
    worker_role worker_role_enum NOT NULL DEFAULT 'action',
    -- ... rest of columns ...
);
```

**MODIFY action table** (add is_adhoc from start):
```sql
CREATE TABLE action (
    -- ... existing columns ...
    -- ADD FROM START:
    is_adhoc BOOLEAN DEFAULT false NOT NULL,
    -- ... rest of columns ...
);
```

**MODIFY sensor table** (add is_adhoc from start):
```sql
CREATE TABLE sensor (
    -- ... existing columns ...
    -- ADD FROM START:
    is_adhoc BOOLEAN DEFAULT false NOT NULL,
    -- ... rest of columns ...
);
```

## Files to Remove Entirely

1. `20260119000001_add_execution_notify_trigger.sql` - Consolidate into notify triggers migration
2. `20260120000001_add_webhook_support.sql` - Columns added in trigger table from start
3. `20260120000002_webhook_advanced_features.sql` - Functions consolidated, columns already in trigger table
4. `20260127000001_consolidate_webhook_config.sql` - Already consolidated in base migration
5. `20260127212500_consolidate_workflow_task_execution.sql` - Already in base execution table
6. `20260129000001_fix_webhook_function_overload.sql` - Use fixed functions from start
7. `20260129140130_add_is_adhoc_flag.sql` - Already in base tables
8. `20260129150000_add_event_notify_trigger.sql` - Consolidate into notify triggers migration
9. `20260130000001_add_rule_to_event.sql` - Already in event table
10. `20260131000001_add_worker_role.sql` - Already in worker table
11. `20260202000001_add_sensor_runtimes.sql` - Data now in YAML files
12. `20260203000001_unify_runtimes.sql` - Changes already applied to base tables
13. `20260203000003_add_rule_trigger_to_execution_notify.sql` - Consolidate into notify triggers migration
14. `20260204000001_add_enforcement_notify_trigger.sql` - Consolidate into notify triggers migration
15. `20260204000001_restore_webhook_functions.sql` - Use final functions from start

## New Files to Create

### `20250101000010_webhook_system.sql`
- Webhook-related tables
- FINAL versions of webhook functions (from 20260204000001_restore_webhook_functions.sql)
- No individual webhook columns (use webhook_config JSONB)

### `20250101000011_pack_environments.sql`
```sql
-- From 20260203000002_add_pack_environments.sql
CREATE TYPE pack_environment_status_enum AS ENUM (...);

CREATE TABLE pack_environment (...);

ALTER TABLE pack ADD COLUMN IF NOT EXISTS installers JSONB DEFAULT '[]'::jsonb;
```

### `20250101000013_notify_triggers.sql`
Consolidate ALL LISTEN/NOTIFY triggers from:
- 20260119000001 - execution
- 20260129150000 - event  
- 20260203000003 - add rule to execution notify
- 20260204000001 - enforcement

Final notify_execution_change() function should include rule field from the start.

## Files to Keep As-Is

1. `20260120200000_add_pack_test_results.sql` → Rename to `20250101000012_pack_testing.sql`
2. `20260122000001_pack_installation_metadata.sql` → Merge into pack_system or keep separate

## Summary

**Original**: 22 migration files  
**Consolidated**: ~13 migration files  
**Removed**: 15 files (consolidation or data moved to YAML)  
**Modified**: 5 files (add columns/constraints from start)  
**New**: 3 files (consolidated functionality)
