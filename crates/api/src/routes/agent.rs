//! Agent binary download endpoints
//!
//! Provides endpoints for downloading the attune-agent binary for injection
//! into arbitrary containers. This supports deployments where shared Docker
//! volumes are impractical (Kubernetes, ECS, remote Docker hosts).

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::fs;
use tokio_util::io::ReaderStream;
use utoipa::{IntoParams, ToSchema};

use crate::state::AppState;

/// Query parameters for the binary download endpoint
#[derive(Debug, Deserialize, IntoParams)]
pub struct BinaryDownloadParams {
    /// Target architecture (x86_64, aarch64). Defaults to x86_64.
    #[param(example = "x86_64")]
    pub arch: Option<String>,
    /// Optional bootstrap token for authentication
    pub token: Option<String>,
}

/// Agent binary metadata
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentBinaryInfo {
    /// Available architectures
    pub architectures: Vec<AgentArchInfo>,
    /// Agent version (from build)
    pub version: String,
}

/// Per-architecture binary info
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentArchInfo {
    /// Architecture name
    pub arch: String,
    /// Binary size in bytes
    pub size_bytes: u64,
    /// Whether this binary is available
    pub available: bool,
}

/// Validate that the architecture name is safe (no path traversal) and normalize it.
fn validate_arch(arch: &str) -> Result<&str, (StatusCode, Json<serde_json::Value>)> {
    match arch {
        "x86_64" | "aarch64" => Ok(arch),
        // Accept arm64 as an alias for aarch64
        "arm64" => Ok("aarch64"),
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid architecture",
                "message": format!("Unsupported architecture '{}'. Supported: x86_64, aarch64", arch),
            })),
        )),
    }
}

/// Validate bootstrap token.
///
/// The agent config MUST have a `bootstrap_token` set; if not, this endpoint
/// is treated as disabled and returns `503 Service Unavailable`. This
/// fail-closed behavior prevents anonymous downloads of the agent binary
/// when the operator has not explicitly opted in to token authentication.
///
/// When configured, the request must provide the matching token via the
/// `X-Agent-Token` header or the `token` query parameter.
fn validate_token(
    config: &attune_common::config::Config,
    headers: &HeaderMap,
    query_token: &Option<String>,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let expected_token = config
        .agent
        .as_ref()
        .and_then(|ac| ac.bootstrap_token.as_ref());

    let expected_token = match expected_token {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "Agent downloads disabled",
                    "message": "Agent binary distribution requires authentication. Set agent.bootstrap_token in config to enable."
                })),
            ));
        }
    };

    // Check X-Agent-Token header first, then query param
    let provided_token = headers
        .get("x-agent-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| query_token.clone());

    match provided_token {
        Some(ref t) if bool::from(t.as_bytes().ct_eq(expected_token.as_bytes())) => Ok(()),
        Some(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Invalid token",
                "message": "The provided bootstrap token is invalid",
            })),
        )),
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Token required",
                "message": "A bootstrap token is required. Provide via X-Agent-Token header or token query parameter.",
            })),
        )),
    }
}

