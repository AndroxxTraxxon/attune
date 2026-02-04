# Dependency Deduplication Analysis and Plan

**Date**: 2026-01-28  
**Status**: Analysis Complete - Ready for Implementation  
**Priority**: Medium (reduces binary size, compilation time, and security surface)

## Executive Summary

The Attune workspace is currently compiling multiple versions of the same dependencies, leading to:
- **Increased binary size**: Multiple versions linked into final binaries
- **Longer compilation times**: Same crates compiled multiple times
- **Larger SBOM**: More entries in software bill of materials
- **Potential subtle bugs**: Different behavior between versions

This document identifies all duplicate dependencies and provides a step-by-step plan to consolidate them.

---

## Duplicate Dependencies Identified

### Critical Duplicates (Direct Dependencies)

These are duplicates caused by our own crate definitions not using workspace versions:

| Dependency | Versions | Impact | Source |
|------------|----------|--------|--------|
| `validator` | 0.16.1, 0.20.0 | High | `executor/Cargo.toml` uses 0.16 directly |
| `hyper` | 0.14.32, 1.8.1 | Medium | `api/Cargo.toml` uses 1.0 directly |

### Transitive Duplicates (Pulled by Dependencies)

These are duplicates caused by our dependencies using different versions:

| Dependency | Versions | Impact | Pulled By |
|------------|----------|--------|-----------|
| `reqwest` | 0.12.28, 0.13.1 | High | 0.12 via `jsonschema`, 0.13 via our code |
| `thiserror` | 1.0.69, 2.0.18 | Low | Mixed ecosystem versions |
| `syn` | 1.0.109, 2.0.114 | Low | Proc macros use different versions |
| `http` | 0.2.12, 1.4.0 | Medium | `hyper` 0.14 vs 1.x ecosystem split |
| `rustls` | 0.21.12, 0.23.36 | Medium | TLS dependencies version mismatch |
| `tokio-rustls` | 0.24.1, 0.26.4 | Medium | Follows `rustls` versions |
| `h2` | 0.3.27, 0.4.13 | Low | Follows `hyper` versions |
| `hashbrown` | 0.14.5, 0.15.5, 0.16.1 | Low | Multiple minor versions |
| `base64` | 0.21.7, 0.22.1 | Low | Old version via `rustls-pemfile` 1.x |
| `socket2` | 0.5.10, 0.6.2 | Low | Minor version bump |
| `getrandom` | 0.2.17, 0.3.4 | Low | Major version split |
| `rand` | 0.8.5, 0.9.2 | Low | Major version split |
| `winnow` | 0.6.26, 0.7.14 | Low | Parser library version bump |
| `nom` | 7.1.3, 8.0.0 | Low | Parser library major version |
| `heck` | 0.4.1, 0.5.0 | Low | Case conversion utility |
| `idna` | 0.4.0, 1.1.0 | Low | Internationalized domain names |
| `colored` | 2.2.0, 3.1.1 | Low | Terminal colors (CLI only) |
| `foldhash` | 0.1.5, 0.2.0 | Low | Hashing algorithm |

### Ecosystem Split Dependencies

These duplicates are caused by ecosystem transitions (e.g., `hyper` 0.14 → 1.x):

| Old Version | New Version | Root Cause |
|-------------|-------------|------------|
| `hyper` 0.14 | `hyper` 1.x | `eventsource-client` dev-dependency uses old ecosystem |
| `http` 0.2 | `http` 1.x | Follows `hyper` ecosystem |
| `rustls` 0.21 | `rustls` 0.23 | `rustls-native-certs` 0.6 uses old version |

---

## Impact Analysis

### Binary Size Impact
- **Estimated overhead**: 2-5 MB per binary (uncompressed)
- **Affected binaries**: All 7 workspace binaries
- **Total waste**: ~10-25 MB across all binaries

### Compilation Time Impact
- **Duplicate compilation**: ~15-20 crates compiled multiple times
- **Estimated overhead**: 30-60 seconds on clean builds
- **Incremental impact**: Minimal (only on first build)

### Security Impact
- **SBOM entries**: ~40 extra entries in software bill of materials
- **Vulnerability surface**: Potential for same CVE in multiple versions
- **Audit complexity**: Need to track multiple versions of same dependency

---

## Resolution Strategy

### Phase 1: Fix Direct Dependencies (Immediate)

**Priority**: High  
**Effort**: Low  
**Risk**: Low

1. ✅ **Fix `validator` version mismatch**
   - Update `crates/executor/Cargo.toml` to use `workspace = true`
   - Remove explicit version `0.16`

2. ✅ **Fix `hyper` version specification**
   - Update `crates/api/Cargo.toml` to use `workspace = true`
   - Add `hyper` to workspace dependencies if needed

3. ✅ **Audit all crate Cargo.toml files**
   - Ensure all direct dependencies use `workspace = true`
   - Remove explicit version numbers where workspace version exists

### Phase 2: Resolve Transitive Conflicts (Medium Priority)

**Priority**: Medium  
**Effort**: Medium  
**Risk**: Medium

