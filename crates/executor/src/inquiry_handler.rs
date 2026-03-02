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
    models::{enums::InquiryStatus, inquiry::Inquiry, Execution, Id},
    mq::{
        Consumer, InquiryCreatedPayload, InquiryRespondedPayload, MessageEnvelope, MessageType,
        Publisher,
    },
    repositories::{
        execution::{ExecutionRepository, UpdateExecutionInput},
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

    /// Create an inquiry for an execution and pause it
    pub async fn create_inquiry_from_result(
        pool: &PgPool,
        publisher: &Publisher,
        execution_id: Id,
        result: &JsonValue,
    ) -> Result<Inquiry> {
        info!("Creating inquiry for execution {}", execution_id);

        // Extract inquiry request
        let inquiry_request = Self::extract_inquiry_request(result)?;

        // Calculate timeout if specified
        let timeout_at = inquiry_request
            .timeout_seconds
            .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds));

        // Create inquiry in database
        let inquiry_input = CreateInquiryInput {
            execution: execution_id,
            prompt: inquiry_request.prompt.clone(),
            response_schema: inquiry_request.response_schema.clone(),
            assigned_to: inquiry_request.assigned_to,
            status: InquiryStatus::Pending,
            response: None,
            timeout_at,
        };

        let inquiry = InquiryRepository::create(pool, inquiry_input).await?;

        info!(
            "Created inquiry {} for execution {}",
            inquiry.id, execution_id
        );

        // Update execution status to paused/waiting
        // Note: We use a special status or keep it as "running" with inquiry tracking
        // For now, we'll keep status as-is and track via inquiry relationship

        // Publish InquiryCreated message
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

        debug!(
            "Published InquiryCreated message for inquiry {}",
            inquiry.id
        );

        Ok(inquiry)
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
        _publisher: &Publisher,
        execution: &Execution,
        inquiry: &Inquiry,
        response: &JsonValue,
    ) -> Result<()> {
        info!(
            "Resuming execution {} with inquiry {} response",
            execution.id, inquiry.id
        );

        // Update execution result to include inquiry response
        let mut updated_result = execution
            .result
            .clone()
            .unwrap_or(JsonValue::Object(Default::default()));

        // Add inquiry response to result
        if let Some(obj) = updated_result.as_object_mut() {
            obj.insert("__inquiry_response".to_string(), response.clone());
            obj.insert(
                "__inquiry_id".to_string(),
                JsonValue::Number(inquiry.id.into()),
            );
        }

        // Update execution with new result
        let update_input = UpdateExecutionInput {
            status: None, // Keep current status, let worker handle completion
            result: Some(updated_result),
            ..Default::default()
        };

        ExecutionRepository::update(pool, execution.id, update_input).await?;

        info!(
            "Updated execution {} with inquiry response, execution can now continue",
            execution.id
        );

        // NOTE: In a full implementation, we would:
        // 1. Re-queue the execution for processing
        // 2. Or have the worker check for inquiry responses
        // 3. Or implement a more sophisticated state machine

        // For now, the execution is marked complete with the inquiry response
        // The calling code can check for __inquiry_response in the result

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

    /// Periodic task to check and handle inquiry timeouts
    pub async fn timeout_check_loop(pool: PgPool, interval_seconds: u64) {
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
