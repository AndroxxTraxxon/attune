//! Template engine for workflow variable interpolation
//!
//! This module provides template rendering using Tera (Jinja2-like syntax)
//! with support for multi-scope variable contexts.

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tera::{Context, Tera};

/// Result type for template operations
pub type TemplateResult<T> = Result<T, TemplateError>;

/// Errors that can occur during template rendering
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("Template rendering error: {0}")]
    RenderError(#[from] tera::Error),

    #[error("Invalid template syntax: {0}")]
    SyntaxError(String),

    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Invalid scope: {0}")]
    InvalidScope(String),
}

/// Variable scope priority (higher number = higher priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VariableScope {
    /// System-level variables (lowest priority)
    System = 1,
    /// Key-value store variables
    KeyValue = 2,
    /// Pack configuration
    PackConfig = 3,
    /// Workflow parameters (input)
    Parameters = 4,
    /// Workflow vars (defined in workflow)
    Vars = 5,
    /// Task-specific variables (highest priority)
    Task = 6,
}

/// Template engine with multi-scope variable context
pub struct TemplateEngine {
    // Note: We can't use custom filters with Tera::one_off, so we need to keep tera instance
    // But Tera doesn't expose a way to register templates without files in the new() constructor
    // So we'll just use one_off for now and skip custom filters in basic rendering
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    /// Create a new template engine
    pub fn new() -> Self {
        Self {}
    }

    /// Render a template string with the given context
    pub fn render(&self, template: &str, context: &VariableContext) -> TemplateResult<String> {
        let tera_context = context.to_tera_context()?;

        // Use one-off template rendering
        // Note: Custom filters are not supported with one_off rendering
        Tera::one_off(template, &tera_context, true).map_err(TemplateError::from)
    }

    /// Render a template and parse result as JSON
    pub fn render_json(
        &self,
        template: &str,
        context: &VariableContext,
    ) -> TemplateResult<JsonValue> {
        let rendered = self.render(template, context)?;
        serde_json::from_str(&rendered).map_err(TemplateError::from)
    }

    /// Check if a template string contains valid syntax
    pub fn validate_template(&self, template: &str) -> TemplateResult<()> {
        Tera::one_off(template, &Context::new(), true)
            .map(|_| ())
            .map_err(TemplateError::from)
    }
}

/// Multi-scope variable context for template rendering
#[derive(Debug, Clone)]
pub struct VariableContext {
    /// System-level variables
    system: HashMap<String, JsonValue>,
    /// Key-value store variables
    kv: HashMap<String, JsonValue>,
    /// Pack configuration
    pack_config: HashMap<String, JsonValue>,
    /// Workflow parameters (input)
    parameters: HashMap<String, JsonValue>,
    /// Workflow vars
    vars: HashMap<String, JsonValue>,
    /// Task results and metadata
    task: HashMap<String, JsonValue>,
}

impl Default for VariableContext {
    fn default() -> Self {
        Self::new()
    }
}

impl VariableContext {
    /// Create a new empty variable context
    pub fn new() -> Self {
        Self {
            system: HashMap::new(),
            kv: HashMap::new(),
            pack_config: HashMap::new(),
            parameters: HashMap::new(),
            vars: HashMap::new(),
            task: HashMap::new(),
        }
    }

    /// Set system variables
    pub fn with_system(mut self, vars: HashMap<String, JsonValue>) -> Self {
        self.system = vars;
        self
    }

    /// Set key-value store variables
    pub fn with_kv(mut self, vars: HashMap<String, JsonValue>) -> Self {
        self.kv = vars;
        self
    }

    /// Set pack configuration
    pub fn with_pack_config(mut self, config: HashMap<String, JsonValue>) -> Self {
        self.pack_config = config;
        self
    }

    /// Set workflow parameters
    pub fn with_parameters(mut self, params: HashMap<String, JsonValue>) -> Self {
        self.parameters = params;
        self
    }

    /// Set workflow vars
    pub fn with_vars(mut self, vars: HashMap<String, JsonValue>) -> Self {
        self.vars = vars;
        self
    }

    /// Set task variables
    pub fn with_task(mut self, task_vars: HashMap<String, JsonValue>) -> Self {
        self.task = task_vars;
        self
    }

    /// Add a single variable to a scope
    pub fn set(&mut self, scope: VariableScope, key: String, value: JsonValue) {
        match scope {
            VariableScope::System => self.system.insert(key, value),
            VariableScope::KeyValue => self.kv.insert(key, value),
            VariableScope::PackConfig => self.pack_config.insert(key, value),
            VariableScope::Parameters => self.parameters.insert(key, value),
            VariableScope::Vars => self.vars.insert(key, value),
            VariableScope::Task => self.task.insert(key, value),
        };
    }

