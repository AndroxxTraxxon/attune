//! Shell Runtime Implementation
//!
//! Executes shell scripts and commands using subprocess execution.

use super::{
    parameter_passing::{self, ParameterDeliveryConfig},
    BoundedLogWriter, ExecutionContext, ExecutionResult, Runtime, RuntimeError, RuntimeResult,
};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Shell runtime for executing shell scripts and commands
pub struct ShellRuntime {
    /// Shell interpreter path (bash, sh, zsh, etc.)
    shell_path: PathBuf,

    /// Base directory for storing action code
    work_dir: PathBuf,
}

impl ShellRuntime {
    /// Create a new Shell runtime with bash
    pub fn new() -> Self {
        Self {
            shell_path: PathBuf::from("/bin/bash"),
            work_dir: PathBuf::from("/tmp/attune/actions"),
        }
    }

    /// Create a Shell runtime with custom shell
    pub fn with_shell(shell_path: PathBuf) -> Self {
        Self {
            shell_path,
            work_dir: PathBuf::from("/tmp/attune/actions"),
        }
    }

    /// Create a Shell runtime with custom settings
    pub fn with_config(shell_path: PathBuf, work_dir: PathBuf) -> Self {
        Self {
            shell_path,
            work_dir,
        }
    }

    /// Execute with streaming and bounded log collection
    async fn execute_with_streaming(
        &self,
        mut cmd: Command,
        secrets: &std::collections::HashMap<String, String>,
        parameters_stdin: Option<&str>,
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

        // Write to stdin - parameters (if using stdin delivery) and/or secrets
        // If this fails, the process has already started, so we continue and capture output
        let stdin_write_error = if let Some(mut stdin) = child.stdin.take() {
            let mut error = None;

            // Write parameters first if using stdin delivery
            if let Some(params_data) = parameters_stdin {
                if let Err(e) = stdin.write_all(params_data.as_bytes()).await {
                    error = Some(format!("Failed to write parameters to stdin: {}", e));
                } else if let Err(e) = stdin.write_all(b"\n---ATTUNE_PARAMS_END---\n").await {
                    error = Some(format!("Failed to write parameter delimiter: {}", e));
                }
            }

            // Write secrets as JSON (always, for backward compatibility)
            if error.is_none() && !secrets.is_empty() {
                match serde_json::to_string(secrets) {
                    Ok(secrets_json) => {
                        if let Err(e) = stdin.write_all(secrets_json.as_bytes()).await {
                            error = Some(format!("Failed to write secrets to stdin: {}", e));
                        } else if let Err(e) = stdin.write_all(b"\n").await {
                            error = Some(format!("Failed to write newline to stdin: {}", e));
                        }
                    }
                    Err(e) => error = Some(format!("Failed to serialize secrets: {}", e)),
                }
            }

            drop(stdin);
            error
        } else {
            None
        };

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

        // Get results from bounded writers - we have these regardless of wait() success
        let stdout_result = stdout_writer.into_result();
        let stderr_result = stderr_writer.into_result();

        // Handle process wait result
        let (exit_code, process_error) = match wait_result {
            Ok(Ok(status)) => (status.code().unwrap_or(-1), None),
            Ok(Err(e)) => {
                // Process wait failed, but we have the output - return it with an error
                warn!("Process wait failed but captured output: {}", e);
                (-1, Some(format!("Process wait failed: {}", e)))
            }
            Err(_) => {
                // Timeout occurred
                return Ok(ExecutionResult {
                    exit_code: -1,
                    stdout: stdout_result.content.clone(),
                    stderr: stderr_result.content.clone(),
                    result: None,
                    duration_ms,
                    error: Some(format!(
                        "Execution timed out after {} seconds",
                        timeout_secs.unwrap()
                    )),
                    stdout_truncated: stdout_result.truncated,
                    stderr_truncated: stderr_result.truncated,
                    stdout_bytes_truncated: stdout_result.bytes_truncated,
                    stderr_bytes_truncated: stderr_result.bytes_truncated,
                });
            }
        };

        debug!(
            "Shell execution completed: exit_code={}, duration={}ms, stdout_truncated={}, stderr_truncated={}",
            exit_code, duration_ms, stdout_result.truncated, stderr_result.truncated
        );

        // Try to parse result from stdout as JSON
        let result = if exit_code == 0 && !stdout_result.content.trim().is_empty() {
            stdout_result
                .content
                .trim()
                .lines()
                .last()
                .and_then(|line| serde_json::from_str(line).ok())
        } else {
            None
        };

        // Determine error message
        let error = if let Some(proc_err) = process_error {
            Some(proc_err)
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
        } else if exit_code != 0 {
            Some(if stderr_result.content.is_empty() {
                format!("Command exited with code {}", exit_code)
            } else {
                // Use last line of stderr as error, or full stderr if short
                if stderr_result.content.lines().count() > 5 {
                    stderr_result
                        .content
                        .lines()
                        .last()
                        .unwrap_or("")
                        .to_string()
                } else {
                    stderr_result.content.clone()
                }
            })
        } else {
            None
        };

        Ok(ExecutionResult {
            exit_code,
            stdout: stdout_result.content.clone(),
            stderr: stderr_result.content.clone(),
            result,
            duration_ms,
            error,
            stdout_truncated: stdout_result.truncated,
            stderr_truncated: stderr_result.truncated,
            stdout_bytes_truncated: stdout_result.bytes_truncated,
            stderr_bytes_truncated: stderr_result.bytes_truncated,
        })
    }

