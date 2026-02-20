//! Parameter validation module
//!
//! Validates trigger and action parameters against their declared JSON schemas.
//! Template-aware: values containing `{{ }}` template expressions are replaced
//! with schema-appropriate placeholders before validation, so template expressions
//! pass type checks while literal values are still validated normally.

use attune_common::models::{action::Action, trigger::Trigger};
use jsonschema::Validator;
use serde_json::Value;

use crate::middleware::ApiError;

/// Check if a JSON value is (or contains) a template expression.
fn is_template_expression(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains("{{") && s.contains("}}"),
        _ => false,
    }
}

/// Given a JSON Schema property definition, produce a placeholder value that
/// satisfies the schema's type constraint. This is used to replace template
/// expressions so that JSON Schema validation passes for the remaining
/// (non-template) parts of the parameters.
fn placeholder_for_schema(property_schema: &Value) -> Value {
    // Handle anyOf / oneOf by picking the first variant
    if let Some(any_of) = property_schema.get("anyOf").and_then(|v| v.as_array()) {
        if let Some(first) = any_of.first() {
            return placeholder_for_schema(first);
        }
    }
    if let Some(one_of) = property_schema.get("oneOf").and_then(|v| v.as_array()) {
        if let Some(first) = one_of.first() {
            return placeholder_for_schema(first);
        }
    }

    let type_value = property_schema.get("type").and_then(|t| t.as_str());

    match type_value {
        Some("integer") => {
            // Use minimum if set, else default if set, else 0
            if let Some(default) = property_schema.get("default") {
                return default.clone();
            }
            if let Some(min) = property_schema.get("minimum").and_then(|v| v.as_i64()) {
                return Value::Number(min.into());
            }
            Value::Number(0.into())
        }
        Some("number") => {
            if let Some(default) = property_schema.get("default") {
                return default.clone();
            }
            if let Some(min) = property_schema.get("minimum").and_then(|v| v.as_f64()) {
                return serde_json::Number::from_f64(min)
                    .map(Value::Number)
                    .unwrap_or(Value::Number(0.into()));
            }
            serde_json::Number::from_f64(0.0)
                .map(Value::Number)
                .unwrap_or(Value::Number(0.into()))
        }
        Some("boolean") => {
            if let Some(default) = property_schema.get("default") {
                return default.clone();
            }
            Value::Bool(true)
        }
        Some("array") => {
            if let Some(default) = property_schema.get("default") {
                return default.clone();
            }
            Value::Array(vec![])
        }
        Some("object") => {
            if let Some(default) = property_schema.get("default") {
                return default.clone();
            }
            Value::Object(serde_json::Map::new())
        }
        Some("string") | None => {
            // For enum fields, use the first valid value so enum validation passes
            if let Some(enum_values) = property_schema.get("enum").and_then(|v| v.as_array()) {
                if let Some(first) = enum_values.first() {
                    return first.clone();
                }
            }
            if let Some(default) = property_schema.get("default") {
                return default.clone();
            }
            Value::String("__template_placeholder__".to_string())
        }
        Some(_) => Value::Null,
    }
}

/// Walk a parameters object and replace any template expression values with
/// schema-appropriate placeholders. Only replaces leaf values that match
/// `{{ ... }}`; non-template values are left untouched for normal validation.
///
/// `schema` should be the full JSON Schema object (with `properties`, `type`, etc).
fn replace_templates_with_placeholders(params: &Value, schema: &Value) -> Value {
    match params {
        Value::Object(map) => {
            let properties = schema.get("properties").and_then(|p| p.as_object());

            let mut result = serde_json::Map::new();
            for (key, value) in map {
                let prop_schema = properties.and_then(|p| p.get(key));

                if is_template_expression(value) {
                    // Replace with a type-appropriate placeholder
                    if let Some(ps) = prop_schema {
                        result.insert(key.clone(), placeholder_for_schema(ps));
                    } else {
                        // No schema for this property — keep as string placeholder
                        result.insert(
                            key.clone(),
                            Value::String("__template_placeholder__".to_string()),
                        );
                    }
                } else if value.is_object() {
                    // Recurse into nested objects
                    let empty_schema = Value::Object(serde_json::Map::new());
                    let nested_schema = prop_schema.unwrap_or(&empty_schema);
                    result.insert(
                        key.clone(),
                        replace_templates_with_placeholders(value, nested_schema),
                    );
                } else if value.is_array() {
                    // Recurse into arrays — check each element
                    if let Some(arr) = value.as_array() {
                        let empty_items_schema = Value::Object(serde_json::Map::new());
                        let item_schema = prop_schema
                            .and_then(|ps| ps.get("items"))
                            .unwrap_or(&empty_items_schema);
                        let new_arr: Vec<Value> = arr
                            .iter()
                            .map(|item| {
                                if is_template_expression(item) {
                                    placeholder_for_schema(item_schema)
                                } else if item.is_object() || item.is_array() {
                                    replace_templates_with_placeholders(item, item_schema)
                                } else {
                                    item.clone()
                                }
                            })
                            .collect();
                        result.insert(key.clone(), Value::Array(new_arr));
                    } else {
                        result.insert(key.clone(), value.clone());
                    }
                } else {
                    result.insert(key.clone(), value.clone());
                }
            }
            Value::Object(result)
        }
        other => other.clone(),
    }
}

