use anyhow::{Context, Result};
use reqwest::{multipart, Client as HttpClient, Method, RequestBuilder, Response, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::config::CliConfig;

/// API client for interacting with Attune API
pub struct ApiClient {
    client: HttpClient,
    base_url: String,
    auth_token: Option<String>,
    refresh_token: Option<String>,
    config_path: Option<PathBuf>,
}

/// Standard API response wrapper
#[derive(Debug, serde::Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

/// API error response
#[derive(Debug, serde::Deserialize)]
pub struct ApiError {
    pub error: String,
    #[serde(default)]
    pub _details: Option<serde_json::Value>,
}

impl ApiClient {
    /// Create a new API client from configuration
    pub fn from_config(config: &CliConfig, api_url_override: &Option<String>) -> Self {
        let base_url = config.effective_api_url(api_url_override);
        let auth_token = config.auth_token().ok().flatten();
        let refresh_token = config.refresh_token().ok().flatten();
        let config_path = CliConfig::config_path().ok();

        Self {
            client: HttpClient::builder()
                .timeout(Duration::from_secs(300)) // longer timeout for uploads
                .build()
                .expect("Failed to build HTTP client"),
            base_url,
            auth_token,
            refresh_token,
            config_path,
        }
    }

    /// Create a new API client
    /// Return the base URL this client is configured to talk to.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[cfg(test)]
    pub fn new(base_url: String, auth_token: Option<String>) -> Self {
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            base_url,
            auth_token,
            refresh_token: None,
            config_path: None,
        }
    }

    /// Set the authentication token
    #[cfg(test)]
    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    /// Clear the authentication token
    #[cfg(test)]
    pub fn clear_auth_token(&mut self) {
        self.auth_token = None;
    }

    /// Refresh the authentication token using the refresh token
    ///
    /// Returns Ok(true) if refresh succeeded, Ok(false) if no refresh token available
    async fn refresh_auth_token(&mut self) -> Result<bool> {
        let refresh_token = match &self.refresh_token {
            Some(token) => token.clone(),
            None => return Ok(false), // No refresh token available
        };

        #[derive(Serialize)]
        struct RefreshRequest {
            refresh_token: String,
        }

        #[derive(serde::Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: String,
        }

        // Build refresh request without auth token
        let url = format!("{}/auth/refresh", self.base_url);
        let req = self
            .client
            .post(&url)
            .json(&RefreshRequest { refresh_token });

        let response = req.send().await.context("Failed to refresh token")?;

        if !response.status().is_success() {
            // Refresh failed - clear tokens
            self.auth_token = None;
            self.refresh_token = None;
            return Ok(false);
        }

        let api_response: ApiResponse<TokenResponse> = response
            .json()
            .await
            .context("Failed to parse refresh response")?;

        // Update in-memory tokens
        self.auth_token = Some(api_response.data.access_token.clone());
        self.refresh_token = Some(api_response.data.refresh_token.clone());

        // Persist to config file if we have the path
        if self.config_path.is_some() {
            if let Ok(mut config) = CliConfig::load() {
                let _ = config.set_auth(
                    api_response.data.access_token,
                    api_response.data.refresh_token,
                );
            }
        }

        Ok(true)
    }

    /// Build a request with common headers
    fn build_request(&self, method: Method, path: &str) -> RequestBuilder {
        // Auth endpoints are at /auth, not /auth
        let url = if path.starts_with("/auth") {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/api/v1{}", self.base_url, path)
        };
        let mut req = self.client.request(method, &url);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        req
    }

    /// Execute a request and handle the response with automatic token refresh
    async fn execute<T: DeserializeOwned>(&mut self, req: RequestBuilder) -> Result<T> {
        let response = req.send().await.context("Failed to send request to API")?;

        // If 401 and we have a refresh token, try to refresh once
        if response.status() == StatusCode::UNAUTHORIZED && self.refresh_token.is_some() {
            // Try to refresh the token
            if self.refresh_auth_token().await? {
                // Rebuild and retry the original request with new token
                // Note: This is a simplified retry - the original request body is already consumed
                // For a production implementation, we'd need to clone the request or store the body
                return Err(anyhow::anyhow!(
                    "Token expired and was refreshed. Please retry your command."
                ));
            }
        }

        self.handle_response(response).await
    }

    /// Handle API response and extract data
    async fn handle_response<T: DeserializeOwned>(&self, response: Response) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            let api_response: ApiResponse<T> = response
                .json()
                .await
                .context("Failed to parse API response")?;
            Ok(api_response.data)
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Try to parse as API error
            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                anyhow::bail!("API error ({}): {}", status, api_error.error);
            } else {
                anyhow::bail!("API error ({}): {}", status, error_text);
            }
        }
    }

    /// GET request
    pub async fn get<T: DeserializeOwned>(&mut self, path: &str) -> Result<T> {
        let req = self.build_request(Method::GET, path);
        self.execute(req).await
    }

    /// GET request with query parameters (query string must be in path)
    ///
    /// Part of REST client API - reserved for future advanced filtering/search features.
    /// Example: `client.get_with_query("/actions?enabled=true&pack=core").await`
    #[allow(dead_code)]
    pub async fn get_with_query<T: DeserializeOwned>(&mut self, path: &str) -> Result<T> {
        let req = self.build_request(Method::GET, path);
        self.execute(req).await
    }

    /// POST request with JSON body
    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &mut self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let req = self.build_request(Method::POST, path).json(body);
        self.execute(req).await
    }

    /// PUT request with JSON body
    ///
    /// Part of REST client API - will be used for update operations
    pub async fn put<T: DeserializeOwned, B: Serialize>(
        &mut self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let req = self.build_request(Method::PUT, path).json(body);
        self.execute(req).await
    }

    /// PATCH request with JSON body
    pub async fn patch<T: DeserializeOwned, B: Serialize>(
        &mut self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let req = self.build_request(Method::PATCH, path).json(body);
        self.execute(req).await
    }

    /// DELETE request with response parsing
    ///
    /// Part of REST client API - reserved for delete operations that return data.
    /// Currently we use `delete_no_response()` for all delete operations.
    /// This method is kept for API completeness and future use cases where
    /// delete operations return metadata (e.g., cascade deletion summaries).
    #[allow(dead_code)]
    pub async fn delete<T: DeserializeOwned>(&mut self, path: &str) -> Result<T> {
        let req = self.build_request(Method::DELETE, path);
        self.execute(req).await
    }

    /// POST request without expecting response body
    ///
    /// Part of REST client API - reserved for fire-and-forget operations.
    /// Example use cases: webhook notifications, event submissions, audit logging.
    /// Kept for API completeness even though not currently used.
    #[allow(dead_code)]
    pub async fn post_no_response<B: Serialize>(&mut self, path: &str, body: &B) -> Result<()> {
        let req = self.build_request(Method::POST, path).json(body);
        let response = req.send().await.context("Failed to send request to API")?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("API error ({}): {}", status, error_text);
        }
    }

    /// DELETE request without expecting response body
    pub async fn delete_no_response(&mut self, path: &str) -> Result<()> {
        let req = self.build_request(Method::DELETE, path);
        let response = req.send().await.context("Failed to send request to API")?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("API error ({}): {}", status, error_text);
        }
    }

    /// POST a multipart/form-data request with a file field and optional text fields.
    ///
    /// - `file_field_name`: the multipart field name for the file
    /// - `file_bytes`: raw bytes of the file content
    /// - `file_name`: filename hint sent in the Content-Disposition header
    /// - `mime_type`: MIME type of the file (e.g. `"application/gzip"`)
    /// - `extra_fields`: additional text key/value fields to include in the form
    pub async fn multipart_post<T: DeserializeOwned>(
        &mut self,
        path: &str,
        file_field_name: &str,
        file_bytes: Vec<u8>,
        file_name: &str,
        mime_type: &str,
        extra_fields: Vec<(&str, String)>,
    ) -> Result<T> {
        let url = format!("{}/api/v1{}", self.base_url, path);

        let file_part = multipart::Part::bytes(file_bytes)
            .file_name(file_name.to_string())
            .mime_str(mime_type)
            .context("Invalid MIME type")?;

        let mut form = multipart::Form::new().part(file_field_name.to_string(), file_part);

        for (key, value) in extra_fields {
            form = form.text(key.to_string(), value);
        }

        let mut req = self.client.post(&url).multipart(form);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let response = req.send().await.context("Failed to send multipart request to API")?;

        // Handle 401 + refresh (same pattern as execute())
        if response.status() == StatusCode::UNAUTHORIZED && self.refresh_token.is_some() {
            if self.refresh_auth_token().await? {
                return Err(anyhow::anyhow!(
                    "Token expired and was refreshed. Please retry your command."
                ));
            }
        }

        self.handle_response(response).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = ApiClient::new("http://localhost:8080".to_string(), None);
        assert_eq!(client.base_url, "http://localhost:8080");
        assert!(client.auth_token.is_none());
    }

    #[test]
    fn test_set_auth_token() {
        let mut client = ApiClient::new("http://localhost:8080".to_string(), None);
        assert!(client.auth_token.is_none());

        client.set_auth_token("test_token".to_string());
        assert_eq!(client.auth_token, Some("test_token".to_string()));

        client.clear_auth_token();
        assert!(client.auth_token.is_none());
    }
}
