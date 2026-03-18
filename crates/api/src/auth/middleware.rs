//! Authentication middleware for protecting routes

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use attune_common::auth::jwt::{
    extract_token_from_header, validate_token, Claims, JwtConfig, TokenType,
};

use super::oidc::{cookie_authenticated_user, ACCESS_COOKIE_NAME};

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
    let claims = extract_claims(request.headers(), &auth.jwt_config)?;

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

        let claims = if let Some(user) =
            cookie_authenticated_user(&parts.headers, state).map_err(map_cookie_auth_error)?
        {
            user.claims
        } else {
            extract_claims(&parts.headers, &state.jwt_config)?
        };

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

fn extract_claims(headers: &HeaderMap, jwt_config: &JwtConfig) -> Result<Claims, AuthError> {
    if let Some(auth_header) = headers.get(AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        let token = extract_token_from_header(auth_header).ok_or(AuthError::InvalidToken)?;
        return validate_token(token, jwt_config).map_err(|e| match e {
            super::jwt::JwtError::Expired => AuthError::ExpiredToken,
            _ => AuthError::InvalidToken,
        });
    }

    if headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|cookies| cookies.contains(ACCESS_COOKIE_NAME))
    {
        return Err(AuthError::InvalidToken);
    }

    Err(AuthError::MissingToken)
}

fn map_cookie_auth_error(error: crate::middleware::error::ApiError) -> AuthError {
    match error {
        crate::middleware::error::ApiError::Unauthorized(_) => AuthError::InvalidToken,
        _ => AuthError::InvalidToken,
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
