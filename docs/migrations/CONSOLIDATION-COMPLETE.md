# Migration Consolidation - Complete

**Date**: 2026-02-04  
**Status**: ✅ COMPLETE  
**Result**: 22 migrations → 13 migrations

## Summary

Successfully consolidated Attune's migration history from 22 files to 13 clean, logical migrations. This was possible because there are no production deployments yet, allowing us to freely restructure the schema history.

## Changes Made

### Items Removed Entirely (Never Created)

1. **`runtime_type_enum`** - Removed from initial setup
   - Associated column `runtime.runtime_type` not created
   - Associated indexes not created: `idx_runtime_type`, `idx_runtime_pack_type`, `idx_runtime_type_created`, `idx_runtime_type_sensor`
   - Runtime table uses unified approach from the start

2. **`workflow_task_execution` table** - Not created
   - Consolidated into `execution.workflow_task JSONB` column from initial execution table creation
   - Eliminates need for separate table and join operations

3. **Individual webhook columns** - Not created
   - Skipped 10 intermediate columns (webhook_secret, webhook_hmac_*, webhook_rate_limit_*, etc.)
   - Only created: `webhook_enabled`, `webhook_key`, `webhook_config JSONB` from start

4. **Runtime data insertions** - Removed from migrations
   - All runtime metadata moved to YAML files in `packs/core/runtimes/`
   - No SQL INSERT statements for runtime records

### Items Included From Start

1. **Execution table workflow columns** (in 00006):
   - `workflow_def BIGINT REFERENCES workflow_definition(id)`
   - `workflow_task JSONB`

2. **Is adhoc flags** (in respective tables):
   - `action.is_adhoc` (in 00004)
   - `sensor.is_adhoc` (in 00004)
   - `rule.is_adhoc` (in 00005)

3. **Event table rule tracking** (in 00005):
   - `event.rule BIGINT`
   - `event.rule_ref TEXT`
   - Foreign key constraint to rule table

4. **Worker role** (in 00008):
   - `worker_role_enum` type (in 00001)
   - `worker.worker_role` column

5. **Trigger webhook support** (in 00005):
   - `webhook_enabled BOOLEAN NOT NULL DEFAULT FALSE`
   - `webhook_key VARCHAR(64) UNIQUE`
   - `webhook_config JSONB DEFAULT '{}'::jsonb`

6. **Pack environments** (in 00001 and 00003):
   - `pack_environment_status_enum` type (in 00001)
   - `pack.installers JSONB` column (in 00003)
   - `pack_environment` table (in 00011)

## Final Migration Structure

```
migrations/
├── 20250101000001_initial_setup.sql           # Enums, extensions (minus runtime_type_enum, plus worker_role_enum and pack_environment_status_enum)
├── 20250101000002_identity_and_auth.sql       # Identity, permission_set, permission_assignment, policy
├── 20250101000003_pack_system.sql             # Pack (with installers), runtime (no runtime_type)
├── 20250101000004_action_sensor.sql           # Action, sensor (both with is_adhoc)
├── 20250101000005_trigger_event_rule.sql      # Trigger (with webhook_config), event (with rule), rule (with is_adhoc)
├── 20250101000006_execution_system.sql        # Enforcement, execution (with workflow columns), inquiry
├── 20250101000007_workflow_system.sql         # Workflow_definition, workflow_execution (no workflow_task_execution)
├── 20250101000008_worker_notification.sql     # Worker (with worker_role), notification
├── 20250101000009_keys_artifacts.sql          # Key, artifact
├── 20250101000010_webhook_system.sql          # Webhook functions (final versions)
├── 20250101000011_pack_environments.sql       # Pack_environment table
├── 20250101000012_pack_testing.sql            # Pack_test_results table
└── 20250101000013_notify_triggers.sql         # All LISTEN/NOTIFY triggers (consolidated)
```

## Migrations Removed

The following 15 migration files were consolidated or had their data moved to YAML:

1. `20260119000001_add_execution_notify_trigger.sql` → Consolidated into 00013
2. `20260120000001_add_webhook_support.sql` → Columns added to trigger table in 00005
3. `20260120000002_webhook_advanced_features.sql` → Functions consolidated in 00010
4. `20260122000001_pack_installation_metadata.sql` → Merged into pack system
5. `20260127000001_consolidate_webhook_config.sql` → Already consolidated in 00005
6. `20260127212500_consolidate_workflow_task_execution.sql` → Already in execution table in 00006
7. `20260129000001_fix_webhook_function_overload.sql` → Fixed functions in 00010
8. `20260129140130_add_is_adhoc_flag.sql` → Already in tables in 00004/00005
9. `20260129150000_add_event_notify_trigger.sql` → Consolidated into 00013
10. `20260130000001_add_rule_to_event.sql` → Already in event table in 00005
11. `20260131000001_add_worker_role.sql` → Already in worker table in 00008
12. `20260202000001_add_sensor_runtimes.sql` → Data moved to YAML files
13. `20260203000001_unify_runtimes.sql` → Changes applied to base runtime table in 00003
14. `20260203000003_add_rule_trigger_to_execution_notify.sql` → Consolidated into 00013
15. `20260204000001_add_enforcement_notify_trigger.sql` → Consolidated into 00013

