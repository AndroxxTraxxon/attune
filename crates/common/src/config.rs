//! Configuration management for Attune services
//!
//! This module provides configuration loading and validation for all services.
//! Configuration is loaded from YAML files with environment variable overrides.
//!
//! ## Configuration Loading Priority
//!
//! 1. Default YAML file (`config.yaml` or path from `ATTUNE_CONFIG` env var)
//! 2. Environment-specific YAML file (`config.{environment}.yaml`)
//! 3. Environment variables with `ATTUNE__` prefix (e.g., `ATTUNE__DATABASE__URL`)
//!
//! ## Example YAML Configuration
//!
//! ```yaml
//! service_name: attune
//! environment: development
//!
//! database:
//!   url: postgresql://postgres:postgres@localhost:5432/attune
//!   max_connections: 50
//!   min_connections: 5
//!
//! server:
//!   host: 0.0.0.0
//!   port: 8080
//!   cors_origins:
//!     - http://localhost:3000
//!     - http://localhost:5173
//!
//! security:
//!   jwt_secret: your-secret-key-here
//!   jwt_access_expiration: 3600
//!
//! log:
//!   level: info
//!   format: json
//! ```

use config as config_crate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Custom deserializer for fields that can be either a comma-separated string or an array
mod string_or_vec {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrVec {
            String(String),
            Vec(Vec<String>),
        }

        match StringOrVec::deserialize(deserializer)? {
            StringOrVec::String(s) => {
                // Split by comma and trim whitespace
                Ok(s.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect())
            }
            StringOrVec::Vec(v) => Ok(v),
        }
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    #[serde(default = "default_database_url")]
    pub url: String,

    /// Maximum number of connections in the pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum number of connections in the pool
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connect_timeout: u64,

    /// Idle timeout in seconds
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,

    /// Enable SQL statement logging
    #[serde(default)]
    pub log_statements: bool,

    /// PostgreSQL schema name (defaults to "attune")
    pub schema: Option<String>,
}

fn default_database_url() -> String {
    "postgresql://postgres:postgres@localhost:5432/attune".to_string()
}

fn default_max_connections() -> u32 {
    50
}

fn default_min_connections() -> u32 {
    5
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_idle_timeout() -> u64 {
    600
}

/// Redis configuration for caching and pub/sub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    #[serde(default = "default_redis_url")]
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: u32,
}

fn default_redis_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_redis_pool_size() -> u32 {
    10
}

/// Message queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueConfig {
    /// AMQP connection URL (RabbitMQ)
    #[serde(default = "default_amqp_url")]
    pub url: String,

    /// Exchange name
    #[serde(default = "default_exchange")]
    pub exchange: String,

    /// Enable dead letter queue
    #[serde(default = "default_true")]
    pub enable_dlq: bool,

    /// Message TTL in seconds
    #[serde(default = "default_message_ttl")]
    pub message_ttl: u64,
}

fn default_amqp_url() -> String {
    "amqp://guest:guest@localhost:5672/%2f".to_string()
}

fn default_exchange() -> String {
    "attune".to_string()
}

fn default_message_ttl() -> u64 {
    3600
}

fn default_true() -> bool {
    true
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to bind to
    #[serde(default = "default_port")]
    pub port: u16,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    /// Enable CORS
    #[serde(default = "default_true")]
    pub enable_cors: bool,

    /// Allowed origins for CORS
    /// Can be specified as a comma-separated string or array
    #[serde(default, deserialize_with = "string_or_vec::deserialize")]
    pub cors_origins: Vec<String>,

    /// Maximum request body size in bytes
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_request_timeout() -> u64 {
    30
}

fn default_max_body_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

/// Notifier service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifierConfig {
    /// Host to bind to
    #[serde(default = "default_notifier_host")]
    pub host: String,

    /// Port to bind to
    #[serde(default = "default_notifier_port")]
    pub port: u16,

    /// Maximum number of concurrent WebSocket connections
    #[serde(default = "default_max_connections_notifier")]
    pub max_connections: usize,
}

