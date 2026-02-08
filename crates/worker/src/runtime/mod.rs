//! Runtime Module
//!
//! Provides runtime abstraction and implementations for executing actions
//! in different environments (Python, Shell, Node.js, Containers).

pub mod dependency;
pub mod local;
pub mod log_writer;
pub mod native;
pub mod parameter_passing;
pub mod python;
pub mod python_venv;
pub mod shell;

// Re-export runtime implementations
pub use local::LocalRuntime;
pub use native::NativeRuntime;
pub use python::PythonRuntime;
pub use shell::ShellRuntime;

use async_trait::async_trait;
use attune_common::models::{ParameterDelivery, ParameterFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

// Re-export dependency management types
pub use dependency::{
    DependencyError, DependencyManager, DependencyManagerRegistry, DependencyResult,
    DependencySpec, EnvironmentInfo,
};
pub use log_writer::{BoundedLogResult, BoundedLogWriter};
pub use parameter_passing::{ParameterDeliveryConfig, PreparedParameters};
pub use python_venv::PythonVenvManager;

/// Runtime execution result
pub type RuntimeResult<T> = std::result::Result<T, RuntimeError>;

/// Runtime execution errors
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),

    #[error("Runtime not found: {0}")]
    RuntimeNotFound(String),

    #[error("Invalid action: {0}")]
    InvalidAction(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Setup error: {0}")]
    SetupError(String),

    #[error("Cleanup error: {0}")]
    CleanupError(String),
}

/// Action execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Execution ID
    pub execution_id: i64,

    /// Action reference (pack.action)
    pub action_ref: String,

    /// Action parameters
    pub parameters: HashMap<String, serde_json::Value>,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Secrets (passed securely via stdin, not environment variables)
    pub secrets: HashMap<String, String>,

    /// Execution timeout in seconds
    pub timeout: Option<u64>,

    /// Working directory
    pub working_dir: Option<PathBuf>,

    /// Action entry point (script, function, etc.)
    pub entry_point: String,

    /// Action code/script content
    pub code: Option<String>,

    /// Action code file path (alternative to code)
    pub code_path: Option<PathBuf>,

    /// Runtime name (python, shell, etc.) - used to select the correct runtime
    pub runtime_name: Option<String>,

    /// Maximum stdout size in bytes (for log truncation)
    #[serde(default = "default_max_log_bytes")]
    pub max_stdout_bytes: usize,

    /// Maximum stderr size in bytes (for log truncation)
    #[serde(default = "default_max_log_bytes")]
    pub max_stderr_bytes: usize,

    /// How parameters should be delivered to the action
    #[serde(default)]
    pub parameter_delivery: ParameterDelivery,

    /// Format for parameter serialization
    #[serde(default)]
    pub parameter_format: ParameterFormat,
}

fn default_max_log_bytes() -> usize {
    10 * 1024 * 1024 // 10MB
}

impl ExecutionContext {
    /// Create a test context with default values (for tests)
    #[cfg(test)]
    pub fn test_context(action_ref: String, code: Option<String>) -> Self {
        use std::collections::HashMap;
        Self {
            execution_id: 1,
            action_ref,
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "run".to_string(),
            code,
            code_path: None,
            runtime_name: None,
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
        }
    }
}

/// Action execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Exit code (0 = success)
    pub exit_code: i32,

    /// Standard output
    pub stdout: String,

    /// Standard error
    pub stderr: String,

    /// Execution result data (parsed from stdout or returned by action)
    pub result: Option<serde_json::Value>,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Error message if execution failed
    pub error: Option<String>,

    /// Whether stdout was truncated due to size limits
    #[serde(default)]
    pub stdout_truncated: bool,

    /// Whether stderr was truncated due to size limits
    #[serde(default)]
    pub stderr_truncated: bool,

    /// Number of bytes truncated from stdout (0 if not truncated)
    #[serde(default)]
    pub stdout_bytes_truncated: usize,

    /// Number of bytes truncated from stderr (0 if not truncated)
    #[serde(default)]
    pub stderr_bytes_truncated: usize,
}

