//! Attune Sensor Service
//!
//! The Sensor Service monitors for trigger conditions and generates events.

use anyhow::Result;
use attune_common::config::Config;
use attune_sensor::startup::{
    init_tracing, log_config_details, run_sensor_service, set_config_path,
};
use clap::Parser;
use tracing::info;

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
    attune_common::auth::install_crypto_provider();

    let args = Args::parse();
    let log_level = args.log_level.parse().unwrap_or(tracing::Level::INFO);
    init_tracing(log_level);

    info!("Starting Attune Sensor Service");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    set_config_path(args.config.as_deref());

    let config = Config::load()?;
    config.validate()?;

    log_config_details(&config);
    run_sensor_service(config, "Attune Sensor Service is ready").await?;
    info!("Attune Sensor Service shutdown complete");

    Ok(())
}
