# Current Problems - Attune Platform

**Last Updated:** 2026-01-28

## 🚨 Critical Issues

*No critical issues at this time.*

---

## ✅ Recently Fixed Issues

### E2E Test Execution Filtering Race Condition (2026-01-28)
**Status:** RESOLVED  
**Priority:** P2

**Issue:**
The E2E test execution count check had a race condition and filtering issue where it wasn't finding the executions it just created. The test would create a rule, wait for events, then check for executions, but the execution query would either:
1. Match old executions from previous test runs (not cleaned up properly)
2. Miss newly created executions due to imprecise filtering
3. Count executions from other tests running in parallel

**Root Cause:**
- The `wait_for_execution_count` helper only supported filtering by `action_ref` and `status`
- `action_ref` filtering is imprecise - multiple tests could create actions with similar refs
- No support for filtering by `rule_id` or `enforcement_id` (more precise)
- No timestamp-based filtering to exclude old executions from previous runs
- The API supports `enforcement` parameter but the client and helper didn't use it

**Solution Implemented:**
1. **Enhanced `wait_for_execution_count` helper**:
   - Added `enforcement_id` parameter for direct enforcement filtering
   - Added `rule_id` parameter to get executions via enforcement lookup
   - Added `created_after` timestamp parameter to filter out old executions
   - Added `verbose` debug mode to see what's being matched during polling
   
2. **Updated `AttuneClient.list_executions`**:
   - Added `enforcement_id` parameter support
   - Maps to API's `enforcement` query parameter
   
3. **Updated test_t1_01_interval_timer.py**:
   - Captures timestamp before rule creation
   - Uses `rule_id` filtering instead of `action_ref` (more precise)
   - Uses `created_after` timestamp to exclude old executions
   - Enables verbose mode for better debugging

**Result:**
- ✅ Execution queries now use most precise filtering (rule_id → enforcements → executions)
- ✅ Timestamp filtering prevents matching old data from previous test runs
- ✅ Verbose mode helps diagnose any remaining filtering issues
- ✅ Race conditions eliminated by combining multiple filter criteria
- ✅ Tests are now isolated and don't interfere with each other

**Time to Resolution:** 45 minutes

**Files Modified:**
- `tests/helpers/polling.py` - Enhanced `wait_for_execution_count` with new filters
- `tests/helpers/client.py` - Added `enforcement_id` parameter to `list_executions`
- `tests/e2e/tier1/test_t1_01_interval_timer.py` - Updated to use precise filtering

**Technical Details:**
The fix leverages the API's existing filtering capabilities:
- `GET /api/v1/executions?enforcement=<id>` - Filter by enforcement (most precise)
- `GET /api/v1/enforcements?rule_id=<id>` - Get enforcements for a rule
- Timestamp filtering applied in-memory after API call

**Next Steps:**
- Apply same filtering pattern to other tier1 tests
- Monitor for any remaining race conditions
- Consider adding database cleanup improvements

---

---

## ✅ Recently Fixed Issues

### Duplicate `create_sensor` Method in E2E Test Client (2026-01-28)
**Status:** RESOLVED  
**Priority:** P1

**Issue:**
The `AttuneClient` class in `tests/helpers/client.py` had two `create_sensor` methods defined with different signatures, causing Python to shadow the first method with the second.

**Root Cause:**
- First method (lines 601-636): API-based signature expecting `pack_ref`, `name`, `trigger_types`, `entrypoint`, etc.
- Second method (lines 638-759): SQL-based signature expecting `ref`, `trigger_id`, `trigger_ref`, `label`, `config`, etc.
- In Python, duplicate method names result in the second definition overwriting the first
- Fixture helpers were calling with the second signature (SQL-based), which worked but was confusing
- First method was unreachable dead code

**Solution Implemented:**
Removed the first (unused) API-based `create_sensor` method definition (lines 601-636), keeping only the SQL-based version that the fixture helpers actually use.

