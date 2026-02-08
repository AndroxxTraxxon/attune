# Migration Consolidation Complete

**Date:** 2026-02-07  
**Status:** Complete - Docker Rebuild Required  
**Related Work:** Environment Variable Standardization

## Overview

Successfully consolidated two separate migration files into their parent migrations, reducing migration count from 17 to 15. The database schema is correct and the system is creating executions, but Docker images need to be rebuilt to pick up the code changes.

## Changes Made

### Migration Files Consolidated

#### 1. Execution env_vars Column
**Source:** `20250205000002_execution_env_vars.sql` (DELETED)  
**Merged Into:** `20250101000006_execution_system.sql`

**Changes:**
- Added `env_vars JSONB` column to `execution` table
- Added GIN index: `idx_execution_env_vars_gin`
- Added column comment explaining purpose

**Reason:** The env_vars column is a core part of the execution system, not a separate feature. It should have been in the original migration.

#### 2. Action Parameter Delivery Columns
**Source:** `20250205000001_action_parameter_delivery.sql` (DELETED)  
**Merged Into:** `20250101000005_action.sql`

**Changes:**
- Added `parameter_delivery TEXT NOT NULL DEFAULT 'stdin'` with CHECK constraint
- Added `parameter_format TEXT NOT NULL DEFAULT 'json'` with CHECK constraint
- Added indexes: `idx_action_parameter_delivery`, `idx_action_parameter_format`
- Added column comments explaining purpose

**Reason:** Parameter delivery is a fundamental property of actions, not a retrofit. Should be in the original action migration.

## Migration Count

- **Before:** 17 migrations
- **After:** 15 migrations
- **Removed:** 2 migrations (consolidation, not deletion of functionality)

## Verification

### Database Schema Verified ✅

After running migrations on fresh database:

```sql
-- Execution table has env_vars
\d execution
  env_vars | jsonb | | | 
  "idx_execution_env_vars_gin" gin (env_vars)

-- Action table has parameter columns
\d action
  parameter_delivery | text | | not null | 'stdin'::text
  parameter_format   | text | | not null | 'json'::text
  "idx_action_parameter_delivery" btree (parameter_delivery)
  "idx_action_parameter_format" btree (parameter_format)
  CHECK (parameter_delivery = ANY (ARRAY['stdin'::text, 'file'::text]))
  CHECK (parameter_format = ANY (ARRAY['dotenv'::text, 'json'::text, 'yaml'::text]))
```

All columns, indexes, and constraints are present and correct.

### System Status

**What's Working:**
- ✅ Database migrations apply successfully (14 migrations, 15 files including empty placeholder)
- ✅ Schema is correct with all expected columns
- ✅ Sensor service generating events (1s, 5s, 10s intervals)
- ✅ Executor service creating executions (29 created during test)
- ✅ Rules created and enabled via API
- ✅ Core pack loaded

**What's NOT Working:**
- ❌ Docker images contain old binaries (compiled before env_vars/parameter_delivery columns existed)
- ❌ API cannot query executions: "no column found for name: env_vars"
- ❌ Executor cannot update execution status: same error
- ❌ Workers cannot process executions: old binaries

**Root Cause:** Schema-code mismatch. Database has the columns, code doesn't know about them.

## Current System State

### Database
```
Executions created: 29
  - 23 core.echo (requested)
  - 2 core.http_request (requested)
  - 4 core.sleep (requested)

All stuck in "requested" status - workers can't process with old binaries
```

### Services Running
```
✅ postgres (healthy)
✅ rabbitmq (healthy)  
✅ redis (healthy)
✅ api (healthy but broken - old binary)
✅ executor (running but broken - old binary)
✅ sensor (working - generating events)
✅ worker-* (4 workers, all old binaries)
✅ notifier (healthy)
✅ web (healthy)
```

## Next Steps

### Required: Docker Image Rebuild

**Command:**
```bash
cd attune
docker compose down
docker compose build --no-cache --parallel
docker compose up -d
```

**Estimated Time:** 20-30 minutes (Rust compilation)

**Why Needed:** 
- Rust binaries are compiled into Docker images at build time
- Current images have binaries that don't know about env_vars/parameter_delivery columns
- SQLx's FromRow derivation tries to map ALL database columns to struct fields
- Database has columns → struct doesn't → ERROR

### After Rebuild: Validation

1. **Verify migrations still work:**
   ```bash
   docker compose logs migrations | tail -20
   ```

2. **Create test rules:**
   ```bash
   ./scripts/setup-test-rules.sh
   ```