/// Validate trigger parameters against the trigger's parameter schema.
/// Template expressions (`{{ ... }}`) are accepted for any field type.
pub fn validate_trigger_params(trigger: &Trigger, params: &Value) -> Result<(), ApiError> {
    // If no schema is defined, accept any parameters
    let Some(schema) = &trigger.param_schema else {
        return Ok(());
    };

    // Replace template expressions with schema-appropriate placeholders
    let sanitized = replace_templates_with_placeholders(params, schema);

    // Compile the JSON schema
    let compiled_schema = Validator::new(schema).map_err(|e| {
        ApiError::InternalServerError(format!(
            "Invalid parameter schema for trigger '{}': {}",
            trigger.r#ref, e
        ))
    })?;

    // Validate the sanitized parameters
    let errors: Vec<String> = compiled_schema
        .iter_errors(&sanitized)
        .map(|e| {
            let path = e.instance_path().to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{} at {}", e, path)
            }
        })
        .collect();

    if !errors.is_empty() {
        return Err(ApiError::ValidationError(format!(
            "Invalid parameters for trigger '{}': {}",
            trigger.r#ref,
            errors.join(", ")
        )));
    }

    Ok(())
}

/// Validate action parameters against the action's parameter schema.
/// Template expressions (`{{ ... }}`) are accepted for any field type.
pub fn validate_action_params(action: &Action, params: &Value) -> Result<(), ApiError> {
    // If no schema is defined, accept any parameters
    let Some(schema) = &action.param_schema else {
        return Ok(());
    };

    // Replace template expressions with schema-appropriate placeholders
    let sanitized = replace_templates_with_placeholders(params, schema);

    // Compile the JSON schema
    let compiled_schema = Validator::new(schema).map_err(|e| {
        ApiError::InternalServerError(format!(
            "Invalid parameter schema for action '{}': {}",
            action.r#ref, e
        ))
    })?;

    // Validate the sanitized parameters
    let errors: Vec<String> = compiled_schema
        .iter_errors(&sanitized)
        .map(|e| {
            let path = e.instance_path().to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{} at {}", e, path)
            }
        })
        .collect();

    if !errors.is_empty() {
        return Err(ApiError::ValidationError(format!(
            "Invalid parameters for action '{}': {}",
            action.r#ref,
            errors.join(", ")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Helper builders ──────────────────────────────────────────────

    fn make_trigger(schema: Option<Value>) -> Trigger {
        Trigger {
            id: 1,
            r#ref: "test.trigger".to_string(),
            pack: Some(1),
            pack_ref: Some("test".to_string()),
            label: "Test Trigger".to_string(),
            description: None,
            enabled: true,
            param_schema: schema,
            out_schema: None,
            webhook_enabled: false,
            webhook_key: None,
            webhook_config: None,
            is_adhoc: false,
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        }
    }

    fn make_action(schema: Option<Value>) -> Action {
        Action {
            id: 1,
            r#ref: "test.action".to_string(),
            pack: 1,
            pack_ref: "test".to_string(),
            label: "Test Action".to_string(),
            description: "Test action".to_string(),
            entrypoint: "test.sh".to_string(),
            runtime: Some(1),
            param_schema: schema,
            out_schema: None,
            is_workflow: false,
            workflow_def: None,
            is_adhoc: false,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
            output_format: attune_common::models::OutputFormat::default(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        }
    }

    // ── No schema ────────────────────────────────────────────────────

    #[test]
    fn test_validate_trigger_params_with_no_schema() {
        let trigger = make_trigger(None);
        let params = json!({ "any": "value" });
        assert!(validate_trigger_params(&trigger, &params).is_ok());
    }

    // ── Basic trigger validation (no templates) ──────────────────────

    #[test]
    fn test_validate_trigger_params_with_valid_params() {
        let schema = json!({
            "type": "object",
            "properties": {
                "unit": { "type": "string", "enum": ["seconds", "minutes", "hours"] },
                "delta": { "type": "integer", "minimum": 1 }
            },
            "required": ["unit", "delta"]
        });

        let trigger = make_trigger(Some(schema));
        let params = json!({ "unit": "seconds", "delta": 10 });
        assert!(validate_trigger_params(&trigger, &params).is_ok());
    }

    #[test]
    fn test_validate_trigger_params_with_invalid_params() {
        let schema = json!({
            "type": "object",
            "properties": {
                "unit": { "type": "string", "enum": ["seconds", "minutes", "hours"] },
                "delta": { "type": "integer", "minimum": 1 }
            },
            "required": ["unit", "delta"]
        });

        let trigger = make_trigger(Some(schema));

        // Missing required field 'delta'
        let params = json!({ "unit": "seconds" });
        assert!(validate_trigger_params(&trigger, &params).is_err());

        // Invalid enum value for 'unit'
        let params = json!({ "unit": "days", "delta": 10 });
        assert!(validate_trigger_params(&trigger, &params).is_err());

        // Invalid type for 'delta'
        let params = json!({ "unit": "seconds", "delta": "10" });
        assert!(validate_trigger_params(&trigger, &params).is_err());
    }

    // ── Basic action validation (no templates) ───────────────────────

    #[test]
    fn test_validate_action_params_with_valid_params() {
        let schema = json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        });

        let action = make_action(Some(schema));
        let params = json!({ "message": "Hello, world!" });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_validate_action_params_with_empty_params_but_required_fields() {
        let schema = json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        });

        let action = make_action(Some(schema));
        let params = json!({});
        assert!(validate_action_params(&action, &params).is_err());
    }

    // ── Template-aware validation ────────────────────────────────────

    #[test]
    fn test_template_in_integer_field_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "counter": { "type": "integer" }
            },
            "required": ["counter"]
        });

        let action = make_action(Some(schema));
        let params = json!({ "counter": "{{ event.payload.counter }}" });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_template_in_boolean_field_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "verbose": { "type": "boolean" }
            },
            "required": ["verbose"]
        });

        let action = make_action(Some(schema));
        let params = json!({ "verbose": "{{ event.payload.debug }}" });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_template_in_number_field_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "threshold": { "type": "number", "minimum": 0.0 }
            },
            "required": ["threshold"]
        });

        let action = make_action(Some(schema));
        let params = json!({ "threshold": "{{ event.payload.threshold }}" });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_template_in_enum_field_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "level": { "type": "string", "enum": ["info", "warn", "error"] }
            },
            "required": ["level"]
        });

        let action = make_action(Some(schema));
        let params = json!({ "level": "{{ event.payload.severity }}" });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_template_in_array_field_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "recipients": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["recipients"]
        });

        let action = make_action(Some(schema));
        let params = json!({ "recipients": "{{ event.payload.emails }}" });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_template_in_object_field_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "metadata": { "type": "object" }
            },
            "required": ["metadata"]
        });

        let action = make_action(Some(schema));
        let params = json!({ "metadata": "{{ event.payload.meta }}" });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_mixed_template_and_literal_values() {
        let schema = json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" },
                "count": { "type": "integer" },
                "verbose": { "type": "boolean" }
            },
            "required": ["message", "count", "verbose"]
        });

        let action = make_action(Some(schema));

        // Mix of literal and template values
        let params = json!({
            "message": "Hello",
            "count": "{{ event.payload.count }}",
            "verbose": true
        });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_literal_values_still_validated() {
        let schema = json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["message", "count"]
        });

        let action = make_action(Some(schema));

        // Template for message is fine, but literal "not_a_number" for integer is not
        let params = json!({
            "message": "{{ event.payload.msg }}",
            "count": "not_a_number"
        });
        assert!(validate_action_params(&action, &params).is_err());
    }

    #[test]
    fn test_required_field_still_enforced_with_templates() {
        let schema = json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["message", "count"]
        });

        let action = make_action(Some(schema));

        // Only message provided (even as template), count is missing
        let params = json!({ "message": "{{ event.payload.msg }}" });
        assert!(validate_action_params(&action, &params).is_err());
    }

    #[test]
    fn test_pack_config_template_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "api_key": { "type": "string" },
                "timeout": { "type": "integer" }
            },
            "required": ["api_key", "timeout"]
        });

        let action = make_action(Some(schema));
        let params = json!({
            "api_key": "{{ pack.config.api_key }}",
            "timeout": "{{ pack.config.default_timeout }}"
        });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_system_template_passes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "timestamp": { "type": "string" },
                "rule_id": { "type": "integer" }
            },
            "required": ["timestamp", "rule_id"]
        });

        let action = make_action(Some(schema));
        let params = json!({
            "timestamp": "{{ system.timestamp }}",
            "rule_id": "{{ system.rule.id }}"
        });
        assert!(validate_action_params(&action, &params).is_ok());
    }

    #[test]
    fn test_trigger_params_template_aware() {
        let schema = json!({
            "type": "object",
            "properties": {
                "unit": { "type": "string", "enum": ["seconds", "minutes", "hours"] },
                "delta": { "type": "integer", "minimum": 1 }
            },
            "required": ["unit", "delta"]
        });

        let trigger = make_trigger(Some(schema));

        // Both fields as templates
        let params = json!({
            "unit": "{{ pack.config.timer_unit }}",
            "delta": "{{ pack.config.timer_delta }}"
        });
        assert!(validate_trigger_params(&trigger, &params).is_ok());
    }

    // ── Placeholder generation ───────────────────────────────────────

    #[test]
    fn test_is_template_expression() {
        assert!(is_template_expression(&json!("{{ event.payload.x }}")));
        assert!(is_template_expression(&json!("{{ pack.config.key }}")));
        assert!(is_template_expression(&json!(
            "prefix {{ system.ts }} suffix"
        )));
        assert!(!is_template_expression(&json!("no braces here")));
        assert!(!is_template_expression(&json!(42)));
        assert!(!is_template_expression(&json!(true)));
        assert!(!is_template_expression(&json!("{ single braces }")));
    }

    #[test]
    fn test_placeholder_for_schema_types() {
        assert_eq!(
            placeholder_for_schema(&json!({"type": "integer"})),
            json!(0)
        );
        assert_eq!(
            placeholder_for_schema(&json!({"type": "number"})),
            json!(0.0)
        );
        assert_eq!(
            placeholder_for_schema(&json!({"type": "boolean"})),
            json!(true)
        );
        assert_eq!(placeholder_for_schema(&json!({"type": "array"})), json!([]));
        assert_eq!(
            placeholder_for_schema(&json!({"type": "object"})),
            json!({})
        );
        assert_eq!(
            placeholder_for_schema(&json!({"type": "string"})),
            json!("__template_placeholder__")
        );
    }

    #[test]
    fn test_placeholder_respects_enum() {
        let schema = json!({"type": "string", "enum": ["a", "b", "c"]});
        assert_eq!(placeholder_for_schema(&schema), json!("a"));
    }

    #[test]
    fn test_placeholder_respects_default() {
        let schema = json!({"type": "integer", "default": 42});
        assert_eq!(placeholder_for_schema(&schema), json!(42));
    }

    #[test]
    fn test_placeholder_respects_minimum() {
        let schema = json!({"type": "integer", "minimum": 5});
        assert_eq!(placeholder_for_schema(&schema), json!(5));
    }

    #[test]
    fn test_nested_object_template_replacement() {
        let schema = json!({
            "type": "object",
            "properties": {
                "outer": {
                    "type": "object",
                    "properties": {
                        "inner_count": { "type": "integer" }
                    }
                }
            }
        });

        let params = json!({
            "outer": {
                "inner_count": "{{ event.payload.count }}"
            }
        });

        let sanitized = replace_templates_with_placeholders(&params, &schema);
        // The inner template should be replaced with an integer placeholder
        assert!(sanitized["outer"]["inner_count"].is_number());
    }

    #[test]
    fn test_array_element_template_replacement() {
        let schema = json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        });

        let params = json!({
            "tags": ["literal", "{{ event.payload.tag }}"]
        });

        let sanitized = replace_templates_with_placeholders(&params, &schema);
        let tags = sanitized["tags"].as_array().unwrap();
        assert_eq!(tags[0], "literal");
        assert!(tags[1].is_string());
        assert_ne!(tags[1], "{{ event.payload.tag }}");
    }
}
