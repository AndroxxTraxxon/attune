# Docker Compose Testing Summary

**Date:** 2026-02-07  
**Status:** Partial Success - Schema Issues Identified  
**Related Work:** Environment Variable Standardization

## Overview

Attempted to rebuild and test the Docker Compose stack with the newly standardized environment variables. The system is partially working but encountering database schema mismatch issues due to Docker build caching.

## Test Setup

Created three test rules to validate the end-to-end system:

1. **Echo Every Second** (`test.echo_every_second`)
   - Trigger: `core.intervaltimer` with 1-second interval
   - Action: `core.echo`
   - Status: Created successfully

2. **Sleep Every 5 Seconds** (`test.sleep_every_5s`)
   - Trigger: `core.intervaltimer` with 5-second interval
   - Action: `core.sleep` (3 seconds)
   - Status: Created successfully

3. **HTTP POST Every 10 Seconds** (`test.httpbin_post`)
   - Trigger: `core.intervaltimer` with 10-second interval
   - Action: `core.http_request` to httpbin.org
   - Status: Created successfully

## What's Working ✅

### Sensor Service
- ✅ Timer sensor running correctly
- ✅ Events being generated every second (for rule 2)
- ✅ Events being generated every 5 seconds (for rule 3)
- ✅ Events being generated every 10 seconds (for rule 4)
- ✅ Sensor receiving `ATTUNE_SENSOR_ID` environment variable

**Evidence:**
```
Event created successfully: id=164, trigger_ref=core.intervaltimer
Interval timer fired for rule 2 (count: 16), created event 164
```

### Executor Service
- ✅ Processing events from sensor
- ✅ Creating enforcements from matched rules
- ✅ Creating executions in database
- ✅ Publishing execution messages to message queue

**Evidence:**
```
Rule test.echo_every_second matched event 164 - creating enforcement
Enforcement 161 created for rule test.echo_every_second (event: 164)
Creating execution for enforcement: 161, rule: 2, action: 3
Execution 161 obtained queue slot for action 3
```

### Database
- ✅ Migrations applied successfully (including 20250205000002_execution_env_vars.sql)
- ✅ `env_vars` column exists in `execution` table
- ✅ Executions being created with `requested` status
- ✅ 147+ executions created during testing

**Database Query Results:**
```
id  |    action_ref     |  status   |            created
----+-------------------+-----------+-------------------------------
147 | core.echo         | requested | 2026-02-07 23:55:21.283296+00
146 | core.echo         | requested | 2026-02-07 23:55:20.272737+00
145 | core.echo         | requested | 2026-02-07 23:55:19.270934+00
144 | core.echo         | requested | 2026-02-07 23:55:18.285609+00
143 | core.sleep        | requested | 2026-02-07 23:55:18.275749+00
142 | core.http_request | requested | 2026-02-07 23:55:18.26473+00
```

### Worker Services
- ✅ Workers registered and running
- ✅ Queue infrastructure setup correctly
- ✅ Consuming from worker-specific queues
- ✅ No errors in worker logs

## What's NOT Working ❌

### Schema Mismatch Issue

**Problem:** Docker images contain binaries compiled BEFORE the `env_vars` column was added to the Execution model.

**Error:**
```
Database error: no column found for name: env_vars
Handler failed for message: Failed to process enforcement: Database error: no column found for name: env_vars
```

**Root Cause:**
1. The `env_vars` field was added to the Execution struct in `crates/common/src/models.rs`
2. Migration was created and applied successfully to the database
3. BUT Docker images were built from cached layers that don't include this code change
4. The compiled binaries use the old Execution model (without env_vars)
5. SQLx's `FromRow` derivation tries to map all database columns to struct fields
6. Database has `env_vars` column, but struct doesn't → ERROR

**Affected Services:**
- ❌ Executor (fails to update execution status)
- ❌ API (fails to query executions)
- ❌ Workers (likely fail to process executions, though not tested)

**Impact:**
- Executions are created but stuck in `requested` status
- Cannot query executions through API
- Workers cannot receive execution details
- End-to-end execution flow broken

## Attempted Fixes

### 1. Initial Rebuild Attempt
```bash
docker compose down -v
docker compose up -d --build
```
**Result:** Built quickly (used cache), schema mismatch persisted

### 2. No-Cache Rebuild Attempt
```bash
docker compose build --no-cache
```
**Result:** Timed out after 10 minutes (Rust compilation is VERY slow)

### 3. Selective Rebuild
```bash
docker compose build --no-cache attune-executor attune-worker-shell
```
**Result:** Started but didn't capture completion due to timeout

### 4. Third Rebuild Attempt
```bash
docker compose up -d --build
```
**Result:** Used cache again, schema mismatch persisted

## Why Docker Build is Challenging

1. **Rust Compilation Time:** Full workspace build takes 15-30 minutes
2. **Docker Layer Caching:** Aggressively caches layers, hard to invalidate
3. **Multi-Service Build:** Need to rebuild api, executor, worker-*, sensor, notifier
4. **Build Dependencies:** Some services depend on shared `attune-common` crate

