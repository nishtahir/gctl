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
use gctl_ghost::{GhostContentApi, GhostError};
use std::process;
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Ghost API error: {0}")]
    GhostError(#[from] GhostError),
}

#[derive(Parser)]
#[command(name = "gctl")]
#[command(about = "Ghost CLI tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, env = "GHOST_URL")]
    url: String,

    #[arg(long, env = "GHOST_ADMIN_KEY")]
    key: String,
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
    /// Create a new post
    Create {
        #[arg(help = "Post title")]
        title: String,
        #[arg(help = "Post content")]
        content: String,
    },
    /// Update an existing post
    Update {
        #[arg(help = "Post ID")]
        id: String,
        #[arg(help = "New title")]
        title: Option<String>,
        #[arg(help = "New content")]
        content: Option<String>,
    },
    /// Delete a post
    Delete {
        #[arg(help = "Post ID")]
        id: String,
    },
}

fn run() -> Result<(), CliError> {
    dotenvy::dotenv().ok();
    
    let cli = Cli::parse();
    
    let url = Url::parse(&cli.url)?;
    let api = GhostContentApi::new(url, cli.key);

    match cli.command {
        Commands::Post { command } => match command {
            PostCommands::List => {
                let posts = api.posts().browse()?;
                for post in posts {
                    println!("ID: {}", post.id);
                    println!("Title: {}", post.title);
                    println!("Status: {}", post.status);
                    println!("URL: {}", post.url);
                    println!("---");
                }
            }
            PostCommands::Get { id } => {
                let post = api.posts().read(&id)?;
                println!("ID: {}", post.id);
                println!("Title: {}", post.title);
                println!("Status: {}", post.status);
                println!("URL: {}", post.url);
                println!("Created: {}", post.created_at);
                println!("Updated: {}", post.updated_at);
            }
            PostCommands::Create { title, content } => {
                let post = api.posts().add(&title, &content)?;
                println!("Created post:");
                println!("ID: {}", post.id);
                println!("Title: {}", post.title);
                println!("Status: {}", post.status);
            }
            PostCommands::Update { id, title, content } => {
                let post = api.posts()
                    .edit(&id, title.as_deref(), content.as_deref())?;
                println!("Updated post:");
                println!("ID: {}", post.id);
                println!("Title: {}", post.title);
                println!("Status: {}", post.status);
            }
            PostCommands::Delete { id } => {
                api.posts().delete(&id)?;
                println!("Post {} deleted", id);
            }
        },
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}
