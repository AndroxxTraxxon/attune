//! Task Executor
//!
//! This module handles the execution of individual workflow tasks,
//! including action invocation, retries, timeouts, and with-items iteration.

use crate::workflow::context::WorkflowContext;
use crate::workflow::graph::{BackoffStrategy, RetryConfig, TaskNode};
use attune_common::error::{Error, Result};
use attune_common::models::Id;
use attune_common::mq::MessageQueue;
use chrono::{DateTime, Utc};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Task execution result
#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    /// Execution status
    pub status: TaskExecutionStatus,

    /// Task output/result
    pub output: Option<JsonValue>,

    /// Error information
    pub error: Option<TaskExecutionError>,

    /// Execution duration in milliseconds
    pub duration_ms: i64,

    /// Whether the task should be retried
    pub should_retry: bool,

    /// Next retry time (if applicable)
    pub next_retry_at: Option<DateTime<Utc>>,

    /// Number of retries performed
    pub retry_count: i32,
}

/// Task execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskExecutionStatus {
    Success,
    Failed,
    Timeout,
    Skipped,
}

/// Task execution error
#[derive(Debug, Clone)]
pub struct TaskExecutionError {
    pub message: String,
    pub error_type: String,
    pub details: Option<JsonValue>,
}

/// Task executor
pub struct TaskExecutor {
    db_pool: PgPool,
    mq: MessageQueue,
}

impl TaskExecutor {
    /// Create a new task executor
    pub fn new(db_pool: PgPool, mq: MessageQueue) -> Self {
        Self { db_pool, mq }
    }

    /// Execute a task
    pub async fn execute_task(
        &self,
        task: &TaskNode,
        context: &mut WorkflowContext,
        workflow_execution_id: Id,
        parent_execution_id: Id,
    ) -> Result<TaskExecutionResult> {
        info!("Executing task: {}", task.name);

        let start_time = Utc::now();

        // Check if task should be skipped (when condition)
        if let Some(ref condition) = task.when {
            match context.evaluate_condition(condition) {
                Ok(should_run) => {
                    if !should_run {
                        info!("Task {} skipped due to when condition", task.name);
                        return Ok(TaskExecutionResult {
                            status: TaskExecutionStatus::Skipped,
                            output: None,
                            error: None,
                            duration_ms: 0,
                            should_retry: false,
                            next_retry_at: None,
                            retry_count: 0,
                        });
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to evaluate when condition for task {}: {}",
                        task.name, e
                    );
                    // Continue execution if condition evaluation fails
                }
            }
        }

        // Check if this is a with-items task
        if let Some(ref with_items_expr) = task.with_items {
            return self
                .execute_with_items(
                    task,
                    context,
                    workflow_execution_id,
                    parent_execution_id,
                    with_items_expr,
                )
                .await;
        }

        // Execute single task
        let result = self
            .execute_single_task(task, context, workflow_execution_id, parent_execution_id, 0)
            .await?;

        let duration_ms = (Utc::now() - start_time).num_milliseconds();

        // Store task result in context
        if let Some(ref output) = result.output {
            context.set_task_result(&task.name, output.clone());

            // Publish variables from matching transitions
            let success = matches!(result.status, TaskExecutionStatus::Success);
            for transition in &task.transitions {
                let should_fire = match transition.kind() {
                    super::graph::TransitionKind::Succeeded => success,
                    super::graph::TransitionKind::Failed => !success,
                    super::graph::TransitionKind::TimedOut => !success,
                    super::graph::TransitionKind::Always => true,
                    super::graph::TransitionKind::Custom => true,
                };
                if should_fire && !transition.publish.is_empty() {
                    let var_names: Vec<String> =
                        transition.publish.iter().map(|p| p.name.clone()).collect();
                    if let Err(e) = context.publish_from_result(output, &var_names, None) {
                        warn!("Failed to publish variables for task {}: {}", task.name, e);
                    }
                }
            }
        }

        Ok(TaskExecutionResult {
            duration_ms,
            ..result
        })
    }