    /// Generate shell wrapper script that injects parameters as environment variables
    fn generate_wrapper_script(&self, context: &ExecutionContext) -> RuntimeResult<String> {
        let mut script = String::new();

        // Add shebang
        script.push_str("#!/bin/bash\n");
        script.push_str("set -e\n\n"); // Exit on error

        // Read secrets from stdin and store in associative array
        script.push_str("# Read secrets from stdin (passed securely, not via environment)\n");
        script.push_str("declare -A ATTUNE_SECRETS\n");
        script.push_str("read -r ATTUNE_SECRETS_JSON\n");
        script.push_str("if [ -n \"$ATTUNE_SECRETS_JSON\" ]; then\n");
        script.push_str("  # Parse JSON secrets using Python (always available)\n");
        script.push_str("  eval \"$(echo \"$ATTUNE_SECRETS_JSON\" | python3 -c \"\n");
        script.push_str("import sys, json\n");
        script.push_str("try:\n");
        script.push_str("    secrets = json.load(sys.stdin)\n");
        script.push_str("    for key, value in secrets.items():\n");
        script.push_str("        # Escape single quotes in value\n");
        script.push_str(
            "        safe_value = value.replace(\\\"'\\\", \\\"'\\\\\\\\\\\\\\\\'\\\")  \n",
        );
        script.push_str("        print(f\\\"ATTUNE_SECRETS['{key}']='{safe_value}'\\\")\n");
        script.push_str("except: pass\n");
        script.push_str("\")\"\n");
        script.push_str("fi\n\n");

        // Helper function to get secrets
        script.push_str("# Helper function to access secrets\n");
        script.push_str("get_secret() {\n");
        script.push_str("  local name=\"$1\"\n");
        script.push_str("  echo \"${ATTUNE_SECRETS[$name]}\"\n");
        script.push_str("}\n\n");

        // Export parameters as environment variables
        script.push_str("# Action parameters\n");
        for (key, value) in &context.parameters {
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(value)?,
            };
            // Export with PARAM_ prefix for consistency
            script.push_str(&format!(
                "export PARAM_{}='{}'\n",
                key.to_uppercase(),
                value_str
            ));
            // Also export without prefix for easier shell script writing
            script.push_str(&format!("export {}='{}'\n", key, value_str));
        }
        script.push_str("\n");

        // Add the action code
        script.push_str("# Action code\n");
        if let Some(code) = &context.code {
            script.push_str(code);
        }

