# Work Session: Dependency Upgrade Fixes

**Date:** 2026-01-17  
**Session:** Session 5b - Fixing Breaking Changes  
**Status:** ✅ Complete

---

## Objective

Fix compilation errors introduced by upgrading dependencies to their latest versions. User upgraded some dependencies manually which introduced breaking changes that needed to be addressed.

---

## Breaking Changes Encountered

### 1. jsonwebtoken 10.2.0 - Missing Crypto Backend Feature

**Error:**
```
error: at least one of the features "rust_crypto" or "aws_lc_rs" must be enabled
```

**Cause:** jsonwebtoken 10.x requires an explicit crypto backend feature to be enabled.

**Fix:** Added `rust_crypto` feature to jsonwebtoken dependency
```toml
# Before
jsonwebtoken = "10.2"

# After
jsonwebtoken = { version = "10.2", features = ["rust_crypto"] }
```

**File:** `crates/api/Cargo.toml`

---

### 2. jsonschema 0.38.1 - API Changes

**Error:**
```
error[E0433]: failed to resolve: could not find `JSONSchema` in `jsonschema`
```

**Cause:** jsonschema upgraded from 0.18 to 0.38, with significant API changes:
- `JSONSchema::compile()` renamed to `validator_for()`
- Error handling structure changed
- Iterator methods changed

**Fixes Applied:**

```rust
// Before (0.18)
let compiled = jsonschema::JSONSchema::compile(&self.schema)
    .map_err(|e| Error::schema_validation(format!("Failed to compile schema: {}", e)))?;

if let Err(errors) = compiled.validate(data) {
    let error_messages: Vec<String> = errors
        .map(|e| format!("{} at {}", e, e.instance_path))
        .collect();
    return Err(Error::schema_validation(error_messages.join(", ")));
}

// After (0.38)
let compiled = jsonschema::validator_for(&self.schema)
    .map_err(|e| Error::schema_validation(format!("Failed to compile schema: {}", e)))?;

if let Err(error) = compiled.validate(data) {
    return Err(Error::schema_validation(format!(
        "Validation failed: {}",
        error
    )));
}
```

**File:** `crates/common/src/schema.rs`

---

### 3. utoipa 5.4 - ToSchema Trait Requirements

**Error:**
```
error[E0277]: the trait `ComposeSchema` is not implemented for `EnforcementStatus`
error[E0277]: the trait `ComposeSchema` is not implemented for `ExecutionStatus`
... (and other enum types)
```

**Cause:** utoipa 5.x changed trait requirements - all types used in OpenAPI schemas must derive `ToSchema`.

**Fixes Applied:**

1. Added utoipa to workspace dependencies:
```toml
# Cargo.toml
[workspace.dependencies]
utoipa = { version = "5.4", features = ["chrono", "uuid"] }
```

2. Added utoipa to common crate:
```toml
# crates/common/Cargo.toml
utoipa = { workspace = true }
```

3. Updated all enum types to derive `ToSchema`:
```rust
// Before
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum EnforcementStatus { ... }

// After
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
pub enum EnforcementStatus { ... }
```

**Enums Updated:**
- `RuntimeType`
- `WorkerType`
- `WorkerStatus`
- `EnforcementStatus`
- `EnforcementCondition`
- `ExecutionStatus`
- `InquiryStatus`
- `PolicyMethod`
- `OwnerType`
- `NotificationState`
- `ArtifactType`

**File:** `crates/common/src/models.rs`

---

### 4. axum 0.8 - async_trait Re-export Removed

**Error:**
```
error[E0433]: failed to resolve: could not find `async_trait` in `axum`
```

**Cause:** axum 0.8 removed the re-export of `async_trait` macro. Must use the crate directly.

**Fixes Applied:**

1. Removed `#[axum::async_trait]` attribute
2. axum 0.8's `FromRequestParts` trait now natively supports async without needing the macro

```rust
// Before
#[axum::async_trait]
impl axum::extract::FromRequestParts<crate::state::SharedState> for RequireAuth {
    type Rejection = AuthError;
    async fn from_request_parts(...) -> Result<Self, Self::Rejection> { ... }
}

// After (axum 0.8 handles async natively)
impl axum::extract::FromRequestParts<crate::state::SharedState> for RequireAuth {
    type Rejection = AuthError;
    async fn from_request_parts(...) -> Result<Self, Self::Rejection> { ... }
}
```

