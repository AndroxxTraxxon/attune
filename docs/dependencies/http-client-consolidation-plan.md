# HTTP Client Consolidation Analysis & Plan

**Date**: 2026-01-28  
**Status**: Ready for Implementation  
**Priority**: High (Phase 1), Medium (Phase 2), Low (Phase 3)  
**Estimated Effort**: 4-6 hours total

## Executive Summary

**Current State**: We have both `reqwest` (high-level) and `hyper` (low-level) as dependencies, plus duplicate versions due to `eventsource-client` and `jsonschema`.

**Goal**: Eliminate redundancy, consolidate on `reqwest`, and resolve version conflicts.

**Key Finding**: We don't actually need direct `hyper` dependency - it's only used in test utilities and can be easily replaced.

---

## Table of Contents

1. [Reqwest vs Hyper - Relationship & Usage](#1-reqwest-vs-hyper---relationship--usage)
2. [EventSource Client Analysis](#2-eventsource-client-analysis)
3. [Test Helpers Using http-body-util](#3-test-helpers-using-http-body-util)
4. [Rustls Version Conflicts](#4-rustls-version-conflicts)
5. [JsonSchema & Reqwest 0.12 vs 0.13](#5-jsonschema--reqwest-012-vs-013)
6. [Implementation Plan](#implementation-plan)
7. [Expected Results](#expected-results-after-implementation)
8. [Risks & Mitigations](#risks--mitigations)
9. [Testing Plan](#testing-plan)
10. [Timeline & Effort](#timeline--effort)
11. [Recommendation](#recommendation)
12. [Success Criteria](#success-criteria)

---

## 1. Reqwest vs Hyper - Relationship & Usage

### What They Are

- **hyper**: Low-level HTTP implementation (protocol details, connection pooling)
- **reqwest**: High-level HTTP client built **on top of** hyper (ergonomic API)

**Key Point**: reqwest uses hyper internally, so having both directly is redundant unless you need low-level hyper features.

### Our Usage Analysis

#### Direct `hyper` Usage

**Location**: `crates/api/Cargo.toml` (dev-dependencies only)

```toml
[dev-dependencies]
hyper = { workspace = true }
http-body-util = "0.1"
```

**Actual Usage**: `crates/api/tests/helpers.rs`

```rust
use http_body_util::BodyExt;

// Used to read response bodies in tests:
let body = response.into_body().collect().await?.to_bytes();
let json: Value = serde_json::from_slice(&body)?;
```

**Conclusion**: ✅ Only used in test utilities for reading HTTP response bodies

#### `reqwest` Usage

**Locations**: Production code across multiple crates

- `attune-common`: HTTP utilities, external API calls
- `attune-api`: Outbound HTTP requests
- `attune-cli`: API client for talking to Attune API
- `attune-worker`: Downloading artifacts, making HTTP requests

**Conclusion**: ✅ Core production dependency used extensively

### Verdict

✅ **We don't need `hyper` directly** - it's only used for test utilities that can be replaced with Axum's built-in utilities or simple helpers.

---

## 2. EventSource Client Analysis

### Current Usage

**Library**: `eventsource-client = "0.13"` (dev-dependency)

**Location**: `crates/api/tests/sse_execution_stream_tests.rs`

**Purpose**: Testing Server-Sent Events (SSE) endpoints

**Test Coverage**:
- `test_sse_stream_receives_execution_updates` - Core SSE functionality
- `test_sse_stream_filters_by_execution_id` - Filtering
- `test_sse_stream_requires_authentication` - Auth
- `test_sse_stream_all_executions` - Multi-execution streaming
- `test_postgresql_notify_trigger_fires` - DB trigger verification

**Functionality Tested**:
- PostgreSQL NOTIFY → Notifier Service → WebSocket → SSE flow
- Real-time execution status updates
- Authentication and authorization
- Event filtering

### Problems with Current Library

❌ **Uses old `hyper` 0.14 ecosystem**
```
eventsource-client 0.13
  └── hyper 0.14.32
      └── http 0.2.12
      └── rustls 0.21.12
      └── tokio-rustls 0.24.1
```

❌ **Not actively maintained**
- Last updated: 1+ years ago
- Issues with newer Rust versions
- No updates to modern ecosystem

❌ **Pulls in entire old dependency tree**
- Creates version conflicts across the board
- Adds ~10-15 transitive dependencies

### Alternative Options

#### Option A: `reqwest-eventsource` ✅ **RECOMMENDED**

**Crate**: `reqwest-eventsource = "0.6"`

**Advantages**:
- ✅ Built on top of `reqwest` (uses our existing HTTP client)
- ✅ Actively maintained (last updated 2024)
- ✅ Clean, simple API
- ✅ Modern dependencies (hyper 1.x, rustls 0.23)
- ✅ Well-documented with good examples

**API Example**:
```rust
use reqwest_eventsource::{Event, EventSource};
use futures::StreamExt;

let mut stream = EventSource::get(url);
while let Some(event) = stream.next().await {
    match event {
        Ok(Event::Open) => println!("Connected"),
        Ok(Event::Message(msg)) => {
            println!("Event: {}", msg.event);
            println!("Data: {}", msg.data);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Migration Complexity**: Low
- Similar API to current library
- Event structure is compatible
- Authentication via headers or URL params

#### Option B: Implement SSE parsing ourselves

**Complexity**: ~100-150 lines of code

**Pros**:
- No external dependency
- Full control over implementation
- SSE protocol is simple (text-based)

**Cons**:
- Additional maintenance burden
- Need to handle edge cases (reconnection, keep-alive, etc.)
- Need to test thoroughly
- Reinventing the wheel

**Recommendation**: ❌ Not worth it - `reqwest-eventsource` is actively maintained and solves this well

#### Option C: `async-sse` or `tokio-sse`

**Status**: Less mature, smaller community

**Recommendation**: ❌ Stick with `reqwest-eventsource` (better maintained)

#### Option D: Remove SSE tests entirely

**Recommendation**: ❌ **Strongly discouraged**
- These tests are valuable
- They test critical real-time notification functionality
- SSE is a key feature of the API

### Recommended Solution

✅ **Migrate to `reqwest-eventsource`**
- Best balance of functionality, maintenance, and ecosystem alignment
- Eliminates entire old dependency tree
- Low migration effort (2-3 hours)

---

## 3. Test Helpers Using `http-body-util`

### Current Code

**Location**: `crates/api/tests/helpers.rs`

```rust
use http_body_util::BodyExt;

// In tests:
let body = response.into_body().collect().await?.to_bytes();
let json: Value = serde_json::from_slice(&body)?;
```

**Purpose**: Converting HTTP response bodies to bytes for JSON parsing in tests

### Replacement Options

#### Option 1: Use Axum's built-in utilities ✅ **RECOMMENDED**

```rust
use axum::body::Body;
use axum::http::Response;

// Axum provides this directly:
let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
let json: Value = serde_json::from_slice(&body_bytes)?;
```

**Advantages**:
- ✅ Already in our dependencies (via axum)
- ✅ No additional dependencies
- ✅ Simple one-line replacement

#### Option 2: Create a helper function

```rust
use futures::stream::StreamExt;

async fn body_to_bytes(body: Body) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
    }
    Ok(bytes)
}

async fn body_to_json<T: DeserializeOwned>(body: Body) -> Result<T> {
    let bytes = body_to_bytes(body).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
```

**Advantages**:
- ✅ More explicit control
- ✅ Can add custom error handling
- ✅ Reusable across tests

### Recommended Solution

✅ **Option 1**: Use Axum's `axum::body::to_bytes()`
- Simplest solution
- Already available
- Well-tested by Axum team

---

## 4. Rustls Version Conflicts

### Current Situation

```
rustls v0.21.12
  └── rustls-native-certs v0.6.3
      └── hyper-rustls v0.24.2
          └── eventsource-client v0.13

rustls v0.23.36
  └── rustls-native-certs v0.8.3
      └── hyper-rustls v0.27.7
          └── reqwest v0.13
```

### Root Cause

`eventsource-client` 0.13 uses the old `hyper` 0.14 ecosystem, which transitively pulls in `rustls` 0.21.

### Impact

- Two versions of entire TLS stack
- ~2-3 MB binary size overhead
- More crates to audit for security
- Potential for subtle TLS configuration differences

### Solution

✅ **Switching to `reqwest-eventsource` eliminates this entirely**

**Reason**:
- `reqwest-eventsource` uses `reqwest` 0.12+
- `reqwest` 0.13 uses modern `rustls` 0.23 ecosystem
- All TLS dependencies consolidated to single version tree

**Result After Migration**:
```
rustls v0.23.36 (single version)
  └── rustls-native-certs v0.8.3
      └── hyper-rustls v0.27.7
          └── reqwest v0.13
              └── reqwest-eventsource v0.6
```

---

## 5. JsonSchema & Reqwest 0.12 vs 0.13

### Current Situation

```
reqwest v0.12.28
  └── jsonschema v0.38.1
      └── attune-common

reqwest v0.13.1
  └── [our workspace dependencies]
```

### Analysis

**Why jsonschema uses reqwest 0.12**:
- `jsonschema` uses `reqwest` to fetch remote schemas (HTTP URLs in `$ref`)
- Library maintainers haven't updated to reqwest 0.13 yet
- jsonschema 0.38.1 is recent (released 2024)

**Actual Impact**:
- ❌ Binary size: ~2 MB duplication (two reqwest versions)
- ❌ SBOM: Additional entries for reqwest 0.12 tree
- ✅ Functionality: Both versions work fine (stable APIs)
- ✅ Risk: Low - both are mature, stable releases

**How Much Do We Use JsonSchema?**

Need to investigate:
```bash
grep -r "jsonschema::" crates/
grep -r "use jsonschema" crates/
```

**Current Usage** (based on dependencies):
- Used in `attune-common`
- Purpose: Likely JSON Schema validation for actions/workflows
- Criticality: TBD (needs verification)

### Options

#### Option A: Wait for upstream update ⏳ **RECOMMENDED**

**Rationale**:
- `jsonschema` maintainers will likely update to reqwest 0.13 soon
- Minimal actual impact (both versions are stable)
- Low risk of issues
- Cost is acceptable (~2 MB per binary)

**Action**: Monitor upstream, update when available

**Timeline**: Likely within 3-6 months

#### Option B: Use cargo patch ⚠️ **NOT RECOMMENDED**

```toml
[patch.crates-io]
jsonschema = { git = "https://github.com/Stranger6667/jsonschema-rs", branch = "main" }
```

**Risks**:
- ⚠️ Untested upstream changes
- ⚠️ Potential breaking changes before release
- ⚠️ Maintenance burden (need to track upstream)
- ⚠️ May break in unexpected ways

**Verdict**: Not worth the risk for ~2 MB savings

#### Option C: Remove jsonschema dependency 🔍 **INVESTIGATE**

**Prerequisites**: Need to determine:
1. Where is jsonschema actually used?
2. Can we replace it with alternatives?
3. Is the validation critical?

**Potential Alternatives**:
- `schemars` - JSON Schema generation (we already use this)
- `validator` - Rust-native validation (we already use this)
- Manual validation with serde

**Action Items**:
1. Audit jsonschema usage: `grep -r "jsonschema" crates/`
2. Determine if removable
3. If yes: Remove and implement alternative
4. If no: Accept the duplication

**Timeline**: 1-2 hours investigation + potential migration

#### Option D: Accept the duplication ✅ **SHORT-TERM**

**Rationale**:
- Only ~2 MB overhead
- Both versions are stable
- Upstream will update eventually
- Other priorities are higher

**Recommendation**: Accept for now, revisit in quarterly review

### Recommended Solution

✅ **Short-term**: Accept reqwest 0.12 duplication (Option D)
🔍 **Medium-term**: Investigate jsonschema usage (Option C)
⏳ **Long-term**: Wait for upstream update (Option A)

---

## Implementation Plan

### Phase 1: Replace EventSource Client (High Impact) ⚡

**Priority**: HIGH  
**Effort**: 2-3 hours  
**Impact**: Eliminates entire old `hyper` ecosystem

#### Steps

**1. Add `reqwest-eventsource` to workspace dependencies**

File: `Cargo.toml`

```diff
[workspace.dependencies]
# ... existing dependencies ...
+reqwest-eventsource = "0.6"
```

**2. Update API dev dependencies**

File: `crates/api/Cargo.toml`

```diff
[dev-dependencies]
mockall = { workspace = true }
tower = { workspace = true }
hyper = { workspace = true }
http-body-util = "0.1"
tempfile = { workspace = true }
-eventsource-client = "0.13"
+reqwest-eventsource = { workspace = true }
```

**3. Rewrite SSE test code**

File: `crates/api/tests/sse_execution_stream_tests.rs`

**Changes Required** (~150 lines):

```diff
-use eventsource_client::{self as es, Client};
+use reqwest_eventsource::{Event, EventSource};
 use futures::StreamExt;

 // Build SSE URL with authentication
 let sse_url = format!(
     "http://localhost:8080/api/v1/executions/stream?execution_id={}&token={}",
     execution.id, token
 );

-// Create SSE client
-let client = es::ClientBuilder::for_url(&sse_url)?
-    .header("Accept", "text/event-stream")?
-    .build();
-
-let mut stream = Client::stream(&client);
+// Create SSE stream
+let mut stream = EventSource::get(&sse_url);

 // Wait for SSE events with timeout
 while attempts < max_attempts && (!received_running || !received_succeeded) {
     match timeout(Duration::from_millis(500), stream.next()).await {
         Ok(Some(Ok(event))) => {
             match event {
-                es::SSE::Connected(_) => {
+                Event::Open => {
                     println!("SSE connection established");
                 }
-                es::SSE::Event(ev) => {
+                Event::Message(msg) => {
-                    if let Ok(data) = serde_json::from_str::<Value>(&ev.data) {
+                    if let Ok(data) = serde_json::from_str::<Value>(&msg.data) {
                         // ... rest of logic unchanged ...
                     }
                 }
-                es::SSE::Comment(_) => {
-                    println!("Received keep-alive comment");
-                }
             }
         }
```

**Key API Differences**:

| eventsource-client | reqwest-eventsource |
|-------------------|---------------------|
| `es::SSE::Connected(_)` | `Event::Open` |
| `es::SSE::Event(ev)` | `Event::Message(msg)` |
| `es::SSE::Comment(_)` | (built into Event::Message) |
| `ev.data` | `msg.data` |
| `ClientBuilder` | Direct `EventSource::get()` |

**4. Update all 5 test functions**

Apply similar changes to:
- `test_sse_stream_receives_execution_updates`
- `test_sse_stream_filters_by_execution_id`
- `test_sse_stream_requires_authentication`
- `test_sse_stream_all_executions`
- `test_postgresql_notify_trigger_fires`

**5. Handle authentication**

The new library supports authentication via:
```rust
// Option 1: URL parameters (current approach, keep working)
let url = format!("{}?token={}", base_url, token);
EventSource::get(url)

// Option 2: Headers (better practice)
use reqwest::Client;
let client = Client::builder()
    .default_headers({
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", token).parse().unwrap()
        );
        headers
    })
    .build()?;
let stream = EventSource::new(client.get(url));
```

**6. Test thoroughly**

```bash
cd crates/api
cargo test sse_execution_stream_tests -- --nocapture
```

#### Expected Benefits

- ✅ Removes `hyper` 0.14 dependency tree (~8-10 crates)
- ✅ Removes `rustls` 0.21 dependency tree (~5-7 crates)
- ✅ Removes `http` 0.2 dependency tree (~3-4 crates)
- ✅ Total: ~15-20 crates eliminated from dependency tree
- ✅ Binary size reduction: ~3-5 MB per binary
- ✅ Compilation time: ~20-40 seconds faster on clean builds
- ✅ SBOM reduction: ~15-20 fewer entries

---

### Phase 2: Remove Direct Hyper Dependency (Low Impact) 🔧

**Priority**: MEDIUM  
**Effort**: 30 minutes  
**Impact**: Cleanup only (hyper is transitive via reqwest anyway)

#### Steps

**1. Replace `http-body-util` usage in test helpers**

File: `crates/api/tests/helpers.rs`

```diff
 use axum::{
     body::Body,
     http::{header, Method, Request, StatusCode},
 };
-use http_body_util::BodyExt;
 use serde::de::DeserializeOwned;

 // Add helper function:
 async fn body_to_json<T: DeserializeOwned>(body: Body) -> Result<T> {
-    let bytes = body.collect().await?.to_bytes();
+    let bytes = axum::body::to_bytes(body, usize::MAX).await?;
     Ok(serde_json::from_slice(&bytes)?)
 }
```

**2. Update all test helper usages**

Find and replace pattern:
```diff
-let body = response.into_body().collect().await?.to_bytes();
-let json: Value = serde_json::from_slice(&body)?;
+let json: Value = body_to_json(response.into_body()).await?;
```

**3. Remove from dev-dependencies**

File: `crates/api/Cargo.toml`

```diff
[dev-dependencies]
mockall = { workspace = true }
tower = { workspace = true }
-hyper = { workspace = true }
-http-body-util = "0.1"
tempfile = { workspace = true }
reqwest-eventsource = { workspace = true }
```

**4. Remove from workspace if no longer needed**

File: `Cargo.toml`

Check if any other crate uses hyper:
```bash
grep -r "hyper" crates/*/Cargo.toml
```

If none:
```diff
[workspace.dependencies]
-hyper = { version = "1.0", features = ["full"] }
```

**5. Test**

```bash
cargo test --workspace
```

#### Expected Benefits

- ✅ No direct `hyper` dependency in our code
- ✅ Cleaner dependency tree
- ✅ ~100 KB binary size reduction (marginal)
- ✅ Hyper still present as transitive dependency (expected and fine)

#### Note

Hyper will remain in the dependency tree because:
- `reqwest` uses it internally
- `axum` uses it internally
- This is expected and desirable (it's the underlying HTTP implementation)

---

### Phase 3: Investigate JsonSchema Usage (Optional) 🔍

**Priority**: LOW  
**Effort**: 1-2 hours  
**Impact**: Medium if removable, low if keeping

#### Steps

**1. Find all uses of jsonschema**

```bash
cd attune
grep -r "jsonschema::" crates/ --include="*.rs"
grep -r "use jsonschema" crates/ --include="*.rs"
grep -r "JsonSchema" crates/ --include="*.rs"
```

**2. Analyze usage patterns**

Determine:
- What is jsonschema used for?
- Is it critical functionality?
- Can it be replaced?

**3. Decision tree**

**If used for JSON Schema validation**:
- **Option A**: Keep it, accept reqwest 0.12 duplication
- **Option B**: Implement validation differently
  - Use `validator` crate (we already have it)
  - Use manual serde validation
  - Generate validation code at compile time

**If used for schema generation**:
- We already have `schemars` for this
- Check if jsonschema can be removed in favor of schemars

**If barely used**:
- Remove it entirely
- Implement needed functionality differently

**4. Implementation (if removing)**

Example replacement:
```diff
-use jsonschema::JSONSchema;
+use validator::Validate;

-let schema = JSONSchema::compile(&schema_json)?;
-schema.validate(&instance)?;
+#[derive(Validate)]
+struct MyData {
+    #[validate(length(min = 1, max = 100))]
+    name: String,
+}
+
+my_data.validate()?;
```

**5. Test thoroughly**

```bash
cargo test --workspace
```

#### Decision Matrix

| Usage Level | Recommendation | Action |
|-------------|----------------|--------|
| Critical feature | Keep it | Accept reqwest 0.12 duplication (~2 MB) |
| Nice-to-have | Evaluate alternatives | If easy to replace, do it |
| Barely used | Remove it | Eliminate dependency entirely |
| Not sure | Keep for now | Revisit in quarterly dependency review |

#### Expected Benefits (If Removed)

- ✅ Eliminates reqwest 0.12 duplication
- ✅ Binary size reduction: ~2 MB per binary
- ✅ SBOM reduction: ~5-8 fewer entries
- ✅ Faster compilation: ~5-10 seconds

#### Expected Cost (If Removed)

- ⚠️ Need to implement alternative validation
- ⚠️ Testing required to ensure validation still works
- ⚠️ Potential behavior changes

---

## Expected Results After Implementation

### Dependency Tree Reduction

#### Before Implementation

```
HTTP Clients:
- reqwest v0.12.28 (via jsonschema)
- reqwest v0.13.1 (workspace)
- hyper v0.14.32 (via eventsource-client)
- hyper v1.8.1 (via reqwest/axum)

HTTP Types:
- http v0.2.12 (via hyper 0.14)
- http v1.4.0 (via hyper 1.x)
- http-body v0.4.6 (via hyper 0.14)
- http-body v1.0.1 (via hyper 1.x)

TLS:
- rustls v0.21.12 (via hyper 0.14 ecosystem)
- rustls v0.23.36 (via reqwest)
- rustls-native-certs v0.6.3 (old)
- rustls-native-certs v0.8.3 (new)
- rustls-pemfile v1.0.4 (old)
- rustls-pemfile v2.2.0 (new)
- tokio-rustls v0.24.1 (old)
- tokio-rustls v0.26.4 (new)

Total duplicate versions: ~15-20
```

#### After Phase 1

```
HTTP Clients:
- reqwest v0.12.28 (via jsonschema) - acceptable
- reqwest v0.13.1 (workspace) ✓
- hyper v1.8.1 (transitive via reqwest/axum) ✓

HTTP Types:
- http v1.4.0 (single version) ✓
- http-body v1.0.1 (single version) ✓

TLS:
- rustls v0.23.36 (single version) ✓
- rustls-native-certs v0.8.3 (single version) ✓
- rustls-pemfile v2.2.0 (single version) ✓
- tokio-rustls v0.26.4 (single version) ✓

Total duplicate versions: ~2-3 (just reqwest)
```

#### After Phase 2

```
HTTP Clients:
- reqwest v0.12.28 (via jsonschema) - acceptable
- reqwest v0.13.1 (workspace) ✓
- hyper v1.8.1 (transitive, no direct dependency) ✓

No direct hyper dependency in our code ✓
```

#### After Phase 3 (If jsonschema removed)

```
HTTP Clients:
- reqwest v0.13.1 (single version) ✓
- hyper v1.8.1 (transitive) ✓

Total duplicate versions: 0 ✓✓✓
```

### Binary Size Impact

| Phase | Per Binary | Total (7 binaries) | Cumulative |
|-------|------------|-------------------|------------|
| **Phase 1** | -3 to -5 MB | -21 to -35 MB | -3 to -5 MB |
| **Phase 2** | -100 KB | -700 KB | -3.1 to -5.1 MB |
| **Phase 3** | -2 MB | -14 MB | -5.1 to -7.1 MB |
| **Total** | **-5 to -7 MB** | **-35 to -50 MB** | - |

### Compilation Time Impact

| Phase | Clean Build | Incremental | Reason |
|-------|-------------|-------------|--------|
| **Phase 1** | -20 to -40 sec | -2 to -5 sec | 15-20 fewer crates compiled |
| **Phase 2** | -2 to -5 sec | < 1 sec | Marginal improvement |
| **Phase 3** | -5 to -10 sec | -1 to -2 sec | reqwest 0.12 tree eliminated |
| **Total** | **-27 to -55 sec** | **-3 to -8 sec** | - |

### SBOM (Software Bill of Materials) Impact

| Phase | Crates Removed | Security Impact |
|-------|----------------|-----------------|
| **Phase 1** | 15-20 | High - eliminates old TLS stack |
| **Phase 2** | 2-3 | Low - cleanup only |
| **Phase 3** | 5-8 | Medium - eliminates reqwest duplication |
| **Total** | **22-31** | **Significant reduction in audit surface** |

### Dependency Count

| Metric | Before | After Phase 1 | After Phase 2 | After Phase 3 |
|--------|--------|---------------|---------------|---------------|
| Total dependencies | ~250 | ~230 | ~228 | ~220 |
| Duplicate versions | 15-20 | 2-3 | 2-3 | 0 |
| Direct HTTP deps | 2 (reqwest, hyper) | 1 (reqwest) | 1 (reqwest) | 1 (reqwest) |
| TLS versions | 2 | 1 | 1 | 1 |

---

## Risks & Mitigations

### Risk 1: SSE Test Behavior Changes

**Phase**: 1  
**Probability**: Low  
**Impact**: Medium

**Description**: `reqwest-eventsource` may handle events slightly differently than `eventsource-client`

**Mitigation**:
- Both libraries implement the SSE specification correctly
- Test extensively before merging
- Keep old tests temporarily in a branch for comparison
- Run tests multiple times to ensure stability
- Test with actual SSE server (not just mocks)

**Rollback Plan**: Revert to `eventsource-client` if tests fail consistently

### Risk 2: API Differences in reqwest-eventsource

**Phase**: 1  
**Probability**: Low  
**Impact**: Low

**Description**: API for creating and handling streams is slightly different

**Mitigation**:
- API is well-documented with clear examples
- Main differences are in connection setup, not event handling
- Event structure is similar (both follow SSE spec)
- Review library documentation before migration

**Rollback Plan**: Easy to revert (only dev-dependency, only tests affected)

### Risk 3: Authentication Handling

**Phase**: 1  
**Probability**: Very Low  
**Impact**: Low

**Description**: Authentication might work differently

**Mitigation**:
- `reqwest-eventsource` supports both URL params and headers
- Current tests pass token in URL (will continue working)
- Can also use Authorization header for better security
- Test authentication explicitly

**Validation**:
```rust
// Test that auth still works
#[test]
async fn test_sse_auth_with_token() {
    let url = format!("{}?token={}", base_url, token);
    let mut stream = EventSource::get(&url);
    // Should connect successfully
}
```

### Risk 4: Test Helper Breakage

**Phase**: 2  
**Probability**: Very Low  
**Impact**: Low

**Description**: Replacing `http-body-util` with Axum utilities might break tests

**Mitigation**:
- Axum's `to_bytes()` is well-tested and stable
- Simple one-line replacement
- Test thoroughly after changes
- Keep old code commented out temporarily

**Rollback Plan**: Revert to `http-body-util` (2-line change)

### Risk 5: JsonSchema Functionality Loss

**Phase**: 3  
**Probability**: Low to Medium (depends on usage)  
**Impact**: Medium to High (depends on criticality)

**Description**: Removing jsonschema might break validation functionality

**Mitigation**:
- **First**: Thoroughly audit usage before making any changes
- Implement alternative validation before removing
- Test all validation scenarios
- Consider keeping if used extensively

**Decision Point**: Don't proceed with Phase 3 if jsonschema is critical

---

## Testing Plan

### Phase 1 Testing

#### Unit Tests
```bash
# Run SSE-specific tests
cd crates/api
cargo test sse_execution_stream_tests -- --nocapture

# Expected output:
# test test_sse_stream_receives_execution_updates ... ok
# test test_sse_stream_filters_by_execution_id ... ok
# test test_sse_stream_requires_authentication ... ok
# test test_sse_stream_all_executions ... ok
# test test_postgresql_notify_trigger_fires ... ok
```

#### Integration Tests
```bash
# Start E2E services
./scripts/setup-e2e-db.sh
./scripts/start-e2e-services.sh

# Run full API test suite
cd crates/api
cargo test

# Stop services
./scripts/stop-e2e-services.sh
```

#### Manual Testing
```bash
# Start API server
cargo run --bin attune-api

# In another terminal, test SSE endpoint manually:
curl -N -H "Accept: text/event-stream" \
  "http://localhost:8080/api/v1/executions/stream?token=YOUR_TOKEN"

# Should see:
# event: connected
# data: {"message":"Connected to execution stream"}
#
# event: execution_update
# data: {"entity_type":"execution","data":{...}}
```

#### Stress Testing
```bash
# Run SSE tests multiple times to ensure stability
for i in {1..10}; do
  echo "Run $i"
  cargo test sse_execution_stream_tests -- --test-threads=1
done
```

### Phase 2 Testing

#### Unit Tests
```bash
# Run all tests that use test helpers
cargo test --workspace

# Specifically test API endpoints
cd crates/api
cargo test
```

#### Verify No Regressions
```bash
# Compare test output before and after
cargo test --workspace 2>&1 | tee before.txt
# ... make changes ...
cargo test --workspace 2>&1 | tee after.txt
diff before.txt after.txt
# Should show no test failures, only dependency changes
```

### Phase 3 Testing (If Implemented)

#### Validation Tests
```bash
# Identify and run all tests that use jsonschema
grep -r "jsonschema" crates/*/tests/ | cut -d: -f1 | sort -u

# Run those specific test files
# Example:
cargo test --test schema_validation_tests
```

#### Manual Validation Testing
```bash
# Test action/workflow validation with various inputs
# Test both valid and invalid schemas
# Ensure error messages are still clear
```

### Dependency Verification

```bash
# Check for duplicate dependencies
cargo tree -d

# Expected after Phase 1:
# Should NOT see: hyper v0.14, rustls v0.21, http v0.2
# Should see: reqwest v0.12 and v0.13 (acceptable until Phase 3)

# Check binary sizes
cargo clean
cargo build --release
ls -lh target/release/attune-* > before_sizes.txt
# ... make changes ...
cargo clean
cargo build --release
ls -lh target/release/attune-* > after_sizes.txt
diff before_sizes.txt after_sizes.txt
```

### Compilation Time Measurement

```bash
# Before changes
cargo clean
time cargo build --workspace > /dev/null

# After changes
cargo clean
time cargo build --workspace > /dev/null

# Compare times
```

### Regression Testing Checklist

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] SSE streaming works correctly
- [ ] Authentication still works
- [ ] Event filtering works
- [ ] PostgreSQL NOTIFY triggers fire
- [ ] WebSocket connections stable
- [ ] No new compiler warnings
- [ ] Documentation is updated
- [ ] Binary sizes reduced (or unchanged)
- [ ] No performance regressions

---

## Timeline & Effort

### Phase 1: Replace EventSource Client

| Task | Estimated Time | Difficulty |
|------|----------------|------------|
| Add reqwest-eventsource dependency | 5 min | Easy |
| Update imports and setup | 15 min | Easy |
| Rewrite event handling logic | 60 min | Medium |
| Update all 5 test functions | 45 min | Medium |
| Test and debug | 30 min | Medium |
| **Total Phase 1** | **2.5-3 hours** | **Medium** |

**Timeline**: Can be completed in one work session

**Priority**: ⚡ **HIGH** - Biggest impact

### Phase 2: Remove Direct Hyper Dependency

| Task | Estimated Time | Difficulty |
|------|----------------|------------|
| Replace http-body-util in helpers | 10 min | Easy |
| Update all usages | 10 min | Easy |
| Remove from dependencies | 5 min | Easy |
| Test | 10 min | Easy |
| **Total Phase 2** | **30-45 min** | **Easy** |

**Timeline**: Can be done immediately after Phase 1

**Priority**: 🔧 **MEDIUM** - Cleanup and polish

### Phase 3: Investigate JsonSchema Usage

| Task | Estimated Time | Difficulty |
|------|----------------|------------|
| Audit jsonschema usage | 20 min | Easy |
| Analyze criticality | 15 min | Easy |
| Research alternatives | 25 min | Medium |
| Decision: keep or remove | - | - |
| **If removing:** | | |
| - Implement alternative | 30-60 min | Medium-Hard |
| - Update all usages | 20-40 min | Medium |
| - Test thoroughly | 30 min | Medium |
| **Total Phase 3** | **1-3 hours** | **Medium-Hard** |

**Timeline**: Separate task, can be done later

**Priority**: 🔍 **LOW** - Nice to have

### Overall Timeline

| Scenario | Total Effort | Timeline | Recommendation |
|----------|--------------|----------|----------------|
| **Phase 1 only** | 2.5-3 hours | Half day | ✅ Do this now |
| **Phases 1 + 2** | 3-4 hours | Half day | ✅ Do together |
| **All phases** | 4-7 hours | 1-2 days | ⏳ Split across time |

---

## Recommendation

### Immediate Actions (This Week)

#### ✅ Phase 1: Replace EventSource Client

**DO THIS NOW** - Highest impact, reasonable effort

**Why**:
- Eliminates entire old `hyper` 0.14 ecosystem
- Removes `rustls` 0.21 security concerns
- Reduces binary size by 3-5 MB
- Speeds up compilation by 20-40 seconds
- Reduces SBOM by 15-20 entries
- Well-maintained replacement available

**Risk**: Low - well-tested library, similar API

**Success Metric**: `cargo tree -d` shows no more `hyper` 0.14

#### ✅ Phase 2: Remove Direct Hyper Dependency

**DO AFTER PHASE 1** - Easy cleanup

**Why**:
- Completes the consolidation
- Simple changes, low risk
- Cleaner architecture

**Risk**: Very low - straightforward replacement

**Success Metric**: No direct `hyper` in our Cargo.toml files

### Follow-up Actions (Next Month)

#### 🔍 Phase 3: Investigate JsonSchema

**DO LATER** - Lower priority

**Why**:
- Need to understand usage first
- May not be worth the effort
- Upstream might update soon anyway

**Timeline**: During next quarterly dependency review

**Decision Point**: Only proceed if:
- jsonschema is barely used, OR
- Easy alternative exists, AND
- Team has bandwidth

### Do NOT Do

#### ❌ Don't use cargo patch for jsonschema

**Reason**: Too risky for minimal benefit (~2 MB)

**Risk**: Potential breaking changes, maintenance burden

**Better**: Wait for upstream to update to reqwest 0.13

#### ❌ Don't implement SSE parsing ourselves

**Reason**: `reqwest-eventsource` is actively maintained

**Risk**: Reinventing the wheel, maintenance burden

**Better**: Use the well-tested library

#### ❌ Don't skip testing

**Reason**: SSE tests are critical for real-time features

**Risk**: Breaking production functionality

**Better**: Test thoroughly, especially SSE streaming

---

## Success Criteria

### After Phase 1

1. ✅ No `hyper` 0.14.x in `cargo tree`
2. ✅ No `rustls` 0.21.x in `cargo tree`
3. ✅ No `http` 0.2.x in `cargo tree`
4. ✅ All 5 SSE tests pass consistently
5. ✅ SSE streaming works in E2E environment
6. ✅ Binary sizes reduced by 3-5 MB each
7. ✅ Compilation time reduced by 20-40 seconds
8. ✅ SBOM reduced by 15-20 entries

### After Phase 2

1. ✅ No direct `hyper` dependency in any Cargo.toml
2. ✅ No `http-body-util` dependency
3. ✅ All tests still pass
4. ✅ Test helpers work correctly
5. ✅ `cargo tree | grep hyper` shows only transitive (via reqwest/axum)

### After Phase 3 (If Implemented)

1. ✅ Only one version of `reqwest` in `cargo tree` (0.13)
2. ✅ No `jsonschema` dependency (if removed)
3. ✅ Alternative validation works correctly
4. ✅ All validation tests pass
5. ✅ Binary sizes reduced by additional ~2 MB

### Overall Success

✅ **Consolidated HTTP client strategy**
- Single high-level HTTP client (`reqwest`)
- No direct low-level HTTP dependencies
- Modern TLS stack (single version)

✅ **Reduced maintenance burden**
- Fewer dependencies to audit
- Fewer security updates to track
- Cleaner dependency tree

✅ **Improved build times and binary sizes**
- Faster CI/CD pipelines
- Smaller deployment artifacts
- Quicker development iteration

✅ **Better developer experience**
- Clearer architecture
- Easier to understand dependencies
- Documented strategy for HTTP clients

---

## Appendix: Code Examples

### Example: reqwest-eventsource Basic Usage

```rust
use reqwest_eventsource::{Event, EventSource, Error};
use futures::StreamExt;

async fn consume_sse_stream(url: &str) -> Result<(), Error> {
    let mut stream = EventSource::get(url);
    
    while let Some(event) = stream.next().await {
        match event {
            Ok(Event::Open) => {
                println!("SSE connection opened");
            }
            Ok(Event::Message(msg)) => {
                println!("Event type: {}", msg.event);
                println!("Data: {}", msg.data);
                println!("ID: {:?}", msg.id);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}
```

### Example: reqwest-eventsource with Authentication

```rust
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use reqwest_eventsource::EventSource;

async fn authenticated_sse_stream(url: &str, token: &str) -> EventSource {
    // Option 1: URL parameter (current approach)
    let url_with_token = format!("{}?token={}", url, token);
    EventSource::get(&url_with_token)
    
    // Option 2: Authorization header (better security)
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap()
    );
    
    let client = Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    
    EventSource::new(client.get(url))
}
```

### Example: Axum Body to JSON (Phase 2)

```rust
use axum::body::Body;
use serde::de::DeserializeOwned;

async fn body_to_json<T: DeserializeOwned>(body: Body) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = axum::body::to_bytes(body, usize::MAX).await?;
    let value = serde_json::from_slice(&bytes)?;
    Ok(value)
}

// Usage in tests:
let response = app.oneshot(request).await?;
let json: MyStruct = body_to_json(response.into_body()).await?;
assert_eq!(json.field, expected_value);
```

---

## References

### Libraries

- **reqwest-eventsource**: https://crates.io/crates/reqwest-eventsource
  - Docs: https://docs.rs/reqwest-eventsource/latest/reqwest_eventsource/
  - GitHub: https://github.com/jpopesculian/reqwest-eventsource

- **reqwest**: https://crates.io/crates/reqwest
  - Docs: https://docs.rs/reqwest/latest/reqwest/

- **axum**: https://crates.io/crates/axum
  - Body utilities: https://docs.rs/axum/latest/axum/body/

### Specifications

- **Server-Sent Events**: https://html.spec.whatwg.org/multipage/server-sent-events.html
- **HTTP/1.1**: https://httpwg.org/specs/rfc9110.html

### Related Documentation

- `docs/dependency-deduplication.md` - General dependency analysis
- `docs/dependency-deduplication-results.md` - Phase 1 results
- `docs/api-sse-streaming.md` - SSE implementation details (if exists)

---

**Document Status**: Ready for Implementation  
**Next Step**: Implement Phase 1  
**Owner**: Engineering Team  
**Last Updated**: 2026-01-28