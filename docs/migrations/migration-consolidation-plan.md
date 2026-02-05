# Migration Consolidation Plan

**Status**: Pre-production consolidation  
**Date**: 2026-02-04  
**Goal**: Consolidate migrations into a clean, minimal set before initial release

## Background

Since this project has no production deployments, we can freely consolidate migrations to create a cleaner initial state. This document identifies items that are created and then dropped/modified, so we can simplify the migration history.

## Issues Identified

### 1. Runtime Type Enum - Created Then Dropped

**Problem**: `runtime_type_enum` is created in the initial migration but dropped in a later migration.

- **Created**: `20250101000001_initial_setup.sql` (line 42)
- **Dropped**: `20260203000001_unify_runtimes.sql` (line 35)
- **Associated column**: `runtime.runtime_type` (also dropped)
- **Associated indexes**: 
  - `idx_runtime_type`
  - `idx_runtime_pack_type` 
  - `idx_runtime_type_created`
  - `idx_runtime_type_sensor`

**Action**: Remove enum type, column, and indexes from initial creation.

### 2. Runtime Table Constraints - Created Then Dropped

**Problem**: Runtime constraints are created with one format, then dropped and not recreated.

- **Created**: `20250101000002_core_tables.sql` (line 84)
  - `runtime_ref_format CHECK (ref ~ '^[^.]+\.(action|sensor)\.[^.]+$')`
  - Expected format: `pack.type.name`
- **Dropped**: `20260203000001_unify_runtimes.sql` (line 16)
- **New format**: `pack.name` (e.g., `core.python` instead of `core.action.python`)

**Action**: Create constraint with final format initially, or omit if not needed.

### 3. Webhook Columns - Added Then Consolidated

**Problem**: Individual webhook columns are added, then dropped in favor of a JSONB column.

**Added in `20260120000001_add_webhook_support.sql`**:
- `webhook_enabled BOOLEAN`
- `webhook_key VARCHAR(64)`
- `webhook_secret VARCHAR(128)`

**Added in `20260120000002_webhook_advanced_features.sql`**:
- `webhook_hmac_enabled BOOLEAN`
- `webhook_hmac_secret VARCHAR(128)`
- `webhook_hmac_algorithm VARCHAR(32)`
- `webhook_rate_limit_enabled BOOLEAN`
- `webhook_rate_limit_requests INTEGER`
- `webhook_rate_limit_window_seconds INTEGER`
- `webhook_ip_whitelist_enabled BOOLEAN`
- `webhook_ip_whitelist JSONB`
- `webhook_payload_size_limit_kb INTEGER`

**Consolidated in `20260127000001_consolidate_webhook_config.sql`**:
- All individual columns dropped
- Single `webhook_config JSONB` column added

**Action**: Add only `webhook_enabled`, `webhook_key`, and `webhook_config` in initial trigger table creation. Skip intermediate columns.

### 4. Runtime Data Insertions - Later Truncated

**Problem**: Runtime records are inserted via SQL, then truncated and moved to YAML files.

**Insertions in `20260202000001_add_sensor_runtimes.sql`**:
- 4 INSERT statements for sensor runtimes
- All records truncated in `20260203000001_unify_runtimes.sql`

**Insertions elsewhere**: Check if initial migrations insert any runtime data.

**Action**: Remove all runtime INSERT statements. Runtime data now loaded from YAML files in `packs/core/runtimes/`.

### 5. Workflow Task Execution Table - Created Then Dropped

**Problem**: Separate table created, then consolidated into execution table JSONB column.

- **Created**: `20250101000004_execution_system.sql` (line 329)
  - `workflow_task_execution` table with multiple columns
- **Consolidated**: `20260127212500_consolidate_workflow_task_execution.sql`
  - Table dropped
  - `execution.workflow_task JSONB` column added instead

**Action**: Don't create `workflow_task_execution` table. Add `workflow_task JSONB` column to `execution` table in initial creation.

### 6. Execution Table Columns - Added for Workflows

**Problem**: Workflow-related columns added after initial table creation.

**Added in `20250101000004_execution_system.sql` (line 381)**:
- `is_workflow BOOLEAN DEFAULT false NOT NULL`
- `workflow_def BIGINT REFERENCES workflow_definition(id)`

**Action**: Include these columns in initial `execution` table creation (line ~60).

### 7. Is Adhoc Flag - Added Later

**Problem**: `is_adhoc` flag added to multiple tables after initial creation.

**Added in `20260129140130_add_is_adhoc_flag.sql`**:
- `action.is_adhoc`
- `sensor.is_adhoc`
- `rule.is_adhoc`

**Action**: Include `is_adhoc BOOLEAN DEFAULT false NOT NULL` in initial table definitions.

### 8. Event Table - Rule Reference Added Later

**Problem**: Rule tracking added to event table after initial creation.

**Added in `20260130000001_add_rule_to_event.sql`**:
- `event.rule BIGINT`
- `event.rule_ref TEXT`
- Foreign key constraint

**Action**: Include rule columns and constraint in initial event table creation.