impl ExecutionResult {
    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.exit_code == 0 && self.error.is_none()
    }

    /// Create a success result
    pub fn success(stdout: String, result: Option<serde_json::Value>, duration_ms: u64) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
            result,
            duration_ms,
            error: None,
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_bytes_truncated: 0,
            stderr_bytes_truncated: 0,
        }
    }

    /// Create a failure result
    pub fn failure(exit_code: i32, stderr: String, error: String, duration_ms: u64) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr,
            result: None,
            duration_ms,
            error: Some(error),
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_bytes_truncated: 0,
            stderr_bytes_truncated: 0,
        }
    }
}

/// Runtime trait for executing actions
#[async_trait]
pub trait Runtime: Send + Sync {
    /// Get the runtime name
    fn name(&self) -> &str;

    /// Check if this runtime can execute the given action
    fn can_execute(&self, context: &ExecutionContext) -> bool;

    /// Execute an action
    async fn execute(&self, context: ExecutionContext) -> RuntimeResult<ExecutionResult>;

    /// Setup the runtime environment (called once on worker startup)
    async fn setup(&self) -> RuntimeResult<()> {
        Ok(())
    }

    /// Cleanup the runtime environment (called on worker shutdown)
    async fn cleanup(&self) -> RuntimeResult<()> {
        Ok(())
    }

    /// Validate the runtime is properly configured
    async fn validate(&self) -> RuntimeResult<()> {
        Ok(())
    }
}

/// Runtime registry for managing multiple runtime implementations
pub struct RuntimeRegistry {
    runtimes: Vec<Box<dyn Runtime>>,
}

impl RuntimeRegistry {
    /// Create a new runtime registry
    pub fn new() -> Self {
        Self {
            runtimes: Vec::new(),
        }
    }

    /// Register a runtime
    pub fn register(&mut self, runtime: Box<dyn Runtime>) {
        self.runtimes.push(runtime);
    }

    /// Get a runtime that can execute the given context
    pub fn get_runtime(&self, context: &ExecutionContext) -> RuntimeResult<&dyn Runtime> {
        // If runtime_name is specified, use it to select the runtime directly
        if let Some(ref runtime_name) = context.runtime_name {
            return self
                .runtimes
                .iter()
                .find(|r| r.name() == runtime_name)
                .map(|r| r.as_ref())
                .ok_or_else(|| {
                    RuntimeError::RuntimeNotFound(format!(
                        "Runtime '{}' not found for action: {} (available: {})",
                        runtime_name,
                        context.action_ref,
                        self.list_runtimes().join(", ")
                    ))
                });
        }

        // Otherwise, fall back to can_execute check
        self.runtimes
            .iter()
            .find(|r| r.can_execute(context))
            .map(|r| r.as_ref())
            .ok_or_else(|| {
                RuntimeError::RuntimeNotFound(format!(
                    "No runtime found for action: {} (available: {})",
                    context.action_ref,
                    self.list_runtimes().join(", ")
                ))
            })
    }

    /// Setup all registered runtimes
    pub async fn setup_all(&self) -> RuntimeResult<()> {
        for runtime in &self.runtimes {
            runtime.setup().await?;
        }
        Ok(())
    }

    /// Cleanup all registered runtimes
    pub async fn cleanup_all(&self) -> RuntimeResult<()> {
        for runtime in &self.runtimes {
            runtime.cleanup().await?;
        }
        Ok(())
    }

    /// Validate all registered runtimes
    pub async fn validate_all(&self) -> RuntimeResult<()> {
        for runtime in &self.runtimes {
            runtime.validate().await?;
        }
        Ok(())
    }

    /// List all registered runtimes
    pub fn list_runtimes(&self) -> Vec<&str> {
        self.runtimes.iter().map(|r| r.name()).collect()
    }
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