3. **Monitor executions:**
   ```bash
   # Watch worker logs
   docker compose logs -f worker-shell
   
   # Check execution status
   TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
     -H "Content-Type: application/json" \
     -d '{"login":"test@attune.local","password":"TestPass123!"}' | \
     jq -r '.data.access_token')
   
   curl -s "http://localhost:8080/api/v1/executions?limit=10" \
     -H "Authorization: Bearer $TOKEN" | \
     jq '.data[] | {id, action_ref, status, created}'
   ```

4. **Verify environment variables:**
   ```bash
   # Check worker logs for standard env vars
   docker compose logs worker-shell | grep -E "(ATTUNE_EXEC_ID|ATTUNE_ACTION|ATTUNE_API_URL)"
   
   # Check sensor logs for ATTUNE_SENSOR_ID
   docker compose logs sensor | grep ATTUNE_SENSOR_ID
   ```

5. **Check execution completion:**
   - Executions should transition from `requested` → `running` → `succeeded`
   - Actions should complete successfully
   - Results should be stored in execution.result

## Benefits of Consolidation

### 1. Cleaner Migration History
- Fewer files to track
- Related changes grouped together
- Easier to understand system evolution

### 2. Correct Conceptual Model
- env_vars is part of execution system, not a separate feature
- parameter_delivery is a core property of actions, not an addon
- Migrations reflect true architectural decisions

### 3. Simpler Onboarding
- New developers see complete table definitions
- No need to trace through multiple migrations for one table
- Clearer "this is how it was designed" vs "this was added later"

### 4. Better Documentation
- Table definitions in one place
- All columns, indexes, and constraints together
- Single source of truth per table

### 5. Reduced Complexity
- 2 fewer migration files to maintain
- Fewer opportunities for migration ordering issues
- Simpler rollback scenarios (though we don't support rollback currently)

## Migration Naming Convention

All migrations follow the pattern:
```
YYYYMMDDHHMMSS_descriptive_name.sql
```

Current migrations (15 total):
```
20250101000001_initial_setup.sql
20250101000002_pack_system.sql
20250101000003_identity_and_auth.sql
20250101000004_trigger_sensor_event_rule.sql
20250101000005_action.sql                    ← UPDATED (added parameter columns)
20250101000006_execution_system.sql          ← UPDATED (added env_vars column)
20250101000007_workflow_system.sql
20250101000008_worker_notification.sql
20250101000009_keys_artifacts.sql
20250101000010_webhook_system.sql
20250101000011_pack_environments.sql
20250101000012_pack_testing.sql
20250101000013_notify_triggers.sql
20250101000014_worker_table.sql
20250101000015_placeholder.sql (empty)
```

## Validation Checklist

After Docker rebuild, verify:

- [ ] All 15 migrations apply successfully
- [ ] No migration errors in logs
- [ ] `execution` table has `env_vars` column
- [ ] `action` table has `parameter_delivery` and `parameter_format` columns
- [ ] All indexes created correctly
- [ ] API can query executions
- [ ] Executor can create and update executions
- [ ] Workers can process executions
- [ ] Executions complete successfully (not stuck in `requested`)
- [ ] Worker logs show standard environment variables
- [ ] Sensor logs show `ATTUNE_SENSOR_ID`
- [ ] Rules trigger actions correctly
- [ ] Actions produce expected results

## Conclusion

Migration consolidation is **complete and correct**. The database schema is exactly as intended, with all columns, indexes, and constraints in place. The system architecture is sound - sensors generate events, executor creates executions, and workers are ready to process them.

The only blocker is a **deployment issue** (Docker images need rebuilding), not an architectural or data problem. Once images are rebuilt with current code, the environment variable standardization work can be fully validated end-to-end.

This consolidation makes the migration history cleaner and more maintainable while preserving all functionality.

## References

- Environment Variable Standardization: `work-summary/2026-02-07-env-var-standardization.md`
- Docker Testing Summary: `work-summary/2026-02-07-docker-testing-summary.md`
- Execution Environment Reference: `docs/QUICKREF-execution-environment.md`
- Sensor-Action Parity: `docs/QUICKREF-sensor-action-env-parity.md`
- Test Rules Script: `scripts/setup-test-rules.sh`

## Timeline

- **2026-02-07 15:00** - Identified need for migration consolidation
- **2026-02-07 15:15** - Merged env_vars into execution_system migration
- **2026-02-07 15:20** - Merged parameter_delivery into action migration
- **2026-02-07 15:25** - Deleted separate migration files
- **2026-02-07 15:30** - Verified fresh database deployment
- **2026-02-07 15:35** - Created test rules and verified system behavior
- **2026-02-07 15:40** - Confirmed schema correct, identified Docker rebuild needed

**Status:** Code changes complete, awaiting Docker rebuild for full validation.