**File:** `crates/api/src/auth/middleware.rs`

---

## Additional Dependencies Upgraded (by user)

Beyond our original upgrade, the user upgraded:
- **axum**: 0.7 → 0.8
- **lapin**: 2.5 → 3.7
- **redis**: 0.27 → 1.0
- **jsonschema**: 0.18 → 0.38

---

## Files Modified

1. `Cargo.toml` - Added utoipa to workspace dependencies
2. `crates/api/Cargo.toml` - Added `rust_crypto` feature to jsonwebtoken
3. `crates/common/Cargo.toml` - Added utoipa dependency
4. `crates/common/src/schema.rs` - Fixed jsonschema 0.38 API usage
5. `crates/common/src/models.rs` - Added ToSchema derive to 11 enum types
6. `crates/api/src/auth/middleware.rs` - Removed async_trait macro (no longer needed)

---

## Compilation Results

### Final Status: ✅ SUCCESS

```bash
$ cargo build
   Compiling attune-common v0.1.0
   Compiling attune-sensor v0.1.0
   Compiling attune-executor v0.1.0
   Compiling attune-worker v0.1.0
   Compiling attune-api v0.1.0
   Compiling attune-notifier v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.00s
```

**Result:** All packages compile successfully with only pre-existing warnings (unused code).

---

## Key Learnings

### 1. Major Version Upgrades Require Careful Review

Major version changes (jsonschema 0.18 → 0.38, axum 0.7 → 0.8) often include breaking API changes that require code modifications.

### 2. Feature Flags Matter

Some crates (like jsonwebtoken) now require explicit feature selection for security/cryptography backends. Always check changelog for new feature requirements.

### 3. Trait Requirements Can Change

OpenAPI/schema libraries may change trait requirements (utoipa requiring ToSchema). These changes cascade through codebases using type system features.

### 4. Macro Re-exports Are Unstable

Framework re-exports of third-party macros (like axum re-exporting async_trait) may be removed in major versions. Better to depend on macros directly if possible.

### 5. Error Handling APIs Evolve

Validation libraries may simplify or change their error handling patterns (jsonschema returning single error vs error iterator).

---

## Testing Recommendations

Despite successful compilation, the following should be tested:

1. **JWT Authentication**
   - Test with rust_crypto backend
   - Verify token signing and verification
   - Check performance compared to previous version

2. **JSON Schema Validation**
   - Test all schema validation paths
   - Verify error messages are still useful
   - Test edge cases (nested objects, arrays, etc.)

3. **OpenAPI Documentation**
   - Verify Swagger UI still works
   - Check that all enum types appear correctly in schema
   - Test API documentation completeness

4. **Axum Extractors**
   - Test RequireAuth middleware
   - Verify authentication flows work correctly
   - Test error handling for auth failures

5. **Message Queue & Redis**
   - Test RabbitMQ connections (lapin 3.7)
   - Test Redis pub/sub and caching (redis 1.0)
   - Verify no performance regressions

---

## Migration Checklist for Future Upgrades

When upgrading major dependency versions:

- [ ] Review CHANGELOG for breaking changes
- [ ] Check for new required feature flags
- [ ] Look for renamed functions/types
- [ ] Verify trait implementations still match
- [ ] Check for removed re-exports
- [ ] Test error handling paths
- [ ] Verify macro expansions (if using proc macros)
- [ ] Run full test suite
- [ ] Update documentation if APIs changed
- [ ] Test integration points thoroughly

---

## Summary

Successfully resolved all compilation errors introduced by dependency upgrades. The project now compiles with:
- jsonwebtoken 10.2.0 (with rust_crypto backend)
- jsonschema 0.38.1 (with updated API)
- utoipa 5.4 (with ToSchema derives on all enums)
- axum 0.8 (with native async support)
- lapin 3.7
- redis 1.0

All breaking changes were addressed with minimal code modifications. The codebase is now up-to-date with the latest ecosystem standards and ready for testing.

**Next Steps:**
1. Run full test suite to verify functionality
2. Perform integration testing with all services
3. Test end-to-end automation flows
4. Monitor for any runtime issues or deprecation warnings

---

**Session Duration:** ~45 minutes  
**Errors Fixed:** 5 compilation errors  
**Files Modified:** 6 files  
**Lines Changed:** ~30 lines

**Status:** ✅ All compilation errors resolved. Ready for testing.