//! Attune Worker Service

use anyhow::Result;
use attune_common::config::Config;
use clap::Parser;
use tracing::info;

use attune_worker::service::WorkerService;

#[derive(Parser, Debug)]
#[command(name = "attune-worker")]
#[command(about = "Attune Worker Service - Executes automation actions", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Worker name (overrides config)
    #[arg(short, long)]
    name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    let args = Args::parse();

    info!("Starting Attune Worker Service");

    // Load configuration
    if let Some(config_path) = args.config {
        std::env::set_var("ATTUNE_CONFIG", config_path);
    }

    let mut config = Config::load()?;
    config.validate()?;

    // Override worker name if provided via CLI
    if let Some(name) = args.name {
        if let Some(ref mut worker_config) = config.worker {
            worker_config.name = Some(name);
        } else {
            config.worker = Some(attune_common::config::WorkerConfig {
                name: Some(name),
                worker_type: None,
                runtime_id: None,
                host: None,
                port: None,
                capabilities: None,
                max_concurrent_tasks: 10,
                heartbeat_interval: 30,
                task_timeout: 300,
                max_stdout_bytes: 10 * 1024 * 1024,
                max_stderr_bytes: 10 * 1024 * 1024,
                stream_logs: true,
            });
        }
    }

    info!("Configuration loaded successfully");
    info!("Environment: {}", config.environment);

    // Initialize and run worker service
    let mut service = WorkerService::new(config).await?;

    info!("Attune Worker Service is ready");

    // Run until interrupted
    service.run().await?;

    info!("Attune Worker Service shutdown complete");

    Ok(())
}
