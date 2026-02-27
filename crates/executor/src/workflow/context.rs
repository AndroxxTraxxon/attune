//! Workflow Context Manager
//!
//! This module manages workflow execution context, including variables,
//! template rendering, and data flow between tasks.
//!
//! ## Function-call expressions
//!
//! Templates support Orquesta-style function calls:
//! - `{{ result() }}` — the last completed task's result
//! - `{{ result().field }}` — nested access into the result
//! - `{{ succeeded() }}` — `true` if the last task succeeded
//! - `{{ failed() }}` — `true` if the last task failed
//! - `{{ timed_out() }}` — `true` if the last task timed out
//!
//! ## Type-preserving rendering
//!
//! When a JSON string value is a *pure* template expression (the entire value
//! is `{{ expr }}`), `render_json` returns the raw `JsonValue` from the
//! expression instead of stringifying it. This means `"{{ item }}"` resolving
//! to integer `5` stays as `5`, not the string `"5"`.

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

/// The status of the last completed task, used by `succeeded()` / `failed()` /
/// `timed_out()` function expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOutcome {
    Succeeded,
    Failed,
    TimedOut,
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

    /// The result of the last completed task (for `result()` expressions)
    last_task_result: Option<JsonValue>,

    /// The outcome of the last completed task (for `succeeded()` / `failed()`)
    last_task_outcome: Option<TaskOutcome>,
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
            last_task_result: None,
            last_task_outcome: None,
        }
    }

    /// Rebuild a workflow context from persisted workflow execution state.
    ///
    /// This is used when advancing a workflow after a child task completes —
    /// the scheduler reconstructs the context from the `workflow_execution`
    /// record's stored `variables` plus the results of all completed child
    /// executions.
    pub fn rebuild(
        parameters: JsonValue,
        stored_variables: &JsonValue,
        task_results: HashMap<String, JsonValue>,
    ) -> Self {
        let variables = DashMap::new();
        if let Some(obj) = stored_variables.as_object() {
            for (k, v) in obj {
                variables.insert(k.clone(), v.clone());
            }
        }

        let results = DashMap::new();
        for (k, v) in task_results {
            results.insert(k, v);
        }

        let system = DashMap::new();
        system.insert("workflow_start".to_string(), json!(chrono::Utc::now()));

        Self {
            variables: Arc::new(variables),
            parameters: Arc::new(parameters),
            task_results: Arc::new(results),
            system: Arc::new(system),
            current_item: None,
            current_index: None,
            last_task_result: None,
            last_task_outcome: None,
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

    /// Record the outcome of the last completed task so that `result()`,
    /// `succeeded()`, `failed()`, and `timed_out()` expressions resolve
    /// correctly.
    pub fn set_last_task_outcome(&mut self, result: JsonValue, outcome: TaskOutcome) {
        self.last_task_result = Some(result);
        self.last_task_outcome = Some(outcome);
    }

    /// Export workflow variables as a JSON object suitable for persisting
    /// back to the `workflow_execution.variables` column.
    pub fn export_variables(&self) -> JsonValue {
        let map: HashMap<String, JsonValue> = self
            .variables
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        json!(map)
    }

    /// Render a template string, always returning a `String`.
    ///
    /// For type-preserving rendering of JSON values use [`render_json`].
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

    /// Try to evaluate a string as a single pure template expression.
    ///
    /// Returns `Some(JsonValue)` when the **entire** string is exactly
    /// `{{ expr }}` (with optional whitespace), preserving the original
    /// JSON type of the evaluated expression.  Returns `None` if the
    /// string contains literal text around the template or multiple
    /// template expressions — in that case the caller should fall back
    /// to `render_template` which always stringifies.
    fn try_evaluate_pure_expression(&self, s: &str) -> Option<ContextResult<JsonValue>> {
        let trimmed = s.trim();
        if !trimmed.starts_with("{{") || !trimmed.ends_with("}}") {
            return None;
        }

        // Make sure there is only ONE template expression in the string.
        // Count `{{` occurrences — if more than one, it's not a pure expr.
        if trimmed.matches("{{").count() != 1 {
            return None;
        }

        let expr = trimmed[2..trimmed.len() - 2].trim();
        if expr.is_empty() {
            return None;
        }

        Some(self.evaluate_expression(expr))
    }

    /// Render a JSON value, recursively resolving `{{ }}` templates in
    /// strings.
    ///
    /// **Type-preserving**: when a string value is a *pure* template
    /// expression (the entire string is `{{ expr }}`), the raw `JsonValue`
    /// from the expression is returned.  For example, if `item` is `5`
    /// (a JSON number), then `"{{ item }}"` resolves to `5` not `"5"`.
    pub fn render_json(&self, value: &JsonValue) -> ContextResult<JsonValue> {
        match value {
            JsonValue::String(s) => {
                // Fast path: try as a pure expression to preserve type
                if let Some(result) = self.try_evaluate_pure_expression(s) {
                    return result;
                }
                // Fallback: render as string (interpolation with surrounding text)
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
        // ---------------------------------------------------------------
        // Function-call expressions: result(), succeeded(), failed(), timed_out()
        // ---------------------------------------------------------------
        // We handle these *before* splitting on `.` because the function
        // name contains parentheses which would confuse the dot-split.
        //
        // Supported patterns:
        //   result()              → last task result
        //   result().foo.bar      → nested access into result
        //   result().data.items   → nested access into result
        //   succeeded()           → boolean
        //   failed()              → boolean
        //   timed_out()           → boolean
        // ---------------------------------------------------------------

        if let Some(result_val) = self.try_evaluate_function_call(expr)? {
            return Ok(result_val);
        }

        // ---------------------------------------------------------------
        // Dot-path expressions
        // ---------------------------------------------------------------
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
            // Direct variable reference (e.g., `number_list` published by a
            // previous task's transition)
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

    /// Try to evaluate `expr` as a function-call expression.
    ///
    /// Returns `Ok(Some(value))` if the expression starts with a recognised
    /// function call, `Ok(None)` if it does not match, or `Err` on failure.
    fn try_evaluate_function_call(&self, expr: &str) -> ContextResult<Option<JsonValue>> {
        // succeeded()
        if expr == "succeeded()" {
            let val = self
                .last_task_outcome
                .map(|o| o == TaskOutcome::Succeeded)
                .unwrap_or(false);
            return Ok(Some(json!(val)));
        }

        // failed()
        if expr == "failed()" {
            let val = self
                .last_task_outcome
                .map(|o| o == TaskOutcome::Failed)
                .unwrap_or(false);
            return Ok(Some(json!(val)));
        }

        // timed_out()
        if expr == "timed_out()" {
            let val = self
                .last_task_outcome
                .map(|o| o == TaskOutcome::TimedOut)
                .unwrap_or(false);
            return Ok(Some(json!(val)));
        }

        // result()  or  result().path.to.field
        if expr == "result()" || expr.starts_with("result().") {
            let base = self.last_task_result.clone().unwrap_or(JsonValue::Null);

            if expr == "result()" {
                return Ok(Some(base));
            }

            // Strip "result()." prefix and navigate the remaining path
            let rest = &expr["result().".len()..];
            let path_parts: Vec<&str> = rest.split('.').collect();
            let val = self.get_nested_value(&base, &path_parts)?;
            return Ok(Some(val));
        }

        Ok(None)
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

    /// Publish variables from a task result.
    ///
    /// Each publish directive is a `(name, expression)` pair where the
    /// expression is a template string like `"{{ result().data.items }}"`.
    /// The expression is rendered with `render_json`-style type preservation
    /// so that non-string values (arrays, numbers, booleans) keep their type.
    pub fn publish_from_result(
        &mut self,
        result: &JsonValue,
        publish_vars: &[String],
        publish_map: Option<&HashMap<String, String>>,
    ) -> ContextResult<()> {
        // If publish map is provided, use it
        if let Some(map) = publish_map {
            for (var_name, template) in map {
                // Use type-preserving rendering: if the entire template is a
                // single expression like `{{ result().data.items }}`, preserve
                // the underlying JsonValue type (e.g. an array stays an array).
                let json_value = JsonValue::String(template.clone());
                let value = self.render_json(&json_value)?;
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
            last_task_result: None,
            last_task_outcome: None,
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
    fn test_render_json_type_preserving_number() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_current_item(json!(5), 0);

        // Pure expression — should preserve the integer type
        let input = json!({"seconds": "{{ item }}"});
        let result = ctx.render_json(&input).unwrap();
        assert_eq!(result["seconds"], json!(5));
        assert!(result["seconds"].is_number());
    }

    #[test]
    fn test_render_json_type_preserving_array() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_last_task_outcome(
            json!({"data": {"items": [0, 1, 2, 3, 4]}}),
            TaskOutcome::Succeeded,
        );

        // Pure expression into result() — should preserve the array type
        let input = json!({"list": "{{ result().data.items }}"});
        let result = ctx.render_json(&input).unwrap();
        assert_eq!(result["list"], json!([0, 1, 2, 3, 4]));
        assert!(result["list"].is_array());
    }

    #[test]
    fn test_render_json_mixed_template_stays_string() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_current_item(json!(5), 0);

        // Mixed text + template — must remain a string
        let input = json!({"msg": "Sleeping for {{ item }} seconds"});
        let result = ctx.render_json(&input).unwrap();
        assert_eq!(result["msg"], json!("Sleeping for 5 seconds"));
        assert!(result["msg"].is_string());
    }

    #[test]
    fn test_render_json_type_preserving_bool() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_last_task_outcome(json!({}), TaskOutcome::Succeeded);

        let input = json!({"ok": "{{ succeeded() }}"});
        let result = ctx.render_json(&input).unwrap();
        assert_eq!(result["ok"], json!(true));
        assert!(result["ok"].is_boolean());
    }

    #[test]
    fn test_result_function() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_last_task_outcome(
            json!({"data": {"items": [10, 20]}, "stdout": "hello"}),
            TaskOutcome::Succeeded,
        );

        // result() returns the full last task result
        let val = ctx.evaluate_expression("result()").unwrap();
        assert_eq!(val["data"]["items"], json!([10, 20]));

        // result().stdout returns nested field
        let val = ctx.evaluate_expression("result().stdout").unwrap();
        assert_eq!(val, json!("hello"));

        // result().data.items returns deeper nested field
        let val = ctx.evaluate_expression("result().data.items").unwrap();
        assert_eq!(val, json!([10, 20]));
    }

    #[test]
    fn test_succeeded_failed_functions() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_last_task_outcome(json!({}), TaskOutcome::Succeeded);

        assert_eq!(ctx.evaluate_expression("succeeded()").unwrap(), json!(true));
        assert_eq!(ctx.evaluate_expression("failed()").unwrap(), json!(false));
        assert_eq!(
            ctx.evaluate_expression("timed_out()").unwrap(),
            json!(false)
        );

        ctx.set_last_task_outcome(json!({}), TaskOutcome::Failed);
        assert_eq!(
            ctx.evaluate_expression("succeeded()").unwrap(),
            json!(false)
        );
        assert_eq!(ctx.evaluate_expression("failed()").unwrap(), json!(true));

        ctx.set_last_task_outcome(json!({}), TaskOutcome::TimedOut);
        assert_eq!(ctx.evaluate_expression("timed_out()").unwrap(), json!(true));
    }

    #[test]
    fn test_publish_with_result_function() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_last_task_outcome(
            json!({"data": {"items": [0, 1, 2]}}),
            TaskOutcome::Succeeded,
        );

        let mut publish_map = HashMap::new();
        publish_map.insert(
            "number_list".to_string(),
            "{{ result().data.items }}".to_string(),
        );

        ctx.publish_from_result(&json!({}), &[], Some(&publish_map))
            .unwrap();

        let val = ctx.get_var("number_list").unwrap();
        assert_eq!(val, json!([0, 1, 2]));
        assert!(val.is_array());
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
    fn test_rebuild_context() {
        let stored_vars = json!({"number_list": [0, 1, 2]});
        let mut task_results = HashMap::new();
        task_results.insert("task1".to_string(), json!({"data": {"items": [0, 1, 2]}}));

        let ctx = WorkflowContext::rebuild(json!({"count": 5}), &stored_vars, task_results);

        assert_eq!(ctx.get_var("number_list").unwrap(), json!([0, 1, 2]));
        assert_eq!(
            ctx.get_task_result("task1").unwrap(),
            json!({"data": {"items": [0, 1, 2]}})
        );
        let rendered = ctx.render_template("{{ parameters.count }}").unwrap();
        assert_eq!(rendered, "5");
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

    #[test]
    fn test_with_items_integer_type_preservation() {
        // Simulates the sleep_2 task from the hello_workflow:
        // input: { seconds: "{{ item }}" }
        // with_items: [0, 1, 2, 3, 4]
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
        ctx.set_current_item(json!(3), 3);

        let input = json!({
            "message": "Sleeping for {{ item }} seconds ",
            "seconds": "{{item}}"
        });

        let rendered = ctx.render_json(&input).unwrap();

        // seconds should be integer 3, not string "3"
        assert_eq!(rendered["seconds"], json!(3));
        assert!(rendered["seconds"].is_number());

        // message should be a string with the value interpolated
        assert_eq!(rendered["message"], json!("Sleeping for 3 seconds "));
        assert!(rendered["message"].is_string());
    }
}
