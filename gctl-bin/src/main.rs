use crate::auth_handler::AuthCommandHandler;
use crate::cli::{GctlCli, GctlCommands};
use crate::config::StoredConfig;
use crate::error::GctlCliError;
use crate::image_handler::ImageCommandHandler;
use crate::post_handler::PostCommandHandler;
use crate::publish_handler::PublishCommandHandler;
use crate::pull_handler::PullCommandHandler;
use clap::Parser;
use console::style;
use gctl_ghost::{GhostAdminToken, GhostContentApi, GhostVersion};
use std::process;
use tracing::{debug, error};
use url::Url;

use crate::log::setup_logging;
use output::OutputFormat;
mod auth_handler;
mod cli;
mod config;
mod error;
mod image_handler;
mod log;
mod markdown;
mod output;
mod post_handler;
mod publish_handler;
mod pull_handler;

fn missing_login() -> GctlCliError {
    GctlCliError::ConfigError(
        "No Ghost URL or Admin API key configured. Run `gctl auth login`.".into(),
    )
}

async fn run() -> Result<(), GctlCliError> {
    dotenvy::dotenv().ok();
    let cli = GctlCli::parse();
    setup_logging(cli.log_level());

    if let GctlCommands::Auth { command } = cli.command {
        return AuthCommandHandler.handle_command(command);
    }

    debug!("Loading configuration...");
    // Flags and environment variables take precedence over stored credentials.
    let stored = StoredConfig::load()?;
    let ghost_url = cli
        .ghost_url
        .or(stored.ghost_url)
        .ok_or_else(missing_login)?;
    let ghost_admin_key = cli
        .ghost_admin_key
        .or(stored.ghost_admin_key)
        .ok_or_else(missing_login)?;
    let url = Url::parse(&ghost_url)?;
    let admin_token = GhostAdminToken::parse(&ghost_admin_key)?;
    let api_version = GhostVersion::try_from(cli.ghost_api_version.as_ref())?;
    let output_format = OutputFormat::try_from(cli.output_format.as_ref())?;

    debug!("Connecting to Ghost at {}", style(&url).cyan());
    let api = GhostContentApi::new(url, admin_token, api_version)?;

    match cli.command {
        GctlCommands::Auth { .. } => unreachable!("handled above"),
        GctlCommands::Post { command } => {
            PostCommandHandler::builder()
                .api(&api)
                .format(output_format)
                .build()
                .handle_command(command)
                .await?;
        }
        GctlCommands::Publish(args) => {
            PublishCommandHandler::builder()
                .api(&api)
                .format(output_format)
                .build()
                .handle_command(args)
                .await?;
        }
        GctlCommands::Pull(args) => {
            PullCommandHandler::builder()
                .api(&api)
                .format(output_format)
                .build()
                .handle_command(args)
                .await?;
        }
        GctlCommands::Image { command } => {
            ImageCommandHandler::builder()
                .api(&api)
                .format(output_format)
                .build()
                .handle_command(command)
                .await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(err) = run().await {
        error!("{}", style(err).red());
        process::exit(1);
    }
    Ok(())
}
