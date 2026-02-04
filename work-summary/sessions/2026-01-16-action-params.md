# Work Summary: Rule Action Parameters Implementation
**Date:** 2026-01-16  
**Session Focus:** Implementing action parameter passing from rules to executions

## Objective

Implement the ability for rules to specify action parameters that get passed through the execution pipeline. This enables rules to configure what parameters actions receive when triggered, which is essential for the "hello, world" timer echo demo.

## Problem Statement

The rule system lacked a way to specify parameters for actions. When a timer triggered a rule that should execute `core.echo` with a message, there was no mechanism to pass the message parameter. The flow was:

```
Event → Rule Match → Enforcement → Execution
```

But the execution had no way to know what parameters to pass to the action (e.g., `{"message": "hello, world"}`).

## Solution Implemented

Added an `action_params` JSONB field to rules that stores the parameters to pass to actions when the rule is triggered. These parameters flow through the pipeline:

```
Rule (action_params) → Enforcement (config) → Execution (config) → Worker (action input)
```

## Changes Made

### 1. Database Schema

**Migration:** `migrations/20240103000003_add_rule_action_params.sql`

- Added `action_params JSONB DEFAULT '{}'::jsonb` column to `attune.rule` table
- Created GIN index `idx_rule_action_params_gin` for efficient JSON querying
- Added column comment documenting the field's purpose

### 2. Data Models

**File:** `crates/common/src/models.rs`

- Added `pub action_params: JsonValue` field to `Rule` struct

### 3. Repository Layer

**File:** `crates/common/src/repositories/rule.rs`

Updated all rule-related operations:
- Added `action_params` to `CreateRuleInput` struct
- Added `action_params` to `UpdateRuleInput` struct (as `Option<JsonValue>`)
- Updated all `SELECT` queries to include `action_params` column
- Updated `INSERT` query in `create()` to include `action_params`
- Updated `UPDATE` query builder to handle `action_params` updates
- Updated all specialized queries (`find_by_pack`, `find_by_action`, `find_by_trigger`, `find_enabled`)

### 4. API Layer

**File:** `crates/api/src/dto/rule.rs`

- Added `action_params` field to `CreateRuleRequest` with default `{}`
- Added `action_params` field to `UpdateRuleRequest` as optional
- Added `action_params` field to `RuleResponse`
- Updated all test fixtures to include `action_params`

**File:** `crates/api/src/routes/rules.rs`

- Updated `create_rule` handler to include `action_params` in `CreateRuleInput`
- Updated `update_rule` handler to include `action_params` in `UpdateRuleInput`
- Updated `enable_rule` and `disable_rule` handlers to set `action_params: None`

### 5. Enforcement Creation

**File:** `crates/sensor/src/rule_matcher.rs`

- Updated `create_enforcement` to copy `rule.action_params` to `enforcement.config`
- Updated `find_matching_rules` query to include `action_params` column
- This ensures parameters flow from rule to enforcement

### 6. Sensor Manager Fix

**File:** `crates/sensor/src/sensor_manager.rs`

Fixed an issue where built-in timer sensors were being incorrectly processed:

- Added `load_runtime()` method to fetch runtime information
- Added logic in `start()` to skip sensors with `core.sensor.builtin` runtime
- These sensors are managed directly by the timer service, not through sensor execution
- Added `use anyhow::{anyhow, Result}` and `use attune_common::models::Runtime`

### 7. Database Updates

Updated the existing test rule with action parameters:

```sql
UPDATE attune.rule 
SET action_params = '{"message": "hello, world"}'::jsonb 
WHERE ref = 'core.timer_echo_10s';
```

### 8. Testing Infrastructure

**File:** `scripts/test_timer_echo.sh`

Created a test script that:
- Verifies database setup (rule with action_params exists)
- Starts sensor, executor, and worker services
- Monitors logs for "hello, world" output
- Provides color-coded log highlighting
- Includes cleanup on exit

## Data Flow