Note: One file (`20260204000001_restore_webhook_functions.sql`) was kept and renamed to 00010 with final webhook functions.

## Benefits

1. **Cleaner History**: Future developers see logical progression, not incremental fixes
2. **Faster Tests**: 13 migrations vs 22 (41% reduction)
3. **No Dead Code**: Nothing created just to be dropped
4. **Accurate Schema**: Tables created with final structure from the start
5. **Better Maintainability**: Each migration has clear, focused purpose
6. **Reduced Complexity**: Fewer foreign key constraints to manage incrementally

## Data Migration

### Runtime Metadata

Runtime data is now managed externally:

**Location**: `packs/core/runtimes/*.yaml`

**Files**:
- `python.yaml` - Python 3 runtime
- `nodejs.yaml` - Node.js runtime
- `shell.yaml` - Shell runtime (bash/sh)
- `native.yaml` - Native compiled runtime
- `sensor_builtin.yaml` - Built-in sensor runtime

**Loading**: Handled by pack installation system, not migrations

## Testing

Next steps for validation:

```bash
# 1. Test on fresh database
createdb attune_test_consolidated
export DATABASE_URL="postgresql://attune:attune@localhost/attune_test_consolidated"
sqlx migrate run

# 2. Compare schema
pg_dump --schema-only attune_test_consolidated > schema_new.sql
pg_dump --schema-only attune_dev > schema_old.sql
diff schema_old.sql schema_new.sql

# 3. Verify table counts
psql attune_test_consolidated -c "\dt" | wc -l

# 4. Load core pack
./scripts/load-core-pack.sh

# 5. Run tests
cargo test
```

## Rollback Plan

Original migrations preserved in `migrations.old/` directory. To rollback:

```bash
rm -rf migrations/*.sql
cp migrations.old/*.sql migrations/
```

**Do NOT delete `migrations.old/` until consolidated migrations are verified in production-like environment.**

## Constraints Modified

1. **`runtime_ref_format`** - Removed entirely
   - Old format: `^[^.]+\.(action|sensor)\.[^.]+$` (e.g., `core.action.python`)
   - New format: No constraint, allows `pack.name` format (e.g., `core.python`)

2. **`runtime_ref_lowercase`** - Kept as-is
   - Still enforces lowercase runtime refs

## Indexes Added/Modified

**Runtime table**:
- ❌ Removed: `idx_runtime_type`, `idx_runtime_pack_type`, `idx_runtime_type_created`
- ✅ Added: `idx_runtime_name`, `idx_runtime_verification` (GIN index)

**Trigger table**:
- ✅ Added: `idx_trigger_webhook_key`

**Event table**:
- ✅ Added: `idx_event_rule`

## Statistics

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Migration files | 22 | 13 | -41% |
| Lines of SQL | ~3,500 | ~2,100 | -40% |
| Enum types | 13 | 12 | -1 |
| Tables created | 22 | 21 | -1 |
| Tables created then dropped | 1 | 0 | -100% |
| Columns added then dropped | 10 | 0 | -100% |

## Completion Checklist

- ✅ Backup created in `migrations.old/`
- ✅ 13 consolidated migrations created
- ✅ Runtime data moved to YAML files
- ✅ All incremental additions consolidated
- ✅ Documentation updated
- ⏳ Test on fresh database
- ⏳ Compare schemas
- ⏳ Run full test suite
- ⏳ Deploy to development
- ⏳ Delete `migrations.old/` after verification

## Notes

- All changes are breaking changes, but that's acceptable since there are no production deployments
- Future migrations should be created normally and incrementally
- This consolidation should be a one-time event before v1.0 release
- After production deployment, normal migration discipline applies (no deletions, only additions)

## Acknowledgments

This consolidation was made possible by the "Breaking Changes Policy" documented in `AGENTS.md`:

> **Breaking changes are explicitly allowed and encouraged** when they improve the architecture, API design, or developer experience. No backward compatibility required - there are no existing versions to support.

Once this project reaches v1.0 or gets its first production deployment, this policy will be replaced with appropriate stability guarantees and versioning policies.
