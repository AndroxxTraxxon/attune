use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum RuleCommands {
    /// List all rules
    List {
        /// Filter by pack name
        #[arg(long)]
        pack: Option<String>,

        /// Filter by enabled status
        #[arg(short, long)]
        enabled: Option<bool>,
    },
    /// Show details of a specific rule
    Show {
        /// Rule reference (pack.rule or ID)
        rule_ref: String,
    },
    /// Update a rule
    Update {
        /// Rule reference (pack.rule or ID)
        rule_ref: String,

        /// Update label
        #[arg(long)]
        label: Option<String>,

        /// Update description
        #[arg(long)]
        description: Option<String>,

        /// Update conditions as JSON string
        #[arg(long)]
        conditions: Option<String>,

        /// Update action parameters as JSON string
        #[arg(long)]
        action_params: Option<String>,

        /// Update trigger parameters as JSON string
        #[arg(long)]
        trigger_params: Option<String>,

        /// Update enabled status
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Enable a rule
    Enable {
        /// Rule reference (pack.rule or ID)
        rule_ref: String,
    },
    /// Disable a rule
    Disable {
        /// Rule reference (pack.rule or ID)
        rule_ref: String,
    },
    /// Create a new rule
    Create {
        /// Rule name
        #[arg(short, long)]
        name: String,

        /// Pack ID or name
        #[arg(short, long)]
        pack: String,

        /// Trigger reference
        #[arg(short, long)]
        trigger: String,

        /// Action reference
        #[arg(short, long)]
        action: String,

        /// Rule description
        #[arg(short, long)]
        description: Option<String>,

        /// Rule criteria as JSON string
        #[arg(long)]
        criteria: Option<String>,

        /// Enable the rule immediately
        #[arg(long)]
        enabled: bool,
    },
    /// Delete a rule
    Delete {
        /// Rule reference (pack.rule or ID)
        rule_ref: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Rule {
    id: i64,
    #[serde(rename = "ref")]
    rule_ref: String,
    #[serde(default)]
    pack: Option<i64>,
    pack_ref: String,
    label: String,
    description: String,
    #[serde(default)]
    trigger: Option<i64>,
    trigger_ref: String,
    #[serde(default)]
    action: Option<i64>,
    action_ref: String,
    enabled: bool,
    created: String,
    updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuleDetail {
    id: i64,
    #[serde(rename = "ref")]
    rule_ref: String,
    #[serde(default)]
    pack: Option<i64>,
    pack_ref: String,
    label: String,
    description: String,
    #[serde(default)]
    trigger: Option<i64>,
    trigger_ref: String,
    #[serde(default)]
    action: Option<i64>,
    action_ref: String,
    enabled: bool,
    #[serde(default)]
    conditions: Option<serde_json::Value>,
    #[serde(default)]
    action_params: Option<serde_json::Value>,
    #[serde(default)]
    trigger_params: Option<serde_json::Value>,
    created: String,
    updated: String,
}

#[derive(Debug, Serialize)]
struct CreateRuleRequest {
    name: String,
    pack_id: String,
    trigger_id: String,
    action_id: String,
    description: Option<String>,
    criteria: Option<serde_json::Value>,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct UpdateRuleRequest {
    enabled: bool,
}

pub async fn handle_rule_command(
    profile: &Option<String>,
    command: RuleCommands,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    match command {
        RuleCommands::List { pack, enabled } => {
            handle_list(profile, pack, enabled, api_url, output_format).await
        }
        RuleCommands::Show { rule_ref } => {
            handle_show(profile, rule_ref, api_url, output_format).await
        }
        RuleCommands::Update {
            rule_ref,
            label,
            description,
            conditions,
            action_params,
            trigger_params,
            enabled,
        } => {
            handle_update(
                profile,
                rule_ref,
                label,
                description,
                conditions,
                action_params,
                trigger_params,
                enabled,
                api_url,
                output_format,
            )
            .await
        }
        RuleCommands::Enable { rule_ref } => {
            handle_toggle(profile, rule_ref, true, api_url, output_format).await
        }
        RuleCommands::Disable { rule_ref } => {
            handle_toggle(profile, rule_ref, false, api_url, output_format).await
        }
        RuleCommands::Create {
            name,
            pack,
            trigger,
            action,
            description,
            criteria,
            enabled,
        } => {
            handle_create(
                profile,
                name,
                pack,
                trigger,
                action,
                description,
                criteria,
                enabled,
                api_url,
                output_format,
            )
            .await
        }
        RuleCommands::Delete { rule_ref, yes } => {
            handle_delete(profile, rule_ref, yes, api_url, output_format).await
        }
    }
}

async fn handle_list(
    profile: &Option<String>,
    pack: Option<String>,
    enabled: Option<bool>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let mut query_params = Vec::new();
    if let Some(pack_name) = pack {
        query_params.push(format!("pack={}", pack_name));
    }
    if let Some(is_enabled) = enabled {
        query_params.push(format!("enabled={}", is_enabled));
    }

    let path = if query_params.is_empty() {
        "/rules".to_string()
    } else {
        format!("/rules?{}", query_params.join("&"))
    };

    let rules: Vec<Rule> = client.get(&path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&rules, output_format)?;
        }
        OutputFormat::Table => {
            if rules.is_empty() {
                output::print_info("No rules found");
            } else {
                let mut table = output::create_table();
                output::add_header(
                    &mut table,
                    vec!["ID", "Pack", "Name", "Trigger", "Action", "Enabled"],
                );

                for rule in rules {
                    table.add_row(vec![
                        rule.id.to_string(),
                        rule.pack_ref.clone(),
                        rule.label.clone(),
                        rule.trigger_ref.clone(),
                        rule.action_ref.clone(),
                        output::format_bool(rule.enabled),
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
    rule_ref: String,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let path = format!("/rules/{}", rule_ref);
    let rule: RuleDetail = client.get(&path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&rule, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section(&format!("Rule: {}", rule.rule_ref));
            output::print_key_value_table(vec![
                ("ID", rule.id.to_string()),
                ("Ref", rule.rule_ref.clone()),
                ("Pack", rule.pack_ref.clone()),
                ("Label", rule.label.clone()),
                ("Description", rule.description.clone()),
                ("Trigger", rule.trigger_ref.clone()),
                ("Action", rule.action_ref.clone()),
                ("Enabled", output::format_bool(rule.enabled)),
                ("Created", output::format_timestamp(&rule.created)),
                ("Updated", output::format_timestamp(&rule.updated)),
            ]);

            if let Some(conditions) = rule.conditions {
                if !conditions.is_null() {
                    output::print_section("Conditions");
                    println!("{}", serde_json::to_string_pretty(&conditions)?);
                }
            }

            if let Some(action_params) = rule.action_params {
                if !action_params.is_null() {
                    output::print_section("Action Parameters");
                    println!("{}", serde_json::to_string_pretty(&action_params)?);
                }
            }

            if let Some(trigger_params) = rule.trigger_params {
                if !trigger_params.is_null() {
                    output::print_section("Trigger Parameters");
                    println!("{}", serde_json::to_string_pretty(&trigger_params)?);
                }
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_update(
    profile: &Option<String>,
    rule_ref: String,
    label: Option<String>,
    description: Option<String>,
    conditions: Option<String>,
    action_params: Option<String>,
    trigger_params: Option<String>,
    enabled: Option<bool>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    // Check that at least one field is provided
    if label.is_none()
        && description.is_none()
        && conditions.is_none()
        && action_params.is_none()
        && trigger_params.is_none()
        && enabled.is_none()
    {
        anyhow::bail!("At least one field must be provided to update");
    }

    // Parse JSON fields
    let conditions_json = if let Some(cond) = conditions {
        Some(serde_json::from_str(&cond)?)
    } else {
        None
    };

    let action_params_json = if let Some(params) = action_params {
        Some(serde_json::from_str(&params)?)
    } else {
        None
    };

    let trigger_params_json = if let Some(params) = trigger_params {
        Some(serde_json::from_str(&params)?)
    } else {
        None
    };

    #[derive(Serialize)]
    struct UpdateRuleRequestCli {
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        conditions: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        action_params: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger_params: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled: Option<bool>,
    }

    let request = UpdateRuleRequestCli {
        label,
        description,
        conditions: conditions_json,
        action_params: action_params_json,
        trigger_params: trigger_params_json,
        enabled,
    };

    let path = format!("/rules/{}", rule_ref);
    let rule: RuleDetail = client.put(&path, &request).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&rule, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Rule '{}' updated successfully", rule.rule_ref));
            output::print_key_value_table(vec![
                ("ID", rule.id.to_string()),
                ("Ref", rule.rule_ref.clone()),
                ("Pack", rule.pack_ref.clone()),
                ("Label", rule.label.clone()),
                ("Description", rule.description.clone()),
                ("Trigger", rule.trigger_ref.clone()),
                ("Action", rule.action_ref.clone()),
                ("Enabled", output::format_bool(rule.enabled)),
                ("Updated", output::format_timestamp(&rule.updated)),
            ]);
        }
    }

    Ok(())
}

async fn handle_toggle(
    profile: &Option<String>,
    rule_ref: String,
    enabled: bool,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let request = UpdateRuleRequest { enabled };
    let path = format!("/rules/{}", rule_ref);
    let rule: Rule = client.patch(&path, &request).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&rule, output_format)?;
        }
        OutputFormat::Table => {
            let action = if enabled { "enabled" } else { "disabled" };
            output::print_success(&format!("Rule '{}' {}", rule.rule_ref, action));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_create(
    profile: &Option<String>,
    name: String,
    pack: String,
    trigger: String,
    action: String,
    description: Option<String>,
    criteria: Option<String>,
    enabled: bool,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    let criteria_value = if let Some(criteria_str) = criteria {
        Some(serde_json::from_str(&criteria_str)?)
    } else {
        None
    };

    let request = CreateRuleRequest {
        name: name.clone(),
        pack_id: pack,
        trigger_id: trigger,
        action_id: action,
        description,
        criteria: criteria_value,
        enabled,
    };

    let rule: Rule = client.post("/rules", &request).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&rule, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Rule '{}' created successfully", rule.rule_ref));
            output::print_info(&format!("ID: {}", rule.id));
            output::print_info(&format!("Enabled: {}", rule.enabled));
        }
    }

    Ok(())
}

async fn handle_delete(
    profile: &Option<String>,
    rule_ref: String,
    yes: bool,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    // Confirm deletion unless --yes is provided
    if !yes && matches!(output_format, OutputFormat::Table) {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete rule '{}'?",
                rule_ref
            ))
            .default(false)
            .interact()?;

        if !confirm {
            output::print_info("Deletion cancelled");
            return Ok(());
        }
    }

    let path = format!("/rules/{}", rule_ref);
    client.delete_no_response(&path).await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let msg = serde_json::json!({"message": "Rule deleted successfully"});
            output::print_output(&msg, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Rule '{}' deleted successfully", rule_ref));
        }
    }

    Ok(())
}
