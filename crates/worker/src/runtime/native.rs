//! Native Runtime
//!
//! Executes compiled native binaries directly without any shell or interpreter wrapper.
//! This runtime is used for Rust binaries and other compiled executables.

use super::{
    BoundedLogWriter, ExecutionContext, ExecutionResult, Runtime, RuntimeError, RuntimeResult,
};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

/// Native runtime for executing compiled binaries
pub struct NativeRuntime {
    work_dir: Option<std::path::PathBuf>,
}

impl NativeRuntime {
    /// Create a new native runtime
    pub fn new() -> Self {
        Self { work_dir: None }
    }

    /// Create a native runtime with custom working directory
    pub fn with_work_dir(work_dir: std::path::PathBuf) -> Self {
        Self {
            work_dir: Some(work_dir),
        }
    }

    /// Execute a native binary with parameters and environment variables
    async fn execute_binary(
        &self,
        binary_path: std::path::PathBuf,
        parameters: &std::collections::HashMap<String, serde_json::Value>,
        secrets: &std::collections::HashMap<String, String>,
        env: &std::collections::HashMap<String, String>,
        exec_timeout: Option<u64>,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
    ) -> RuntimeResult<ExecutionResult> {
        let start = Instant::now();

        // Check if binary exists and is executable
        if !binary_path.exists() {
            return Err(RuntimeError::ExecutionFailed(format!(
                "Binary not found: {}",
                binary_path.display()
            )));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&binary_path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(RuntimeError::ExecutionFailed(format!(
                    "Binary is not executable: {}",
                    binary_path.display()
                )));
            }
        }

        debug!("Executing native binary: {}", binary_path.display());

        // Build command
        let mut cmd = Command::new(&binary_path);

        // Set working directory
        if let Some(ref work_dir) = self.work_dir {
            cmd.current_dir(work_dir);
        }

