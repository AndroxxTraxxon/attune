//! JWT token generation and validation

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("Failed to encode JWT: {0}")]
    EncodeError(String),
    #[error("Failed to decode JWT: {0}")]
    DecodeError(String),
    #[error("Token has expired")]
    Expired,
    #[error("Invalid token")]
    Invalid,
}

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (identity ID)
    pub sub: String,
    /// Identity login
    pub login: String,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Token type (access or refresh)
    #[serde(default)]
    pub token_type: TokenType,
    /// Optional scope (e.g., "sensor", "service")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Optional metadata (e.g., trigger_types for sensors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Refresh,
    Sensor,
}

impl Default for TokenType {
    fn default() -> Self {
        Self::Access
    }
}

/// Configuration for JWT tokens
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Secret key for signing tokens
    pub secret: String,
    /// Access token expiration duration (in seconds)
    pub access_token_expiration: i64,
    /// Refresh token expiration duration (in seconds)
    pub refresh_token_expiration: i64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "insecure_default_secret_change_in_production".to_string(),
            access_token_expiration: 3600,    // 1 hour
            refresh_token_expiration: 604800, // 7 days
        }
    }
}

/// Generate a JWT access token
///
/// # Arguments
/// * `identity_id` - The identity ID
/// * `login` - The identity login
/// * `config` - JWT configuration
///
/// # Returns
/// * `Result<String, JwtError>` - The encoded JWT token
pub fn generate_access_token(
    identity_id: i64,
    login: &str,
    config: &JwtConfig,
) -> Result<String, JwtError> {
    generate_token(identity_id, login, config, TokenType::Access)
}

/// Generate a JWT refresh token
///
/// # Arguments
/// * `identity_id` - The identity ID
/// * `login` - The identity login
/// * `config` - JWT configuration
///
/// # Returns
/// * `Result<String, JwtError>` - The encoded JWT token
pub fn generate_refresh_token(
    identity_id: i64,
    login: &str,
    config: &JwtConfig,
) -> Result<String, JwtError> {
    generate_token(identity_id, login, config, TokenType::Refresh)
}

