//! Integration tests for CLI pack commands

#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;

mod common;
use common::*;

#[tokio::test]
async fn test_pack_list_authenticated() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock pack list endpoint
    mock_pack_list(&fixture.mock_server).await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("pack")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("core"))
        .stdout(predicate::str::contains("linux"));
}

#[tokio::test]
async fn test_pack_list_unauthenticated() {
    let fixture = TestFixture::new().await;
    fixture.write_default_config();

    // Mock unauthorized response
    mock_unauthorized(&fixture.mock_server, "/api/v1/packs").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("pack")
        .arg("list");

    cmd.assert().failure();
}

#[tokio::test]
async fn test_pack_list_json_output() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock pack list endpoint
    mock_pack_list(&fixture.mock_server).await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("--json")
        .arg("pack")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""ref": "core""#))
        .stdout(predicate::str::contains(r#""ref": "linux""#));
}

#[tokio::test]
async fn test_pack_list_yaml_output() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock pack list endpoint
    mock_pack_list(&fixture.mock_server).await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("--yaml")
        .arg("pack")
        .arg("list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ref: core"))
        .stdout(predicate::str::contains("ref: linux"));
}

#[tokio::test]
async fn test_pack_get_by_ref() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock pack get endpoint
    mock_pack_get(&fixture.mock_server, "core").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("pack")
        .arg("show")
        .arg("core");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("core"))
        .stdout(predicate::str::contains("core pack"));
}

#[tokio::test]
async fn test_pack_get_not_found() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock 404 response
    mock_not_found(&fixture.mock_server, "/api/v1/packs/nonexistent").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("pack")
        .arg("show")
        .arg("nonexistent");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[tokio::test]
async fn test_pack_list_with_profile() {
    let fixture = TestFixture::new().await;

    // Create multi-profile config with authentication on default
    let config = format!(
        r#"
current_profile: staging
default_output_format: table
profiles:
  default:
    api_url: {}
    auth_token: valid_token
    refresh_token: refresh_token
    description: Default server
  staging:
    api_url: {}
    auth_token: staging_token
    refresh_token: staging_refresh
    description: Staging server
"#,
        fixture.server_url(),
        fixture.server_url()
    );
    fixture.write_config(&config);

    // Mock pack list endpoint
    mock_pack_list(&fixture.mock_server).await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--profile")
        .arg("staging")
        .arg("pack")
        .arg("list");

    cmd.assert().success();
}

#[tokio::test]
async fn test_pack_list_with_api_url_override() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock pack list endpoint
    mock_pack_list(&fixture.mock_server).await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("pack")
        .arg("list");

    cmd.assert().success();
}

#[tokio::test]
async fn test_pack_get_json_output() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock pack get endpoint
    mock_pack_get(&fixture.mock_server, "core").await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("-j")
        .arg("pack")
        .arg("show")
        .arg("core");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(r#""ref": "core""#));
}

#[tokio::test]
async fn test_pack_list_empty_result() {
    let fixture = TestFixture::new().await;
    fixture.write_authenticated_config("valid_token", "refresh_token");

    // Mock empty pack list
    use serde_json::json;
    use wiremock::{
        matchers::{method, path},
        Mock, ResponseTemplate,
    };

    Mock::given(method("GET"))
        .and(path("/api/v1/packs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .mount(&fixture.mock_server)
        .await;

    let mut cmd = Command::cargo_bin("attune").unwrap();
    cmd.env("XDG_CONFIG_HOME", fixture.config_dir_path())
        .env("HOME", fixture.config_dir_path())
        .arg("--api-url")
        .arg(fixture.server_url())
        .arg("pack")
        .arg("list");

    cmd.assert().success();
}
