//! Integration tests for CLI config and profile management commands
#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;

mod common;
use common::*;

#[tokio::test]
async fn test_config_show_default() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("api_url"));
}

#[tokio::test]
async fn test_config_show_json_output() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--json")
        .arg("config")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""profile""#))
        .stdout(predicate::str::contains(r#""api_url""#));
}

#[tokio::test]
async fn test_config_show_yaml_output() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--yaml")
        .arg("config")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("profile:"))
        .stdout(predicate::str::contains("api_url:"));
}

#[tokio::test]
async fn test_config_get_specific_key() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("get")
        .arg("api_url");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(fixture.server_url()));
}

#[tokio::test]
async fn test_config_get_nonexistent_key() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("get")
        .arg("nonexistent_key");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[tokio::test]
async fn test_config_set_api_url() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("set")
        .arg("api_url")
        .arg("https://new-api.example.com");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration updated"));

    // Verify the change was persisted
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("https://new-api.example.com"));
}

#[tokio::test]
async fn test_config_set_format() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("set")
        .arg("format")
        .arg("json");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration updated"));

    // Verify the change was persisted
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("format: json"));
}

#[tokio::test]
async fn test_profile_list() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("profiles");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("default"))
        .stdout(predicate::str::contains("staging"))
        .stdout(predicate::str::contains("production"));
}

#[tokio::test]
async fn test_profile_list_shows_current() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("profiles");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("*").or(predicate::str::contains("(active)")));
}

#[tokio::test]
async fn test_profile_show_specific() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("show-profile")
        .arg("staging");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("staging.example.com"));
}

#[tokio::test]
async fn test_profile_show_nonexistent() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("show-profile")
        .arg("nonexistent");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[tokio::test]
async fn test_profile_add_new() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("add-profile")
        .arg("testing")
        .arg("--api-url")
        .arg("https://test.example.com")
        .arg("--description")
        .arg("Testing environment");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Profile 'testing' added"));

    // Verify the profile was added
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("testing:"));
    assert!(config_content.contains("https://test.example.com"));
}

#[tokio::test]
async fn test_profile_add_without_description() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("add-profile")
        .arg("newprofile")
        .arg("--api-url")
        .arg("https://new.example.com");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Profile 'newprofile' added"));
}

#[tokio::test]
async fn test_profile_use_switch() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("use")
        .arg("staging");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Switched to profile 'staging'"));

    // Verify the current profile was changed
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("profile: staging"));
}

#[tokio::test]
async fn test_profile_use_nonexistent() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("use")
        .arg("nonexistent");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[tokio::test]
async fn test_profile_remove() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("remove-profile")
        .arg("staging");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Profile 'staging' removed"));

    // Verify the profile was removed
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(!config_content.contains("staging:"));
}

#[tokio::test]
async fn test_profile_remove_default_fails() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("remove-profile")
        .arg("default");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Cannot remove"));
}

#[tokio::test]
async fn test_profile_remove_active_fails() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    // Try to remove the currently active profile
    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("remove-profile")
        .arg("default");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Cannot remove active profile"));
}

#[tokio::test]
async fn test_profile_remove_nonexistent() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("remove-profile")
        .arg("nonexistent");

    cmd.assert().success(); // Removing non-existent profile might be a no-op
}

#[tokio::test]
async fn test_profile_override_with_flag() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    // Use --profile flag to temporarily override
    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--profile")
        .arg("staging")
        .arg("config")
        .arg("list");

    cmd.assert().success();

    // Verify current profile wasn't changed in the config file
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("profile: default"));
}

#[tokio::test]
async fn test_profile_override_with_env_var() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    // Use ATTUNE_PROFILE env var to temporarily override
    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .env("ATTUNE_PROFILE", "production")
        .arg("config")
        .arg("list");

    cmd.assert().success();

    // Verify current profile wasn't changed in the config file
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("profile: default"));
}

#[tokio::test]
async fn test_config_format_respected_by_commands() {
    let fixture = TestFixture::new().await;
    // Write a config with format set to json
    let config = format!(
        r#"
profile: default
format: json
profiles:
  default:
    api_url: {}
    description: Test server
"#,
        fixture.server_url()
    );
    fixture.write_config(&config);

    // Run config list without --json flag; should output JSON because config says so
    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("list");

    // JSON output contains curly braces
    cmd.assert().success().stdout(predicate::str::contains("{"));
}

#[tokio::test]
async fn test_config_list_all_keys() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("test_token", "test_refresh");

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("api_url"))
        .stdout(predicate::str::contains("format"))
        .stdout(predicate::str::contains("auth_token"));
}

#[tokio::test]
async fn test_config_masks_sensitive_data() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("secret_token_123", "secret_refresh_456");

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("get")
        .arg("auth_token");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("***"));
}

#[tokio::test]
async fn test_profile_add_duplicate_overwrites() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    // Add a profile with the same name as existing one
    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("add-profile")
        .arg("staging")
        .arg("--api-url")
        .arg("https://new-staging.example.com");

    cmd.assert().success();

    // Verify the profile was updated
    let config_content =
        std::fs::read_to_string(&fixture.config_path).expect("Failed to read config");
    assert!(config_content.contains("https://new-staging.example.com"));
}

#[tokio::test]
async fn test_profile_list_json_output() {
    let fixture = TestFixture::new().await;
    fixture.write_multi_profile_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--json")
        .arg("config")
        .arg("profiles");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""default""#))
        .stdout(predicate::str::contains(r#""staging""#));
}

#[tokio::test]
async fn test_config_path_display() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("config")
        .arg("path");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("config.yaml"));
}
