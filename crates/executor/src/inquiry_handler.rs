//! Inquiry Handler - Manages inquiry lifecycle and execution pausing/resuming
//!
//! This module handles:
//! - Creating inquiries from action results
//! - Pausing executions waiting for inquiry responses
//! - Listening for InquiryResponded messages
//! - Resuming executions with inquiry responses
//! - Handling inquiry timeouts

use anyhow::Result;
use attune_common::{
    error::Error as AttuneError,
    models::{
        enums::{ExecutionStatus, InquiryStatus},
        inquiry::Inquiry,
        Execution, Id,
    },
    mq::{
        Consumer, ExecutionCompletedPayload, InquiryCreatedPayload, InquiryRespondedPayload,
        MessageEnvelope, MessageType, Publisher,
    },
    repositories::{
        execution::{ExecutionRepository, UpdateExecutionInput, SELECT_COLUMNS},
        inquiry::{CreateInquiryInput, InquiryRepository},
        Create, FindById, Update,
    },
};
use chrono::Utc;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Special key in action result to indicate an inquiry should be created
pub const INQUIRY_RESULT_KEY: &str = "__inquiry";
const INQUIRY_ID_RESULT_KEY: &str = "__inquiry_id";
const INQUIRY_CREATED_PUBLISHED_RESULT_KEY: &str = "__inquiry_created_published";

/// Structure for inquiry data in action results
#[derive(Debug, Clone, serde::Deserialize)]
pub struct InquiryRequest {
    /// Prompt text for the user
    pub prompt: String,
    /// Optional JSON schema for expected response
    #[serde(default)]
    pub response_schema: Option<JsonValue>,
    /// Optional user/identity to assign inquiry to
    #[serde(default)]
    pub assigned_to: Option<Id>,
    /// Optional timeout in seconds from now
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

/// Inquiry handler manages the inquiry lifecycle
pub struct InquiryHandler {
    pool: PgPool,
    publisher: Arc<Publisher>,
    consumer: Arc<Consumer>,
}

impl InquiryHandler {
    /// Create a new inquiry handler
    pub fn new(pool: PgPool, publisher: Arc<Publisher>, consumer: Arc<Consumer>) -> Self {
        Self {
            pool,
            publisher,
            consumer,
        }
    }

    /// Start listening for InquiryResponded messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting inquiry handler");

        let pool = self.pool.clone();
        let publisher = self.publisher.clone();

        // Listen for inquiry responded messages
        self.consumer
            .consume_with_handler(move |envelope: MessageEnvelope<InquiryRespondedPayload>| {
                let pool = pool.clone();
                let publisher = publisher.clone();

                async move {
                    if let Err(e) =
                        Self::handle_inquiry_response(&pool, &publisher, &envelope).await
                    {
                        error!("Error handling inquiry response: {}", e);
                        return Err(format!("Failed to handle inquiry response: {}", e).into());
                    }
                    Ok(())
                }
            })
            .await?;

        Ok(())
    }

    /// Check if an execution result contains an inquiry request
    pub fn has_inquiry_request(result: &JsonValue) -> bool {
        result.get(INQUIRY_RESULT_KEY).is_some()
    }

    /// Extract inquiry request from execution result
    pub fn extract_inquiry_request(result: &JsonValue) -> Result<InquiryRequest> {
        let inquiry_value = result
            .get(INQUIRY_RESULT_KEY)
            .ok_or_else(|| anyhow::anyhow!("No inquiry request found in result"))?;

        let inquiry_request: InquiryRequest = serde_json::from_value(inquiry_value.clone())?;
        Ok(inquiry_request)
    }
}

/// Returns true when `e` represents a PostgreSQL unique constraint violation (code 23505).
fn is_db_unique_violation(e: &AttuneError) -> bool {
    if let AttuneError::Database(sqlx_err) = e {
        return sqlx_err
            .as_database_error()
            .and_then(|db| db.code())
            .as_deref()
            == Some("23505");
    }
    false
}