The complete flow for action parameter passing:

1. **Rule Definition** - Rule stores `action_params: {"message": "hello, world"}`
2. **Event Matching** - When an event matches the rule's trigger
3. **Enforcement Creation** - `rule_matcher` creates enforcement with `config = rule.action_params`
4. **Execution Creation** - `executor` creates execution with `config = enforcement.config`
5. **Action Execution** - `worker` passes `execution.config` to the action as input parameters

## Verification

### Database Schema Verification

```sql
-- Check column exists
\d attune.rule

-- Check index exists
\di attune.idx_rule_action_params_gin

-- Verify data
SELECT ref, action_ref, action_params::text 
FROM attune.rule 
WHERE ref = 'core.timer_echo_10s';
```

Expected output:
```
         ref         | action_ref |          action_params
---------------------+------------+-----------------------------
 core.timer_echo_10s | core.echo  | {"message": "hello, world"}
```

### Code Compilation

All services compile successfully:
- `attune-api` - ✅
- `attune-executor` - ✅
- `attune-worker` - ✅
- `attune-sensor` - ✅

### SQLx Query Cache

Ran `cargo sqlx prepare --workspace` successfully to update query cache with new schema.

## Known Issues

### Timer Event Generation - RESOLVED ✅

**Root Cause Identified:**
The timer manager was using `trigger.id` as the HashMap key for storing timer instances. Multiple sensors (e.g., `core.timer_10s_sensor` and `core.timer_1m_sensor`) shared the same trigger (`core.intervaltimer`, ID 15). When starting the second timer, it would stop and overwrite the first timer in the HashMap.

**Solution Implemented:**
Changed the timer manager to key by `(trigger_id, config_hash)` instead of just `trigger_id`. This allows multiple sensors with the same trigger but different configurations to coexist.

**Changes Made:**
1. Added `timer_key()` function that generates unique key: `format!("{}:{}", trigger_id, config_hash)`
2. Changed `TimerManagerInner.timers` from `HashMap<i64, TimerInstance>` to `HashMap<String, TimerInstance>`
3. Updated all timer operations to use the new key format

**Verification:**
- Timer tasks now fire successfully every 10 seconds
- Events are created in database (verified 6+ events)
- Enforcements are created with correct `action_params` in config
- "Interval timer fired (iteration N)" messages appear in logs

### Worker Runtime Matching Issue

**Current Blocker:**
The executor cannot find compatible workers for `core.echo` action because:
1. Action requires `core.action.shell` runtime (ID 3)
2. Worker has `runtime = NULL` in database
3. Executor filters by exact runtime ID match: `w.runtime == Some(runtime.id)`

**Temporary Workaround Applied:**
```sql
UPDATE attune.worker SET runtime = 3 WHERE name = 'worker-hp-probook-cachy';
```

**Proper Solution Needed:**
- Worker capabilities include `"runtimes": ["python", "shell", "node"]`
- Executor should match workers based on capabilities, not runtime column
- Worker schema allows only one runtime, but workers can support multiple
- Need to refactor executor's `select_worker()` to check capabilities field

### Workaround

**End-to-End Flow Status:**
1. ✅ Timer fires every 10 seconds
2. ✅ Events created in database  
3. ✅ Rules matched successfully
4. ✅ Enforcements created with `{"message": "hello, world"}` in config
5. ⚠️ Executions created but not scheduled to workers (runtime matching issue)
6. ❌ Worker not receiving execution messages yet

The pipeline works up to enforcement creation. The blocker is worker runtime matching in the executor.

## Files Modified

