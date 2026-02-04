//! Python Runtime Implementation
//!
//! Executes Python actions using subprocess execution.

use super::{
    BoundedLogWriter, DependencyManagerRegistry, DependencySpec, ExecutionContext, ExecutionResult,
    Runtime, RuntimeError, RuntimeResult,
};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Python runtime for executing Python scripts and functions
pub struct PythonRuntime {
    /// Python interpreter path (fallback when no venv exists)
    python_path: PathBuf,

    /// Base directory for storing action code
    work_dir: PathBuf,

    /// Optional dependency manager registry for isolated environments
    dependency_manager: Option<Arc<DependencyManagerRegistry>>,
}

impl PythonRuntime {
    /// Create a new Python runtime
    pub fn new() -> Self {
        Self {
            python_path: PathBuf::from("python3"),
            work_dir: PathBuf::from("/tmp/attune/actions"),
            dependency_manager: None,
        }
    }

    /// Create a Python runtime with custom settings
    pub fn with_config(python_path: PathBuf, work_dir: PathBuf) -> Self {
        Self {
            python_path,
            work_dir,
            dependency_manager: None,
        }
    }

    /// Create a Python runtime with dependency manager support
    pub fn with_dependency_manager(
        python_path: PathBuf,
        work_dir: PathBuf,
        dependency_manager: Arc<DependencyManagerRegistry>,
    ) -> Self {
        Self {
            python_path,
            work_dir,
            dependency_manager: Some(dependency_manager),
        }
    }

    /// Get the Python executable path to use for a given context
    ///
    /// If the action has a pack_ref with dependencies, use the venv Python.
    /// Otherwise, use the default Python interpreter.
    async fn get_python_executable(&self, context: &ExecutionContext) -> RuntimeResult<PathBuf> {
        // Check if we have a dependency manager and can extract pack_ref
        if let Some(ref dep_mgr) = self.dependency_manager {
            // Extract pack_ref from action_ref (format: "pack_ref.action_name")
            if let Some(pack_ref) = context.action_ref.split('.').next() {
                // Try to get the executable path for this pack
                match dep_mgr.get_executable_path(pack_ref, "python").await {
                    Ok(python_path) => {
                        debug!(
                            "Using pack-specific Python from venv: {}",
                            python_path.display()
                        );
                        return Ok(python_path);
                    }
                    Err(e) => {
                        // Venv doesn't exist or failed - this is OK if pack has no dependencies
                        debug!(
                            "No venv found for pack {} ({}), using default Python",
                            pack_ref, e
                        );
                    }
                }
            }
        }

        // Fall back to default Python interpreter
        debug!("Using default Python interpreter: {:?}", self.python_path);
        Ok(self.python_path.clone())
    }

    /// Generate Python wrapper script that loads parameters and executes the action
    fn generate_wrapper_script(&self, context: &ExecutionContext) -> RuntimeResult<String> {
        let params_json = serde_json::to_string(&context.parameters)?;

        // Use base64 encoding for code to avoid any quote/escape issues
        let code_bytes = context.code.as_deref().unwrap_or("").as_bytes();
        let code_base64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, code_bytes);