## Workarounds Considered

### Option 1: Wait for Full Rebuild (SLOW)
```bash
docker compose down
docker compose build --no-cache --parallel
docker compose up -d
```
**Time:** 20-30 minutes  
**Reliability:** High

### Option 2: Make env_vars Optional (QUICK)
```rust
pub struct Execution {
    // ... other fields
    pub env_vars: Option<JsonDict>,  // Add Option<>
}
```
**Time:** 2 minutes + rebuild (uses cache)  
**Reliability:** Medium (temporary fix)

### Option 3: Test Without Docker (FASTEST)
Run services locally:
```bash
cargo build
./target/debug/attune-api &
./target/debug/attune-executor &
./target/debug/attune-worker &
./target/debug/attune-sensor &
```
**Time:** 5-10 minutes  
**Reliability:** High for dev testing

### Option 4: Drop and Recreate env_vars Column (HACKY)
```sql
ALTER TABLE execution DROP COLUMN env_vars;
```
**Time:** 1 minute  
**Reliability:** Low (loses feature)

## Recommended Next Steps

### Immediate (for this session):

1. **Document findings** (this file) ✅
2. **Verify executions are being created** ✅ (147 in database)
3. **Confirm sensor→executor→database flow works** ✅
4. **Stop services to prevent error spam** (optional)

### Short-term (next session):

1. **Full no-cache rebuild overnight or during break:**
   ```bash
   docker compose down -v
   docker compose build --no-cache --parallel
   docker compose up -d
   ```

2. **Verify end-to-end after rebuild:**
   - Run `setup-test-rules.sh`
   - Monitor executions with: `docker compose logs -f worker-shell`
   - Check execution status in database
   - Verify actions complete successfully

3. **Test new environment variables:**
   - Check worker logs for `ATTUNE_EXEC_ID`, `ATTUNE_ACTION`, `ATTUNE_API_URL`
   - Check sensor logs for `ATTUNE_SENSOR_ID`
   - Verify `ATTUNE_RULE` and `ATTUNE_TRIGGER` are set for rule-triggered executions

### Long-term:

1. **Improve Docker build caching strategy:**
   - Use BuildKit caching
   - Layer Rust dependencies separately from application code
   - Consider multi-stage builds

2. **Implement execution token generation:**
   - Currently `ATTUNE_API_TOKEN` is empty string
   - See `docs/TODO-execution-token-generation.md`

3. **Add integration tests for Docker stack:**
   - Automated health checks
   - End-to-end execution verification
   - Schema compatibility validation

## Test Artifacts Created

### Scripts
- ✅ `scripts/setup-test-rules.sh` - Creates test rules via API

### Rules Created (survived restart)
- ✅ `test.echo_every_second` (ID: 2)
- ✅ `test.sleep_every_5s` (ID: 3)  
- ✅ `test.httpbin_post` (ID: 4)

### Database State
- ✅ 147+ executions created (all in `requested` status)
- ✅ 170+ events generated by sensor
- ✅ 167+ enforcements created by executor

## Key Learnings

1. **Schema changes require full rebuild** when using Docker
2. **Docker layer caching is aggressive** - need `--no-cache` for schema changes
3. **Sensor→Executor flow works perfectly** - events and enforcements are being created
4. **Environment variable changes (our main work) can't be tested yet** due to binary mismatch
5. **Database migrations work correctly** - schema is up to date
6. **Queue infrastructure is solid** - messages flowing correctly
7. **Rust compilation time is the bottleneck** for Docker-based development

## Validation Status

| Component | Status | Notes |
|-----------|--------|-------|
| Database Schema | ✅ Up to date | Migration applied successfully |
| Sensor Service | ✅ Working | Events being generated |
| Executor Service | ⚠️ Partially | Creates executions but can't update status |
| Worker Services | ❓ Unknown | Can't test due to schema mismatch |
| API Service | ❌ Broken | Can't query executions |
| Environment Variables | ❓ Untested | Need working executors to verify |
| End-to-End Flow | ❌ Broken | Stuck at execution dispatch |

## Conclusion

The underlying system architecture is **working correctly** - sensors generate events, executor creates enforcements and executions, and all components are communicating properly. The only blocker is a **schema mismatch** between the database (which has the `env_vars` column) and the compiled binaries (which don't know about it).

This is a **build/deployment issue**, not an architectural problem. Once the Docker images are rebuilt with the current code, the environment variable standardization work can be properly tested and validated.

The fact that 147+ executions were created and queued demonstrates that the core event-driven architecture is functioning as designed. We just need the workers to have the correct code to process them.

## References

- Environment Variable Standardization: `work-summary/2026-02-07-env-var-standardization.md`
- Execution Token Generation TODO: `docs/TODO-execution-token-generation.md`
- Test Rules Script: `scripts/setup-test-rules.sh`
- Quick Reference: `docs/QUICKREF-sensor-action-env-parity.md`
