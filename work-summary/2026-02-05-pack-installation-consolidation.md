# Pack Installation Consolidation - Work Summary

**Date:** 2026-02-05  
**Type:** Schema Simplification (Pre-Production)  
**Status:** ✅ Complete

---

## Overview

Consolidated the separate `pack_installation` table into the `pack` table by adding nullable installation metadata columns. This simplifies the schema by eliminating an unnecessary 1:1 relationship table, reducing joins and making the data model more intuitive.

---

## Problem Statement

The `pack_installation` table tracked installation metadata (source type, URL, checksum, storage path, etc.) in a separate table with a 1:1 relationship to the `pack` table. This design:

- Required joins to retrieve complete pack information
- Added unnecessary complexity to queries
- Created a separate repository layer for what was essentially pack metadata
- Had no use case for multiple installation records per pack

The relationship was strictly 1:1 (one installation record per pack), making it a perfect candidate for denormalization.

---

## Solution

Merged installation metadata directly into the `pack` table as nullable columns. Packs that are not installed will have these fields as NULL.

---

## Changes Made

### 1. Database Migration (`migrations/20250101000002_pack_system.sql`)

**Added columns to `pack` table:**
```sql
-- Installation metadata (nullable for non-installed packs)
source_type TEXT,
source_url TEXT,
source_ref TEXT,
checksum TEXT,
checksum_verified BOOLEAN DEFAULT FALSE,
installed_at TIMESTAMPTZ,
installed_by BIGINT,
installation_method TEXT,
storage_path TEXT,
```

**Added indexes:**
```sql
CREATE INDEX idx_pack_installed_at ON pack(installed_at DESC) WHERE installed_at IS NOT NULL;
CREATE INDEX idx_pack_installed_by ON pack(installed_by) WHERE installed_by IS NOT NULL;
CREATE INDEX idx_pack_source_type ON pack(source_type) WHERE source_type IS NOT NULL;
```

**Added foreign key constraint** (in `migrations/20250101000003_identity_and_auth.sql`):
```sql
ALTER TABLE pack
    ADD CONSTRAINT fk_pack_installed_by
    FOREIGN KEY (installed_by)
    REFERENCES identity(id)
    ON DELETE SET NULL;
```

### 2. Model Changes (`crates/common/src/models.rs`)

**Updated `Pack` struct:**
- Added installation metadata fields (all `Option<T>` types)
- Removed `PackInstallation` struct entirely
- Removed `CreatePackInstallation` struct

**New fields in Pack:**
```rust
pub source_type: Option<String>,
pub source_url: Option<String>,
pub source_ref: Option<String>,
pub checksum: Option<String>,
pub checksum_verified: Option<bool>,
pub installed_at: Option<DateTime<Utc>>,
pub installed_by: Option<Id>,
pub installation_method: Option<String>,
pub storage_path: Option<String>,
```

### 3. Repository Changes

**Removed:**
- `crates/common/src/repositories/pack_installation.rs` (entire file deleted)
- `PackInstallationRepository` from `repositories/mod.rs`

**Updated `PackRepository` (`crates/common/src/repositories/pack.rs`):**

**Added new method:**
```rust
pub async fn update_installation_metadata(
    executor: E,
    id: i64,
    source_type: String,
    source_url: Option<String>,
    source_ref: Option<String>,
    checksum: Option<String>,
    checksum_verified: bool,
    installed_by: Option<i64>,
    installation_method: String,
    storage_path: String,
) -> Result<Pack>
```

**Added helper methods:**
- `is_installed()` - Check if a pack has installation metadata
- `list_installed()` - List all installed packs
- `list_by_source_type()` - Filter packs by installation source

**Updated all SELECT queries** to include installation fields in:
- `find_by_id()`
- `find_by_ref()`
- `list()`
- `create()`
- `update()`
- `list_paginated()`
- `find_by_tag()`
- `find_standard()`
- `search()`