        let wrapper = format!(
            r#"#!/usr/bin/env python3
import sys
import json
import traceback
import base64
from pathlib import Path

# Global secrets storage (read from stdin, NOT from environment)
_attune_secrets = {{}}

def get_secret(name):
    """
    Get a secret value by name.

    Secrets are passed securely via stdin and are never exposed in
    environment variables or process listings.

    Args:
        name (str): The name of the secret to retrieve

    Returns:
        str: The secret value, or None if not found
    """
    return _attune_secrets.get(name)

def main():
    global _attune_secrets

    try:
        # Read secrets from stdin FIRST (before executing action code)
        # This prevents secrets from being visible in process environment
        secrets_line = sys.stdin.readline().strip()
        if secrets_line:
            _attune_secrets = json.loads(secrets_line)

        # Parse parameters
        parameters = json.loads('''{}''')

        # Decode action code from base64 (avoids quote/escape issues)
        action_code = base64.b64decode('{}').decode('utf-8')

        # Execute the code in a controlled namespace
        # Include get_secret helper function
        namespace = {{
            '__name__': '__main__',
            'parameters': parameters,
            'get_secret': get_secret
        }}
        exec(action_code, namespace)

        # Look for main function or run function
        if '{}' in namespace:
            result = namespace['{}'](**parameters)
        elif 'run' in namespace:
            result = namespace['run'](**parameters)
        elif 'main' in namespace:
            result = namespace['main'](**parameters)
        else:
            # No entry point found, return the namespace (only JSON-serializable values)
            def is_json_serializable(obj):
                """Check if an object is JSON serializable"""
                if obj is None:
                    return True
                if isinstance(obj, (bool, int, float, str)):
                    return True
                if isinstance(obj, (list, tuple)):
                    return all(is_json_serializable(item) for item in obj)
                if isinstance(obj, dict):
                    return all(is_json_serializable(k) and is_json_serializable(v)
                              for k, v in obj.items())
                return False

            result = {{k: v for k, v in namespace.items()
                      if not k.startswith('__') and is_json_serializable(v)}}

        # Output result as JSON
        if result is not None:
            print(json.dumps({{'result': result, 'status': 'success'}}))
        else:
            print(json.dumps({{'status': 'success'}}))

        sys.exit(0)

    except Exception as e:
        error_info = {{
            'status': 'error',
            'error': str(e),
            'error_type': type(e).__name__,
            'traceback': traceback.format_exc()
        }}
        print(json.dumps(error_info), file=sys.stderr)
        sys.exit(1)

if __name__ == '__main__':
    main()
"#,
            params_json, code_base64, context.entry_point, context.entry_point
        );

        Ok(wrapper)
    }

    /// Execute with streaming and bounded log collection
    async fn execute_with_streaming(
        &self,
        mut cmd: Command,
        secrets: &std::collections::HashMap<String, String>,
        timeout_secs: Option<u64>,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
    ) -> RuntimeResult<ExecutionResult> {
        let start = Instant::now();

        // Spawn process with piped I/O
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write secrets to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let secrets_json = serde_json::to_string(secrets)?;
            stdin.write_all(secrets_json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            drop(stdin);
        }

        // Create bounded writers
        let mut stdout_writer = BoundedLogWriter::new_stdout(max_stdout_bytes);
        let mut stderr_writer = BoundedLogWriter::new_stderr(max_stderr_bytes);

        // Take stdout and stderr streams
        let stdout = child.stdout.take().expect("stdout not captured");
        let stderr = child.stderr.take().expect("stderr not captured");

        // Create buffered readers
        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);

        // Stream both outputs concurrently
        let stdout_task = async {
            let mut line = Vec::new();
            loop {
                line.clear();
                match stdout_reader.read_until(b'\n', &mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if stdout_writer.write_all(&line).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            stdout_writer
        };

        let stderr_task = async {
            let mut line = Vec::new();
            loop {
                line.clear();
                match stderr_reader.read_until(b'\n', &mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if stderr_writer.write_all(&line).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            stderr_writer
        };

        // Wait for both streams and the process
        let (stdout_writer, stderr_writer, wait_result) =
            tokio::join!(stdout_task, stderr_task, async {
                if let Some(timeout_secs) = timeout_secs {
                    timeout(std::time::Duration::from_secs(timeout_secs), child.wait()).await
                } else {
                    Ok(child.wait().await)
                }
            });

        let duration_ms = start.elapsed().as_millis() as u64;

        // Handle timeout
        let status = match wait_result {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => {
                return Err(RuntimeError::ProcessError(format!(
                    "Process wait failed: {}",
                    e
                )));
            }
            Err(_) => {
                return Ok(ExecutionResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: String::new(),
                    result: None,
                    duration_ms,
                    error: Some(format!(
                        "Execution timed out after {} seconds",
                        timeout_secs.unwrap()
                    )),
                    stdout_truncated: false,
                    stderr_truncated: false,
                    stdout_bytes_truncated: 0,
                    stderr_bytes_truncated: 0,
                });
            }
        };

        // Get results from bounded writers
        let stdout_result = stdout_writer.into_result();
        let stderr_result = stderr_writer.into_result();

        let exit_code = status.code().unwrap_or(-1);

        debug!(
            "Python execution completed: exit_code={}, duration={}ms, stdout_truncated={}, stderr_truncated={}",
            exit_code, duration_ms, stdout_result.truncated, stderr_result.truncated
        );

        // Try to parse result from stdout
        let result = if exit_code == 0 {
            stdout_result
                .content
                .lines()
                .last()
                .and_then(|line| serde_json::from_str(line).ok())
        } else {
            None
        };

        Ok(ExecutionResult {
            exit_code,
            stdout: stdout_result.content.clone(),
            stderr: stderr_result.content.clone(),
            result,
            duration_ms,
            error: if exit_code != 0 {
                Some(stderr_result.content)
            } else {
                None
            },
            stdout_truncated: stdout_result.truncated,
            stderr_truncated: stderr_result.truncated,
            stdout_bytes_truncated: stdout_result.bytes_truncated,
            stderr_bytes_truncated: stderr_result.bytes_truncated,
        })
    }

    async fn execute_python_code(
        &self,
        script: String,
        secrets: &std::collections::HashMap<String, String>,
        env: &std::collections::HashMap<String, String>,
        timeout_secs: Option<u64>,
        python_path: PathBuf,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
    ) -> RuntimeResult<ExecutionResult> {
        debug!(
            "Executing Python script with {} secrets (passed via stdin)",
            secrets.len()
        );

        // Build command
        let mut cmd = Command::new(&python_path);
        cmd.arg("-c").arg(&script);

        // Add environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        self.execute_with_streaming(
            cmd,
            secrets,
            timeout_secs,
            max_stdout_bytes,
            max_stderr_bytes,
        )
        .await
    }

    /// Execute Python script from file
    async fn execute_python_file(
        &self,
        code_path: PathBuf,
        secrets: &std::collections::HashMap<String, String>,
        env: &std::collections::HashMap<String, String>,
        timeout_secs: Option<u64>,
        python_path: PathBuf,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
    ) -> RuntimeResult<ExecutionResult> {
        debug!(
            "Executing Python file: {:?} with {} secrets",
            code_path,
            secrets.len()
        );

        // Build command
        let mut cmd = Command::new(&python_path);
        cmd.arg(&code_path);

        // Add environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        self.execute_with_streaming(
            cmd,
            secrets,
            timeout_secs,
            max_stdout_bytes,
            max_stderr_bytes,
        )
        .await
    }
}

