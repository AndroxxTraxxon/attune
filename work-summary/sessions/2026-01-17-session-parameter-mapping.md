# Session Summary: Rule Parameter Mapping Documentation
**Date:** 2026-01-17
**Duration:** ~2 hours
**Focus:** Discovery and documentation of rule action parameter mapping requirements

## Session Overview

User realized that the Rule table lacks functionality for dynamic parameter mapping from trigger payloads and pack configurations. Upon investigation, discovered that the database schema and API already support `action_params`, but only for static values. This session focused on documenting the requirements and design for dynamic parameter templating.

## What We Discovered

### Current State

1. **Database:** `action_params` column already exists in `rule` table (added in migration `20240103000003_add_rule_action_params.sql`)
2. **API:** DTOs already include `action_params` field in create/update requests and responses
3. **Data Flow:** Parameters flow from Rule → Enforcement → Execution → Worker
4. **Limitation:** Only static values are supported - no dynamic extraction from event payload or pack config

### Current Implementation

```rust
// In attune/crates/sensor/src/rule_matcher.rs:309
let config = Some(&rule.action_params);
```

This line simply copies static `action_params` to the enforcement. No template resolution happens.

### What's Missing

**Dynamic parameter mapping using templates:**
- `{{ trigger.payload.field }}` - Extract from event payload
- `{{ pack.config.setting }}` - Reference pack configuration
- `{{ system.timestamp }}` - System-provided values

## Work Completed

### 1. Comprehensive Documentation Created

**File:** `docs/rule-parameter-mapping.md` (742 lines)

A complete guide covering:
- Template syntax (`{{ source.path.to.value }}`)
- All three parameter types (static, trigger payload, pack config)
- Nested object and array access
- Default values and filters (future enhancement)
- Real-world examples (Slack, JIRA, PagerDuty, HTTP)
- Implementation details and data flow
- Security considerations
- Testing strategy
- Migration guide
- Troubleshooting
- Best practices

### 2. API Documentation Updated

**File:** `docs/api-rules.md`

Updated to include:
- `action_params` field in data model examples
- Field descriptions explaining template support
- Examples showing dynamic parameter mapping
- Reference to detailed parameter mapping documentation
- Create and update endpoint examples with templates

### 3. Implementation Plan Created

**File:** `work-summary/2026-01-17-parameter-templating.md` (561 lines)

Detailed technical plan including:
- Current vs desired behavior with examples
- Architecture decision (resolve in sensor service)
- Template syntax specification
- Data sources (trigger, pack config, system)
- Two-phase implementation plan (MVP + advanced features)
- Code structure and testing strategy
- Performance considerations
- Security analysis
- Timeline estimates (2-3 days for MVP)
- Success criteria

## Key Design Decisions

### 1. Where to Resolve Templates?

**Decision: Sensor Service (at enforcement creation)**

**Rationale:**
- ✅ Resolves once, not on every execution
- ✅ Audit trail shows actual parameters used
- ✅ Can replay executions with same parameters
- ✅ Template errors caught early
- ✅ Less load on executor/worker

### 2. Template Syntax

**Decision: Simple `{{ path.to.value }}` syntax**

**Rationale:**
- Similar to Jinja2/Handlebars (familiar to users)
- Simple to implement (regex + JSON path traversal)
- No code execution (security)
- Sufficient for 80% of use cases

### 3. Missing Value Behavior

**Decision: Use `null` and log warning**

**Rationale:**
- Allows actions to handle missing data
- Visible in logs for debugging
- Doesn't break JSON structure

### 4. Type Preservation

**Decision: Preserve JSON types when possible**

**Example:**
```json
{
  "count": "{{ trigger.payload.count }}"  // If count=42, result is number 42, not "42"
}
```

**Rationale:**
- Actions expect correct types
- String interpolation only when needed
- More intuitive behavior

## Implementation Requirements

