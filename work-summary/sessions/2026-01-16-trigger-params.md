# Work Summary: Rule Trigger Parameters Feature

**Date:** 2026-01-16  
**Session Focus:** Adding trigger_params field to rules for event filtering and trigger configuration

---

## Overview

Implemented a new `trigger_params` field for the Rule table that allows rules to configure trigger behavior and filter which events should activate the rule. This complements the existing `action_params` field and enables more flexible rule definitions.

---

## Problem Statement

The rule table had `action_params` to configure how actions should be executed, but lacked a corresponding field to configure trigger behavior or filter events. This meant:

1. All events from a trigger type would match all rules referencing that trigger
2. No way to have multiple rules share the same trigger but respond to different event subsets
3. Event filtering could only be done via complex `conditions` expressions
4. No declarative way to specify what types of events a rule is interested in

---

## Solution

Added a `trigger_params` JSONB field to the `attune.rule` table that stores parameters for:
- Event filtering (e.g., severity levels, service names)
- Trigger configuration (e.g., thresholds, durations)
- Metadata about event matching requirements

---

## Changes Made

### 1. Database Migration

**File:** `migrations/20240103000004_add_rule_trigger_params.sql`

- Added `trigger_params JSONB DEFAULT '{}'::jsonb` column to `attune.rule` table
- Created GIN index on `trigger_params` for efficient querying
- Added column comment documenting purpose

### 2. Data Model Updates

**File:** `crates/common/src/models.rs`

- Added `trigger_params: JsonValue` field to `Rule` struct after `action_params`

### 3. Repository Layer Updates

**File:** `crates/common/src/repositories/rule.rs`

- Added `trigger_params` to `CreateRuleInput` struct
- Added `trigger_params` to `UpdateRuleInput` struct
- Updated all SQL queries to include `trigger_params` column:
  - `find_by_id()` - SELECT query
  - `find_by_ref()` - SELECT query
  - `list()` - SELECT query
  - `create()` - INSERT query (13 parameters now)
  - `update()` - Dynamic UPDATE query builder
  - `find_by_pack()` - SELECT query
  - `find_by_action()` - SELECT query
  - `find_by_trigger()` - SELECT query
  - `find_enabled()` - SELECT query

### 4. API DTO Updates

**File:** `crates/api/src/dto/rule.rs`

- Added `trigger_params` field to `CreateRuleRequest` (defaults to `{}`)
- Added `trigger_params` field to `UpdateRuleRequest` (optional)
- Added `trigger_params` field to `RuleResponse`
- Updated `From<Rule>` implementation for `RuleResponse`
- Updated all test cases to include `trigger_params`

### 5. API Route Handler Updates

**File:** `crates/api/src/routes/rules.rs`

- Updated `create_rule()` handler to pass `trigger_params` to repository
- Updated `update_rule()` handler to pass `trigger_params` to repository
- Updated `enable_rule()` handler to include `trigger_params: None` in update input
- Updated `disable_rule()` handler to include `trigger_params: None` in update input

### 6. Axum 0.7 Route Syntax Fix

**Files:** All route files in `crates/api/src/routes/`

Fixed panic caused by Axum 0.7 breaking change where route path parameters changed from `:param` to `{param}` syntax:

- `packs.rs` - Updated `:ref` and `:id` to `{ref}` and `{id}`
- `actions.rs` - Updated `:ref`, `:id`, `:pack_ref`
- `events.rs` - Updated `:id`
- `executions.rs` - Updated `:id`, `:status`, `:enforcement_id`
- `inquiries.rs` - Updated `:id`, `:status`, `:execution_id`
- `keys.rs` - Updated `:ref`
- `rules.rs` - Updated `:ref`, `:id`, `:pack_ref`, `:action_ref`, `:trigger_ref`
- `triggers.rs` - Updated all trigger and sensor route parameters

### 7. Documentation

**File:** `docs/rule-trigger-params.md` (NEW)

Created comprehensive documentation covering:
- Overview and purpose of trigger_params
- Use cases with examples:
  - Event filtering by severity
  - Service-specific monitoring
  - Threshold-based rules
- Architecture flow and evaluation logic
- Comparison of trigger_params vs conditions
- Implementation details for different services
- Best practices
- Schema and validation
- Migration guide
- Multiple real-world examples

---

## Use Cases

### 1. Event Filtering by Severity

Different rules handle different severity levels from the same error trigger:

```json
{
  "ref": "alerts.critical_errors",
  "trigger_ref": "core.error_event",
  "trigger_params": {
    "severity": "critical",
    "min_priority": 5
  },
  "action_ref": "pagerduty.create_incident"
}
```

```json
{
  "ref": "alerts.minor_errors",
  "trigger_ref": "core.error_event",
  "trigger_params": {
    "severity": "warning",
    "max_priority": 2
  },
  "action_ref": "slack.post_message"
}
```

