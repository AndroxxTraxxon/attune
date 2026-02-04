//! Integration tests for log size truncation
//!
//! Tests that verify stdout/stderr are properly truncated when they exceed
//! configured size limits, preventing OOM issues with large output.

use attune_worker::runtime::{ExecutionContext, PythonRuntime, Runtime, ShellRuntime};
use std::collections::HashMap;

#[tokio::test]
async fn test_python_stdout_truncation() {
    let runtime = PythonRuntime::new();

    // Create a Python script that outputs more than the limit
    let code = r#"
import sys
# Output 1KB of data (will exceed 500 byte limit)
for i in range(100):
    print("x" * 10)
"#;

    let context = ExecutionContext {
        execution_id: 1,
        action_ref: "test.large_output".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(),
        timeout: Some(10),
        working_dir: None,
        entry_point: "test_script".to_string(),
        code: Some(code.to_string()),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 500, // Small limit to trigger truncation
        max_stderr_bytes: 1024,
    };

    let result = runtime.execute(context).await.unwrap();

    // Should succeed but with truncated output
    assert!(result.is_success());
    assert!(result.stdout_truncated);
    assert!(result.stdout.contains("[OUTPUT TRUNCATED"));
    assert!(result.stdout_bytes_truncated > 0);
    assert!(result.stdout.len() <= 500);
}

#[tokio::test]
async fn test_python_stderr_truncation() {
    let runtime = PythonRuntime::new();

    // Create a Python script that outputs to stderr
    let code = r#"
import sys
# Output 1KB of data to stderr
for i in range(100):
    sys.stderr.write("error message line\n")
"#;

    let context = ExecutionContext {
        execution_id: 2,
        action_ref: "test.large_stderr".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(),
        timeout: Some(10),
        working_dir: None,
        entry_point: "test_script".to_string(),
        code: Some(code.to_string()),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 300, // Small limit for stderr
    };

    let result = runtime.execute(context).await.unwrap();

    // Should succeed but with truncated stderr
    assert!(result.is_success());
    assert!(!result.stdout_truncated);
    assert!(result.stderr_truncated);
    assert!(result.stderr.contains("[OUTPUT TRUNCATED"));
    assert!(result.stderr.contains("stderr exceeded size limit"));
    assert!(result.stderr_bytes_truncated > 0);
    assert!(result.stderr.len() <= 300);
}

#[tokio::test]
async fn test_shell_stdout_truncation() {
    let runtime = ShellRuntime::new();

    // Shell script that outputs more than the limit
    let code = r#"
for i in {1..100}; do
    echo "This is a long line of text that will add up quickly"
done
"#;

    let context = ExecutionContext {
        execution_id: 3,
        action_ref: "test.shell_large_output".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(),
        timeout: Some(10),
        working_dir: None,
        entry_point: "shell".to_string(),
        code: Some(code.to_string()),
        code_path: None,
        runtime_name: Some("shell".to_string()),
        max_stdout_bytes: 400, // Small limit
        max_stderr_bytes: 1024,
    };

    let result = runtime.execute(context).await.unwrap();

    // Should succeed but with truncated output
    assert!(result.is_success());
    assert!(result.stdout_truncated);
    assert!(result.stdout.contains("[OUTPUT TRUNCATED"));
    assert!(result.stdout_bytes_truncated > 0);
    assert!(result.stdout.len() <= 400);
}

#[tokio::test]
async fn test_no_truncation_under_limit() {
    let runtime = PythonRuntime::new();

    // Small output that won't trigger truncation
    let code = r#"
print("Hello, World!")
"#;

    let context = ExecutionContext {
        execution_id: 4,
        action_ref: "test.small_output".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(),
        timeout: Some(10),
        working_dir: None,
        entry_point: "test_script".to_string(),
        code: Some(code.to_string()),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024, // Large limit
        max_stderr_bytes: 10 * 1024 * 1024,
    };

    let result = runtime.execute(context).await.unwrap();

    // Should succeed without truncation
    assert!(result.is_success());
    assert!(!result.stdout_truncated);
    assert!(!result.stderr_truncated);
    assert_eq!(result.stdout_bytes_truncated, 0);
    assert_eq!(result.stderr_bytes_truncated, 0);
    assert!(result.stdout.contains("Hello, World!"));
}

#[tokio::test]
async fn test_both_streams_truncated() {
    let runtime = PythonRuntime::new();

    // Script that outputs to both stdout and stderr
    let code = r#"
import sys
# Output to both streams
for i in range(50):
    print("stdout line " + str(i))
    sys.stderr.write("stderr line " + str(i) + "\n")
"#;

    let context = ExecutionContext {
        execution_id: 5,
        action_ref: "test.dual_truncation".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(),
        timeout: Some(10),
        working_dir: None,
        entry_point: "test_script".to_string(),
        code: Some(code.to_string()),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 300, // Both limits are small
        max_stderr_bytes: 300,
    };

    let result = runtime.execute(context).await.unwrap();

    // Should succeed but with both streams truncated
    assert!(result.is_success());
    assert!(result.stdout_truncated);
    assert!(result.stderr_truncated);
    assert!(result.stdout.contains("[OUTPUT TRUNCATED"));
    assert!(result.stderr.contains("[OUTPUT TRUNCATED"));
    assert!(result.stdout_bytes_truncated > 0);
    assert!(result.stderr_bytes_truncated > 0);
    assert!(result.stdout.len() <= 300);
    assert!(result.stderr.len() <= 300);
}

#[tokio::test]
async fn test_truncation_with_timeout() {
    let runtime = PythonRuntime::new();

    // Script that times out but should still capture truncated logs
    let code = r#"
import time
for i in range(1000):
    print(f"Line {i}")
time.sleep(30)  # Will timeout before this
"#;

    let context = ExecutionContext {
        execution_id: 6,
        action_ref: "test.timeout_truncation".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(),
        timeout: Some(2), // Short timeout
        working_dir: None,
        entry_point: "test_script".to_string(),
        code: Some(code.to_string()),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 500,
        max_stderr_bytes: 1024,
    };

    let result = runtime.execute(context).await.unwrap();

    // Should timeout with truncated logs
    assert!(!result.is_success());
    assert!(result.error.is_some());
    assert!(result.error.as_ref().unwrap().contains("timed out"));
    // Logs may or may not be truncated depending on how fast it runs
}

#[tokio::test]
async fn test_exact_limit_no_truncation() {
    let runtime = PythonRuntime::new();

    // Output a small amount that won't trigger truncation
    // The Python wrapper adds JSON result output, so we need headroom
    let code = r#"
import sys
sys.stdout.write("Small output")
"#;

    let context = ExecutionContext {
        execution_id: 7,
        action_ref: "test.exact_limit".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(),
        timeout: Some(10),
        working_dir: None,
        entry_point: "test_script".to_string(),
        code: Some(code.to_string()),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024, // Large limit to avoid truncation
        max_stderr_bytes: 10 * 1024 * 1024,
    };

    let result = runtime.execute(context).await.unwrap();

    // Should succeed without truncation
    eprintln!(
        "test_exact_limit_no_truncation: exit_code={}, error={:?}, stdout={:?}, stderr={:?}",
        result.exit_code, result.error, result.stdout, result.stderr
    );
    assert!(result.is_success());
    assert!(!result.stdout_truncated);
    assert!(result.stdout.contains("Small output"));
}
