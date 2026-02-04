//! Request/Response logging middleware

use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;
use tracing::{info, warn};

/// Middleware for logging HTTP requests and responses
pub async fn log_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let version = req.version();

    let start = Instant::now();

    info!(
        method = %method,
        uri = %uri,
        version = ?version,
        "request started"
    );

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    if status.is_success() {
        info!(
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "request completed"
        );
    } else if status.is_client_error() {
        warn!(
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "request failed (client error)"
        );
    } else if status.is_server_error() {
        warn!(
            method = %method,
            uri = %uri,
            status = %status.as_u16(),
            duration_ms = %duration.as_millis(),
            "request failed (server error)"
        );
    }

    response
}