        Ok(script)
    }

    /// Execute shell script directly
    async fn execute_shell_code(
        &self,
        code: String,
        secrets: &std::collections::HashMap<String, String>,
        env: &std::collections::HashMap<String, String>,
        parameters_stdin: Option<&str>,
        timeout_secs: Option<u64>,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
    ) -> RuntimeResult<ExecutionResult> {
        debug!(
            "Executing shell script with {} secrets (passed via stdin)",
            secrets.len()
        );

        // Build command
        let mut cmd = Command::new(&self.shell_path);
        cmd.arg("-c").arg(&code);

        // Add environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        self.execute_with_streaming(
            cmd,
            secrets,
            parameters_stdin,
            timeout_secs,
            max_stdout_bytes,
            max_stderr_bytes,
        )
        .await
    }

    /// Execute shell script from file
    async fn execute_shell_file(
        &self,
        script_path: PathBuf,
        secrets: &std::collections::HashMap<String, String>,
        env: &std::collections::HashMap<String, String>,
        parameters_stdin: Option<&str>,
        timeout_secs: Option<u64>,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
    ) -> RuntimeResult<ExecutionResult> {
        debug!(
            "Executing shell file: {:?} with {} secrets",
            script_path,
            secrets.len()
        );

        // Build command
        let mut cmd = Command::new(&self.shell_path);
        cmd.arg(&script_path);

        // Add environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }

        self.execute_with_streaming(
            cmd,
            secrets,
            parameters_stdin,
            timeout_secs,
            max_stdout_bytes,
            max_stderr_bytes,
        )
        .await
    }
}

impl Default for ShellRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Runtime for ShellRuntime {
    fn name(&self) -> &str {
        "shell"
    }

    fn can_execute(&self, context: &ExecutionContext) -> bool {
        // Check if action reference suggests shell script
        let is_shell = context.action_ref.contains(".sh")
            || context.entry_point.ends_with(".sh")
            || context
                .code_path
                .as_ref()
                .map(|p| p.extension().and_then(|e| e.to_str()) == Some("sh"))
                .unwrap_or(false)
            || context.entry_point == "bash"
            || context.entry_point == "sh"
            || context.entry_point == "shell";

        is_shell
    }

    async fn execute(&self, context: ExecutionContext) -> RuntimeResult<ExecutionResult> {
        info!(
            "Executing shell action: {} (execution_id: {}) with parameter delivery: {:?}, format: {:?}",
            context.action_ref, context.execution_id, context.parameter_delivery, context.parameter_format
        );
        info!(
            "Action parameters (count: {}): {:?}",
            context.parameters.len(),
            context.parameters
        );

        // Prepare environment and parameters according to delivery method
        let mut env = context.env.clone();
        let config = ParameterDeliveryConfig {
            delivery: context.parameter_delivery,
            format: context.parameter_format,
        };

        let prepared_params = parameter_passing::prepare_parameters(
            &context.parameters,
            &mut env,
            config,
        )?;

        // Get stdin content if parameters are delivered via stdin
        let parameters_stdin = prepared_params.stdin_content();

        if let Some(stdin_data) = parameters_stdin {
            info!(
                "Parameters to be sent via stdin (length: {} bytes):\n{}",
                stdin_data.len(),
                stdin_data
            );
        } else {
            info!("No parameters will be sent via stdin");
        }

        // If code_path is provided, execute the file directly
        if let Some(code_path) = &context.code_path {
            return self
                .execute_shell_file(
                    code_path.clone(),
                    &context.secrets,
                    &env,
                    parameters_stdin,
                    context.timeout,
                    context.max_stdout_bytes,
                    context.max_stderr_bytes,
                )
                .await;
        }

        // Otherwise, generate wrapper script and execute
        let script = self.generate_wrapper_script(&context)?;
        self.execute_shell_code(
            script,
            &context.secrets,
            &env,
            parameters_stdin,
            context.timeout,
            context.max_stdout_bytes,
            context.max_stderr_bytes,
        )
        .await
    }