fn default_notifier_host() -> String {
    "0.0.0.0".to_string()
}

fn default_notifier_port() -> u16 {
    8081
}

fn default_max_connections_notifier() -> usize {
    10000
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format (json, pretty)
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Enable console logging
    #[serde(default = "default_true")]
    pub console: bool,

    /// Optional log file path
    pub file: Option<PathBuf>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// JWT secret key
    pub jwt_secret: Option<String>,

    /// JWT access token expiration in seconds
    #[serde(default = "default_jwt_access_expiration")]
    pub jwt_access_expiration: u64,

    /// JWT refresh token expiration in seconds
    #[serde(default = "default_jwt_refresh_expiration")]
    pub jwt_refresh_expiration: u64,

    /// Encryption key for secrets
    pub encryption_key: Option<String>,

    /// Enable authentication
    #[serde(default = "default_true")]
    pub enable_auth: bool,
}

fn default_jwt_access_expiration() -> u64 {
    3600 // 1 hour
}

fn default_jwt_refresh_expiration() -> u64 {
    604800 // 7 days
}

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Worker name/identifier (optional, defaults to hostname)
    pub name: Option<String>,

    /// Worker type (local, remote, container)
    pub worker_type: Option<crate::models::WorkerType>,

    /// Runtime ID this worker is associated with
    pub runtime_id: Option<i64>,

    /// Worker host (optional, defaults to hostname)
    pub host: Option<String>,

    /// Worker port
    pub port: Option<i32>,

    /// Worker capabilities (runtimes, max_concurrent_executions, etc.)
    /// Can be overridden by ATTUNE_WORKER_RUNTIMES environment variable
    pub capabilities: Option<std::collections::HashMap<String, serde_json::Value>>,

    /// Maximum concurrent tasks
    #[serde(default = "default_max_concurrent_tasks")]
    pub max_concurrent_tasks: usize,

    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,

    /// Task timeout in seconds
    #[serde(default = "default_task_timeout")]
    pub task_timeout: u64,

    /// Maximum stdout size in bytes (default 10MB)
    #[serde(default = "default_max_stdout_bytes")]
    pub max_stdout_bytes: usize,

    /// Maximum stderr size in bytes (default 10MB)
    #[serde(default = "default_max_stderr_bytes")]
    pub max_stderr_bytes: usize,

    /// Graceful shutdown timeout in seconds
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout: Option<u64>,

    /// Enable log streaming instead of buffering
    #[serde(default = "default_true")]
    pub stream_logs: bool,
}

fn default_max_concurrent_tasks() -> usize {
    10
}

fn default_heartbeat_interval() -> u64 {
    30
}

fn default_shutdown_timeout() -> Option<u64> {
    Some(30)
}

fn default_task_timeout() -> u64 {
    300 // 5 minutes
}

fn default_max_stdout_bytes() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_max_stderr_bytes() -> usize {
    10 * 1024 * 1024 // 10MB
}

/// Sensor service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    /// Sensor worker name/identifier (optional, defaults to hostname)
    pub worker_name: Option<String>,

    /// Sensor worker host (optional, defaults to hostname)
    pub host: Option<String>,

    /// Sensor worker capabilities (runtimes, max_concurrent_sensors, etc.)
    /// Can be overridden by ATTUNE_SENSOR_RUNTIMES environment variable
    pub capabilities: Option<std::collections::HashMap<String, serde_json::Value>>,

    /// Maximum concurrent sensors
    pub max_concurrent_sensors: Option<usize>,

    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,

    /// Sensor poll interval in seconds
    #[serde(default = "default_sensor_poll_interval")]
    pub poll_interval: u64,

    /// Sensor execution timeout in seconds
    #[serde(default = "default_sensor_timeout")]
    pub sensor_timeout: u64,

    /// Graceful shutdown timeout in seconds
    #[serde(default = "default_sensor_shutdown_timeout")]
    pub shutdown_timeout: u64,
}

