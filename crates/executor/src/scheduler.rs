//! Execution Scheduler - Routes executions to available workers
//!
//! This module is responsible for:
//! - Listening for ExecutionRequested messages
//! - Selecting appropriate workers for executions
//! - Queuing executions to worker-specific queues
//! - Updating execution status to Scheduled
//! - Handling worker unavailability and retries
//! - Detecting workflow actions and orchestrating them via child task executions
//! - Resolving `{{ }}` template expressions in workflow task inputs
//! - Processing `publish` directives from transitions
//! - Expanding `with_items` into parallel child executions

use anyhow::Result;
use attune_common::{
    models::{enums::ExecutionStatus, execution::WorkflowTaskMetadata, Action, Execution, Runtime},
    mq::{
        Consumer, ExecutionCompletedPayload, ExecutionRequestedPayload, MessageEnvelope,
        MessageType, Publisher,
    },
    repositories::{
        action::ActionRepository,
        execution::{CreateExecutionInput, ExecutionRepository, UpdateExecutionInput},
        runtime::{RuntimeRepository, WorkerRepository},
        workflow::{
            CreateWorkflowExecutionInput, WorkflowDefinitionRepository, WorkflowExecutionRepository,
        },
        Create, FindById, FindByRef, Update,
    },
    runtime_detection::runtime_aliases_contain,
    workflow::WorkflowDefinition,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::workflow::context::{TaskOutcome, WorkflowContext};
use crate::workflow::graph::TaskGraph;

/// Extract workflow parameters from an execution's `config` field.
///
/// All executions store config in flat format: `{"n": 5, ...}`.
/// The config object itself IS the parameters map.
fn extract_workflow_params(config: &Option<JsonValue>) -> JsonValue {
    match config {
        Some(c) if c.is_object() => c.clone(),
        _ => serde_json::json!({}),
    }
}

/// Apply default values from a workflow's `param_schema` to the provided
/// parameters.
///
/// The param_schema uses the flat format where each key maps to an object
/// that may contain a `"default"` field:
///
/// ```json
/// { "n": { "type": "integer", "default": 10 } }
/// ```
///
/// Any parameter that has a default in the schema but is missing (or `null`)
/// in the supplied `params` will be filled in. Parameters already provided
/// by the caller are never overwritten.
fn apply_param_defaults(params: JsonValue, param_schema: &Option<JsonValue>) -> JsonValue {
    let schema = match param_schema {
        Some(s) if s.is_object() => s,
        _ => return params,
    };

    let mut obj = match params {
        JsonValue::Object(m) => m,
        _ => return params,
    };

    if let Some(schema_obj) = schema.as_object() {
        for (key, prop) in schema_obj {
            // Only fill in missing / null parameters
            let needs_default = matches!(obj.get(key), None | Some(JsonValue::Null));
            if needs_default {
                if let Some(default_val) = prop.get("default") {
                    debug!("Applying default for parameter '{}': {}", key, default_val);
                    obj.insert(key.clone(), default_val.clone());
                }
            }
        }
    }

    JsonValue::Object(obj)
}

/// Payload for execution scheduled messages
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionScheduledPayload {
    execution_id: i64,
    worker_id: i64,
    action_ref: String,
    config: Option<JsonValue>,
}

/// Execution scheduler that routes executions to workers
pub struct ExecutionScheduler {
    pool: PgPool,
    publisher: Arc<Publisher>,
    consumer: Arc<Consumer>,
    /// Round-robin counter for distributing executions across workers
    round_robin_counter: AtomicUsize,
}

/// Default heartbeat interval in seconds (should match worker config default)
const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;

/// Maximum age multiplier for heartbeat staleness check
/// Workers are considered stale if heartbeat is older than HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER
const HEARTBEAT_STALENESS_MULTIPLIER: u64 = 3;

impl ExecutionScheduler {
    /// Create a new execution scheduler
    pub fn new(pool: PgPool, publisher: Arc<Publisher>, consumer: Arc<Consumer>) -> Self {
        Self {
            pool,
            publisher,
            consumer,
            round_robin_counter: AtomicUsize::new(0),
        }
    }

    /// Start processing execution requested messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting execution scheduler");

        let pool = self.pool.clone();
        let publisher = self.publisher.clone();
        // Share the counter with the handler closure via Arc.
        // We wrap &self's AtomicUsize in a new Arc<AtomicUsize> by copying the
        // current value so the closure is 'static.
        let counter = Arc::new(AtomicUsize::new(
            self.round_robin_counter.load(Ordering::Relaxed),
        ));

        // Use the handler pattern to consume messages
        self.consumer
            .consume_with_handler(
                move |envelope: MessageEnvelope<ExecutionRequestedPayload>| {
                    let pool = pool.clone();
                    let publisher = publisher.clone();
                    let counter = counter.clone();

                    async move {
                        if let Err(e) = Self::process_execution_requested(
                            &pool, &publisher, &counter, &envelope,
                        )
                        .await
                        {
                            error!("Error scheduling execution: {}", e);
                            // Return error to trigger nack with requeue
                            return Err(format!("Failed to schedule execution: {}", e).into());
                        }
                        Ok(())
                    }
                },
            )
            .await?;

        Ok(())
    }

    /// Process an execution requested message
    async fn process_execution_requested(
        pool: &PgPool,
        publisher: &Publisher,
        round_robin_counter: &AtomicUsize,
        envelope: &MessageEnvelope<ExecutionRequestedPayload>,
    ) -> Result<()> {
        debug!("Processing execution requested message: {:?}", envelope);

        let execution_id = envelope.payload.execution_id;

        info!("Scheduling execution: {}", execution_id);

        // Fetch execution from database
        let execution = ExecutionRepository::find_by_id(pool, execution_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Execution not found: {}", execution_id))?;

        // Fetch action to determine runtime requirements
        let action = Self::get_action_for_execution(pool, &execution).await?;

        // Check if this action is a workflow (has workflow_def set)
        if action.workflow_def.is_some() {
            info!(
                "Action '{}' is a workflow, orchestrating instead of dispatching to worker",
                action.r#ref
            );
            return Self::process_workflow_execution(
                pool,
                publisher,
                round_robin_counter,
                &execution,
                &action,
            )
            .await;
        }

        // Regular action: select appropriate worker (round-robin among compatible workers)
        let worker = match Self::select_worker(pool, &action, round_robin_counter).await {
            Ok(worker) => worker,
            Err(err) if Self::is_unschedulable_error(&err) => {
                Self::fail_unschedulable_execution(
                    pool,
                    publisher,
                    envelope,
                    execution_id,
                    action.id,
                    &action.r#ref,
                    &err.to_string(),
                )
                .await?;
                return Ok(());
            }
            Err(err) => return Err(err),
        };

        info!(
            "Selected worker {} for execution {}",
            worker.id, execution_id
        );

        // Apply parameter defaults from the action's param_schema.
        // This mirrors what `process_workflow_execution` does for workflows
        // so that non-workflow executions also get missing parameters filled
        // in from the action's declared defaults.
        let execution_config = {
            let raw_config = execution.config.clone();
            let params = extract_workflow_params(&raw_config);
            let params_with_defaults = apply_param_defaults(params, &action.param_schema);
            // Config is already flat — just use the defaults-applied version
            if params_with_defaults.is_object()
                && !params_with_defaults.as_object().unwrap().is_empty()
            {
                Some(params_with_defaults)
            } else {
                raw_config
            }
        };

        // Persist the selected worker so later cancellation requests can be
        // routed to the correct per-worker cancel queue.
        let mut execution_for_update = execution;
        execution_for_update.status = ExecutionStatus::Scheduled;
        execution_for_update.worker = Some(worker.id);
        ExecutionRepository::update(pool, execution_for_update.id, execution_for_update.into())
            .await?;

        // Publish message to worker-specific queue
        Self::queue_to_worker(
            publisher,
            &execution_id,
            &worker.id,
            &envelope.payload.action_ref,
            &execution_config,
            &action,
        )
        .await?;

        info!(
            "Execution {} scheduled to worker {}",
            execution_id, worker.id
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Workflow orchestration
    // -----------------------------------------------------------------------

    /// Handle a workflow execution by loading its definition, creating a
    /// `workflow_execution` record, and dispatching the entry-point tasks as
    /// child executions that workers *can* handle.
    async fn process_workflow_execution(
        pool: &PgPool,
        publisher: &Publisher,
        round_robin_counter: &AtomicUsize,
        execution: &Execution,
        action: &Action,
    ) -> Result<()> {
        let workflow_def_id = action
            .workflow_def
            .ok_or_else(|| anyhow::anyhow!("Action '{}' has no workflow_def", action.r#ref))?;

        // Load workflow definition
        let workflow_def = WorkflowDefinitionRepository::find_by_id(pool, workflow_def_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Workflow definition {} not found for action '{}'",
                    workflow_def_id,
                    action.r#ref
                )
            })?;

        // Parse workflow definition JSON into the strongly-typed struct
        let definition: WorkflowDefinition =
            serde_json::from_value(workflow_def.definition.clone()).map_err(|e| {
                anyhow::anyhow!(
                    "Invalid workflow definition for '{}': {}",
                    workflow_def.r#ref,
                    e
                )
            })?;

        // Build the task graph to determine entry points and transitions
        let graph = TaskGraph::from_workflow(&definition).map_err(|e| {
            anyhow::anyhow!(
                "Failed to build task graph for workflow '{}': {}",
                workflow_def.r#ref,
                e
            )
        })?;

        let task_graph_json: JsonValue = serde_json::to_value(&graph).unwrap_or_default();

        // Gather initial variables from the definition
        let initial_vars: JsonValue =
            serde_json::to_value(&definition.vars).unwrap_or_else(|_| serde_json::json!({}));

        // Create workflow_execution record
        let workflow_execution = WorkflowExecutionRepository::create(
            pool,
            CreateWorkflowExecutionInput {
                execution: execution.id,
                workflow_def: workflow_def.id,
                task_graph: task_graph_json,
                variables: initial_vars,
                status: ExecutionStatus::Running,
            },
        )
        .await?;

        info!(
            "Created workflow_execution {} for workflow '{}' (parent execution {})",
            workflow_execution.id, workflow_def.r#ref, execution.id
        );

        // Mark the parent execution as Running
        let mut running_exec = execution.clone();
        running_exec.status = ExecutionStatus::Running;
        ExecutionRepository::update(pool, running_exec.id, running_exec.into()).await?;

        if graph.entry_points.is_empty() {
            warn!(
                "Workflow '{}' has no entry-point tasks, completing immediately",
                workflow_def.r#ref
            );
            Self::complete_workflow(pool, execution.id, workflow_execution.id, true, None).await?;
            return Ok(());
        }

        // Build initial workflow context from execution parameters and
        // workflow-level vars so that entry-point task inputs are rendered.
        // Apply defaults from the workflow's param_schema for any parameters
        // that were not supplied by the caller.
        let workflow_params = extract_workflow_params(&execution.config);
        let workflow_params = apply_param_defaults(workflow_params, &workflow_def.param_schema);
        let wf_ctx = WorkflowContext::new(
            workflow_params,
            definition
                .vars
                .iter()
                .map(|(k, v)| {
                    let jv: JsonValue =
                        serde_json::to_value(v).unwrap_or(JsonValue::String(v.to_string()));
                    (k.clone(), jv)
                })
                .collect(),
        );

        // For each entry-point task, create a child execution and dispatch it
        for entry_task_name in &graph.entry_points {
            if let Some(task_node) = graph.get_task(entry_task_name) {
                Self::dispatch_workflow_task(
                    pool,
                    publisher,
                    round_robin_counter,
                    execution,
                    &workflow_execution.id,
                    task_node,
                    &wf_ctx,
                    None, // entry-point task — no predecessor
                )
                .await?;
            } else {
                warn!(
                    "Entry-point task '{}' not found in graph for workflow '{}'",
                    entry_task_name, workflow_def.r#ref
                );
            }
        }

        Ok(())
    }

    /// Create a child execution for a single workflow task and dispatch it to
    /// a worker. The child execution references the parent workflow execution
    /// via `workflow_task` metadata.
    ///
    /// `triggered_by` is the name of the predecessor task whose completion
    /// caused this task to be scheduled.  Pass `None` for entry-point tasks
    /// dispatched at workflow start.
    #[allow(clippy::too_many_arguments)]
    async fn dispatch_workflow_task(
        pool: &PgPool,
        publisher: &Publisher,
        _round_robin_counter: &AtomicUsize,
        parent_execution: &Execution,
        workflow_execution_id: &i64,
        task_node: &crate::workflow::graph::TaskNode,
        wf_ctx: &WorkflowContext,
        triggered_by: Option<&str>,
    ) -> Result<()> {
        let action_ref: String = match &task_node.action {
            Some(a) => a.clone(),
            None => {
                warn!(
                    "Workflow task '{}' has no action reference, skipping",
                    task_node.name
                );
                return Ok(());
            }
        };

        // Resolve the task's action from the database
        let task_action = ActionRepository::find_by_ref(pool, &action_ref).await?;
        let task_action = match task_action {
            Some(a) => a,
            None => {
                error!(
                    "Action '{}' not found for workflow task '{}'",
                    action_ref, task_node.name
                );
                return Err(anyhow::anyhow!(
                    "Action '{}' not found for workflow task '{}'",
                    action_ref,
                    task_node.name
                ));
            }
        };

        // -----------------------------------------------------------------
        // with_items expansion: if the task declares `with_items`, resolve
        // the list expression and create one child execution per item (up
        // to `concurrency` in parallel — though concurrency limiting is
        // left for a future enhancement; we fan out all items now).
        // -----------------------------------------------------------------
        if let Some(ref with_items_expr) = task_node.with_items {
            return Self::dispatch_with_items_task(
                pool,
                publisher,
                parent_execution,
                workflow_execution_id,
                task_node,
                &task_action,
                &action_ref,
                with_items_expr,
                wf_ctx,
                triggered_by,
            )
            .await;
        }

        // -----------------------------------------------------------------
        // Render task input templates through the WorkflowContext
        // -----------------------------------------------------------------
        let rendered_input =
            if task_node.input.is_object() && !task_node.input.as_object().unwrap().is_empty() {
                match wf_ctx.render_json(&task_node.input) {
                    Ok(rendered) => rendered,
                    Err(e) => {
                        warn!(
                            "Template rendering failed for task '{}': {}. Using raw input.",
                            task_node.name, e
                        );
                        task_node.input.clone()
                    }
                }
            } else {
                task_node.input.clone()
            };

        // Build task config from the (rendered) input.
        // Store as flat parameters (consistent with manual and rule-triggered
        // executions) — no {"parameters": ...} wrapper.
        let task_config: Option<JsonValue> =
            if rendered_input.is_object() && !rendered_input.as_object().unwrap().is_empty() {
                Some(rendered_input.clone())
            } else {
                parent_execution.config.clone()
            };

        // Build workflow task metadata
        let workflow_task = WorkflowTaskMetadata {
            workflow_execution: *workflow_execution_id,
            task_name: task_node.name.clone(),
            triggered_by: triggered_by.map(String::from),
            task_index: None,
            task_batch: None,
            retry_count: 0,
            max_retries: task_node
                .retry
                .as_ref()
                .map(|r| r.count as i32)
                .unwrap_or(0),
            next_retry_at: None,
            timeout_seconds: task_node.timeout.map(|t| t as i32),
            timed_out: false,
            duration_ms: None,
            started_at: None,
            completed_at: None,
        };

        // Create child execution record
        let child_execution = ExecutionRepository::create(
            pool,
            CreateExecutionInput {
                action: Some(task_action.id),
                action_ref: action_ref.clone(),
                config: task_config,
                env_vars: parent_execution.env_vars.clone(),
                parent: Some(parent_execution.id),
                enforcement: parent_execution.enforcement,
                executor: None,
                worker: None,
                status: ExecutionStatus::Requested,
                result: None,
                workflow_task: Some(workflow_task),
            },
        )
        .await?;

        info!(
            "Created child execution {} for workflow task '{}' (action '{}', workflow_execution {})",
            child_execution.id, task_node.name, action_ref, workflow_execution_id
        );

        // If the task's action is itself a workflow, the recursive
        // `process_execution_requested` call will detect that and orchestrate
        // it in turn. For regular actions it will be dispatched to a worker.
        let payload = ExecutionRequestedPayload {
            execution_id: child_execution.id,
            action_id: Some(task_action.id),
            action_ref: action_ref.clone(),
            parent_id: Some(parent_execution.id),
            enforcement_id: parent_execution.enforcement,
            config: child_execution.config.clone(),
        };

        let envelope = MessageEnvelope::new(MessageType::ExecutionRequested, payload)
            .with_source("executor-scheduler");

        publisher.publish_envelope(&envelope).await?;

        info!(
            "Published ExecutionRequested for child execution {} (task '{}')",
            child_execution.id, task_node.name
        );

        Ok(())
    }

    /// Expand a `with_items` task into child executions.
    ///
    /// The `with_items` expression (e.g. `"{{ number_list }}"`) is resolved
    /// via the workflow context to produce a JSON array.  ALL child execution
    /// records are created in the database up front so that the sibling-count
    /// query in [`advance_workflow`] sees the complete set.
    ///
    /// When a `concurrency` limit is set on the task, only the first
    /// `concurrency` items are published to the message queue.  The remaining
    /// children stay at `Requested` status in the database.  As each item
    /// completes, [`advance_workflow`] publishes the next `Requested` sibling
    /// to keep the concurrency window full.
    #[allow(clippy::too_many_arguments)]
    async fn dispatch_with_items_task(
        pool: &PgPool,
        publisher: &Publisher,
        parent_execution: &Execution,
        workflow_execution_id: &i64,
        task_node: &crate::workflow::graph::TaskNode,
        task_action: &Action,
        action_ref: &str,
        with_items_expr: &str,
        wf_ctx: &WorkflowContext,
        triggered_by: Option<&str>,
    ) -> Result<()> {
        // Resolve the with_items expression to a JSON array
        let items_value = wf_ctx
            .render_json(&JsonValue::String(with_items_expr.to_string()))
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to resolve with_items expression '{}' for task '{}': {}",
                    with_items_expr,
                    task_node.name,
                    e
                )
            })?;

        let items = match items_value.as_array() {
            Some(arr) => arr.clone(),
            None => {
                warn!(
                    "with_items for task '{}' resolved to non-array value: {:?}. \
                     Wrapping in single-element array.",
                    task_node.name, items_value
                );
                vec![items_value]
            }
        };

        let total = items.len();
        let concurrency_limit = task_node.concurrency.unwrap_or(1);
        let dispatch_count = total.min(concurrency_limit);

        info!(
            "Expanding with_items for task '{}': {} items (concurrency: {}, dispatching first {})",
            task_node.name, total, concurrency_limit, dispatch_count
        );

        // Phase 1: Create ALL child execution records in the database.
        // Each row captures the fully-rendered input so we never need to
        // re-render templates later when publishing deferred items.
        let mut child_ids: Vec<i64> = Vec::with_capacity(total);

        for (index, item) in items.iter().enumerate() {
            let mut item_ctx = wf_ctx.clone();
            item_ctx.set_current_item(item.clone(), index);

            let rendered_input = if task_node.input.is_object()
                && !task_node.input.as_object().unwrap().is_empty()
            {
                match item_ctx.render_json(&task_node.input) {
                    Ok(rendered) => rendered,
                    Err(e) => {
                        warn!(
                            "Template rendering failed for task '{}' item {}: {}. Using raw input.",
                            task_node.name, index, e
                        );
                        task_node.input.clone()
                    }
                }
            } else {
                task_node.input.clone()
            };

            // Store as flat parameters (consistent with manual and rule-triggered
            // executions) — no {"parameters": ...} wrapper.
            let task_config: Option<JsonValue> =
                if rendered_input.is_object() && !rendered_input.as_object().unwrap().is_empty() {
                    Some(rendered_input.clone())
                } else {
                    parent_execution.config.clone()
                };

            let workflow_task = WorkflowTaskMetadata {
                workflow_execution: *workflow_execution_id,
                task_name: task_node.name.clone(),
                triggered_by: triggered_by.map(String::from),
                task_index: Some(index as i32),
                task_batch: None,
                retry_count: 0,
                max_retries: task_node
                    .retry
                    .as_ref()
                    .map(|r| r.count as i32)
                    .unwrap_or(0),
                next_retry_at: None,
                timeout_seconds: task_node.timeout.map(|t| t as i32),
                timed_out: false,
                duration_ms: None,
                started_at: None,
                completed_at: None,
            };

            let child_execution = ExecutionRepository::create(
                pool,
                CreateExecutionInput {
                    action: Some(task_action.id),
                    action_ref: action_ref.to_string(),
                    config: task_config,
                    env_vars: parent_execution.env_vars.clone(),
                    parent: Some(parent_execution.id),
                    enforcement: parent_execution.enforcement,
                    executor: None,
                    worker: None,
                    status: ExecutionStatus::Requested,
                    result: None,
                    workflow_task: Some(workflow_task),
                },
            )
            .await?;

            info!(
                "Created with_items child execution {} for task '{}' item {} \
                 (action '{}', workflow_execution {})",
                child_execution.id, task_node.name, index, action_ref, workflow_execution_id
            );

            child_ids.push(child_execution.id);
        }

        // Phase 2: Publish only the first `dispatch_count` to the MQ.
        // The rest stay at Requested status until advance_workflow picks
        // them up as earlier items complete.
        for &child_id in child_ids.iter().take(dispatch_count) {
            Self::publish_execution_requested(
                pool,
                publisher,
                child_id,
                task_action.id,
                action_ref,
                parent_execution,
            )
            .await?;
        }

        info!(
            "Dispatched {} of {} with_items child executions for task '{}'",
            dispatch_count, total, task_node.name
        );

        Ok(())
    }

    /// Publish an `ExecutionRequested` message for an existing execution row.
    ///
    /// Used to MQ-publish child executions that were created in the database
    /// but not yet dispatched (deferred by concurrency limiting).
    async fn publish_execution_requested(
        pool: &PgPool,
        publisher: &Publisher,
        execution_id: i64,
        action_id: i64,
        action_ref: &str,
        parent_execution: &Execution,
    ) -> Result<()> {
        let child = ExecutionRepository::find_by_id(pool, execution_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Execution {} not found", execution_id))?;

        let payload = ExecutionRequestedPayload {
            execution_id: child.id,
            action_id: Some(action_id),
            action_ref: action_ref.to_string(),
            parent_id: Some(parent_execution.id),
            enforcement_id: parent_execution.enforcement,
            config: child.config.clone(),
        };

        let envelope = MessageEnvelope::new(MessageType::ExecutionRequested, payload)
            .with_source("executor-scheduler");

        publisher.publish_envelope(&envelope).await?;

        debug!(
            "Published deferred ExecutionRequested for child execution {}",
            execution_id
        );

        Ok(())
    }

    /// Publish the next `Requested`-status with_items siblings to fill freed
    /// concurrency slots.
    ///
    /// When a with_items child completes, this method queries for siblings
    /// that are still at `Requested` status (created in DB but never
    /// published to MQ) and publishes enough of them to restore the
    /// concurrency window.
    ///
    /// Returns the number of items dispatched.
    async fn publish_pending_with_items_children(
        pool: &PgPool,
        publisher: &Publisher,
        parent_execution: &Execution,
        workflow_execution_id: i64,
        task_name: &str,
        slots: usize,
    ) -> Result<usize> {
        if slots == 0 {
            return Ok(0);
        }

        // Find siblings still at Requested status, ordered by task_index.
        let pending_rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT id, COALESCE(action, 0) as action_id \
             FROM execution \
             WHERE workflow_task->>'workflow_execution' = $1::text \
               AND workflow_task->>'task_name' = $2 \
               AND status = 'requested' \
             ORDER BY (workflow_task->>'task_index')::int ASC \
             LIMIT $3",
        )
        .bind(workflow_execution_id.to_string())
        .bind(task_name)
        .bind(slots as i64)
        .fetch_all(pool)
        .await?;

        let mut dispatched = 0usize;
        for (child_id, action_id) in &pending_rows {
            // Read action_ref from the execution row
            let child = match ExecutionRepository::find_by_id(pool, *child_id).await? {
                Some(c) => c,
                None => continue,
            };

            if let Err(e) = Self::publish_execution_requested(
                pool,
                publisher,
                *child_id,
                *action_id,
                &child.action_ref,
                parent_execution,
            )
            .await
            {
                error!(
                    "Failed to publish pending with_items child {}: {}",
                    child_id, e
                );
            } else {
                dispatched += 1;
            }
        }

        if dispatched > 0 {
            info!(
                "Published {} pending with_items children for task '{}' \
                 (workflow_execution {})",
                dispatched, task_name, workflow_execution_id
            );
        }

        Ok(dispatched)
    }

    /// Advance a workflow after a child task completes. Called from the
    /// completion listener when it detects that the completed execution has
    /// `workflow_task` metadata.
    ///
    /// This evaluates transitions from the completed task, schedules successor
    /// tasks, and completes the workflow when all tasks are done.
    pub async fn advance_workflow(
        pool: &PgPool,
        publisher: &Publisher,
        round_robin_counter: &AtomicUsize,
        execution: &Execution,
    ) -> Result<()> {
        let workflow_task = match &execution.workflow_task {
            Some(wt) => wt,
            None => return Ok(()), // Not a workflow task, nothing to do
        };

        let workflow_execution_id = workflow_task.workflow_execution;
        let task_name = &workflow_task.task_name;
        let task_succeeded = execution.status == ExecutionStatus::Completed;
        let task_timed_out = execution.status == ExecutionStatus::Timeout;

        let task_outcome = if task_succeeded {
            TaskOutcome::Succeeded
        } else if task_timed_out {
            TaskOutcome::TimedOut
        } else {
            TaskOutcome::Failed
        };

        info!(
            "Advancing workflow_execution {} after task '{}' {:?} (execution {})",
            workflow_execution_id, task_name, task_outcome, execution.id,
        );

        // Load the workflow execution record
        let workflow_execution =
            WorkflowExecutionRepository::find_by_id(pool, workflow_execution_id)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("Workflow execution {} not found", workflow_execution_id)
                })?;

        // Already fully terminal (Completed / Failed) — nothing to do
        if matches!(
            workflow_execution.status,
            ExecutionStatus::Completed | ExecutionStatus::Failed
        ) {
            debug!(
                "Workflow execution {} already in terminal state {:?}, skipping advance",
                workflow_execution_id, workflow_execution.status
            );
            return Ok(());
        }

        let parent_execution = ExecutionRepository::find_by_id(pool, workflow_execution.execution)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Parent execution {} not found for workflow_execution {}",
                    workflow_execution.execution,
                    workflow_execution_id
                )
            })?;

        // Cancellation must be a hard stop for workflow orchestration. Once
        // either the workflow record, the parent execution, or the completed
        // child itself is in a cancellation state, do not evaluate transitions,
        // release more with_items siblings, or dispatch any successor tasks.
        if Self::should_halt_workflow_advancement(
            workflow_execution.status,
            parent_execution.status,
            execution.status,
        ) {
            if workflow_execution.status == ExecutionStatus::Cancelled {
                let running = Self::count_running_workflow_children(
                    pool,
                    workflow_execution_id,
                    &workflow_execution.completed_tasks,
                    &workflow_execution.failed_tasks,
                )
                .await?;

                if running == 0 {
                    info!(
                        "Cancelled workflow_execution {} has no more running children, \
                         finalizing parent execution {} as Cancelled",
                        workflow_execution_id, workflow_execution.execution
                    );
                    Self::finalize_cancelled_workflow(
                        pool,
                        workflow_execution.execution,
                        workflow_execution_id,
                    )
                    .await?;
                } else {
                    debug!(
                        "Workflow_execution {} is cancelling/cancelled with {} running children, \
                         skipping advancement",
                        workflow_execution_id, running
                    );
                }
            } else {
                debug!(
                    "Workflow_execution {} advancement halted due to cancellation state \
                     (workflow: {:?}, parent: {:?}, child: {:?})",
                    workflow_execution_id,
                    workflow_execution.status,
                    parent_execution.status,
                    execution.status
                );
            }

            return Ok(());
        }

        // Load the workflow definition so we can apply param_schema defaults
        let workflow_def =
            WorkflowDefinitionRepository::find_by_id(pool, workflow_execution.workflow_def)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Workflow definition {} not found for workflow_execution {}",
                        workflow_execution.workflow_def,
                        workflow_execution_id
                    )
                })?;

        // Rebuild the task graph from the stored JSON
        let graph: TaskGraph = serde_json::from_value(workflow_execution.task_graph.clone())
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to deserialize task graph for workflow_execution {}: {}",
                    workflow_execution_id,
                    e
                )
            })?;

        // Update completed/failed task lists
        let mut completed_tasks: Vec<String> = workflow_execution.completed_tasks.clone();
        let mut failed_tasks: Vec<String> = workflow_execution.failed_tasks.clone();

        // For with_items tasks, only mark completed/failed when ALL items
        // for this task are done (no more running children with the same
        // task_name).
        let is_with_items = workflow_task.task_index.is_some();
        if is_with_items {
            // ---------------------------------------------------------
            // Concurrency: publish next Requested-status sibling(s) to
            // fill the slot freed by this completion.
            // ---------------------------------------------------------
            let parent_for_pending =
                ExecutionRepository::find_by_id(pool, workflow_execution.execution)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Parent execution {} not found for workflow_execution {}",
                            workflow_execution.execution,
                            workflow_execution_id
                        )
                    })?;

            // Count siblings that are actively in-flight (Scheduling,
            // Scheduled, or Running — NOT Requested, which means "created
            // but not yet published to MQ").
            let in_flight_count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) \
                 FROM execution \
                 WHERE workflow_task->>'workflow_execution' = $1::text \
                   AND workflow_task->>'task_name' = $2 \
                   AND status IN ('scheduling', 'scheduled', 'running') \
                   AND id != $3",
            )
            .bind(workflow_execution_id.to_string())
            .bind(task_name)
            .bind(execution.id)
            .fetch_one(pool)
            .await?;

            // Determine the concurrency limit from the task graph
            let concurrency_limit = graph
                .get_task(task_name)
                .and_then(|n| n.concurrency)
                .unwrap_or(1);

            let free_slots = concurrency_limit.saturating_sub(in_flight_count.0 as usize);

            if free_slots > 0 {
                if let Err(e) = Self::publish_pending_with_items_children(
                    pool,
                    publisher,
                    &parent_for_pending,
                    workflow_execution_id,
                    task_name,
                    free_slots,
                )
                .await
                {
                    error!(
                        "Failed to publish pending with_items for task '{}': {}",
                        task_name, e
                    );
                }
            }

            // Count how many siblings are NOT in a terminal state
            // (Requested items are pending, in-flight items are working).
            let siblings_remaining: Vec<(String,)> = sqlx::query_as(
                "SELECT workflow_task->>'task_name' as task_name \
                 FROM execution \
                 WHERE workflow_task->>'workflow_execution' = $1::text \
                   AND workflow_task->>'task_name' = $2 \
                   AND status NOT IN ('completed', 'failed', 'timeout', 'cancelled') \
                   AND id != $3",
            )
            .bind(workflow_execution_id.to_string())
            .bind(task_name)
            .bind(execution.id)
            .fetch_all(pool)
            .await?;

            if !siblings_remaining.is_empty() {
                debug!(
                    "with_items task '{}' item {} done, but {} siblings remaining — \
                     not advancing yet",
                    task_name,
                    workflow_task.task_index.unwrap_or(-1),
                    siblings_remaining.len(),
                );
                return Ok(());
            }

            // ---------------------------------------------------------
            // Race-condition guard: when multiple with_items children
            // complete nearly simultaneously, the worker updates their
            // DB status to Completed *before* the completion MQ message
            // is processed.  This means several advance_workflow calls
            // (processed sequentially by the completion listener) can
            // each see "0 siblings remaining" and fall through to
            // transition evaluation, dispatching successor tasks
            // multiple times.
            //
            // To prevent this we re-check the *persisted*
            // completed/failed task lists that were loaded from the
            // workflow_execution record at the top of this function.
            // If `task_name` is already present, a previous
            // advance_workflow invocation already handled the final
            // completion of this with_items task and dispatched its
            // successors — we can safely return.
            // ---------------------------------------------------------
            if workflow_execution
                .completed_tasks
                .contains(&task_name.to_string())
                || workflow_execution
                    .failed_tasks
                    .contains(&task_name.to_string())
            {
                debug!(
                    "with_items task '{}' already in persisted completed/failed list — \
                     another advance_workflow call already handled final completion, skipping",
                    task_name,
                );
                return Ok(());
            }

            // All items done — check if any failed
            let any_failed: Vec<(i64,)> = sqlx::query_as(
                "SELECT id \
                 FROM execution \
                 WHERE workflow_task->>'workflow_execution' = $1::text \
                   AND workflow_task->>'task_name' = $2 \
                   AND status IN ('failed', 'timeout') \
                 LIMIT 1",
            )
            .bind(workflow_execution_id.to_string())
            .bind(task_name)
            .fetch_all(pool)
            .await?;

            if any_failed.is_empty() {
                if !completed_tasks.contains(task_name) {
                    completed_tasks.push(task_name.clone());
                }
            } else if !failed_tasks.contains(task_name) {
                failed_tasks.push(task_name.clone());
            }
        } else {
            // Normal (non-with_items) task
            if task_succeeded {
                if !completed_tasks.contains(task_name) {
                    completed_tasks.push(task_name.clone());
                }
            } else if !failed_tasks.contains(task_name) {
                failed_tasks.push(task_name.clone());
            }
        }

        // -----------------------------------------------------------------
        // Rebuild the WorkflowContext from persisted state + completed task
        // results so that successor task inputs can be rendered.
        // -----------------------------------------------------------------
        let workflow_params = extract_workflow_params(&parent_execution.config);
        let workflow_params = apply_param_defaults(workflow_params, &workflow_def.param_schema);

        // Collect results from all completed children of this workflow
        let child_executions =
            ExecutionRepository::find_by_parent(pool, parent_execution.id).await?;
        let mut task_results_map: HashMap<String, JsonValue> = HashMap::new();
        for child in &child_executions {
            if let Some(ref wt) = child.workflow_task {
                if wt.workflow_execution == workflow_execution_id
                    && matches!(
                        child.status,
                        ExecutionStatus::Completed
                            | ExecutionStatus::Failed
                            | ExecutionStatus::Timeout
                    )
                {
                    let result_val = child.result.clone().unwrap_or(serde_json::json!({}));
                    task_results_map.insert(wt.task_name.clone(), result_val);
                }
            }
        }

        let mut wf_ctx = WorkflowContext::rebuild(
            workflow_params,
            &workflow_execution.variables,
            task_results_map,
        );

        // Set the just-completed task's outcome so that `result()`,
        // `succeeded()`, `failed()` resolve correctly for publish and
        // transition conditions.
        let completed_result = execution.result.clone().unwrap_or(serde_json::json!({}));
        wf_ctx.set_last_task_outcome(completed_result, task_outcome);

        // -----------------------------------------------------------------
        // Process transitions: evaluate conditions, process publish
        // directives, collect successor tasks.
        // -----------------------------------------------------------------
        let mut tasks_to_schedule: Vec<String> = Vec::new();

        if let Some(completed_task_node) = graph.get_task(task_name) {
            for transition in &completed_task_node.transitions {
                let should_fire = match transition.kind() {
                    crate::workflow::graph::TransitionKind::Succeeded => task_succeeded,
                    crate::workflow::graph::TransitionKind::Failed => {
                        !task_succeeded && !task_timed_out
                    }
                    crate::workflow::graph::TransitionKind::Always => true,
                    crate::workflow::graph::TransitionKind::TimedOut => task_timed_out,
                    crate::workflow::graph::TransitionKind::Custom => {
                        // Try to evaluate via the workflow context
                        if let Some(ref when_expr) = transition.when {
                            match wf_ctx.evaluate_condition(when_expr) {
                                Ok(val) => val,
                                Err(e) => {
                                    warn!(
                                        "Custom condition '{}' evaluation failed: {}. \
                                         Defaulting to fire-on-success.",
                                        when_expr, e
                                    );
                                    task_succeeded
                                }
                            }
                        } else {
                            task_succeeded
                        }
                    }
                };

                if should_fire {
                    // Process publish directives from this transition
                    if !transition.publish.is_empty() {
                        let publish_map: HashMap<String, JsonValue> = transition
                            .publish
                            .iter()
                            .map(|p| (p.name.clone(), p.value.clone()))
                            .collect();
                        if let Err(e) = wf_ctx.publish_from_result(
                            &serde_json::json!({}),
                            &[],
                            Some(&publish_map),
                        ) {
                            warn!("Failed to process publish for task '{}': {}", task_name, e);
                        } else {
                            debug!(
                                "Published {} variables from task '{}' transition",
                                publish_map.len(),
                                task_name
                            );
                        }
                    }

                    for next_task_name in &transition.do_tasks {
                        // Skip tasks that are already completed or failed
                        if completed_tasks.contains(next_task_name)
                            || failed_tasks.contains(next_task_name)
                        {
                            debug!(
                                "Skipping task '{}' — already completed or failed",
                                next_task_name
                            );
                            continue;
                        }

                        // Guard against dispatching a task that has already
                        // been dispatched (exists as a child execution in
                        // this workflow).  This catches edge cases where
                        // the persisted completed/failed lists haven't been
                        // updated yet but a child execution was already
                        // created by a prior advance_workflow call.
                        //
                        // This is critical for with_items predecessors:
                        // workers update DB status to Completed before the
                        // completion MQ message is processed, so multiple
                        // with_items items can each see "0 siblings
                        // remaining" and attempt to dispatch the same
                        // successor.  The query covers both regular tasks
                        // (task_index IS NULL) and with_items tasks
                        // (task_index IS NOT NULL) so that neither case
                        // can be double-dispatched.
                        let already_dispatched: (i64,) = sqlx::query_as(
                            "SELECT COUNT(*) \
                             FROM execution \
                             WHERE workflow_task->>'workflow_execution' = $1::text \
                               AND workflow_task->>'task_name' = $2",
                        )
                        .bind(workflow_execution_id.to_string())
                        .bind(next_task_name.as_str())
                        .fetch_one(pool)
                        .await?;

                        if already_dispatched.0 > 0 {
                            debug!(
                                "Skipping task '{}' — already dispatched ({} existing execution(s))",
                                next_task_name, already_dispatched.0
                            );
                            continue;
                        }

                        // Check join barrier: if the task has a `join` count,
                        // only schedule it when enough predecessors are done.
                        if let Some(next_node) = graph.get_task(next_task_name) {
                            if let Some(join_count) = next_node.join {
                                let inbound_completed = next_node
                                    .inbound_tasks
                                    .iter()
                                    .filter(|t| completed_tasks.contains(*t))
                                    .count();
                                if inbound_completed < join_count {
                                    debug!(
                                        "Task '{}' join barrier not met ({}/{} predecessors done)",
                                        next_task_name, inbound_completed, join_count
                                    );
                                    continue;
                                }
                            }
                        }

                        if !tasks_to_schedule.contains(next_task_name) {
                            tasks_to_schedule.push(next_task_name.clone());
                        }
                    }
                }
            }
        }

        // Check if any tasks are still running (children of this workflow
        // that haven't completed yet). We query child executions that have
        // workflow_task metadata pointing to our workflow_execution.
        let running_children = Self::count_running_workflow_children(
            pool,
            workflow_execution_id,
            &completed_tasks,
            &failed_tasks,
        )
        .await?;

        // Dispatch successor tasks, passing the updated workflow context
        for next_task_name in &tasks_to_schedule {
            if let Some(task_node) = graph.get_task(next_task_name) {
                if let Err(e) = Self::dispatch_workflow_task(
                    pool,
                    publisher,
                    round_robin_counter,
                    &parent_execution,
                    &workflow_execution_id,
                    task_node,
                    &wf_ctx,
                    Some(task_name), // predecessor that triggered this task
                )
                .await
                {
                    error!(
                        "Failed to dispatch workflow task '{}': {}",
                        next_task_name, e
                    );
                    if !failed_tasks.contains(next_task_name) {
                        failed_tasks.push(next_task_name.clone());
                    }
                }
            }
        }

        // Determine current executing tasks (for the workflow_execution record)
        let current_tasks: Vec<String> = tasks_to_schedule.clone();

        // Persist updated workflow variables (from publish directives) and
        // completed/failed task lists.
        let updated_variables = wf_ctx.export_variables();
        WorkflowExecutionRepository::update(
            pool,
            workflow_execution_id,
            attune_common::repositories::workflow::UpdateWorkflowExecutionInput {
                current_tasks: Some(current_tasks),
                completed_tasks: Some(completed_tasks.clone()),
                failed_tasks: Some(failed_tasks.clone()),
                skipped_tasks: None,
                variables: Some(updated_variables),
                status: None, // Updated below if terminal
                error_message: None,
                paused: None,
                pause_reason: None,
            },
        )
        .await?;

        // Check if workflow is complete: no more tasks to schedule and no
        // children still running (excluding the ones we just scheduled).
        let all_done = tasks_to_schedule.is_empty() && running_children == 0;

        if all_done {
            let has_failures = !failed_tasks.is_empty();
            let error_msg = if has_failures {
                Some(format!(
                    "Workflow failed: {} task(s) failed: {}",
                    failed_tasks.len(),
                    failed_tasks.join(", ")
                ))
            } else {
                None
            };
            Self::complete_workflow(
                pool,
                parent_execution.id,
                workflow_execution_id,
                !has_failures,
                error_msg.as_deref(),
            )
            .await?;
        }

        Ok(())
    }

    /// Count child executions that are still in progress for a workflow.
    async fn count_running_workflow_children(
        pool: &PgPool,
        workflow_execution_id: i64,
        completed_tasks: &[String],
        failed_tasks: &[String],
    ) -> Result<usize> {
        // Query child executions that reference this workflow_execution and
        // are not yet in a terminal state. We use the workflow_task JSONB
        // field to filter.
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT workflow_task->>'task_name' as task_name \
             FROM execution \
             WHERE workflow_task->>'workflow_execution' = $1::text \
               AND status NOT IN ('completed', 'failed', 'timeout', 'cancelled')",
        )
        .bind(workflow_execution_id.to_string())
        .fetch_all(pool)
        .await?;

        let count = rows
            .iter()
            .filter(|(tn,)| !completed_tasks.contains(tn) && !failed_tasks.contains(tn))
            .count();

        Ok(count)
    }

    fn should_halt_workflow_advancement(
        workflow_status: ExecutionStatus,
        parent_status: ExecutionStatus,
        child_status: ExecutionStatus,
    ) -> bool {
        matches!(
            workflow_status,
            ExecutionStatus::Canceling | ExecutionStatus::Cancelled
        ) || matches!(
            parent_status,
            ExecutionStatus::Canceling | ExecutionStatus::Cancelled
        ) || matches!(
            child_status,
            ExecutionStatus::Canceling | ExecutionStatus::Cancelled
        )
    }

    /// Finalize a cancelled workflow by updating the parent `execution` record
    /// to `Cancelled`.  The `workflow_execution` record is already `Cancelled`
    /// (set by `cancel_workflow_children`); this only touches the parent.
    async fn finalize_cancelled_workflow(
        pool: &PgPool,
        parent_execution_id: i64,
        workflow_execution_id: i64,
    ) -> Result<()> {
        info!(
            "Finalizing cancelled workflow: parent execution {} (workflow_execution {})",
            parent_execution_id, workflow_execution_id
        );

        let update = UpdateExecutionInput {
            status: Some(ExecutionStatus::Cancelled),
            result: Some(serde_json::json!({
                "error": "Workflow cancelled",
                "succeeded": false,
            })),
            ..Default::default()
        };
        ExecutionRepository::update(pool, parent_execution_id, update).await?;

        Ok(())
    }

    /// Mark a workflow as completed (success or failure) and update both the
    /// `workflow_execution` and parent `execution` records.
    async fn complete_workflow(
        pool: &PgPool,
        parent_execution_id: i64,
        workflow_execution_id: i64,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<()> {
        let status = if success {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };

        info!(
            "Completing workflow_execution {} with status {:?} (parent execution {})",
            workflow_execution_id, status, parent_execution_id
        );

        // Update workflow_execution status
        WorkflowExecutionRepository::update(
            pool,
            workflow_execution_id,
            attune_common::repositories::workflow::UpdateWorkflowExecutionInput {
                current_tasks: Some(vec![]),
                completed_tasks: None,
                failed_tasks: None,
                skipped_tasks: None,
                variables: None,
                status: Some(status),
                error_message: error_message.map(|s| s.to_string()),
                paused: None,
                pause_reason: None,
            },
        )
        .await?;

        // Update parent execution
        let parent = ExecutionRepository::find_by_id(pool, parent_execution_id).await?;
        if let Some(mut parent) = parent {
            parent.status = status;
            parent.result = if !success {
                Some(serde_json::json!({
                    "error": error_message.unwrap_or("Workflow failed"),
                    "succeeded": false,
                }))
            } else {
                Some(serde_json::json!({
                    "succeeded": true,
                }))
            };
            ExecutionRepository::update(pool, parent.id, parent.into()).await?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Regular action scheduling helpers
    // -----------------------------------------------------------------------

    /// Get the action associated with an execution
    async fn get_action_for_execution(pool: &PgPool, execution: &Execution) -> Result<Action> {
        // Try to get action by ID first
        if let Some(action_id) = execution.action {
            if let Some(action) = ActionRepository::find_by_id(pool, action_id).await? {
                return Ok(action);
            }
        }

        // Fall back to action_ref
        ActionRepository::find_by_ref(pool, &execution.action_ref)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Action not found for execution: {}", execution.id))
    }

    /// Select an appropriate worker for the execution
    ///
    /// Uses round-robin selection among compatible, active, and healthy workers
    /// to distribute load evenly across the worker pool.
    async fn select_worker(
        pool: &PgPool,
        action: &Action,
        round_robin_counter: &AtomicUsize,
    ) -> Result<attune_common::models::Worker> {
        // Get runtime requirements for the action
        let runtime = if let Some(runtime_id) = action.runtime {
            RuntimeRepository::find_by_id(pool, runtime_id).await?
        } else {
            None
        };

        // Find available action workers (role = 'action')
        let workers = WorkerRepository::find_action_workers(pool).await?;

        if workers.is_empty() {
            return Err(anyhow::anyhow!("No action workers available"));
        }

        // Filter workers by runtime compatibility if runtime is specified
        let compatible_workers: Vec<_> = if let Some(ref runtime) = runtime {
            workers
                .into_iter()
                .filter(|w| Self::worker_supports_runtime(w, runtime))
                .collect()
        } else {
            workers
        };

        if compatible_workers.is_empty() {
            let runtime_name = runtime.as_ref().map(|r| r.name.as_str()).unwrap_or("any");
            return Err(anyhow::anyhow!(
                "No compatible workers found for action: {} (requires runtime: {})",
                action.r#ref,
                runtime_name
            ));
        }

        // Filter by worker status (only active workers)
        let active_workers: Vec<_> = compatible_workers
            .into_iter()
            .filter(|w| w.status == Some(attune_common::models::enums::WorkerStatus::Active))
            .collect();

        if active_workers.is_empty() {
            return Err(anyhow::anyhow!("No active workers available"));
        }

        // Filter by heartbeat freshness (only workers with recent heartbeats)
        let fresh_workers: Vec<_> = active_workers
            .into_iter()
            .filter(Self::is_worker_heartbeat_fresh)
            .collect();

        if fresh_workers.is_empty() {
            warn!("No workers with fresh heartbeats available. All active workers have stale heartbeats.");
            return Err(anyhow::anyhow!(
                "No workers with fresh heartbeats available (heartbeat older than {} seconds)",
                DEFAULT_HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER
            ));
        }

        // Round-robin selection: distribute executions evenly across workers.
        // Each call increments the counter and picks the next worker in the list.
        let count = round_robin_counter.fetch_add(1, Ordering::Relaxed);
        let index = count % fresh_workers.len();
        let selected = fresh_workers
            .into_iter()
            .nth(index)
            .expect("Worker list should not be empty");

        info!(
            "Selected worker {} (id={}) via round-robin (index {} of available workers)",
            selected.name, selected.id, index
        );

        Ok(selected)
    }

    /// Check if a worker supports a given runtime
    ///
    /// This checks the worker's capabilities.runtimes array against the runtime's aliases.
    /// If aliases are missing, fall back to the runtime's canonical name.
    fn worker_supports_runtime(worker: &attune_common::models::Worker, runtime: &Runtime) -> bool {
        let runtime_names = Self::runtime_capability_names(runtime);

        // Try to parse capabilities and check runtimes array
        if let Some(ref capabilities) = worker.capabilities {
            if let Some(runtimes) = capabilities.get("runtimes") {
                if let Some(runtime_array) = runtimes.as_array() {
                    // Check if any runtime in the array matches via aliases
                    for runtime_value in runtime_array {
                        if let Some(runtime_str) = runtime_value.as_str() {
                            if runtime_names
                                .iter()
                                .any(|candidate| candidate.eq_ignore_ascii_case(runtime_str))
                                || runtime_aliases_contain(&runtime.aliases, runtime_str)
                            {
                                debug!(
                                    "Worker {} supports runtime '{}' via capabilities (matched '{}', candidates: {:?})",
                                    worker.name, runtime.name, runtime_str, runtime_names
                                );
                                return true;
                            }
                        }
                    }
                }
            }
        }

        debug!(
            "Worker {} does not support runtime '{}' (candidates: {:?})",
            worker.name, runtime.name, runtime_names
        );
        false
    }

    fn runtime_capability_names(runtime: &Runtime) -> Vec<String> {
        let mut names: Vec<String> = runtime
            .aliases
            .iter()
            .map(|alias| alias.to_ascii_lowercase())
            .filter(|alias| !alias.is_empty())
            .collect();

        let runtime_name = runtime.name.to_ascii_lowercase();
        if !runtime_name.is_empty() && !names.iter().any(|name| name == &runtime_name) {
            names.push(runtime_name);
        }

        names
    }

    fn is_unschedulable_error(error: &anyhow::Error) -> bool {
        let message = error.to_string();
        message.starts_with("No compatible workers found")
            || message.starts_with("No action workers available")
            || message.starts_with("No active workers available")
            || message.starts_with("No workers with fresh heartbeats available")
    }

    async fn fail_unschedulable_execution(
        pool: &PgPool,
        publisher: &Publisher,
        envelope: &MessageEnvelope<ExecutionRequestedPayload>,
        execution_id: i64,
        action_id: i64,
        action_ref: &str,
        error_message: &str,
    ) -> Result<()> {
        let completed_at = Utc::now();
        let result = serde_json::json!({
            "error": "Execution is unschedulable",
            "message": error_message,
            "action_ref": action_ref,
            "failed_by": "execution_scheduler",
            "failed_at": completed_at.to_rfc3339(),
        });

        ExecutionRepository::update(
            pool,
            execution_id,
            UpdateExecutionInput {
                status: Some(ExecutionStatus::Failed),
                result: Some(result.clone()),
                ..Default::default()
            },
        )
        .await?;

        let completed = MessageEnvelope::new(
            MessageType::ExecutionCompleted,
            ExecutionCompletedPayload {
                execution_id,
                action_id,
                action_ref: action_ref.to_string(),
                status: "failed".to_string(),
                result: Some(result),
                completed_at,
            },
        )
        .with_correlation_id(envelope.correlation_id)
        .with_source("attune-executor");

        publisher.publish_envelope(&completed).await?;

        warn!(
            "Execution {} marked failed as unschedulable: {}",
            execution_id, error_message
        );

        Ok(())
    }

    /// Check if a worker's heartbeat is fresh enough to schedule work
    ///
    /// A worker is considered fresh if its last heartbeat is within
    /// HEARTBEAT_STALENESS_MULTIPLIER * HEARTBEAT_INTERVAL seconds.
    fn is_worker_heartbeat_fresh(worker: &attune_common::models::Worker) -> bool {
        let Some(last_heartbeat) = worker.last_heartbeat else {
            warn!(
                "Worker {} has no heartbeat recorded, considering stale",
                worker.name
            );
            return false;
        };

        let now = Utc::now();
        let age = now.signed_duration_since(last_heartbeat);
        let max_age =
            Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER);

        let is_fresh = age.to_std().unwrap_or(Duration::MAX) <= max_age;

        if !is_fresh {
            warn!(
                "Worker {} heartbeat is stale: last seen {} seconds ago (max: {} seconds)",
                worker.name,
                age.num_seconds(),
                max_age.as_secs()
            );
        } else {
            debug!(
                "Worker {} heartbeat is fresh: last seen {} seconds ago",
                worker.name,
                age.num_seconds()
            );
        }

        is_fresh
    }

    /// Queue execution to a specific worker
    async fn queue_to_worker(
        publisher: &Publisher,
        execution_id: &i64,
        worker_id: &i64,
        action_ref: &str,
        config: &Option<JsonValue>,
        _action: &Action,
    ) -> Result<()> {
        debug!("Queuing execution {} to worker {}", execution_id, worker_id);

        // Create payload for worker
        let payload = ExecutionScheduledPayload {
            execution_id: *execution_id,
            worker_id: *worker_id,
            action_ref: action_ref.to_string(),
            config: config.clone(),
        };

        let envelope =
            MessageEnvelope::new(MessageType::ExecutionRequested, payload).with_source("executor");

        // Publish to worker-specific queue with routing key
        let routing_key = format!("execution.dispatch.worker.{}", worker_id);
        let exchange = "attune.executions";

        publisher
            .publish_envelope_with_routing(&envelope, exchange, &routing_key)
            .await?;

        info!(
            "Published execution.scheduled message to worker {} (routing key: {})",
            worker_id, routing_key
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attune_common::models::{Worker, WorkerRole, WorkerStatus, WorkerType};
    use chrono::{Duration as ChronoDuration, Utc};

    fn create_test_worker(name: &str, heartbeat_offset_secs: i64) -> Worker {
        let last_heartbeat = if heartbeat_offset_secs == 0 {
            None
        } else {
            Some(Utc::now() - ChronoDuration::seconds(heartbeat_offset_secs))
        };

        Worker {
            id: 1,
            name: name.to_string(),
            worker_type: WorkerType::Local,
            worker_role: WorkerRole::Action,
            runtime: None,
            host: Some("localhost".to_string()),
            port: Some(8080),
            status: Some(WorkerStatus::Active),
            capabilities: Some(serde_json::json!({
                "runtimes": ["shell", "python"]
            })),
            meta: None,
            last_heartbeat,
            created: Utc::now(),
            updated: Utc::now(),
        }
    }

    #[test]
    fn test_heartbeat_freshness_with_recent_heartbeat() {
        // Worker with heartbeat 30 seconds ago (within limit)
        let worker = create_test_worker("test-worker", 30);
        assert!(
            ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 30s old heartbeat should be considered fresh"
        );
    }

    #[test]
    fn test_heartbeat_freshness_with_stale_heartbeat() {
        // Worker with heartbeat 100 seconds ago (beyond 3x30s = 90s limit)
        let worker = create_test_worker("test-worker", 100);
        assert!(
            !ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 100s old heartbeat should be considered stale"
        );
    }

    #[test]
    fn test_heartbeat_freshness_at_boundary() {
        // Worker with heartbeat exactly at the 90 second boundary
        let worker = create_test_worker("test-worker", 90);
        assert!(
            !ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 90s old heartbeat should be considered stale (at boundary)"
        );
    }

    #[test]
    fn test_heartbeat_freshness_with_no_heartbeat() {
        // Worker with no heartbeat recorded
        let worker = create_test_worker("test-worker", 0);
        assert!(
            !ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with no heartbeat should be considered stale"
        );
    }

    #[test]
    fn test_heartbeat_freshness_with_very_recent() {
        // Worker with heartbeat 5 seconds ago
        let worker = create_test_worker("test-worker", 5);
        assert!(
            ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 5s old heartbeat should be considered fresh"
        );
    }

    #[test]
    fn test_scheduler_creation() {
        // This is a placeholder test
        // Real tests will require database and message queue setup
    }

    #[test]
    fn test_worker_supports_runtime_with_alias_match() {
        let worker = create_test_worker("test-worker", 5);
        let runtime = Runtime {
            id: 1,
            r#ref: "core.shell".to_string(),
            pack: None,
            pack_ref: Some("core".to_string()),
            description: Some("Shell runtime".to_string()),
            name: "Shell".to_string(),
            aliases: vec!["shell".to_string(), "bash".to_string()],
            distributions: serde_json::json!({}),
            installation: None,
            installers: serde_json::json!({}),
            execution_config: serde_json::json!({}),
            auto_detected: false,
            detection_config: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        };

        assert!(ExecutionScheduler::worker_supports_runtime(
            &worker, &runtime
        ));
    }

    #[test]
    fn test_worker_supports_runtime_falls_back_to_runtime_name_when_aliases_missing() {
        let worker = create_test_worker("test-worker", 5);
        let runtime = Runtime {
            id: 1,
            r#ref: "core.shell".to_string(),
            pack: None,
            pack_ref: Some("core".to_string()),
            description: Some("Shell runtime".to_string()),
            name: "Shell".to_string(),
            aliases: vec![],
            distributions: serde_json::json!({}),
            installation: None,
            installers: serde_json::json!({}),
            execution_config: serde_json::json!({}),
            auto_detected: false,
            detection_config: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        };

        assert!(ExecutionScheduler::worker_supports_runtime(
            &worker, &runtime
        ));
    }

    #[test]
    fn test_unschedulable_error_classification() {
        assert!(ExecutionScheduler::is_unschedulable_error(
            &anyhow::anyhow!(
                "No compatible workers found for action: core.sleep (requires runtime: Shell)"
            )
        ));
        assert!(!ExecutionScheduler::is_unschedulable_error(
            &anyhow::anyhow!("database temporarily unavailable")
        ));
    }

    #[test]
    fn test_concurrency_limit_dispatch_count() {
        // Verify the dispatch_count calculation used by dispatch_with_items_task
        let total = 20usize;
        let concurrency_limit = 3usize;
        let dispatch_count = total.min(concurrency_limit);
        assert_eq!(dispatch_count, 3);

        // No concurrency limit → default to serial (1 at a time)
        let concurrency_limit = 1usize;
        let dispatch_count = total.min(concurrency_limit);
        assert_eq!(dispatch_count, 1);

        // Concurrency exceeds total → dispatch all
        let concurrency_limit = 50usize;
        let dispatch_count = total.min(concurrency_limit);
        assert_eq!(dispatch_count, 20);
    }

    #[test]
    fn test_free_slots_calculation() {
        // Simulates the free-slots logic in advance_workflow
        let concurrency_limit = 3usize;

        // 2 in-flight → 1 free slot
        let in_flight = 2usize;
        let free = concurrency_limit.saturating_sub(in_flight);
        assert_eq!(free, 1);

        // 0 in-flight → 3 free slots
        let in_flight = 0usize;
        let free = concurrency_limit.saturating_sub(in_flight);
        assert_eq!(free, 3);

        // 3 in-flight → 0 free slots
        let in_flight = 3usize;
        let free = concurrency_limit.saturating_sub(in_flight);
        assert_eq!(free, 0);
    }

    #[test]
    fn test_extract_workflow_params_flat_format() {
        let config = Some(serde_json::json!({"n": 5, "name": "test"}));
        let params = extract_workflow_params(&config);
        assert_eq!(params, serde_json::json!({"n": 5, "name": "test"}));
    }

    #[test]
    fn test_extract_workflow_params_none() {
        let params = extract_workflow_params(&None);
        assert_eq!(params, serde_json::json!({}));
    }

    #[test]
    fn test_extract_workflow_params_non_object() {
        let config = Some(serde_json::json!("not an object"));
        let params = extract_workflow_params(&config);
        assert_eq!(params, serde_json::json!({}));
    }

    #[test]
    fn test_extract_workflow_params_empty_object() {
        let config = Some(serde_json::json!({}));
        let params = extract_workflow_params(&config);
        assert_eq!(params, serde_json::json!({}));
    }

    #[test]
    fn test_extract_workflow_params_with_parameters_key() {
        // A "parameters" key is just a regular parameter — not unwrapped
        let config = Some(serde_json::json!({
            "parameters": {"n": 5},
            "context": {"rule": "test"}
        }));
        let params = extract_workflow_params(&config);
        // Returns the whole object as-is — "parameters" is treated as a normal key
        assert_eq!(
            params,
            serde_json::json!({"parameters": {"n": 5}, "context": {"rule": "test"}})
        );
    }

    #[test]
    fn test_scheduling_persists_selected_worker() {
        let mut execution = attune_common::models::Execution {
            id: 42,
            action: Some(7),
            action_ref: "core.sleep".to_string(),
            config: None,
            env_vars: None,
            parent: None,
            enforcement: None,
            executor: None,
            worker: None,
            status: ExecutionStatus::Requested,
            result: None,
            started_at: None,
            workflow_task: None,
            created: Utc::now(),
            updated: Utc::now(),
        };

        execution.status = ExecutionStatus::Scheduled;
        execution.worker = Some(99);

        let update: UpdateExecutionInput = execution.into();
        assert_eq!(update.status, Some(ExecutionStatus::Scheduled));
        assert_eq!(update.worker, Some(99));
    }

    #[test]
    fn test_workflow_advancement_halts_for_any_cancellation_state() {
        assert!(ExecutionScheduler::should_halt_workflow_advancement(
            ExecutionStatus::Running,
            ExecutionStatus::Canceling,
            ExecutionStatus::Completed
        ));
        assert!(ExecutionScheduler::should_halt_workflow_advancement(
            ExecutionStatus::Cancelled,
            ExecutionStatus::Running,
            ExecutionStatus::Failed
        ));
        assert!(ExecutionScheduler::should_halt_workflow_advancement(
            ExecutionStatus::Running,
            ExecutionStatus::Running,
            ExecutionStatus::Cancelled
        ));
        assert!(!ExecutionScheduler::should_halt_workflow_advancement(
            ExecutionStatus::Running,
            ExecutionStatus::Running,
            ExecutionStatus::Failed
        ));
    }
}