/// Download the agent binary
///
/// Returns the statically-linked attune-agent binary for the requested architecture.
/// The binary can be injected into any container to turn it into an Attune worker.
#[utoipa::path(
    get,
    path = "/api/v1/agent/binary",
    params(BinaryDownloadParams),
    responses(
        (status = 200, description = "Agent binary", content_type = "application/octet-stream"),
        (status = 400, description = "Invalid architecture"),
        (status = 401, description = "Invalid or missing bootstrap token"),
        (status = 404, description = "Agent binary not found"),
        (status = 503, description = "Agent binary distribution not configured"),
    ),
    tag = "agent"
)]
pub async fn download_agent_binary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<BinaryDownloadParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Validate bootstrap token if configured
    validate_token(&state.config, &headers, &params.token)?;

    let agent_config = state.config.agent.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Not configured",
                "message": "Agent binary distribution is not configured. Set agent.binary_dir in config.",
            })),
        )
    })?;

    let arch = params.arch.as_deref().unwrap_or("x86_64");
    let arch = validate_arch(arch)?;

    let binary_dir = std::path::Path::new(&agent_config.binary_dir);

    // Try arch-specific binary first, then fall back to generic name.
    // IMPORTANT: The generic `attune-agent` binary is only safe to serve for
    // x86_64 requests, because the current build pipeline produces an
    // x86_64-unknown-linux-musl binary. Serving it for aarch64/arm64 would
    // give the caller an incompatible executable (exec format error).
    let arch_specific = binary_dir.join(format!("attune-agent-{}", arch));
    let generic = binary_dir.join("attune-agent");

    let binary_path = if arch_specific.exists() {
        arch_specific
    } else if arch == "x86_64" && generic.exists() {
        tracing::debug!(
            "Arch-specific binary not found at {:?}, falling back to generic {:?} (safe for x86_64)",
            arch_specific,
            generic
        );
        generic
    } else {
        tracing::warn!(
            "Agent binary not found. Checked: {:?} and {:?}",
            arch_specific,
            generic
        );
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Not found",
                "message": format!(
                    "Agent binary not found for architecture '{}'. Ensure the agent binary is built and placed in '{}'.",
                    arch,
                    agent_config.binary_dir
                ),
            })),
        ));
    };

    // Get file metadata for Content-Length
    let metadata = fs::metadata(&binary_path).await.map_err(|e| {
        tracing::error!(
            "Failed to read agent binary metadata at {:?}: {}",
            binary_path,
            e
        );
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Internal error",
                "message": "Failed to read agent binary",
            })),
        )
    })?;

    // Open file for streaming
    let file = fs::File::open(&binary_path).await.map_err(|e| {
        tracing::error!("Failed to open agent binary at {:?}: {}", binary_path, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Internal error",
                "message": "Failed to open agent binary",
            })),
        )
    })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let headers_response = [
        (header::CONTENT_TYPE, "application/octet-stream".to_string()),
        (
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"attune-agent\"".to_string(),
        ),
        (header::CONTENT_LENGTH, metadata.len().to_string()),
        (header::CACHE_CONTROL, "public, max-age=3600".to_string()),
    ];

    tracing::info!(
        arch = arch,
        size_bytes = metadata.len(),
        path = ?binary_path,
        "Serving agent binary download"
    );

    Ok((headers_response, body))
}