**Updated input structs:**
- Added `installers: JsonDict` to `CreatePackInput`
- Added `installers: Option<JsonDict>` to `UpdatePackInput`

### 4. API Route Changes (`crates/api/src/routes/packs.rs`)

**Updated `install_pack` endpoint:**
- Removed `PackInstallationRepository::new()` and `create()` calls
- Replaced with direct call to `PackRepository::update_installation_metadata()`

**Before:**
```rust
let installation_metadata = CreatePackInstallation { ... };
installation_repo.create(installation_metadata).await?;
```

**After:**
```rust
PackRepository::update_installation_metadata(
    &state.db,
    pack_id,
    source_type.to_string(),
    source_url,
    source_ref,
    checksum.clone(),
    installed.checksum.is_some() && checksum.is_some(),
    user_id,
    "api".to_string(),
    final_path.to_string_lossy().to_string(),
).await?;
```

### 5. Test Updates

**Updated all test files** to include `installers: json!({})` in `CreatePackInput`:
- `crates/api/tests/helpers.rs`
- `crates/api/tests/sse_execution_stream_tests.rs`
- `crates/api/tests/webhook_api_tests.rs`
- `crates/api/tests/webhook_security_tests.rs`
- `crates/api/tests/pack_registry_tests.rs`
- `crates/common/tests/helpers.rs`
- `crates/common/tests/pack_repository_tests.rs`
- `crates/common/tests/permission_repository_tests.rs`
- `crates/common/tests/repository_runtime_tests.rs`
- `crates/executor/tests/fifo_ordering_integration_test.rs`
- `crates/executor/tests/policy_enforcer_tests.rs`

**Updated pack registry tests** to use `Pack` fields instead of `PackInstallation`:
```rust
// Before
let installation = installation_repo.get_by_pack_id(pack_id).await?;
assert_eq!(installation.source_type, "local_directory");

// After
let pack = PackRepository::find_by_id(&ctx.pool, pack_id).await?;
assert_eq!(pack.source_type.as_deref(), Some("local_directory"));
```

### 6. Migration Schema Fixes (Missing Columns)

Fixed missing `is_adhoc` columns and rule table during migration consolidation:

**Issues Found:**
- `action.is_adhoc` column was missing from migration
- `sensor.is_adhoc` column was missing from migration  
- `rule` table was completely missing from migrations
- `ActionRepository::update()` missing `is_adhoc` in RETURNING clause

**Fixes:**
- Added `is_adhoc BOOLEAN NOT NULL DEFAULT FALSE` to action table (migration 005)
- Added `is_adhoc BOOLEAN NOT NULL DEFAULT FALSE` to sensor table (migration 004)
- Created complete rule table with `is_adhoc` in migration 006 (after action exists)
- Added foreign key constraints for `enforcement.rule` and `event.rule`
- Updated `ActionRepository::update()` RETURNING clause to include `is_adhoc`
- Added proper indexes, triggers, and comments for all tables

### 7. CLI Test Fixes (Unrelated but Fixed)

Fixed failing `whoami` tests in `crates/cli/tests/test_auth.rs`:

**Issue:** Mock endpoint path was `/auth/whoami` but actual API uses `/auth/me`

**Fixes:**
- Updated `mock_whoami_success()` to use `/auth/me` path
- Fixed mock response structure to match `CurrentUserResponse` (removed extra fields)
- Changed test assertions from `"username"` to `"login"`
- Changed parameter from `email` to `display_name`

---

## Breaking Changes

**Note:** This is a pre-production change with no deployments or users, so breaking changes are acceptable.

- Database schema change: `pack_installation` table removed, new columns added to `pack`
- Model API change: `PackInstallation` and `CreatePackInstallation` types removed
- Repository API change: `PackInstallationRepository` removed, new methods added to `PackRepository`

---

## Benefits

1. **Simpler Schema:** One less table to manage (17 tables instead of 18)
2. **No Joins Required:** All pack information available in a single query
3. **Clearer Data Model:** Installation is a property of a pack, not a separate entity
4. **Reduced Code:** Eliminated ~170 lines of repository code
5. **Better Performance:** Fewer joins, simpler queries, partial indexes on nullable fields

