//! Local Runtime Module
//!
//! Provides local execution capabilities by combining Python and Shell runtimes.
//! This module serves as a facade for all local process-based execution.

use super::native::NativeRuntime;
use super::python::PythonRuntime;
use super::shell::ShellRuntime;
use super::{ExecutionContext, ExecutionResult, Runtime, RuntimeError, RuntimeResult};
use async_trait::async_trait;
use tracing::{debug, info};

/// Local runtime that delegates to Python, Shell, or Native based on action type
pub struct LocalRuntime {
    native: NativeRuntime,
    python: PythonRuntime,
    shell: ShellRuntime,
}

impl LocalRuntime {
    /// Create a new local runtime with default settings
    pub fn new() -> Self {
        Self {
            native: NativeRuntime::new(),
            python: PythonRuntime::new(),
            shell: ShellRuntime::new(),
        }
    }

    /// Create a local runtime with custom runtimes
    pub fn with_runtimes(
        native: NativeRuntime,
        python: PythonRuntime,
        shell: ShellRuntime,
    ) -> Self {
        Self {
            native,
            python,
            shell,
        }
    }

    /// Get the appropriate runtime for the given context
    fn select_runtime(&self, context: &ExecutionContext) -> RuntimeResult<&dyn Runtime> {
        if self.native.can_execute(context) {
            debug!("Selected Native runtime for action: {}", context.action_ref);
            Ok(&self.native)
        } else if self.python.can_execute(context) {
            debug!("Selected Python runtime for action: {}", context.action_ref);
            Ok(&self.python)
        } else if self.shell.can_execute(context) {
            debug!("Selected Shell runtime for action: {}", context.action_ref);
            Ok(&self.shell)
        } else {
            Err(RuntimeError::RuntimeNotFound(format!(
                "No suitable local runtime found for action: {}",
                context.action_ref
            )))
        }
    }
}

impl Default for LocalRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Runtime for LocalRuntime {
    fn name(&self) -> &str {
        "local"
    }

    fn can_execute(&self, context: &ExecutionContext) -> bool {
        self.native.can_execute(context)
            || self.python.can_execute(context)
            || self.shell.can_execute(context)
    }

    async fn execute(&self, context: ExecutionContext) -> RuntimeResult<ExecutionResult> {
        info!(
            "Executing local action: {} (execution_id: {})",
            context.action_ref, context.execution_id
        );

        let runtime = self.select_runtime(&context)?;
        runtime.execute(context).await
    }

    async fn setup(&self) -> RuntimeResult<()> {
        info!("Setting up Local runtime");

        self.native.setup().await?;
        self.python.setup().await?;
        self.shell.setup().await?;

        info!("Local runtime setup complete");
        Ok(())
    }

    async fn cleanup(&self) -> RuntimeResult<()> {
        info!("Cleaning up Local runtime");

        self.native.cleanup().await?;
        self.python.cleanup().await?;
        self.shell.cleanup().await?;

        Ok(())
    }

    async fn validate(&self) -> RuntimeResult<()> {
        debug!("Validating Local runtime");

        self.native.validate().await?;
        self.python.validate().await?;
        self.shell.validate().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{ParameterDelivery, ParameterFormat};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_local_runtime_python() {
        let runtime = LocalRuntime::new();

        let context = ExecutionContext {
            execution_id: 1,
            action_ref: "test.python_action".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "run".to_string(),
            code: Some(
                r#"
def run():
    return "hello from python"
"#
                .to_string(),
            ),
            code_path: None,
            runtime_name: Some("python".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        assert!(runtime.can_execute(&context));
        let result = runtime.execute(context).await.unwrap();
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_local_runtime_shell() {
        let runtime = LocalRuntime::new();

        let context = ExecutionContext {
            execution_id: 2,
            action_ref: "test.shell_action".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "run.sh".to_string(),
            code: Some("#!/bin/bash\necho 'hello from shell'".to_string()),
            code_path: None,
            runtime_name: Some("shell".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        assert!(runtime.can_execute(&context));
        let result = runtime.execute(context).await.unwrap();
        assert!(result.is_success());
        assert!(result.stdout.contains("hello from shell"));
    }

    #[tokio::test]
    async fn test_local_runtime_unknown() {
        let runtime = LocalRuntime::new();

        let context = ExecutionContext {
            execution_id: 3,
            action_ref: "test.unknown_action".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "run".to_string(),
            code: Some("some code".to_string()),
            code_path: None,
            runtime_name: Some("unknown".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        assert!(!runtime.can_execute(&context));
    }
}
