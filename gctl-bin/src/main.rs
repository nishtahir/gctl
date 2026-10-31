use crate::auth_handler::AuthCommandHandler;
use crate::cli::{GctlCli, GctlCommands};
use crate::config::StoredConfig;
use crate::diff_handler::DiffCommandHandler;
use crate::error::GctlCliError;
use crate::post_handler::PostCommandHandler;
use crate::pull_handler::PullCommandHandler;
use crate::push_handler::PushCommandHandler;
use crate::status_handler::StatusCommandHandler;
use clap::Parser;
use console::style;
use gctl_ghost::{GhostAdminToken, GhostContentApi, GhostVersion};
use std::process;
use tracing::{debug, error};
use url::Url;

use crate::log::setup_logging;
mod auth_handler;
mod cli;
mod config;
mod diff_handler;
mod error;
mod images;
mod lint_handler;
mod log;
mod markdown;
mod new_handler;
mod output;
mod post_handler;
mod pull_handler;
mod push_handler;
mod state;
mod status_handler;

fn missing_login() -> GctlCliError {
    GctlCliError::Config("No Ghost URL or Admin API key configured. Run `gctl auth login`.".into())
}

async fn run() -> Result<(), GctlCliError> {
    dotenvy::dotenv().ok();
    let cli = GctlCli::parse();
    setup_logging(cli.log_level());

    if let GctlCommands::Auth { command } = cli.command {
        return AuthCommandHandler.handle_command(command);
    }

    if let GctlCommands::New(args) = cli.command {
        return new_handler::handle_command(args);
    }
    if let GctlCommands::Lint(args) = cli.command {
        return lint_handler::handle_command(args);
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

    debug!("Connecting to Ghost at {}", style(&url).cyan());
    let api = GhostContentApi::new(url, admin_token, api_version)?;

    match cli.command {
        GctlCommands::Auth { .. } | GctlCommands::New(_) | GctlCommands::Lint(_) => {
            unreachable!("handled above")
        }
        GctlCommands::Post { command } => {
            PostCommandHandler::builder()
                .api(&api)
                .build()
                .handle_command(command)
                .await?;
        }
        GctlCommands::Push(args) => {
            PushCommandHandler::builder()
                .api(&api)
                .build()
                .handle_command(args)
                .await?;
        }
        GctlCommands::Diff(args) => {
            DiffCommandHandler::builder()
                .api(&api)
                .build()
                .handle_command(args)
                .await?;
        }
        GctlCommands::Status(args) => {
            StatusCommandHandler::builder()
                .api(&api)
                .build()
                .handle_command(args)
                .await?;
        }
        GctlCommands::Pull(args) => {
            PullCommandHandler::builder()
                .api(&api)
                .build()
                .handle_command(args)
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