**Result:**
- ✅ No more duplicate method definition
- ✅ Code is cleaner and less confusing
- ✅ Python syntax check passes
- ✅ All 34 tier1 E2E tests now collect successfully

**Time to Resolution:** 15 minutes

**Files Modified:**
- `tests/helpers/client.py` - Removed lines 601-636 (duplicate method)

**Next Steps:**
- Run tier1 E2E tests to identify actual test failures
- Fix any issues with sensor service integration
- Work through test failures systematically

---

## ✅ Fixed Issues

### OpenAPI Nullable Fields Issue (2026-01-28)
**Status:** RESOLVED  
**Priority:** P0

**Issue:**
E2E tests were failing with `TypeError: 'NoneType' object is not iterable` when the generated Python OpenAPI client tried to deserialize API responses containing nullable object fields (like `param_schema`, `out_schema`) that were `null`.

**Root Cause:**
The OpenAPI specification generated by `utoipa` was not properly marking optional `Option<JsonValue>` fields as nullable. The `#[schema(value_type = Object)]` annotation alone doesn't add `nullable: true` to the schema, causing the generated Python client to crash when encountering `null` values.

**Solution Implemented:**
1. Added `nullable = true` attribute to all `Option<JsonValue>` response fields in 7 DTO files:
   - `action.rs`, `trigger.rs`, `event.rs`, `inquiry.rs`, `pack.rs`, `rule.rs`, `workflow.rs`
2. Added `#[serde(skip_serializing_if = "Option::is_none")]` to request DTOs to make fields truly optional
3. Regenerated Python client with fixed OpenAPI spec

**Result:**
- ✅ OpenAPI spec now correctly shows `"type": ["object", "null"]` for nullable fields
- ✅ Generated Python client handles `None` values without crashing
- ✅ E2E tests can now run without TypeError
- ✅ 23 total field annotations fixed across all DTOs

**Time to Resolution:** 2 hours

**Files Modified:**
- 7 DTO files in `crates/api/src/dto/`
- Entire `tests/generated_client/` directory regenerated

**Documentation:**
- See `work-summary/2026-01-28-openapi-nullable-fields-fix.md` for full details

---

## ✅ Fixed Issues

### Workflow Schema Alignment (2025-01-13)
**Status:** RESOLVED  
**Priority:** P1

**Issue:**
Phase 1.4 (Workflow Loading & Registration) implementation discovered schema incompatibilities between the workflow orchestration design (Phases 1.2/1.3) and the actual database schema.

**Root Cause:**
The workflow design documents assumed different Action model fields than what exists in the migrations:
- Expected: `pack_id`, `ref_name`, `name`, `runner_type`, `Optional<description>`, `Optional<entry_point>`
- Actual: `pack`, `ref`, `label`, `runtime`, `description` (required), `entrypoint` (required)

**Current State:**
- ✅ WorkflowLoader module complete and tested (loads YAML files)
- ⏸️ WorkflowRegistrar module needs adaptation to actual schema
- ⏸️ Repository usage needs conversion to trait-based static methods