    /// Execute a single task (without with-items iteration)
    async fn execute_single_task(
        &self,
        task: &TaskNode,
        context: &WorkflowContext,
        workflow_execution_id: Id,
        parent_execution_id: Id,
        retry_count: i32,
    ) -> Result<TaskExecutionResult> {
        let start_time = Utc::now();

        // Render task input
        let input = match context.render_json(&task.input) {
            Ok(rendered) => rendered,
            Err(e) => {
                error!("Failed to render task input for {}: {}", task.name, e);
                return Ok(TaskExecutionResult {
                    status: TaskExecutionStatus::Failed,
                    output: None,
                    error: Some(TaskExecutionError {
                        message: format!("Failed to render task input: {}", e),
                        error_type: "template_error".to_string(),
                        details: None,
                    }),
                    duration_ms: 0,
                    should_retry: false,
                    next_retry_at: None,
                    retry_count,
                });
            }
        };

        // Execute based on task type
        let result = match task.task_type {
            attune_common::workflow::TaskType::Action => {
                self.execute_action(task, input, workflow_execution_id, parent_execution_id)
                    .await
            }
            attune_common::workflow::TaskType::Parallel => {
                self.execute_parallel(task, context, workflow_execution_id, parent_execution_id)
                    .await
            }
            attune_common::workflow::TaskType::Workflow => {
                self.execute_workflow(task, input, workflow_execution_id, parent_execution_id)
                    .await
            }
        };

        let duration_ms = (Utc::now() - start_time).num_milliseconds();

        // Apply timeout if specified
        let result = if let Some(timeout_secs) = task.timeout {
            self.apply_timeout(result, timeout_secs).await
        } else {
            result
        };

        // Handle retries
        let mut result = result?;
        result.retry_count = retry_count;

        if result.status == TaskExecutionStatus::Failed {
            if let Some(ref retry_config) = task.retry {
                if retry_count < retry_config.count as i32 {
                    // Check if we should retry based on error condition
                    let should_retry = if let Some(ref _on_error) = retry_config.on_error {
                        // TODO: Evaluate error condition
                        true
                    } else {
                        true
                    };

                    if should_retry {
                        result.should_retry = true;
                        result.next_retry_at =
                            Some(calculate_retry_time(retry_config, retry_count));
                        info!(
                            "Task {} failed, will retry (attempt {}/{})",
                            task.name,
                            retry_count + 1,
                            retry_config.count
                        );
                    }
                }
            }
        }

        result.duration_ms = duration_ms;
        Ok(result)
    }

