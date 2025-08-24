use std::path::Path;

use bon::Builder;
use console::style;
use gctl_ghost::GhostContentApi;
use tracing::info;

use crate::cli::ImageCommands;
use crate::error::GctlCliError;
use crate::output::{format_images, OutputFormat};

#[derive(Builder)]
pub struct ImageCommandHandler<'a> {
    api: &'a GhostContentApi<'a>,
    format: OutputFormat,
}

impl<'a> ImageCommandHandler<'a> {
    pub async fn handle_command(&self, command: ImageCommands) -> Result<(), GctlCliError> {
        match command {
            ImageCommands::Upload {
                file,
                purpose,
                ref_,
            } => {
                info!("Uploading image {}...", style(&file).green());
                let file = Path::new(&file);
                let images = self.api.images().upload(file, purpose, ref_).await?;
                format_images(images, self.format);
                Ok(())
            }
        }
    }
}
