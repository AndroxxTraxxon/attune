use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::output::{self, OutputFormat};
use crate::wait::{wait_for_execution, WaitOptions};

#[derive(Subcommand)]
pub enum ActionCommands {
    /// List all actions
    List {
        /// Filter by pack name
        #[arg(long)]
        pack: Option<String>,

        /// Filter by action name
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Show details of a specific action
    Show {
        /// Action reference (pack.action or ID)
        action_ref: String,
    },
    /// Update an action
    Update {
        /// Action reference (pack.action or ID)
        action_ref: String,

        /// Update label
        #[arg(long)]
        label: Option<String>,

        /// Update description
        #[arg(long)]
        description: Option<String>,

        /// Update entrypoint
        #[arg(long)]
        entrypoint: Option<String>,

        /// Update runtime ID
        #[arg(long)]
        runtime: Option<i64>,
    },
    /// Delete an action
    Delete {
        /// Action reference (pack.action or ID)
        action_ref: String,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Execute an action
    Execute {
        /// Action reference (pack.action or ID)
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

#[derive(Debug, Serialize, Deserialize)]
struct Action {
    id: i64,
    #[serde(rename = "ref")]
    action_ref: String,
    pack_ref: String,
    label: String,
    description: Option<String>,
    entrypoint: String,
    runtime: Option<i64>,
    created: String,
    updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ActionDetail {
    id: i64,
    #[serde(rename = "ref")]
    action_ref: String,
    pack: i64,
    pack_ref: String,
    label: String,
    description: Option<String>,
    entrypoint: String,
    runtime: Option<i64>,
    param_schema: Option<serde_json::Value>,
    out_schema: Option<serde_json::Value>,
    created: String,
    updated: String,
}

#[derive(Debug, Serialize)]
struct UpdateActionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entrypoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ExecuteActionRequest {
    action_ref: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct Execution {
    id: i64,
    action: Option<i64>,
    action_ref: String,
    config: Option<serde_json::Value>,
    parent: Option<i64>,
    enforcement: Option<i64>,
    executor: Option<i64>,
    status: String,
    result: Option<serde_json::Value>,
    created: String,
    updated: String,
}

pub async fn handle_action_command(
    profile: &Option<String>,
    command: ActionCommands,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    match command {
        ActionCommands::List { pack, name } => {
            handle_list(pack, name, profile, api_url, output_format).await
        }
        ActionCommands::Show { action_ref } => {
            handle_show(action_ref, profile, api_url, output_format).await
        }
        ActionCommands::Update {
            action_ref,
            label,
            description,
            entrypoint,
            runtime,
        } => {
            handle_update(
                action_ref,
                label,
                description,
                entrypoint,
                runtime,
                profile,
                api_url,
                output_format,
            )
            .await
        }
        ActionCommands::Delete { action_ref, yes } => {
            handle_delete(action_ref, yes, profile, api_url, output_format).await
        }
        ActionCommands::Execute {
            action_ref,
            param,
            params_json,
            wait,
            timeout,
            notifier_url,
        } => {
            handle_execute(
                action_ref,
                param,
                params_json,
                profile,
                api_url,
                wait,
                timeout,
                notifier_url,
                output_format,
            )
            .await
        }
    }
}

async fn handle_list(
    pack: Option<String>,
    name: Option<String>,
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    // Use pack-specific endpoint if pack filter is specified
    let path = if let Some(pack_ref) = pack {
        format!("/packs/{}/actions", pack_ref)
    } else {
        "/actions".to_string()
    };

    let mut actions: Vec<Action> = client.get(&path).await?;

    // Filter by name if specified (client-side filtering)
    if let Some(action_name) = name {
        actions.retain(|a| a.action_ref.contains(&action_name));
    }

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&actions, output_format)?;
        }
        OutputFormat::Table => {
            if actions.is_empty() {
                output::print_info("No actions found");
            } else {
                let mut table = output::create_table();
                output::add_header(
                    &mut table,
                    vec!["ID", "Pack", "Name", "Runner", "Description"],
                );

                for action in actions {
                    table.add_row(vec![
                        action.id.to_string(),
                        action.pack_ref.clone(),
                        action.action_ref.clone(),
                        action
                            .runtime
                            .map(|r| r.to_string())
                            .unwrap_or_else(|| "none".to_string()),
                        output::truncate(&action.description.unwrap_or_default(), 40),
                    ]);
                }

                println!("{}", table);
            }
        }
    }

    Ok(())
}

async fn handle_show(
    action_ref: String,
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let path = format!("/actions/{}", action_ref);
    let action: ActionDetail = client.get(&path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&action, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section(&format!("Action: {}", action.action_ref));
            output::print_key_value_table(vec![
                ("ID", action.id.to_string()),
                ("Reference", action.action_ref.clone()),
                ("Pack", action.pack_ref.clone()),
                ("Label", action.label.clone()),
                (
                    "Description",
                    action.description.unwrap_or_else(|| "None".to_string()),
                ),
                ("Entry Point", action.entrypoint.clone()),
                (
                    "Runtime",
                    action
                        .runtime
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                ),
                ("Created", output::format_timestamp(&action.created)),
                ("Updated", output::format_timestamp(&action.updated)),
            ]);

            if let Some(params) = action.param_schema {
                if !params.is_null() {
                    output::print_section("Parameters Schema");
                    println!("{}", serde_json::to_string_pretty(&params)?);
                }
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_update(
    action_ref: String,
    label: Option<String>,
    description: Option<String>,
    entrypoint: Option<String>,
    runtime: Option<i64>,
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    // Check that at least one field is provided
    if label.is_none() && description.is_none() && entrypoint.is_none() && runtime.is_none() {
        anyhow::bail!("At least one field must be provided to update");
    }

    let request = UpdateActionRequest {
        label,
        description,
        entrypoint,
        runtime,
    };

    let path = format!("/actions/{}", action_ref);
    let action: ActionDetail = client.put(&path, &request).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&action, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!(
                "Action '{}' updated successfully",
                action.action_ref
            ));
            output::print_key_value_table(vec![
                ("ID", action.id.to_string()),
                ("Ref", action.action_ref.clone()),
                ("Pack", action.pack_ref.clone()),
                ("Label", action.label.clone()),
                (
                    "Description",
                    action.description.unwrap_or_else(|| "None".to_string()),
                ),
                ("Entrypoint", action.entrypoint.clone()),
                (
                    "Runtime",
                    action
                        .runtime
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "None".to_string()),
                ),
                ("Updated", output::format_timestamp(&action.updated)),
            ]);
        }
    }

    Ok(())
}

async fn handle_delete(
    action_ref: String,
    yes: bool,
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    // Confirm deletion unless --yes is provided
    if !yes && matches!(output_format, OutputFormat::Table) {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete action '{}'?",
                action_ref
            ))
            .default(false)
            .interact()?;

        if !confirm {
            output::print_info("Delete cancelled");
            return Ok(());
        }
    }