1. **Resolve `reqwest` version conflict**
   - **Issue**: `jsonschema` 0.38.1 pulls in `reqwest` 0.12.28
   - **Options**:
     - A. Wait for `jsonschema` to update (passive)
     - B. Pin `reqwest` to 0.12.x in workspace (breaking change)
     - C. Use workspace patch to override `jsonschema`'s `reqwest` version
   - **Recommendation**: Option C (patch section)

2. **Consolidate `rustls` ecosystem**
   - **Issue**: `rustls-native-certs` 0.6 uses old `rustls` 0.21
   - **Solution**: Update to `rustls-native-certs` 0.8+ (uses `rustls` 0.23)
   - **Impact**: Should be transparent (same API)

3. **Remove old `hyper` 0.14 dependency**
   - **Issue**: `eventsource-client` dev-dependency uses `hyper` 0.14
   - **Solution**: Only used in `attune-api` dev-dependencies
   - **Action**: Move to `[dev-dependencies]` or consider alternative

### Phase 3: Optimize Ecosystem Dependencies (Low Priority)

**Priority**: Low  
**Effort**: High  
**Risk**: Low

These are mostly minor version differences in transitive dependencies. Can be addressed by:
1. Upgrading direct dependencies to latest versions
2. Using `[patch]` sections for critical duplicates
3. Waiting for ecosystem to consolidate

---

## Implementation Plan

### Step 1: Audit Workspace Dependencies (5 minutes)

```bash
# Verify all workspace dependencies are defined
grep -r "workspace = true" crates/*/Cargo.toml

# Find any crates NOT using workspace
for crate in crates/*/Cargo.toml; do
  echo "=== $crate ==="
  grep -E "^[a-z-]+ = \"" "$crate" | grep -v "workspace = true" || echo "  (all use workspace)"
done
```

### Step 2: Fix Direct Dependency Issues (10 minutes)

**File**: `crates/executor/Cargo.toml`
```diff
- validator = { version = "0.16", features = ["derive"] }
+ validator = { workspace = true }
```

**File**: `crates/api/Cargo.toml`
```diff
- hyper = { version = "1.0", features = ["full"] }
+ hyper = { workspace = true }
```

**File**: `Cargo.toml` (workspace root)
```toml
[workspace.dependencies]
# ... existing dependencies ...
hyper = { version = "1.0", features = ["full"] }
```

### Step 3: Add Dependency Patches (15 minutes)

**File**: `Cargo.toml` (workspace root)

Add `[patch.crates-io]` section to force consistent versions:

```toml
[patch.crates-io]
# Force jsonschema to use our reqwest version
# (jsonschema 0.38.1 depends on reqwest 0.12, we use 0.13)
# Note: This may need testing to ensure compatibility
```

**Research needed**: Check if `jsonschema` works with `reqwest` 0.13

### Step 4: Test Changes (20 minutes)

```bash
# Clean build to ensure no cached artifacts
cargo clean

# Full rebuild
cargo build --all-targets

# Run all tests
cargo test --workspace

# Check for remaining duplicates
cargo tree -d

# Verify binary sizes
ls -lh target/debug/attune-*
```

### Step 5: Document Changes (10 minutes)

1. Update `.rules` file with new policy
2. Add pre-commit check for workspace dependency usage
3. Document any remaining duplicates and why they're acceptable

---

## Success Criteria

After implementation, the following should be true:

1. ✅ **No direct dependency version conflicts**
   - All direct dependencies use `workspace = true`
   - Only workspace-defined versions are used

2. ✅ **Reduced duplicate count**
   - Target: < 10 duplicate dependencies
   - Focus on high-impact duplicates (large crates)

3. ✅ **All tests pass**
   - No regressions introduced
   - Same behavior with consolidated versions

4. ✅ **Binary size reduction**
   - Measurable reduction in binary sizes
   - Target: 5-10% reduction

5. ✅ **Documentation updated**
   - Process documented for future maintenance
   - Remaining duplicates explained

---

## Ongoing Maintenance

### Policy: All Dependencies Must Use Workspace Versions

**Rule**: Every direct dependency in a crate's `Cargo.toml` MUST use `workspace = true` unless there's a documented exception.

**Exceptions Allowed**:
1. Crate-specific dependencies not used elsewhere
2. Different feature sets required per crate (document in comment)
3. Dev/build dependencies with no runtime impact

### Automated Checks

Add to CI pipeline:

```bash
# Check for non-workspace dependencies
./scripts/check-workspace-deps.sh
```

**File**: `scripts/check-workspace-deps.sh`
```bash
#!/bin/bash
# Check that all dependencies use workspace = true

ERRORS=0
for crate in crates/*/Cargo.toml; do
  # Find dependencies that specify version directly
  if grep -E "^[a-z-]+ = (\"|\\{).*(version = )" "$crate" | grep -v "workspace = true" > /dev/null; then
    echo "ERROR: $crate has non-workspace dependencies:"
    grep -E "^[a-z-]+ = (\"|\\{).*(version = )" "$crate" | grep -v "workspace = true"
    ERRORS=$((ERRORS + 1))
  fi
done

if [ $ERRORS -gt 0 ]; then
  echo ""
  echo "Found $ERRORS crate(s) with non-workspace dependencies"
  echo "All dependencies should use 'workspace = true'"
  exit 1
fi

echo "All crates use workspace dependencies correctly"
```

