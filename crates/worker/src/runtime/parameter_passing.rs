//! Parameter Passing Module
//!
//! Provides utilities for formatting and delivering action parameters
//! in different formats (dotenv, JSON, YAML) via different methods
//! (environment variables, stdin, temporary files).

use attune_common::models::{ParameterDelivery, ParameterFormat};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tracing::debug;

use super::RuntimeError;

/// Format parameters according to the specified format
pub fn format_parameters(
    parameters: &HashMap<String, JsonValue>,
    format: ParameterFormat,
) -> Result<String, RuntimeError> {
    match format {
        ParameterFormat::Dotenv => format_dotenv(parameters),
        ParameterFormat::Json => format_json(parameters),
        ParameterFormat::Yaml => format_yaml(parameters),
    }
}

/// Format parameters as dotenv (key='value')
/// Note: Parameter names are preserved as-is (case-sensitive)
fn format_dotenv(parameters: &HashMap<String, JsonValue>) -> Result<String, RuntimeError> {
    let mut lines = Vec::new();

    for (key, value) in parameters {
        let value_str = value_to_string(value);

        // Escape single quotes in value
        let escaped_value = value_str.replace('\'', "'\\''");

        lines.push(format!("{}='{}'", key, escaped_value));
    }

    Ok(lines.join("\n"))
}

/// Format parameters as JSON
fn format_json(parameters: &HashMap<String, JsonValue>) -> Result<String, RuntimeError> {
    serde_json::to_string_pretty(parameters).map_err(|e| {
        RuntimeError::ExecutionFailed(format!(
            "Failed to serialize parameters to JSON: {}",
            e
        ))
    })
}

/// Format parameters as YAML
fn format_yaml(parameters: &HashMap<String, JsonValue>) -> Result<String, RuntimeError> {
    serde_yaml_ng::to_string(parameters).map_err(|e| {
        RuntimeError::ExecutionFailed(format!(
            "Failed to serialize parameters to YAML: {}",
            e
        ))
    })
}

/// Convert JSON value to string representation
fn value_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => String::new(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| String::new()),
    }
}

/// Create a temporary file with parameters
pub fn create_parameter_file(
    parameters: &HashMap<String, JsonValue>,
    format: ParameterFormat,
) -> Result<NamedTempFile, RuntimeError> {
    let formatted = format_parameters(parameters, format)?;

    let mut temp_file = NamedTempFile::new()
        .map_err(|e| RuntimeError::IoError(e))?;

    // Set restrictive permissions (owner read-only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = temp_file.as_file().metadata()
            .map_err(|e| RuntimeError::IoError(e))?
            .permissions();
        perms.set_mode(0o400); // Read-only for owner
        temp_file.as_file().set_permissions(perms)
            .map_err(|e| RuntimeError::IoError(e))?;
    }

    temp_file
        .write_all(formatted.as_bytes())
        .map_err(|e| RuntimeError::IoError(e))?;

    temp_file
        .flush()
        .map_err(|e| RuntimeError::IoError(e))?;

    debug!(
        "Created parameter file at {:?} with format {:?}",
        temp_file.path(),
        format
    );

    Ok(temp_file)
}

/// Parameter delivery configuration
#[derive(Debug, Clone)]
pub struct ParameterDeliveryConfig {
    pub delivery: ParameterDelivery,
    pub format: ParameterFormat,
}

/// Prepared parameters ready for execution
#[derive(Debug)]
pub enum PreparedParameters {
    /// Parameters are in environment variables
    Environment,
    /// Parameters will be passed via stdin
    Stdin(String),
    /// Parameters are in a temporary file
    File {
        path: PathBuf,
        #[allow(dead_code)]
        temp_file: NamedTempFile,
    },
}

