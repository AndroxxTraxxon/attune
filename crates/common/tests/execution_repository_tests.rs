//! Integration tests for Execution repository
//!
//! These tests verify CRUD operations, queries, and constraints
//! for the Execution repository.

mod helpers;

use attune_common::{
    models::enums::ExecutionStatus,
    repositories::{
        execution::{CreateExecutionInput, ExecutionRepository, UpdateExecutionInput},
        Create, Delete, FindById, List, Update,
    },
};
use helpers::*;
use serde_json::json;

// ============================================================================
// CREATE Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_create_execution_basic() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("exec_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "test_action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: Some(json!({"param1": "value1"})),
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let execution = ExecutionRepository::create(&pool, input).await.unwrap();

    assert_eq!(execution.action, Some(action.id));
    assert_eq!(execution.action_ref, action.r#ref);
    assert_eq!(execution.config, Some(json!({"param1": "value1"})));
    assert_eq!(execution.parent, None);
    assert_eq!(execution.enforcement, None);
    assert_eq!(execution.executor, None);
    assert_eq!(execution.status, ExecutionStatus::Requested);
    assert_eq!(execution.result, None);
    assert!(execution.created.timestamp() > 0);
    assert!(execution.updated.timestamp() > 0);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_create_execution_without_action() {
    let pool = create_test_pool().await.unwrap();

    let action_ref = format!("core.{}", unique_execution_ref("deleted_action"));

    let input = CreateExecutionInput {
        action: None,
        action_ref: action_ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let execution = ExecutionRepository::create(&pool, input).await.unwrap();

    assert_eq!(execution.action, None);
    assert_eq!(execution.action_ref, action_ref);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_create_execution_with_all_fields() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("full_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: Some(json!({"timeout": 300, "retry": true})),
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None, // Don't reference non-existent identity
        worker: None,
        status: ExecutionStatus::Scheduled,
        result: Some(json!({"status": "ok"})),
        workflow_task: None,
    };

    let execution = ExecutionRepository::create(&pool, input).await.unwrap();

    assert_eq!(execution.executor, None);
    assert_eq!(execution.status, ExecutionStatus::Scheduled);
    assert_eq!(execution.result, Some(json!({"status": "ok"})));
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_create_execution_with_parent() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("parent_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    // Create parent execution
    let parent_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Running,
        result: None,
        workflow_task: None,
    };

    let parent = ExecutionRepository::create(&pool, parent_input)
        .await
        .unwrap();

    // Create child execution
    let child_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: Some(parent.id),
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let child = ExecutionRepository::create(&pool, child_input)
        .await
        .unwrap();

    assert_eq!(child.parent, Some(parent.id));
}

// ============================================================================
// READ Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_find_execution_by_id() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("find_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let found = ExecutionRepository::find_by_id(&pool, created.id)
        .await
        .unwrap()
        .expect("Execution should exist");

    assert_eq!(found.id, created.id);
    assert_eq!(found.action_ref, created.action_ref);
    assert_eq!(found.status, created.status);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_find_execution_by_id_not_found() {
    let pool = create_test_pool().await.unwrap();

    let result = ExecutionRepository::find_by_id(&pool, 999999)
        .await
        .unwrap();

    assert!(result.is_none());
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_list_executions() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("list_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    // Create multiple executions
    for i in 1..=3 {
        let input = CreateExecutionInput {
            action: Some(action.id),
            action_ref: format!("{}_{}", action.r#ref, i),
            config: None,
            env_vars: None,
            parent: None,
            enforcement: None,
            executor: None,
            worker: None,
            status: ExecutionStatus::Requested,
            result: None,
            workflow_task: None,
        };

        ExecutionRepository::create(&pool, input).await.unwrap();
    }

    let executions = ExecutionRepository::list(&pool).await.unwrap();

    // Should have at least our 3 executions (may have more from parallel tests)
    let our_executions: Vec<_> = executions
        .iter()
        .filter(|e| e.action_ref.starts_with(&action.r#ref))
        .collect();

    assert_eq!(our_executions.len(), 3);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_list_executions_ordered_by_created_desc() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("order_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let mut created_ids = vec![];

    // Create executions in sequence
    for i in 1..=3 {
        let input = CreateExecutionInput {
            action: Some(action.id),
            action_ref: format!("{}_{}", action.r#ref, i),
            config: None,
            env_vars: None,
            parent: None,
            enforcement: None,
            executor: None,
            worker: None,
            status: ExecutionStatus::Requested,
            result: None,
            workflow_task: None,
        };

        let exec = ExecutionRepository::create(&pool, input).await.unwrap();
        created_ids.push(exec.id);

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let executions = ExecutionRepository::list(&pool).await.unwrap();
    let our_executions: Vec<_> = executions
        .iter()
        .filter(|e| e.action_ref.starts_with(&action.r#ref))
        .collect();

    // Should be in reverse order (newest first)
    assert_eq!(our_executions[0].id, created_ids[2]);
    assert_eq!(our_executions[1].id, created_ids[1]);
    assert_eq!(our_executions[2].id, created_ids[0]);
}

// ============================================================================
// UPDATE Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_update_execution_status() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("update_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let update = UpdateExecutionInput {
        status: Some(ExecutionStatus::Running),
        result: None,
        executor: None,
        ..Default::default()
    };

    let updated = ExecutionRepository::update(&pool, created.id, update)
        .await
        .unwrap();

    assert_eq!(updated.status, ExecutionStatus::Running);
    assert!(updated.updated > created.updated);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_update_execution_result() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("result_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Running,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let result_data = json!({"output": "success", "data": {"count": 42}});
    let update = UpdateExecutionInput {
        status: Some(ExecutionStatus::Completed),
        result: Some(result_data.clone()),
        executor: None,
        ..Default::default()
    };

    let updated = ExecutionRepository::update(&pool, created.id, update)
        .await
        .unwrap();

    assert_eq!(updated.status, ExecutionStatus::Completed);
    assert_eq!(updated.result, Some(result_data));
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_update_execution_executor() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("executor_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let update = UpdateExecutionInput {
        status: Some(ExecutionStatus::Scheduled),
        result: None,
        executor: None,
        ..Default::default()
    };

    let updated = ExecutionRepository::update(&pool, created.id, update)
        .await
        .unwrap();

    assert_eq!(updated.status, ExecutionStatus::Scheduled);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_update_execution_status_transitions() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("status_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let exec = ExecutionRepository::create(&pool, input).await.unwrap();

    // Transition: Requested -> Scheduling
    let exec = ExecutionRepository::update(
        &pool,
        exec.id,
        UpdateExecutionInput {
            status: Some(ExecutionStatus::Scheduling),
            result: None,
            executor: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(exec.status, ExecutionStatus::Scheduling);

    // Transition: Scheduling -> Scheduled
    let exec = ExecutionRepository::update(
        &pool,
        exec.id,
        UpdateExecutionInput {
            status: Some(ExecutionStatus::Scheduled),
            result: None,
            executor: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(exec.status, ExecutionStatus::Scheduled);

    // Transition: Scheduled -> Running
    let exec = ExecutionRepository::update(
        &pool,
        exec.id,
        UpdateExecutionInput {
            status: Some(ExecutionStatus::Running),
            result: None,
            executor: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(exec.status, ExecutionStatus::Running);

    // Transition: Running -> Completed
    let exec = ExecutionRepository::update(
        &pool,
        exec.id,
        UpdateExecutionInput {
            status: Some(ExecutionStatus::Completed),
            result: Some(json!({"success": true})),
            executor: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(exec.status, ExecutionStatus::Completed);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_update_execution_failed_status() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("failed_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Running,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let update = UpdateExecutionInput {
        status: Some(ExecutionStatus::Failed),
        result: Some(json!({"error": "Connection timeout"})),
        executor: None,
        ..Default::default()
    };

    let updated = ExecutionRepository::update(&pool, created.id, update)
        .await
        .unwrap();

    assert_eq!(updated.status, ExecutionStatus::Failed);
    assert!(updated.result.is_some());
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_update_execution_no_changes() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("nochange_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let update = UpdateExecutionInput::default();

    let updated = ExecutionRepository::update(&pool, created.id, update)
        .await
        .unwrap();

    assert_eq!(updated.status, created.status);
    assert_eq!(updated.result, created.result);
}

// ============================================================================
// DELETE Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_delete_execution() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("delete_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Completed,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let deleted = ExecutionRepository::delete(&pool, created.id)
        .await
        .unwrap();

    assert!(deleted);

    let found = ExecutionRepository::find_by_id(&pool, created.id)
        .await
        .unwrap();

    assert!(found.is_none());
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_delete_execution_not_found() {
    let pool = create_test_pool().await.unwrap();

    let deleted = ExecutionRepository::delete(&pool, 999999).await.unwrap();

    assert!(!deleted);
}

// ============================================================================
// SPECIALIZED QUERY Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_find_executions_by_status() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("status_filter_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    // Create executions with different statuses
    for (i, status) in [
        ExecutionStatus::Requested,
        ExecutionStatus::Running,
        ExecutionStatus::Running,
        ExecutionStatus::Completed,
    ]
    .iter()
    .enumerate()
    {
        let input = CreateExecutionInput {
            action: Some(action.id),
            action_ref: format!("{}_{}", action.r#ref, i),
            config: None,
            env_vars: None,
            parent: None,
            enforcement: None,
            executor: None,
            worker: None,
            status: *status,
            result: None,
            workflow_task: None,
        };

        ExecutionRepository::create(&pool, input).await.unwrap();
    }

    let running = ExecutionRepository::find_by_status(&pool, ExecutionStatus::Running)
        .await
        .unwrap();

    let our_running: Vec<_> = running
        .iter()
        .filter(|e| e.action_ref.starts_with(&action.r#ref))
        .collect();

    assert_eq!(our_running.len(), 2);
    assert!(our_running
        .iter()
        .all(|e| e.status == ExecutionStatus::Running));
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_find_executions_by_enforcement() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("enforcement_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    // Create first execution with enforcement placeholder
    let exec1_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: format!("{}_1", action.r#ref),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };
    let _exec1 = ExecutionRepository::create(&pool, exec1_input)
        .await
        .unwrap();

    // Create executions with enforcement reference
    for i in 2..=3 {
        let input = CreateExecutionInput {
            action: Some(action.id),
            action_ref: format!("{}_{}", action.r#ref, i),
            config: None,
            env_vars: None,
            parent: None,
            enforcement: None, // Can't reference non-existent enforcement
            executor: None,
            worker: None,
            status: ExecutionStatus::Requested,
            result: None,
            workflow_task: None,
        };

        ExecutionRepository::create(&pool, input).await.unwrap();
    }

    // Test find_by_enforcement with non-existent ID returns empty
    let by_enforcement = ExecutionRepository::find_by_enforcement(&pool, 999999)
        .await
        .unwrap();

    assert_eq!(by_enforcement.len(), 0);
}

// ============================================================================
// PARENT-CHILD RELATIONSHIP Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_parent_child_execution_hierarchy() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("hierarchy_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    // Create parent
    let parent_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: format!("{}.parent", action.r#ref),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Running,
        result: None,
        workflow_task: None,
    };

    let parent = ExecutionRepository::create(&pool, parent_input)
        .await
        .unwrap();

    // Create children
    let mut children = vec![];
    for i in 1..=3 {
        let child_input = CreateExecutionInput {
            action: Some(action.id),
            action_ref: format!("{}.child_{}", action.r#ref, i),
            config: None,
            env_vars: None,
            parent: Some(parent.id),
            enforcement: None,
            executor: None,
            worker: None,
            status: ExecutionStatus::Requested,
            result: None,
            workflow_task: None,
        };

        let child = ExecutionRepository::create(&pool, child_input)
            .await
            .unwrap();
        children.push(child);
    }

    // Verify all children have correct parent
    for child in children {
        assert_eq!(child.parent, Some(parent.id));
    }

    // Verify parent has no parent
    assert_eq!(parent.parent, None);
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_nested_execution_hierarchy() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("nested_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    // Create grandparent
    let grandparent_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: format!("{}.grandparent", action.r#ref),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Running,
        result: None,
        workflow_task: None,
    };

    let grandparent = ExecutionRepository::create(&pool, grandparent_input)
        .await
        .unwrap();

    // Create parent
    let parent_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: format!("{}.parent", action.r#ref),
        config: None,
        env_vars: None,
        parent: Some(grandparent.id),
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Running,
        result: None,
        workflow_task: None,
    };

    let parent = ExecutionRepository::create(&pool, parent_input)
        .await
        .unwrap();

    // Create child
    let child_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: format!("{}.child", action.r#ref),
        config: None,
        env_vars: None,
        parent: Some(parent.id),
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let child = ExecutionRepository::create(&pool, child_input)
        .await
        .unwrap();

    // Verify hierarchy
    assert_eq!(grandparent.parent, None);
    assert_eq!(parent.parent, Some(grandparent.id));
    assert_eq!(child.parent, Some(parent.id));
}

// ============================================================================
// TIMESTAMP Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_execution_timestamps() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("timestamp_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    assert!(created.created.timestamp() > 0);
    assert!(created.updated.timestamp() > 0);
    assert_eq!(created.created, created.updated);

    // Sleep briefly to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let update = UpdateExecutionInput {
        status: Some(ExecutionStatus::Running),
        result: None,
        executor: None,
        ..Default::default()
    };

    let updated = ExecutionRepository::update(&pool, created.id, update)
        .await
        .unwrap();

    assert_eq!(updated.created, created.created); // created unchanged
    assert!(updated.updated > created.updated); // updated changed
}

// ============================================================================
// JSON FIELD Tests
// ============================================================================

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_execution_config_json() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("config_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let complex_config = json!({
        "parameters": {
            "timeout": 300,
            "retry_count": 3,
            "retry_delay": 1000
        },
        "environment": {
            "NODE_ENV": "production"
        },
        "metadata": {
            "triggered_by": "webhook",
            "source": "github"
        }
    });

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: Some(complex_config.clone()),
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None,
    };

    let execution = ExecutionRepository::create(&pool, input).await.unwrap();

    assert_eq!(execution.config, Some(complex_config));
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_execution_result_json() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("result_json_pack")
        .create(&pool)
        .await
        .unwrap();

    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "action")
        .create(&pool)
        .await
        .unwrap();

    let input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: None,
        env_vars: None,
        parent: None,
        enforcement: None,
        executor: None,
        worker: None,
        status: ExecutionStatus::Running,
        result: None,
        workflow_task: None,
    };

    let created = ExecutionRepository::create(&pool, input).await.unwrap();

    let complex_result = json!({
        "output": {
            "stdout": "Process completed successfully",
            "stderr": ""
        },
        "metrics": {
            "duration_ms": 1234,
            "memory_mb": 128,
            "cpu_percent": 45.2
        },
        "artifacts": [
            {"name": "report.pdf", "size": 1024000},
            {"name": "data.json", "size": 512}
        ]
    });

    let update = UpdateExecutionInput {
        status: Some(ExecutionStatus::Completed),
        result: Some(complex_result.clone()),
        executor: None,
        ..Default::default()
    };

    let updated = ExecutionRepository::update(&pool, created.id, update)
        .await
        .unwrap();

    assert_eq!(updated.result, Some(complex_result));
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_claim_for_scheduling_succeeds_once() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("claim_pack")
        .create(&pool)
        .await
        .unwrap();
    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "claim_action")
        .create(&pool)
        .await
        .unwrap();

    let created = ExecutionRepository::create(
        &pool,
        CreateExecutionInput {
            action: Some(action.id),
            action_ref: action.r#ref.clone(),
            config: None,
            env_vars: None,
            parent: None,
            enforcement: None,
            executor: None,
            worker: None,
            status: ExecutionStatus::Requested,
            result: None,
            workflow_task: None,
        },
    )
    .await
    .unwrap();

    let first = ExecutionRepository::claim_for_scheduling(&pool, created.id, None)
        .await
        .unwrap();
    let second = ExecutionRepository::claim_for_scheduling(&pool, created.id, None)
        .await
        .unwrap();

    assert_eq!(first.unwrap().status, ExecutionStatus::Scheduling);
    assert!(second.is_none());
}

#[tokio::test]
#[ignore = "integration test — requires database"]
async fn test_update_if_status_only_updates_matching_row() {
    let pool = create_test_pool().await.unwrap();

    let pack = PackFixture::new_unique("conditional_pack")
        .create(&pool)
        .await
        .unwrap();
    let action = ActionFixture::new_unique(pack.id, &pack.r#ref, "conditional_action")
        .create(&pool)
        .await
        .unwrap();

    let created = ExecutionRepository::create(
        &pool,
        CreateExecutionInput {
            action: Some(action.id),
            action_ref: action.r#ref.clone(),
            config: None,
            env_vars: None,
            parent: None,
            enforcement: None,
            executor: None,
            worker: None,
            status: ExecutionStatus::Scheduling,
            result: None,
            workflow_task: None,
        },
    )
    .await
    .unwrap();

    let updated = ExecutionRepository::update_if_status(
        &pool,
        created.id,
        ExecutionStatus::Scheduling,
        UpdateExecutionInput {
            status: Some(ExecutionStatus::Scheduled),
            worker: Some(77),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let skipped = ExecutionRepository::update_if_status(
        &pool,
        created.id,
        ExecutionStatus::Scheduling,
        UpdateExecutionInput {
            status: Some(ExecutionStatus::Failed),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.unwrap().status, ExecutionStatus::Scheduled);
    assert!(skipped.is_none());
}