/// Get agent binary metadata
///
/// Returns information about available agent binaries, including
/// supported architectures and binary sizes.
#[utoipa::path(
    get,
    path = "/api/v1/agent/info",
    responses(
        (status = 200, description = "Agent binary info", body = AgentBinaryInfo),
        (status = 503, description = "Agent binary distribution not configured"),
    ),
    tag = "agent"
)]
pub async fn agent_info(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let agent_config = state.config.agent.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Not configured",
                "message": "Agent binary distribution is not configured.",
            })),
        )
    })?;

    let binary_dir = std::path::Path::new(&agent_config.binary_dir);
    let architectures = ["x86_64", "aarch64"];

    let mut arch_infos = Vec::new();
    for arch in &architectures {
        let arch_specific = binary_dir.join(format!("attune-agent-{}", arch));
        let generic = binary_dir.join("attune-agent");

        // Only fall back to the generic binary for x86_64, since the build
        // pipeline currently produces x86_64-only generic binaries.
        let (available, size_bytes) = if arch_specific.exists() {
            match fs::metadata(&arch_specific).await {
                Ok(m) => (true, m.len()),
                Err(_) => (false, 0),
            }
        } else if *arch == "x86_64" && generic.exists() {
            match fs::metadata(&generic).await {
                Ok(m) => (true, m.len()),
                Err(_) => (false, 0),
            }
        } else {
            (false, 0)
        };

        arch_infos.push(AgentArchInfo {
            arch: arch.to_string(),
            size_bytes,
            available,
        });
    }

    Ok(Json(AgentBinaryInfo {
        architectures: arch_infos,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

/// Create agent routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agent/binary", get(download_agent_binary))
        .route("/agent/info", get(agent_info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use attune_common::config::AgentConfig;
    use axum::http::{HeaderMap, HeaderValue};

    // ── validate_arch tests ─────────────────────────────────────────

    #[test]
    fn test_validate_arch_valid_x86_64() {
        let result = validate_arch("x86_64");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "x86_64");
    }

    #[test]
    fn test_validate_arch_valid_aarch64() {
        let result = validate_arch("aarch64");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "aarch64");
    }

    #[test]
    fn test_validate_arch_arm64_alias() {
        // "arm64" is an alias for "aarch64"
        let result = validate_arch("arm64");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "aarch64");
    }

    #[test]
    fn test_validate_arch_invalid() {
        let result = validate_arch("mips");
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.0["error"], "Invalid architecture");
    }

    // ── validate_token tests ────────────────────────────────────────

    /// Helper: build a minimal Config with the given agent config.
    /// Only the `agent` field is relevant for `validate_token`.
    fn test_config(agent: Option<AgentConfig>) -> attune_common::config::Config {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let config_path = format!("{}/../../config.test.yaml", manifest_dir);
        let mut config = attune_common::config::Config::load_from_file(&config_path)
            .expect("Failed to load test config");
        config.agent = agent;
        config
    }

    #[test]
    fn test_validate_token_no_config_returns_503() {
        // When no agent config is set at all, the endpoint is disabled.
        // SECURITY: This must fail closed (503) rather than fail open (Ok).
        let config = test_config(None);
        let headers = HeaderMap::new();
        let query_token = None;

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.0["error"], "Agent downloads disabled");
    }

    #[test]
    fn test_validate_token_no_bootstrap_token_configured_returns_503() {
        // Agent config exists but bootstrap_token is None → endpoint disabled.
        // SECURITY: This must fail closed (503) rather than fail open (Ok).
        let config = test_config(Some(AgentConfig {
            binary_dir: "/tmp/test".to_string(),
            bootstrap_token: None,
        }));
        let headers = HeaderMap::new();
        let query_token = None;

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.0["error"], "Agent downloads disabled");
    }

    #[test]
    fn test_validate_token_no_bootstrap_token_ignores_provided_token() {
        // Even if the caller provides a token, an unconfigured endpoint
        // must still return 503 — never accept a caller-supplied token
        // as authoritative when no expected token is configured.
        let config = test_config(Some(AgentConfig {
            binary_dir: "/tmp/test".to_string(),
            bootstrap_token: None,
        }));
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-token", HeaderValue::from_static("anything"));
        let query_token = Some("anything".to_string());

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_err());
        let (status, _body) = result.unwrap_err();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_validate_token_valid_from_header() {
        let config = test_config(Some(AgentConfig {
            binary_dir: "/tmp/test".to_string(),
            bootstrap_token: Some("s3cret-bootstrap".to_string()),
        }));
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-agent-token",
            HeaderValue::from_static("s3cret-bootstrap"),
        );
        let query_token = None;

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_token_valid_from_query() {
        let config = test_config(Some(AgentConfig {
            binary_dir: "/tmp/test".to_string(),
            bootstrap_token: Some("s3cret-bootstrap".to_string()),
        }));
        let headers = HeaderMap::new();
        let query_token = Some("s3cret-bootstrap".to_string());

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_token_invalid() {
        let config = test_config(Some(AgentConfig {
            binary_dir: "/tmp/test".to_string(),
            bootstrap_token: Some("correct-token".to_string()),
        }));
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-token", HeaderValue::from_static("wrong-token"));
        let query_token = None;

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.0["error"], "Invalid token");
    }

    #[test]
    fn test_validate_token_missing_when_required() {
        // bootstrap_token is configured but caller provides nothing.
        let config = test_config(Some(AgentConfig {
            binary_dir: "/tmp/test".to_string(),
            bootstrap_token: Some("required-token".to_string()),
        }));
        let headers = HeaderMap::new();
        let query_token = None;

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.0["error"], "Token required");
    }

    #[test]
    fn test_validate_token_header_takes_precedence_over_query() {
        // When both header and query provide a token, the header value is
        // checked first (it appears first in the or_else chain). Provide a
        // valid token in the header and an invalid one in the query — should
        // succeed because the header matches.
        let config = test_config(Some(AgentConfig {
            binary_dir: "/tmp/test".to_string(),
            bootstrap_token: Some("the-real-token".to_string()),
        }));
        let mut headers = HeaderMap::new();
        headers.insert("x-agent-token", HeaderValue::from_static("the-real-token"));
        let query_token = Some("wrong-token".to_string());

        let result = validate_token(&config, &headers, &query_token);
        assert!(result.is_ok());
    }
}
