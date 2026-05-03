//! Attune Timer Sensor
//!
//! A standalone sensor daemon that monitors timer-based triggers and emits events
//! to the Attune platform. Each timer sensor instance manages multiple timer schedules
//! based on active rules.
//!
//! Configuration is provided via environment variables or stdin JSON:
//! - ATTUNE_API_URL: Base URL of the Attune API
//! - ATTUNE_API_TOKEN: Service account token for authentication
//! - ATTUNE_SENSOR_REF: Reference name for this sensor (e.g., "core.timer")
//! - ATTUNE_MQ_URL: RabbitMQ connection URL
//! - ATTUNE_MQ_EXCHANGE: RabbitMQ exchange name (default: "attune")
//! - ATTUNE_LOG_LEVEL: Logging verbosity (default: "info")

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info};

mod api_client;
mod config;
mod rule_listener;
mod timer_manager;
mod token_refresh;
mod types;

use config::SensorConfig;
use rule_listener::RuleLifecycleListener;
use timer_manager::TimerManager;
use token_refresh::TokenRefreshManager;

#[derive(Parser, Debug)]
#[command(name = "attune-core-timer-sensor")]
#[command(about = "Standalone timer sensor for Attune automation platform", long_about = None)]
struct Args {
    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Read configuration from stdin as JSON instead of environment variables
    #[arg(long)]
    stdin_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = args.log_level.parse().unwrap_or(tracing::Level::INFO);

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(true)
        .json()
        .init();

    info!("Starting Attune Timer Sensor");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = if args.stdin_config {
        info!("Reading configuration from stdin");
        SensorConfig::from_stdin().await?
    } else {
        info!("Reading configuration from environment variables");
        SensorConfig::from_env()?
    };

    config.validate()?;
    info!(
        "Configuration loaded successfully: sensor_ref={}, api_url={}",
        config.sensor_ref, config.api_url
    );

    // Create API client
    let api_client = api_client::ApiClient::new(config.api_url.clone(), config.api_token.clone());

    // Verify API connectivity
    info!("Verifying API connectivity...");
    api_client
        .health_check()
        .await
        .context("Failed to connect to Attune API")?;
    info!("API connectivity verified");

    // Create timer manager
    let timer_manager = TimerManager::new(api_client.clone(), config.sensor_ref.clone())
        .await
        .context("Failed to initialize timer manager")?;
    info!("Timer manager initialized");

    // Create rule lifecycle listener
    let listener = RuleLifecycleListener::new(
        config.mq_url.clone(),
        config.mq_exchange.clone(),
        config.sensor_ref.clone(),
        api_client.clone(),
        timer_manager.clone(),
    );

    info!("Rule lifecycle listener initialized");

    // Start token refresh manager (auto-refresh when 80% of TTL elapsed)
    let refresh_manager = TokenRefreshManager::new(api_client.clone(), 0.8);
    let _refresh_handle = refresh_manager.start();
    info!("Token refresh manager started (will refresh at 80% of TTL)");

    // Set up graceful shutdown handler
    let timer_manager_clone = timer_manager.clone();
    let shutdown_signal = tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Shutdown signal received");
                if let Err(e) = timer_manager_clone.shutdown().await {
                    error!("Error during timer manager shutdown: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to listen for shutdown signal: {}", e);
            }
        }
    });

    // Start the listener (this will block until stopped)
    info!("Starting rule lifecycle listener...");
    match listener.start().await {
        Ok(()) => {
            info!("Rule lifecycle listener stopped gracefully");
        }
        Err(e) => {
            error!("Rule lifecycle listener error: {}", e);
            return Err(e);
        }
    }

    // Wait for shutdown to complete
    let _ = shutdown_signal.await;

    // Ensure timer manager is fully shutdown
    timer_manager.shutdown().await?;

    info!("Timer sensor has shut down gracefully");
    Ok(())
}
