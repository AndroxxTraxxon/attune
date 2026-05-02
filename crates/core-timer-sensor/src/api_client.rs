//! API Client for Attune Platform
//!
//! Provides methods for interacting with the Attune API, including:
//! - Health checks
//! - Event creation
//! - Rule fetching

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// API client for communicating with Attune
#[derive(Clone)]
pub struct ApiClient {
    inner: Arc<ApiClientInner>,
}

struct ApiClientInner {
    base_url: String,
    token: RwLock<String>,
    client: Client,
}

/// Request to create an event
#[derive(Debug, Clone, Serialize)]
pub struct CreateEventRequest {
    pub trigger_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_instance_id: Option<String>,
}

/// Response from creating an event
#[derive(Debug, Deserialize)]
pub struct CreateEventResponse {
    pub data: EventData,
}

#[derive(Debug, Deserialize)]
pub struct EventData {
    pub id: i64,
}

/// Response wrapper for API responses (supports both `data` and `items` keys)
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    #[serde(alias = "items")]
    pub data: T,
}

/// Rule information from API
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub id: i64,
    #[serde(default)]
    pub trigger_params: serde_json::Value,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Response from token refresh
#[derive(Debug, Deserialize)]
pub struct RefreshTokenResponse {
    pub token: String,
    pub expires_at: String,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String, token: String) -> Self {
        // Remove trailing slash from base URL if present
        let base_url = base_url.trim_end_matches('/').to_string();

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            inner: Arc::new(ApiClientInner {
                base_url,
                token: RwLock::new(token),
                client,
            }),
        }
    }

    /// Get the current token (for reading)
    pub async fn get_token(&self) -> String {
        self.inner.token.read().await.clone()
    }

    /// Update the token (for refresh)
    async fn set_token(&self, new_token: String) {
        let mut token = self.inner.token.write().await;
        *token = new_token;
    }

    /// Perform health check
    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/health", self.inner.base_url);

        debug!("Health check: GET {}", url);

        let response = self
            .inner
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send health check request")?;

        if response.status().is_success() {
            info!("Health check succeeded");
            Ok(())
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unable to read response>".to_string());
            error!("Health check failed: {} - {}", status, body);
            Err(anyhow::anyhow!("Health check failed: {}", status))
        }
    }

    /// Create an event
    pub async fn create_event(&self, request: CreateEventRequest) -> Result<i64> {
        let url = format!("{}/api/v1/events", self.inner.base_url);

        debug!(
            "Creating event: POST {} (trigger_ref={})",
            url, request.trigger_ref
        );

        let token = self.get_token().await;
        let response = self
            .inner
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send create event request")?;

        let status = response.status();

        if status.is_success() {
            let event_response: CreateEventResponse = response
                .json()
                .await
                .context("Failed to parse create event response")?;

            info!(
                "Event created successfully: id={}, trigger_ref={}",
                event_response.data.id, request.trigger_ref
            );

            Ok(event_response.data.id)
        } else {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unable to read response>".to_string());

            error!("Failed to create event: {} - {}", status, body);

            // Special handling for 403 Forbidden (trigger type not allowed)
            if status == StatusCode::FORBIDDEN {
                return Err(anyhow::anyhow!(
                    "Insufficient permissions to create event for trigger ref '{}'. \
                     This sensor token may not be authorized for this trigger type.",
                    request.trigger_ref
                ));
            }

            Err(anyhow::anyhow!(
                "Failed to create event: {} - {}",
                status,
                body
            ))
        }
    }

    /// Fetch active rules for a specific trigger reference
    pub async fn fetch_rules(&self, trigger_ref: &str) -> Result<Vec<Rule>> {
        let url = format!(
            "{}/api/v1/triggers/{}/rules",
            self.inner.base_url,
            urlencoding::encode(trigger_ref)
        );

        debug!("Fetching rules: GET {}", url);

        let token = self.get_token().await;
        let response = self
            .inner
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to send fetch rules request")?;

        let status = response.status();

        if status.is_success() {
            let api_response: ApiResponse<Vec<Rule>> = response
                .json()
                .await
                .context("Failed to parse fetch rules response")?;

            info!(
                "Fetched {} rules for trigger ref {}",
                api_response.data.len(),
                trigger_ref
            );

            Ok(api_response.data)
        } else {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unable to read response>".to_string());

            warn!("Failed to fetch rules: {} - {}", status, body);

            Err(anyhow::anyhow!(
                "Failed to fetch rules: {} - {}",
                status,
                body
            ))
        }
    }

    /// Create event with retry logic
    pub async fn create_event_with_retry(&self, request: CreateEventRequest) -> Result<i64> {
        const MAX_RETRIES: u32 = 3;
        const INITIAL_BACKOFF_MS: u64 = 100;

        let mut attempt = 0;
        let mut last_error = None;

        while attempt < MAX_RETRIES {
            match self.create_event(request.clone()).await {
                Ok(event_id) => return Ok(event_id),
                Err(e) => {
                    // Don't retry on 403 Forbidden (authorization error)
                    if e.to_string().contains("Insufficient permissions") {
                        return Err(e);
                    }

                    attempt += 1;
                    last_error = Some(e);

                    if attempt < MAX_RETRIES {
                        let backoff_ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1);
                        warn!(
                            "Event creation failed (attempt {}/{}), retrying in {}ms",
                            attempt, MAX_RETRIES, backoff_ms
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Event creation failed after retries")))
    }

    /// Refresh the current token
    pub async fn refresh_token(&self) -> Result<String> {
        let url = format!("{}/api/v1/auth/refresh", self.inner.base_url);

        debug!("Refreshing token: POST {}", url);

        let current_token = self.get_token().await;
        let response = self
            .inner
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", current_token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("Failed to send token refresh request")?;

        let status = response.status();

        if status.is_success() {
            let refresh_response: RefreshTokenResponse = response
                .json()
                .await
                .context("Failed to parse token refresh response")?;

            info!(
                "Token refreshed successfully, expires at: {}",
                refresh_response.expires_at
            );

            // Update stored token
            self.set_token(refresh_response.token.clone()).await;

            Ok(refresh_response.token)
        } else {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unable to read response>".to_string());

            error!("Failed to refresh token: {} - {}", status, body);

            Err(anyhow::anyhow!(
                "Failed to refresh token: {} - {}",
                status,
                body
            ))
        }
    }
}

impl CreateEventRequest {
    /// Create a new event request
    pub fn new(trigger_ref: String, payload: serde_json::Value) -> Self {
        Self {
            trigger_ref,
            payload: Some(payload),
            config: None,
            trigger_instance_id: None,
        }
    }

    /// Set trigger instance ID (typically rule_id)
    pub fn with_trigger_instance_id(mut self, id: String) -> Self {
        self.trigger_instance_id = Some(id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_event_request() {
        let payload = serde_json::json!({
            "timestamp": "2025-01-27T12:34:56Z",
            "scheduled_time": "2025-01-27T12:34:56Z"
        });

        let request = CreateEventRequest::new("core.timer".to_string(), payload.clone());

        assert_eq!(request.trigger_ref, "core.timer");
        assert_eq!(request.payload, Some(payload));
        assert!(request.trigger_instance_id.is_none());
    }

    #[test]
    fn test_create_event_request_with_instance_id() {
        let payload = serde_json::json!({
            "timestamp": "2025-01-27T12:34:56Z"
        });

        let request = CreateEventRequest::new("core.timer".to_string(), payload)
            .with_trigger_instance_id("rule_123".to_string());

        assert_eq!(request.trigger_instance_id, Some("rule_123".to_string()));
    }

    #[test]
    fn test_base_url_trailing_slash_removed() {
        let client = ApiClient::new("http://localhost:8080/".to_string(), "token".to_string());
        assert_eq!(client.inner.base_url, "http://localhost:8080");
    }
}
