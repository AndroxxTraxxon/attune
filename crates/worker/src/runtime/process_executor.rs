//! Shared Process Executor
//!
//! Provides common subprocess execution infrastructure used by all runtime
//! implementations. Handles streaming stdout/stderr capture, bounded log
//! collection, timeout management, stdin parameter delivery, and
//! output format parsing.
//!
//! ## Cancellation Support
//!
//! When a `CancellationToken` is provided, the executor monitors it alongside
//! the running process. On cancellation:
//! 1. SIGTERM is sent to the process immediately
//! 2. After a 5-second grace period, SIGKILL is sent as a last resort

use super::{BoundedLogWriter, ExecutionResult, OutputFormat, RuntimeResult};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Execute a subprocess command with streaming output capture.
///
/// This is the core execution function used by all runtime implementations.
/// It handles:
/// - Spawning the process with piped I/O
/// - Writing parameters (with secrets merged in) to stdin
/// - Streaming stdout/stderr with bounded log collection
/// - Timeout management
/// - Output format parsing (JSON, YAML, JSONL, text)
///
/// # Arguments
/// * `cmd` - Pre-configured `Command` (interpreter, args, env vars, working dir already set)
/// * `secrets` - Deprecated/unused — secrets are now merged into parameters by the caller
/// * `parameters_stdin` - Optional parameter data (including secrets) to write to stdin
/// * `timeout_secs` - Optional execution timeout in seconds
/// * `max_stdout_bytes` - Maximum stdout size before truncation
/// * `max_stderr_bytes` - Maximum stderr size before truncation
/// * `output_format` - How to parse stdout (Text, Json, Yaml, Jsonl)
pub async fn execute_streaming(
    cmd: Command,
    _secrets: &HashMap<String, serde_json::Value>,
    parameters_stdin: Option<&str>,
    timeout_secs: Option<u64>,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
    output_format: OutputFormat,
) -> RuntimeResult<ExecutionResult> {
    execute_streaming_cancellable(
        cmd,
        _secrets,
        parameters_stdin,
        timeout_secs,
        max_stdout_bytes,
        max_stderr_bytes,
        output_format,
        None,
    )
    .await
}

