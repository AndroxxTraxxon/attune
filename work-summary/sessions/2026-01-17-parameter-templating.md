# Work Summary: Rule Parameter Templating
**Date:** 2026-01-17  
**Session Focus:** Documenting rule parameter templating requirements

## Overview

Currently, the rule system supports `action_params` as a static JSONB field that gets copied directly from the rule to the enforcement and execution. This work item is about adding **dynamic parameter templating** to enable:

1. **Static values** (already works) - Hard-coded values in rules
2. **Dynamic from trigger payload** - Extract values from event data using `{{ trigger.payload.* }}`
3. **Dynamic from pack config** - Reference pack configuration using `{{ pack.config.* }}`
4. **System variables** - Access system-provided values using `{{ system.* }}`

## Current Behavior

**Rule Definition:**
```json
{
  "action_params": {
    "message": "hello, world",
    "channel": "#alerts"
  }
}
```

**Result:** These exact values are passed to the action (static only).

## Desired Behavior

**Rule Definition:**
```json
{
  "action_params": {
    "message": "Error in {{ trigger.payload.service }}: {{ trigger.payload.message }}",
    "channel": "{{ pack.config.alert_channel }}",
    "severity": "{{ trigger.payload.severity }}",
    "timestamp": "{{ system.timestamp }}"
  }
}
```

**Event Payload:**
```json
{
  "service": "api-gateway",
  "message": "Database connection timeout",
  "severity": "critical"
}
```

**Pack Config:**
```json
{
  "alert_channel": "#incidents",
  "api_token": "xoxb-secret-token"
}
```

**Result (Resolved Parameters):**
```json
{
  "message": "Error in api-gateway: Database connection timeout",
  "channel": "#incidents",
  "severity": "critical",
  "timestamp": "2026-01-17T15:30:00Z"
}
```

## Implementation Location

The template resolution should happen in **`attune/crates/sensor/src/rule_matcher.rs`** in the `create_enforcement()` method, specifically at line 309 where:

```rust
let config = Some(&rule.action_params);
```

This currently passes the raw `action_params` directly. We need to add a template resolution step:

```rust
// Resolve templates in action_params
let resolved_params = self.resolve_parameter_templates(
    &rule.action_params,
    event,
    &rule.pack_ref
).await?;

let config = Some(resolved_params);
```

## Architecture Decision

### Where to Resolve Templates?

**Option 1: Sensor Service (RECOMMENDED)**
- ✅ Resolves once at enforcement creation
- ✅ Enforcement stores resolved values (audit trail)
- ✅ Can replay execution with same parameters
- ✅ Less load on executor/worker
- ✅ Template errors caught early in pipeline
- ❌ Can't override at execution time

**Option 2: Executor Service**
- ✅ Could allow execution-time overrides
- ❌ Resolves every time enforcement is processed
- ❌ More complex to handle template errors
- ❌ Enforcement doesn't show actual parameters used

**Option 3: Worker Service**
- ❌ Too late to handle errors gracefully
- ❌ Makes worker more complex
- ❌ Can't see resolved params in enforcement/execution records

**Decision: Option 1 (Sensor Service)** - Resolve at enforcement creation time.

## Template Syntax

Using a simple `{{ path.to.value }}` syntax inspired by Jinja2/Handlebars but simplified:

### Basic Reference
```
{{ trigger.payload.field_name }}
```

### Nested Access
```
{{ trigger.payload.user.profile.email }}
```

### Array Access
```
{{ trigger.payload.errors.0 }}
{{ trigger.payload.tags.1 }}
```

### Default Values (Future)
```
{{ trigger.payload.priority | default: 'medium' }}
{{ pack.config.timeout | default: 30 }}
```

### Filters (Future)
```
{{ trigger.payload.name | upper }}
{{ trigger.payload.email | lower }}
{{ trigger.payload.timestamp | date: '%Y-%m-%d' }}
```

## Data Sources

### 1. Trigger Payload (`trigger.payload.*`)

Access data from the event that triggered the rule:

```json
{
  "trigger": {
    "payload": {
      "service": "api-gateway",
      "severity": "error",
      "metadata": {
        "host": "server-01",
        "port": 8080
      }
    }
  }
}
```

**Available in:** `event.payload` (already have this)

### 2. Pack Config (`pack.config.*`)

Access pack configuration values:

```json
{
  "pack": {
    "config": {
      "api_token": "secret-token",
      "alert_channel": "#incidents",
      "webhook_url": "https://hooks.example.com/webhook"
    }
  }
}
```

**Need to fetch:** Load pack config from database using `rule.pack` or `rule.pack_ref`

### 3. System Variables (`system.*`)

System-provided values:

```json
{
  "system": {
    "timestamp": "2026-01-17T15:30:00Z",
    "rule": {
      "id": 123,
      "ref": "mypack.myrule"
    },
    "event": {
      "id": 456
    }
  }
}
```

**Available from:** Rule and event objects already in scope

## Implementation Plan

### Phase 1: Basic Template Resolution (MVP)

1. **Create Template Resolver Module** (`attune/crates/sensor/src/template_resolver.rs`)
   - `resolve_templates(params: &JsonValue, context: &TemplateContext) -> Result<JsonValue>`
   - `extract_value(path: &str, context: &TemplateContext) -> Option<JsonValue>`
   - Regex to find `{{ ... }}` patterns
   - Dot notation path parser
   - Type preservation (strings, numbers, booleans, objects, arrays)

2. **Define Template Context**
   ```rust
   pub struct TemplateContext {
       pub trigger_payload: JsonValue,
       pub pack_config: JsonValue,
       pub system_vars: JsonValue,
   }
   ```

3. **Update RuleMatcher**
   - Add method to load pack config
   - Build template context
   - Call template resolver
   - Handle resolution errors gracefully

4. **Error Handling**
   - Missing values → use `null` or empty string, log warning
   - Invalid syntax → use literal string, log error
   - Type conversion errors → log error, use string representation

### Phase 2: Advanced Features (Future)

1. **Default Values**
   - `{{ trigger.payload.priority | default: 'medium' }}`
   - Fallback when value is null/missing

2. **Filters**
   - `upper`, `lower`, `trim` - String manipulation
   - `date` - Date formatting
   - `truncate`, `length` - String operations
   - `json` - JSON serialization

3. **Conditional Logic (Far Future)**
   - `{% if condition %}...{% endif %}`
   - More complex than needed for MVP

## Code Structure

```
attune/crates/sensor/src/
├── rule_matcher.rs          # Update create_enforcement()
├── template_resolver.rs     # NEW: Template resolution logic
└── lib.rs                   # Export new module
```

## Testing Strategy

### Unit Tests (template_resolver.rs)

```rust
#[test]
fn test_simple_string_substitution() {
    let template = json!({"message": "Hello {{ trigger.payload.name }}"});
    let context = TemplateContext {
        trigger_payload: json!({"name": "Alice"}),
        pack_config: json!({}),
        system_vars: json!({}),
    };
    let result = resolve_templates(&template, &context).unwrap();
    assert_eq!(result["message"], "Hello Alice");
}

#[test]
fn test_nested_object_access() {
    let template = json!({"host": "{{ trigger.payload.server.hostname }}"});
    let context = TemplateContext {
        trigger_payload: json!({"server": {"hostname": "web-01"}}),
        pack_config: json!({}),
        system_vars: json!({}),
    };
    let result = resolve_templates(&template, &context).unwrap();
    assert_eq!(result["host"], "web-01");
}

#[test]
fn test_type_preservation() {
    let template = json!({"count": "{{ trigger.payload.count }}"});
    let context = TemplateContext {
        trigger_payload: json!({"count": 42}),
        pack_config: json!({}),
        system_vars: json!({}),
    };
    let result = resolve_templates(&template, &context).unwrap();
    assert_eq!(result["count"], 42); // Number, not string
}

#[test]
fn test_missing_value_null() {
    let template = json!({"value": "{{ trigger.payload.missing }}"});
    let context = TemplateContext {
        trigger_payload: json!({}),
        pack_config: json!({}),
        system_vars: json!({}),
    };
    let result = resolve_templates(&template, &context).unwrap();
    assert!(result["value"].is_null());
}

#[test]
fn test_pack_config_reference() {
    let template = json!({"token": "{{ pack.config.api_token }}"});
    let context = TemplateContext {
        trigger_payload: json!({}),
        pack_config: json!({"api_token": "secret123"}),
        system_vars: json!({}),
    };
    let result = resolve_templates(&template, &context).unwrap();
    assert_eq!(result["token"], "secret123");
}
```

