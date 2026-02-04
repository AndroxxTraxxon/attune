//! Workflow Execution Coordinator
//!
//! This module orchestrates workflow execution, managing task dependencies,
//! parallel execution, state transitions, and error handling.

use crate::workflow::context::WorkflowContext;
use crate::workflow::graph::{TaskGraph, TaskNode};
use crate::workflow::task_executor::{TaskExecutionResult, TaskExecutionStatus, TaskExecutor};
use attune_common::error::{Error, Result};
use attune_common::models::{
    execution::{Execution, WorkflowTaskMetadata},
    ExecutionStatus, Id, WorkflowExecution,
};
use attune_common::mq::MessageQueue;
use attune_common::workflow::WorkflowDefinition;
use chrono::Utc;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Workflow execution coordinator
pub struct WorkflowCoordinator {
    db_pool: PgPool,
    mq: MessageQueue,
    task_executor: TaskExecutor,
}

impl WorkflowCoordinator {
    /// Create a new workflow coordinator
    pub fn new(db_pool: PgPool, mq: MessageQueue) -> Self {
        let task_executor = TaskExecutor::new(db_pool.clone(), mq.clone());

        Self {
            db_pool,
            mq,
            task_executor,
        }
    }

    /// Start a new workflow execution
    pub async fn start_workflow(
        &self,
        workflow_ref: &str,
        parameters: JsonValue,
        parent_execution_id: Option<Id>,
    ) -> Result<WorkflowExecutionHandle> {
        info!(
            "Starting workflow: {} with params: {:?}",
            workflow_ref, parameters
        );

        // Load workflow definition
        let workflow_def = sqlx::query_as::<_, attune_common::models::WorkflowDefinition>(
            "SELECT * FROM attune.workflow_definition WHERE ref = $1",
        )
        .bind(workflow_ref)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or_else(|| Error::not_found("workflow_definition", "ref", workflow_ref))?;

        if !workflow_def.enabled {
            return Err(Error::validation("Workflow is disabled"));
        }

        // Parse workflow definition
        let definition: WorkflowDefinition = serde_json::from_value(workflow_def.definition)
            .map_err(|e| Error::validation(format!("Invalid workflow definition: {}", e)))?;

        // Build task graph
        let graph = TaskGraph::from_workflow(&definition)
            .map_err(|e| Error::validation(format!("Failed to build task graph: {}", e)))?;

        // Create parent execution record
        // TODO: Implement proper execution creation
        let _parent_execution_id_temp = parent_execution_id.unwrap_or(1); // Placeholder

        let parent_execution = sqlx::query_as::<_, attune_common::models::Execution>(
            r#"
            INSERT INTO attune.execution (action_ref, pack, input, parent, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(workflow_ref)
        .bind(workflow_def.pack)
        .bind(&parameters)
        .bind(parent_execution_id)
        .bind(ExecutionStatus::Running)
        .fetch_one(&self.db_pool)
        .await?;

        // Initialize workflow context
        let initial_vars: HashMap<String, JsonValue> = definition
            .vars
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let context = WorkflowContext::new(parameters, initial_vars);

        // Create workflow execution record
        let workflow_execution = self
            .create_workflow_execution_record(
                parent_execution.id,
                workflow_def.id,
                &graph,
                &context,
            )
            .await?;

        info!(
            "Created workflow execution {} for workflow {}",
            workflow_execution.id, workflow_ref
        );

        // Create execution handle
        let handle = WorkflowExecutionHandle {
            coordinator: Arc::new(self.clone_ref()),
            execution_id: workflow_execution.id,
            parent_execution_id: parent_execution.id,
            workflow_def_id: workflow_def.id,
            graph,
            state: Arc::new(Mutex::new(WorkflowExecutionState {
                context,
                status: ExecutionStatus::Running,
                completed_tasks: HashSet::new(),
                failed_tasks: HashSet::new(),
                skipped_tasks: HashSet::new(),
                executing_tasks: HashSet::new(),
                scheduled_tasks: HashSet::new(),
                join_state: HashMap::new(),
                task_executions: HashMap::new(),
                paused: false,
                pause_reason: None,
                error_message: None,
            })),
        };

        // Update execution status to running
        self.update_workflow_execution_status(workflow_execution.id, ExecutionStatus::Running)
            .await?;

        Ok(handle)
    }

    /// Create workflow execution record in database
    async fn create_workflow_execution_record(
        &self,
        execution_id: Id,
        workflow_def_id: Id,
        graph: &TaskGraph,
        context: &WorkflowContext,
    ) -> Result<WorkflowExecution> {
        let task_graph_json = serde_json::to_value(graph)
            .map_err(|e| Error::internal(format!("Failed to serialize task graph: {}", e)))?;

        let variables = context.export();

        sqlx::query_as::<_, WorkflowExecution>(
            r#"
            INSERT INTO attune.workflow_execution (
                execution, workflow_def, current_tasks, completed_tasks,
                failed_tasks, skipped_tasks, variables, task_graph,
                status, paused
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(execution_id)
        .bind(workflow_def_id)
        .bind(&[] as &[String])
        .bind(&[] as &[String])
        .bind(&[] as &[String])
        .bind(&[] as &[String])
        .bind(variables)
        .bind(task_graph_json)
        .bind(ExecutionStatus::Running)
        .bind(false)
        .fetch_one(&self.db_pool)
        .await
        .map_err(Into::into)
    }

    /// Update workflow execution status
    async fn update_workflow_execution_status(
        &self,
        workflow_execution_id: Id,
        status: ExecutionStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE attune.workflow_execution
            SET status = $1, updated = NOW()
            WHERE id = $2
            "#,
        )
        .bind(status)
        .bind(workflow_execution_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Update workflow execution state
    async fn update_workflow_execution_state(
        &self,
        workflow_execution_id: Id,
        state: &WorkflowExecutionState,
    ) -> Result<()> {
        let current_tasks: Vec<String> = state.executing_tasks.iter().cloned().collect();
        let completed_tasks: Vec<String> = state.completed_tasks.iter().cloned().collect();
        let failed_tasks: Vec<String> = state.failed_tasks.iter().cloned().collect();
        let skipped_tasks: Vec<String> = state.skipped_tasks.iter().cloned().collect();

        sqlx::query(
            r#"
            UPDATE attune.workflow_execution
            SET
                current_tasks = $1,
                completed_tasks = $2,
                failed_tasks = $3,
                skipped_tasks = $4,
                variables = $5,
                status = $6,
                paused = $7,
                pause_reason = $8,
                error_message = $9,
                updated = NOW()
            WHERE id = $10
            "#,
        )
        .bind(&current_tasks)
        .bind(&completed_tasks)
        .bind(&failed_tasks)
        .bind(&skipped_tasks)
        .bind(state.context.export())
        .bind(state.status)
        .bind(state.paused)
        .bind(&state.pause_reason)
        .bind(&state.error_message)
        .bind(workflow_execution_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Create a task execution record
    async fn create_task_execution_record(
        &self,
        workflow_execution_id: Id,
        parent_execution_id: Id,
        task: &TaskNode,
        task_index: Option<i32>,
        task_batch: Option<i32>,
    ) -> Result<Execution> {
        let max_retries = task.retry.as_ref().map(|r| r.count as i32).unwrap_or(0);
        let timeout = task.timeout.map(|t| t as i32);

        // Create workflow task metadata
        let workflow_task = WorkflowTaskMetadata {
            workflow_execution: workflow_execution_id,
            task_name: task.name.clone(),
            task_index,
            task_batch,
            retry_count: 0,
            max_retries,
            next_retry_at: None,
            timeout_seconds: timeout,
            timed_out: false,
            duration_ms: None,
            started_at: Some(Utc::now()),
            completed_at: None,
        };

        sqlx::query_as::<_, Execution>(
            r#"
            INSERT INTO attune.execution (
                action_ref, parent, status, workflow_task
            )
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(&task.name)
        .bind(parent_execution_id)
        .bind(ExecutionStatus::Running)
        .bind(sqlx::types::Json(&workflow_task))
        .fetch_one(&self.db_pool)
        .await
        .map_err(Into::into)
    }

    /// Update task execution record
    async fn update_task_execution_record(
        &self,
        task_execution_id: Id,
        result: &TaskExecutionResult,
    ) -> Result<()> {
        let status = match result.status {
            TaskExecutionStatus::Success => ExecutionStatus::Completed,
            TaskExecutionStatus::Failed => ExecutionStatus::Failed,
            TaskExecutionStatus::Timeout => ExecutionStatus::Timeout,
            TaskExecutionStatus::Skipped => ExecutionStatus::Cancelled,
        };

        // Fetch current execution to get workflow_task metadata
        let execution =
            sqlx::query_as::<_, Execution>("SELECT * FROM attune.execution WHERE id = $1")
                .bind(task_execution_id)
                .fetch_one(&self.db_pool)
                .await?;

        // Update workflow_task metadata
        if let Some(mut workflow_task) = execution.workflow_task {
            workflow_task.completed_at = if result.status == TaskExecutionStatus::Success {
                Some(Utc::now())
            } else {
                None
            };
            workflow_task.duration_ms = Some(result.duration_ms);
            workflow_task.retry_count = result.retry_count;
            workflow_task.next_retry_at = result.next_retry_at;
            workflow_task.timed_out = result.status == TaskExecutionStatus::Timeout;

            let _error_json = result.error.as_ref().map(|e| {
                json!({
                    "message": e.message,
                    "type": e.error_type,
                    "details": e.details
                })
            });

            sqlx::query(
                r#"
                UPDATE attune.execution
                SET
                    status = $1,
                    result = $2,
                    workflow_task = $3,
                    updated = NOW()
                WHERE id = $4
                "#,
            )
            .bind(status)
            .bind(&result.output)
            .bind(sqlx::types::Json(&workflow_task))
            .bind(task_execution_id)
            .execute(&self.db_pool)
            .await?;
        }

        Ok(())
    }

    /// Clone reference for Arc sharing
    fn clone_ref(&self) -> Self {
        Self {
            db_pool: self.db_pool.clone(),
            mq: self.mq.clone(),
            task_executor: TaskExecutor::new(self.db_pool.clone(), self.mq.clone()),
        }
    }
}

/// Workflow execution state
#[derive(Debug, Clone)]
pub struct WorkflowExecutionState {
    pub context: WorkflowContext,
    pub status: ExecutionStatus,
    pub completed_tasks: HashSet<String>,
    pub failed_tasks: HashSet<String>,
    pub skipped_tasks: HashSet<String>,
    /// Tasks currently executing
    pub executing_tasks: HashSet<String>,
    /// Tasks scheduled but not yet executing
    pub scheduled_tasks: HashSet<String>,
    /// Join state tracking: task_name -> set of completed predecessor tasks
    pub join_state: HashMap<String, HashSet<String>>,
    pub task_executions: HashMap<String, Vec<Id>>,
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub error_message: Option<String>,
}

/// Handle for managing a workflow execution
pub struct WorkflowExecutionHandle {
    coordinator: Arc<WorkflowCoordinator>,
    execution_id: Id,
    parent_execution_id: Id,
    #[allow(dead_code)]
    workflow_def_id: Id,
    graph: TaskGraph,
    state: Arc<Mutex<WorkflowExecutionState>>,
}

impl WorkflowExecutionHandle {
    /// Execute the workflow to completion
    pub async fn execute(&self) -> Result<WorkflowExecutionResult> {
        info!("Executing workflow {}", self.execution_id);

        // Start with entry point tasks
        {
            let mut state = self.state.lock().await;
            for task_name in &self.graph.entry_points {
                info!("Scheduling entry point task: {}", task_name);
                state.scheduled_tasks.insert(task_name.clone());
            }
        }

        // Wait for all tasks to complete
        loop {
            // Check for and spawn scheduled tasks
            let tasks_to_spawn = {
                let mut state = self.state.lock().await;
                let mut to_spawn = Vec::new();
                for task_name in state.scheduled_tasks.iter() {
                    to_spawn.push(task_name.clone());
                }
                // Clear scheduled tasks as we're about to spawn them
                state.scheduled_tasks.clear();
                to_spawn
            };

            // Spawn scheduled tasks
            for task_name in tasks_to_spawn {
                self.spawn_task_execution(task_name).await;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let state = self.state.lock().await;

            // Check if workflow is paused
            if state.paused {
                info!("Workflow {} is paused", self.execution_id);
                break;
            }

            // Check if workflow is complete (nothing executing and nothing scheduled)
            if state.executing_tasks.is_empty() && state.scheduled_tasks.is_empty() {
                info!("Workflow {} completed", self.execution_id);
                drop(state);

                let mut state = self.state.lock().await;
                if state.failed_tasks.is_empty() {
                    state.status = ExecutionStatus::Completed;
                } else {
                    state.status = ExecutionStatus::Failed;
                    state.error_message = Some(format!(
                        "Workflow failed: {} tasks failed",
                        state.failed_tasks.len()
                    ));
                }
                self.coordinator
                    .update_workflow_execution_state(self.execution_id, &state)
                    .await?;
                break;
            }
        }

        let state = self.state.lock().await;
        Ok(WorkflowExecutionResult {
            status: state.status,
            output: state.context.export(),
            completed_tasks: state.completed_tasks.len(),
            failed_tasks: state.failed_tasks.len(),
            skipped_tasks: state.skipped_tasks.len(),
            error_message: state.error_message.clone(),
        })
    }

    /// Spawn a task execution in a new tokio task
    async fn spawn_task_execution(&self, task_name: String) {
        let coordinator = self.coordinator.clone();
        let state_arc = self.state.clone();
        let workflow_execution_id = self.execution_id;
        let parent_execution_id = self.parent_execution_id;
        let graph = self.graph.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::execute_task_async(
                coordinator,
                state_arc,
                workflow_execution_id,
                parent_execution_id,
                graph,
                task_name,
            )
            .await
            {
                error!("Task execution failed: {}", e);
            }
        });
    }

    /// Execute a single task asynchronously
    async fn execute_task_async(
        coordinator: Arc<WorkflowCoordinator>,
        state: Arc<Mutex<WorkflowExecutionState>>,
        workflow_execution_id: Id,
        parent_execution_id: Id,
        graph: TaskGraph,
        task_name: String,
    ) -> Result<()> {
        // Move task from scheduled to executing
        let task = {
            let mut state = state.lock().await;
            state.scheduled_tasks.remove(&task_name);
            state.executing_tasks.insert(task_name.clone());

            // Get the task node
            match graph.get_task(&task_name) {
                Some(task) => task.clone(),
                None => {
                    error!("Task {} not found in graph", task_name);
                    return Ok(());
                }
            }
        };

        info!("Executing task: {}", task.name);

        // Create task execution record
        let task_execution = coordinator
            .create_task_execution_record(
                workflow_execution_id,
                parent_execution_id,
                &task,
                None,
                None,
            )
            .await?;

        // Get context for execution
        let mut context = {
            let state = state.lock().await;
            state.context.clone()
        };

        // Execute task
        let result = coordinator
            .task_executor
            .execute_task(
                &task,
                &mut context,
                workflow_execution_id,
                parent_execution_id,
            )
            .await?;

        // Update task execution record
        coordinator
            .update_task_execution_record(task_execution.id, &result)
            .await?;

        // Update workflow state based on result
        let success = matches!(result.status, TaskExecutionStatus::Success);

        {
            let mut state = state.lock().await;
            state.executing_tasks.remove(&task.name);

            match result.status {
                TaskExecutionStatus::Success => {
                    state.completed_tasks.insert(task.name.clone());
                    // Update context with task result
                    if let Some(output) = result.output {
                        state.context.set_task_result(&task.name, output);
                    }
                }
                TaskExecutionStatus::Failed => {
                    if result.should_retry {
                        // Task will be retried, keep it in scheduled
                        info!("Task {} will be retried", task.name);
                        state.scheduled_tasks.insert(task.name.clone());
                        // TODO: Schedule retry with delay
                    } else {
                        state.failed_tasks.insert(task.name.clone());
                        if let Some(ref error) = result.error {
                            warn!("Task {} failed: {}", task.name, error.message);
                        }
                    }
                }
                TaskExecutionStatus::Timeout => {
                    state.failed_tasks.insert(task.name.clone());
                    warn!("Task {} timed out", task.name);
                }
                TaskExecutionStatus::Skipped => {
                    state.skipped_tasks.insert(task.name.clone());
                    debug!("Task {} skipped", task.name);
                }
            }

            // Persist state
            coordinator
                .update_workflow_execution_state(workflow_execution_id, &state)
                .await?;
        }

        // Evaluate transitions and schedule next tasks
        Self::on_task_completion(state.clone(), graph.clone(), task.name.clone(), success).await?;

        Ok(())
    }

    /// Handle task completion by evaluating transitions and scheduling next tasks
    async fn on_task_completion(
        state: Arc<Mutex<WorkflowExecutionState>>,
        graph: TaskGraph,
        completed_task: String,
        success: bool,
    ) -> Result<()> {
        // Get next tasks based on transitions
        let next_tasks = graph.next_tasks(&completed_task, success);

        info!(
            "Task {} completed (success={}), next tasks: {:?}",
            completed_task, success, next_tasks
        );

        // Collect tasks to schedule
        let mut tasks_to_schedule = Vec::new();

        for next_task_name in next_tasks {
            let mut state = state.lock().await;

            // Check if task already scheduled or executing
            if state.scheduled_tasks.contains(&next_task_name)
                || state.executing_tasks.contains(&next_task_name)
            {
                continue;
            }

            if let Some(task_node) = graph.get_task(&next_task_name) {
                // Check join conditions
                if let Some(join_count) = task_node.join {
                    // Update join state
                    let join_completions = state
                        .join_state
                        .entry(next_task_name.clone())
                        .or_insert_with(HashSet::new);
                    join_completions.insert(completed_task.clone());

                    // Check if join is satisfied
                    if join_completions.len() >= join_count {
                        info!(
                            "Join condition satisfied for task {}: {}/{} completed",
                            next_task_name,
                            join_completions.len(),
                            join_count
                        );
                        state.scheduled_tasks.insert(next_task_name.clone());
                        tasks_to_schedule.push(next_task_name);
                    } else {
                        info!(
                            "Join condition not yet satisfied for task {}: {}/{} completed",
                            next_task_name,
                            join_completions.len(),
                            join_count
                        );
                    }
                } else {
                    // No join, schedule immediately
                    state.scheduled_tasks.insert(next_task_name.clone());
                    tasks_to_schedule.push(next_task_name);
                }
            } else {
                error!("Next task {} not found in graph", next_task_name);
            }
        }

        Ok(())
    }

    /// Pause workflow execution
    pub async fn pause(&self, reason: Option<String>) -> Result<()> {
        let mut state = self.state.lock().await;
        state.paused = true;
        state.pause_reason = reason;

        self.coordinator
            .update_workflow_execution_state(self.execution_id, &state)
            .await?;

        info!("Workflow {} paused", self.execution_id);
        Ok(())
    }

    /// Resume workflow execution
    pub async fn resume(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        state.paused = false;
        state.pause_reason = None;

        self.coordinator
            .update_workflow_execution_state(self.execution_id, &state)
            .await?;

        info!("Workflow {} resumed", self.execution_id);
        Ok(())
    }

    /// Cancel workflow execution
    pub async fn cancel(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        state.status = ExecutionStatus::Cancelled;

        self.coordinator
            .update_workflow_execution_state(self.execution_id, &state)
            .await?;

        info!("Workflow {} cancelled", self.execution_id);
        Ok(())
    }

    /// Get current execution status
    pub async fn status(&self) -> WorkflowExecutionStatus {
        let state = self.state.lock().await;
        WorkflowExecutionStatus {
            execution_id: self.execution_id,
            status: state.status,
            completed_tasks: state.completed_tasks.len(),
            failed_tasks: state.failed_tasks.len(),
            skipped_tasks: state.skipped_tasks.len(),
            executing_tasks: state.executing_tasks.iter().cloned().collect(),
            scheduled_tasks: state.scheduled_tasks.iter().cloned().collect(),
            total_tasks: self.graph.nodes.len(),
            paused: state.paused,
        }
    }
}

/// Result of workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowExecutionResult {
    pub status: ExecutionStatus,
    pub output: JsonValue,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub skipped_tasks: usize,
    pub error_message: Option<String>,
}

/// Current status of workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowExecutionStatus {
    pub execution_id: Id,
    pub status: ExecutionStatus,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub skipped_tasks: usize,
    pub executing_tasks: Vec<String>,
    pub scheduled_tasks: Vec<String>,
    pub total_tasks: usize,
    pub paused: bool,
}

#[cfg(test)]
mod tests {

    // Note: These tests require a database connection and are integration tests
    // They should be run with `cargo test --features integration-tests`

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_workflow_coordinator_creation() {
        // This is a placeholder test
        // Actual tests would require database setup
        assert!(true);
    }
}
