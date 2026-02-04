//! API Client for Sensor Service
//!
//! This module provides an HTTP client for the sensor service to communicate
//! with the Attune API for token provisioning and other operations.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// API client for sensor service
#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: Client,
    /// Optional admin token for authentication (if available)
    admin_token: Option<String>,
}

/// Request to create a sensor token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSensorTokenRequest {
    pub sensor_ref: String,
    pub trigger_types: Vec<String>,
    pub ttl_seconds: Option<i64>,
}

/// Response from sensor token creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorTokenResponse {
    pub identity_id: i64,
    pub sensor_ref: String,
    pub token: String,
    pub expires_at: String,
    pub trigger_types: Vec<String>,
}

/// Wrapper for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String, admin_token: Option<String>) -> Self {
        Self {
            base_url,
            client: Client::new(),
            admin_token,
        }
    }

    /// Create a sensor token via the API
    ///
    /// This is used internally by the sensor service to provision tokens
    /// for standalone sensors when they are started.
    pub async fn create_sensor_token(
        &self,
        sensor_ref: &str,
        trigger_types: Vec<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<SensorTokenResponse> {
        let url = format!("{}/auth/internal/sensor-token", self.base_url);

        let request = CreateSensorTokenRequest {
            sensor_ref: sensor_ref.to_string(),
            trigger_types,
            ttl_seconds,
        };

        let mut req = self.client.post(&url).json(&request);

        // Add authorization header if admin token is available
        if let Some(token) = &self.admin_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let response = req
            .send()
            .await
            .context("Failed to send sensor token creation request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "API request failed with status {}: {}",
                status,
                body
            ));
        }

        let api_response: ApiResponse<SensorTokenResponse> = response
            .json()
            .await
            .context("Failed to parse sensor token response")?;

        Ok(api_response.data)
    }

    /// Health check endpoint
    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/health", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send health check request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Health check failed with status: {}",
                response.status()
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_creation() {
        let client = ApiClient::new("http://localhost:8080".to_string(), None);
        assert_eq!(client.base_url, "http://localhost:8080");
    }

    #[test]
    fn test_api_client_with_token() {
        let client = ApiClient::new(
            "http://localhost:8080".to_string(),
            Some("test_token".to_string()),
        );
        assert_eq!(client.admin_token, Some("test_token".to_string()));
    }
}