### 9. Worker Role Column - Added Later

**Problem**: Worker role enum and column added after initial creation.

**Added in `20260131000001_add_worker_role.sql`**:
- `worker_role_enum` type
- `worker.worker_role` column

**Action**: Include enum type and column in initial worker table creation.

### 10. Pack Environments - Added Later

**Problem**: Pack installers column added after initial creation.

**Added in `20260203000002_add_pack_environments.sql`**:
- `pack_environment_status_enum` type
- `pack.installers JSONB` column
- `pack_environment` table

**Action**: Include in initial pack/environment setup.

### 11. Notify Triggers - Added Incrementally

**Problem**: PostgreSQL LISTEN/NOTIFY triggers added across multiple migrations.

**Migrations**:
- `20260119000001_add_execution_notify_trigger.sql` - execution events
- `20260129150000_add_event_notify_trigger.sql` - event creation
- `20260203000003_add_rule_trigger_to_execution_notify.sql` - add rule to execution notify
- `20260204000001_add_enforcement_notify_trigger.sql` - enforcement events

**Action**: Create all notify triggers in a single migration after table creation.

### 12. Webhook Functions - Created, Modified, Dropped, Restored

**Problem**: Webhook validation/processing functions have been rewritten multiple times.

**Timeline**:
- `20260120000001_add_webhook_support.sql` - Initial functions (4 created)
- `20260120000002_webhook_advanced_features.sql` - Advanced functions (7 created)
- `20260127000001_consolidate_webhook_config.sql` - Modified (2 dropped, 3 created)
- `20260129000001_fix_webhook_function_overload.sql` - Fixed overloading (3 dropped)
- `20260204000001_restore_webhook_functions.sql` - Restored (4 dropped, 3 created)

**Action**: Determine final set of webhook functions needed and create them once.

## Consolidation Strategy

### Phase 1: Analyze Dependencies

1. Map all foreign key relationships
2. Identify minimum viable table set
3. Document final schema for each table

### Phase 2: Create New Base Migrations

Create consolidated migrations:

1. **`00001_initial_setup.sql`** - Enums, extensions, base types
2. **`00002_identity_and_auth.sql`** - Identity, keys, auth tables
3. **`00003_pack_system.sql`** - Pack, runtime, action, sensor tables (with final schema)
4. **`00004_event_system.sql`** - Trigger, sensor, event, rule tables
5. **`00005_execution_system.sql`** - Execution, enforcement, inquiry, policy tables (including workflow columns)
6. **`00006_supporting_tables.sql`** - Worker, notification, artifact, etc.
7. **`00007_webhook_system.sql`** - Webhook tables, triggers, functions (final versions)
8. **`00008_notify_triggers.sql`** - All LISTEN/NOTIFY triggers
9. **`00009_pack_testing.sql`** - Pack test results table

### Phase 3: Validation

1. Test migrations on fresh database
2. Compare final schema to current production-like schema
3. Verify all indexes, constraints, triggers present
4. Load core pack and verify runtime data loads correctly

### Phase 4: Documentation

1. Update migration README
2. Document schema version
3. Add migration best practices

## Items to Remove Entirely

**Never created in consolidated migrations**:

1. `runtime_type_enum` type
2. `runtime.runtime_type` column
3. `runtime_ref_format` constraint (old format)
4. Indexes: `idx_runtime_type`, `idx_runtime_pack_type`, `idx_runtime_type_created`, `idx_runtime_type_sensor`
5. Individual webhook columns (9 columns that were later consolidated)
6. `idx_trigger_webhook_enabled` index
7. `workflow_task_execution` table
8. All runtime INSERT statements
9. Intermediate webhook function versions

## Items to Include From Start

**Must be in initial table creation**:

1. `execution.is_workflow` column
2. `execution.workflow_def` column
3. `execution.workflow_task` JSONB column
4. `action.is_adhoc` column
5. `sensor.is_adhoc` column  
6. `rule.is_adhoc` column
7. `event.rule` and `event.rule_ref` columns
8. `worker_role_enum` type
9. `worker.worker_role` column
10. `trigger.webhook_enabled` column
11. `trigger.webhook_key` column
12. `trigger.webhook_config` JSONB column
13. `pack.installers` JSONB column
14. `pack_environment` table and `pack_environment_status_enum`

## Data Migration Notes

**Runtime Data**: 
- Remove all INSERT statements from migrations
- Runtime records loaded from YAML files in `packs/core/runtimes/`
- Loader: `scripts/load_core_pack.py` or pack installation system

**Core Pack Data**:
- Check if any other core pack data is inserted via migrations
- Move to appropriate YAML files in `packs/core/`

## Next Steps

1. ✅ Create this consolidation plan
2. ⏳ Review with team
3. ⏳ Back up current migration directory
4. ⏳ Create consolidated migrations
5. ⏳ Test on fresh database
6. ⏳ Verify schema matches current state
7. ⏳ Replace old migrations
8. ⏳ Update documentation

## Rollback Plan

Keep copy of old migrations in `migrations.old/` directory until consolidated migrations are verified in development environment.