/// Generate a JWT token
///
/// # Arguments
/// * `identity_id` - The identity ID
/// * `login` - The identity login
/// * `config` - JWT configuration
/// * `token_type` - Type of token to generate
///
/// # Returns
/// * `Result<String, JwtError>` - The encoded JWT token
pub fn generate_token(
    identity_id: i64,
    login: &str,
    config: &JwtConfig,
    token_type: TokenType,
) -> Result<String, JwtError> {
    let now = Utc::now();
    let expiration = match token_type {
        TokenType::Access => config.access_token_expiration,
        TokenType::Refresh => config.refresh_token_expiration,
        TokenType::Sensor => 86400, // Sensor tokens handled separately via generate_sensor_token()
    };

    let exp = (now + Duration::seconds(expiration)).timestamp();

    let claims = Claims {
        sub: identity_id.to_string(),
        login: login.to_string(),
        iat: now.timestamp(),
        exp,
        token_type,
        scope: None,
        metadata: None,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(|e| JwtError::EncodeError(e.to_string()))
}

/// Generate a sensor token with specific trigger types
///
/// # Arguments
/// * `identity_id` - The identity ID for the sensor
/// * `sensor_ref` - The sensor reference (e.g., "sensor:core.timer")
/// * `trigger_types` - List of trigger types this sensor can create events for
/// * `config` - JWT configuration
/// * `ttl_seconds` - Time to live in seconds (default: 24 hours)
///
/// # Returns
/// * `Result<String, JwtError>` - The encoded JWT token
pub fn generate_sensor_token(
    identity_id: i64,
    sensor_ref: &str,
    trigger_types: Vec<String>,
    config: &JwtConfig,
    ttl_seconds: Option<i64>,
) -> Result<String, JwtError> {
    let now = Utc::now();
    let expiration = ttl_seconds.unwrap_or(86400); // Default: 24 hours
    let exp = (now + Duration::seconds(expiration)).timestamp();

    let metadata = serde_json::json!({
        "trigger_types": trigger_types,
    });

    let claims = Claims {
        sub: identity_id.to_string(),
        login: sensor_ref.to_string(),
        iat: now.timestamp(),
        exp,
        token_type: TokenType::Sensor,
        scope: Some("sensor".to_string()),
        metadata: Some(metadata),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(|e| JwtError::EncodeError(e.to_string()))
}

/// Validate and decode a JWT token
///
/// # Arguments
/// * `token` - The JWT token string
/// * `config` - JWT configuration
///
/// # Returns
/// * `Result<Claims, JwtError>` - The decoded claims if valid
pub fn validate_token(token: &str, config: &JwtConfig) -> Result<Claims, JwtError> {
    let validation = Validation::default();

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| {
        if e.to_string().contains("ExpiredSignature") {
            JwtError::Expired
        } else {
            JwtError::DecodeError(e.to_string())
        }
    })
}

/// Extract token from Authorization header
///
/// # Arguments
/// * `auth_header` - The Authorization header value
///
/// # Returns
/// * `Option<&str>` - The token if present and valid format
pub fn extract_token_from_header(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> JwtConfig {
        JwtConfig {
            secret: "test_secret_key_for_testing".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 604800,
        }
    }

    #[test]
    fn test_generate_and_validate_access_token() {
        let config = test_config();
        let token =
            generate_access_token(123, "testuser", &config).expect("Failed to generate token");

        let claims = validate_token(&token, &config).expect("Failed to validate token");

        assert_eq!(claims.sub, "123");
        assert_eq!(claims.login, "testuser");
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn test_generate_and_validate_refresh_token() {
        let config = test_config();
        let token =
            generate_refresh_token(456, "anotheruser", &config).expect("Failed to generate token");

        let claims = validate_token(&token, &config).expect("Failed to validate token");

        assert_eq!(claims.sub, "456");
        assert_eq!(claims.login, "anotheruser");
        assert_eq!(claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn test_invalid_token() {
        let config = test_config();
        let result = validate_token("invalid.token.here", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_token_with_wrong_secret() {
        let config = test_config();
        let token = generate_access_token(789, "user", &config).expect("Failed to generate token");

        let wrong_config = JwtConfig {
            secret: "different_secret".to_string(),
            ..config
        };

        let result = validate_token(&token, &wrong_config);
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_token() {
        // Create a token that's already expired by setting exp in the past
        let now = Utc::now().timestamp();
        let expired_claims = Claims {
            sub: "999".to_string(),
            login: "expireduser".to_string(),
            iat: now - 3600,
            exp: now - 1800, // Expired 30 minutes ago
            token_type: TokenType::Access,
            scope: None,
            metadata: None,
        };

        let config = test_config();

        let expired_token = encode(
            &Header::default(),
            &expired_claims,
            &EncodingKey::from_secret(config.secret.as_bytes()),
        )
        .expect("Failed to encode token");

        // Validate the expired token
        let result = validate_token(&expired_token, &config);
        assert!(matches!(result, Err(JwtError::Expired)));
    }

    #[test]
    fn test_extract_token_from_header() {
        let header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let token = extract_token_from_header(header);
        assert_eq!(token, Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));

        let invalid_header = "Token abc123";
        let token = extract_token_from_header(invalid_header);
        assert_eq!(token, None);

        let no_token = "Bearer ";
        let token = extract_token_from_header(no_token);
        assert_eq!(token, Some(""));
    }

    #[test]
    fn test_claims_serialization() {
        let claims = Claims {
            sub: "123".to_string(),
            login: "testuser".to_string(),
            iat: 1234567890,
            exp: 1234571490,
            token_type: TokenType::Access,
            scope: None,
            metadata: None,
        };

        let json = serde_json::to_string(&claims).expect("Failed to serialize");
        let deserialized: Claims = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(claims.sub, deserialized.sub);
        assert_eq!(claims.login, deserialized.login);
        assert_eq!(claims.token_type, deserialized.token_type);
    }

    #[test]
    fn test_generate_sensor_token() {
        let config = test_config();
        let trigger_types = vec!["core.timer".to_string(), "core.webhook".to_string()];

        let token = generate_sensor_token(
            999,
            "sensor:core.timer",
            trigger_types.clone(),
            &config,
            Some(86400),
        )
        .expect("Failed to generate sensor token");

        let claims = validate_token(&token, &config).expect("Failed to validate token");

        assert_eq!(claims.sub, "999");
        assert_eq!(claims.login, "sensor:core.timer");
        assert_eq!(claims.token_type, TokenType::Sensor);
        assert_eq!(claims.scope, Some("sensor".to_string()));

        let metadata = claims.metadata.expect("Metadata should be present");
        let trigger_types_from_token = metadata["trigger_types"]
            .as_array()
            .expect("trigger_types should be an array");

        assert_eq!(trigger_types_from_token.len(), 2);
    }
}
