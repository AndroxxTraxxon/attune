# Workspace Dependency Compliance Audit

**Date:** 2026-01-28  
**Status:** ✅ Complete

## Overview

This document records the results of a comprehensive audit of all `Cargo.toml` files in the Attune workspace to ensure proper use of workspace dependencies. The goal was to ensure that when crates use dependencies declared in the workspace root, they consistently use `{ workspace = true }` instead of declaring version numbers directly.

## Audit Scope

All crates in the workspace were examined:
- `crates/common`
- `crates/api`
- `crates/executor`
- `crates/sensor`
- `crates/notifier`
- `crates/worker`
- `crates/cli`

## Issues Found & Fixed

### 1. attune-api: Direct argon2 Version

**Issue:** The API crate was declaring `argon2 = "0.5"` directly instead of using the workspace version.

**Before:**
```toml
argon2 = "0.5"
```

**After:**
```toml
argon2 = { workspace = true }
```

**Impact:** Ensures consistent argon2 version across all crates and simplifies dependency management.

---

### 2. attune-worker: Formatting Issue

**Issue:** The worker crate had inconsistent spacing in the workspace reference for `base64`.

**Before:**
```toml
base64 = {workspace = true}
```

**After:**
```toml
base64 = { workspace = true }
```

**Impact:** Improves code consistency and readability.

---

### 3. attune-cli: Redundant reqwest Features

**Issue:** The CLI crate was explicitly declaring features for `reqwest` that were already present in the workspace definition.

**Before:**
```toml
reqwest = { workspace = true, features = ["json"] }
```

**Workspace Definition:**
```toml
reqwest = { version = "0.13", features = ["json"] }
```

**After:**
```toml
reqwest = { workspace = true }
```

**Impact:** Eliminates redundancy and prevents confusion about which features are actually being used.

---

### 4. attune-api: utoipa Feature Extension

**Issue:** The API crate needed the `"axum_extras"` feature for `utoipa` in addition to the workspace's base features (`"chrono"`, `"uuid"`).

**Before:**
```toml
utoipa = { version = "5.4", features = ["axum_extras", "chrono", "uuid"] }
```

**After:**
```toml
utoipa = { workspace = true, features = ["axum_extras"] }
```

**Impact:** Now inherits base features from workspace and only adds the API-specific feature, following Cargo's feature inheritance pattern.

---

## Dependencies Properly Using workspace = true

The following patterns were found to be correct and idiomatic:

### Feature Extension Pattern (Correct)

**attune-cli: clap with additional features**
```toml
clap = { workspace = true, features = ["derive", "env", "string"] }
```

Workspace has:
```toml
clap = { version = "4.5", features = ["derive"] }
```

This pattern is **correct** - the CLI crate inherits the `"derive"` feature from the workspace and adds `"env"` and `"string"`. This is the idiomatic way to extend workspace dependency features in Cargo.

## Crate-Specific Dependencies (Allowed)

The audit identified 25 crate-specific dependencies that are not in the workspace. These are expected and allowed because they are only used by specific crates:

- `jsonwebtoken` (api, cli)
- `rand` (api)
- `hmac`, `sha1`, `hex` (api)
- `utoipa-swagger-ui` (api)
- `dirs`, `urlencoding`, `colored`, `comfy-table`, `indicatif`, `dialoguer` (cli)
- `wiremock`, `assert_cmd`, `predicates`, `mockito`, `tokio-test` (cli dev-dependencies)
- `tera` (executor)
- `criterion` (executor dev-dependency)
- `cron` (sensor)
- `hostname` (worker)
- `async-recursion` (common)

## Verification

All changes were verified using:

1. **Build Check:**
   ```bash
   cargo check --workspace
   ```
   Result: ✅ Success

2. **Workspace Dependency Compliance Script:**
   ```bash
   ./scripts/check-workspace-deps.sh
   ```
   Result: ✅ All crates use workspace dependencies correctly (25 allowed exceptions)

3. **Test Suite:**
   ```bash
   cargo test --workspace --lib
   ```
   Result: ✅ All tests pass (220 tests across all crates)

## Summary

- **Total Issues Fixed:** 4
- **Files Modified:** 3 (`crates/api/Cargo.toml`, `crates/worker/Cargo.toml`, `crates/cli/Cargo.toml`)
- **Build Status:** ✅ Pass
- **Test Status:** ✅ Pass (220 tests)
- **Compliance Status:** ✅ 100% compliant

## Benefits

1. **Consistency:** All workspace dependencies now use the same version across all crates
2. **Maintainability:** Dependency versions can be updated in one place (workspace root)
3. **Clarity:** Clear distinction between workspace-managed and crate-specific dependencies
4. **Build Efficiency:** Cargo can better optimize builds with consistent dependency versions

## Recommendations

1. **Quarterly Reviews:** Run `./scripts/check-workspace-deps.sh` as part of quarterly dependency audits
2. **CI Integration:** Consider adding the compliance script to CI pipeline
3. **Documentation:** Update contributor guidelines to explain workspace dependency patterns
4. **Pre-commit Hook:** Consider adding a pre-commit hook to check workspace dependency compliance

## References

- [Cargo Workspace Documentation](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Cargo Features Documentation](https://doc.rust-lang.org/cargo/reference/features.html)
- Project: `scripts/check-workspace-deps.sh` - Automated compliance checker