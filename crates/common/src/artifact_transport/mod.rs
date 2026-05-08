//! Artifact file transport abstraction
//!
//! This module provides a transport layer for artifact file content,
//! decoupling metadata operations (always via DB) from file content
//! transfer (via shared volume or API).
//!
//! Two implementations:
//! - [`VolumeTransport`]: Direct filesystem I/O on a shared volume (fast path)
//! - [`ApiTransport`]: HTTP-based upload/download via API internal endpoints (remote workers)
//!
//! Workers and sensors auto-detect which transport to use at startup
//! by checking for a sentinel file written by the API.

mod api;
pub mod detection;
mod volume;

pub use api::ApiTransport;
pub use detection::{detect_transport_mode, TransportMode};
pub use volume::VolumeTransport;

use async_trait::async_trait;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::Result;

/// Async writer returned by `create_writer`.
pub type BoxAsyncWriter = Pin<Box<dyn AsyncWrite + Send + Sync>>;
/// Async reader returned by `open_reader`.
pub type BoxAsyncReader = Pin<Box<dyn AsyncRead + Send + Sync>>;

/// Abstraction over artifact file content storage.
///
/// Metadata (artifact/version rows) continues to flow through direct DB access.
/// This trait handles only the *file bytes* — writing, reading, appending,
/// existence checks, and cleanup.
#[async_trait]
pub trait ArtifactFileTransport: Send + Sync + std::fmt::Debug {
    /// Write complete file content, creating parent directories as needed.
    async fn write_file(
        &self,
        file_path: &str,
        content: &[u8],
        content_type: Option<&str>,
    ) -> Result<()>;

    /// Read complete file content.
    async fn read_file(&self, file_path: &str) -> Result<Vec<u8>>;

    /// Append bytes to an existing file (creates if missing).
    async fn append_file(&self, file_path: &str, content: &[u8]) -> Result<()>;

    /// Check whether a file exists.
    async fn file_exists(&self, file_path: &str) -> Result<bool>;

    /// Return the file size in bytes, or `None` if the file does not exist.
    async fn file_size(&self, file_path: &str) -> Result<Option<u64>>;

    /// Delete a file. No error if it does not exist.
    async fn delete_file(&self, file_path: &str) -> Result<()>;

    /// Rename / move a file within the transport.
    async fn rename_file(&self, from: &str, to: &str) -> Result<()>;

    /// Create a streaming writer for live output capture.
    async fn create_writer(&self, file_path: &str) -> Result<BoxAsyncWriter>;

    /// Open a streaming reader, optionally starting at `offset` bytes.
    async fn open_reader(&self, file_path: &str, offset: u64) -> Result<BoxAsyncReader>;

    /// Returns the transport mode name for diagnostics / logging.
    fn transport_mode(&self) -> &'static str;

    /// Returns the base directory for resolving absolute paths (if applicable).
    fn base_dir(&self) -> &str;

    /// Ensure parent directories exist for a given file path.
    /// Default implementation is a no-op (API transport handles this server-side).
    async fn ensure_parent_dirs(&self, _file_path: &str) -> Result<()> {
        Ok(())
    }
}

/// Build the appropriate transport from config + detection.
pub fn build_transport(
    artifacts_dir: &str,
    api_url: Option<&str>,
    auth_token: Option<&str>,
    config_transport: &TransportMode,
) -> Box<dyn ArtifactFileTransport> {
    match config_transport {
        TransportMode::Volume => Box::new(VolumeTransport::new(artifacts_dir)),
        TransportMode::Api => {
            let url = api_url.unwrap_or("http://localhost:8080");
            let token = auth_token.unwrap_or("");
            Box::new(ApiTransport::new(url, token, artifacts_dir))
        }
        TransportMode::Auto => {
            let detected = detect_transport_mode(artifacts_dir);
            match detected {
                TransportMode::Volume => Box::new(VolumeTransport::new(artifacts_dir)),
                _ => {
                    let url = api_url.unwrap_or("http://localhost:8080");
                    let token = auth_token.unwrap_or("");
                    Box::new(ApiTransport::new(url, token, artifacts_dir))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_mode_default() {
        let mode = TransportMode::default();
        assert!(matches!(mode, TransportMode::Auto));
    }
}
