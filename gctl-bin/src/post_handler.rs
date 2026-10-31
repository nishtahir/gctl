use gctl_ghost::{GhostContentApi, Post, PostFilter};
use tracing::{debug, info};
use url::Url;

use crate::{
    cli::{PostCommands, PostListArgs, PostOpenArgs},
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

/// Ghost ids are 24 hex characters; anything else is taken to be a slug.
fn looks_like_id(key: &str) -> bool {
    key.len() == 24 && key.chars().all(|c| c.is_ascii_hexdigit())
}

/// The post with this id or slug.
pub(crate) async fn find_post(api: &GhostContentApi<'_>, key: &str) -> Result<Post, GctlCliError> {
    if looks_like_id(key) {
        return Ok(api.posts().read(key).await?);
    }
    let filter = PostFilter {
        slug: Some(key.to_string()),
        limit: Some(1),
        ..PostFilter::default()
    };
    api.posts()
        .browse(&filter)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| GctlCliError::Config(format!("No post with the id or slug `{key}`")))
}

/// The post's address in the Ghost editor.
fn editor_url(site: &Url, id: &str) -> Result<Url, GctlCliError> {
    Ok(site.join(&format!("ghost/#/editor/post/{id}"))?)
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
                let json = args.json;
                let posts = self.api.posts().browse(&args.into()).await?;
                debug!("Found {} posts", posts.len());
                format_posts(posts, json)?;
            }
            PostCommands::Open(args) => self.open(args).await?,
        }
        Ok(())
    }
}

impl<'a> PostCommandHandler<'a> {
    async fn open(&self, args: PostOpenArgs) -> Result<(), GctlCliError> {
        let post = find_post(self.api, &args.post).await?;
        let url = if args.admin {
            let id = post.id.as_deref().unwrap_or(&args.post);
            editor_url(self.api.site_url(), id)?.to_string()
        } else {
            post.url
                .ok_or_else(|| GctlCliError::Config("The post has no address".into()))?
        };
        if args.print {
            println!("{url}");
        } else {
            open::that(&url)
                .map_err(|e| GctlCliError::Config(format!("Could not open {url}: {e}")))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ids_are_told_from_slugs() {
        let keys = [
            "6891829ae317750001061429",
            "hello-world",
            "6891829ae31775000106142",
            "6891829ae317750001061429a",
            "zzzzzzzzzzzzzzzzzzzzzzzz",
        ];
        let results: Vec<_> = keys.iter().map(|k| (*k, looks_like_id(k))).collect();
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn editor_urls_are_under_the_site() {
        let urls: Vec<_> = ["https://example.com", "https://example.com/blog/"]
            .iter()
            .map(|site| {
                editor_url(&Url::parse(site).unwrap(), "abc")
                    .unwrap()
                    .to_string()
            })
            .collect();
        insta::assert_debug_snapshot!(urls);
    }
}
