//! Security Tests for Secret Handling
//!
//! These tests verify that secrets are NOT exposed in process environment
//! or command-line arguments, ensuring secure secret passing via stdin.

use attune_worker::runtime::python::PythonRuntime;
use attune_worker::runtime::shell::ShellRuntime;
use attune_worker::runtime::{ExecutionContext, Runtime};
use std::collections::HashMap;

#[tokio::test]
async fn test_python_secrets_not_in_environ() {
    let runtime = PythonRuntime::new();

    let context = ExecutionContext {
        execution_id: 1,
        action_ref: "security.test_environ".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: {
            let mut s = HashMap::new();
            s.insert(
                "api_key".to_string(),
                "super_secret_key_do_not_expose".to_string(),
            );
            s.insert("password".to_string(), "secret_pass_123".to_string());
            s
        },
        timeout: Some(10),
        working_dir: None,
        entry_point: "run".to_string(),
        code: Some(
            r#"
import os

def run():
    # Check if secrets are in environment variables
    environ_str = str(os.environ)

    # Secrets should NOT be in environment
    has_secret_in_env = 'super_secret_key_do_not_expose' in environ_str
    has_password_in_env = 'secret_pass_123' in environ_str
    has_secret_prefix = 'SECRET_API_KEY' in os.environ or 'SECRET_PASSWORD' in os.environ

    # But they SHOULD be accessible via get_secret()
    api_key_accessible = get_secret('api_key') == 'super_secret_key_do_not_expose'
    password_accessible = get_secret('password') == 'secret_pass_123'

    return {
        'secrets_in_environ': has_secret_in_env or has_password_in_env or has_secret_prefix,
        'api_key_accessible': api_key_accessible,
        'password_accessible': password_accessible,
        'environ_check': 'SECRET_' not in environ_str
    }
"#
            .to_string(),
        ),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 10 * 1024 * 1024,
        parameter_delivery: attune_worker::runtime::ParameterDelivery::default(),
        parameter_format: attune_worker::runtime::ParameterFormat::default(),
    };

    let result = runtime.execute(context).await.unwrap();
    assert!(result.is_success(), "Execution should succeed");

    let result_data = result.result.unwrap();
    let result_obj = result_data.get("result").unwrap();

    // Critical security check: secrets should NOT be in environment
    assert_eq!(
        result_obj.get("secrets_in_environ").unwrap(),
        &serde_json::json!(false),
        "SECURITY FAILURE: Secrets found in process environment!"
    );

    // Verify secrets ARE accessible via secure method
    assert_eq!(
        result_obj.get("api_key_accessible").unwrap(),
        &serde_json::json!(true),
        "Secrets should be accessible via get_secret()"
    );
    assert_eq!(
        result_obj.get("password_accessible").unwrap(),
        &serde_json::json!(true),
        "Secrets should be accessible via get_secret()"
    );

    // Verify no SECRET_ prefix in environment
    assert_eq!(
        result_obj.get("environ_check").unwrap(),
        &serde_json::json!(true),
        "Environment should not contain SECRET_ prefix variables"
    );
}