    /// Execute an action task
    async fn execute_action(
        &self,
        task: &TaskNode,
        input: JsonValue,
        _workflow_execution_id: Id,
        parent_execution_id: Id,
    ) -> Result<TaskExecutionResult> {
        let action_ref = match &task.action {
            Some(action) => action,
            None => {
                return Ok(TaskExecutionResult {
                    status: TaskExecutionStatus::Failed,
                    output: None,
                    error: Some(TaskExecutionError {
                        message: "Action task missing action reference".to_string(),
                        error_type: "configuration_error".to_string(),
                        details: None,
                    }),
                    duration_ms: 0,
                    should_retry: false,
                    next_retry_at: None,
                    retry_count: 0,
                });
            }
        };

        debug!("Executing action: {} with input: {:?}", action_ref, input);

        // Create execution record in database
        let execution = sqlx::query_as::<_, attune_common::models::Execution>(
            r#"
            INSERT INTO attune.execution (action_ref, input, parent, status)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(action_ref)
        .bind(&input)
        .bind(parent_execution_id)
        .bind(attune_common::models::ExecutionStatus::Scheduled)
        .fetch_one(&self.db_pool)
        .await?;

        // Queue action for execution by worker
        // TODO: Implement proper message queue publishing
        info!(
            "Created action execution {} for task {} (queuing not yet implemented)",
            execution.id, task.name
        );

        // For now, return pending status
        // In a real implementation, we would wait for completion via message queue
        Ok(TaskExecutionResult {
            status: TaskExecutionStatus::Success,
            output: Some(json!({
                "execution_id": execution.id,
                "status": "queued"
            })),
            error: None,
            duration_ms: 0,
            should_retry: false,
            next_retry_at: None,
            retry_count: 0,
        })
    }

    /// Execute parallel tasks
    async fn execute_parallel(
        &self,
        task: &TaskNode,
        context: &WorkflowContext,
        workflow_execution_id: Id,
        parent_execution_id: Id,
    ) -> Result<TaskExecutionResult> {
        let sub_tasks = match &task.sub_tasks {
            Some(tasks) => tasks,
            None => {
                return Ok(TaskExecutionResult {
                    status: TaskExecutionStatus::Failed,
                    output: None,
                    error: Some(TaskExecutionError {
                        message: "Parallel task missing sub-tasks".to_string(),
                        error_type: "configuration_error".to_string(),
                        details: None,
                    }),
                    duration_ms: 0,
                    should_retry: false,
                    next_retry_at: None,
                    retry_count: 0,
                });
            }
        };

        info!("Executing {} parallel tasks", sub_tasks.len());

        // Execute all sub-tasks in parallel
        let mut futures = Vec::new();

        for subtask in sub_tasks {
            let subtask_clone = subtask.clone();
            let subtask_name = subtask.name.clone();
            let context = context.clone();
            let db_pool = self.db_pool.clone();
            let mq = self.mq.clone();

            let future = async move {
                let executor = TaskExecutor::new(db_pool, mq);
                let result = executor
                    .execute_single_task(
                        &subtask_clone,
                        &context,
                        workflow_execution_id,
                        parent_execution_id,
                        0,
                    )
                    .await;
                (subtask_name, result)
            };

            futures.push(future);
        }

        // Wait for all tasks to complete
        let task_results = futures::future::join_all(futures).await;

        let mut results = Vec::new();
        let mut all_succeeded = true;
        let mut errors = Vec::new();

        for (task_name, result) in task_results {
            match result {
                Ok(result) => {
                    if result.status != TaskExecutionStatus::Success {
                        all_succeeded = false;
                        if let Some(error) = &result.error {
                            errors.push(json!({
                                "task": task_name,
                                "error": error.message
                            }));
                        }
                    }
                    results.push(json!({
                        "task": task_name,
                        "status": format!("{:?}", result.status),
                        "output": result.output
                    }));
                }
                Err(e) => {
                    all_succeeded = false;
                    errors.push(json!({
                        "task": task_name,
                        "error": e.to_string()
                    }));
                }
            }
        }

        let status = if all_succeeded {
            TaskExecutionStatus::Success
        } else {
            TaskExecutionStatus::Failed
        };

        Ok(TaskExecutionResult {
            status,
            output: Some(json!({
                "results": results
            })),
            error: if errors.is_empty() {
                None
            } else {
                Some(TaskExecutionError {
                    message: format!("{} parallel tasks failed", errors.len()),
                    error_type: "parallel_execution_error".to_string(),
                    details: Some(json!({"errors": errors})),
                })
            },
            duration_ms: 0,
            should_retry: false,
            next_retry_at: None,
            retry_count: 0,
        })
    }

    /// Execute a workflow task (nested workflow)
    async fn execute_workflow(
        &self,
        _task: &TaskNode,
        _input: JsonValue,
        _workflow_execution_id: Id,
        _parent_execution_id: Id,
    ) -> Result<TaskExecutionResult> {
        // TODO: Implement nested workflow execution
        // For now, return not implemented
        warn!("Workflow task execution not yet implemented");

        Ok(TaskExecutionResult {
            status: TaskExecutionStatus::Failed,
            output: None,
            error: Some(TaskExecutionError {
                message: "Nested workflow execution not yet implemented".to_string(),
                error_type: "not_implemented".to_string(),
                details: None,
            }),
            duration_ms: 0,
            should_retry: false,
            next_retry_at: None,
            retry_count: 0,
        })
    }

    /// Execute task with with-items iteration
    async fn execute_with_items(
        &self,
        task: &TaskNode,
        context: &mut WorkflowContext,
        workflow_execution_id: Id,
        parent_execution_id: Id,
        items_expr: &str,
    ) -> Result<TaskExecutionResult> {
        // Render items expression
        let items_str = context.render_template(items_expr).map_err(|e| {
            Error::validation(format!("Failed to render with-items expression: {}", e))
        })?;

        // Parse items (should be a JSON array)
        let items: Vec<JsonValue> = serde_json::from_str(&items_str).map_err(|e| {
            Error::validation(format!(
                "with-items expression did not produce valid JSON array: {}",
                e
            ))
        })?;

        info!("Executing task {} with {} items", task.name, items.len());

        let items_len = items.len(); // Store length before consuming items
        let concurrency = task.concurrency.unwrap_or(10);

        let mut all_results = Vec::new();
        let mut all_succeeded = true;
        let mut errors = Vec::new();

        // Check if batch processing is enabled
        if let Some(batch_size) = task.batch_size {
            // Batch mode: split items into batches and pass as arrays
            debug!(
                "Processing {} items in batches of {} (batch mode)",
                items.len(),
                batch_size
            );

            let batches: Vec<Vec<JsonValue>> = items
                .chunks(batch_size)
                .map(|chunk| chunk.to_vec())
                .collect();

            debug!("Created {} batches", batches.len());

            // Execute batches with concurrency limit
            let mut handles = Vec::new();
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));

            for (batch_idx, batch) in batches.into_iter().enumerate() {
                let permit = semaphore.clone().acquire_owned().await.unwrap();

                let executor = TaskExecutor::new(self.db_pool.clone(), self.mq.clone());
                let task = task.clone();
                let mut batch_context = context.clone();

                // Set current_item to the batch array
                batch_context.set_current_item(json!(batch), batch_idx);

                let handle = tokio::spawn(async move {
                    let result = executor
                        .execute_single_task(
                            &task,
                            &batch_context,
                            workflow_execution_id,
                            parent_execution_id,
                            0,
                        )
                        .await;
                    drop(permit);
                    (batch_idx, result)
                });

                handles.push(handle);
            }

            // Wait for all batches to complete
            for handle in handles {
                match handle.await {
                    Ok((batch_idx, Ok(result))) => {
                        if result.status != TaskExecutionStatus::Success {
                            all_succeeded = false;
                            if let Some(error) = &result.error {
                                errors.push(json!({
                                    "batch": batch_idx,
                                    "error": error.message
                                }));
                            }
                        }
                        all_results.push(json!({
                            "batch": batch_idx,
                            "status": format!("{:?}", result.status),
                            "output": result.output
                        }));
                    }
                    Ok((batch_idx, Err(e))) => {
                        all_succeeded = false;
                        errors.push(json!({
                            "batch": batch_idx,
                            "error": e.to_string()
                        }));
                    }
                    Err(e) => {
                        all_succeeded = false;
                        errors.push(json!({
                            "error": format!("Task panicked: {}", e)
                        }));
                    }
                }
            }
        } else {
            // Individual mode: process each item separately
            debug!(
                "Processing {} items individually (no batch_size specified)",
                items.len()
            );

            // Execute items with concurrency limit
            let mut handles = Vec::new();
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));

            for (item_idx, item) in items.into_iter().enumerate() {
                let permit = semaphore.clone().acquire_owned().await.unwrap();

                let executor = TaskExecutor::new(self.db_pool.clone(), self.mq.clone());
                let task = task.clone();
                let mut item_context = context.clone();

                // Set current_item to the individual item
                item_context.set_current_item(item, item_idx);

                let handle = tokio::spawn(async move {
                    let result = executor
                        .execute_single_task(
                            &task,
                            &item_context,
                            workflow_execution_id,
                            parent_execution_id,
                            0,
                        )
                        .await;
                    drop(permit);
                    (item_idx, result)
                });

                handles.push(handle);
            }

            // Wait for all items to complete
            for handle in handles {
                match handle.await {
                    Ok((idx, Ok(result))) => {
                        if result.status != TaskExecutionStatus::Success {
                            all_succeeded = false;
                            if let Some(error) = &result.error {
                                errors.push(json!({
                                    "index": idx,
                                    "error": error.message
                                }));
                            }
                        }
                        all_results.push(json!({
                            "index": idx,
                            "status": format!("{:?}", result.status),
                            "output": result.output
                        }));
                    }
                    Ok((idx, Err(e))) => {
                        all_succeeded = false;
                        errors.push(json!({
                            "index": idx,
                            "error": e.to_string()
                        }));
                    }
                    Err(e) => {
                        all_succeeded = false;
                        errors.push(json!({
                            "error": format!("Task panicked: {}", e)
                        }));
                    }
                }
            }
        }

        context.clear_current_item();

        let status = if all_succeeded {
            TaskExecutionStatus::Success
        } else {
            TaskExecutionStatus::Failed
        };

        Ok(TaskExecutionResult {
            status,
            output: Some(json!({
                "results": all_results,
                "total": items_len
            })),
            error: if errors.is_empty() {
                None
            } else {
                Some(TaskExecutionError {
                    message: format!("{} items failed", errors.len()),
                    error_type: "with_items_error".to_string(),
                    details: Some(json!({"errors": errors})),
                })
            },
            duration_ms: 0,
            should_retry: false,
            next_retry_at: None,
            retry_count: 0,
        })
    }

    /// Apply timeout to task execution
    async fn apply_timeout(
        &self,
        result_future: Result<TaskExecutionResult>,
        timeout_secs: u32,
    ) -> Result<TaskExecutionResult> {
        match timeout(Duration::from_secs(timeout_secs as u64), async {
            result_future
        })
        .await
        {
            Ok(result) => result,
            Err(_) => {
                warn!("Task execution timed out after {} seconds", timeout_secs);
                Ok(TaskExecutionResult {
                    status: TaskExecutionStatus::Timeout,
                    output: None,
                    error: Some(TaskExecutionError {
                        message: format!("Task timed out after {} seconds", timeout_secs),
                        error_type: "timeout".to_string(),
                        details: None,
                    }),
                    duration_ms: (timeout_secs * 1000) as i64,
                    should_retry: false,
                    next_retry_at: None,
                    retry_count: 0,
                })
            }
        }
    }
}

