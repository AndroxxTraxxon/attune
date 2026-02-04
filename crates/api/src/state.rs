//! Application state shared across request handlers

use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::auth::jwt::JwtConfig;
use attune_common::{config::Config, mq::Publisher};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub db: PgPool,
    /// JWT configuration
    pub jwt_config: Arc<JwtConfig>,
    /// CORS allowed origins
    pub cors_origins: Vec<String>,
    /// Application configuration
    pub config: Arc<Config>,
    /// Optional message queue publisher
    pub publisher: Option<Arc<Publisher>>,
    /// Broadcast channel for SSE notifications
    pub broadcast_tx: broadcast::Sender<String>,
}

impl AppState {
    /// Create new application state
    pub fn new(db: PgPool, config: Config) -> Self {
        let jwt_secret = config.security.jwt_secret.clone().unwrap_or_else(|| {
            tracing::warn!(
                "JWT_SECRET not set in config, using default (INSECURE for production!)"
            );
            "insecure_default_secret_change_in_production".to_string()
        });

        let jwt_config = JwtConfig {
            secret: jwt_secret,
            access_token_expiration: config.security.jwt_access_expiration as i64,
            refresh_token_expiration: config.security.jwt_refresh_expiration as i64,
        };

        let cors_origins = config.server.cors_origins.clone();

        // Create broadcast channel for SSE notifications (capacity 1000)
        let (broadcast_tx, _) = broadcast::channel(1000);

        Self {
            db,
            jwt_config: Arc::new(jwt_config),
            cors_origins,
            config: Arc::new(config),
            publisher: None,
            broadcast_tx,
        }
    }

    /// Set the message queue publisher
    pub fn with_publisher(mut self, publisher: Arc<Publisher>) -> Self {
        self.publisher = Some(publisher);
        self
    }
}

/// Type alias for Arc-wrapped application state
/// Used by Axum handlers
pub type SharedState = Arc<AppState>;
