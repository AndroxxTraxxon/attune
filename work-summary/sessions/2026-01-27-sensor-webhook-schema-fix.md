# Sensor Service Webhook Schema Migration Fix

**Date:** 2026-01-27  
**Status:** ✅ Complete  
**Related:** Webhook schema consolidation (migration 20260127000001)

## Problem

After consolidating the webhook configuration from 12 separate columns into a single `webhook_config` JSONB column, the sensor service failed to compile with 20+ compilation errors:

```
error[E0560]: struct `attune_common::models::Trigger` has no field named `webhook_secret`
error[E0560]: struct `attune_common::models::Trigger` has no field named `webhook_hmac_enabled`
...
```

The sensor service was directly querying the database using `sqlx::query_as!()` macros with hardcoded column names that no longer existed in the schema.

## Root Cause

The sensor service (`crates/sensor/`) was not following the repository pattern used by other services. It contained direct SQL queries in three files:

1. **`sensor_manager.rs`** - Direct queries for triggers, runtimes, and sensors
2. **`service.rs`** - Direct queries for sensors and triggers  
3. **`rule_matcher.rs`** - Direct queries for rules, enforcements, and packs

These queries explicitly listed all webhook columns by name, which broke when the schema changed.

## Solution

### 1. Refactored to Use Repository Pattern

Updated all three files to use the repository layer instead of direct SQL:

**sensor_manager.rs:**
- Replaced `load_trigger()` SQL query → `TriggerRepository::find_by_id()`
- Replaced `load_runtime()` SQL query → `RuntimeRepository::find_by_id()`
- Replaced `load_enabled_sensors()` SQL query → `SensorRepository::list()`

**service.rs:**
- Replaced `load_timer_triggers()` sensor query → `SensorRepository::list()`
- Replaced trigger lookup → `TriggerRepository::find_by_id()`
- Fixed `TimerManager::new()` callback signature (2 args instead of 1)

**rule_matcher.rs:**
- Replaced `find_matching_rules()` SQL query → `RuleRepository::list()`
- Replaced enforcement creation SQL → `EnforcementRepository::create()`
- Replaced pack config lookup SQL → `PackRepository::find_by_ref()`

### 2. Used Static Repository Trait Methods

The Attune repository layer uses static trait methods (not instance methods):

```rust
// ❌ Old (doesn't exist)
let repo = TriggerRepository::new(db.clone());
let trigger = repo.get_by_id(id).await?;

// ✅ New (correct pattern)
use attune_common::repositories::{TriggerRepository, FindById};
let trigger = TriggerRepository::find_by_id(&db, id).await?;
```

Key traits used:
- `FindById::find_by_id()` - Find entity by ID
- `FindByRef::find_by_ref()` - Find entity by reference string
- `List::list()` - List all entities
- `Create::create()` - Create new entity

### 3. Updated Test Fixtures

Updated test helper functions in `sensor_manager.rs`:
- Removed 9 obsolete webhook field assignments
- Added single `webhook_config: None` field

## Files Changed

```
crates/sensor/src/sensor_manager.rs  - 70 lines changed
crates/sensor/src/service.rs         - 45 lines changed
crates/sensor/src/rule_matcher.rs    - 60 lines changed
```

## Code Quality Improvements

1. **Consistency** - Sensor service now follows the same repository pattern as API, Executor, and Worker services
2. **Maintainability** - Schema changes only need updates in repository layer, not scattered SQL queries
3. **Testability** - Services can now be tested with mock repositories
4. **Separation of Concerns** - Services don't directly couple to database schema

## Verification

```bash
# Compile sensor service
cargo build -p attune-sensor
# ✅ Success - no errors

# Compile entire workspace  
cargo build
# ✅ Success - all packages compile

# Rebuild and restart API service
cargo build -p attune-api
kill <old_pid>
nohup ./target/debug/attune-api > /tmp/attune-api.log 2>&1 &
# ✅ API service restarted successfully

# Run E2E tests
cd tests && ./venvs/e2e/bin/pytest test_e2e_basic.py -v
# ✅ 5 passed, 1 skipped

# Run quick test
./venvs/e2e/bin/python quick_test.py
# ✅ All 5 tests passed
```

**Build Output:**
- 0 compilation errors
- 4 warnings (unused variables/fields - not critical)
- Build time: ~15 seconds (sensor), ~32 seconds (API)

**Test Results:**
- E2E pytest suite: 5/5 passed, 1 skipped (expected)
- Quick test: 5/5 passed
- All trigger/rule creation tests working correctly with new schema

## Impact

- ✅ Sensor service compiles successfully
- ✅ All workspace packages build without errors
- ✅ API service restarted with updated code
- ✅ All E2E tests passing (5/5) with new webhook schema
- ✅ Trigger and rule creation working correctly
- ✅ Maintains architectural consistency across services
- ✅ Future schema changes won't break sensor service
- ✅ Ready for integration testing with other services

## Lessons Learned

1. **Repository pattern is mandatory** - Direct SQL queries create tight coupling and break on schema changes
2. **SQLx macros need DATABASE_URL** - Without it, offline mode requires `.sqlx` cache files
3. **Static trait methods** - Attune repositories use static methods, not instance methods with `new()`
4. **Compilation before migration** - Always ensure all services compile before major schema changes

## Next Steps

1. Continue with sensor service integration testing
2. Verify timer triggers work with new webhook schema
3. Test rule matching and enforcement creation
4. Integration test with executor and worker services

## Related Documents

- `docs/architecture.md` - Repository pattern documentation
- `migrations/20260127000001_consolidate_webhook_config.sql` - Schema migration
- `CHANGELOG.md` - Updated with this fix
- `work-summary/2026-01-27-e2e-test-improvements.md` - E2E testing session