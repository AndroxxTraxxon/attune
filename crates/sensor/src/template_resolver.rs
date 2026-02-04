//! Template Resolver
//!
//! Resolves template variables in rule action parameters using context from
//! trigger payloads, pack configuration, and system variables.
//!
//! Supports template syntax: `{{ source.path.to.value }}`
//!
//! Example:
//! ```rust
//! use serde_json::json;
//! use attune_sensor::template_resolver::{TemplateContext, resolve_templates};
//!
//! let params = json!({
//!     "message": "Error in {{ trigger.payload.service }}"
//! });
//!
//! let context = TemplateContext {
//!     trigger_payload: json!({"service": "api-gateway"}),
//!     pack_config: json!({}),
//!     system_vars: json!({}),
//! };
//!
//! let resolved = resolve_templates(&params, &context).unwrap();
//! assert_eq!(resolved["message"], "Error in api-gateway");
//! ```

use anyhow::Result;
use regex::Regex;
use serde_json::Value as JsonValue;
use std::sync::LazyLock;
use tracing::{debug, warn};

/// Template context containing all available data sources
#[derive(Debug, Clone)]
pub struct TemplateContext {
    /// Event/trigger payload data
    pub trigger_payload: JsonValue,
    /// Pack configuration
    pub pack_config: JsonValue,
    /// System-provided variables
    pub system_vars: JsonValue,
}

impl TemplateContext {
    /// Create a new template context
    pub fn new(trigger_payload: JsonValue, pack_config: JsonValue, system_vars: JsonValue) -> Self {
        Self {
            trigger_payload,
            pack_config,
            system_vars,
        }
    }

    /// Get a value from the context using a dotted path
    ///
    /// Supports paths like:
    /// - `trigger.payload.field`
    /// - `pack.config.setting`
    /// - `system.timestamp`
    pub fn get_value(&self, path: &str) -> Option<JsonValue> {
        let parts: Vec<&str> = path.split('.').collect();

        if parts.is_empty() {
            return None;
        }

        // Determine the root source
        let root = match parts[0] {
            "trigger" => {
                // trigger.payload.* paths
                if parts.len() < 2 || parts[1] != "payload" {
                    warn!(
                        "Invalid trigger path: {}, expected 'trigger.payload.*'",
                        path
                    );
                    return None;
                }
                &self.trigger_payload
            }
            "pack" => {
                // pack.config.* paths
                if parts.len() < 2 || parts[1] != "config" {
                    warn!("Invalid pack path: {}, expected 'pack.config.*'", path);
                    return None;
                }
                &self.pack_config
            }
            "system" => &self.system_vars,
            _ => {
                warn!("Unknown template source: {}", parts[0]);
                return None;
            }
        };

        // Navigate the path (skip the first 2 parts for trigger/pack, 1 for system)
        let skip_count = match parts[0] {
            "trigger" | "pack" => 2,
            "system" => 1,
            _ => return None,
        };

        extract_nested_value(root, &parts[skip_count..])
    }
}

/// Regex pattern to match template variables: {{ ... }}
static TEMPLATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").expect("Failed to compile template regex")
});

/// Resolve all template variables in a JSON value
///
/// Recursively processes objects and arrays, replacing template strings
/// with values from the context.
pub fn resolve_templates(value: &JsonValue, context: &TemplateContext) -> Result<JsonValue> {
    match value {
        JsonValue::String(s) => resolve_string_template(s, context),
        JsonValue::Object(map) => {
            let mut resolved = serde_json::Map::new();
            for (key, val) in map {
                resolved.insert(key.clone(), resolve_templates(val, context)?);
            }
            Ok(JsonValue::Object(resolved))
        }
        JsonValue::Array(arr) => {
            let resolved: Result<Vec<JsonValue>> =
                arr.iter().map(|v| resolve_templates(v, context)).collect();
            Ok(JsonValue::Array(resolved?))
        }
        // Pass through other types unchanged
        other => Ok(other.clone()),
    }
}

