//! Attune Sensor Service
//!
//! The Sensor Service monitors for trigger conditions and generates events.
//! It executes custom sensor code, manages sensor lifecycle, and publishes
//! events to the message queue for rule matching and enforcement creation.

use anyhow::Result;
use attune_common::config::Config;
use attune_sensor::service::SensorService;
use clap::Parser;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "attune-sensor")]
#[command(about = "Attune Sensor Service - Event monitoring and generation", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing with specified log level
    let log_level = args.log_level.parse().unwrap_or(tracing::Level::INFO);
    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Attune Sensor Service");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    if let Some(config_path) = args.config {
        info!("Loading configuration from: {}", config_path);
        std::env::set_var("ATTUNE_CONFIG", config_path);
    }

    let config = Config::load()?;
    config.validate()?;

    info!("Configuration loaded successfully");
    info!("Environment: {}", config.environment);
    info!("Database: {}", mask_connection_string(&config.database.url));
    if let Some(ref mq_config) = config.message_queue {
        info!("Message Queue: {}", mask_connection_string(&mq_config.url));
    }

    // Create and start sensor service
    let service = SensorService::new(config).await?;

    info!("Sensor Service initialized successfully");

    // Start the service (spawns background tasks and returns)
    info!("Starting Sensor Service components...");
    service.start().await?;

    info!("Attune Sensor Service is ready");

    // Setup signal handlers for graceful shutdown
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    tokio::select! {
        _ = sigint.recv() => {
            info!("Received SIGINT signal");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM signal");
        }
    }

    info!("Shutting down gracefully...");

    // Stop the service: deregister worker, stop sensors, clean up connections
    if let Err(e) = service.stop().await {
        error!("Error during shutdown: {}", e);
    }

    info!("Attune Sensor Service shutdown complete");

    Ok(())
}

/// Mask sensitive parts of connection strings for logging
fn mask_connection_string(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(proto_end) = url.find("://") {
            let protocol = &url[..proto_end + 3];
            let host_and_path = &url[at_pos..];
            return format!("{}***:***{}", protocol, host_and_path);
        }
    }
    "***:***@***".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_connection_string() {
        let url = "postgresql://user:password@localhost:5432/attune";
        let masked = mask_connection_string(url);
        assert!(!masked.contains("user"));
        assert!(!masked.contains("password"));
        assert!(masked.contains("@localhost"));
    }

    #[test]
    fn test_mask_connection_string_no_credentials() {
        let url = "postgresql://localhost:5432/attune";
        let masked = mask_connection_string(url);
        assert_eq!(masked, "***:***@***");
    }

    #[test]
    fn test_mask_rabbitmq_connection() {
        let url = "amqp://admin:secret@rabbitmq:5672/%2F";
        let masked = mask_connection_string(url);
        assert!(!masked.contains("admin"));
        assert!(!masked.contains("secret"));
        assert!(masked.contains("@rabbitmq"));
    }
}