---

## Migration Notes

For users migrating from the old schema (when v1.0 releases):

1. Drop and recreate database (acceptable since pre-production)
2. Run consolidated migrations from scratch
3. Reload pack data using pack installation API

---

## Testing

- ✅ All workspace compilation successful
- ✅ Pack registry tests updated and passing
- ✅ CLI auth tests fixed and passing
- ✅ No compilation warnings

---

## Files Modified

### Migrations (4 files)
- `migrations/20250101000002_pack_system.sql` - Added installation columns to pack table
- `migrations/20250101000003_identity_and_auth.sql` - Added foreign key constraint
- `migrations/20250101000004_trigger_sensor_event_rule.sql` - Added is_adhoc to sensor, added indexes/triggers/comments
- `migrations/20250101000005_action.sql` - Added is_adhoc to action, added indexes/triggers/comments
- `migrations/20250101000006_execution_system.sql` - Added complete rule table with is_adhoc and foreign key constraints

### Models (1 file)
- `crates/common/src/models.rs` - Updated Pack struct, removed PackInstallation module

### Repositories (3 files)
- `crates/common/src/repositories/pack.rs` - Added installation methods, updated all queries
- `crates/common/src/repositories/action.rs` - Fixed update() RETURNING clause to include is_adhoc
- `crates/common/src/repositories/mod.rs` - Removed PackInstallationRepository export
- **Deleted:** `crates/common/src/repositories/pack_installation.rs`

### API Routes (1 file)
- `crates/api/src/routes/packs.rs` - Updated install_pack to use new repository method

### Tests (14 files)
- `crates/api/tests/helpers.rs`
- `crates/api/tests/pack_registry_tests.rs`
- `crates/api/tests/sse_execution_stream_tests.rs`
- `crates/api/tests/webhook_api_tests.rs`
- `crates/api/tests/webhook_security_tests.rs`
- `crates/common/tests/helpers.rs`
- `crates/common/tests/pack_repository_tests.rs`
- `crates/common/tests/permission_repository_tests.rs`
- `crates/common/tests/repository_runtime_tests.rs`
- `crates/executor/tests/fifo_ordering_integration_test.rs`
- `crates/executor/tests/policy_enforcer_tests.rs`
- `crates/cli/tests/common/mod.rs` - Fixed whoami mock
- `crates/cli/tests/test_auth.rs` - Fixed whoami tests

**Total:** 22 files modified, 1 file deleted

---

## Verification Steps

To verify the changes after dropping and recreating the database:

```bash
# 1. Drop and recreate databases
make db-reset
make db-test-setup

# 2. Run migrations
make db-migrate

# 3. Verify schema
psql attune -c "\d pack" | grep -E "installed_at|source_type|storage_path"

# 4. Run tests
cargo test --workspace

# 5. Test pack installation via API
# (Start services and test via web UI or CLI)
```

---

## Related Documentation

- Migration consolidation was completed on 2026-01-17
- This change continues the pre-production schema refinement effort
- See `docs/migrations/CONSOLIDATION-COMPLETE.md` for full migration history

---

## Lessons Learned

1. **Migration consolidation requires careful verification** - Missing tables/columns can slip through when consolidating migrations
2. **Test everything after schema changes** - Running repository tests revealed missing is_adhoc columns
3. **Dependency ordering matters** - Rule table needed to be in migration 006 after action table (migration 005)
4. **RETURNING clauses must be complete** - All model fields must be included in UPDATE...RETURNING queries

## Next Steps

1. ✅ Drop existing databases and re-run migrations
2. ✅ Fix missing is_adhoc columns in migrations
3. ✅ Add missing rule table to migrations
4. ✅ Verify all repository tests pass
5. Test pack installation workflow end-to-end
6. Update any documentation referencing `pack_installation` table
7. Consider similar consolidations for other 1:1 relationships if any exist