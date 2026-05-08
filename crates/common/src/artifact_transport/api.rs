//! API-based artifact file transport.
//!
//! Transfers file content over HTTP to/from the API service's internal
//! file endpoints. Used by remote workers and sensors that do not share
//! a mounted volume with the API.

use async_trait::async_trait;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

use super::{ArtifactFileTransport, BoxAsyncReader, BoxAsyncWriter};
use crate::error::{Error, Result};

/// HTTP-based transport that calls internal file endpoints on the API.
#[derive(Debug, Clone)]
pub struct ApiTransport {
    base_url: String,
    auth_token: String,
    artifacts_dir: String,
    client: Client,
}

impl ApiTransport {
    pub fn new(api_url: &str, auth_token: &str, artifacts_dir: &str) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap_or_default();

        Self {
            base_url: api_url.trim_end_matches('/').to_string(),
            auth_token: auth_token.to_string(),
            artifacts_dir: artifacts_dir.to_string(),
            client,
        }
    }

    /// Update the auth token (e.g., after token refresh).
    pub fn set_auth_token(&mut self, token: &str) {
        self.auth_token = token.to_string();
    }

    fn file_url(&self, file_path: &str) -> String {
        // Percent-encode each path segment individually
        use url::form_urlencoded;
        let encoded_path: String = file_path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|segment| form_urlencoded::byte_serialize(segment.as_bytes()).collect::<String>())
            .collect::<Vec<_>>()
            .join("/");
        format!("{}/api/v1/internal/files/{}", self.base_url, encoded_path)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.auth_token)
    }
}

#[async_trait]
impl ArtifactFileTransport for ApiTransport {
    async fn write_file(
        &self,
        file_path: &str,
        content: &[u8],
        content_type: Option<&str>,
    ) -> Result<()> {
        let url = self.file_url(file_path);
        let ct = content_type.unwrap_or("application/octet-stream");

        let resp = self
            .client
            .put(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", ct)
            .body(content.to_vec())
            .send()
            .await
            .map_err(|e| {
                Error::Io(format!(
                    "API write_file request failed for {file_path}: {e}"
                ))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Io(format!(
                "API write_file failed for {file_path}: HTTP {status} — {body}"
            )));
        }
        Ok(())
    }

    async fn read_file(&self, file_path: &str) -> Result<Vec<u8>> {
        let url = self.file_url(file_path);

        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| Error::Io(format!("API read_file request failed for {file_path}: {e}")))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::NotFound {
                entity: "file".to_string(),
                field: "path".to_string(),
                value: file_path.to_string(),
            });
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Io(format!(
                "API read_file failed for {file_path}: HTTP {status} — {body}"
            )));
        }

        resp.bytes().await.map(|b| b.to_vec()).map_err(|e| {
            Error::Io(format!(
                "API read_file body read failed for {file_path}: {e}"
            ))
        })
    }

    async fn append_file(&self, file_path: &str, content: &[u8]) -> Result<()> {
        let url = self.file_url(file_path);

        let resp = self
            .client
            .patch(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/octet-stream")
            .body(content.to_vec())
            .send()
            .await
            .map_err(|e| {
                Error::Io(format!(
                    "API append_file request failed for {file_path}: {e}"
                ))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Io(format!(
                "API append_file failed for {file_path}: HTTP {status} — {body}"
            )));
        }
        Ok(())
    }

    async fn file_exists(&self, file_path: &str) -> Result<bool> {
        let url = self.file_url(file_path);

        let resp = self
            .client
            .head(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| {
                Error::Io(format!(
                    "API file_exists request failed for {file_path}: {e}"
                ))
            })?;

        Ok(resp.status().is_success())
    }

    async fn file_size(&self, file_path: &str) -> Result<Option<u64>> {
        let url = self.file_url(file_path);

        let resp = self
            .client
            .head(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| Error::Io(format!("API file_size request failed for {file_path}: {e}")))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Ok(None);
        }

        let size = resp
            .headers()
            .get("Content-Length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        Ok(size)
    }

    async fn delete_file(&self, file_path: &str) -> Result<()> {
        let url = self.file_url(file_path);

        let resp = self
            .client
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| {
                Error::Io(format!(
                    "API delete_file request failed for {file_path}: {e}"
                ))
            })?;

        // 404 is OK — file already gone
        if resp.status() == reqwest::StatusCode::NOT_FOUND || resp.status().is_success() {
            return Ok(());
        }

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(Error::Io(format!(
            "API delete_file failed for {file_path}: HTTP {status} — {body}"
        )))
    }

    async fn rename_file(&self, from: &str, to: &str) -> Result<()> {
        // API transport implements rename as read + write + delete
        // (no server-side rename endpoint to keep the API simple)
        let content = self.read_file(from).await?;
        self.write_file(to, &content, None).await?;
        self.delete_file(from).await?;
        Ok(())
    }

    async fn create_writer(&self, file_path: &str) -> Result<BoxAsyncWriter> {
        // Return a buffered writer that flushes to API via append calls.
        let writer = ApiBufferedWriter::new(
            self.client.clone(),
            self.file_url(file_path),
            self.auth_header(),
            file_path.to_string(),
        );
        // Ensure file starts empty
        let _ = self.delete_file(file_path).await;
        Ok(Box::pin(writer))
    }

    async fn open_reader(&self, file_path: &str, offset: u64) -> Result<BoxAsyncReader> {
        // Download the full content starting from offset and wrap in a cursor
        let url = self.file_url(file_path);
        let mut req = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header());

        if offset > 0 {
            req = req.header("Range", format!("bytes={offset}-"));
        }

        let resp = req.send().await.map_err(|e| {
            Error::Io(format!(
                "API open_reader request failed for {file_path}: {e}"
            ))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(Error::Io(format!(
                "API open_reader failed for {file_path}: HTTP {status}"
            )));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::Io(format!("API open_reader body read failed: {e}")))?;

        let cursor = std::io::Cursor::new(bytes.to_vec());
        Ok(Box::pin(cursor))
    }

    fn transport_mode(&self) -> &'static str {
        "api"
    }

    fn base_dir(&self) -> &str {
        &self.artifacts_dir
    }
}

