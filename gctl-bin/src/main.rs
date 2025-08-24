use crate::cli::{GctlCli, GctlCommands};
use crate::error::GctlCliError;
use crate::image_handler::ImageCommandHandler;
use crate::post_handler::PostCommandHandler;
use clap::Parser;
use console::style;
use gctl_ghost::{GhostAdminToken, GhostContentApi, GhostVersion};
use std::process;
use tracing::{debug, error};
use url::Url;

use crate::log::setup_logging;
use output::OutputFormat;
mod cli;
mod error;
mod image_handler;
mod log;
mod markdown;
mod output;
mod post_handler;

async fn run() -> Result<(), GctlCliError> {
    dotenvy::dotenv().ok();
    let cli = GctlCli::parse();
    setup_logging(cli.log_level());

    debug!("Loading configuration...");
    let url = Url::parse(cli.ghost_url.as_ref())?;
    let admin_token = GhostAdminToken::parse(cli.ghost_admin_key.as_ref())?;
    let api_version = GhostVersion::try_from(cli.ghost_api_version.as_ref())?;
    let output_format = OutputFormat::try_from(cli.output_format.as_ref())?;

    debug!("Connecting to Ghost at {}", style(&url).cyan());
    let api = GhostContentApi::new(url, admin_token, api_version)?;

    match cli.command {
        GctlCommands::Post { command } => {
            PostCommandHandler::builder()
                .api(&api)
                .format(output_format)
                .build()
                .handle_command(command)
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