        // Add environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Add parameters as environment variables with ATTUNE_ACTION_ prefix
        for (key, value) in parameters {
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(value)?,
            };
            cmd.env(format!("ATTUNE_ACTION_{}", key.to_uppercase()), value_str);
        }

        // Configure stdio
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn process
        let mut child = cmd
            .spawn()
            .map_err(|e| RuntimeError::ExecutionFailed(format!("Failed to spawn binary: {}", e)))?;

        // Write secrets to stdin - if this fails, the process has already started
        // so we should continue and capture whatever output we can
        let stdin_write_error = if !secrets.is_empty() {
            if let Some(mut stdin) = child.stdin.take() {
                match serde_json::to_string(secrets) {
                    Ok(secrets_json) => {
                        if let Err(e) = stdin.write_all(secrets_json.as_bytes()).await {
                            Some(format!("Failed to write secrets to stdin: {}", e))
                        } else if let Err(e) = stdin.shutdown().await {
                            Some(format!("Failed to close stdin: {}", e))
                        } else {
                            None
                        }
                    }
                    Err(e) => Some(format!("Failed to serialize secrets: {}", e)),
                }
            } else {
                None
            }
        } else {
            if let Some(stdin) = child.stdin.take() {
                drop(stdin); // Close stdin if no secrets
            }
            None
        };

        // Capture stdout and stderr with size limits
        let stdout_handle = child
            .stdout
            .take()
            .ok_or_else(|| RuntimeError::ProcessError("Failed to capture stdout".to_string()))?;
        let stderr_handle = child
            .stderr
            .take()
            .ok_or_else(|| RuntimeError::ProcessError("Failed to capture stderr".to_string()))?;

        let mut stdout_writer = BoundedLogWriter::new_stdout(max_stdout_bytes);
        let mut stderr_writer = BoundedLogWriter::new_stderr(max_stderr_bytes);

        // Create buffered readers
        let mut stdout_reader = BufReader::new(stdout_handle);
        let mut stderr_reader = BufReader::new(stderr_handle);

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

        // Wait for both streams to complete
        let (stdout_writer, stderr_writer) = tokio::join!(stdout_task, stderr_task);

        // Wait for process with timeout
        let wait_result = if let Some(timeout_secs) = exec_timeout {
            match timeout(Duration::from_secs(timeout_secs), child.wait()).await {
                Ok(result) => result,
                Err(_) => {
                    warn!(
                        "Native binary execution timed out after {} seconds",
                        timeout_secs
                    );
                    let _ = child.kill().await;
                    return Err(RuntimeError::Timeout(timeout_secs));
                }
            }
        } else {
            child.wait().await
        };

        let status = wait_result.map_err(|e| {
            RuntimeError::ExecutionFailed(format!("Failed to wait for process: {}", e))
        })?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let exit_code = status.code().unwrap_or(-1);

        // Extract logs with truncation info
        let stdout_log = stdout_writer.into_result();
        let stderr_log = stderr_writer.into_result();

        debug!(
            "Native binary completed with exit code {} in {}ms",
            exit_code, duration_ms
        );

        if stdout_log.truncated {
            warn!(
                "stdout truncated: {} bytes over limit",
                stdout_log.bytes_truncated
            );
        }
        if stderr_log.truncated {
            warn!(
                "stderr truncated: {} bytes over limit",
                stderr_log.bytes_truncated
            );
        }

        // Parse result from stdout if successful
        let result = if exit_code == 0 {
            serde_json::from_str(&stdout_log.content).ok()
        } else {
            None
        };

        // Determine error message
        let error = if exit_code != 0 {
            Some(format!(
                "Native binary exited with code {}: {}",
                exit_code,
                stderr_log.content.trim()
            ))
        } else if let Some(stdin_err) = stdin_write_error {
            // Ignore broken pipe errors for fast-exiting successful actions
            // These occur when the process exits before we finish writing secrets to stdin
            let is_broken_pipe =
                stdin_err.contains("Broken pipe") || stdin_err.contains("os error 32");
            let is_fast_exit = duration_ms < 500;
            let is_success = exit_code == 0;

            if is_broken_pipe && is_fast_exit && is_success {
                debug!(
                    "Ignoring broken pipe error for fast-exiting successful action ({}ms)",
                    duration_ms
                );
                None
            } else {
                Some(stdin_err)
            }
        } else {
            None
        };

        Ok(ExecutionResult {
            exit_code,
            stdout: stdout_log.content,
            stderr: stderr_log.content,
            result,
            duration_ms,
            error,
            stdout_truncated: stdout_log.truncated,
            stderr_truncated: stderr_log.truncated,
            stdout_bytes_truncated: stdout_log.bytes_truncated,
            stderr_bytes_truncated: stderr_log.bytes_truncated,
        })
    }
}

impl Default for NativeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Runtime for NativeRuntime {
    fn name(&self) -> &str {
        "native"
    }

    fn can_execute(&self, context: &ExecutionContext) -> bool {
        // Check if runtime_name is explicitly set to "native"
        if let Some(ref runtime_name) = context.runtime_name {
            return runtime_name.to_lowercase() == "native";
        }

        // Otherwise, check if code_path points to an executable binary
        // This is a heuristic - native binaries typically don't have common script extensions
        if let Some(ref code_path) = context.code_path {
            let extension = code_path.extension().and_then(|e| e.to_str()).unwrap_or("");

            // Exclude common script extensions
            let is_script = matches!(
                extension,
                "py" | "js" | "sh" | "bash" | "rb" | "pl" | "php" | "lua"
            );

            // If it's not a script and the file exists, it might be a native binary
            !is_script && code_path.exists()
        } else {
            false
        }
    }

    async fn execute(&self, context: ExecutionContext) -> RuntimeResult<ExecutionResult> {
        info!(
            "Executing native action: {} (execution_id: {})",
            context.action_ref, context.execution_id
        );

        // Get the binary path
        let binary_path = context.code_path.ok_or_else(|| {
            RuntimeError::InvalidAction("Native runtime requires code_path to be set".to_string())
        })?;

        self.execute_binary(
            binary_path,
            &context.parameters,
            &context.secrets,
            &context.env,
            context.timeout,
            context.max_stdout_bytes,
            context.max_stderr_bytes,
        )
        .await
    }