impl PreparedParameters {
    /// Get the file path if this is file-based delivery
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            PreparedParameters::File { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Get the stdin content if this is stdin-based delivery
    pub fn stdin_content(&self) -> Option<&str> {
        match self {
            PreparedParameters::Stdin(content) => Some(content),
            _ => None,
        }
    }
}

/// Prepare parameters for delivery according to the specified method and format
pub fn prepare_parameters(
    parameters: &HashMap<String, JsonValue>,
    env: &mut HashMap<String, String>,
    config: ParameterDeliveryConfig,
) -> Result<PreparedParameters, RuntimeError> {
    match config.delivery {
        ParameterDelivery::Stdin => {
            // Format parameters for stdin
            let formatted = format_parameters(parameters, config.format)?;

            // Add environment variables to indicate delivery method
            env.insert(
                "ATTUNE_PARAMETER_DELIVERY".to_string(),
                "stdin".to_string(),
            );
            env.insert(
                "ATTUNE_PARAMETER_FORMAT".to_string(),
                config.format.to_string(),
            );

            Ok(PreparedParameters::Stdin(formatted))
        }
        ParameterDelivery::File => {
            // Create temporary file with parameters
            let temp_file = create_parameter_file(parameters, config.format)?;
            let path = temp_file.path().to_path_buf();

            // Add environment variables to indicate delivery method and file location
            env.insert(
                "ATTUNE_PARAMETER_DELIVERY".to_string(),
                "file".to_string(),
            );
            env.insert(
                "ATTUNE_PARAMETER_FORMAT".to_string(),
                config.format.to_string(),
            );
            env.insert(
                "ATTUNE_PARAMETER_FILE".to_string(),
                path.to_string_lossy().to_string(),
            );

            Ok(PreparedParameters::File { path, temp_file })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_dotenv() {
        let mut params = HashMap::new();
        params.insert("message".to_string(), json!("Hello, World!"));
        params.insert("count".to_string(), json!(42));
        params.insert("enabled".to_string(), json!(true));

        let result = format_dotenv(&params).unwrap();

        assert!(result.contains("message='Hello, World!'"));
        assert!(result.contains("count='42'"));
        assert!(result.contains("enabled='true'"));
    }

    #[test]
    fn test_format_dotenv_escaping() {
        let mut params = HashMap::new();
        params.insert("message".to_string(), json!("It's a test"));

        let result = format_dotenv(&params).unwrap();

        assert!(result.contains("message='It'\\''s a test'"));
    }

    #[test]
    fn test_format_json() {
        let mut params = HashMap::new();
        params.insert("message".to_string(), json!("Hello"));
        params.insert("count".to_string(), json!(42));

        let result = format_json(&params).unwrap();
        let parsed: HashMap<String, JsonValue> = serde_json::from_str(&result).unwrap();

        assert_eq!(parsed.get("message"), Some(&json!("Hello")));
        assert_eq!(parsed.get("count"), Some(&json!(42)));
    }

    #[test]
    fn test_format_yaml() {
        let mut params = HashMap::new();
        params.insert("message".to_string(), json!("Hello"));
        params.insert("count".to_string(), json!(42));

        let result = format_yaml(&params).unwrap();

        assert!(result.contains("message:"));
        assert!(result.contains("Hello"));
        assert!(result.contains("count:"));
        assert!(result.contains("42"));
    }

    #[test]
    #[test]
    fn test_create_parameter_file() {
        let mut params = HashMap::new();
        params.insert("key".to_string(), json!("value"));

        let temp_file = create_parameter_file(&params, ParameterFormat::Json).unwrap();
        let content = std::fs::read_to_string(temp_file.path()).unwrap();

        assert!(content.contains("key"));
        assert!(content.contains("value"));
    }

    #[test]
    fn test_prepare_parameters_stdin() {
        let mut params = HashMap::new();
        params.insert("test".to_string(), json!("value"));

        let mut env = HashMap::new();
        let config = ParameterDeliveryConfig {
            delivery: ParameterDelivery::Stdin,
            format: ParameterFormat::Json,
        };

        let result = prepare_parameters(&params, &mut env, config).unwrap();

        assert!(matches!(result, PreparedParameters::Stdin(_)));
        assert_eq!(
            env.get("ATTUNE_PARAMETER_DELIVERY"),
            Some(&"stdin".to_string())
        );
        assert_eq!(
            env.get("ATTUNE_PARAMETER_FORMAT"),
            Some(&"json".to_string())
        );
    }

    #[test]
    fn test_prepare_parameters_file() {
        let mut params = HashMap::new();
        params.insert("test".to_string(), json!("value"));

        let mut env = HashMap::new();
        let config = ParameterDeliveryConfig {
            delivery: ParameterDelivery::File,
            format: ParameterFormat::Yaml,
        };

        let result = prepare_parameters(&params, &mut env, config).unwrap();

        assert!(matches!(result, PreparedParameters::File { .. }));
        assert_eq!(
            env.get("ATTUNE_PARAMETER_DELIVERY"),
            Some(&"file".to_string())
        );
        assert_eq!(
            env.get("ATTUNE_PARAMETER_FORMAT"),
            Some(&"yaml".to_string())
        );
        assert!(env.contains_key("ATTUNE_PARAMETER_FILE"));
    }
}