/// Resolve templates in a string value
///
/// If the string contains a single template that matches the entire string,
/// returns the value with its original type (preserving numbers, booleans, etc).
///
/// If the string contains multiple templates or mixed content, performs
/// string interpolation.
fn resolve_string_template(s: &str, context: &TemplateContext) -> Result<JsonValue> {
    // Check if the entire string is a single template (for type preservation)
    if let Some(captures) = TEMPLATE_REGEX.captures(s) {
        let full_match = captures.get(0).unwrap();
        if full_match.start() == 0 && full_match.end() == s.len() {
            // Single template - preserve type
            let path = captures.get(1).unwrap().as_str().trim();
            debug!("Resolving single template: {}", path);

            return match context.get_value(path) {
                Some(value) => {
                    debug!("Resolved {} -> {:?}", path, value);
                    Ok(value)
                }
                None => {
                    warn!("Template variable not found: {}", path);
                    Ok(JsonValue::Null)
                }
            };
        }
    }

    // Multiple templates or mixed content - perform string interpolation
    let mut result = s.to_string();
    let mut any_replaced = false;

    for captures in TEMPLATE_REGEX.captures_iter(s) {
        let full_match = captures.get(0).unwrap().as_str();
        let path = captures.get(1).unwrap().as_str().trim();

        debug!("Resolving template in string: {}", path);

        match context.get_value(path) {
            Some(value) => {
                let replacement = value_to_string(&value);
                debug!("Resolved {} -> {}", path, replacement);
                result = result.replace(full_match, &replacement);
                any_replaced = true;
            }
            None => {
                warn!("Template variable not found: {}", path);
                result = result.replace(full_match, "");
            }
        }
    }

    if any_replaced {
        debug!("String interpolation result: {}", result);
    }

    Ok(JsonValue::String(result))
}