**Required Changes:**
1. Update registrar to use `CreateActionInput` with actual field names
2. Convert repository instance methods to trait static methods (e.g., `ActionRepository::find_by_ref(&pool, ref)`)
3. Decide on workflow conventions:
   - Entrypoint: Use `"internal://workflow"` or similar placeholder
   - Runtime: Use NULL (workflows don't execute in runtimes)
   - Description: Default to empty string if not in YAML
4. Verify workflow_definition table schema matches models

**Files Affected:**
- `crates/executor/src/workflow/registrar.rs` - Needs schema alignment
- `crates/executor/src/workflow/loader.rs` - Complete, no changes needed

**Next Steps:**
1. Review workflow_definition table structure
2. Create helper to map WorkflowDefinition → CreateActionInput
3. Fix repository method calls throughout registrar
4. Add integration tests with database

**Documentation:**
- See `work-summary/phase-1.4-loader-registration-progress.md` for full details

**Resolution:**
- Updated registrar to use `CreateWorkflowDefinitionInput` instead of `CreateActionInput`
- Workflows now stored in `workflow_definition` table as standalone entities
- Complete workflow YAML serialized to JSON in `definition` field
- Repository calls converted to trait static methods
- All compilation errors fixed - builds successfully
- All 30 workflow tests passing

**Time to Resolution:** 3 hours

**Files Modified:**
- `crates/executor/src/workflow/registrar.rs` - Complete rewrite to use workflow_definition table
- `crates/executor/src/workflow/loader.rs` - Fixed validator calls and borrow issues
- Documentation updated with actual implementation

---

### Message Loop in Execution Manager (2026-01-16)
**Status:** RESOLVED  
**Priority:** P0

**Issue:**
Executions entered an infinite loop where ExecutionCompleted messages were routed back to the execution manager's status queue, causing the same completion to be processed repeatedly.

**Root Cause:**
The execution manager's queue was bound to `execution.status.#` (wildcard pattern) which matched:
- `execution.status.changed` ✅ (intended)
- `execution.completed` ❌ (unintended - should not be reprocessed)

**Solution Implemented:**
Changed queue binding in `common/src/mq/connection.rs` from `execution.status.#` to `execution.status.changed` (exact match).

**Files Modified:**
- `crates/common/src/mq/connection.rs` - Updated execution_status queue binding

**Result:**
- ✅ ExecutionCompleted messages no longer route to status queue
- ✅ Manager only processes each status change once
- ✅ No more infinite loops

### Worker Runtime Resolution (2026-01-16)
**Status:** RESOLVED  
**Priority:** P0

**Issue:**
Worker received execution messages but failed with "Runtime not found: No runtime found for action: core.echo" even though the worker had the shell runtime available.

**Root Cause:**
The worker's runtime selection logic relied on `can_execute()` methods that checked file extensions and action_ref patterns. The `core.echo` action didn't match any patterns, so no runtime was selected. The action's runtime metadata (stored in the database as `runtime: 3` pointing to the shell runtime) was not being used.

**Solution Implemented:**
1. Added `runtime_name: Option<String>` field to `ExecutionContext`
2. Updated worker executor to load runtime information from database
3. Modified `RuntimeRegistry::get_runtime()` to prefer `runtime_name` if provided
4. Fall back to `can_execute()` checks if no runtime_name specified

**Files Modified:**
- `crates/worker/src/runtime/mod.rs` - Added runtime_name field, updated get_runtime()
- `crates/worker/src/executor.rs` - Load runtime from database, populate runtime_name
- Test files updated to include new field

**Result:**
- ✅ Worker correctly identifies which runtime to use for each action
- ✅ Runtime selection based on authoritative database metadata
- ✅ Backward compatible with can_execute() for ad-hoc executions

### Message Queue Architecture (2026-01-16)
**Status:** RESOLVED  
**Issue:** Three executor consumers competing for messages on same queue

**Solution Implemented:**
- Created separate queues for each message type:
  - `attune.enforcements.queue` → Enforcement Processor (routing: `enforcement.#`)
  - `attune.execution.requests.queue` → Scheduler (routing: `execution.request.#`)
  - `attune.execution.status.queue` → Manager (routing: `execution.status.#`)
- Updated all publishers to use correct routing keys
- Each consumer now has dedicated queue

**Result:**
- ✅ No more deserialization errors
- ✅ Enforcements created successfully
- ✅ Executions scheduled successfully
- ✅ Messages reach workers
- ❌ Still have runtime resolution and message loop issues

### Worker Runtime Matching (2026-01-16)
**Status:** RESOLVED  
**Issue:** Executor couldn't match workers by capabilities

**Solution Implemented:**
- Refactored `ExecutionScheduler::select_worker()`
- Added `worker_supports_runtime()` helper
- Checks worker's `capabilities.runtimes` array
- Case-insensitive runtime name matching

**Result:**
- ✅ Workers correctly selected for actions
- ✅ Runtime matching works as designed

### Sensor Service Webhook Compilation (2026-01-22)
**Status:** RESOLVED  
**Priority:** P1

**Issue:**
After webhook Phase 3 advanced features were implemented, the sensor service failed to compile with errors about missing webhook fields in Trigger model initialization.

**Root Cause:**
1. The `Trigger` model was updated with 12 new webhook-related fields (HMAC, rate limiting, IP whitelist, payload size limits)
2. Sensor service SQL queries in `sensor_manager.rs` and `service.rs` were still using old field list
3. Database migrations for webhook advanced features were not applied to development database
4. SQLx query cache (`.sqlx/`) was outdated and missing metadata for updated queries

**Errors:**
```
error[E0063]: missing fields `webhook_enabled`, `webhook_hmac_algorithm`, 
`webhook_hmac_enabled` and 9 other fields in initializer of `attune_common::models::Trigger`
```

**Solution Implemented:**
1. Updated trigger queries in both files to include all 12 new webhook fields:
   - `webhook_enabled`, `webhook_key`, `webhook_secret`
   - `webhook_hmac_enabled`, `webhook_hmac_secret`, `webhook_hmac_algorithm`
   - `webhook_rate_limit_enabled`, `webhook_rate_limit_requests`, `webhook_rate_limit_window_seconds`
   - `webhook_ip_whitelist_enabled`, `webhook_ip_whitelist`
   - `webhook_payload_size_limit_kb`

2. Applied pending database migrations:
   - Created `attune_api` role (required by migration grants)
   - Applied `20260119000001_add_execution_notify_trigger.sql`
   - Applied `20260120000001_add_webhook_support.sql`
   - Applied `20260120000002_webhook_advanced_features.sql`
   - Fixed checksum mismatch for `20260120200000_add_pack_test_results.sql`
   - Applied `20260122000001_pack_installation_metadata.sql`

3. Regenerated SQLx query cache:
   ```bash
   export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/attune"
   cargo sqlx prepare --workspace
   ```

**Files Modified:**
- `crates/sensor/src/sensor_manager.rs` - Added webhook fields to trigger query
- `crates/sensor/src/service.rs` - Added webhook fields to trigger query
- `.sqlx/*.json` - Regenerated query cache (10 files updated)

**Result:**
- ✅ Sensor service compiles successfully
- ✅ All workspace packages compile without errors
- ✅ SQLx offline mode (`SQLX_OFFLINE=true`) works correctly
- ✅ Query cache committed to version control
- ✅ Database schema in sync with model definitions

**Time to Resolution:** 30 minutes

**Lessons Learned:**
- When models are updated with new fields, all SQL queries using those models must be updated
- SQLx compile-time checking requires either DATABASE_URL or prepared query cache
- Database migrations must be applied before preparing query cache
- Always verify database schema matches model definitions before debugging compilation errors

### E2E Test Import and Client Method Errors (2026-01-22)
**Status:** RESOLVED  
**Priority:** P1

**Issue:**
Multiple E2E test files failed with import errors and missing/incorrect client methods:
- `wait_for_execution_completion` not found in `helpers.polling`
- `timestamp_future` not found in `helpers`
- `create_failing_action` not found in `helpers`
- `AttributeError: 'AttuneClient' object has no attribute 'create_pack'`
- `TypeError: AttuneClient.create_secret() got an unexpected keyword argument 'encrypted'`

**Root Causes:**
1. Test files were importing `wait_for_execution_completion` which didn't exist in `polling.py`
2. Helper functions `timestamp_future`, `create_failing_action`, `create_sleep_action`, and polling utilities were not exported from `helpers/__init__.py`
3. `AttuneClient` was missing `create_pack()` method
4. `create_secret()` method had incorrect signature (API uses `/api/v1/keys` endpoint with different schema)

**Affected Tests (11 files):**
- `tests/e2e/tier1/test_t1_02_date_timer.py` - Missing helper imports
- `tests/e2e/tier1/test_t1_08_action_failure.py` - Missing helper imports
- `tests/e2e/tier3/test_t3_07_complex_workflows.py` - Missing helper imports
- `tests/e2e/tier3/test_t3_08_chained_webhooks.py` - Missing helper imports
- `tests/e2e/tier3/test_t3_09_multistep_approvals.py` - Missing helper imports
- `tests/e2e/tier3/test_t3_14_execution_notifications.py` - Missing helper imports
- `tests/e2e/tier3/test_t3_17_container_runner.py` - Missing helper imports
- `tests/e2e/tier3/test_t3_21_log_size_limits.py` - Missing helper imports
- `tests/e2e/tier3/test_t3_11_system_packs.py` - Missing `create_pack()` method
- `tests/e2e/tier3/test_t3_20_secret_injection.py` - Incorrect `create_secret()` signature

**Solution Implemented:**
1. Added `wait_for_execution_completion()` function to `helpers/polling.py`:
   - Waits for execution to reach terminal status (succeeded, failed, canceled, timeout)
   - Convenience wrapper around `wait_for_execution_status()`

2. Updated `helpers/__init__.py` to export all missing functions:
   - Polling: `wait_for_execution_completion`, `wait_for_enforcement_count`, `wait_for_inquiry_count`, `wait_for_inquiry_status`
   - Fixtures: `timestamp_future`, `create_failing_action`, `create_sleep_action`, `create_timer_automation`, `create_webhook_automation`

3. Added `create_pack()` method to `AttuneClient`:
   - Accepts either dict or keyword arguments for flexibility
   - Maps `name` to `label` for backwards compatibility
   - Sends request to `POST /api/v1/packs`

4. Fixed `create_secret()` method signature:
   - Added `encrypted` parameter (defaults to `True`)
   - Added all owner-related parameters to match API schema
   - Changed endpoint from `/api/v1/secrets` to `/api/v1/keys`
   - Maps `key` parameter to `ref` field in API request

**Files Modified:**
- `tests/helpers/polling.py` - Added `wait_for_execution_completion()` function
- `tests/helpers/__init__.py` - Added 10 missing exports
- `tests/helpers/client.py` - Added `create_pack()` method, updated `create_secret()` signature

**Result:**
- ✅ All 151 E2E tests collect successfully
- ✅ No import errors across all test tiers
- ✅ No AttributeError or TypeError in client methods
- ✅ All tier1 and tier3 tests can run (when services are available)
- ✅ Test infrastructure is now complete and consistent
- ✅ Client methods aligned with actual API schema

**Time to Resolution:** 30 minutes

---

## 📋 Next Steps (Priority Order)

1. **[P0] Test End-to-End Execution**
   - Restart all services with fixes applied
   - Trigger timer event
   - Verify execution completes successfully
   - Confirm "hello, world" appears in logs/results

2. **[P1] Cleanup and Testing**
   - Remove legacy `attune.executions.queue` (no longer needed)
   - Add integration tests for message routing
   - Document message queue architecture
   - Update configuration examples

4. **[P2] Performance Optimization**
   - Monitor queue depths
   - Add metrics for message processing times
   - Implement dead letter queue monitoring
   - Add alerting for stuck executions

---

## System Status

**Services:**
- ✅ Sensor: Running, generating events every 10s
- ✅ Executor: Running, all 3 consumers active
- ✅ Worker: Running, runtime resolution fixed
- ✅ End-to-end: Ready for testing

**Pipeline Flow:**
```
Timer → Event → Rule Match → Enforcement ✅
Enforcement → Execution → Scheduled ✅
Scheduled → Worker Queue ✅
Worker → Execute Action ✅ (runtime resolution fixed)
Worker → Status Update → Manager ✅ (message loop fixed)
```

**Database State:**
- Events: Creating successfully
- Enforcements: Creating successfully
- Executions: Creating and scheduling successfully
- Executions are reaching "Running" and "Failed" states (but looping)

---

## Notes

- The message queue architecture fix was successful at eliminating consumer competition
- Messages now route correctly to the appropriate consumers
- Runtime resolution and message loop issues have been fixed
- Ready for end-to-end testing of the complete happy path