impl Default for PythonRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonRuntime {
    /// Ensure pack dependencies are installed (called before execution if needed)
    ///
    /// This is a helper method that can be called by the worker service to ensure
    /// a pack's Python dependencies are set up before executing actions.
    pub async fn ensure_pack_dependencies(
        &self,
        pack_ref: &str,
        spec: &DependencySpec,
    ) -> RuntimeResult<()> {
        if let Some(ref dep_mgr) = self.dependency_manager {
            if spec.has_dependencies() {
                info!(
                    "Ensuring Python dependencies for pack: {} ({} dependencies)",
                    pack_ref,
                    spec.dependencies.len()
                );

                dep_mgr
                    .ensure_environment(pack_ref, spec)
                    .await
                    .map_err(|e| {
                        RuntimeError::SetupError(format!(
                            "Failed to setup Python environment for {}: {}",
                            pack_ref, e
                        ))
                    })?;

                info!("Python dependencies ready for pack: {}", pack_ref);
            } else {
                debug!("Pack {} has no Python dependencies", pack_ref);
            }
        } else {
            warn!("Dependency manager not configured, skipping dependency isolation");
        }

        Ok(())
    }
}

#[async_trait]
impl Runtime for PythonRuntime {
    fn name(&self) -> &str {
        "python"
    }

    fn can_execute(&self, context: &ExecutionContext) -> bool {
        // Check if action reference suggests Python
        let is_python = context.action_ref.contains(".py")
            || context.entry_point.ends_with(".py")
            || context
                .code_path
                .as_ref()
                .map(|p| p.extension().and_then(|e| e.to_str()) == Some("py"))
                .unwrap_or(false);

        is_python
    }

    async fn execute(&self, context: ExecutionContext) -> RuntimeResult<ExecutionResult> {
        info!(
            "Executing Python action: {} (execution_id: {})",
            context.action_ref, context.execution_id
        );

        // Get the appropriate Python executable (venv or default)
        let python_path = self.get_python_executable(&context).await?;

        // If code_path is provided, execute the file directly
        if let Some(code_path) = &context.code_path {
            return self
                .execute_python_file(
                    code_path.clone(),
                    &context.secrets,
                    &context.env,
                    context.timeout,
                    python_path,
                    context.max_stdout_bytes,
                    context.max_stderr_bytes,
                )
                .await;
        }

        // Otherwise, generate wrapper script and execute
        let script = self.generate_wrapper_script(&context)?;
        self.execute_python_code(
            script,
            &context.secrets,
            &context.env,
            context.timeout,
            python_path,
            context.max_stdout_bytes,
            context.max_stderr_bytes,
        )
        .await
    }

