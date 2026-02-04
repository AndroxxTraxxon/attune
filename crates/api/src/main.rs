//! Attune API Service
//!
//! REST API gateway for all client interactions with the Attune platform.
//! Provides endpoints for managing packs, actions, triggers, rules, executions,
//! inquiries, and other automation components.

use anyhow::Result;
use attune_common::{
    config::Config,
    db::Database,
    mq::{Connection, Publisher, PublisherConfig},
};
use clap::Parser;
use std::sync::Arc;
use tracing::{info, warn};

use attune_api::{postgres_listener, AppState, Server};

#[derive(Parser, Debug)]
#[command(name = "attune-api")]
#[command(about = "Attune API Service", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Server host address
    #[arg(long)]
    host: Option<String>,

    /// Server port
    #[arg(long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    let args = Args::parse();

    info!("Starting Attune API Service");

    // Load configuration
    if let Some(config_path) = args.config {
        std::env::set_var("ATTUNE_CONFIG", config_path);
    }

    let config = Config::load()?;
    config.validate()?;

    info!("Configuration loaded successfully");
    info!("Environment: {}", config.environment);
    info!(
        "Server will bind to {}:{}",
        config.server.host, config.server.port
    );

    // Initialize database connection pool
    info!("Connecting to database...");
    let database = Database::new(&config.database).await?;
    info!("Database connection established");

    // Initialize message queue connection and publisher (optional)
    let mut state = AppState::new(database.pool().clone(), config.clone());

    if let Some(ref mq_config) = config.message_queue {
        info!("Connecting to message queue...");
        match Connection::connect(&mq_config.url).await {
            Ok(mq_connection) => {
                info!("Message queue connection established");

                // Create publisher
                match Publisher::new(
                    &mq_connection,
                    PublisherConfig {
                        confirm_publish: true,
                        timeout_secs: 30,
                        exchange: "attune.executions".to_string(),
                    },
                )
                .await
                {
                    Ok(publisher) => {
                        info!("Message queue publisher initialized");
                        state = state.with_publisher(Arc::new(publisher));
                    }
                    Err(e) => {
                        warn!("Failed to create publisher: {}", e);
                        warn!("Executions will not be queued for processing");
                    }
                }
            }
            Err(e) => {
                warn!("Failed to connect to message queue: {}", e);
                warn!("Executions will not be queued for processing");
            }
        }
    } else {
        warn!("Message queue not configured");
        warn!("Executions will not be queued for processing");
    }

    info!(
        "CORS configured with {} allowed origin(s)",
        if config.server.cors_origins.is_empty() {
            "default development"
        } else {
            "custom"
        }
    );

    // Start PostgreSQL listener for SSE broadcasting
    let broadcast_tx = state.broadcast_tx.clone();
    let listener_db = database.pool().clone();
    tokio::spawn(async move {
        if let Err(e) = postgres_listener::start_postgres_listener(listener_db, broadcast_tx).await
        {
            tracing::error!("PostgreSQL listener error: {}", e);
        }
    });

    info!("PostgreSQL notification listener started");

    // Create and start server
    let server = Server::new(std::sync::Arc::new(state));

    info!("Attune API Service is ready");

    // Run server with graceful shutdown
    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
                return Err(e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    info!("Shutting down Attune API Service");

    Ok(())
}
