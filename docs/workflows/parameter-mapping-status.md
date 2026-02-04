# Parameter Mapping Status

## Quick Reference

This document provides a quick overview of what exists and what needs to be implemented for rule parameter mapping.

---

## ✅ What Already Exists

### Database Schema
- **Migration:** `migrations/20240103000003_add_rule_action_params.sql`
- **Column:** `attune.rule.action_params` (JSONB, default `{}`)
- **Index:** `idx_rule_action_params_gin` (GIN index for efficient querying)
- **Status:** ✅ Complete

### Data Models
- **File:** `crates/common/src/models.rs`
- **Struct:** `rule::Rule` has `pub action_params: JsonValue` field
- **Status:** ✅ Complete

### API Layer
- **File:** `crates/api/src/dto/rule.rs`
- **Request DTOs:**
  - `CreateRuleRequest.action_params` (with default `{}`)
  - `UpdateRuleRequest.action_params` (optional)
- **Response DTOs:**
  - `RuleResponse.action_params`
- **Status:** ✅ Complete

### Repository Layer
- **File:** `crates/common/src/repositories/rule.rs`
- **Operations:**
  - `CreateRuleInput.action_params` included in INSERT
  - `UpdateRuleInput.action_params` handled in UPDATE
  - All SELECT queries include `action_params` column
- **Status:** ✅ Complete

### API Routes
- **File:** `crates/api/src/routes/rules.rs`
- **Handlers:**
  - `create_rule()` accepts `action_params` from request
  - `update_rule()` updates `action_params` if provided
- **Status:** ✅ Complete

### Data Flow (Static Parameters)
```
Rule.action_params (static JSON)
  ↓
Enforcement.config (copied verbatim)
  ↓
Execution.config (passed through)
  ↓
Worker (receives as action parameters)
```
- **Status:** ✅ Working for static values

---

## ❌ What's Missing

### Template Resolution Logic
- **Needed:** Parse and resolve `{{ }}` templates in `action_params`
- **Location:** `crates/sensor/src/` (new module needed)
- **Status:** ❌ Not implemented

### Template Resolver Module
```rust
// NEW FILE: crates/sensor/src/template_resolver.rs
pub struct TemplateContext {
    pub trigger_payload: JsonValue,
    pub pack_config: JsonValue,
    pub system_vars: JsonValue,
}

pub fn resolve_templates(
    params: &JsonValue,
    context: &TemplateContext
) -> Result<JsonValue> {
    // Implementation needed
}
```
- **Status:** ❌ Does not exist

### Pack Config Loading
- **Needed:** Load pack configuration from database
- **Current:** Rule matcher doesn't load pack config
- **Required for:** `{{ pack.config.* }}` templates
- **Status:** ❌ Not implemented

### Integration in Rule Matcher
- **File:** `crates/sensor/src/rule_matcher.rs`
- **Method:** `create_enforcement()`
- **Current code (line 309):**
```rust
let config = Some(&rule.action_params);
```
- **Needed code:**
```rust
// Load pack config
let pack_config = self.load_pack_config(&rule.pack_ref).await?;

// Build template context
let context = TemplateContext {
    trigger_payload: event.payload.clone().unwrap_or_default(),
    pack_config,
    system_vars: self.build_system_vars(rule, event),
};

// Resolve templates
let resolved_params = resolve_templates(&rule.action_params, &context)?;
let config = Some(resolved_params);
```
- **Status:** ❌ Not implemented

### Unit Tests
- **File:** `crates/sensor/src/template_resolver.rs` (tests module)
- **Needed tests:**
  - Simple string substitution
  - Nested object access
  - Array element access
  - Type preservation
  - Missing value handling
  - Pack config reference
  - System variables
  - Multiple templates in one string
  - Invalid syntax handling
- **Status:** ❌ Not implemented

### Integration Tests
- **Needed:** End-to-end test of template resolution
- **Scenario:** Create rule with templates → fire event → verify enforcement has resolved params
- **Status:** ❌ Not implemented

---

## 📋 Implementation Checklist

### Phase 1: MVP (2-3 days)

- [ ] **Create template resolver module**
  - [ ] Define `TemplateContext` struct
  - [ ] Implement `resolve_templates()` function
  - [ ] Regex pattern matching for `{{ }}`
  - [ ] JSON path extraction with dot notation
  - [ ] Type preservation logic
  - [ ] Error handling for missing values
  - [ ] Unit tests (9+ test cases)

- [ ] **Add pack config loading**
  - [ ] Add method to load pack config from database
  - [ ] Implement in-memory cache with TTL
  - [ ] Handle missing pack config gracefully

- [ ] **Integrate with rule matcher**
  - [ ] Update `create_enforcement()` method
  - [ ] Load pack config before resolution
  - [ ] Build template context
  - [ ] Call template resolver
  - [ ] Handle resolution errors
  - [ ] Log warnings for missing values

- [ ] **System variables**
  - [ ] Build system context (timestamp, rule ID, event ID)
  - [ ] Document available system variables

- [ ] **Testing**
  - [ ] Unit tests for template resolver
  - [ ] Integration test: end-to-end flow
  - [ ] Test with missing values
  - [ ] Test with nested objects
  - [ ] Test with arrays
  - [ ] Test performance (benchmark)

- [ ] **Documentation**
  - [x] User documentation (`docs/rule-parameter-mapping.md`) ✅
  - [x] API documentation updates (`docs/api-rules.md`) ✅
  - [ ] Code documentation (inline comments)
  - [ ] Update sensor service docs

### Phase 2: Advanced Features (1-2 days, future)

- [ ] **Default values**
  - [ ] Parse `| default: 'value'` syntax
  - [ ] Apply defaults when value is null/missing
  - [ ] Unit tests