    async fn setup(&self) -> RuntimeResult<()> {
        info!("Setting up Python runtime");

        // Ensure work directory exists
        tokio::fs::create_dir_all(&self.work_dir)
            .await
            .map_err(|e| RuntimeError::SetupError(format!("Failed to create work dir: {}", e)))?;

        // Verify Python is available
        let output = Command::new(&self.python_path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| {
                RuntimeError::SetupError(format!(
                    "Python not found at {:?}: {}",
                    self.python_path, e
                ))
            })?;

        if !output.status.success() {
            return Err(RuntimeError::SetupError(
                "Python interpreter is not working".to_string(),
            ));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        info!("Python runtime ready: {}", version.trim());

        Ok(())
    }

    async fn cleanup(&self) -> RuntimeResult<()> {
        info!("Cleaning up Python runtime");
        // Could clean up temporary files here
        Ok(())
    }

    async fn validate(&self) -> RuntimeResult<()> {
        debug!("Validating Python runtime");

        // Check if Python is available
        let output = Command::new(&self.python_path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| RuntimeError::SetupError(format!("Python validation failed: {}", e)))?;

        if !output.status.success() {
            return Err(RuntimeError::SetupError(
                "Python interpreter validation failed".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_python_runtime_simple() {
        let runtime = PythonRuntime::new();

        let context = ExecutionContext {
            execution_id: 1,
            action_ref: "test.simple".to_string(),
            parameters: {
                let mut map = HashMap::new();
                map.insert("x".to_string(), serde_json::json!(5));
                map.insert("y".to_string(), serde_json::json!(10));
                map
            },
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "run".to_string(),
            code: Some(
                r#"
def run(x, y):
    return x + y
"#
                .to_string(),
            ),
            code_path: None,
            runtime_name: Some("python".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(result.is_success());
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_python_runtime_timeout() {
        let runtime = PythonRuntime::new();

        let context = ExecutionContext {
            execution_id: 2,
            action_ref: "test.timeout".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(1),
            working_dir: None,
            entry_point: "run".to_string(),
            code: Some(
                r#"
import time
def run():
    time.sleep(10)
    return "done"
"#
                .to_string(),
            ),
            code_path: None,
            runtime_name: Some("python".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(!result.is_success());
        assert!(result.error.is_some());
        let error_msg = result.error.unwrap();
        assert!(error_msg.contains("timeout") || error_msg.contains("timed out"));
    }

    #[tokio::test]
    async fn test_python_runtime_error() {
        let runtime = PythonRuntime::new();

        let context = ExecutionContext {
            execution_id: 3,
            action_ref: "test.error".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "run".to_string(),
            code: Some(
                r#"
def run():
    raise ValueError("Test error")
"#
                .to_string(),
            ),
            code_path: None,
            runtime_name: Some("python".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(!result.is_success());
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_python_runtime_with_secrets() {
        let runtime = PythonRuntime::new();

        let context = ExecutionContext {
            execution_id: 4,
            action_ref: "test.secrets".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: {
                let mut s = HashMap::new();
                s.insert("api_key".to_string(), "secret_key_12345".to_string());
                s.insert("db_password".to_string(), "super_secret_pass".to_string());
                s
            },
            timeout: Some(10),
            working_dir: None,
            entry_point: "run".to_string(),
            code: Some(
                r#"
def run():
    # Access secrets via get_secret() helper
    api_key = get_secret('api_key')
    db_pass = get_secret('db_password')
    missing = get_secret('nonexistent')

    return {
        'api_key': api_key,
        'db_pass': db_pass,
        'missing': missing
    }
"#
                .to_string(),
            ),
            code_path: None,
            runtime_name: Some("python".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(result.is_success());
        assert_eq!(result.exit_code, 0);

        // Verify secrets are accessible in action code
        let result_data = result.result.unwrap();
        let result_obj = result_data.get("result").unwrap();
        assert_eq!(result_obj.get("api_key").unwrap(), "secret_key_12345");
        assert_eq!(result_obj.get("db_pass").unwrap(), "super_secret_pass");
        assert_eq!(result_obj.get("missing"), Some(&serde_json::Value::Null));
    }
}
