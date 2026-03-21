//! Attune Universal Worker Agent
//!
//! This is the entrypoint for the universal worker agent binary (`attune-agent`).
//! Unlike the standard `attune-worker` binary which requires explicit runtime
//! configuration, the agent automatically detects available interpreters in the
//! container environment and configures itself accordingly.
//!
//! ## Usage
//!
//! The agent is designed to be injected into any container image. On startup it:
//!
//! 1. Probes the system for available interpreters (python3, node, bash, etc.)
//! 2. Sets `ATTUNE_WORKER_RUNTIMES` based on what it finds
//! 3. Loads configuration (env vars are the primary config source)
//! 4. Initializes and runs the standard `WorkerService`
//!
//! ## Configuration
//!
//! Environment variables (primary):
//! - `ATTUNE__DATABASE__URL` — PostgreSQL connection string
//! - `ATTUNE__MESSAGE_QUEUE__URL` — RabbitMQ connection string
//! - `ATTUNE_WORKER_RUNTIMES` — Override auto-detection with explicit runtime list
//! - `ATTUNE_CONFIG` — Path to optional config YAML file
//!
//! CLI arguments:
//! - `--config` / `-c` — Path to configuration file (optional)
//! - `--name` / `-n` — Worker name override
//! - `--detect-only` — Run runtime detection, print results, and exit

use anyhow::Result;
use attune_common::config::Config;
use clap::Parser;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{info, warn};

use attune_worker::dynamic_runtime::auto_register_detected_runtimes;
use attune_worker::runtime_detect::{detect_runtimes, print_detection_report};
use attune_worker::service::WorkerService;

#[derive(Parser, Debug)]
#[command(name = "attune-agent")]
#[command(
    about = "Attune Universal Worker Agent - Injected into any container to auto-detect and execute actions",
    long_about = "The Attune Agent automatically discovers available runtime interpreters \
                  in the current environment and registers as a worker capable of executing \
                  actions for those runtimes. It is designed to be injected into arbitrary \
                  container images without requiring manual runtime configuration."
)]
struct Args {
    /// Path to configuration file (optional — env vars are the primary config source)
    #[arg(short, long)]
    config: Option<String>,

    /// Worker name (overrides config and auto-generated name)
    #[arg(short, long)]
    name: Option<String>,

    /// Run runtime detection, print results, and exit without starting the worker
    #[arg(long)]
    detect_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install HMAC-only JWT crypto provider (must be before any token operations)
    attune_common::auth::install_crypto_provider();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    let args = Args::parse();

    info!("Starting Attune Universal Worker Agent");

    // --- Phase 1: Runtime auto-detection ---
    //
    // Check if the user has explicitly set ATTUNE_WORKER_RUNTIMES. If so, skip
    // auto-detection and respect their override. Otherwise, probe the system for
    // available interpreters.
    let runtimes_override = std::env::var("ATTUNE_WORKER_RUNTIMES").ok();

    // Holds the detected runtimes so we can pass them to WorkerService later.
    // Populated only when auto-detection actually runs (no env var override).
    let mut agent_detected_runtimes: Option<Vec<attune_worker::runtime_detect::DetectedRuntime>> =
        None;

    if let Some(ref override_value) = runtimes_override {
        info!(
            "ATTUNE_WORKER_RUNTIMES already set (override), skipping auto-detection: {}",
            override_value
        );
    } else {
        info!("No ATTUNE_WORKER_RUNTIMES override — running auto-detection...");

        let detected = detect_runtimes();

        if detected.is_empty() {
            warn!("No runtimes detected! The agent may not be able to execute any actions.");
        } else {
            info!("Detected {} runtime(s):", detected.len());
            for rt in &detected {
                match &rt.version {
                    Some(ver) => info!("  ✓ {} — {} ({})", rt.name, rt.path, ver),
                    None => info!("  ✓ {} — {}", rt.name, rt.path),
                }
            }

            // Build comma-separated runtime list and set the env var so that
            // Config::load() and WorkerService pick it up downstream.
            let runtime_list: Vec<&str> = detected.iter().map(|r| r.name.as_str()).collect();
            let runtime_csv = runtime_list.join(",");
            info!("Setting ATTUNE_WORKER_RUNTIMES={}", runtime_csv);
            std::env::set_var("ATTUNE_WORKER_RUNTIMES", &runtime_csv);

            // Stash for Phase 2: pass to WorkerService for rich capability registration
            agent_detected_runtimes = Some(detected);
        }
    }

    // --- Handle --detect-only ---
    if args.detect_only {
        if runtimes_override.is_some() {
            // User set an override, but --detect-only should show what's actually
            // on this system regardless, so re-run detection.
            info!(
                "--detect-only: re-running detection to show what is available on this system..."
            );
            println!("NOTE: ATTUNE_WORKER_RUNTIMES is set — auto-detection was skipped during normal startup.");
            println!("      Showing what auto-detection would find on this system:");
            println!();
            let detected = detect_runtimes();
            print_detection_report(&detected);
        } else {
            // We already ran detection above; re-run to get a fresh Vec for the report
            // (the previous one was consumed by env var setup).
            let detected = detect_runtimes();
            print_detection_report(&detected);
        }
        return Ok(());
    }

    // --- Phase 2: Load configuration ---
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
                shutdown_timeout: Some(30),
                stream_logs: true,
            });
        }
    }

    info!("Configuration loaded successfully");
    info!("Environment: {}", config.environment);

    // --- Phase 2b: Dynamic runtime registration ---
    //
    // Before creating the WorkerService (which loads runtimes from the DB into
    // its runtime registry), ensure that every detected runtime has a
    // corresponding entry in the database. This handles the case where the
    // agent detects a runtime (e.g., Ruby) that has a template in the core
    // pack but hasn't been explicitly loaded by this agent before.
    if let Some(ref detected) = agent_detected_runtimes {
        info!(
            "Ensuring {} detected runtime(s) are registered in the database...",
            detected.len()
        );

        // We need a temporary DB connection for dynamic registration.
        // WorkerService::new() will create its own connection, so this is
        // a short-lived pool just for the registration step.
        let db = attune_common::db::Database::new(&config.database).await?;
        let pool = db.pool().clone();

        match auto_register_detected_runtimes(&pool, detected).await {
            Ok(count) => {
                if count > 0 {
                    info!(
                        "Dynamic registration complete: {} new runtime(s) added to database",
                        count
                    );
                } else {
                    info!("Dynamic registration: all detected runtimes already in database");
                }
            }
            Err(e) => {
                warn!(
                    "Dynamic runtime registration failed (non-fatal, continuing): {}",
                    e
                );
            }
        }
    }

    // --- Phase 3: Initialize and run the worker service ---
    let service = WorkerService::new(config).await?;

    // If we auto-detected runtimes, pass them to the worker service so that
    // registration includes the full `detected_interpreters` capability
    // (binary paths + versions) and the `agent_mode` flag.
    let mut service = if let Some(detected) = agent_detected_runtimes {
        info!(
            "Passing {} detected runtime(s) to worker registration",
            detected.len()
        );
        service.with_detected_runtimes(detected)
    } else {
        service
    };

    info!("Attune Agent is ready");

    service.start().await?;

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

    service.stop().await?;

    info!("Attune Agent shutdown complete");

    Ok(())
}