- [ ] **Filters**
  - [ ] `upper` - Convert to uppercase
  - [ ] `lower` - Convert to lowercase
  - [ ] `trim` - Remove whitespace
  - [ ] `date: <format>` - Format timestamp
  - [ ] `truncate: <length>` - Truncate string
  - [ ] `json` - Serialize to JSON string
  - [ ] Unit tests for each filter

- [ ] **Performance optimization**
  - [ ] Cache compiled regex patterns
  - [ ] Skip resolution if no `{{ }}` found
  - [ ] Parallel template resolution
  - [ ] Benchmark improvements

---

## 🔍 Key Implementation Details

### Current Enforcement Creation (line 306-348)

```rust
async fn create_enforcement(&self, rule: &Rule, event: &Event) -> Result<Id> {
    let payload = event.payload.clone().unwrap_or_default();
    let config = Some(&rule.action_params);  // ← This line needs to change

    let enforcement_id = sqlx::query_scalar!(
        r#"
        INSERT INTO attune.enforcement
            (rule, rule_ref, trigger_ref, config, event, status, payload, condition, conditions)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
        Some(rule.id),
        &rule.r#ref,
        &rule.trigger_ref,
        config,  // ← Resolved params go here
        Some(event.id),
        EnforcementStatus::Created as EnforcementStatus,
        payload,
        EnforcementCondition::All as EnforcementCondition,
        &rule.conditions
    )
    .fetch_one(&self.db)
    .await?;

    // ... rest of method
}
```

### Template Examples

**Input (Rule):**
```json
{
  "action_params": {
    "message": "Error in {{ trigger.payload.service }}: {{ trigger.payload.message }}",
    "channel": "{{ pack.config.alert_channel }}",
    "severity": "{{ trigger.payload.severity }}"
  }
}
```

**Context:**
```json
{
  "trigger": {
    "payload": {
      "service": "api-gateway",
      "message": "Connection timeout",
      "severity": "critical"
    }
  },
  "pack": {
    "config": {
      "alert_channel": "#incidents"
    }
  }
}
```

**Output (Enforcement):**
```json
{
  "config": {
    "message": "Error in api-gateway: Connection timeout",
    "channel": "#incidents",
    "severity": "critical"
  }
}
```

---

## 📊 Dependencies

### Existing (Already in Cargo.toml)
- `serde_json` - JSON manipulation ✅
- `regex` - Pattern matching ✅
- `anyhow` - Error handling ✅
- `sqlx` - Database access ✅

### New Dependencies
- **None required** - Can implement with existing dependencies

---

## 🎯 Success Criteria

- [ ] Static parameters continue to work unchanged
- [ ] Can reference `{{ trigger.payload.* }}` fields
- [ ] Can reference `{{ pack.config.* }}` fields
- [ ] Can reference `{{ system.* }}` variables
- [ ] Type preservation (strings, numbers, booleans, objects, arrays)
- [ ] Nested object access with dot notation works
- [ ] Array element access by index works
- [ ] Missing values handled gracefully (null + warning)
- [ ] Invalid syntax handled gracefully (literal + error)
- [ ] Unit tests pass (90%+ coverage)
- [ ] Integration tests pass
- [ ] Documentation accurate and complete
- [ ] No performance regression (<500µs overhead)
- [ ] Backward compatibility maintained (100%)

---

## 🚀 Getting Started

1. **Read documentation:**
   - `docs/rule-parameter-mapping.md` - User guide
   - `work-summary/2026-01-17-parameter-templating.md` - Technical spec

2. **Review current code:**
   - `crates/sensor/src/rule_matcher.rs:306-348` - Where to integrate
   - `crates/common/src/models.rs` - Rule model structure
   - `migrations/20240103000003_add_rule_action_params.sql` - Schema

3. **Start implementation:**
   - Create `crates/sensor/src/template_resolver.rs`
   - Write unit tests first (TDD approach)
   - Implement template parsing and resolution
   - Integrate with rule_matcher
   - Run integration tests

4. **Test thoroughly:**
   - Unit tests for all edge cases
   - Integration test with real database
   - Manual testing with example rules
   - Performance benchmarks

---

## 📚 Related Documentation

- [Rule Parameter Mapping Guide](./rule-parameter-mapping.md) - Complete user documentation
- [Rule Management API](./api-rules.md) - API reference with examples
- [Sensor Service Architecture](./sensor-service.md) - Service overview
- [Implementation Plan](../work-summary/2026-01-17-parameter-templating.md) - Technical specification
- [Session Summary](../work-summary/2026-01-17-session-parameter-mapping.md) - Discovery notes

---

## 🏷️ Status Summary

| Component | Status | Notes |
|-----------|--------|-------|
| Database schema | ✅ Complete | `action_params` column exists |
| Data models | ✅ Complete | Rule struct has field |
| API DTOs | ✅ Complete | Request/response support |
| API routes | ✅ Complete | CRUD operations work |
| Repository | ✅ Complete | All queries include field |
| Static parameters | ✅ Working | Flow end-to-end |
| Template resolver | ❌ Missing | Core implementation needed |
| Pack config loading | ❌ Missing | Required for `{{ pack.config }}` |
| Integration | ❌ Missing | Need to wire up resolver |
| Unit tests | ❌ Missing | Tests for resolver needed |
| Integration tests | ❌ Missing | E2E test needed |
| Documentation | ✅ Complete | User and tech docs done |

**Overall Status:** 📝 Documented, ⏳ Implementation Pending

**Priority:** P1 (High)

**Estimated Effort:** 2-3 days (MVP), 1-2 days (advanced features)

**Risk:** Low (backward compatible, well-scoped, clear requirements)

**Value:** High (unlocks production use cases, user expectation)