    async fn setup(&self) -> RuntimeResult<()> {
        info!("Setting up Shell runtime");

        // Ensure work directory exists
        tokio::fs::create_dir_all(&self.work_dir)
            .await
            .map_err(|e| RuntimeError::SetupError(format!("Failed to create work dir: {}", e)))?;

        // Verify shell is available
        let output = Command::new(&self.shell_path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| {
                RuntimeError::SetupError(format!("Shell not found at {:?}: {}", self.shell_path, e))
            })?;

        if !output.status.success() {
            return Err(RuntimeError::SetupError(
                "Shell interpreter is not working".to_string(),
            ));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        info!("Shell runtime ready: {}", version.trim());

        Ok(())
    }

    async fn cleanup(&self) -> RuntimeResult<()> {
        info!("Cleaning up Shell runtime");
        // Could clean up temporary files here
        Ok(())
    }

    async fn validate(&self) -> RuntimeResult<()> {
        debug!("Validating Shell runtime");

        // Check if shell is available
        let output = Command::new(&self.shell_path)
            .arg("-c")
            .arg("echo 'test'")
            .output()
            .await
            .map_err(|e| RuntimeError::SetupError(format!("Shell validation failed: {}", e)))?;

        if !output.status.success() {
            return Err(RuntimeError::SetupError(
                "Shell interpreter validation failed".to_string(),
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
    async fn test_shell_runtime_simple() {
        let runtime = ShellRuntime::new();

        let context = ExecutionContext {
            execution_id: 1,
            action_ref: "test.simple".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "shell".to_string(),
            code: Some("echo 'Hello, World!'".to_string()),
            code_path: None,
            runtime_name: Some("shell".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(result.is_success());
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_shell_runtime_with_params() {
        let runtime = ShellRuntime::new();

        let context = ExecutionContext {
            execution_id: 2,
            action_ref: "test.params".to_string(),
            parameters: {
                let mut map = HashMap::new();
                map.insert("name".to_string(), serde_json::json!("Alice"));
                map
            },
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "shell".to_string(),
            code: Some("echo \"Hello, $name!\"".to_string()),
            code_path: None,
            runtime_name: Some("shell".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(result.is_success());
        assert!(result.stdout.contains("Hello, Alice!"));
    }

    #[tokio::test]
    async fn test_shell_runtime_timeout() {
        let runtime = ShellRuntime::new();

        let context = ExecutionContext {
            execution_id: 3,
            action_ref: "test.timeout".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(1),
            working_dir: None,
            entry_point: "shell".to_string(),
            code: Some("sleep 10".to_string()),
            code_path: None,
            runtime_name: Some("shell".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(!result.is_success());
        assert!(result.error.is_some());
        let error_msg = result.error.unwrap();
        assert!(error_msg.contains("timeout") || error_msg.contains("timed out"));
    }

    #[tokio::test]
    async fn test_shell_runtime_error() {
        let runtime = ShellRuntime::new();

        let context = ExecutionContext {
            execution_id: 4,
            action_ref: "test.error".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "shell".to_string(),
            code: Some("exit 1".to_string()),
            code_path: None,
            runtime_name: Some("shell".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(!result.is_success());
        assert_eq!(result.exit_code, 1);
    }

    #[tokio::test]
    async fn test_shell_runtime_with_secrets() {
        let runtime = ShellRuntime::new();

        let context = ExecutionContext {
            execution_id: 5,
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
            entry_point: "shell".to_string(),
            code: Some(
                r#"
# Access secrets via get_secret function
api_key=$(get_secret 'api_key')
db_pass=$(get_secret 'db_password')
missing=$(get_secret 'nonexistent')

echo "api_key=$api_key"
echo "db_pass=$db_pass"
echo "missing=$missing"
"#
                .to_string(),
            ),
            code_path: None,
            runtime_name: Some("shell".to_string()),
            max_stdout_bytes: 10 * 1024 * 1024,
            max_stderr_bytes: 10 * 1024 * 1024,
            parameter_delivery: attune_common::models::ParameterDelivery::default(),
            parameter_format: attune_common::models::ParameterFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert!(result.is_success());
        assert_eq!(result.exit_code, 0);

        // Verify secrets are accessible in action code
        assert!(result.stdout.contains("api_key=secret_key_12345"));
        assert!(result.stdout.contains("db_pass=super_secret_pass"));
        assert!(result.stdout.contains("missing="));
    }
}