fn default_sensor_poll_interval() -> u64 {
    30
}

fn default_sensor_timeout() -> u64 {
    30
}

fn default_sensor_shutdown_timeout() -> u64 {
    30
}

/// Pack registry index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndexConfig {
    /// Registry index URL (https://, http://, or file://)
    pub url: String,

    /// Registry priority (lower number = higher priority)
    #[serde(default = "default_registry_priority")]
    pub priority: u32,

    /// Whether this registry is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Human-readable registry name
    pub name: Option<String>,

    /// Custom HTTP headers for authenticated registries
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_registry_priority() -> u32 {
    100
}

/// Pack registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackRegistryConfig {
    /// Enable pack registry system
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// List of registry indices
    #[serde(default)]
    pub indices: Vec<RegistryIndexConfig>,

    /// Cache TTL in seconds (how long to cache index files)
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,

    /// Enable registry index caching
    #[serde(default = "default_true")]
    pub cache_enabled: bool,

    /// Download timeout in seconds
    #[serde(default = "default_registry_timeout")]
    pub timeout: u64,

    /// Verify checksums during installation
    #[serde(default = "default_true")]
    pub verify_checksums: bool,

    /// Allow HTTP (non-HTTPS) registries
    #[serde(default)]
    pub allow_http: bool,
}

fn default_cache_ttl() -> u64 {
    3600 // 1 hour
}

fn default_registry_timeout() -> u64 {
    120 // 2 minutes
}

impl Default for PackRegistryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            indices: Vec::new(),
            cache_ttl: default_cache_ttl(),
            cache_enabled: true,
            timeout: default_registry_timeout(),
            verify_checksums: true,
            allow_http: false,
        }
    }
}

/// Executor service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// How long an execution can remain in SCHEDULED status before timing out (seconds)
    #[serde(default)]
    pub scheduled_timeout: Option<u64>,

    /// How often to check for stale executions (seconds)
    #[serde(default)]
    pub timeout_check_interval: Option<u64>,

    /// Whether to enable the execution timeout monitor
    #[serde(default)]
    pub enable_timeout_monitor: Option<bool>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            scheduled_timeout: Some(300),     // 5 minutes
            timeout_check_interval: Some(60), // 1 minute
            enable_timeout_monitor: Some(true),
        }
    }
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Service name
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Environment (development, staging, production)
    #[serde(default = "default_environment")]
    pub environment: String,

    /// Database configuration
    #[serde(default)]
    pub database: DatabaseConfig,

    /// Redis configuration
    #[serde(default)]
    pub redis: Option<RedisConfig>,

    /// Message queue configuration
    #[serde(default)]
    pub message_queue: Option<MessageQueueConfig>,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Logging configuration
    #[serde(default)]
    pub log: LogConfig,

    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,

    /// Worker configuration (optional, for worker services)
    pub worker: Option<WorkerConfig>,

    /// Sensor configuration (optional, for sensor services)
    pub sensor: Option<SensorConfig>,

    /// Packs base directory (where pack directories are located)
    #[serde(default = "default_packs_base_dir")]
    pub packs_base_dir: String,

    /// Notifier configuration (optional, for notifier service)
    pub notifier: Option<NotifierConfig>,

    /// Pack registry configuration
    #[serde(default)]
    pub pack_registry: PackRegistryConfig,

    /// Executor configuration (optional, for executor service)
    pub executor: Option<ExecutorConfig>,
}

fn default_service_name() -> String {
    "attune".to_string()
}

fn default_environment() -> String {
    "development".to_string()
}

fn default_packs_base_dir() -> String {
    "/opt/attune/packs".to_string()
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_database_url(),
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            connect_timeout: default_connection_timeout(),
            idle_timeout: default_idle_timeout(),
            log_statements: false,
            schema: None,
        }
    }
}

