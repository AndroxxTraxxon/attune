# Migration Consolidation - Executive Summary

**Date**: 2026-02-04  
**Status**: Pre-production - Safe to consolidate  
**Impact**: No production deployments exist

## Overview

The Attune project has accumulated 22 migrations during active development. Since there are no production deployments, we can safely consolidate these into a clean initial state, removing items that were created and then dropped or modified.

## Key Findings

### Items Created Then Dropped (Remove Entirely)

1. **`runtime_type_enum`** - Created in 00001, dropped in 20260203000001
   - Associated column: `runtime.runtime_type`
   - Associated indexes: 4 indexes referencing this column
   - **Action**: Don't create at all

2. **`workflow_task_execution` table** - Created in 00004, dropped in 20260127212500
   - Consolidated into `execution.workflow_task JSONB` column
   - **Action**: Don't create table, add JSONB column to execution from start

3. **Individual webhook columns (10 columns)** - Added in 20260120000001/000002, dropped in 20260127000001
   - `webhook_secret`, `webhook_hmac_enabled`, `webhook_hmac_secret`, etc.
   - Consolidated into single `webhook_config JSONB`
   - **Action**: Only create `webhook_enabled`, `webhook_key`, `webhook_config` from start

4. **Runtime INSERT statements** - Added in 20260202000001, truncated in 20260203000001
   - Now loaded from YAML files in `packs/core/runtimes/`
   - **Action**: Remove all runtime data from migrations

### Items Added Later (Include From Start)

1. **Execution table workflow columns**:
   - `is_workflow BOOLEAN` (added later)
   - `workflow_def BIGINT` (added later)
   - `workflow_task JSONB` (added in consolidation migration)

2. **Is adhoc flags** (added in 20260129140130):
   - `action.is_adhoc`
   - `sensor.is_adhoc`
   - `rule.is_adhoc`

3. **Event table rule tracking** (added in 20260130000001):
   - `event.rule BIGINT`
   - `event.rule_ref TEXT`

4. **Worker role** (added in 20260131000001):
   - `worker_role_enum` type
   - `worker.worker_role` column

5. **Pack environments** (added in 20260203000002):
   - `pack_environment_status_enum` type
   - `pack.installers JSONB` column
   - `pack_environment` table

6. **LISTEN/NOTIFY triggers** (added across 4 migrations):
   - Execution notify
   - Event notify
   - Enforcement notify
   - Consolidated into single migration

### Constraints Modified

1. **`runtime_ref_format`** - Original: `^[^.]+\.(action|sensor)\.[^.]+$`
   - Expected format was `pack.type.name` (e.g., `core.action.python`)
   - Changed to allow `pack.name` format (e.g., `core.python`)
   - **Action**: Drop constraint entirely or create with final format

2. **`runtime_ref_lowercase`** - Dropped and not recreated
   - **Action**: Determine if needed in final schema

## Recommended Consolidation Structure

```
migrations/
├── 20250101000001_initial_setup.sql           # Enums, extensions (FINAL VERSIONS)
├── 20250101000002_identity_and_auth.sql       # Identity, keys
├── 20250101000003_pack_system.sql             # Pack, runtime (no runtime_type)
├── 20250101000004_action_sensor.sql           # Action, sensor (with is_adhoc)
├── 20250101000005_trigger_event_rule.sql      # Trigger (with webhook_config), event (with rule), rule (with is_adhoc)
├── 20250101000006_execution_system.sql        # Execution (with workflow cols), enforcement, inquiry, policy
├── 20250101000007_workflow_system.sql         # Workflow_definition only (no workflow_task_execution)
├── 20250101000008_worker_notification.sql     # Worker (with role), notification
├── 20250101000009_artifacts.sql               # Artifact table
├── 20250101000010_webhook_system.sql          # Webhook tables, FINAL functions
├── 20250101000011_pack_environments.sql       # Pack_environment table and enum
├── 20250101000012_pack_testing.sql            # Pack_test_results table
├── 20250101000013_notify_triggers.sql         # ALL LISTEN/NOTIFY triggers
└── README.md                                  # Migration documentation
```

## Benefits

1. **Cleaner git history** - Future developers see logical progression
2. **Faster test setup** - Fewer migrations to run (13 vs 22)
3. **No dead code** - Nothing created just to be dropped
4. **Accurate from start** - Tables created with final schema
5. **Better documentation** - Clear purpose for each migration

## Risks

**NONE** - No production deployments exist. This is the ideal time to consolidate.

## Data Considerations

**Runtime metadata** is now managed in YAML files:
- Location: `packs/core/runtimes/*.yaml`
- Loaded by: Pack installation system
- Files: `python.yaml`, `nodejs.yaml`, `shell.yaml`, `native.yaml`, `sensor_builtin.yaml`

**Core pack data** should be checked for any other SQL insertions that should move to YAML.

## Validation Plan

1. Create consolidated migrations in new directory
2. Test on fresh database: `createdb attune_test && sqlx migrate run`
3. Compare schema output: `pg_dump --schema-only` before/after
4. Verify table counts, column counts, constraint counts match
5. Load core pack and verify all data loads correctly
6. Run full test suite
7. If successful, replace old migrations

## Timeline Estimate

- **Analysis complete**: ✅ Done
- **Create consolidated migrations**: 2-3 hours
- **Testing and validation**: 1-2 hours
- **Documentation updates**: 30 minutes
- **Total**: ~4-6 hours

## Recommendation

**PROCEED** with consolidation. This is a textbook case for migration consolidation:
- Pre-production system ✅
- No user data ✅
- Clear improvement to codebase ✅
- Low risk ✅
- High maintainability gain ✅

The longer we wait, the harder this becomes. Do it now while it's straightforward.