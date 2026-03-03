//! Authentication middleware for protecting routes

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use attune_common::auth::jwt::{
    extract_token_from_header, validate_token, Claims, JwtConfig, TokenType,
};

/// Authentication middleware state
#[derive(Clone)]
pub struct AuthMiddleware {
    pub jwt_config: Arc<JwtConfig>,
}

impl AuthMiddleware {
    pub fn new(jwt_config: JwtConfig) -> Self {
        Self {
            jwt_config: Arc::new(jwt_config),
        }
    }
}

/// Extension type for storing authenticated claims in request
#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub claims: Claims,
}

impl AuthenticatedUser {
    pub fn identity_id(&self) -> Result<i64, std::num::ParseIntError> {
        self.claims.sub.parse()
    }

    pub fn login(&self) -> &str {
        &self.claims.login
    }
}

/// Middleware function that validates JWT tokens
pub async fn require_auth(
    State(auth): State<AuthMiddleware>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(AuthError::MissingToken)?;

    // Extract token from Bearer scheme
    let token = extract_token_from_header(auth_header).ok_or(AuthError::InvalidToken)?;

    // Validate token
    let claims = validate_token(token, &auth.jwt_config).map_err(|e| match e {
        super::jwt::JwtError::Expired => AuthError::ExpiredToken,
        _ => AuthError::InvalidToken,
    })?;

    // Add claims to request extensions
    request
        .extensions_mut()
        .insert(AuthenticatedUser { claims });

    // Continue to next middleware/handler
    Ok(next.run(request).await)
}

/// Extractor for authenticated user
pub struct RequireAuth(pub AuthenticatedUser);

impl axum::extract::FromRequestParts<crate::state::SharedState> for RequireAuth {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &crate::state::SharedState,
    ) -> Result<Self, Self::Rejection> {
        // First check if middleware already added the user
        if let Some(user) = parts.extensions.get::<AuthenticatedUser>() {
            return Ok(RequireAuth(user.clone()));
        }

        // Otherwise, extract and validate token directly from header
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        // Extract token from Bearer scheme
        let token = extract_token_from_header(auth_header).ok_or(AuthError::InvalidToken)?;

        // Validate token using jwt_config from app state
        let claims = validate_token(token, &state.jwt_config).map_err(|e| match e {
            super::jwt::JwtError::Expired => AuthError::ExpiredToken,
            _ => AuthError::InvalidToken,
        })?;

        // Allow access, sensor, and execution-scoped tokens
        if claims.token_type != TokenType::Access
            && claims.token_type != TokenType::Sensor
            && claims.token_type != TokenType::Execution
        {
            return Err(AuthError::InvalidToken);
        }

        Ok(RequireAuth(AuthenticatedUser { claims }))
    }
}

/// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredToken,
    Unauthorized,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authentication token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid authentication token"),
            AuthError::ExpiredToken => (StatusCode::UNAUTHORIZED, "Authentication token expired"),
            AuthError::Unauthorized => (StatusCode::FORBIDDEN, "Insufficient permissions"),
        };

        let body = Json(json!({
            "error": {
                "code": status.as_u16(),
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticated_user() {
        let claims = Claims {
            sub: "123".to_string(),
            login: "testuser".to_string(),
            iat: 1234567890,
            exp: 1234571490,
            token_type: TokenType::Access,
            scope: None,
            metadata: None,
        };

        let auth_user = AuthenticatedUser { claims };

        assert_eq!(auth_user.identity_id().unwrap(), 123);
        assert_eq!(auth_user.login(), "testuser");
    }

    #[test]
    fn test_extract_token_from_header() {
        let token = extract_token_from_header("Bearer test.token.here");
        assert_eq!(token, Some("test.token.here"));

        let no_bearer = extract_token_from_header("test.token.here");
        assert_eq!(no_bearer, None);
    }
}