### Phase 1: MVP (2-3 days)

1. **Create `template_resolver.rs` module**
   - Regex to find `{{ }}` patterns
   - JSON path parser with dot notation
   - Type-preserving substitution
   - Error handling for missing values

2. **Update `rule_matcher.rs`**
   - Load pack config from database
   - Build template context (trigger, pack, system)
   - Call template resolver before creating enforcement
   - Handle resolution errors gracefully

3. **Testing**
   - Unit tests for template resolution
   - Integration tests for end-to-end flow
   - Test missing values, nested access, arrays

4. **Documentation**
   - Already complete! ✅

### Phase 2: Advanced Features (1-2 days, future)

1. **Default values:** `{{ field | default: 'value' }}`
2. **Filters:** `upper`, `lower`, `trim`, `date`, `truncate`, `json`
3. **Advanced use cases** based on user feedback

## Data Flow

```
Rule (action_params with {{ }} templates)
  ↓
Sensor Service (template_resolver.rs)
  ↓ Loads pack config, builds context
  ↓ Resolves all {{ }} patterns
  ↓
Enforcement (config = resolved parameters)
  ↓
Executor Service
  ↓
Execution (config = enforcement.config)
  ↓
Worker Service
  ↓
Action (receives resolved parameters)
```

## Real-World Examples Documented

### 1. Slack Alert with Dynamic Content
```json
{
  "action_params": {
    "channel": "{{ pack.config.alert_channel }}",
    "token": "{{ pack.config.slack_token }}",
    "message": "⚠️ Alert from {{ trigger.payload.source }}: {{ trigger.payload.message }}"
  }
}
```

### 2. JIRA Ticket Creation
```json
{
  "action_params": {
    "project": "{{ pack.config.jira_project }}",
    "auth": {
      "username": "{{ pack.config.jira_username }}",
      "token": "{{ pack.config.jira_token }}"
    },
    "summary": "[{{ trigger.payload.severity }}] {{ trigger.payload.service }}: {{ trigger.payload.message }}"
  }
}
```

### 3. PagerDuty Incident
```json
{
  "action_params": {
    "routing_key": "{{ pack.config.pagerduty_routing_key }}",
    "payload": {
      "summary": "{{ trigger.payload.metric_name }} exceeded threshold on {{ trigger.payload.host }}",
      "severity": "critical",
      "custom_details": {
        "current_value": "{{ trigger.payload.current_value }}",
        "threshold": "{{ trigger.payload.threshold }}"
      }
    }
  }
}
```

## Security Considerations

### ✅ Safe
- No code execution (only data substitution)
- No SQL/command injection risk
- Access control enforced (rules only access own pack config)
- Backward compatible (static params still work)

### ⚠️ Caution
- Must not log resolved parameters containing secrets
- Pack config secrets should use secrets management
- Template syntax validation to prevent abuse

## Performance Impact

**Estimated overhead per enforcement:**
- Regex pattern matching: ~1-10 µs
- JSON path extraction: ~1-5 µs per template
- Pack config lookup (cached): ~10-100 µs
- **Total: ~50-500 µs per enforcement**

**Conclusion:** Negligible compared to database operations (~1-10 ms) and action execution (100 ms - seconds)

## Backward Compatibility

✅ **100% Backward Compatible**

- Existing rules with static params work unchanged
- No breaking changes to API
- No database migration needed (column already exists)
- Rules without `{{ }}` syntax are unaffected
- Users can migrate incrementally

## Files Created/Modified

### Created (3 files)
1. `docs/rule-parameter-mapping.md` - Complete user documentation (742 lines)
2. `work-summary/2026-01-17-parameter-templating.md` - Implementation plan (561 lines)
3. `work-summary/2026-01-17-session-parameter-mapping.md` - This file

### Modified (1 file)
1. `docs/api-rules.md` - Added action_params examples and documentation

## Next Steps

