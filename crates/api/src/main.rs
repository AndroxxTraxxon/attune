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

/// Attempt to connect to RabbitMQ and create a publisher.
/// Returns the publisher on success.
async fn try_connect_publisher(mq_url: &str) -> Result<Publisher> {
    let mq_connection = Connection::connect(mq_url).await?;

    // Setup common message queue infrastructure (exchanges and DLX)
    let mq_setup_config = attune_common::mq::MessageQueueConfig::default();
    if let Err(e) = mq_connection
        .setup_common_infrastructure(&mq_setup_config)
        .await
    {
        warn!(
            "Failed to setup common MQ infrastructure (may already exist): {}",
            e
        );
    }

    let publisher = Publisher::new(
        &mq_connection,
        PublisherConfig {
            confirm_publish: true,
            timeout_secs: 30,
            exchange: "attune.executions".to_string(),
        },
    )
    .await?;

    Ok(publisher)
}

/// Background task that keeps trying to establish the MQ publisher connection.
/// Once connected it installs the publisher into `state`, then monitors the
/// connection health and reconnects if it drops.
async fn mq_reconnect_loop(state: Arc<AppState>, mq_url: String) {
    // Retry delay sequence (seconds): 1, 2, 4, 8, 16, 30, 30, …
    let delays: &[u64] = &[1, 2, 4, 8, 16, 30];
    let mut attempt: usize = 0;

    loop {
        let delay = delays.get(attempt).copied().unwrap_or(30);

        match try_connect_publisher(&mq_url).await {
            Ok(publisher) => {
                info!(
                    "Message queue publisher connected (attempt {})",
                    attempt + 1
                );
                state.set_publisher(Arc::new(publisher)).await;
                attempt = 0; // reset backoff after a successful connect

                // Poll liveness: the publisher will error on use when the
                // underlying channel is gone.  We do a lightweight wait here so
                // we notice disconnections and attempt to reconnect.
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    if state.get_publisher().await.is_none() {
                        // Something cleared the publisher externally; re-enter
                        // the outer connect loop.
                        break;
                    }
                    // TODO: add a real health-check ping when the lapin API
                    // exposes one (e.g. channel.basic_noop).  For now a broken
                    // publisher will be detected on the first failed publish and
                    // can be cleared by the handler to trigger reconnection here.
                }
            }
            Err(e) => {
                warn!(
                    "Failed to connect to message queue (attempt {}, retrying in {}s): {}",
                    attempt + 1,
                    delay,
                    e
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                attempt = attempt.saturating_add(1);
            }
        }
    }
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

    // Initialize application state (publisher starts as None)
    let state = Arc::new(AppState::new(database.pool().clone(), config.clone()));

    // Spawn background MQ reconnect loop if a message queue is configured.
    // The loop will keep retrying until it connects, then install the publisher
    // into the shared state so request handlers can use it immediately.
    if let Some(ref mq_config) = config.message_queue {
        info!("Message queue configured – starting background connection loop...");
        let mq_url = mq_config.url.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            mq_reconnect_loop(state_clone, mq_url).await;
        });
    } else {
        warn!("Message queue not configured – executions will not be queued for processing");
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
    let server = Server::new(state.clone());

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