/// Extract a nested value from JSON using a path
fn extract_nested_value(root: &JsonValue, path: &[&str]) -> Option<JsonValue> {
    if path.is_empty() {
        return Some(root.clone());
    }

    let mut current = root;

    for part in path {
        match current {
            JsonValue::Object(map) => {
                current = map.get(*part)?;
            }
            JsonValue::Array(arr) => {
                // Try to parse part as array index
                if let Ok(index) = part.parse::<usize>() {
                    current = arr.get(index)?;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

/// Convert a JSON value to a string for interpolation
fn value_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => String::new(),
        JsonValue::Array(_) | JsonValue::Object(_) => {
            // For complex types, serialize as JSON
            serde_json::to_string(value).unwrap_or_else(|_| String::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_context() -> TemplateContext {
        TemplateContext {
            trigger_payload: json!({
                "service": "api-gateway",
                "message": "Connection timeout",
                "severity": "critical",
                "count": 42,
                "enabled": true,
                "metadata": {
                    "host": "web-01",
                    "port": 8080
                },
                "tags": ["production", "backend"]
            }),
            pack_config: json!({
                "api_token": "secret123",
                "alert_channel": "#incidents",
                "timeout": 30
            }),
            system_vars: json!({
                "timestamp": "2026-01-17T15:30:00Z",
                "rule": {
                    "id": 42,
                    "ref": "test.rule"
                },
                "event": {
                    "id": 123
                }
            }),
        }
    }

    #[test]
    fn test_simple_string_substitution() {
        let context = create_test_context();
        let template = json!({
            "message": "Hello {{ trigger.payload.service }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["message"], "Hello api-gateway");
    }

    #[test]
    fn test_single_template_type_preservation() {
        let context = create_test_context();

        // Number
        let template = json!({"count": "{{ trigger.payload.count }}"});
        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["count"], 42);

        // Boolean
        let template = json!({"enabled": "{{ trigger.payload.enabled }}"});
        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["enabled"], true);
    }

    #[test]
    fn test_nested_object_access() {
        let context = create_test_context();
        let template = json!({
            "host": "{{ trigger.payload.metadata.host }}",
            "port": "{{ trigger.payload.metadata.port }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["host"], "web-01");
        assert_eq!(result["port"], 8080);
    }

    #[test]
    fn test_array_access() {
        let context = create_test_context();
        let template = json!({
            "first_tag": "{{ trigger.payload.tags.0 }}",
            "second_tag": "{{ trigger.payload.tags.1 }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["first_tag"], "production");
        assert_eq!(result["second_tag"], "backend");
    }

    #[test]
    fn test_pack_config_reference() {
        let context = create_test_context();
        let template = json!({
            "token": "{{ pack.config.api_token }}",
            "channel": "{{ pack.config.alert_channel }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["token"], "secret123");
        assert_eq!(result["channel"], "#incidents");
    }

    #[test]
    fn test_system_variables() {
        let context = create_test_context();
        let template = json!({
            "timestamp": "{{ system.timestamp }}",
            "rule_id": "{{ system.rule.id }}",
            "event_id": "{{ system.event.id }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["timestamp"], "2026-01-17T15:30:00Z");
        assert_eq!(result["rule_id"], 42);
        assert_eq!(result["event_id"], 123);
    }

    #[test]
    fn test_missing_value_returns_null() {
        let context = create_test_context();
        let template = json!({
            "missing": "{{ trigger.payload.nonexistent }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert!(result["missing"].is_null());
    }

    #[test]
    fn test_multiple_templates_in_string() {
        let context = create_test_context();
        let template = json!({
            "message": "Error in {{ trigger.payload.service }}: {{ trigger.payload.message }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(
            result["message"],
            "Error in api-gateway: Connection timeout"
        );
    }

    #[test]
    fn test_static_values_unchanged() {
        let context = create_test_context();
        let template = json!({
            "static": "This is static",
            "number": 123,
            "boolean": false
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["static"], "This is static");
        assert_eq!(result["number"], 123);
        assert_eq!(result["boolean"], false);
    }

    #[test]
    fn test_nested_objects_and_arrays() {
        let context = create_test_context();
        let template = json!({
            "nested": {
                "field1": "{{ trigger.payload.service }}",
                "field2": "{{ pack.config.timeout }}"
            },
            "array": [
                "{{ trigger.payload.severity }}",
                "static value"
            ]
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["nested"]["field1"], "api-gateway");
        assert_eq!(result["nested"]["field2"], 30);
        assert_eq!(result["array"][0], "critical");
        assert_eq!(result["array"][1], "static value");
    }

    #[test]
    fn test_empty_template_context() {
        let context = TemplateContext {
            trigger_payload: json!({}),
            pack_config: json!({}),
            system_vars: json!({}),
        };

        let template = json!({
            "message": "{{ trigger.payload.missing }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert!(result["message"].is_null());
    }

    #[test]
    fn test_whitespace_in_templates() {
        let context = create_test_context();
        let template = json!({
            "message": "{{  trigger.payload.service  }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["message"], "api-gateway");
    }

    #[test]
    fn test_complex_real_world_example() {
        let context = create_test_context();
        let template = json!({
            "channel": "{{ pack.config.alert_channel }}",
            "message": "🚨 Error in {{ trigger.payload.service }}: {{ trigger.payload.message }}",
            "severity": "{{ trigger.payload.severity }}",
            "details": {
                "host": "{{ trigger.payload.metadata.host }}",
                "count": "{{ trigger.payload.count }}",
                "tags": "{{ trigger.payload.tags }}"
            },
            "timestamp": "{{ system.timestamp }}"
        });

        let result = resolve_templates(&template, &context).unwrap();
        assert_eq!(result["channel"], "#incidents");
        assert_eq!(
            result["message"],
            "🚨 Error in api-gateway: Connection timeout"
        );
        assert_eq!(result["severity"], "critical");
        assert_eq!(result["details"]["host"], "web-01");
        assert_eq!(result["details"]["count"], 42);
        assert_eq!(result["timestamp"], "2026-01-17T15:30:00Z");
    }
}
