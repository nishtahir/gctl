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
    #[command(about = "Inspect posts on Ghost")]
    Post {
        #[command(subcommand)]
        command: PostCommands,
    },
    #[command(
        about = "Push markdown files (YAML front matter) to Ghost as posts",
        long_about = "Compiles markdown files with YAML front matter to Lexical and creates or \
updates the matching Ghost post.\n\n\
The first push writes the post's Ghost `id` back into the file's front matter; \
later pushes update that post, even if the slug or file name changes. A file \
without an id is matched by slug. The slug defaults to the file name (or the \
directory name for `index.md`). Files whose name starts with `_` or `.` are \
skipped.\n\n\
Status comes from the front matter (`status: draft|published|scheduled`, or \
`draft: true|false`). Without either, a new post is a draft and an existing \
post keeps its status.\n\n\
Recognised front matter: title, slug, id, status, draft, date, tags, \
categories, description, image, featured, visibility, canonical, \
codeinjection_head, codeinjection_foot.\n\n\
Reuse content with `{{< include \"snippets/footer.md\" >}}` on a line of its own: \
the other file's markdown is spliced in there (paths are relative to the including \
file, and includes may nest). Add `lang=\"rust\"` to include a file as a code block. \
Includes can only read files inside the pushed directory (or --include-root). Name \
snippet files or folders starting with `_` so they aren't pushed as posts."
    )]
    Push(PushArgs),
    #[command(
        about = "Pull posts from Ghost into markdown files",
        long_about = "Writes each post as `<dir>/<slug>.md` with YAML front matter (including its \
Ghost `id`, so a later `gctl push` updates the same post). Only posts with Lexical \
content are pulled. Existing files are matched by id.\n\n\
A file is only overwritten when it already matches the post, or with --force; \
otherwise it is reported as a conflict and left alone.\n\n\
Content markdown can't express (callouts, bookmarks, footnotes, ...) is \
approximated and listed in the file's `lossy:` front matter. `gctl push` refuses \
such files until you review them and remove that line."
    )]
    Pull(PullArgs),
    #[command(
        about = "Show how local markdown files differ from their posts on Ghost",
        long_about = "Compares each markdown file with the post it maps to (by id, or by slug \
when the file has no id) and prints a diff of the file against how `gctl pull` \
would write the post. Lines starting with `+` are in your file and would be \
pushed; lines starting with `-` are on Ghost.\n\n\
Formatting that markdown ignores is not a difference. Files that use includes or \
injection files are compared after those are resolved, but the diff shows the \
file as written."
    )]
    Diff(DiffArgs),
    #[command(
        about = "Summarize which files and posts are in sync, modified, new or remote only",
        long_about = "Lists every markdown file with how it compares to its Ghost post (in sync, \
modified, or new), plus posts on Ghost that have no local file. Use `gctl diff` \
to see what changed."
    )]
    Status(StatusArgs),
}

#[derive(Args)]
pub struct PushArgs {
    #[arg(
        default_value = "content",
        help = "Markdown file or directory of markdown files"
    )]
    pub path: String,
    #[arg(
        long,
        help = "Show what would be created or updated without changing anything"
    )]
    pub dry_run: bool,
    #[arg(
        long,
        value_name = "DIR",
        help = "Directory `{{< include >}}` may read from (default: the directory being pushed)"
    )]
    pub include_root: Option<String>,
    #[arg(
        long,
        help = "Push files that `gctl pull` marked as lossy (`lossy:` in the front matter)"
    )]
    pub force: bool,
}

#[derive(Args)]
pub struct DiffArgs {
    #[arg(
        default_value = "content",
        help = "Markdown file or directory of markdown files"
    )]
    pub path: String,
    #[arg(
        long,
        value_name = "DIR",
        help = "Directory `{{< include >}}` may read from (default: the directory being compared)"
    )]
    pub include_root: Option<String>,
}

#[derive(Args)]
pub struct StatusArgs {
    #[arg(
        default_value = "content",
        help = "Markdown file or directory of markdown files"
    )]
    pub path: String,
    #[arg(
        long,
        value_name = "DIR",
        help = "Directory `{{< include >}}` may read from (default: the directory being compared)"
    )]
    pub include_root: Option<String>,
    #[arg(long, help = "Don't look for posts on Ghost that have no local file")]
    pub no_remote: bool,
}

#[derive(Args)]
pub struct PullArgs {
    #[arg(
        default_value = "content",
        help = "Directory to write markdown files to"
    )]
    pub path: String,
    #[arg(
        long,
        conflicts_with_all = ["status", "tag", "author", "filter", "limit"],
        help = "Pull only the post with this Ghost id"
    )]
    pub id: Option<String>,
    #[arg(
        long,
        help = "Only posts with this status",
        value_parser = ["draft", "published", "scheduled", "sent"]
    )]
    pub status: Option<String>,
    #[arg(long, help = "Only posts with this tag slug")]
    pub tag: Option<String>,
    #[arg(long, help = "Only posts by this author slug")]
    pub author: Option<String>,
    #[arg(long, help = "Raw Ghost filter, ANDed with the other flags")]
    pub filter: Option<String>,
    #[arg(long, short = 'n', help = "Maximum number of posts to pull")]
    pub limit: Option<usize>,
    #[arg(
        long,
        help = "Overwrite files whose content differs from the post in Ghost"
    )]
    pub force: bool,
    #[arg(long, help = "Show what would be written without changing anything")]
    pub dry_run: bool,
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
