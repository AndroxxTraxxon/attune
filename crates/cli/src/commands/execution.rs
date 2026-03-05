use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum ExecutionCommands {
    /// List all executions
    List {
        /// Filter by pack name
        #[arg(long)]
        pack: Option<String>,

        /// Filter by action name
        #[arg(short, long)]
        action: Option<String>,

        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Search in execution result (case-insensitive)
        #[arg(short, long)]
        result: Option<String>,

        /// Limit number of results
        #[arg(short, long, default_value = "50")]
        limit: i32,
    },
    /// Show details of a specific execution
    Show {
        /// Execution ID
        execution_id: i64,
    },
    /// Show execution logs
    Logs {
        /// Execution ID
        execution_id: i64,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
    /// Cancel a running execution
    Cancel {
        /// Execution ID
        execution_id: i64,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Get raw execution result
    Result {
        /// Execution ID
        execution_id: i64,

        /// Output format (json or yaml, default: json)
        #[arg(short = 'f', long, value_enum, default_value = "json")]
        format: ResultFormat,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ResultFormat {
    Json,
    Yaml,
}

#[derive(Debug, Serialize, Deserialize)]
struct Execution {
    id: i64,
    action_ref: String,
    status: String,
    #[serde(default)]
    parent: Option<i64>,
    #[serde(default)]
    enforcement: Option<i64>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    created: String,
    #[serde(default)]
    updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExecutionDetail {
    id: i64,
    #[serde(default)]
    action: Option<i64>,
    action_ref: String,
    #[serde(default)]
    config: Option<serde_json::Value>,
    status: String,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    parent: Option<i64>,
    #[serde(default)]
    enforcement: Option<i64>,
    #[serde(default)]
    executor: Option<i64>,
    created: String,
    updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExecutionLogs {
    execution_id: i64,
    logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

pub async fn handle_execution_command(
    profile: &Option<String>,
    command: ExecutionCommands,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    match command {
        ExecutionCommands::List {
            pack,
            action,
            status,
            result,
            limit,
        } => {
            handle_list(
                profile,
                pack,
                action,
                status,
                result,
                limit,
                api_url,
                output_format,
            )
            .await
        }
        ExecutionCommands::Show { execution_id } => {
            handle_show(profile, execution_id, api_url, output_format).await
        }
        ExecutionCommands::Logs {
            execution_id,
            follow,
        } => handle_logs(profile, execution_id, follow, api_url, output_format).await,
        ExecutionCommands::Cancel { execution_id, yes } => {
            handle_cancel(profile, execution_id, yes, api_url, output_format).await
        }
        ExecutionCommands::Result {
            execution_id,
            format,
        } => handle_result(profile, execution_id, format, api_url).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_list(
    profile: &Option<String>,
    pack: Option<String>,
    action: Option<String>,
    status: Option<String>,
    result: Option<String>,
    limit: i32,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let mut query_params = vec![format!("per_page={}", limit)];
    if let Some(pack_name) = pack {
        query_params.push(format!("pack_name={}", pack_name));
    }
    if let Some(action_name) = action {
        query_params.push(format!("action_ref={}", action_name));
    }
    if let Some(status_filter) = status {
        query_params.push(format!("status={}", status_filter));
    }
    if let Some(result_search) = result {
        query_params.push(format!(
            "result_contains={}",
            urlencoding::encode(&result_search)
        ));
    }

    let path = format!("/executions?{}", query_params.join("&"));
    let executions: Vec<Execution> = client.get(&path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&executions, output_format)?;
        }
        OutputFormat::Table => {
            if executions.is_empty() {
                output::print_info("No executions found");
            } else {
                let mut table = output::create_table();
                output::add_header(
                    &mut table,
                    vec!["ID", "Action", "Status", "Started", "Duration"],
                );

                for execution in executions {
                    table.add_row(vec![
                        execution.id.to_string(),
                        execution.action_ref.clone(),
                        output::format_status(&execution.status),
                        output::format_timestamp(&execution.created),
                        "-".to_string(),
                    ]);
                }

                println!("{}", table);
            }
        }
    }

    Ok(())
}

async fn handle_show(
    profile: &Option<String>,
    execution_id: i64,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let path = format!("/executions/{}", execution_id);
    let execution: ExecutionDetail = client.get(&path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&execution, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section(&format!("Execution: {}", execution.id));

            output::print_key_value_table(vec![
                ("ID", execution.id.to_string()),
                ("Action", execution.action_ref.clone()),
                ("Status", output::format_status(&execution.status)),
                (
                    "Parent ID",
                    execution
                        .parent
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                ),
                (
                    "Enforcement ID",
                    execution
                        .enforcement
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                ),
                (
                    "Executor ID",
                    execution
                        .executor
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                ),
                ("Created", output::format_timestamp(&execution.created)),
                ("Updated", output::format_timestamp(&execution.updated)),
            ]);

            if let Some(config) = execution.config {
                if !config.is_null() {
                    output::print_section("Configuration");
                    println!("{}", serde_json::to_string_pretty(&config)?);
                }
            }

            if let Some(result) = execution.result {
                if !result.is_null() {
                    output::print_section("Result");
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
    }

    Ok(())
}

async fn handle_logs(
    profile: &Option<String>,
    execution_id: i64,
    follow: bool,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let path = format!("/executions/{}/logs", execution_id);

    if follow {
        // Polling implementation for following logs
        let mut last_count = 0;
        loop {
            let logs: ExecutionLogs = client.get(&path).await?;

            // Print new logs only
            for log in logs.logs.iter().skip(last_count) {
                match output_format {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string(log)?);
                    }
                    OutputFormat::Yaml => {
                        println!("{}", serde_yaml_ng::to_string(log)?);
                    }
                    OutputFormat::Table => {
                        println!(
                            "[{}] [{}] {}",
                            output::format_timestamp(&log.timestamp),
                            log.level.to_uppercase(),
                            log.message
                        );
                    }
                }
            }

            last_count = logs.logs.len();

            // Check if execution is complete
            let exec_path = format!("/executions/{}", execution_id);
            let execution: ExecutionDetail = client.get(&exec_path).await?;
            let status_lower = execution.status.to_lowercase();
            if status_lower == "succeeded" || status_lower == "failed" || status_lower == "canceled"
            {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    } else {
        let logs: ExecutionLogs = client.get(&path).await?;

        match output_format {
            OutputFormat::Json | OutputFormat::Yaml => {
                output::print_output(&logs, output_format)?;
            }
            OutputFormat::Table => {
                if logs.logs.is_empty() {
                    output::print_info("No logs available");
                } else {
                    for log in logs.logs {
                        println!(
                            "[{}] [{}] {}",
                            output::format_timestamp(&log.timestamp),
                            log.level.to_uppercase(),
                            log.message
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_result(
    profile: &Option<String>,
    execution_id: i64,
    format: ResultFormat,
    api_url: &Option<String>,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let path = format!("/executions/{}", execution_id);
    let execution: ExecutionDetail = client.get(&path).await?;

    // Check if execution has a result
    if let Some(result) = execution.result {
        // Output raw result in requested format
        match format {
            ResultFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            ResultFormat::Yaml => {
                println!("{}", serde_yaml_ng::to_string(&result)?);
            }
        }
    } else {
        anyhow::bail!("Execution {} has no result yet", execution_id);
    }

    Ok(())
}

async fn handle_cancel(
    profile: &Option<String>,
    execution_id: i64,
    yes: bool,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    // Confirm cancellation unless --yes is provided
    if !yes && matches!(output_format, OutputFormat::Table) {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to cancel execution {}?",
                execution_id
            ))
            .default(false)
            .interact()?;

        if !confirm {
            output::print_info("Cancellation aborted");
            return Ok(());
        }
    }

    let path = format!("/executions/{}/cancel", execution_id);
    let execution: ExecutionDetail = client.post(&path, &serde_json::json!({})).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&execution, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Execution {} cancelled", execution_id));
        }
    }

    Ok(())
}
