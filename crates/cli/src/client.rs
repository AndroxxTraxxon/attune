use anyhow::{Context, Result};
use reqwest::{header, multipart, Client as HttpClient, Method, RequestBuilder, StatusCode};
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

    /// Refresh the authentication token using the refresh token.
    ///
    /// Returns `Ok(true)` if refresh succeeded, `Ok(false)` if no refresh token
    /// is available or the server rejected it.
    async fn refresh_auth_token(&mut self) -> Result<bool> {
        let refresh_token = match &self.refresh_token {
            Some(token) => token.clone(),
            None => return Ok(false),
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

        let url = format!("{}/auth/refresh", self.base_url);
        let req = self
            .client
            .post(&url)
            .json(&RefreshRequest { refresh_token });

        let response = req.send().await.context("Failed to refresh token")?;

        if !response.status().is_success() {
            // Refresh failed — clear tokens so we don't keep retrying
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

        // Persist to config file
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

    // ── Request building helpers ────────────────────────────────────────

    /// Build a full URL from a path.
    fn url_for(&self, path: &str) -> String {
        if path.starts_with("/auth") {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/api/v1{}", self.base_url, path)
        }
    }

    /// Build a `RequestBuilder` with auth header applied.
    fn build_request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = self.url_for(path);
        let mut req = self.client.request(method, &url);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }
        req
    }

    // ── Core execute-with-retry machinery ──────────────────────────────

    /// Send a request that carries a JSON body.  On a 401 response the token
    /// is refreshed and the request is rebuilt & retried exactly once.
    async fn execute_json<T, B>(
        &mut self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        // First attempt
        let req = self.attach_body(self.build_request(method.clone(), path), body);
        let response = req.send().await.context("Failed to send request to API")?;

        if response.status() == StatusCode::UNAUTHORIZED
            && self.refresh_token.is_some()
            && self.refresh_auth_token().await?
        {
            // Retry with new token
            let req = self.attach_body(self.build_request(method, path), body);
            let response = req
                .send()
                .await
                .context("Failed to send request to API (retry)")?;
            return self.handle_response(response).await;
        }

        self.handle_response(response).await
    }

    /// Send a request that carries a JSON body and expects no response body.
    async fn execute_json_no_response<B: Serialize>(
        &mut self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<()> {
        let req = self.attach_body(self.build_request(method.clone(), path), body);
        let response = req.send().await.context("Failed to send request to API")?;

        if response.status() == StatusCode::UNAUTHORIZED
            && self.refresh_token.is_some()
            && self.refresh_auth_token().await?
        {
            let req = self.attach_body(self.build_request(method, path), body);
            let response = req
                .send()
                .await
                .context("Failed to send request to API (retry)")?;
            return self.handle_empty_response(response).await;
        }

        self.handle_empty_response(response).await
    }

    /// Optionally attach a JSON body to a request builder.
    fn attach_body<B: Serialize>(&self, req: RequestBuilder, body: Option<&B>) -> RequestBuilder {
        match body {
            Some(b) => req.json(b),
            None => req,
        }
    }

    // ── Response handling ──────────────────────────────────────────────

    /// Parse a successful API response or return a descriptive error.
    async fn handle_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
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

            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                anyhow::bail!("API error ({}): {}", status, api_error.error);
            } else {
                anyhow::bail!("API error ({}): {}", status, error_text);
            }
        }
    }

    /// Handle a response where we only care about success/failure, not a body.
    async fn handle_empty_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                anyhow::bail!("API error ({}): {}", status, api_error.error);
            } else {
                anyhow::bail!("API error ({}): {}", status, error_text);
            }
        }
    }

    // ── Public convenience methods ─────────────────────────────────────

    /// GET request
    pub async fn get<T: DeserializeOwned>(&mut self, path: &str) -> Result<T> {
        self.execute_json::<T, ()>(Method::GET, path, None).await
    }

    /// GET request with query parameters (query string must be in path)
    ///
    /// Part of REST client API - reserved for future advanced filtering/search features.
    /// Example: `client.get_with_query("/actions?enabled=true&pack=core").await`
    #[allow(dead_code)]
    pub async fn get_with_query<T: DeserializeOwned>(&mut self, path: &str) -> Result<T> {
        self.execute_json::<T, ()>(Method::GET, path, None).await
    }

    /// POST request with JSON body
    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &mut self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.execute_json(Method::POST, path, Some(body)).await
    }

    /// PUT request with JSON body
    ///
    /// Part of REST client API - will be used for update operations
    pub async fn put<T: DeserializeOwned, B: Serialize>(
        &mut self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.execute_json(Method::PUT, path, Some(body)).await
    }

    /// PATCH request with JSON body
    pub async fn patch<T: DeserializeOwned, B: Serialize>(
        &mut self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.execute_json(Method::PATCH, path, Some(body)).await
    }

    /// DELETE request with response parsing
    ///
    /// Part of REST client API - reserved for delete operations that return data.
    /// Currently we use `delete_no_response()` for all delete operations.
    /// This method is kept for API completeness and future use cases where
    /// delete operations return metadata (e.g., cascade deletion summaries).
    #[allow(dead_code)]
    pub async fn delete<T: DeserializeOwned>(&mut self, path: &str) -> Result<T> {
        self.execute_json::<T, ()>(Method::DELETE, path, None).await
    }

    /// POST request without expecting response body
    ///
    /// Part of REST client API - reserved for fire-and-forget operations.
    /// Example use cases: webhook notifications, event submissions, audit logging.
    /// Kept for API completeness even though not currently used.
    #[allow(dead_code)]
    pub async fn post_no_response<B: Serialize>(&mut self, path: &str, body: &B) -> Result<()> {
        self.execute_json_no_response(Method::POST, path, Some(body))
            .await
    }

    /// DELETE request without expecting response body
    pub async fn delete_no_response(&mut self, path: &str) -> Result<()> {
        self.execute_json_no_response::<()>(Method::DELETE, path, None)
            .await
    }

    /// GET request that returns raw bytes and optional filename from Content-Disposition.
    ///
    /// Used for downloading binary content (e.g., artifact files).
    /// Returns `(bytes, content_type, optional_filename)`.
    pub async fn download_bytes(
        &mut self,
        path: &str,
    ) -> Result<(Vec<u8>, String, Option<String>)> {
        // First attempt
        let req = self.build_request(Method::GET, path);
        let response = req.send().await.context("Failed to send request to API")?;

        if response.status() == StatusCode::UNAUTHORIZED
            && self.refresh_token.is_some()
            && self.refresh_auth_token().await?
        {
            // Retry with new token
            let req = self.build_request(Method::GET, path);
            let response = req
                .send()
                .await
                .context("Failed to send request to API (retry)")?;
            return self.handle_bytes_response(response).await;
        }

        self.handle_bytes_response(response).await
    }

    /// Parse a binary response, extracting content type and optional filename.
    async fn handle_bytes_response(
        &self,
        response: reqwest::Response,
    ) -> Result<(Vec<u8>, String, Option<String>)> {
        let status = response.status();

        if status.is_success() {
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();

            let filename = response
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| {
                    // Parse filename from Content-Disposition: attachment; filename="name.ext"
                    v.split("filename=")
                        .nth(1)
                        .map(|f| f.trim_matches('"').trim_matches('\'').to_string())
                });

            let bytes = response
                .bytes()
                .await
                .context("Failed to read response bytes")?;

            Ok((bytes.to_vec(), content_type, filename))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                anyhow::bail!("API error ({}): {}", status, api_error.error);
            } else {
                anyhow::bail!("API error ({}): {}", status, error_text);
            }
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
        // Closure-like helper to build the multipart request from scratch.
        // We need this because reqwest::multipart::Form is not Clone, so we
        // must rebuild it for the retry attempt.
        let build_multipart_request =
            |client: &ApiClient, bytes: &[u8]| -> Result<reqwest::RequestBuilder> {
                let url = format!("{}/api/v1{}", client.base_url, path);

                let file_part = multipart::Part::bytes(bytes.to_vec())
                    .file_name(file_name.to_string())
                    .mime_str(mime_type)
                    .context("Invalid MIME type")?;

                let mut form = multipart::Form::new().part(file_field_name.to_string(), file_part);

                for (key, value) in &extra_fields {
                    form = form.text(key.to_string(), value.clone());
                }

                let mut req = client.client.post(&url).multipart(form);
                if let Some(token) = &client.auth_token {
                    req = req.bearer_auth(token);
                }
                Ok(req)
            };

        // First attempt
        let req = build_multipart_request(self, &file_bytes)?;
        let response = req
            .send()
            .await
            .context("Failed to send multipart request to API")?;

        if response.status() == StatusCode::UNAUTHORIZED
            && self.refresh_token.is_some()
            && self.refresh_auth_token().await?
        {
            // Retry with new token
            let req = build_multipart_request(self, &file_bytes)?;
            let response = req
                .send()
                .await
                .context("Failed to send multipart request to API (retry)")?;
            return self.handle_response(response).await;
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

    #[test]
    fn test_url_for_api_path() {
        let client = ApiClient::new("http://localhost:8080".to_string(), None);
        assert_eq!(
            client.url_for("/actions"),
            "http://localhost:8080/api/v1/actions"
        );
    }

    #[test]
    fn test_url_for_auth_path() {
        let client = ApiClient::new("http://localhost:8080".to_string(), None);
        assert_eq!(
            client.url_for("/auth/login"),
            "http://localhost:8080/auth/login"
        );
    }
}
