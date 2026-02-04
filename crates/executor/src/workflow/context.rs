//! Workflow Context Manager
//!
//! This module manages workflow execution context, including variables,
//! template rendering, and data flow between tasks.

use dashmap::DashMap;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Result type for context operations
pub type ContextResult<T> = Result<T, ContextError>;

/// Errors that can occur during context operations
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Template rendering error: {0}")]
    TemplateError(String),

    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    #[error("Invalid expression: {0}")]
    InvalidExpression(String),

    #[error("Type conversion error: {0}")]
    TypeConversion(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Workflow execution context
///
/// Uses Arc for shared immutable data to enable efficient cloning.
/// When cloning for with-items iterations, only Arc pointers are copied,
/// not the underlying data, making it O(1) instead of O(context_size).
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// Workflow-level variables (shared via Arc)
    variables: Arc<DashMap<String, JsonValue>>,

    /// Workflow input parameters (shared via Arc)
    parameters: Arc<JsonValue>,

    /// Task results (shared via Arc, keyed by task name)
    task_results: Arc<DashMap<String, JsonValue>>,

    /// System variables (shared via Arc)
    system: Arc<DashMap<String, JsonValue>>,

    /// Current item (for with-items iteration) - per-item data
    current_item: Option<JsonValue>,

    /// Current item index (for with-items iteration) - per-item data
    current_index: Option<usize>,
}

impl WorkflowContext {
    /// Create a new workflow context
    pub fn new(parameters: JsonValue, initial_vars: HashMap<String, JsonValue>) -> Self {
        let system = DashMap::new();
        system.insert("workflow_start".to_string(), json!(chrono::Utc::now()));

        let variables = DashMap::new();
        for (k, v) in initial_vars {
            variables.insert(k, v);
        }

        Self {
            variables: Arc::new(variables),
            parameters: Arc::new(parameters),
            task_results: Arc::new(DashMap::new()),
            system: Arc::new(system),
            current_item: None,
            current_index: None,
        }
    }

    /// Set a variable
    pub fn set_var(&mut self, name: &str, value: JsonValue) {
        self.variables.insert(name.to_string(), value);
    }

    /// Get a variable
    pub fn get_var(&self, name: &str) -> Option<JsonValue> {
        self.variables.get(name).map(|entry| entry.value().clone())
    }

    /// Store a task result
    pub fn set_task_result(&mut self, task_name: &str, result: JsonValue) {
        self.task_results.insert(task_name.to_string(), result);
    }

    /// Get a task result
    pub fn get_task_result(&self, task_name: &str) -> Option<JsonValue> {
        self.task_results
            .get(task_name)
            .map(|entry| entry.value().clone())
    }

    /// Set current item for iteration
    pub fn set_current_item(&mut self, item: JsonValue, index: usize) {
        self.current_item = Some(item);
        self.current_index = Some(index);
    }

    /// Clear current item
    pub fn clear_current_item(&mut self) {
        self.current_item = None;
        self.current_index = None;
    }

    /// Render a template string
    pub fn render_template(&self, template: &str) -> ContextResult<String> {
        // Simple template rendering (Jinja2-like syntax)
        // Supports: {{ variable }}, {{ task.result }}, {{ parameters.key }}

        let mut result = template.to_string();

        // Find all template expressions
        let mut start = 0;
        while let Some(open_pos) = result[start..].find("{{") {
            let open_pos = start + open_pos;
            if let Some(close_pos) = result[open_pos..].find("}}") {
                let close_pos = open_pos + close_pos;
                let expr = &result[open_pos + 2..close_pos].trim();

                // Evaluate expression
                let value = self.evaluate_expression(expr)?;

                // Replace template with value
                let value_str = value_to_string(&value);
                result.replace_range(open_pos..close_pos + 2, &value_str);

                start = open_pos + value_str.len();
            } else {
                break;
            }
        }

        Ok(result)
    }

