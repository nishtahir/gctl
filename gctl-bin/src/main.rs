use clap::{Parser, Subcommand};
use console::style;
use gctl_ghost::{GhostContentApi, GhostError};
use gctl_liquid::LiquidParser;
use std::fs;
use std::path::Path;
use std::process;
use thiserror::Error;
use tracing::{debug, error, info, trace, Level};
use tracing_subscriber::{self, EnvFilter};
use url::Url;

mod output;
use gctl_liquid::GhostLiquidError;
use output::{format_images, format_post, format_posts, OutputFormat};

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Ghost API error: {0}")]
    GhostError(#[from] GhostError),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Liquid template error: {0}")]
    LiquidError(#[from] GhostLiquidError),
}

#[derive(Parser)]
#[command(name = "gctl")]
#[command(about = "Ghost CLI tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Ghost blog URL
    #[arg(long, env = "GHOST_URL", global = true, required = false)]
    url: Option<String>,

    /// Ghost Admin API key
    #[arg(long, env = "GHOST_ADMIN_KEY", global = true, required = false)]
    key: Option<String>,

    /// Output format (table or json)
    #[arg(long, default_value = "table", global = true)]
    format: String,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable very verbose logging (trace level)
    #[arg(short = 'V', long, global = true)]
    very_verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Post management commands
    Post {
        #[command(subcommand)]
        command: PostCommands,
    },
    /// Image management commands
    Image {
        #[command(subcommand)]
        command: ImageCommands,
    },
}

#[derive(Subcommand)]
enum PostCommands {
    /// List all posts
    List,
    /// Get a specific post
    Get {
        #[arg(help = "Post ID")]
        id: String,
    },
    /// Add a new post
    Add {
        #[arg(long)]
        title: String,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        json: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        file: Option<String>,
    },
    /// Update an existing post
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        json: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        file: Option<String>,
    },
    /// Delete a post
    Delete {
        #[arg(help = "Post ID")]
        id: String,
    },
}

#[derive(Subcommand)]
enum ImageCommands {
    Upload {
        #[arg(help = "Image file")]
        file: String,

        #[arg(help = "Image Purpose", long)]
        purpose: Option<String>,

        #[arg(help = "Image ref", long)]
        ref_: Option<String>,
    },
}

fn setup_logging(verbose: bool, very_verbose: bool) {
    let filter = if very_verbose {
        EnvFilter::from_default_env().add_directive(Level::TRACE.into())
    } else if verbose {
        EnvFilter::from_default_env().add_directive(Level::DEBUG.into())
    } else {
        EnvFilter::from_default_env().add_directive(Level::INFO.into())
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .init();
}

struct PostCommandHandler<'a> {
    api: &'a GhostContentApi,
    format: OutputFormat,
}

impl<'a> PostCommandHandler<'a> {
    fn new(api: &'a GhostContentApi, format: OutputFormat) -> Self {
        Self { api, format }
    }

    async fn handle_file_or_json(
        &self,
        file: Option<&str>,
        json: Option<&str>,
    ) -> Result<Option<serde_json::Value>, CliError> {
        match (file, json) {
            (Some(file_path), _) => {
                debug!("Using template file");
                let file_content = fs::read_to_string(file_path)
                    .map_err(|e| CliError::ConfigError(format!("Failed to read file: {}", e)))?;
                trace!("File content: {}", file_content);
                Ok(Some(LiquidParser::parse_template(&file_content)?))
            }
            (None, Some(json_str)) => {
                debug!("Using JSON input");
                trace!("JSON data: {}", json_str);
                Ok(Some(serde_json::from_str(json_str).map_err(|e| {
                    CliError::ConfigError(format!("Invalid JSON: {}", e))
                })?))
            }
            (None, None) => Ok(None),
        }
    }

    async fn handle_command(&self, command: PostCommands) -> Result<(), CliError> {
        match command {
            PostCommands::List => {
                info!("Fetching all posts...");
                let posts = self.api.posts().browse().await?;
                debug!("Found {} posts", posts.len());
                format_posts(posts, self.format);
            }
            PostCommands::Get { id } => {
                info!("Fetching post {}...", style(&id).yellow());
                let post = self.api.posts().read(&id).await?;
                format_post(post, self.format);
            }
            PostCommands::Add {
                title,
                content,
                json,
                status,
                file,
            } => {
                info!("Creating new post '{}'...", style(&title).green());
                let json_data = self.handle_file_or_json(file.as_deref(), json.as_deref()).await?;
                let post = self.api
                    .posts()
                    .add(&title, content.as_deref(), json_data, status.as_deref())
                    .await?;
                debug!("Post created with ID: {}", style(&post.id).yellow());
                format_post(post, self.format);
            }
            PostCommands::Update {
                id,
                title,
                content,
                json,
                status,
                file,
            } => {
                info!("Updating post {}...", style(&id).yellow());
                let json_data = self.handle_file_or_json(file.as_deref(), json.as_deref()).await?;
                let post = self.api
                    .posts()
                    .edit(&id, title.as_deref(), content.as_deref(), json_data, status.as_deref())
                    .await?;
                format_post(post, self.format);
            }
            PostCommands::Delete { id } => {
                info!("Deleting post {}...", style(&id).red());
                self.api.posts().delete(&id).await?;
                info!("Post deleted successfully");
            }
        }
        Ok(())
    }
}

async fn run() -> Result<(), CliError> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    setup_logging(cli.verbose, cli.very_verbose);

    debug!("Loading configuration...");
    let url = cli.url.ok_or_else(|| CliError::ConfigError("GHOST_URL is required".to_string()))?;
    let key = cli.key.ok_or_else(|| CliError::ConfigError("GHOST_ADMIN_KEY is required".to_string()))?;

    let url = Url::parse(&url)?;
    debug!("Connecting to Ghost at {}", style(&url).cyan());
    let api = GhostContentApi::new(url, key);

    let format = match cli.format.as_str() {
        "json" => OutputFormat::Json,
        _ => OutputFormat::Table,
    };

    match cli.command {
        Commands::Post { command } => {
            PostCommandHandler::new(&api, format).handle_command(command).await?;
        }
        Commands::Image { command } => match command {
            ImageCommands::Upload { file, purpose, ref_ } => {
                info!("Uploading image {}...", style(&file).green());
                let file = Path::new(&file);
                let images = api.images().upload(file, purpose, ref_).await?;
                format_images(images, format);
            }
        },
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
