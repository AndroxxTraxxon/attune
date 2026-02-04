# Password Hash Column Fix

**Date:** 2026-01-18  
**Status:** ✅ Complete  
**Priority:** P1 - Security/Data Model Integrity

---

## Summary

Fixed the authentication system to properly use the dedicated `password_hash` column on the `identity` table instead of storing password hashes in the `attributes` JSON field. This is a critical fix for data model integrity and performance.

---

## Problem

The API authentication system was storing password hashes in the `attributes` JSONB field:

```json
{
  "email": "user@example.com",
  "password_hash": "$argon2id$v=19$m=19456,t=2,p=1$..."
}
```

**Issues:**
- ❌ Dedicated `password_hash` column in database schema was unused
- ❌ Querying on JSON fields is slower than indexed columns
- ❌ Password hashes mixed with other user metadata
- ❌ Inconsistent with database schema design
- ❌ Type safety concerns with JSON field access

---

## Solution

Updated all authentication code to use the `password_hash` column directly:

### 1. Model Changes

**File:** `crates/common/src/models.rs`

```rust
pub struct Identity {
    pub id: Id,
    pub login: String,
    pub display_name: Option<String>,
    pub password_hash: Option<String>,  // ✅ Added
    pub attributes: JsonDict,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}
```

### 2. Repository Changes

**File:** `crates/common/src/repositories/identity.rs`

**CreateIdentityInput:**
```rust
pub struct CreateIdentityInput {
    pub login: String,
    pub display_name: Option<String>,
    pub password_hash: Option<String>,  // ✅ Added
    pub attributes: JsonDict,
}
```

**UpdateIdentityInput:**
```rust
pub struct UpdateIdentityInput {
    pub display_name: Option<String>,
    pub password_hash: Option<String>,  // ✅ Added
    pub attributes: Option<JsonDict>,
}
```

**All SQL queries updated to include `password_hash` column:**
- `find_by_id` - Added to SELECT
- `find_by_login` - Added to SELECT
- `list` - Added to SELECT
- `create` - Added to INSERT
- `update` - Added to UPDATE with query builder

### 3. API Changes

**File:** `crates/api/src/routes/auth.rs`

**Login Endpoint:**
```rust
// Before: Reading from attributes JSON
let password_hash = identity
    .attributes
    .get("password_hash")
    .and_then(|v| v.as_str())
    .ok_or_else(...)?;

// After: Reading from column
let password_hash = identity
    .password_hash
    .as_ref()
    .ok_or_else(...)?;
```

**Register Endpoint:**
```rust
// Before: Storing in attributes JSON
let mut attrs = serde_json::Map::new();
attrs.insert("password_hash".to_string(), json!(password_hash));

// After: Storing in column
let input = CreateIdentityInput {
    login: payload.login.clone(),
    display_name: payload.display_name,
    password_hash: Some(password_hash),  // ✅ Column
    attributes: serde_json::json!({}),
};
```

**Change Password Endpoint:**
```rust
// Before: Raw SQL update of attributes
sqlx::query("UPDATE identities SET attributes = $1...")
    .bind(&attributes)
    .execute(&state.db)
    .await?;

// After: Using repository with proper input
let update_input = UpdateIdentityInput {
    display_name: None,
    password_hash: Some(new_password_hash),  // ✅ Column
    attributes: None,
};
IdentityRepository::update(&state.db, identity_id, update_input).await?;
```

### 4. Setup Script Changes

**File:** `scripts/setup-e2e-db.sh`

```sql
-- Before: Hash in attributes JSON
INSERT INTO attune.identity (login, display_name, attributes)
VALUES (
    'e2e_test_user',
    'E2E Test User',
    jsonb_build_object('password_hash', '$HASH', ...)
);

-- After: Hash in column
INSERT INTO attune.identity (login, display_name, password_hash, attributes)
VALUES (
    'e2e_test_user',
    'E2E Test User',
    '$HASH',  -- ✅ Dedicated column
    jsonb_build_object('email', 'e2e@test.local', ...)
);
```

---

## Files Modified

1. `crates/common/src/models.rs` - Added `password_hash` field to `Identity`
2. `crates/common/src/repositories/identity.rs` - Updated all queries and inputs
3. `crates/api/src/routes/auth.rs` - Fixed login, register, change_password
4. `scripts/setup-e2e-db.sh` - Fixed test user creation

---

## Verification

### Database Schema
```sql
-- Verify password_hash column usage
SELECT 
    id, 
    login, 
    password_hash IS NOT NULL as has_hash,
    attributes ? 'password_hash' as hash_in_attrs 
FROM attune.identity;

-- Result:
-- id | login          | has_hash | hash_in_attrs
-- 1  | e2e_test_user  | t        | f           ✅
-- 2  | test_user_2    | t        | f           ✅
```

### Authentication Tests

**Login Test:**
```bash
curl -X POST http://127.0.0.1:18080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"login":"e2e_test_user","password":"test_password_123"}'
# ✅ Returns JWT tokens
```

**Register Test:**
```bash
curl -X POST http://127.0.0.1:18080/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"login":"test_user_2","password":"password123","display_name":"Test User 2"}'
# ✅ Creates user with password_hash in column
```

### Unit Tests
```bash
cargo test -p attune-common --lib
# ✅ 96 tests passed

cargo test -p attune-api --lib
# ✅ 46 tests passed
```

---

## Benefits

1. **Performance:** Direct column access is faster than JSON field extraction
2. **Indexing:** Can create index on `password_hash` column if needed
3. **Type Safety:** Rust type system enforces `Option<String>` instead of JSON value
4. **Clarity:** Password hash storage is explicit in the data model
5. **Consistency:** Aligns with database schema design
6. **Separation:** Authentication data separate from user metadata

---

## Migration Notes

**For Production Deployments:**

If you have existing identities with passwords in attributes, run this migration:

```sql
-- Move password_hash from attributes to column
UPDATE attune.identity 
SET password_hash = attributes->>'password_hash'
WHERE attributes ? 'password_hash' 
  AND password_hash IS NULL;

-- Clean up attributes (optional)
UPDATE attune.identity
SET attributes = attributes - 'password_hash'
WHERE attributes ? 'password_hash';
```

**For Development/Testing:**

Simply recreate the E2E database:
```bash
./scripts/setup-e2e-db.sh
```

---

## Security Considerations

- ✅ Password hashes remain Argon2id encrypted
- ✅ Same verification logic (no functional changes)
- ✅ Column is `Option<String>` (nullable) for identities without passwords
- ✅ All existing security measures preserved
- ✅ No plaintext passwords ever stored

---

## Related Files

- Database schema: `migrations/20250101000001_initial_setup.sql`
- Identity model: `crates/common/src/models.rs`
- Identity repository: `crates/common/src/repositories/identity.rs`
- Auth routes: `crates/api/src/routes/auth.rs`
- E2E setup: `scripts/setup-e2e-db.sh`

---

**Status:** Production-ready. All tests passing. Authentication verified with proper column usage.
