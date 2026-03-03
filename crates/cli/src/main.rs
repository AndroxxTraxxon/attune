use clap::{Parser, Subcommand};
use std::process;

mod client;
mod commands;
mod config;
mod output;
mod wait;

use commands::{
    action::{handle_action_command, ActionCommands},
    auth::AuthCommands,
    config::ConfigCommands,
    execution::ExecutionCommands,
    pack::PackCommands,
    rule::RuleCommands,
    sensor::SensorCommands,
    trigger::TriggerCommands,
};

#[derive(Parser)]
#[command(name = "attune")]
#[command(author, version, about = "Attune CLI - Event-driven automation platform", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Profile to use (overrides config)
    #[arg(short = 'p', long, env = "ATTUNE_PROFILE", global = true)]
    profile: Option<String>,

    /// API endpoint URL (overrides config)
    #[arg(long, env = "ATTUNE_API_URL", global = true)]
    api_url: Option<String>,

    /// Output format
    #[arg(long, value_enum, default_value = "table", global = true, conflicts_with_all = ["json", "yaml"])]
    output: output::OutputFormat,

    /// Output as JSON (shorthand for --output json)
    #[arg(short = 'j', long, global = true, conflicts_with_all = ["output", "yaml"])]
    json: bool,

    /// Output as YAML (shorthand for --output yaml)
    #[arg(short = 'y', long, global = true, conflicts_with_all = ["output", "json"])]
    yaml: bool,

    /// Verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authentication commands
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Pack management
    Pack {
        #[command(subcommand)]
        command: PackCommands,
    },
    /// Action management and execution
    Action {
        #[command(subcommand)]
        command: ActionCommands,
    },
    /// Rule management
    Rule {
        #[command(subcommand)]
        command: RuleCommands,
    },
    /// Execution monitoring
    Execution {
        #[command(subcommand)]
        command: ExecutionCommands,
    },
    /// Trigger management
    Trigger {
        #[command(subcommand)]
        command: TriggerCommands,
    },
    /// Sensor management
    Sensor {
        #[command(subcommand)]
        command: SensorCommands,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Run an action (shortcut for 'action execute')
    Run {
        /// Action reference (pack.action)
        action_ref: String,

        /// Action parameters in key=value format
        #[arg(long)]
        param: Vec<String>,

        /// Parameters as JSON string
        #[arg(long, conflicts_with = "param")]
        params_json: Option<String>,

        /// Wait for execution to complete
        #[arg(short, long)]
        wait: bool,

        /// Timeout in seconds when waiting (default: 300)
        #[arg(long, default_value = "300", requires = "wait")]
        timeout: u64,

        /// Notifier WebSocket base URL (e.g. ws://localhost:8081).
        /// Derived from --api-url automatically when not set.
        #[arg(long, requires = "wait")]
        notifier_url: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    // Determine output format from flags
    let output_format = if cli.json {
        output::OutputFormat::Json
    } else if cli.yaml {
        output::OutputFormat::Yaml
    } else {
        cli.output
    };

    let result = match cli.command {
        Commands::Auth { command } => {
            commands::auth::handle_auth_command(&cli.profile, command, &cli.api_url, output_format)
                .await
        }
        Commands::Pack { command } => {
            commands::pack::handle_pack_command(&cli.profile, command, &cli.api_url, output_format)
                .await
        }
        Commands::Action { command } => {
            commands::action::handle_action_command(
                &cli.profile,
                command,
                &cli.api_url,
                output_format,
            )
            .await
        }
        Commands::Rule { command } => {
            commands::rule::handle_rule_command(&cli.profile, command, &cli.api_url, output_format)
                .await
        }
        Commands::Execution { command } => {
            commands::execution::handle_execution_command(
                &cli.profile,
                command,
                &cli.api_url,
                output_format,
            )
            .await
        }
        Commands::Trigger { command } => {
            commands::trigger::handle_trigger_command(
                &cli.profile,
                command,
                &cli.api_url,
                output_format,
            )
            .await
        }
        Commands::Sensor { command } => {
            commands::sensor::handle_sensor_command(
                &cli.profile,
                command,
                &cli.api_url,
                output_format,
            )
            .await
        }
        Commands::Config { command } => {
            commands::config::handle_config_command(&cli.profile, command, output_format).await
        }
        Commands::Run {
            action_ref,
            param,
            params_json,
            wait,
            timeout,
            notifier_url,
        } => {
            // Delegate to action execute command
            handle_action_command(
                &cli.profile,
                ActionCommands::Execute {
                    action_ref,
                    param,
                    params_json,
                    wait,
                    timeout,
                    notifier_url,
                },
                &cli.api_url,
                output_format,
            )
            .await
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