#[tokio::test]
async fn test_shell_secrets_not_in_environ() {
    let runtime = ShellRuntime::new();

    let context = ExecutionContext {
        execution_id: 2,
        action_ref: "security.test_shell_environ".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: {
            let mut s = HashMap::new();
            s.insert(
                "api_key".to_string(),
                "super_secret_key_do_not_expose".to_string(),
            );
            s.insert("password".to_string(), "secret_pass_123".to_string());
            s
        },
        timeout: Some(10),
        working_dir: None,
        entry_point: "shell".to_string(),
        code: Some(
            r#"
# Check if secrets are in environment variables
if printenv | grep -q "super_secret_key_do_not_expose"; then
    echo "SECURITY_FAIL: Secret found in environment"
    exit 1
fi

if printenv | grep -q "secret_pass_123"; then
    echo "SECURITY_FAIL: Password found in environment"
    exit 1
fi

if printenv | grep -q "SECRET_API_KEY"; then
    echo "SECURITY_FAIL: SECRET_ prefix found in environment"
    exit 1
fi

# But secrets SHOULD be accessible via get_secret function
api_key=$(get_secret 'api_key')
password=$(get_secret 'password')

if [ "$api_key" != "super_secret_key_do_not_expose" ]; then
    echo "ERROR: Secret not accessible via get_secret"
    exit 1
fi

if [ "$password" != "secret_pass_123" ]; then
    echo "ERROR: Password not accessible via get_secret"
    exit 1
fi

echo "SECURITY_PASS: Secrets not in environment but accessible via get_secret"
"#
            .to_string(),
        ),
        code_path: None,
        runtime_name: Some("shell".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 10 * 1024 * 1024,
        parameter_delivery: attune_worker::runtime::ParameterDelivery::default(),
        parameter_format: attune_worker::runtime::ParameterFormat::default(),
    };

    let result = runtime.execute(context).await.unwrap();

    // Check execution succeeded
    assert!(result.is_success(), "Execution should succeed");
    assert_eq!(result.exit_code, 0, "Exit code should be 0");

    // Verify security pass message
    assert!(
        result.stdout.contains("SECURITY_PASS"),
        "Security checks should pass"
    );
    assert!(
        !result.stdout.contains("SECURITY_FAIL"),
        "Should not have security failures"
    );
}

#[tokio::test]
async fn test_python_secret_isolation_between_actions() {
    let runtime = PythonRuntime::new();

    // First action with secret A
    let context1 = ExecutionContext {
        execution_id: 3,
        action_ref: "security.action1".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: {
            let mut s = HashMap::new();
            s.insert("secret_a".to_string(), "value_a".to_string());
            s
        },
        timeout: Some(10),
        working_dir: None,
        entry_point: "run".to_string(),
        code: Some(
            r#"
def run():
    return {'secret_a': get_secret('secret_a')}
"#
            .to_string(),
        ),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 10 * 1024 * 1024,
        parameter_delivery: attune_worker::runtime::ParameterDelivery::default(),
        parameter_format: attune_worker::runtime::ParameterFormat::default(),
    };

    let result1 = runtime.execute(context1).await.unwrap();
    assert!(result1.is_success());

    // Second action with secret B (should not see secret A)
    let context2 = ExecutionContext {
        execution_id: 4,
        action_ref: "security.action2".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: {
            let mut s = HashMap::new();
            s.insert("secret_b".to_string(), "value_b".to_string());
            s
        },
        timeout: Some(10),
        working_dir: None,
        entry_point: "run".to_string(),
        code: Some(
            r#"
def run():
    # Should NOT see secret_a from previous action
    secret_a = get_secret('secret_a')
    secret_b = get_secret('secret_b')
    return {
        'secret_a_leaked': secret_a is not None,
        'secret_b_present': secret_b == 'value_b'
    }
"#
            .to_string(),
        ),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 10 * 1024 * 1024,
        parameter_delivery: attune_worker::runtime::ParameterDelivery::default(),
        parameter_format: attune_worker::runtime::ParameterFormat::default(),
    };

    let result2 = runtime.execute(context2).await.unwrap();
    assert!(result2.is_success());

    let result_data = result2.result.unwrap();
    let result_obj = result_data.get("result").unwrap();

    // Verify secrets don't leak between actions
    assert_eq!(
        result_obj.get("secret_a_leaked").unwrap(),
        &serde_json::json!(false),
        "Secret from previous action should not leak"
    );
    assert_eq!(
        result_obj.get("secret_b_present").unwrap(),
        &serde_json::json!(true),
        "Current action's secret should be present"
    );
}

#[tokio::test]
async fn test_python_empty_secrets() {
    let runtime = PythonRuntime::new();

    let context = ExecutionContext {
        execution_id: 5,
        action_ref: "security.no_secrets".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(), // No secrets
        timeout: Some(10),
        working_dir: None,
        entry_point: "run".to_string(),
        code: Some(
            r#"
def run():
    # get_secret should return None for non-existent secrets
    result = get_secret('nonexistent')
    return {'result': result}
"#
            .to_string(),
        ),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 10 * 1024 * 1024,
        parameter_delivery: attune_worker::runtime::ParameterDelivery::default(),
        parameter_format: attune_worker::runtime::ParameterFormat::default(),
    };

    let result = runtime.execute(context).await.unwrap();
    assert!(
        result.is_success(),
        "Should handle empty secrets gracefully"
    );

    let result_data = result.result.unwrap();
    let result_obj = result_data.get("result").unwrap();
    assert_eq!(result_obj.get("result").unwrap(), &serde_json::Value::Null);
}

#[tokio::test]
async fn test_shell_empty_secrets() {
    let runtime = ShellRuntime::new();

    let context = ExecutionContext {
        execution_id: 6,
        action_ref: "security.no_secrets".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: HashMap::new(), // No secrets
        timeout: Some(10),
        working_dir: None,
        entry_point: "shell".to_string(),
        code: Some(
            r#"
# get_secret should return empty string for non-existent secrets
result=$(get_secret 'nonexistent')
if [ -z "$result" ]; then
    echo "PASS: Empty secret returns empty string"
else
    echo "FAIL: Expected empty string"
    exit 1
fi
"#
            .to_string(),
        ),
        code_path: None,
        runtime_name: Some("shell".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 10 * 1024 * 1024,
        parameter_delivery: attune_worker::runtime::ParameterDelivery::default(),
        parameter_format: attune_worker::runtime::ParameterFormat::default(),
    };

    let result = runtime.execute(context).await.unwrap();
    assert!(
        result.is_success(),
        "Should handle empty secrets gracefully"
    );
    assert!(result.stdout.contains("PASS"));
}

#[tokio::test]
async fn test_python_special_characters_in_secrets() {
    let runtime = PythonRuntime::new();

    let context = ExecutionContext {
        execution_id: 7,
        action_ref: "security.special_chars".to_string(),
        parameters: HashMap::new(),
        env: HashMap::new(),
        secrets: {
            let mut s = HashMap::new();
            s.insert("special_chars".to_string(), "test!@#$%^&*()".to_string());
            s.insert("with_newline".to_string(), "line1\nline2".to_string());
            s
        },
        timeout: Some(10),
        working_dir: None,
        entry_point: "run".to_string(),
        code: Some(
            r#"
def run():
    special = get_secret('special_chars')
    newline = get_secret('with_newline')

    newline_char = chr(10)
    newline_parts = newline.split(newline_char) if newline else []

    return {
        'special_correct': special == 'test!@#$%^&*()',
        'newline_has_two_parts': len(newline_parts) == 2,
        'newline_first_part': newline_parts[0] if len(newline_parts) > 0 else '',
        'newline_second_part': newline_parts[1] if len(newline_parts) > 1 else '',
        'special_len': len(special) if special else 0
    }
"#
            .to_string(),
        ),
        code_path: None,
        runtime_name: Some("python".to_string()),
        max_stdout_bytes: 10 * 1024 * 1024,
        max_stderr_bytes: 10 * 1024 * 1024,
        parameter_delivery: attune_worker::runtime::ParameterDelivery::default(),
        parameter_format: attune_worker::runtime::ParameterFormat::default(),
    };

    let result = runtime.execute(context).await.unwrap();
    assert!(
        result.is_success(),
        "Should handle special characters: {:?}",
        result.error
    );

    let result_data = result.result.unwrap();
    let result_obj = result_data.get("result").unwrap();

    assert_eq!(
        result_obj.get("special_correct").unwrap(),
        &serde_json::json!(true),
        "Special characters should be preserved"
    );
    assert_eq!(
        result_obj.get("newline_has_two_parts").unwrap(),
        &serde_json::json!(true),
        "Newline should split into two parts"
    );
    assert_eq!(
        result_obj.get("newline_first_part").unwrap(),
        &serde_json::json!("line1"),
        "First part should be 'line1'"
    );
    assert_eq!(
        result_obj.get("newline_second_part").unwrap(),
        &serde_json::json!("line2"),
        "Second part should be 'line2'"
    );
}