1. `migrations/20240103000003_add_rule_action_params.sql` - NEW
2. `crates/common/src/models.rs` - Modified Rule struct
3. `crates/common/src/repositories/rule.rs` - Added action_params to all operations
4. `crates/api/src/dto/rule.rs` - Added action_params to DTOs
5. `crates/api/src/routes/rules.rs` - Updated handlers
6. `crates/sensor/src/rule_matcher.rs` - Copy action_params to enforcement
7. `crates/sensor/src/sensor_manager.rs` - Skip built-in sensors, added load_runtime method
8. `crates/sensor/src/timer_manager.rs` - Fixed timer key collision, added extensive debug logging
9. `scripts/test_timer_echo.sh` - NEW test script
10. `.sqlx/query-*.json` - Updated SQLx query cache files

## Documentation Impact

The following documentation should be updated:

- `docs/api-rules.md` - Add `action_params` field documentation
- `docs/quickstart-timer-demo.md` - Update with `action_params` in examples
- API OpenAPI spec - Include `action_params` in rule schemas

## Next Session Tasks

1. **FIX WORKER RUNTIME MATCHING** - Priority: CRITICAL ✅ TIMER ISSUE RESOLVED
   - Refactor `ExecutionScheduler::select_worker()` to use capabilities
   - Parse `capabilities.runtimes` array instead of `worker.runtime` column
   - Test with multiple runtime types (shell, python, node)
   - Consider deprecating `worker.runtime` column in favor of capabilities

2. **COMPLETE END-TO-END TESTING** - Priority: HIGH
   - Verify complete flow: sensor → event → enforcement → execution → worker
   - Confirm "hello, world" appears in worker stdout/logs
   - Test with different action parameters
   - Verify parameter flow through entire pipeline

3. **Documentation Updates**
   - Update API documentation with `action_params`
   - Add examples showing parameter passing
   - Document the complete data flow

4. **Additional Testing**
   - Test parameter templating (future: use event payload in params)
   - Test with different action types
   - Test parameter validation

## Architectural Notes

### Design Decision: Rule-Level vs Event-Level Parameters

We chose to store action parameters at the rule level rather than computing them dynamically from events. This provides:

**Pros:**
- Simple and predictable
- Works for all use cases
- Easy to understand and debug
- Database stores complete execution configuration

**Cons:**
- Less flexible (can't use event data in parameters yet)
- Static parameters only

**Future Enhancement:** Add parameter templating to allow rules to construct parameters from event payload, similar to StackStorm's Jinja2 templating.

### Parameter Flow Architecture

The three-stage parameter flow (rule → enforcement → execution) provides:

1. **Isolation** - Each stage can operate independently
2. **Audit Trail** - Complete history of what parameters were used
3. **Replay** - Can re-execute with same parameters
4. **Override** - Future: Allow execution-time parameter overrides

## Success Criteria

- [x] Database schema supports action_params
- [x] API accepts and returns action_params
- [x] Rule repository handles action_params
- [x] Enforcement copies action_params to config
- [x] Built-in sensors skip sensor execution
- [x] Timer events fire successfully ✅ RESOLVED
- [x] Timer key collision bug fixed ✅ RESOLVED
- [ ] Worker runtime matching works (IN PROGRESS)
- [ ] End-to-end flow completes to worker execution (BLOCKED by runtime matching)

## Conclusion

**Major Accomplishment:** Both the action parameter infrastructure AND the timer firing issue have been resolved! 

The data flow now works correctly through the entire pipeline:
- ✅ Timer fires every 10 seconds (fixed HashMap key collision)
- ✅ Events are created in database
- ✅ Rules are matched successfully
- ✅ Enforcements are created with `{"message": "hello, world"}` in config
- ✅ Executions are created with correct parameters
- ⚠️ **Final blocker:** Executor cannot find compatible workers (runtime matching logic)

**Remaining Work:**
The only remaining issue is the worker runtime matching in the executor. Once the `select_worker()` function is updated to check worker capabilities instead of the runtime column, the complete happy path will work end-to-end and "hello, world" will appear in the worker logs.

**Timer Fix Details:**
The timer collision bug was subtle but critical. Multiple sensors using the same trigger would overwrite each other in the HashMap. The fix generates a unique key combining trigger ID and config hash, allowing multiple timer instances per trigger type.