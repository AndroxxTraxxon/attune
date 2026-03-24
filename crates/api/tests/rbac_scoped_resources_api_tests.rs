use axum::http::StatusCode;
use helpers::*;
use serde_json::json;

use attune_common::{
    models::enums::{ArtifactType, ArtifactVisibility, OwnerType, RetentionPolicyType},
    repositories::{
        artifact::{ArtifactRepository, CreateArtifactInput},
        identity::{
            CreatePermissionAssignmentInput, CreatePermissionSetInput, IdentityRepository,
            PermissionAssignmentRepository, PermissionSetRepository,
        },
        key::{CreateKeyInput, KeyRepository},
        Create,
    },
};

mod helpers;

async fn register_scoped_user(
    ctx: &TestContext,
    login: &str,
    grants: serde_json::Value,
) -> Result<String> {
    let response = ctx
        .post(
            "/auth/register",
            json!({
                "login": login,
                "password": "TestPassword123!",
                "display_name": format!("Scoped User {}", login),
            }),
            None,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await?;
    let token = body["data"]["access_token"]
        .as_str()
        .expect("missing access token")
        .to_string();

    let identity = IdentityRepository::find_by_login(&ctx.pool, login)
        .await?
        .expect("registered identity should exist");

    let permset = PermissionSetRepository::create(
        &ctx.pool,
        CreatePermissionSetInput {
            r#ref: format!("test.scoped_{}", uuid::Uuid::new_v4().simple()),
            pack: None,
            pack_ref: None,
            label: Some("Scoped Test Permission Set".to_string()),
            description: Some("Scoped test grants".to_string()),
            grants,
        },
    )
    .await?;

    PermissionAssignmentRepository::create(
        &ctx.pool,
        CreatePermissionAssignmentInput {
            identity: identity.id,
            permset: permset.id,
        },
    )
    .await?;

    Ok(token)
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_pack_scoped_key_permissions_enforce_owner_refs() {
    let ctx = TestContext::new()
        .await
        .expect("Failed to create test context");

    let token = register_scoped_user(
        &ctx,
        &format!("scoped_keys_{}", uuid::Uuid::new_v4().simple()),
        json!([
            {
                "resource": "keys",
                "actions": ["read"],
                "constraints": {
                    "owner_types": ["pack"],
                    "owner_refs": ["python_example"]
                }
            }
        ]),
    )
    .await
    .expect("Failed to register scoped user");

    KeyRepository::create(
        &ctx.pool,
        CreateKeyInput {
            r#ref: format!("python_example_key_{}", uuid::Uuid::new_v4().simple()),
            owner_type: OwnerType::Pack,
            owner: Some("python_example".to_string()),
            owner_identity: None,
            owner_pack: None,
            owner_pack_ref: Some("python_example".to_string()),
            owner_action: None,
            owner_action_ref: None,
            owner_sensor: None,
            owner_sensor_ref: None,
            name: "Python Example Key".to_string(),
            encrypted: false,
            encryption_key_hash: None,
            value: json!("allowed"),
        },
    )
    .await
    .expect("Failed to create scoped key");

    let blocked_key = KeyRepository::create(
        &ctx.pool,
        CreateKeyInput {
            r#ref: format!("other_pack_key_{}", uuid::Uuid::new_v4().simple()),
            owner_type: OwnerType::Pack,
            owner: Some("other_pack".to_string()),
            owner_identity: None,
            owner_pack: None,
            owner_pack_ref: Some("other_pack".to_string()),
            owner_action: None,
            owner_action_ref: None,
            owner_sensor: None,
            owner_sensor_ref: None,
            name: "Other Pack Key".to_string(),
            encrypted: false,
            encryption_key_hash: None,
            value: json!("blocked"),
        },
    )
    .await
    .expect("Failed to create blocked key");

    let allowed_list = ctx
        .get("/api/v1/keys", Some(&token))
        .await
        .expect("Failed to list keys");
    assert_eq!(allowed_list.status(), StatusCode::OK);
    let allowed_body: serde_json::Value = allowed_list.json().await.expect("Invalid key list");
    assert_eq!(
        allowed_body["data"]
            .as_array()
            .expect("expected list")
            .len(),
        1
    );
    assert_eq!(allowed_body["data"][0]["owner"], "python_example");

    let blocked_get = ctx
        .get(&format!("/api/v1/keys/{}", blocked_key.r#ref), Some(&token))
        .await
        .expect("Failed to fetch blocked key");
    assert_eq!(blocked_get.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_pack_scoped_artifact_permissions_enforce_owner_refs() {
    let ctx = TestContext::new()
        .await
        .expect("Failed to create test context");

    let token = register_scoped_user(
        &ctx,
        &format!("scoped_artifacts_{}", uuid::Uuid::new_v4().simple()),
        json!([
            {
                "resource": "artifacts",
                "actions": ["read", "create"],
                "constraints": {
                    "owner_types": ["pack"],
                    "owner_refs": ["python_example"]
                }
            }
        ]),
    )
    .await
    .expect("Failed to register scoped user");

    let allowed_artifact = ArtifactRepository::create(
        &ctx.pool,
        CreateArtifactInput {
            r#ref: format!("python_example.allowed_{}", uuid::Uuid::new_v4().simple()),
            scope: OwnerType::Pack,
            owner: "python_example".to_string(),
            r#type: ArtifactType::FileText,
            visibility: ArtifactVisibility::Private,
            retention_policy: RetentionPolicyType::Versions,
            retention_limit: 5,
            name: Some("Allowed Artifact".to_string()),
            description: None,
            content_type: Some("text/plain".to_string()),
            execution: None,
            data: None,
        },
    )
    .await
    .expect("Failed to create allowed artifact");

    let blocked_artifact = ArtifactRepository::create(
        &ctx.pool,
        CreateArtifactInput {
            r#ref: format!("other_pack.blocked_{}", uuid::Uuid::new_v4().simple()),
            scope: OwnerType::Pack,
            owner: "other_pack".to_string(),
            r#type: ArtifactType::FileText,
            visibility: ArtifactVisibility::Private,
            retention_policy: RetentionPolicyType::Versions,
            retention_limit: 5,
            name: Some("Blocked Artifact".to_string()),
            description: None,
            content_type: Some("text/plain".to_string()),
            execution: None,
            data: None,
        },
    )
    .await
    .expect("Failed to create blocked artifact");

    let allowed_get = ctx
        .get(
            &format!("/api/v1/artifacts/{}", allowed_artifact.id),
            Some(&token),
        )
        .await
        .expect("Failed to fetch allowed artifact");
    assert_eq!(allowed_get.status(), StatusCode::OK);

    let blocked_get = ctx
        .get(
            &format!("/api/v1/artifacts/{}", blocked_artifact.id),
            Some(&token),
        )
        .await
        .expect("Failed to fetch blocked artifact");
    assert_eq!(blocked_get.status(), StatusCode::NOT_FOUND);

    let create_allowed = ctx
        .post(
            "/api/v1/artifacts",
            json!({
                "ref": format!("python_example.created_{}", uuid::Uuid::new_v4().simple()),
                "scope": "pack",
                "owner": "python_example",
                "type": "file_text",
                "name": "Created Artifact"
            }),
            Some(&token),
        )
        .await
        .expect("Failed to create allowed artifact");
    assert_eq!(create_allowed.status(), StatusCode::CREATED);

    let create_blocked = ctx
        .post(
            "/api/v1/artifacts",
            json!({
                "ref": format!("other_pack.created_{}", uuid::Uuid::new_v4().simple()),
                "scope": "pack",
                "owner": "other_pack",
                "type": "file_text",
                "name": "Blocked Artifact"
            }),
            Some(&token),
        )
        .await
        .expect("Failed to create blocked artifact");
    assert_eq!(create_blocked.status(), StatusCode::FORBIDDEN);
}
