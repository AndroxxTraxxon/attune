use axum::http::StatusCode;
use helpers::*;
use serde_json::json;

use attune_common::{
    models::{
        enums::{ArtifactType, ArtifactVisibility, OwnerType, RetentionPolicyType},
        WorkQueueBatchMode, WorkQueueItemStatus, WorkQueueUpdateStrategy,
    },
    repositories::{
        action::{ActionRepository, CreateActionInput},
        artifact::{ArtifactRepository, CreateArtifactInput},
        identity::{
            CreatePermissionAssignmentInput, CreatePermissionSetInput, IdentityRepository,
            PermissionAssignmentRepository, PermissionSetRepository,
        },
        key::{CreateKeyInput, KeyRepository},
        work_queue::{
            CreateWorkQueueInput, UpdateWorkQueueItemInput, WorkQueueItemRepository,
            WorkQueueRepository,
        },
        Create, FindByRef, Update,
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

async fn create_pack_with_action(
    ctx: &TestContext,
    pack_ref: &str,
    action_ref: &str,
) -> Result<(
    attune_common::models::Pack,
    attune_common::models::action::Action,
)> {
    let pack = create_test_pack(&ctx.pool, pack_ref).await?;
    let action = ActionRepository::create(
        &ctx.pool,
        CreateActionInput {
            r#ref: action_ref.to_string(),
            pack: pack.id,
            pack_ref: pack.r#ref.clone(),
            label: format!("Action {}", action_ref),
            description: Some("Queue dispatch action".to_string()),
            entrypoint: "main.py".to_string(),
            runtime: None,
            runtime_version_constraint: None,
            param_schema: None,
            out_schema: None,
            is_adhoc: false,
        },
    )
    .await?;

    Ok((pack, action))
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

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_queue_admin_like_crud_and_pending_item_guards() {
    let ctx = TestContext::new()
        .await
        .expect("Failed to create test context");

    let (_pack, action) = create_pack_with_action(
        &ctx,
        &format!("queue_admin_pack_{}", uuid::Uuid::new_v4().simple()),
        &format!(
            "queue_admin_pack.dispatch_{}",
            uuid::Uuid::new_v4().simple()
        ),
    )
    .await
    .expect("Failed to create queue fixture");

    let token = register_scoped_user(
        &ctx,
        &format!("queue_admin_{}", uuid::Uuid::new_v4().simple()),
        json!([
            {
                "resource": "queues",
                "actions": ["read", "create", "update", "delete"]
            }
        ]),
    )
    .await
    .expect("Failed to register queue admin user");

    let queue_ref = format!("adhoc_queue_{}", uuid::Uuid::new_v4().simple());
    let create_queue = ctx
        .post(
            "/api/v1/queues",
            json!({
                "ref": queue_ref,
                "label": "Adhoc Queue",
                "dispatch_action_ref": action.r#ref,
                "allow_pending_update": true,
                "update_strategy": "merge_patch",
                "batch_mode": "single"
            }),
            Some(&token),
        )
        .await
        .expect("Failed to create adhoc queue");
    assert_eq!(create_queue.status(), StatusCode::CREATED);

    let enqueue = ctx
        .post(
            &format!("/api/v1/queues/{}/items", queue_ref),
            json!({
                "item_key": "order-123",
                "payload": {"state": "queued"},
                "metadata": {"source": "api"}
            }),
            Some(&token),
        )
        .await
        .expect("Failed to enqueue queue item");
    assert_eq!(enqueue.status(), StatusCode::CREATED);
    let enqueue_body: serde_json::Value = enqueue.json().await.expect("Invalid enqueue response");
    let item_id = enqueue_body["data"]["id"]
        .as_i64()
        .expect("Missing queue item id");

    let enqueue_merge = ctx
        .post(
            &format!("/api/v1/queues/{}/items", queue_ref),
            json!({
                "item_key": "order-123",
                "payload": {"extra": true},
                "metadata": {"attempt": 2}
            }),
            Some(&token),
        )
        .await
        .expect("Failed to merge queue item");
    assert_eq!(enqueue_merge.status(), StatusCode::OK);

    let update_item = ctx
        .put(
            &format!("/api/v1/queues/{}/items/{}", queue_ref, item_id),
            json!({
                "priority": 9,
                "metadata": {"attempt": 3}
            }),
            Some(&token),
        )
        .await
        .expect("Failed to update pending queue item");
    assert_eq!(update_item.status(), StatusCode::OK);

    let list_items = ctx
        .get(&format!("/api/v1/queues/{}/items", queue_ref), Some(&token))
        .await
        .expect("Failed to list queue items");
    assert_eq!(list_items.status(), StatusCode::OK);
    let list_body: serde_json::Value = list_items.json().await.expect("Invalid queue item list");
    assert_eq!(
        list_body["data"].as_array().expect("Expected array").len(),
        1
    );
    assert_eq!(list_body["data"][0]["payload"]["state"], "queued");
    assert_eq!(list_body["data"][0]["payload"]["extra"], true);
    assert_eq!(list_body["data"][0]["priority"], 9);

    let queue = WorkQueueRepository::find_by_ref(&ctx.pool, &queue_ref)
        .await
        .expect("Failed to load queue")
        .expect("Queue should exist");
    let item = WorkQueueItemRepository::find_by_queue_and_id(&ctx.pool, queue.id, item_id)
        .await
        .expect("Failed to load queue item")
        .expect("Queue item should exist");
    WorkQueueItemRepository::update(
        &ctx.pool,
        item.id,
        UpdateWorkQueueItemInput {
            status: Some(WorkQueueItemStatus::Completed),
            ..Default::default()
        },
    )
    .await
    .expect("Failed to force queue item to completed");

    let blocked_update = ctx
        .put(
            &format!("/api/v1/queues/{}/items/{}", queue_ref, item_id),
            json!({"priority": 5}),
            Some(&token),
        )
        .await
        .expect("Failed to call blocked queue item update");
    assert_eq!(blocked_update.status(), StatusCode::CONFLICT);

    let blocked_delete = ctx
        .delete(
            &format!("/api/v1/queues/{}/items/{}", queue_ref, item_id),
            Some(&token),
        )
        .await
        .expect("Failed to call blocked queue item delete");
    assert_eq!(blocked_delete.status(), StatusCode::CONFLICT);

    let delete_queue = ctx
        .delete(&format!("/api/v1/queues/{}", queue_ref), Some(&token))
        .await
        .expect("Failed to delete adhoc queue");
    assert_eq!(delete_queue.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_pack_scoped_queue_permissions_cover_definitions_and_items() {
    let ctx = TestContext::new()
        .await
        .expect("Failed to create test context");

    let (allowed_pack, allowed_action) =
        create_pack_with_action(&ctx, "python_example", "python_example.dispatch_queue")
            .await
            .expect("Failed to create allowed queue fixture");
    let (blocked_pack, blocked_action) =
        create_pack_with_action(&ctx, "other_pack", "other_pack.dispatch_queue")
            .await
            .expect("Failed to create blocked queue fixture");

    let blocked_queue = WorkQueueRepository::create(
        &ctx.pool,
        CreateWorkQueueInput {
            r#ref: "other_pack.blocked_queue".to_string(),
            pack: Some(blocked_pack.id),
            pack_ref: Some(blocked_pack.r#ref.clone()),
            is_adhoc: false,
            label: "Blocked Queue".to_string(),
            description: None,
            enabled: true,
            accepting_new_items: true,
            dispatch_action: Some(blocked_action.id),
            dispatch_action_ref: blocked_action.r#ref.clone(),
            default_priority: 0,
            allow_pending_update: true,
            update_strategy: WorkQueueUpdateStrategy::Replace,
            batch_mode: WorkQueueBatchMode::Single,
            item_schema: json!({
                "item": { "type": "object", "required": true }
            }),
            action_params: json!({
                "item": "{{ item }}"
            }),
            config: json!({}),
        },
    )
    .await
    .expect("Failed to create blocked queue");

    let token = register_scoped_user(
        &ctx,
        &format!("queue_scoped_{}", uuid::Uuid::new_v4().simple()),
        json!([
            {
                "resource": "queues",
                "actions": ["read", "create", "update", "delete"],
                "constraints": {
                    "pack_refs": [allowed_pack.r#ref]
                }
            }
        ]),
    )
    .await
    .expect("Failed to register queue scoped user");

    let create_allowed = ctx
        .post(
            "/api/v1/queues",
            json!({
                "ref": "python_example.scoped_queue",
                "pack_ref": allowed_pack.r#ref,
                "label": "Scoped Queue",
                "dispatch_action_ref": allowed_action.r#ref,
                "allow_pending_update": true,
                "update_strategy": "replace"
            }),
            Some(&token),
        )
        .await
        .expect("Failed to create allowed scoped queue");
    assert_eq!(create_allowed.status(), StatusCode::CREATED);

    let create_blocked = ctx
        .post(
            "/api/v1/queues",
            json!({
                "ref": "other_pack.denied_queue",
                "pack_ref": blocked_pack.r#ref,
                "label": "Denied Queue",
                "dispatch_action_ref": blocked_action.r#ref
            }),
            Some(&token),
        )
        .await
        .expect("Failed to create blocked scoped queue");
    assert_eq!(create_blocked.status(), StatusCode::FORBIDDEN);

    let list_allowed = ctx
        .get(
            &format!("/api/v1/packs/{}/queues", allowed_pack.r#ref),
            Some(&token),
        )
        .await
        .expect("Failed to list allowed pack queues");
    assert_eq!(list_allowed.status(), StatusCode::OK);
    let list_allowed_body: serde_json::Value = list_allowed
        .json()
        .await
        .expect("Invalid allowed queue list");
    assert!(list_allowed_body["data"]
        .as_array()
        .expect("Expected queue list")
        .iter()
        .any(|queue| queue["ref"] == "python_example.scoped_queue"));

    let get_allowed = ctx
        .get("/api/v1/queues/python_example.scoped_queue", Some(&token))
        .await
        .expect("Failed to get allowed queue");
    assert_eq!(get_allowed.status(), StatusCode::OK);

    let get_blocked = ctx
        .get(
            &format!("/api/v1/queues/{}", blocked_queue.r#ref),
            Some(&token),
        )
        .await
        .expect("Failed to get blocked queue");
    assert_eq!(get_blocked.status(), StatusCode::NOT_FOUND);

    let enqueue_allowed = ctx
        .post(
            "/api/v1/queues/python_example.scoped_queue/items",
            json!({
                "item_key": "job-1",
                "payload": {"hello": "world"}
            }),
            Some(&token),
        )
        .await
        .expect("Failed to enqueue allowed queue item");
    assert_eq!(enqueue_allowed.status(), StatusCode::CREATED);
    let enqueue_allowed_body: serde_json::Value = enqueue_allowed
        .json()
        .await
        .expect("Invalid allowed enqueue body");
    let item_id = enqueue_allowed_body["data"]["id"]
        .as_i64()
        .expect("Missing item id");

    let update_allowed = ctx
        .put(
            &format!(
                "/api/v1/queues/python_example.scoped_queue/items/{}",
                item_id
            ),
            json!({"priority": 11}),
            Some(&token),
        )
        .await
        .expect("Failed to update allowed queue item");
    assert_eq!(update_allowed.status(), StatusCode::OK);

    let delete_allowed = ctx
        .delete(
            &format!(
                "/api/v1/queues/python_example.scoped_queue/items/{}",
                item_id
            ),
            Some(&token),
        )
        .await
        .expect("Failed to delete allowed queue item");
    assert_eq!(delete_allowed.status(), StatusCode::OK);

    let enqueue_blocked = ctx
        .post(
            &format!("/api/v1/queues/{}/items", blocked_queue.r#ref),
            json!({"payload": {"blocked": true}}),
            Some(&token),
        )
        .await
        .expect("Failed to enqueue blocked queue item");
    assert_eq!(enqueue_blocked.status(), StatusCode::FORBIDDEN);
}
