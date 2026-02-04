//! CORS middleware configuration

use axum::http::{header, HeaderValue, Method};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Create CORS layer configured from allowed origins
///
/// If no origins are provided, defaults to common development origins.
/// Cannot use `allow_origin(Any)` with credentials enabled.
pub fn create_cors_layer(allowed_origins: Vec<String>) -> CorsLayer {
    // Get the list of allowed origins
    let origins = if allowed_origins.is_empty() {
        // Default development origins
        vec![
            "http://localhost:3000".to_string(),
            "http://localhost:5173".to_string(),
            "http://localhost:8080".to_string(),
            "http://127.0.0.1:3000".to_string(),
            "http://127.0.0.1:5173".to_string(),
            "http://127.0.0.1:8080".to_string(),
        ]
    } else {
        allowed_origins
    };

    // Convert origins to HeaderValues for matching
    let allowed_origin_values: Arc<Vec<HeaderValue>> = Arc::new(
        origins
            .iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect(),
    );

    CorsLayer::new()
        // Allow common HTTP methods
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        // Allow specific headers (required when using credentials)
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
        // Expose headers to the frontend
        .expose_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::CONTENT_LENGTH,
            header::ACCEPT,
        ])
        // Allow credentials (cookies, authorization headers)
        .allow_credentials(true)
        // Use predicate to match against allowed origins
        // Arc allows the closure to be called multiple times (preflight + actual request)
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            allowed_origin_values.contains(origin)
        }))
}