### Immediate (This Week)
1. Review documentation with stakeholders
2. Prioritize against other roadmap items
3. If approved, create implementation branch

### Implementation (2-3 days)
1. Create `sensor/src/template_resolver.rs` module
2. Implement template parsing and resolution
3. Update `rule_matcher.rs` to use resolver
4. Add pack config loading and caching
5. Write comprehensive tests
6. Manual testing with real scenarios

### Future Enhancements
1. Default values: `{{ field | default: 'value' }}`
2. Filters: `upper`, `lower`, `date`, `truncate`
3. Conditional templates (if needed)
4. Performance optimization (template caching)

## Success Metrics

- [ ] Documentation reviewed and approved
- [ ] Implementation plan validated
- [ ] Timeline and priority agreed
- [ ] No blocking questions remain
- [ ] Ready to begin implementation

## Priority Assessment

**Recommended Priority: P1 (High)**

**Rationale:**
- **Essential for production use** - Most real-world automation needs dynamic parameters
- **User expectation** - StackStorm users expect this functionality
- **No workaround** - Cannot achieve without custom code
- **Low risk** - Clean implementation, backward compatible
- **High value** - Unlocks many automation scenarios

**Comparison:**
- P0 (Blocking): Policy execution ordering, secret passing fix
- **P1 (High): This feature** - Critical for usability
- P2 (Medium): Nice-to-have features

## Questions Raised

### 1. Should we cache pack configs?
**Answer:** Yes, with 5-minute TTL, invalidate on updates

### 2. What happens with missing values?
**Answer:** Use `null`, log warning, continue execution

### 3. Should types be preserved?
**Answer:** Yes, `{{ trigger.payload.count }}` returns number if count is number

### 4. Is this blocking any features?
**Answer:** No, but significantly improves usability and unlocks use cases

## Lessons Learned

1. **Database already prepared** - Previous session added `action_params` column
2. **API already ready** - DTOs support action_params
3. **Clear insertion point** - Sensor service is right place for resolution
4. **Simple solution** - Don't need full template engine, simple regex + JSON paths sufficient
5. **Documentation first** - Writing docs clarified requirements and design

## Comparison to StackStorm

| Feature | StackStorm | Attune (Planned) |
|---------|-----------|------------------|
| Template syntax | `{{ }}` (Jinja2) | `{{ }}` (simplified) |
| Trigger data | `{{ trigger.payload }}` | `{{ trigger.payload }}` |
| Pack config | `{{ config }}` | `{{ pack.config }}` |
| System vars | `{{ st2kv }}` (key-value store) | `{{ system }}` |
| Filters | Full Jinja2 | Basic (Phase 2) |
| Conditionals | Yes | No (not needed) |
| Code execution | Python expressions | No (security) |

**Advantage:** Simpler, more secure, easier to understand

## Related Documentation

- [Rule Management API](../docs/api-rules.md)
- [Rule Parameter Mapping](../docs/rule-parameter-mapping.md) - NEW
- [Sensor Service Architecture](../docs/sensor-service.md)
- [Secrets Management](../docs/secrets-management.md)
- [StackStorm Lessons Learned](./StackStorm-Lessons-Learned.md)

## Conclusion

This session successfully identified and documented a critical usability feature. While the database and API infrastructure already exists (from previous work), the dynamic templating capability is missing. The feature is well-scoped, implementable in 2-3 days, and provides significant value with minimal risk.

The comprehensive documentation created today serves as both:
1. **User documentation** - How to use parameter templating
2. **Implementation guide** - How to build the feature

Ready to proceed with implementation pending priority/timeline approval.

---

**Status:** ✅ Documentation Complete, ⏳ Implementation Pending
**Priority:** P1 (High)
**Estimated Effort:** 2-3 days (MVP), 1-2 days (advanced features)
**Risk:** Low (backward compatible, clear scope)
**Value:** High (unlocks real-world automation scenarios)