impl Default for NotifierConfig {
    fn default() -> Self {
        Self {
            host: default_notifier_host(),
            port: default_notifier_port(),
            max_connections: default_max_connections_notifier(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            request_timeout: default_request_timeout(),
            enable_cors: true,
            cors_origins: vec![],
            max_body_size: default_max_body_size(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            console: true,
            file: None,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: None,
            jwt_access_expiration: default_jwt_access_expiration(),
            jwt_refresh_expiration: default_jwt_refresh_expiration(),
            encryption_key: None,
            enable_auth: true,
        }
    }
}

impl Config {
    /// Load configuration from YAML files and environment variables
    ///
    /// Loading priority (later sources override earlier ones):
    /// 1. Base config file (config.yaml or ATTUNE_CONFIG env var)
    /// 2. Environment-specific config (config.{environment}.yaml)
    /// 3. Environment variables (ATTUNE__ prefix)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use attune_common::config::Config;
    /// // Load from default config.yaml
    /// let config = Config::load().unwrap();
    ///
    /// // Load from custom path
    /// std::env::set_var("ATTUNE_CONFIG", "/path/to/config.yaml");
    /// let config = Config::load().unwrap();
    ///
    /// // Override with environment variables
    /// std::env::set_var("ATTUNE__DATABASE__URL", "postgresql://localhost/mydb");
    /// let config = Config::load().unwrap();
    /// ```
    pub fn load() -> crate::Result<Self> {
        let mut builder = config_crate::Config::builder();

        // 1. Load base config file
        let config_path =
            std::env::var("ATTUNE_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());

        // Try to load the base config file (optional)
        if std::path::Path::new(&config_path).exists() {
            builder =
                builder.add_source(config_crate::File::with_name(&config_path).required(false));
        }

        // 2. Load environment-specific config file (e.g., config.development.yaml)
        // First, we need to get the environment from env var or default
        let environment =
            std::env::var("ATTUNE__ENVIRONMENT").unwrap_or_else(|_| default_environment());

        let env_config_path = format!("config.{}.yaml", environment);
        if std::path::Path::new(&env_config_path).exists() {
            builder =
                builder.add_source(config_crate::File::with_name(&env_config_path).required(false));
        }

        // 3. Load environment variables (highest priority)
        builder = builder.add_source(
            config_crate::Environment::with_prefix("ATTUNE")
                .separator("__")
                .try_parsing(true),
        );

        let config: config_crate::Config = builder
            .build()
            .map_err(|e: config_crate::ConfigError| crate::Error::configuration(e.to_string()))?;

        config
            .try_deserialize::<Self>()
            .map_err(|e: config_crate::ConfigError| crate::Error::configuration(e.to_string()))
    }

    /// Load configuration from a specific file path
    ///
    /// This bypasses the default config file discovery and loads directly from the specified path.
    /// Environment variables can still override values.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the YAML configuration file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use attune_common::config::Config;
    /// let config = Config::load_from_file("./config.production.yaml").unwrap();
    /// ```
    pub fn load_from_file(path: &str) -> crate::Result<Self> {
        let mut builder = config_crate::Config::builder();

        // Load from specified file
        builder = builder.add_source(config_crate::File::with_name(path).required(true));

        // Load environment variables (for overrides)
        builder = builder.add_source(
            config_crate::Environment::with_prefix("ATTUNE")
                .separator("__")
                .try_parsing(true)
                .list_separator(","),
        );

        let config: config_crate::Config = builder
            .build()
            .map_err(|e: config_crate::ConfigError| crate::Error::configuration(e.to_string()))?;

        config
            .try_deserialize::<Self>()
            .map_err(|e: config_crate::ConfigError| crate::Error::configuration(e.to_string()))
    }

    /// Validate configuration
    pub fn validate(&self) -> crate::Result<()> {
        // Validate database URL
        if self.database.url.is_empty() {
            return Err(crate::Error::validation("Database URL cannot be empty"));
        }

        // Validate JWT secret if auth is enabled
        if self.security.enable_auth && self.security.jwt_secret.is_none() {
            return Err(crate::Error::validation(
                "JWT secret is required when authentication is enabled",
            ));
        }

        // Validate encryption key if provided
        if let Some(ref key) = self.security.encryption_key {
            if key.len() < 32 {
                return Err(crate::Error::validation(
                    "Encryption key must be at least 32 characters",
                ));
            }
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.log.level.as_str()) {
            return Err(crate::Error::validation(format!(
                "Invalid log level: {}. Must be one of: {:?}",
                self.log.level, valid_levels
            )));
        }

        // Validate log format
        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&self.log.format.as_str()) {
            return Err(crate::Error::validation(format!(
                "Invalid log format: {}. Must be one of: {:?}",
                self.log.format, valid_formats
            )));
        }

        Ok(())
    }

    /// Check if running in production
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }

    /// Check if running in development
    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config {
            service_name: default_service_name(),
            environment: default_environment(),
            database: DatabaseConfig::default(),
            redis: None,
            message_queue: None,
            server: ServerConfig::default(),
            log: LogConfig::default(),
            security: SecurityConfig::default(),
            worker: None,
            sensor: None,
            packs_base_dir: default_packs_base_dir(),
            notifier: None,
            pack_registry: PackRegistryConfig::default(),
        };

        assert_eq!(config.service_name, "attune");
        assert_eq!(config.environment, "development");
        assert!(config.is_development());
        assert!(!config.is_production());
    }