/// Execute a subprocess command with streaming output capture and optional cancellation.
///
/// This is the core execution function used by all runtime implementations.
/// It handles:
/// - Spawning the process with piped I/O
/// - Writing parameters (with secrets merged in) to stdin
/// - Streaming stdout/stderr with bounded log collection
/// - Timeout management
/// - Prompt cancellation via SIGTERM → SIGKILL escalation
/// - Output format parsing (JSON, YAML, JSONL, text)
///
/// # Arguments
/// * `cmd` - Pre-configured `Command` (interpreter, args, env vars, working dir already set)
/// * `secrets` - Deprecated/unused — secrets are now merged into parameters by the caller
/// * `parameters_stdin` - Optional parameter data (including secrets) to write to stdin
/// * `timeout_secs` - Optional execution timeout in seconds
/// * `max_stdout_bytes` - Maximum stdout size before truncation
/// * `max_stderr_bytes` - Maximum stderr size before truncation
/// * `output_format` - How to parse stdout (Text, Json, Yaml, Jsonl)
/// * `cancel_token` - Optional cancellation token for graceful process termination
#[allow(clippy::too_many_arguments)]
pub async fn execute_streaming_cancellable(
    mut cmd: Command,
    _secrets: &HashMap<String, serde_json::Value>,
    parameters_stdin: Option<&str>,
    timeout_secs: Option<u64>,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
    output_format: OutputFormat,
    cancel_token: Option<CancellationToken>,
) -> RuntimeResult<ExecutionResult> {
    let start = Instant::now();

    // Spawn process with piped I/O
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Write to stdin - parameters (with secrets already merged in by the caller).
    // If this fails, the process has already started, so we continue and capture output.
    let stdin_write_error = if let Some(mut stdin) = child.stdin.take() {
        let mut error = None;

        // Write parameters to stdin as a single JSON line.
        // Secrets are merged into the parameters map by the caller, so the
        // action reads everything with a single readline().
        if let Some(params_data) = parameters_stdin {
            if let Err(e) = stdin.write_all(params_data.as_bytes()).await {
                error = Some(format!("Failed to write parameters to stdin: {}", e));
            } else if let Err(e) = stdin.write_all(b"\n").await {
                error = Some(format!("Failed to write newline to stdin: {}", e));
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

    // Determine the process ID for signal-based cancellation.
    // Must be read before we move `child` into the wait future.
    let child_pid = child.id();

    // Build the wait future that handles timeout, cancellation, and normal completion.
    //
    // The result is a tuple: (wait_result, was_cancelled)
    //   - wait_result mirrors the original type: Result<Result<ExitStatus, io::Error>, Elapsed>
    //   - was_cancelled indicates the process was stopped by a cancel request
    let wait_future = async {
        // Inner future: wait for the child process to exit
        let wait_child = child.wait();

        // Apply optional timeout wrapping
        let timed_wait = async {
            if let Some(timeout_secs) = timeout_secs {
                timeout(std::time::Duration::from_secs(timeout_secs), wait_child).await
            } else {
                Ok(wait_child.await)
            }
        };

        // If we have a cancel token, race it against the (possibly-timed) wait
        if let Some(ref token) = cancel_token {
            tokio::select! {
                result = timed_wait => (result, false),
                _ = token.cancelled() => {
                    // Cancellation requested — terminate the child process promptly.
                    info!("Cancel signal received, sending SIGTERM to process");
                    if let Some(pid) = child_pid {
                        send_signal(pid, libc::SIGTERM);
                    }

                    // Grace period: wait up to 5s for the process to exit after SIGTERM.
                    match timeout(std::time::Duration::from_secs(5), child.wait()).await {
                        Ok(status) => (Ok(status), true),
                        Err(_) => {
                            // Last resort — SIGKILL
                            warn!("Process did not exit after SIGTERM + 5s, sending SIGKILL");
                            if let Some(pid) = child_pid {
                                send_signal(pid, libc::SIGKILL);
                            }
                            // Wait indefinitely for the SIGKILL to take effect
                            (Ok(child.wait().await), true)
                        }
                    }
                }
            }
        } else {
            (timed_wait.await, false)
        }
    };

    // Wait for both streams and the process
    let (stdout_writer, stderr_writer, (wait_result, was_cancelled)) =
        tokio::join!(stdout_task, stderr_task, wait_future);

    let duration_ms = start.elapsed().as_millis() as u64;

    // Get results from bounded writers
    let stdout_result = stdout_writer.into_result();
    let stderr_result = stderr_writer.into_result();

    // Handle process wait result
    let (exit_code, process_error) = match wait_result {
        Ok(Ok(status)) => (status.code().unwrap_or(-1), None),
        Ok(Err(e)) => {
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

    // If the process was cancelled, return a specific result
    if was_cancelled {
        return Ok(ExecutionResult {
            exit_code,
            stdout: stdout_result.content.clone(),
            stderr: stderr_result.content.clone(),
            result: None,
            duration_ms,
            error: Some("Execution cancelled by user".to_string()),
            stdout_truncated: stdout_result.truncated,
            stderr_truncated: stderr_result.truncated,
            stdout_bytes_truncated: stdout_result.bytes_truncated,
            stderr_bytes_truncated: stderr_result.bytes_truncated,
        });
    }

    debug!(
        "Process execution completed: exit_code={}, duration={}ms, stdout_truncated={}, stderr_truncated={}",
        exit_code, duration_ms, stdout_result.truncated, stderr_result.truncated
    );

    // Parse result from stdout based on output_format
    let result = if exit_code == 0 && !stdout_result.content.trim().is_empty() {
        parse_output(&stdout_result.content, output_format)
    } else {
        None
    };

    // Determine error message
    let error = if let Some(proc_err) = process_error {
        Some(proc_err)
    } else if let Some(stdin_err) = stdin_write_error {
        // Ignore broken pipe errors for fast-exiting successful actions.
        // These occur when the process exits before we finish writing secrets to stdin.
        let is_broken_pipe = stdin_err.contains("Broken pipe") || stdin_err.contains("os error 32");
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
        // Only populate stdout if result wasn't parsed (avoid duplication)
        stdout: if result.is_some() {
            String::new()
        } else {
            stdout_result.content.clone()
        },
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

/// Parse stdout content according to the specified output format.
/// Send a Unix signal to a process by PID.
///
/// Uses raw `libc::kill()` to deliver signals for graceful process termination.
/// This is safe because we only send signals to child processes we spawned.
fn send_signal(pid: u32, signal: i32) {
    // Safety: we're sending a signal to a known child process PID.
    // The PID is valid because we obtained it from `child.id()` before the
    // child exited.
    unsafe {
        libc::kill(pid as i32, signal);
    }
}

fn parse_output(stdout: &str, format: OutputFormat) -> Option<serde_json::Value> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }

    match format {
        OutputFormat::Text => {
            // No parsing - text output is captured in stdout field
            None
        }
        OutputFormat::Json => {
            // Try to parse full stdout as JSON first (handles multi-line JSON),
            // then fall back to last line only (for scripts that log before output)
            serde_json::from_str(trimmed).ok().or_else(|| {
                trimmed
                    .lines()
                    .last()
                    .and_then(|line| serde_json::from_str(line).ok())
            })
        }
        OutputFormat::Yaml => {
            // Try to parse stdout as YAML
            serde_yaml_ng::from_str(trimmed).ok()
        }
        OutputFormat::Jsonl => {
            // Parse each line as JSON and collect into array
            let mut items = Vec::new();
            for line in trimmed.lines() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                    items.push(value);
                }
            }
            if items.is_empty() {
                None
            } else {
                Some(serde_json::Value::Array(items))
            }
        }
    }
}

/// Build a `Command` for executing an action script with the given interpreter.
///
/// This configures the command with:
/// - The interpreter binary and any additional args
/// - The action file path as the final argument
/// - Environment variables from the execution context
/// - Working directory (pack directory)
///
/// # Arguments
/// * `interpreter` - Path to the interpreter binary
/// * `interpreter_args` - Additional args before the action file
/// * `action_file` - Path to the action script file
/// * `working_dir` - Working directory for the process (typically the pack dir)
/// * `env_vars` - Environment variables to set
pub fn build_action_command(
    interpreter: &Path,
    interpreter_args: &[String],
    action_file: &Path,
    working_dir: Option<&Path>,
    env_vars: &HashMap<String, String>,
) -> Command {
    let mut cmd = Command::new(interpreter);

    // Add interpreter args (e.g., "-u" for unbuffered Python)
    for arg in interpreter_args {
        cmd.arg(arg);
    }

    // Add the action file as the last argument
    cmd.arg(action_file);

    // Set working directory
    if let Some(dir) = working_dir {
        if dir.exists() {
            cmd.current_dir(dir);
        }
    }

    // Set environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    cmd
}

/// Build a `Command` for executing inline code with the given interpreter.
///
/// This is used for ad-hoc/inline actions where code is passed as a string
/// rather than a file path.
///
/// # Arguments
/// * `interpreter` - Path to the interpreter binary
/// * `code` - The inline code to execute
/// * `env_vars` - Environment variables to set
pub fn build_inline_command(
    interpreter: &Path,
    code: &str,
    env_vars: &HashMap<String, String>,
) -> Command {
    let mut cmd = Command::new(interpreter);

    // Pass code via -c flag (works for bash, python, etc.)
    cmd.arg("-c").arg(code);

    // Set environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_output_text() {
        let result = parse_output("hello world", OutputFormat::Text);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_output_json() {
        let result = parse_output(r#"{"key": "value"}"#, OutputFormat::Json);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["key"], "value");
    }

    #[test]
    fn test_parse_output_json_with_log_prefix() {
        let result = parse_output(
            "some log line\nanother log\n{\"key\": \"value\"}",
            OutputFormat::Json,
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap()["key"], "value");
    }

    #[test]
    fn test_parse_output_jsonl() {
        let result = parse_output("{\"a\": 1}\n{\"b\": 2}\n{\"c\": 3}", OutputFormat::Jsonl);
        assert!(result.is_some());
        let arr = result.unwrap();
        assert_eq!(arr.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_parse_output_yaml() {
        let result = parse_output("key: value\nother: 42", OutputFormat::Yaml);
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["key"], "value");
        assert_eq!(val["other"], 42);
    }

    #[test]
    fn test_parse_output_empty() {
        assert!(parse_output("", OutputFormat::Json).is_none());
        assert!(parse_output("  ", OutputFormat::Yaml).is_none());
        assert!(parse_output("\n", OutputFormat::Jsonl).is_none());
    }

    #[tokio::test]
    async fn test_execute_streaming_simple() {
        let mut cmd = Command::new("/bin/echo");
        cmd.arg("hello world");

        let result = execute_streaming(
            cmd,
            &HashMap::new(),
            None,
            Some(10),
            1024 * 1024,
            1024 * 1024,
            OutputFormat::Text,
        )
        .await
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello world"));
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_execute_streaming_json_output() {
        let mut cmd = Command::new("/bin/bash");
        cmd.arg("-c").arg(r#"echo '{"status": "ok", "count": 42}'"#);

        let result = execute_streaming(
            cmd,
            &HashMap::new(),
            None,
            Some(10),
            1024 * 1024,
            1024 * 1024,
            OutputFormat::Json,
        )
        .await
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.result.is_some());
        let parsed = result.result.unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["count"], 42);
    }

    #[tokio::test]
    async fn test_execute_streaming_failure() {
        let mut cmd = Command::new("/bin/bash");
        cmd.arg("-c").arg("echo 'error msg' >&2; exit 1");

        let result = execute_streaming(
            cmd,
            &HashMap::new(),
            None,
            Some(10),
            1024 * 1024,
            1024 * 1024,
            OutputFormat::Text,
        )
        .await
        .unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(result.error.is_some());
        assert!(result.stderr.contains("error msg"));
    }

    #[tokio::test]
    async fn test_build_action_command() {
        let interpreter = Path::new("/usr/bin/python3");
        let args = vec!["-u".to_string()];
        let action_file = Path::new("/opt/attune/packs/mypack/actions/hello.py");
        let mut env = HashMap::new();
        env.insert("ATTUNE_EXEC_ID".to_string(), "123".to_string());

        let cmd = build_action_command(interpreter, &args, action_file, None, &env);

        // We can't easily inspect Command internals, but at least verify it builds without panic
        let _ = cmd;
    }
}