### 2. Service-Specific Monitoring

Multiple rules monitor the same trigger type but for different services:

```json
{
  "ref": "monitoring.api_gateway_health",
  "trigger_ref": "core.health_check",
  "trigger_params": {
    "service": "api-gateway",
    "environment": "production"
  },
  "action_ref": "alerts.notify_team"
}
```

### 3. Threshold-Based Rules

Different alert thresholds for the same metric:

```json
{
  "ref": "metrics.cpu_high_warning",
  "trigger_ref": "monitoring.cpu_usage",
  "trigger_params": {
    "threshold": 80,
    "comparison": "greater_than",
    "duration_seconds": 300
  },
  "action_ref": "slack.post_message"
}
```

---

## Technical Details

### Database Schema

```sql
ALTER TABLE attune.rule
ADD COLUMN trigger_params JSONB DEFAULT '{}'::jsonb;

CREATE INDEX idx_rule_trigger_params_gin ON attune.rule USING GIN (trigger_params);
```

### Default Value

All existing rules automatically get `trigger_params = {}` which means "match all events from this trigger" (no filtering). This ensures backward compatibility.

### Trigger Params vs Conditions

| Feature | `trigger_params` | `conditions` |
|---------|------------------|--------------|
| **Purpose** | Declare intent about which events this rule handles | Complex conditional logic for rule activation |
| **Format** | Simple JSON key-value pairs | JSON Logic expressions or complex DSL |
| **Evaluation** | Direct comparison/matching | Expression evaluation engine |
| **Use Case** | Event filtering, metadata | Business logic, complex conditions |
| **Performance** | Fast direct matching | May require expression parsing |

---

## Testing Performed

1. ✅ Database migration applied successfully
2. ✅ `attune-common` crate compiles
3. ✅ `attune-api` service compiles
4. ✅ API service starts without errors
5. ✅ All route handlers updated correctly
6. ✅ Axum 0.7 route syntax issues resolved

---

## Future Work

### Executor Service Integration

The Executor service will need to be updated to use `trigger_params` when evaluating which rules should fire for an event:

```rust
// Pseudo-code for future implementation
async fn evaluate_rules_for_event(event: &Event) -> Vec<Enforcement> {
    let rules = find_rules_by_trigger(event.trigger_id);
    let mut enforcements = Vec::new();
    
    for rule in rules {
        // NEW: Check trigger_params match
        if !matches_trigger_params(&rule.trigger_params, &event.payload) {
            continue; // Skip this rule
        }
        
        // Existing: Check conditions
        if !evaluate_conditions(&rule.conditions, &event.payload) {
            continue;
        }
        
        // Rule matches - create enforcement
        enforcements.push(create_enforcement(&rule, &event));
    }
    
    enforcements
}
```

### Validation

Consider adding validation that checks if trigger_params match the trigger's param_schema (similar to how action_params should match action's param_schema).

---

## Impact

- **Backward Compatible:** Existing rules default to `{}` (no filtering)
- **API Compatible:** New field is optional in create/update requests
- **Performance:** GIN index enables efficient querying of trigger_params
- **Flexibility:** Enables more expressive rule definitions
- **Organization:** Multiple rules can share triggers without conflicts

---

## Files Modified

1. `migrations/20240103000004_add_rule_trigger_params.sql` - NEW
2. `crates/common/src/models.rs` - Added field to Rule struct
3. `crates/common/src/repositories/rule.rs` - Updated all queries and structs
4. `crates/api/src/dto/rule.rs` - Updated all DTOs
5. `crates/api/src/routes/rules.rs` - Updated handlers
6. `crates/api/src/routes/packs.rs` - Fixed Axum 0.7 route syntax
7. `crates/api/src/routes/actions.rs` - Fixed Axum 0.7 route syntax
8. `crates/api/src/routes/events.rs` - Fixed Axum 0.7 route syntax
9. `crates/api/src/routes/executions.rs` - Fixed Axum 0.7 route syntax
10. `crates/api/src/routes/inquiries.rs` - Fixed Axum 0.7 route syntax
11. `crates/api/src/routes/keys.rs` - Fixed Axum 0.7 route syntax
12. `crates/api/src/routes/triggers.rs` - Fixed Axum 0.7 route syntax
13. `docs/rule-trigger-params.md` - NEW (comprehensive documentation)
14. `CHANGELOG.md` - Added entry for this feature

---

## Conclusion

Successfully implemented the `trigger_params` feature for rules, providing a declarative way to filter events and configure trigger behavior. The implementation is backward compatible, well-documented, and sets the foundation for more flexible rule definitions in the Attune platform.

The bonus fix for Axum 0.7 route syntax ensures the API service runs correctly after the recent dependency upgrades.