    /// Render a JSON value (recursively render templates in strings)
    pub fn render_json(&self, value: &JsonValue) -> ContextResult<JsonValue> {
        match value {
            JsonValue::String(s) => {
                let rendered = self.render_template(s)?;
                Ok(JsonValue::String(rendered))
            }
            JsonValue::Array(arr) => {
                let mut result = Vec::new();
                for item in arr {
                    result.push(self.render_json(item)?);
                }
                Ok(JsonValue::Array(result))
            }
            JsonValue::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (key, val) in obj {
                    result.insert(key.clone(), self.render_json(val)?);
                }
                Ok(JsonValue::Object(result))
            }
            other => Ok(other.clone()),
        }
    }

    /// Evaluate a template expression
    fn evaluate_expression(&self, expr: &str) -> ContextResult<JsonValue> {
        let parts: Vec<&str> = expr.split('.').collect();

        if parts.is_empty() {
            return Err(ContextError::InvalidExpression(expr.to_string()));
        }

        match parts[0] {
            "parameters" => self.get_nested_value(&self.parameters, &parts[1..]),
            "vars" | "variables" => {
                if parts.len() < 2 {
                    return Err(ContextError::InvalidExpression(expr.to_string()));
                }
                let var_name = parts[1];
                if let Some(entry) = self.variables.get(var_name) {
                    let value = entry.value().clone();
                    drop(entry);
                    if parts.len() > 2 {
                        self.get_nested_value(&value, &parts[2..])
                    } else {
                        Ok(value)
                    }
                } else {
                    Err(ContextError::VariableNotFound(var_name.to_string()))
                }
            }
            "task" | "tasks" => {
                if parts.len() < 2 {
                    return Err(ContextError::InvalidExpression(expr.to_string()));
                }
                let task_name = parts[1];
                if let Some(entry) = self.task_results.get(task_name) {
                    let result = entry.value().clone();
                    drop(entry);
                    if parts.len() > 2 {
                        self.get_nested_value(&result, &parts[2..])
                    } else {
                        Ok(result)
                    }
                } else {
                    Err(ContextError::VariableNotFound(format!(
                        "task.{}",
                        task_name
                    )))
                }
            }
            "item" => {
                if let Some(ref item) = self.current_item {
                    if parts.len() > 1 {
                        self.get_nested_value(item, &parts[1..])
                    } else {
                        Ok(item.clone())
                    }
                } else {
                    Err(ContextError::VariableNotFound("item".to_string()))
                }
            }
            "index" => {
                if let Some(index) = self.current_index {
                    Ok(json!(index))
                } else {
                    Err(ContextError::VariableNotFound("index".to_string()))
                }
            }
            "system" => {
                if parts.len() < 2 {
                    return Err(ContextError::InvalidExpression(expr.to_string()));
                }
                let key = parts[1];
                if let Some(entry) = self.system.get(key) {
                    Ok(entry.value().clone())
                } else {
                    Err(ContextError::VariableNotFound(format!("system.{}", key)))
                }
            }
            // Direct variable reference
            var_name => {
                if let Some(entry) = self.variables.get(var_name) {
                    let value = entry.value().clone();
                    drop(entry);
                    if parts.len() > 1 {
                        self.get_nested_value(&value, &parts[1..])
                    } else {
                        Ok(value)
                    }
                } else {
                    Err(ContextError::VariableNotFound(var_name.to_string()))
                }
            }
        }
    }

    /// Get nested value from JSON
    fn get_nested_value(&self, value: &JsonValue, path: &[&str]) -> ContextResult<JsonValue> {
        let mut current = value;

        for key in path {
            match current {
                JsonValue::Object(obj) => {
                    current = obj
                        .get(*key)
                        .ok_or_else(|| ContextError::VariableNotFound(key.to_string()))?;
                }
                JsonValue::Array(arr) => {
                    let index: usize = key.parse().map_err(|_| {
                        ContextError::InvalidExpression(format!("Invalid array index: {}", key))
                    })?;
                    current = arr.get(index).ok_or_else(|| {
                        ContextError::InvalidExpression(format!(
                            "Array index out of bounds: {}",
                            index
                        ))
                    })?;
                }
                _ => {
                    return Err(ContextError::InvalidExpression(format!(
                        "Cannot access property '{}' on non-object/array value",
                        key
                    )));
                }
            }
        }

        Ok(current.clone())
    }

    /// Evaluate a conditional expression (for 'when' clauses)
    pub fn evaluate_condition(&self, condition: &str) -> ContextResult<bool> {
        // For now, simple boolean evaluation
        // TODO: Support more complex expressions (comparisons, logical operators)

        let rendered = self.render_template(condition)?;

        // Try to parse as boolean
        match rendered.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" | "" => Ok(false),
            other => {
                // Try to evaluate as truthy/falsy
                Ok(!other.is_empty())
            }
        }
    }

    /// Publish variables from a task result
    pub fn publish_from_result(
        &mut self,
        result: &JsonValue,
        publish_vars: &[String],
        publish_map: Option<&HashMap<String, String>>,
    ) -> ContextResult<()> {
        // If publish map is provided, use it
        if let Some(map) = publish_map {
            for (var_name, template) in map {
                // Create temporary context with result
                let mut temp_ctx = self.clone();
                temp_ctx.set_var("result", result.clone());

                let value_str = temp_ctx.render_template(template)?;

                // Try to parse as JSON, otherwise store as string
                let value = serde_json::from_str(&value_str)
                    .unwrap_or_else(|_| JsonValue::String(value_str));

                self.set_var(var_name, value);
            }
        } else {
            // Simple variable publishing - store entire result
            for var_name in publish_vars {
                self.set_var(var_name, result.clone());
            }
        }

        Ok(())
    }

    /// Export context for storage
    pub fn export(&self) -> JsonValue {
        let variables: HashMap<String, JsonValue> = self
            .variables
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let task_results: HashMap<String, JsonValue> = self
            .task_results
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let system: HashMap<String, JsonValue> = self
            .system
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        json!({
            "variables": variables,
            "parameters": self.parameters.as_ref(),
            "task_results": task_results,
            "system": system,
        })
    }

    /// Import context from stored data
    pub fn import(data: JsonValue) -> ContextResult<Self> {
        let variables = DashMap::new();
        if let Some(obj) = data["variables"].as_object() {
            for (k, v) in obj {
                variables.insert(k.clone(), v.clone());
            }
        }

        let parameters = data["parameters"].clone();

        let task_results = DashMap::new();
        if let Some(obj) = data["task_results"].as_object() {
            for (k, v) in obj {
                task_results.insert(k.clone(), v.clone());
            }
        }

        let system = DashMap::new();
        if let Some(obj) = data["system"].as_object() {
            for (k, v) in obj {
                system.insert(k.clone(), v.clone());
            }
        }

        Ok(Self {
            variables: Arc::new(variables),
            parameters: Arc::new(parameters),
            task_results: Arc::new(task_results),
            system: Arc::new(system),
            current_item: None,
            current_index: None,
        })
    }
}