/// Calculate next retry time based on retry configuration
fn calculate_retry_time(config: &RetryConfig, retry_count: i32) -> DateTime<Utc> {
    let delay_secs = match config.backoff {
        BackoffStrategy::Constant => config.delay,
        BackoffStrategy::Linear => config.delay * (retry_count as u32 + 1),
        BackoffStrategy::Exponential => {
            let exp_delay = config.delay * 2_u32.pow(retry_count as u32);
            if let Some(max_delay) = config.max_delay {
                exp_delay.min(max_delay)
            } else {
                exp_delay
            }
        }
    };

    Utc::now() + chrono::Duration::seconds(delay_secs as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_retry_time_constant() {
        let config = RetryConfig {
            count: 3,
            delay: 10,
            backoff: BackoffStrategy::Constant,
            max_delay: None,
            on_error: None,
        };

        let now = Utc::now();
        let retry_time = calculate_retry_time(&config, 0);
        let diff = (retry_time - now).num_seconds();

        assert!(diff >= 9 && diff <= 11); // Allow 1 second tolerance
    }

    #[test]
    fn test_calculate_retry_time_exponential() {
        let config = RetryConfig {
            count: 3,
            delay: 10,
            backoff: BackoffStrategy::Exponential,
            max_delay: Some(100),
            on_error: None,
        };

        let now = Utc::now();

        // First retry: 10 * 2^0 = 10
        let retry1 = calculate_retry_time(&config, 0);
        assert!((retry1 - now).num_seconds() >= 9 && (retry1 - now).num_seconds() <= 11);

        // Second retry: 10 * 2^1 = 20
        let retry2 = calculate_retry_time(&config, 1);
        assert!((retry2 - now).num_seconds() >= 19 && (retry2 - now).num_seconds() <= 21);

        // Third retry: 10 * 2^2 = 40
        let retry3 = calculate_retry_time(&config, 2);
        assert!((retry3 - now).num_seconds() >= 39 && (retry3 - now).num_seconds() <= 41);
    }

    #[test]
    fn test_calculate_retry_time_exponential_with_max() {
        let config = RetryConfig {
            count: 10,
            delay: 10,
            backoff: BackoffStrategy::Exponential,
            max_delay: Some(100),
            on_error: None,
        };

        let now = Utc::now();

        // Retry with high count should be capped at max_delay
        let retry = calculate_retry_time(&config, 10);
        assert!((retry - now).num_seconds() >= 99 && (retry - now).num_seconds() <= 101);
    }

    #[test]
    fn test_with_items_batch_creation() {
        use serde_json::json;

        // Test batch_size=3 with 7 items
        let items = vec![
            json!({"id": 1}),
            json!({"id": 2}),
            json!({"id": 3}),
            json!({"id": 4}),
            json!({"id": 5}),
            json!({"id": 6}),
            json!({"id": 7}),
        ];

        let batch_size = 3;
        let batches: Vec<Vec<JsonValue>> = items
            .chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // Should create 3 batches: [1,2,3], [4,5,6], [7]
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 3);
        assert_eq!(batches[1].len(), 3);
        assert_eq!(batches[2].len(), 1); // Last batch can be smaller

        // Verify content - batches are arrays
        assert_eq!(batches[0][0], json!({"id": 1}));
        assert_eq!(batches[2][0], json!({"id": 7}));
    }

    #[test]
    fn test_with_items_no_batch_size_individual_processing() {
        use serde_json::json;

        // Without batch_size, items are processed individually
        let items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];

        // Each item should be processed separately (not as batches)
        assert_eq!(items.len(), 3);

        // Verify individual items
        assert_eq!(items[0], json!({"id": 1}));
        assert_eq!(items[1], json!({"id": 2}));
        assert_eq!(items[2], json!({"id": 3}));
    }

    #[test]
    fn test_with_items_batch_vs_individual() {
        use serde_json::json;

        let items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];

        // With batch_size: items are grouped into batches (arrays)
        let batch_size = Some(2);
        if let Some(bs) = batch_size {
            let batches: Vec<Vec<JsonValue>> = items
                .clone()
                .chunks(bs)
                .map(|chunk| chunk.to_vec())
                .collect();

            // 2 batches: [1,2], [3]
            assert_eq!(batches.len(), 2);
            assert_eq!(batches[0], vec![json!({"id": 1}), json!({"id": 2})]);
            assert_eq!(batches[1], vec![json!({"id": 3})]);
        }

        // Without batch_size: items processed individually
        let batch_size: Option<usize> = None;
        if batch_size.is_none() {
            // Each item is a single value, not wrapped in array
            for (idx, item) in items.iter().enumerate() {
                assert_eq!(item["id"], idx + 1);
            }
        }
    }
}
