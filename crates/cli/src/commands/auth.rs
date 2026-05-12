use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Log in to Attune API
    Login {
        /// Username or email
        #[arg(short, long)]
        username: String,

        /// Password (will prompt if not provided)
        #[arg(long)]
        password: Option<String>,

        /// API URL to log in to (saved into the profile for future use)
        #[arg(long)]
        url: Option<String>,

        /// Save credentials into a named profile (creates it if it doesn't exist)
        #[arg(long)]
        save_profile: Option<String>,
    },
    /// Log in with a revokable integration token
    TokenLogin {
        /// Integration token (will prompt if not provided)
        #[arg(long)]
        token: Option<String>,

        /// API URL to log in to (saved into the profile for future use)
        #[arg(long)]
        url: Option<String>,

        /// Save credentials into a named profile (creates it if it doesn't exist)
        #[arg(long)]
        save_profile: Option<String>,
    },
    /// Manage revokable integration tokens
    Token {
        #[command(subcommand)]
        command: IntegrationTokenCommands,
    },
    /// Log out and clear authentication tokens
    Logout,
    /// Show current authentication status
    Whoami,
    /// Refresh authentication token
    Refresh,
}

#[derive(Subcommand)]
pub enum IntegrationTokenCommands {
    /// Create an integration token for an identity
    Create {
        /// Identity ID that the token authenticates as
        #[arg(long)]
        identity_id: i64,

        /// Human-readable label for the token
        #[arg(long)]
        label: String,

        /// Optional token description
        #[arg(long)]
        description: Option<String>,

        /// Optional RFC3339 expiration timestamp
        #[arg(long)]
        expires_at: Option<String>,
    },
    /// List integration tokens for an identity
    List {
        /// Identity ID
        #[arg(long)]
        identity_id: i64,
    },
    /// Revoke an integration token
    Revoke {
        /// Identity ID that owns the token
        #[arg(long)]
        identity_id: i64,

        /// Integration token ID
        token_id: i64,

        /// Optional revocation reason
        #[arg(long)]
        reason: Option<String>,
    },
    /// Delete an integration token metadata record
    Delete {
        /// Identity ID that owns the token
        #[arg(long)]
        identity_id: i64,

        /// Integration token ID
        token_id: i64,

        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginRequest {
    login: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenLoginRequest {
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Identity {
    id: i64,
    login: String,
    display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateIntegrationTokenRequest {
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RevokeIntegrationTokenRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct IntegrationToken {
    id: i64,
    identity_id: i64,
    label: String,
    description: Option<String>,
    token_prefix: String,
    token_suffix: String,
    expires_at: Option<String>,
    last_used_at: Option<String>,
    revoked_at: Option<String>,
    active: bool,
    created: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateIntegrationTokenResponse {
    token: String,
    integration_token: IntegrationToken,
}

#[derive(Debug, Serialize, Deserialize)]
struct SuccessResponse {
    message: String,
}

pub async fn handle_auth_command(
    profile: &Option<String>,
    command: AuthCommands,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    match command {
        AuthCommands::Login {
            username,
            password,
            url,
            save_profile,
        } => {
            // --url is a convenient alias for --api-url at login time
            let effective_api_url = url.or_else(|| api_url.clone());
            handle_login(
                username,
                password,
                save_profile.as_ref().or(profile.as_ref()),
                &effective_api_url,
                output_format,
            )
            .await
        }
        AuthCommands::TokenLogin {
            token,
            url,
            save_profile,
        } => {
            let effective_api_url = url.or_else(|| api_url.clone());
            handle_token_login(
                token,
                save_profile.as_ref().or(profile.as_ref()),
                &effective_api_url,
                output_format,
            )
            .await
        }
        AuthCommands::Token { command } => {
            handle_integration_token_command(profile, command, api_url, output_format).await
        }
        AuthCommands::Logout => handle_logout(profile, output_format).await,
        AuthCommands::Whoami => handle_whoami(profile, api_url, output_format).await,
        AuthCommands::Refresh => handle_refresh(profile, api_url, output_format).await,
    }
}

async fn handle_login(
    username: String,
    password: Option<String>,
    profile: Option<&String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    // Determine which profile name will own these credentials.
    // If --save-profile / --profile was given, use that; otherwise use the
    // currently-active profile.
    let mut config = CliConfig::load()?;
    let target_profile_name = profile
        .cloned()
        .unwrap_or_else(|| config.current_profile.clone());

    // If a URL was provided and the target profile doesn't exist yet, create it.
    if !config.profiles.contains_key(&target_profile_name) {
        let url = api_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8080".to_string());
        use crate::config::Profile;
        config.set_profile(
            target_profile_name.clone(),
            Profile {
                api_url: url,
                auth_token: None,
                refresh_token: None,
                output_format: None,
                description: None,
            },
        )?;
    } else if let Some(url) = api_url {
        // Profile exists — update its api_url if an explicit URL was provided.
        if let Some(p) = config.profiles.get_mut(&target_profile_name) {
            p.api_url = url.clone();
        }
        config.save()?;
    }

    // Build a temporary config view that points at the target profile so
    // ApiClient uses the right base URL.
    let mut login_config = CliConfig::load()?;
    login_config.current_profile = target_profile_name.clone();

    // Prompt for password if not provided
    let password = match password {
        Some(p) => p,
        None => {
            let pw = dialoguer::Password::new()
                .with_prompt("Password")
                .interact()?;
            pw
        }
    };

    let mut client = ApiClient::from_config(&login_config, api_url);

    let login_req = LoginRequest {
        login: username,
        password,
    };

    let response: LoginResponse = client.post("/auth/login", &login_req).await?;

    // Persist tokens into the target profile.
    let mut config = CliConfig::load()?;
    // Ensure the profile exists (it may have just been created above and saved).
    if let Some(p) = config.profiles.get_mut(&target_profile_name) {
        p.auth_token = Some(response.access_token.clone());
        p.refresh_token = Some(response.refresh_token.clone());
        config.save()?;
    } else {
        // Fallback: set_auth writes to the current profile.
        config.set_auth(
            response.access_token.clone(),
            response.refresh_token.clone(),
        )?;
    }

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&response, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success("Successfully logged in");
            output::print_info(&format!("Token expires in {} seconds", response.expires_in));
            if target_profile_name != config.current_profile {
                output::print_info(&format!(
                    "Credentials saved to profile '{}'",
                    target_profile_name
                ));
            }
        }
    }

    Ok(())
}

async fn handle_token_login(
    token: Option<String>,
    profile: Option<&String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let mut config = CliConfig::load()?;
    let target_profile_name = profile
        .cloned()
        .unwrap_or_else(|| config.current_profile.clone());

    if !config.profiles.contains_key(&target_profile_name) {
        let url = api_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8080".to_string());
        use crate::config::Profile;
        config.set_profile(
            target_profile_name.clone(),
            Profile {
                api_url: url,
                auth_token: None,
                refresh_token: None,
                output_format: None,
                description: None,
            },
        )?;
    } else if let Some(url) = api_url {
        if let Some(p) = config.profiles.get_mut(&target_profile_name) {
            p.api_url = url.clone();
        }
        config.save()?;
    }

    let mut login_config = CliConfig::load()?;
    login_config.current_profile = target_profile_name.clone();

    let token = match token {
        Some(token) => token,
        None => dialoguer::Password::new()
            .with_prompt("Integration token")
            .interact()?,
    };

    let mut client = ApiClient::from_config(&login_config, api_url);
    let response: LoginResponse = client
        .post("/auth/token-login", &TokenLoginRequest { token })
        .await?;

    let mut config = CliConfig::load()?;
    if let Some(p) = config.profiles.get_mut(&target_profile_name) {
        p.auth_token = Some(response.access_token.clone());
        p.refresh_token = Some(response.refresh_token.clone());
        config.save()?;
    } else {
        config.set_auth(
            response.access_token.clone(),
            response.refresh_token.clone(),
        )?;
    }

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&response, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success("Successfully logged in with integration token");
            output::print_info(&format!("Token expires in {} seconds", response.expires_in));
            if target_profile_name != config.current_profile {
                output::print_info(&format!(
                    "Credentials saved to profile '{}'",
                    target_profile_name
                ));
            }
        }
    }

    Ok(())
}

async fn handle_logout(profile: &Option<String>, output_format: OutputFormat) -> Result<()> {
    let mut config = CliConfig::load_with_profile(profile.as_deref())?;
    config.clear_auth()?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let msg = serde_json::json!({"message": "Successfully logged out"});
            output::print_output(&msg, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success("Successfully logged out");
        }
    }

    Ok(())
}

async fn handle_integration_token_command(
    profile: &Option<String>,
    command: IntegrationTokenCommands,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;
    let mut client = ApiClient::from_config(&config, api_url);

    match command {
        IntegrationTokenCommands::Create {
            identity_id,
            label,
            description,
            expires_at,
        } => {
            let response: CreateIntegrationTokenResponse = client
                .post(
                    &format!("/identities/{identity_id}/integration-tokens"),
                    &CreateIntegrationTokenRequest {
                        label,
                        description,
                        expires_at,
                    },
                )
                .await?;

            match output_format {
                OutputFormat::Json | OutputFormat::Yaml => {
                    output::print_output(&response, output_format)?;
                }
                OutputFormat::Table => {
                    output::print_success("Integration token created");
                    output::print_warning(
                        "Copy this token now. It will not be shown again after this response.",
                    );
                    output::print_key_value_table(vec![
                        ("Token", response.token),
                        ("ID", response.integration_token.id.to_string()),
                        (
                            "Identity ID",
                            response.integration_token.identity_id.to_string(),
                        ),
                        ("Label", response.integration_token.label),
                        (
                            "Expires",
                            response
                                .integration_token
                                .expires_at
                                .unwrap_or_else(|| "never".to_string()),
                        ),
                    ]);
                }
            }
        }
        IntegrationTokenCommands::List { identity_id } => {
            let tokens: Vec<IntegrationToken> = client
                .get(&format!("/identities/{identity_id}/integration-tokens"))
                .await?;

            match output_format {
                OutputFormat::Json | OutputFormat::Yaml => {
                    output::print_output(&tokens, output_format)?;
                }
                OutputFormat::Table => {
                    let mut table = output::create_table();
                    output::add_header(
                        &mut table,
                        vec!["ID", "Label", "Token", "Active", "Expires", "Last Used"],
                    );
                    for token in tokens {
                        table.add_row(vec![
                            token.id.to_string(),
                            token.label,
                            format!("{}...{}", token.token_prefix, token.token_suffix),
                            token.active.to_string(),
                            token.expires_at.unwrap_or_else(|| "never".to_string()),
                            token.last_used_at.unwrap_or_else(|| "-".to_string()),
                        ]);
                    }
                    println!("{}", table);
                }
            }
        }
        IntegrationTokenCommands::Revoke {
            identity_id,
            token_id,
            reason,
        } => {
            let token: IntegrationToken = client
                .post(
                    &format!("/identities/{identity_id}/integration-tokens/{token_id}/revoke"),
                    &RevokeIntegrationTokenRequest { reason },
                )
                .await?;

            match output_format {
                OutputFormat::Json | OutputFormat::Yaml => {
                    output::print_output(&token, output_format)?;
                }
                OutputFormat::Table => {
                    output::print_success("Integration token revoked");
                    output::print_key_value_table(vec![
                        ("ID", token.id.to_string()),
                        ("Label", token.label),
                        (
                            "Revoked At",
                            token.revoked_at.unwrap_or_else(|| "-".to_string()),
                        ),
                    ]);
                }
            }
        }
        IntegrationTokenCommands::Delete {
            identity_id,
            token_id,
            yes,
        } => {
            if !yes
                && !dialoguer::Confirm::new()
                    .with_prompt(format!(
                        "Delete integration token metadata record {token_id}?"
                    ))
                    .default(false)
                    .interact()?
            {
                output::print_info("Delete cancelled");
                return Ok(());
            }

            let response: SuccessResponse = client
                .delete(&format!(
                    "/identities/{identity_id}/integration-tokens/{token_id}"
                ))
                .await?;

            match output_format {
                OutputFormat::Json | OutputFormat::Yaml => {
                    output::print_output(&response, output_format)?;
                }
                OutputFormat::Table => output::print_success(&response.message),
            }
        }
    }

    Ok(())
}

async fn handle_whoami(
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;

    if config.auth_token().ok().flatten().is_none() {
        anyhow::bail!("Not logged in. Use 'attune auth login' to authenticate.");
    }

    let mut client = ApiClient::from_config(&config, api_url);

    let identity: Identity = client.get("/auth/me").await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&identity, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section("Current Identity");
            output::print_key_value_table(vec![
                ("ID", identity.id.to_string()),
                ("Login", identity.login),
                (
                    "Display Name",
                    identity.display_name.unwrap_or_else(|| "-".to_string()),
                ),
            ]);
        }
    }

    Ok(())
}

async fn handle_refresh(
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;

    // Check if we have a refresh token
    let refresh_token = config
        .refresh_token()
        .ok()
        .flatten()
        .ok_or_else(|| anyhow::anyhow!("No refresh token found. Please log in again."))?;

    let mut client = ApiClient::from_config(&config, api_url);

    #[derive(Serialize)]
    struct RefreshRequest {
        refresh_token: String,
    }

    // Call the refresh endpoint
    let response: LoginResponse = client
        .post("/auth/refresh", &RefreshRequest { refresh_token })
        .await?;

    // Save new tokens to config
    let mut config = CliConfig::load()?;
    config.set_auth(
        response.access_token.clone(),
        response.refresh_token.clone(),
    )?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&response, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success("Token refreshed successfully");
            output::print_info(&format!(
                "New token expires in {} seconds",
                response.expires_in
            ));
        }
    }

    Ok(())
}