/// Convert a JSON value to a string for template rendering
fn value_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_template_rendering() {
        let params = json!({
            "name": "World"
        });
        let ctx = WorkflowContext::new(params, HashMap::new());

        let result = ctx.render_template("Hello {{ parameters.name }}!").unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_variable_access() {
        let mut vars = HashMap::new();
        vars.insert("greeting".to_string(), json!("Hello"));

        let ctx = WorkflowContext::new(json!({}), vars);

        let result = ctx.render_template("{{ greeting }} World").unwrap();
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_task_result_access() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_task_result("task1", json!({"status": "success"}));

        let result = ctx
            .render_template("Status: {{ task.task1.status }}")
            .unwrap();
        assert_eq!(result, "Status: success");
    }

    #[test]
    fn test_nested_value_access() {
        let params = json!({
            "config": {
                "server": {
                    "port": 8080
                }
            }
        });
        let ctx = WorkflowContext::new(params, HashMap::new());

        let result = ctx
            .render_template("Port: {{ parameters.config.server.port }}")
            .unwrap();
        assert_eq!(result, "Port: 8080");
    }

    #[test]
    fn test_item_context() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_current_item(json!({"name": "item1"}), 0);

        let result = ctx
            .render_template("Item: {{ item.name }}, Index: {{ index }}")
            .unwrap();
        assert_eq!(result, "Item: item1, Index: 0");
    }

    #[test]
    fn test_condition_evaluation() {
        let params = json!({"enabled": true});
        let ctx = WorkflowContext::new(params, HashMap::new());

        assert!(ctx.evaluate_condition("true").unwrap());
        assert!(!ctx.evaluate_condition("false").unwrap());
    }

    #[test]
    fn test_render_json() {
        let params = json!({"name": "test"});
        let ctx = WorkflowContext::new(params, HashMap::new());

        let input = json!({
            "message": "Hello {{ parameters.name }}",
            "count": 42,
            "nested": {
                "value": "Name is {{ parameters.name }}"
            }
        });

        let result = ctx.render_json(&input).unwrap();
        assert_eq!(result["message"], "Hello test");
        assert_eq!(result["count"], 42);
        assert_eq!(result["nested"]["value"], "Name is test");
    }

    #[test]
    fn test_publish_variables() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        let result = json!({"output": "success"});

        ctx.publish_from_result(&result, &["my_var".to_string()], None)
            .unwrap();

        assert_eq!(ctx.get_var("my_var").unwrap(), result);
    }

    #[test]
    fn test_export_import() {
        let mut ctx = WorkflowContext::new(json!({"key": "value"}), HashMap::new());
        ctx.set_var("test", json!("data"));
        ctx.set_task_result("task1", json!({"result": "ok"}));

        let exported = ctx.export();
        let _imported = WorkflowContext::import(exported).unwrap();

        assert_eq!(ctx.get_var("test").unwrap(), json!("data"));
        assert_eq!(
            ctx.get_task_result("task1").unwrap(),
            json!({"result": "ok"})
        );
    }
}
