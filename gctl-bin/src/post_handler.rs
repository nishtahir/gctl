use std::fs;
use std::path::Path;

use console::style;
use gctl_ghost::{GhostContentApi, Post, PostFilter};
use tracing::{debug, info};

use crate::{
    cli::{PostAddArgs, PostCommands, PostDeleteArgs, PostGetArgs, PostListArgs, PostUpdateArgs},
    error::GctlCliError,
    markdown::{compile_markdown, Source},
    output::{format_post, format_posts, OutputFormat},
};
use bon::Builder;

impl From<PostListArgs> for PostFilter {
    fn from(args: PostListArgs) -> Self {
        PostFilter {
            status: args.status,
            tag: args.tag,
            author: args.author,
            featured: args.featured,
            visibility: args.visibility,
            slug: None,
            raw: args.filter,
            limit: (!args.all).then_some(args.limit),
        }
    }
}

#[derive(Builder)]
pub struct PostCommandHandler<'a> {
    api: &'a GhostContentApi<'a>,
    format: OutputFormat,
}

impl<'a> PostCommandHandler<'a> {
    pub async fn handle_command(&self, command: PostCommands) -> Result<(), GctlCliError> {
        match command {
            PostCommands::List(args) => {
                info!("Fetching all posts...");
                let posts = self.api.posts().browse(&args.into()).await?;
                debug!("Found {} posts", posts.len());
                format_posts(posts, self.format);
            }
            PostCommands::Get(PostGetArgs { id }) => {
                info!("Fetching post {}...", style(&id).yellow());
                let post = self.api.posts().read(&id).await?;
                format_post(post, self.format);
            }
            PostCommands::Add(PostAddArgs {
                title,
                content,
                json,
                status,
                file,
            }) => {
                info!("Creating new post '{}'...", style(&title).green());
                // let json_data = self
                //     .handle_file_or_json(file.as_deref(), json.as_deref())
                //     .await?;
                // let post = self
                //     .api
                //     .posts()
                //     .add(&title, content.as_deref(), json_data, status.as_deref())
                //     .await?;
                // debug!("Post created with ID: {}", style(&post.id).yellow());
                // format_post(post, self.format);
            }
            PostCommands::Update(PostUpdateArgs {
                id,
                json,
                markdown,
                title,
                status,
                slug,
                visibility,
            }) => {
                let mut post_data = Post::default();
                if let Some(json) = json {
                    let content = fs::read_to_string(json)?;
                    post_data = serde_json::from_str(&content)?;
                }

                if let Some(markdown) = markdown {
                    let content = fs::read_to_string(&markdown)?;
                    // TODO: These should be merged into the default to allow
                    // cascading overwritting.
                    let root = Path::new(&markdown).parent().unwrap_or(Path::new("."));
                    let source = Source {
                        file: Path::new(&markdown),
                        root,
                    };
                    post_data = compile_markdown(&content, Some(&source))?.post;
                }

                if let Some(id) = id {
                    post_data.id = Some(id)
                }

                if let Some(title) = title {
                    post_data.title = Some(title);
                }

                if let Some(status) = status {
                    post_data.status = Some(status)
                }

                if let Some(slug) = slug {
                    post_data.slug = Some(slug)
                }

                if let Some(visibility) = visibility {
                    post_data.visibility = Some(visibility)
                }

                let post_id = post_data.id.as_ref().unwrap();
                info!("Updating post {}...", style(&post_id).yellow());

                let post = self.api.posts().edit(&post_id, &post_data).await?;
                format_post(post, self.format);
            }
            PostCommands::Delete(PostDeleteArgs { id }) => {
                info!("Deleting post {}...", style(&id).red());
                self.api.posts().delete(&id).await?;
                info!("Post deleted successfully");
            }
        }
        Ok(())
    }
}