### Quarterly Dependency Review

Every quarter:
1. Run `cargo tree -d` and review duplicates
2. Check for new major versions of key dependencies
3. Update workspace dependencies as appropriate
4. Re-run this deduplication analysis

---

## Risks and Mitigations

### Risk: Breaking API Changes

**Probability**: Low  
**Impact**: Medium  
**Mitigation**: 
- Run full test suite after changes
- Test in dev environment before committing
- Review changelogs for any breaking changes

### Risk: Incompatible Transitive Dependencies

**Probability**: Medium  
**Impact**: Low  
**Mitigation**:
- Use `cargo tree` to verify dependency graph
- Test with `--locked` flag
- Keep `Cargo.lock` in version control

### Risk: Performance Regressions

**Probability**: Low  
**Impact**: Low  
**Mitigation**:
- Run benchmarks if available
- Most version bumps are bug fixes, not performance changes

---

## Tools and Commands

### Check for Duplicates
```bash
cargo tree -d
```

### Find Why a Package is Duplicated
```bash
cargo tree -i <package>@<version>
```

### Find All Versions of a Package
```bash
cargo tree | grep "^<package>"
```

### Check Binary Sizes
```bash
ls -lh target/debug/attune-* target/release/attune-*
```

### Audit Dependencies
```bash
cargo audit
```

### Update Dependencies
```bash
cargo update
cargo outdated  # requires cargo-outdated
```

---

## References

- [Cargo Workspace Documentation](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Cargo Patch Section](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html)
- [Dependency Resolution RFC](https://github.com/rust-lang/rfcs/blob/master/text/2957-cargo-features2.md)

---

## Appendix: Full Duplicate List

Generated with: `cargo tree -d 2>&1 | grep -E "^[a-z]" | sort | uniq`

```
async-global-executor-trait v2.2.0 / v3.1.0
base64 v0.21.7 / v0.22.1
bitflags v2.10.0 (multiple uses, same version)
byteorder v1.5.0 (multiple uses, same version)
chrono v0.4.43 (multiple uses, same version)
colored v2.2.0 / v3.1.1
crypto-common v0.1.7 (multiple uses, same version)
either v1.15.0 (multiple uses, same version)
executor-trait v2.1.2 / v3.1.0
foldhash v0.1.5 / v0.2.0
futures-channel v0.3.31 (multiple uses, same version)
futures-sink v0.3.31 (multiple uses, same version)
generic-array v0.14.7 (multiple uses, same version)
getrandom v0.2.17 / v0.3.4
h2 v0.3.27 / v0.4.13
hashbrown v0.14.5 / v0.15.5 / v0.16.1
heck v0.4.1 / v0.5.0
http-body v0.4.6 / v1.0.1
http v0.2.12 / v1.4.0
hyper-rustls v0.24.2 / v0.27.7
hyper v0.14.32 / v1.8.1
idna v0.4.0 / v1.1.0
indexmap v2.13.0 (multiple uses, same version)
lazy_static v1.5.0 (multiple uses, same version)
log v0.4.29 (multiple uses, same version)
md-5 v0.10.6 (multiple uses, same version)
nom v7.1.3 / v8.0.0
num-traits v0.2.19 (multiple uses, same version)
openssl-probe v0.1.6 / v0.2.1
rand_chacha v0.3.1 / v0.9.0
rand_core v0.6.4 / v0.9.5
rand v0.8.5 / v0.9.2
reactor-trait v2.8.0 / v3.1.1
reqwest v0.12.28 / v0.13.1
rustls-native-certs v0.6.3 / v0.8.3
rustls-pemfile v1.0.4 / v2.2.0
rustls v0.21.12 / v0.23.36
rustls-webpki v0.101.7 / v0.103.9
serde_core v1.0.228 (multiple uses, same version)
sha2 v0.10.9 (multiple uses, same version)
smallvec v1.15.1 (multiple uses, same version)
socket2 v0.5.10 / v0.6.2
sqlx-postgres v0.8.6 (multiple uses, same version)
subtle v2.6.1 (multiple uses, same version)
syn v1.0.109 / v2.0.114
thiserror-impl v1.0.69 / v2.0.18
thiserror v1.0.69 / v2.0.18
tokio-rustls v0.24.1 / v0.26.4
tokio v1.49.0 (multiple uses, same version)
uuid v1.20.0 (multiple uses, same version)
validator_derive v0.16.0 / v0.20.0
validator v0.16.1 / v0.20.0
webpki-roots v0.26.11 / v1.0.5
winnow v0.6.26 / v0.7.14
```

**Note**: Some "duplicates" listed are actually the same version used multiple times (which is fine). Focus on actual version conflicts.