    #[test]
    fn test_cors_origins_deserializer() {
        use serde_json::json;

        // Test with comma-separated string
        let json_str = json!({
            "cors_origins": "http://localhost:3000,http://localhost:5173,http://test.com"
        });
        let config: ServerConfig = serde_json::from_value(json_str).unwrap();
        assert_eq!(config.cors_origins.len(), 3);
        assert_eq!(config.cors_origins[0], "http://localhost:3000");
        assert_eq!(config.cors_origins[1], "http://localhost:5173");
        assert_eq!(config.cors_origins[2], "http://test.com");

        // Test with array format
        let json_array = json!({
            "cors_origins": ["http://localhost:3000", "http://localhost:5173"]
        });
        let config: ServerConfig = serde_json::from_value(json_array).unwrap();
        assert_eq!(config.cors_origins.len(), 2);
        assert_eq!(config.cors_origins[0], "http://localhost:3000");
        assert_eq!(config.cors_origins[1], "http://localhost:5173");

        // Test with empty string
        let json_empty = json!({
            "cors_origins": ""
        });
        let config: ServerConfig = serde_json::from_value(json_empty).unwrap();
        assert_eq!(config.cors_origins.len(), 0);

        // Test with string containing spaces - should trim properly
        let json_spaces = json!({
            "cors_origins": "http://localhost:3000 , http://localhost:5173 , http://test.com"
        });
        let config: ServerConfig = serde_json::from_value(json_spaces).unwrap();
        assert_eq!(config.cors_origins.len(), 3);
        assert_eq!(config.cors_origins[0], "http://localhost:3000");
        assert_eq!(config.cors_origins[1], "http://localhost:5173");
        assert_eq!(config.cors_origins[2], "http://test.com");
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config {
            service_name: default_service_name(),
            environment: default_environment(),
            database: DatabaseConfig::default(),
            redis: None,
            message_queue: None,
            server: ServerConfig::default(),
            log: LogConfig::default(),
            security: SecurityConfig {
                jwt_secret: Some("test_secret".to_string()),
                jwt_access_expiration: 3600,
                jwt_refresh_expiration: 604800,
                encryption_key: Some("a".repeat(32)),
                enable_auth: true,
            },
            worker: None,
            sensor: None,
            packs_base_dir: default_packs_base_dir(),
            notifier: None,
            pack_registry: PackRegistryConfig::default(),
        };

        assert!(config.validate().is_ok());

        // Test invalid encryption key
        config.security.encryption_key = Some("short".to_string());
        assert!(config.validate().is_err());

        // Test missing JWT secret
        config.security.encryption_key = Some("a".repeat(32));
        config.security.jwt_secret = None;
        assert!(config.validate().is_err());
    }
}