### Integration Tests

1. **End-to-End Template Resolution**
   - Create rule with templated action_params
   - Fire event with payload
   - Verify enforcement has resolved parameters
   - Verify execution receives correct parameters

2. **Pack Config Loading**
   - Verify pack config is loaded correctly
   - Test with missing pack config (empty object fallback)

3. **Error Handling**
   - Invalid template syntax
   - Missing values
   - Circular references (shouldn't be possible with our syntax)

## Dependencies

### Existing Dependencies (Already in Cargo.toml)
- `serde_json` - JSON manipulation
- `regex` - Pattern matching for `{{ }}` syntax
- `anyhow` - Error handling

### No New Dependencies Required

## Migration Path

### Backward Compatibility

The implementation MUST be backward compatible:

1. **Static parameters continue to work**
   - Rules without `{{ }}` syntax work unchanged
   - No breaking changes to existing rules

2. **Gradual adoption**
   - Users can migrate rules incrementally
   - Mix static and dynamic parameters

3. **Feature flag (optional)**
   - Could add `enable_parameter_templating` config flag
   - Default: enabled (since it's backward compatible)

## Documentation Updates

1. **Create:** `docs/rule-parameter-mapping.md` ✅ DONE
2. **Update:** `docs/api-rules.md` to mention action_params templating ✅ DONE
3. **Update:** `docs/sensor-service.md` to explain template resolution
4. **Create:** Example rules in `docs/examples/`
5. **Update:** Quick-start guide with templating examples

## Security Considerations

### 1. Secret Exposure
- ✅ Pack config secrets are already handled by secrets management
- ✅ Templates don't enable new secret exposure paths
- ⚠️ Must not log resolved parameters that contain secrets

### 2. Injection Attacks
- ✅ No code execution (only data substitution)
- ✅ No SQL/command injection risk
- ✅ JSON structure is preserved (no string eval)

### 3. Access Control
- ✅ Rules can only access:
  - Their own event payload
  - Their own pack config
  - System-provided values
- ❌ Cannot access other packs' configs
- ❌ Cannot access arbitrary database data

## Performance Considerations

### Template Resolution Cost

**Per enforcement creation:**
1. Regex match for `{{ }}` patterns: ~1-10 µs
2. JSON path extraction per template: ~1-5 µs
3. Pack config lookup (cached): ~10-100 µs
4. Total overhead: ~50-500 µs per enforcement

**Optimization opportunities:**
1. Cache compiled regex patterns
2. Cache pack configs in memory
3. Skip resolution if no `{{ }}` found in params
4. Parallel template resolution for multiple fields

**Acceptable:** This overhead is negligible compared to database operations and action execution.

## Success Criteria

- [ ] Static parameters continue to work unchanged
- [ ] Can reference trigger payload fields: `{{ trigger.payload.* }}`
- [ ] Can reference pack config fields: `{{ pack.config.* }}`
- [ ] Can reference system variables: `{{ system.* }}`
- [ ] Type preservation (strings, numbers, booleans, objects, arrays)
- [ ] Nested object access with dot notation
- [ ] Array element access by index
- [ ] Missing values handled gracefully (null + warning)
- [ ] Invalid syntax handled gracefully (literal + error)
- [ ] Unit tests cover all template resolution cases
- [ ] Integration tests verify end-to-end flow
- [ ] Documentation complete and accurate
- [ ] No performance regression in rule matching

## Example Use Cases

### 1. Dynamic Slack Alerts
```json
{
  "action_params": {
    "channel": "{{ pack.config.alert_channel }}",
    "token": "{{ pack.config.slack_token }}",
    "message": "🔴 Error in {{ trigger.payload.service }}: {{ trigger.payload.message }}",
    "severity": "{{ trigger.payload.severity }}"
  }
}
```

### 2. Webhook to Ticket System
```json
{
  "action_params": {
    "url": "{{ pack.config.jira_url }}/rest/api/2/issue",
    "auth_token": "{{ pack.config.jira_token }}",
    "project": "PROD",
    "summary": "[{{ trigger.payload.severity }}] {{ trigger.payload.service }}",
    "description": "Error occurred at {{ system.timestamp }}\nHost: {{ trigger.payload.host }}\nMessage: {{ trigger.payload.message }}"
  }
}
```

### 3. Metric Threshold Alert
```json
{
  "action_params": {
    "pagerduty_key": "{{ pack.config.pd_routing_key }}",
    "summary": "{{ trigger.payload.metric }} exceeded threshold on {{ trigger.payload.host }}",
    "severity": "critical",
    "details": {
      "current": "{{ trigger.payload.value }}",
      "threshold": "{{ trigger.payload.threshold }}",
      "duration": "{{ trigger.payload.duration_seconds }}"
    }
  }
}
```

## Related Work

- **StackStorm:** Uses Jinja2 templating extensively (Python-based)
- **Ansible:** Similar `{{ variable }}` syntax
- **Terraform:** `${var.name}` syntax
- **GitHub Actions:** `${{ expression }}` syntax

Our syntax is simpler (no logic, just substitution) but sufficient for most automation needs.

## Timeline Estimate

**Phase 1 (MVP):**
- Template resolver module: 4-6 hours
- RuleMatcher integration: 2-3 hours
- Pack config loading: 1-2 hours
- Unit tests: 3-4 hours
- Integration tests: 2-3 hours
- Documentation updates: 2-3 hours
- **Total: 14-21 hours (2-3 days)**

**Phase 2 (Advanced Features):**
- Default values: 2-3 hours
- Filters: 4-6 hours per filter
- Testing: 3-4 hours
- **Total: 10-15 hours (1-2 days)**

## Priority Assessment

**Priority: HIGH (P1)**

**Rationale:**
1. **Essential for real-world use cases** - Most automation needs dynamic parameters
2. **User expectation** - StackStorm users expect this functionality
3. **No workaround** - Can't achieve this without custom code
4. **Relatively low complexity** - Clean implementation without major architectural changes

**Blocking:** Not blocking any other features, but significantly improves usability.

## Next Steps

1. **Review this document** with team/stakeholders
2. **Create implementation branch** `feature/parameter-templating`
3. **Implement Phase 1 (MVP)**
   - Create template_resolver module
   - Update rule_matcher
   - Add tests
   - Update documentation
4. **Test with real-world scenarios**
5. **Merge to main** after review
6. **Plan Phase 2** (advanced features) based on user feedback

## Notes

- Keep implementation simple for MVP
- Avoid over-engineering (no full Jinja2/Liquid parser)
- Focus on 80% use case: simple field substitution
- Advanced features (filters, logic) can wait for v2

## Questions/Decisions Needed

1. **Default behavior for missing values?**
   - Option A: Use `null` (current recommendation)
   - Option B: Use empty string `""`
   - Option C: Keep template literal `"{{ ... }}"`
   - **Decision: Use `null` and log warning**

2. **Should we support string interpolation in non-string fields?**
   - Example: `"count": "{{ trigger.payload.count }}"` where count is a number
   - **Decision: Yes, preserve types when possible**

3. **Cache pack configs?**
   - **Decision: Yes, cache in memory with TTL (5 minutes)**
   - Invalidate on pack config updates

4. **Template resolution timeout?**
   - **Decision: 1 second timeout for complex templates**
   - Fail gracefully if exceeded

## Conclusion

Parameter templating is a critical feature for making Attune usable in real-world scenarios. The implementation is straightforward, backward compatible, and provides significant value with minimal complexity. The MVP can be completed in 2-3 days and immediately unlocks many automation use cases that are currently impossible or require workarounds.