    let path = format!("/actions/{}", action_ref);
    client.delete_no_response(&path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let msg = serde_json::json!({"message": "Action deleted successfully"});
            output::print_output(&msg, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Action '{}' deleted successfully", action_ref));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_execute(
    action_ref: String,
    params: Vec<String>,
    params_json: Option<String>,
    profile: &Option<String>,
    api_url: &Option<String>,
    wait: bool,
    timeout: u64,
    notifier_url: Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    // Parse parameters
    let parameters = if let Some(json_str) = params_json {
        serde_json::from_str(&json_str)?
    } else if !params.is_empty() {
        let mut map = HashMap::new();
        for p in params {
            let parts: Vec<&str> = p.splitn(2, '=').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid parameter format: '{}'. Expected key=value", p);
            }
            // Try to parse as JSON value, fall back to string
            let value: serde_json::Value = serde_json::from_str(parts[1])
                .unwrap_or_else(|_| serde_json::Value::String(parts[1].to_string()));
            map.insert(parts[0].to_string(), value);
        }
        serde_json::to_value(map)?
    } else {
        serde_json::json!({})
    };

    let request = ExecuteActionRequest {
        action_ref: action_ref.clone(),
        parameters,
    };

    if output_format == OutputFormat::Table {
        output::print_info(&format!("Executing action: {}", action_ref));
    }

    let path = "/executions/execute".to_string();
    let execution: Execution = client.post(&path, &request).await?;

    if !wait {
        match output_format {
            OutputFormat::Json | OutputFormat::Yaml => {
                output::print_output(&execution, output_format)?;
            }
            OutputFormat::Table => {
                output::print_success(&format!("Execution {} started", execution.id));
                output::print_key_value_table(vec![
                    ("Execution ID", execution.id.to_string()),
                    ("Action", execution.action_ref.clone()),
                    ("Status", output::format_status(&execution.status)),
                ]);
            }
        }
        return Ok(());
    }

    if output_format == OutputFormat::Table {
        output::print_info(&format!(
            "Waiting for execution {} to complete...",
            execution.id
        ));
    }

    let verbose = matches!(output_format, OutputFormat::Table);
    let summary = wait_for_execution(WaitOptions {
        execution_id: execution.id,
        timeout_secs: timeout,
        api_client: &mut client,
        notifier_ws_url: notifier_url,
        verbose,
    })
    .await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&summary, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Execution {} completed", summary.id));
            output::print_section("Execution Details");
            output::print_key_value_table(vec![
                ("Execution ID", summary.id.to_string()),
                ("Action", summary.action_ref.clone()),
                ("Status", output::format_status(&summary.status)),
                ("Created", output::format_timestamp(&summary.created)),
                ("Updated", output::format_timestamp(&summary.updated)),
            ]);

            if let Some(result) = summary.result {
                if !result.is_null() {
                    output::print_section("Result");
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
    }

    Ok(())
}