    async fn setup(&self) -> RuntimeResult<()> {
        info!("Setting up Native runtime");

        // Verify we can execute native binaries (basic check)
        #[cfg(unix)]
        {
            use std::process::Command;
            let output = Command::new("uname").arg("-s").output().map_err(|e| {
                RuntimeError::SetupError(format!("Failed to verify native runtime: {}", e))
            })?;

            if !output.status.success() {
                return Err(RuntimeError::SetupError(
                    "Failed to execute native commands".to_string(),
                ));
            }

            debug!("Native runtime setup complete");
        }

        Ok(())
    }

    async fn cleanup(&self) -> RuntimeResult<()> {
        info!("Cleaning up Native runtime");
        // No cleanup needed for native runtime
        Ok(())
    }

    async fn validate(&self) -> RuntimeResult<()> {
        debug!("Validating Native runtime");

        // Basic validation - ensure we can execute commands
        #[cfg(unix)]
        {
            use std::process::Command;
            Command::new("echo").arg("test").output().map_err(|e| {
                RuntimeError::SetupError(format!("Native runtime validation failed: {}", e))
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_native_runtime_name() {
        let runtime = NativeRuntime::new();
        assert_eq!(runtime.name(), "native");
    }

    #[tokio::test]
    async fn test_native_runtime_can_execute() {
        let runtime = NativeRuntime::new();

        // Test with explicit runtime_name
        let mut context = ExecutionContext::test_context("test.action".to_string(), None);
        context.runtime_name = Some("native".to_string());
        assert!(runtime.can_execute(&context));

        // Test with uppercase runtime_name
        context.runtime_name = Some("NATIVE".to_string());
        assert!(runtime.can_execute(&context));

        // Test with wrong runtime_name
        context.runtime_name = Some("python".to_string());
        assert!(!runtime.can_execute(&context));
    }

    #[tokio::test]
    async fn test_native_runtime_setup() {
        let runtime = NativeRuntime::new();
        let result = runtime.setup().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_native_runtime_validate() {
        let runtime = NativeRuntime::new();
        let result = runtime.validate().await;
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_native_runtime_execute_simple() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let binary_path = temp_dir.path().join("test_binary.sh");

        // Create a simple shell script as our "binary"
        fs::write(
            &binary_path,
            "#!/bin/bash\necho 'Hello from native runtime'",
        )
        .unwrap();

        // Make it executable
        let metadata = fs::metadata(&binary_path).unwrap();
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary_path, permissions).unwrap();

        let runtime = NativeRuntime::new();
        let mut context = ExecutionContext::test_context("test.native".to_string(), None);
        context.code_path = Some(binary_path);
        context.runtime_name = Some("native".to_string());

        let result = runtime.execute(context).await;
        assert!(result.is_ok());

        let exec_result = result.unwrap();
        assert_eq!(exec_result.exit_code, 0);
        assert!(exec_result.stdout.contains("Hello from native runtime"));
    }

    #[tokio::test]
    async fn test_native_runtime_missing_binary() {
        let runtime = NativeRuntime::new();
        let mut context = ExecutionContext::test_context("test.native".to_string(), None);
        context.code_path = Some(std::path::PathBuf::from("/nonexistent/binary"));
        context.runtime_name = Some("native".to_string());

        let result = runtime.execute(context).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RuntimeError::ExecutionFailed(_)
        ));
    }

    #[tokio::test]
    async fn test_native_runtime_no_code_path() {
        let runtime = NativeRuntime::new();
        let mut context = ExecutionContext::test_context("test.native".to_string(), None);
        context.runtime_name = Some("native".to_string());
        // code_path is None

        let result = runtime.execute(context).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RuntimeError::InvalidAction(_)
        ));
    }
}