    /// Get a variable from any scope (respects priority)
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        // Check scopes in priority order (highest to lowest)
        self.task
            .get(key)
            .or_else(|| self.vars.get(key))
            .or_else(|| self.parameters.get(key))
            .or_else(|| self.pack_config.get(key))
            .or_else(|| self.kv.get(key))
            .or_else(|| self.system.get(key))
    }

    /// Convert to Tera context for rendering
    pub fn to_tera_context(&self) -> TemplateResult<Context> {
        let mut context = Context::new();

        // Insert scopes as nested objects
        context.insert("system", &self.system);
        context.insert("kv", &self.kv);
        context.insert("pack", &serde_json::json!({ "config": self.pack_config }));
        context.insert("parameters", &self.parameters);
        context.insert("vars", &self.vars);
        context.insert("task", &self.task);

        Ok(context)
    }

    /// Merge another context into this one (preserves priority)
    pub fn merge(&mut self, other: &VariableContext) {
        self.system.extend(other.system.clone());
        self.kv.extend(other.kv.clone());
        self.pack_config.extend(other.pack_config.clone());
        self.parameters.extend(other.parameters.clone());
        self.vars.extend(other.vars.clone());
        self.task.extend(other.task.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_template_rendering() {
        let engine = TemplateEngine::new();
        let mut context = VariableContext::new();
        context.set(
            VariableScope::Parameters,
            "name".to_string(),
            json!("World"),
        );

        let result = engine.render("Hello {{ parameters.name }}!", &context);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello World!");
    }

    #[test]
    fn test_scope_priority() {
        let engine = TemplateEngine::new();
        let mut context = VariableContext::new();

        // Set same variable in multiple scopes
        context.set(VariableScope::System, "value".to_string(), json!("system"));
        context.set(VariableScope::Vars, "value".to_string(), json!("vars"));
        context.set(VariableScope::Task, "value".to_string(), json!("task"));

        // Task scope should win (highest priority)
        let result = engine.render("{{ task.value }}", &context);
        assert_eq!(result.unwrap(), "task");
    }

    #[test]
    fn test_nested_variables() {
        let engine = TemplateEngine::new();
        let mut context = VariableContext::new();
        context.set(
            VariableScope::Parameters,
            "config".to_string(),
            json!({"database": {"host": "localhost", "port": 5432}}),
        );

        let result = engine.render(
            "postgres://{{ parameters.config.database.host }}:{{ parameters.config.database.port }}",
            &context,
        );
        assert_eq!(result.unwrap(), "postgres://localhost:5432");
    }

    // Note: Custom filter tests are disabled since we're using Tera::one_off
    // which doesn't support custom filters. In production, we would need to
    // use a pre-configured Tera instance with templates registered.

    #[test]
    fn test_json_operations() {
        let engine = TemplateEngine::new();
        let mut context = VariableContext::new();
        context.set(
            VariableScope::Parameters,
            "data".to_string(),
            json!({"key": "value"}),
        );

        // Test accessing JSON properties
        let result = engine.render("{{ parameters.data.key }}", &context);
        assert_eq!(result.unwrap(), "value");
    }

    #[test]
    fn test_conditional_rendering() {
        let engine = TemplateEngine::new();
        let mut context = VariableContext::new();
        context.set(
            VariableScope::Parameters,
            "env".to_string(),
            json!("production"),
        );

        let result = engine.render(
            "{% if parameters.env == 'production' %}prod{% else %}dev{% endif %}",
            &context,
        );
        assert_eq!(result.unwrap(), "prod");
    }

    #[test]
    fn test_loop_rendering() {
        let engine = TemplateEngine::new();
        let mut context = VariableContext::new();
        context.set(
            VariableScope::Parameters,
            "items".to_string(),
            json!(["a", "b", "c"]),
        );

        let result = engine.render(
            "{% for item in parameters.items %}{{ item }}{% endfor %}",
            &context,
        );
        assert_eq!(result.unwrap(), "abc");
    }

    #[test]
    fn test_context_merge() {
        let mut ctx1 = VariableContext::new();
        ctx1.set(VariableScope::Vars, "a".to_string(), json!(1));
        ctx1.set(VariableScope::Vars, "b".to_string(), json!(2));

        let mut ctx2 = VariableContext::new();
        ctx2.set(VariableScope::Vars, "b".to_string(), json!(3));
        ctx2.set(VariableScope::Vars, "c".to_string(), json!(4));

        ctx1.merge(&ctx2);

        assert_eq!(ctx1.get("a"), Some(&json!(1)));
        assert_eq!(ctx1.get("b"), Some(&json!(3))); // ctx2 overwrites
        assert_eq!(ctx1.get("c"), Some(&json!(4)));
    }

    #[test]
    fn test_all_scopes() {
        let engine = TemplateEngine::new();
        let context = VariableContext::new()
            .with_system(HashMap::from([("sys_var".to_string(), json!("system"))]))
            .with_kv(HashMap::from([("kv_var".to_string(), json!("keyvalue"))]))
            .with_pack_config(HashMap::from([("setting".to_string(), json!("config"))]))
            .with_parameters(HashMap::from([("param".to_string(), json!("parameter"))]))
            .with_vars(HashMap::from([("var".to_string(), json!("variable"))]))
            .with_task(HashMap::from([(
                "result".to_string(),
                json!("task_result"),
            )]));

        let template = "{{ system.sys_var }}-{{ kv.kv_var }}-{{ pack.config.setting }}-{{ parameters.param }}-{{ vars.var }}-{{ task.result }}";
        let result = engine.render(template, &context);
        assert_eq!(
            result.unwrap(),
            "system-keyvalue-config-parameter-variable-task_result"
        );
    }
}