/// Buffered async writer that batches writes and flushes to API via PATCH/append.
///
/// Accumulates bytes in an internal buffer and flushes when the buffer exceeds
/// a threshold or when `shutdown` is called.
struct ApiBufferedWriter {
    client: Client,
    url: String,
    auth: String,
    file_path: String,
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl ApiBufferedWriter {
    fn new(client: Client, url: String, auth: String, file_path: String) -> Self {
        Self {
            client,
            url,
            auth,
            file_path,
            buffer: Arc::new(Mutex::new(Vec::with_capacity(8192))),
        }
    }
}

impl std::fmt::Debug for ApiBufferedWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiBufferedWriter")
            .field("url", &self.url)
            .field("file_path", &self.file_path)
            .finish()
    }
}

const FLUSH_THRESHOLD: usize = 4096;

impl tokio::io::AsyncWrite for ApiBufferedWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        let buffer = this.buffer.clone();

        let result = buffer.try_lock();
        match result {
            Ok(mut guard) => {
                guard.extend_from_slice(buf);
                let should_flush = guard.len() >= FLUSH_THRESHOLD;
                if should_flush {
                    let data = std::mem::take(&mut *guard);
                    drop(guard);
                    let client = this.client.clone();
                    let url = this.url.clone();
                    let auth = this.auth.clone();
                    let file_path = this.file_path.clone();
                    tokio::spawn(async move {
                        if let Err(e) = flush_to_api(&client, &url, &auth, &data).await {
                            warn!("Failed to flush buffer to API for {file_path}: {e}");
                        }
                    });
                }
                std::task::Poll::Ready(Ok(buf.len()))
            }
            Err(_) => {
                // Lock contention — rare, just report as would-block
                std::task::Poll::Pending
            }
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let buffer = this.buffer.clone();
        let result = buffer.try_lock();
        if let Ok(mut guard) = result {
            if !guard.is_empty() {
                let data = std::mem::take(&mut *guard);
                drop(guard);
                let client = this.client.clone();
                let url = this.url.clone();
                let auth = this.auth.clone();
                let file_path = this.file_path.clone();
                tokio::spawn(async move {
                    if let Err(e) = flush_to_api(&client, &url, &auth, &data).await {
                        warn!("Failed to flush final buffer to API for {file_path}: {e}");
                    }
                });
            }
        }
        std::task::Poll::Ready(Ok(()))
    }
}

async fn flush_to_api(client: &Client, url: &str, auth: &str, data: &[u8]) -> Result<()> {
    let resp = client
        .patch(url)
        .header("Authorization", auth)
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .map_err(|e| Error::Io(format!("API flush request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::Io(format!(
            "API flush failed: HTTP {status} — {body}"
        )));
    }
    debug!("Flushed {} bytes to API", data.len());
    Ok(())
}