impl InquiryHandler {
    /// Create an inquiry for an execution and pause it
    pub async fn create_inquiry_from_result(
        pool: &PgPool,
        publisher: &Publisher,
        execution_id: Id,
        _result: &JsonValue,
    ) -> Result<Inquiry> {
        info!("Creating inquiry for execution {}", execution_id);

        let mut tx = pool.begin().await?;
        let execution = sqlx::query_as::<_, Execution>(&format!(
            "SELECT {SELECT_COLUMNS} FROM execution WHERE id = $1 FOR UPDATE"
        ))
        .bind(execution_id)
        .fetch_one(&mut *tx)
        .await?;

        let mut result = execution
            .result
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Execution {} has no result", execution_id))?;
        let inquiry_request = Self::extract_inquiry_request(&result)?;
        let timeout_at = inquiry_request
            .timeout_seconds
            .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds));

        let existing_inquiry_id = result
            .get(INQUIRY_ID_RESULT_KEY)
            .and_then(|value| value.as_i64());
        let published = result
            .get(INQUIRY_CREATED_PUBLISHED_RESULT_KEY)
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        let (inquiry, should_publish) = if let Some(inquiry_id) = existing_inquiry_id {
            let inquiry = InquiryRepository::find_by_id(&mut *tx, inquiry_id)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Inquiry {} referenced by execution {} result not found",
                        inquiry_id,
                        execution_id
                    )
                })?;
            let should_publish = !published && inquiry.status == InquiryStatus::Pending;
            (inquiry, should_publish)
        } else {
            let create_result = InquiryRepository::create(
                &mut *tx,
                CreateInquiryInput {
                    execution: execution_id,
                    prompt: inquiry_request.prompt.clone(),
                    response_schema: inquiry_request.response_schema.clone(),
                    assigned_to: inquiry_request.assigned_to,
                    status: InquiryStatus::Pending,
                    response: None,
                    timeout_at,
                },
            )
            .await;

            let inquiry = match create_result {
                Ok(inq) => inq,
                Err(e) => {
                    // Unique constraint violation (23505): another replica already
                    // created the inquiry for this execution. Treat as idempotent
                    // success — drop the aborted transaction and return the existing row.
                    if is_db_unique_violation(&e) {
                        info!(
                            "Inquiry for execution {} already created by another replica \
                             (unique constraint 23505); treating as idempotent",
                            execution_id
                        );
                        // tx is in an aborted state; dropping it issues ROLLBACK.
                        drop(tx);
                        let inquiries =
                            InquiryRepository::find_by_execution(pool, execution_id).await?;
                        let existing = inquiries.into_iter().next().ok_or_else(|| {
                            anyhow::anyhow!(
                                "Inquiry for execution {} not found after unique constraint violation",
                                execution_id
                            )
                        })?;
                        return Ok(existing);
                    }
                    return Err(e.into());
                }
            };

            Self::set_inquiry_result_metadata(&mut result, inquiry.id, false)?;
            ExecutionRepository::update(
                &mut *tx,
                execution_id,
                UpdateExecutionInput {
                    result: Some(result),
                    ..Default::default()
                },
            )
            .await?;

            (inquiry, true)
        };

        tx.commit().await?;

        if should_publish {
            let payload = InquiryCreatedPayload {
                inquiry_id: inquiry.id,
                execution_id,
                prompt: inquiry_request.prompt,
                response_schema: inquiry_request.response_schema,
                assigned_to: inquiry_request.assigned_to,
                timeout_at,
            };

            let envelope =
                MessageEnvelope::new(MessageType::InquiryCreated, payload).with_source("executor");

            publisher.publish_envelope(&envelope).await?;
            Self::mark_inquiry_created_published(pool, execution_id).await?;

            debug!(
                "Published InquiryCreated message for inquiry {}",
                inquiry.id
            );
        }

        Ok(inquiry)
    }

    fn set_inquiry_result_metadata(
        result: &mut JsonValue,
        inquiry_id: Id,
        published: bool,
    ) -> Result<()> {
        let obj = result
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("execution result is not a JSON object"))?;

        obj.insert(
            INQUIRY_ID_RESULT_KEY.to_string(),
            JsonValue::Number(inquiry_id.into()),
        );
        obj.insert(
            INQUIRY_CREATED_PUBLISHED_RESULT_KEY.to_string(),
            JsonValue::Bool(published),
        );
        Ok(())
    }

    async fn mark_inquiry_created_published(pool: &PgPool, execution_id: Id) -> Result<()> {
        let execution = ExecutionRepository::find_by_id(pool, execution_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Execution {} not found", execution_id))?;
        let mut result = execution
            .result
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Execution {} has no result", execution_id))?;
        let inquiry_id = result
            .get(INQUIRY_ID_RESULT_KEY)
            .and_then(|value| value.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Execution {} missing __inquiry_id", execution_id))?;

        Self::set_inquiry_result_metadata(&mut result, inquiry_id, true)?;
        ExecutionRepository::update(
            pool,
            execution_id,
            UpdateExecutionInput {
                result: Some(result),
                ..Default::default()
            },
        )
        .await?;

        Ok(())
    }

    /// Handle an inquiry response message
    async fn handle_inquiry_response(
        pool: &PgPool,
        publisher: &Publisher,
        envelope: &MessageEnvelope<InquiryRespondedPayload>,
    ) -> Result<()> {
        let payload = &envelope.payload;

        info!(
            "Handling inquiry response for inquiry {} (execution {})",
            payload.inquiry_id, payload.execution_id
        );

        // Fetch the inquiry to verify it exists and is in correct state
        let inquiry = InquiryRepository::find_by_id(pool, payload.inquiry_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Inquiry {} not found", payload.inquiry_id))?;

        // Verify inquiry is responded (should already be updated by API)
        if inquiry.status != InquiryStatus::Responded {
            warn!(
                "Inquiry {} is not in responded state (current: {:?}), skipping resume",
                payload.inquiry_id, inquiry.status
            );
            return Ok(());
        }

        // Fetch the execution
        let execution = ExecutionRepository::find_by_id(pool, payload.execution_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Execution {} not found", payload.execution_id))?;

        // Resume the execution with the inquiry response
        Self::resume_execution_with_response(
            pool,
            publisher,
            &execution,
            &inquiry,
            &payload.response,
        )
        .await?;

        Ok(())
    }

    /// Resume an execution with inquiry response data
    async fn resume_execution_with_response(
        pool: &PgPool,
        publisher: &Publisher,
        execution: &Execution,
        inquiry: &Inquiry,
        response: &JsonValue,
    ) -> Result<()> {
        info!(
            "Resuming execution {} with inquiry {} response",
            execution.id, inquiry.id
        );

        let updated_result = serde_json::json!({
            "response": response,
            INQUIRY_ID_RESULT_KEY: inquiry.id,
            INQUIRY_CREATED_PUBLISHED_RESULT_KEY: true
        });

        let update_input = UpdateExecutionInput {
            status: Some(ExecutionStatus::Completed),
            result: Some(updated_result),
            ..Default::default()
        };

        let updated_execution =
            ExecutionRepository::update(pool, execution.id, update_input).await?;

        let payload = ExecutionCompletedPayload {
            execution_id: updated_execution.id,
            action_id: updated_execution.action.unwrap_or_default(),
            action_ref: updated_execution.action_ref.clone(),
            status: "completed".to_string(),
            result: updated_execution.result.clone(),
            completed_at: Utc::now(),
        };
        let envelope = MessageEnvelope::new(MessageType::ExecutionCompleted, payload)
            .with_source("executor-inquiry");
        publisher.publish_envelope(&envelope).await?;

        info!("Completed execution {} with inquiry response", execution.id);

        Ok(())
    }

    /// Check for timed out inquiries and mark them accordingly
    pub async fn check_inquiry_timeouts(pool: &PgPool) -> Result<Vec<Id>> {
        debug!("Checking for timed out inquiries");

        // Query for pending inquiries with expired timeouts
        let timed_out = sqlx::query_as::<_, Inquiry>(
            r#"
            UPDATE inquiry
            SET status = 'timeout', updated = NOW()
            WHERE status = 'pending'
                AND timeout_at IS NOT NULL
                AND timeout_at < NOW()
            RETURNING id, execution, prompt, response_schema, assigned_to, status,
                      response, timeout_at, responded_at, created, updated
            "#,
        )
        .fetch_all(pool)
        .await?;

        let count = timed_out.len();
        if count > 0 {
            info!("Marked {} inquiries as timed out", count);

            let ids: Vec<Id> = timed_out.iter().map(|i| i.id).collect();

            // TODO: Optionally publish timeout messages or update executions
            // For now, just return the IDs

            return Ok(ids);
        }

        Ok(vec![])
    }

    async fn finalize_timed_out_inquiry_executions(
        pool: &PgPool,
        publisher: &Publisher,
    ) -> Result<Vec<Id>> {
        let inquiries = sqlx::query_as::<_, Inquiry>(
            "SELECT id, execution, prompt, response_schema, assigned_to, status, \
                    response, timeout_at, responded_at, created, updated \
             FROM inquiry \
             WHERE status = 'timeout'",
        )
        .fetch_all(pool)
        .await?;

        let mut finalized = Vec::new();
        for inquiry in inquiries {
            let Some(execution) = ExecutionRepository::find_by_id(pool, inquiry.execution).await?
            else {
                continue;
            };
            if execution.status != ExecutionStatus::Running {
                continue;
            }

            let mut workflow_task = execution.workflow_task.clone();
            if let Some(metadata) = workflow_task.as_mut() {
                metadata.timed_out = true;
                metadata.completed_at = Some(Utc::now());
            }
            let result = serde_json::json!({
                "error": "inquiry timed out",
                INQUIRY_ID_RESULT_KEY: inquiry.id
            });
            let updated_execution = ExecutionRepository::update(
                pool,
                execution.id,
                UpdateExecutionInput {
                    status: Some(ExecutionStatus::Timeout),
                    result: Some(result),
                    workflow_task,
                    ..Default::default()
                },
            )
            .await?;

            let payload = ExecutionCompletedPayload {
                execution_id: updated_execution.id,
                action_id: updated_execution.action.unwrap_or_default(),
                action_ref: updated_execution.action_ref.clone(),
                status: "timeout".to_string(),
                result: updated_execution.result.clone(),
                completed_at: Utc::now(),
            };
            let envelope = MessageEnvelope::new(MessageType::ExecutionCompleted, payload)
                .with_source("executor-inquiry-timeout");
            publisher.publish_envelope(&envelope).await?;
            finalized.push(inquiry.id);
        }

        Ok(finalized)
    }

    /// Periodic task to check and handle inquiry timeouts
    pub async fn timeout_check_loop(
        pool: PgPool,
        publisher: Arc<Publisher>,
        interval_seconds: u64,
    ) {
        info!(
            "Starting inquiry timeout check loop (interval: {}s)",
            interval_seconds
        );

        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_seconds));

        loop {
            interval.tick().await;

            match Self::check_inquiry_timeouts(&pool).await {
                Ok(timed_out) if !timed_out.is_empty() => {
                    info!(
                        "Found {} timed out inquiries: {:?}",
                        timed_out.len(),
                        timed_out
                    );
                }
                Err(e) => {
                    error!("Error checking inquiry timeouts: {}", e);
                }
                _ => {}
            }

            match Self::finalize_timed_out_inquiry_executions(&pool, &publisher).await {
                Ok(finalized) if !finalized.is_empty() => {
                    info!("Finalized {} timed out inquiry executions", finalized.len());
                }
                Err(e) => {
                    error!("Error finalizing timed out inquiry executions: {}", e);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_has_inquiry_request() {
        let result_with_inquiry = json!({
            "__inquiry": {
                "prompt": "Approve?",
            },
            "data": "some data"
        });

        let result_without_inquiry = json!({
            "data": "some data"
        });

        assert!(InquiryHandler::has_inquiry_request(&result_with_inquiry));
        assert!(!InquiryHandler::has_inquiry_request(
            &result_without_inquiry
        ));
    }

    #[test]
    fn test_extract_inquiry_request() {
        let result = json!({
            "__inquiry": {
                "prompt": "Approve deployment?",
                "response_schema": {"type": "boolean"},
                "timeout_seconds": 3600
            }
        });

        let inquiry = InquiryHandler::extract_inquiry_request(&result).unwrap();
        assert_eq!(inquiry.prompt, "Approve deployment?");
        assert_eq!(inquiry.timeout_seconds, Some(3600));
    }

    #[test]
    fn test_extract_inquiry_request_minimal() {
        let result = json!({
            "__inquiry": {
                "prompt": "Continue?"
            }
        });

        let inquiry = InquiryHandler::extract_inquiry_request(&result).unwrap();
        assert_eq!(inquiry.prompt, "Continue?");
        assert_eq!(inquiry.response_schema, None);
        assert_eq!(inquiry.assigned_to, None);
        assert_eq!(inquiry.timeout_seconds, None);
    }

    #[test]
    fn test_extract_inquiry_request_missing() {
        let result = json!({"data": "value"});
        assert!(InquiryHandler::extract_inquiry_request(&result).is_err());
    }
}
