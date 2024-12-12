//! # gctl
//!
//! A command-line interface for managing Ghost blog content.
//!
//! ## Environment Variables
//!
//! - `GHOST_URL`: Your Ghost blog URL
//! - `GHOST_ADMIN_KEY`: Your Ghost Admin API key
//!
//! ## Usage
//!
//! List posts:
//! ```bash
//! gctl post list
//! ```
//!
//! Create a post:
//! ```bash
//! gctl post create "Title" "Content"
//! ```
//!
//! Update a post:
//! ```bash
//! gctl post update <id> --title "New Title" --content "New Content"
//! ```
//!
//! Delete a post:
//! ```bash
//! gctl post delete <id>
//! ```

use clap::{Parser, Subcommand};
use console::style;
use gctl_ghost::{GhostContentApi, GhostError};
use std::process;
use thiserror::Error;
use tracing::{debug, error, info, trace, Level};
use tracing_subscriber::{self, EnvFilter};
use url::Url;

mod output;
use output::{format_post, format_posts, OutputFormat};

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Ghost API error: {0}")]
    GhostError(#[from] GhostError),
    #[error("Configuration error: {0}")]
    ConfigError(String),
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
        #[arg(long, help = "Post title")]
        title: String,
        #[arg(long, help = "Post content", required = false)]
        content: Option<String>,
        #[arg(long, help = "JSON data for the post")]
        json: Option<String>,
    },
    /// Update an existing post
    Update {
        #[arg(help = "Post ID")]
        id: String,
        #[arg(long, help = "New title")]
        title: Option<String>,
        #[arg(long, help = "New content")]
        content: Option<String>,
        #[arg(long, help = "JSON data for the post")]
        json: Option<String>,
    },
    /// Delete a post
    Delete {
        #[arg(help = "Post ID")]
        id: String,
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

fn run() -> Result<(), CliError> {
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
        Commands::Post { command } => match command {
            PostCommands::List => {
                info!("Fetching all posts...");
                let posts = api.posts().browse()?;
                debug!("Found {} posts", posts.len());
                format_posts(posts, format);
            }
            PostCommands::Get { id } => {
                info!("Fetching post {}...", style(&id).yellow());
                let post = api.posts().read(&id)?;
                format_post(post, format);
            }
            PostCommands::Add { title, content, json } => {
                info!("Creating new post '{}'...", style(&title).green());
                
                let post = if let Some(json_str) = json {
                    debug!("Using JSON input");
                    trace!("JSON data: {}", json_str);
                    let mut json_data: serde_json::Value = serde_json::from_str(&json_str)
                        .map_err(|e| CliError::ConfigError(format!("Invalid JSON: {}", e)))?;
                    
                    if let Some(obj) = json_data.as_object_mut() {
                        obj.retain(|_, v| !v.is_null());
                        
                        obj.insert("title".to_string(), serde_json::Value::String(title.clone()));
                        obj.insert("status".to_string(), serde_json::Value::String("draft".to_string()));
                    }
                    
                    debug!("Merged JSON data: {}", serde_json::to_string_pretty(&json_data).unwrap());
                    api.posts().add(&title, None)?
                } else {
                    api.posts().add(&title, content.as_deref())?
                };
                
                debug!("Post created with ID: {}", style(&post.id).yellow());
                format_post(post, format);
            }
            PostCommands::Update { id, title, content, json } => {
                info!("Updating post {}...", style(&id).yellow());
                
                let post = if let Some(json_str) = json {
                    debug!("Using JSON input");
                    trace!("JSON data: {}", json_str);
                    let mut json_data: serde_json::Value = serde_json::from_str(&json_str)
                        .map_err(|e| CliError::ConfigError(format!("Invalid JSON: {}", e)))?;
                    
                    if let Some(obj) = json_data.as_object_mut() {
                        obj.retain(|_, v| !v.is_null());
                        
                        if let Some(title) = title {
                            obj.insert("title".to_string(), serde_json::Value::String(title));
                        }
                        if let Some(content) = content {
                            obj.insert("markdown".to_string(), serde_json::Value::String(content));
                        }
                    }
                    
                    debug!("Update data: {}", serde_json::to_string_pretty(&json_data).unwrap());
                    api.posts().edit(&id, None, None, Some(json_data))?
                } else {
                    api.posts().edit(&id, title.as_deref(), content.as_deref(), None)?
                };
                
                format_post(post, format);
            }
            PostCommands::Delete { id } => {
                info!("Deleting post {}...", style(&id).red());
                api.posts().delete(&id)?;
                info!("Post deleted successfully");
            }
        },
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        error!("{}", style(err).red());
        process::exit(1);
    }
}
