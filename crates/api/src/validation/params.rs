//! Parameter validation module
//!
//! Validates trigger and action parameters against their declared JSON schemas.

use attune_common::models::{action::Action, trigger::Trigger};
use jsonschema::Validator;
use serde_json::Value;

use crate::middleware::ApiError;

/// Validate trigger parameters against the trigger's parameter schema
pub fn validate_trigger_params(trigger: &Trigger, params: &Value) -> Result<(), ApiError> {
    // If no schema is defined, accept any parameters
    let Some(schema) = &trigger.param_schema else {
        return Ok(());
    };

    // If parameters are empty object and schema exists, validate against schema
    // (schema might allow empty object or have defaults)

    // Compile the JSON schema
    let compiled_schema = Validator::new(schema).map_err(|e| {
        ApiError::InternalServerError(format!(
            "Invalid parameter schema for trigger '{}': {}",
            trigger.r#ref, e
        ))
    })?;

    // Validate the parameters
    let errors: Vec<String> = compiled_schema
        .iter_errors(params)
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

/// Validate action parameters against the action's parameter schema
pub fn validate_action_params(action: &Action, params: &Value) -> Result<(), ApiError> {
    // If no schema is defined, accept any parameters
    let Some(schema) = &action.param_schema else {
        return Ok(());
    };

    // Compile the JSON schema
    let compiled_schema = Validator::new(schema).map_err(|e| {
        ApiError::InternalServerError(format!(
            "Invalid parameter schema for action '{}': {}",
            action.r#ref, e
        ))
    })?;

    // Validate the parameters
    let errors: Vec<String> = compiled_schema
        .iter_errors(params)
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

    #[test]
    fn test_validate_trigger_params_with_no_schema() {
        let trigger = Trigger {
            id: 1,
            r#ref: "test.trigger".to_string(),
            pack: Some(1),
            pack_ref: Some("test".to_string()),
            label: "Test Trigger".to_string(),
            description: None,
            enabled: true,
            param_schema: None,
            out_schema: None,
            webhook_enabled: false,
            webhook_key: None,
            webhook_config: None,
            is_adhoc: false,
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        };

        let params = json!({ "any": "value" });
        assert!(validate_trigger_params(&trigger, &params).is_ok());
    }

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

        let trigger = Trigger {
            id: 1,
            r#ref: "test.trigger".to_string(),
            pack: Some(1),
            pack_ref: Some("test".to_string()),
            label: "Test Trigger".to_string(),
            description: None,
            enabled: true,
            param_schema: Some(schema),
            out_schema: None,
            webhook_enabled: false,
            webhook_key: None,
            webhook_config: None,
            is_adhoc: false,
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        };

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

        let trigger = Trigger {
            id: 1,
            r#ref: "test.trigger".to_string(),
            pack: Some(1),
            pack_ref: Some("test".to_string()),
            label: "Test Trigger".to_string(),
            description: None,
            enabled: true,
            param_schema: Some(schema),
            out_schema: None,
            webhook_enabled: false,
            webhook_key: None,
            webhook_config: None,
            is_adhoc: false,
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        };

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

    #[test]
    fn test_validate_action_params_with_valid_params() {
        let schema = json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        });

        let action = Action {
            id: 1,
            r#ref: "test.action".to_string(),
            pack: 1,
            pack_ref: "test".to_string(),
            label: "Test Action".to_string(),
            description: "Test action".to_string(),
            entrypoint: "test.sh".to_string(),
            runtime: Some(1),
            param_schema: Some(schema),
            out_schema: None,
            is_workflow: false,
            workflow_def: None,
            is_adhoc: false,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        };

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

        let action = Action {
            id: 1,
            r#ref: "test.action".to_string(),
            pack: 1,
            pack_ref: "test".to_string(),
            label: "Test Action".to_string(),
            description: "Test action".to_string(),
            entrypoint: "test.sh".to_string(),
            runtime: Some(1),
            param_schema: Some(schema),
            out_schema: None,
            is_workflow: false,
            workflow_def: None,
            is_adhoc: false,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        };

        let params = json!({});
        assert!(validate_action_params(&action, &params).is_err());
    }
}
