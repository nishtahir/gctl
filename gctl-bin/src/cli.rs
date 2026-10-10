use clap::{Args, Parser, Subcommand};

pub enum GctlLogLevel {
    Normal,
    Verbose,
    VeryVerbose,
}

#[derive(Parser)]
#[command(name = "gctl")]
#[command(about = "A Ghost CLI tool", long_about = None)]
pub struct GctlCli {
    #[command(subcommand)]
    pub command: GctlCommands,
    #[arg(long, env = "GHOST_URL", global = true, required = false)]
    pub ghost_url: Option<String>,
    #[arg(long, env = "GHOST_ADMIN_KEY", global = true, required = false)]
    pub ghost_admin_key: Option<String>,
    #[arg(
        long,
        env = "GHOST_VERSION",
        global = true,
        required = false,
        default_value = "v5"
    )]
    pub ghost_api_version: String,
    #[arg(long, default_value = "table", global = true)]
    pub output_format: String,
    #[arg(short, long, global = true)]
    pub verbose: bool,
    #[arg(short = 'V', long, global = true)]
    pub very_verbose: bool,
}

impl GctlCli {
    pub fn log_level(&self) -> GctlLogLevel {
        if self.very_verbose {
            GctlLogLevel::VeryVerbose
        } else if self.verbose {
            GctlLogLevel::Verbose
        } else {
            GctlLogLevel::Normal
        }
    }
}

#[derive(Subcommand)]
pub enum GctlCommands {
    #[command(about = "Manage stored credentials")]
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    Post {
        #[command(subcommand)]
        command: PostCommands,
    },
    Image {
        #[command(subcommand)]
        command: ImageCommands,
    },
}

#[derive(Subcommand)]
pub enum AuthCommands {
    #[command(about = "Prompt for a Ghost URL and Admin API key and store them locally")]
    Login,
    #[command(about = "Remove stored credentials")]
    Logout,
    #[command(about = "Show the stored login")]
    Status,
}

#[derive(Subcommand)]
pub enum PostCommands {
    #[command(about = "List posts (all statuses unless --status is given)")]
    List(PostListArgs),
    Get(PostGetArgs),
    Add(PostAddArgs),
    Update(PostUpdateArgs),
    Delete(PostDeleteArgs),
}

#[derive(Args)]
pub struct PostListArgs {
    #[arg(
        long,
        help = "Only posts with this status [default: all]",
        value_parser = ["draft", "published", "scheduled", "sent"]
    )]
    pub status: Option<String>,
    #[arg(long, help = "Only posts with this tag slug")]
    pub tag: Option<String>,
    #[arg(long, help = "Only posts by this author slug")]
    pub author: Option<String>,
    #[arg(long, help = "Only featured (true) or non-featured (false) posts")]
    pub featured: Option<bool>,
    #[arg(
        long,
        help = "Only posts with this visibility",
        value_parser = ["public", "members", "paid", "tiers"]
    )]
    pub visibility: Option<String>,
    #[arg(
        long,
        help = "Raw Ghost filter, e.g. \"published_at:>'2024-01-01'\"; ANDed with the other flags"
    )]
    pub filter: Option<String>,
    #[arg(
        long,
        short = 'n',
        default_value_t = 10,
        help = "Maximum number of posts to return"
    )]
    pub limit: usize,
    #[arg(long, conflicts_with = "limit", help = "Return every matching post")]
    pub all: bool,
}

#[derive(Args)]
pub struct PostGetArgs {
    #[arg(help = "Post ID")]
    pub id: String,
}

#[derive(Args)]
pub struct PostAddArgs {
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub content: Option<String>,
    #[arg(long)]
    pub json: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub file: Option<String>,
}

#[derive(Args)]
#[command(group(
    clap::ArgGroup::new("update_source")
        .args(["id", "json", "markdown"])
        .required(true)
        .multiple(true)
))]
pub struct PostUpdateArgs {
    #[arg(help = "Post ID to update", group = "update_source")]
    pub id: Option<String>,
    #[arg(
        long,
        help = "Path to a JSON file with post data",
        group = "update_source"
    )]
    pub json: Option<String>,
    #[arg(
        long,
        short,
        help = "Path to a Jekyll formatted markdown file with yaml frontmatter",
        group = "update_source"
    )]
    pub markdown: Option<String>,
    #[arg(long, help = "Update post title")]
    pub title: Option<String>,
    #[arg(long, help = "Update post slug")]
    pub slug: Option<String>,
    #[arg(long, help = "Update post status")]
    pub status: Option<String>,
    #[arg(long, help = "Update post visibility")]
    pub visibility: Option<String>,
}

#[derive(Args)]
pub struct PostDeleteArgs {
    #[arg(help = "Post ID")]
    pub id: String,
}

#[derive(Subcommand)]
pub enum ImageCommands {
    Upload {
        #[arg(help = "Image file")]
        file: String,

        #[arg(help = "Image Purpose", long)]
        purpose: Option<String>,

        #[arg(help = "Image ref", long)]
        ref_: Option<String>,
    },
}
