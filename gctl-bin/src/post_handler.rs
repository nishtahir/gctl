use gctl_ghost::{GhostContentApi, PostFilter};
use tracing::{debug, info};

use crate::{
    cli::{PostCommands, PostListArgs},
    error::GctlCliError,
    output::format_posts,
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
}

impl<'a> PostCommandHandler<'a> {
    pub async fn handle_command(&self, command: PostCommands) -> Result<(), GctlCliError> {
        match command {
            PostCommands::List(args) => {
                info!("Fetching posts...");
                let posts = self.api.posts().browse(&args.into()).await?;
                debug!("Found {} posts", posts.len());
                format_posts(posts);
            }
        }
        Ok(())
    }
}
