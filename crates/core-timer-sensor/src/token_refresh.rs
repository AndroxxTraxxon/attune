//! Token Refresh Manager
//!
//! Automatically refreshes sensor tokens before they expire to enable
//! zero-downtime operation without manual intervention.
//!
//! Refresh Strategy:
//! - Token TTL: 90 days
//! - Refresh threshold: 80% of TTL (72 days)
//! - Check interval: 1 hour
//! - Retry on failure: Exponential backoff (1min, 2min, 4min, 8min, max 1 hour)

use crate::api_client::ApiClient;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

/// Token refresh manager
pub struct TokenRefreshManager {
    api_client: ApiClient,
    refresh_threshold: f64,
}

/// JWT claims for decoding token expiration
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    #[serde(default)]
    exp: i64,
    #[serde(default)]
    iat: i64,
    #[serde(default)]
    sub: String,
}

impl TokenRefreshManager {
    /// Create a new token refresh manager
    ///
    /// # Arguments
    /// * `api_client` - API client with the current token
    /// * `refresh_threshold` - Percentage of TTL before refreshing (e.g., 0.8 for 80%)
    pub fn new(api_client: ApiClient, refresh_threshold: f64) -> Self {
        Self {
            api_client,
            refresh_threshold,
        }
    }

    /// Start the token refresh background task
    ///
    /// This spawns a tokio task that:
    /// 1. Checks token expiration every hour
    /// 2. Refreshes when threshold reached (e.g., 80% of TTL)
    /// 3. Retries on failure with exponential backoff
    /// 4. Logs all refresh events
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!(
                "Token refresh manager started (threshold: {}%)",
                self.refresh_threshold * 100.0
            );

            let mut retry_delay = Duration::from_secs(60); // Start with 1 minute
            let max_retry_delay = Duration::from_secs(3600); // Max 1 hour
            let check_interval = Duration::from_secs(3600); // Check every hour

            loop {
                match self.check_and_refresh().await {
                    Ok(RefreshStatus::Refreshed) => {
                        info!("Token refresh successful");
                        retry_delay = Duration::from_secs(60); // Reset retry delay
                        sleep(check_interval).await;
                    }
                    Ok(RefreshStatus::NotNeeded) => {
                        debug!("Token refresh not needed yet");
                        retry_delay = Duration::from_secs(60); // Reset retry delay
                        sleep(check_interval).await;
                    }
                    Err(e) => {
                        error!("Token refresh failed: {}", e);
                        warn!("Retrying token refresh in {:?}", retry_delay);
                        sleep(retry_delay).await;

                        // Exponential backoff with max limit
                        retry_delay = std::cmp::min(retry_delay * 2, max_retry_delay);
                    }
                }
            }
        })
    }

    /// Check if token needs refresh and refresh if necessary
    async fn check_and_refresh(&self) -> Result<RefreshStatus> {
        let token = self.api_client.get_token().await;

        // Decode token to get expiration
        let claims = self.decode_token(&token)?;

        let now = Utc::now().timestamp();
        let ttl = claims.exp - claims.iat;
        let refresh_at = claims.iat + ((ttl as f64) * self.refresh_threshold) as i64;

        debug!(
            "Token check: iat={}, exp={}, ttl={}s, refresh_at={}, now={}",
            claims.iat, claims.exp, ttl, refresh_at, now
        );

        if now >= refresh_at {
            let time_until_expiry = claims.exp - now;
            info!(
                "Token refresh threshold reached, refreshing (expires in {} seconds)",
                time_until_expiry
            );

            // Refresh the token
            self.api_client.refresh_token().await?;

            Ok(RefreshStatus::Refreshed)
        } else {
            let time_until_refresh = refresh_at - now;
            let time_until_expiry = claims.exp - now;

            debug!(
                "Token still valid, refresh in {} seconds (expires in {} seconds)",
                time_until_refresh, time_until_expiry
            );

            Ok(RefreshStatus::NotNeeded)
        }
    }

    /// Decode JWT token to extract claims
    fn decode_token(&self, token: &str) -> Result<JwtClaims> {
        // JWT format: header.payload.signature
        let parts: Vec<&str> = token.split('.').collect();

        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid JWT format: expected 3 parts"));
        }

        // Decode base64 payload
        let payload = parts[1];
        let decoded = general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .or_else(|_| general_purpose::STANDARD.decode(payload))
            .map_err(|e| anyhow::anyhow!("Failed to decode JWT payload: {}", e))?;

        // Parse JSON
        let claims: JwtClaims = serde_json::from_slice(&decoded)
            .map_err(|e| anyhow::anyhow!("Failed to parse JWT claims: {}", e))?;

        Ok(claims)
    }

    /// Get token expiration time
    #[allow(dead_code)]
    pub async fn get_token_expiration(&self) -> Result<DateTime<Utc>> {
        let token = self.api_client.get_token().await;
        let claims = self.decode_token(&token)?;

        let expiration = DateTime::from_timestamp(claims.exp, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid expiration timestamp"))?;

        Ok(expiration)
    }
}

/// Result of a refresh check
#[derive(Debug)]
enum RefreshStatus {
    /// Token was refreshed
    Refreshed,
    /// Refresh not needed yet
    NotNeeded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_valid_token() {
        // Valid JWT with exp and iat claims
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJzZW5zb3I6Y29yZS50aW1lciIsImlhdCI6MTcwNjM1NjQ5NiwiZXhwIjoxNzE0MTMyNDk2fQ.signature";

        let manager = TokenRefreshManager::new(
            ApiClient::new("http://localhost:8080".to_string(), token.to_string()),
            0.8,
        );

        let claims = manager.decode_token(token).unwrap();
        assert_eq!(claims.iat, 1706356496);
        assert_eq!(claims.exp, 1714132496);
        assert_eq!(claims.sub, "sensor:core.timer");
    }

    #[test]
    fn test_decode_invalid_token() {
        let manager = TokenRefreshManager::new(
            ApiClient::new("http://localhost:8080".to_string(), "invalid".to_string()),
            0.8,
        );

        let result = manager.decode_token("invalid_token");
        assert!(result.is_err());
    }

    #[test]
    fn test_refresh_threshold_calculation() {
        // Token issued at epoch 1000, expires at 2000 (TTL = 1000)
        // Refresh threshold 80% = 800 seconds after issuance
        // Refresh at: 1000 + 800 = 1800

        let iat = 1000;
        let exp = 2000;
        let ttl = exp - iat;
        let threshold = 0.8;

        let refresh_at = iat + ((ttl as f64) * threshold) as i64;

        assert_eq!(refresh_at, 1800);
    }
}
