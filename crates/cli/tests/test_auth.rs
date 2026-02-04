//! Integration tests for CLI authentication commands

#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;

mod common;
use common::*;

#[tokio::test]
async fn test_login_success() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    // Mock successful login
    mock_login_success(
        &fixture.mock_server,
        "test_access_token",
        "test_refresh_token",
    )
    .await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("auth")
        .arg("login")
        .arg("--username")
        .arg("testuser")
        .arg("--password")
        .arg("testpass");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Successfully logged in"));

    // Verify tokens were saved to config
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("test_access_token"));
    assert!(config_content.contains("test_refresh_token"));
}

#[tokio::test]
async fn test_login_failure() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    // Mock failed login
    mock_login_failure(&fixture.mock_server).await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("auth")
        .arg("login")
        .arg("--username")
        .arg("baduser")
        .arg("--password")
        .arg("badpass");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[tokio::test]
async fn test_whoami_authenticated() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock whoami endpoint
    mock_whoami_success(&fixture.mock_server, "testuser", "test@example.com").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("auth")
        .arg("whoami");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("testuser"))
        .stdout(predicate::str::contains("test@example.com"));
}

#[tokio::test]
async fn test_whoami_unauthenticated() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    // Mock unauthorized response
    mock_unauthorized(&fixture.mock_server, "/auth/whoami").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("auth")
        .arg("whoami");

    cmd.assert().failure();
}

#[tokio::test]
async fn test_logout() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Verify tokens exist before logout
    let config_before =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_before.contains("valid_token"));

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("auth")
        .arg("logout");

    cmd.assert().success().stdout(
        predicate::str::contains("logged out")
            .or(predicate::str::contains("Successfully logged out")),
    );

    // Verify tokens were removed from config
    let config_after =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(!config_after.contains("valid_token"));
}

#[tokio::test]
async fn test_login_with_profile_override() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    // Mock successful login
    mock_login_success(&fixture.mock_server, "staging_token", "staging_refresh").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--profile")
        .arg("default")
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("auth")
        .arg("login")
        .arg("--username")
        .arg("testuser")
        .arg("--password")
        .arg("testpass");

    cmd.assert().success();
}

#[tokio::test]
async fn test_login_missing_username() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .arg("auth")
        .arg("login")
        .arg("--password")
        .arg("testpass");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[tokio::test]
async fn test_whoami_json_output() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock whoami endpoint
    mock_whoami_success(&fixture.mock_server, "testuser", "test@example.com").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("--json")
        .arg("auth")
        .arg("whoami");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""username":"#))
        .stdout(predicate::str::contains("testuser"));
}

#[tokio::test]
async fn test_whoami_yaml_output() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock whoami endpoint
    mock_whoami_success(&fixture.mock_server, "testuser", "test@example.com").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("--yaml")
        .arg("auth")
        .arg("whoami");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("username:"))
        .stdout(predicate::str::contains("testuser"));
}
