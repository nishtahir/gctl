use crate::{
    error::{self, GhostError::ApiError},
    options::GhostContentApiOptions,
    Post, PostsResponse,
};
use tracing::{debug, error, trace};

use crate::auth;
use crate::model::GhostErrorResponse;
use reqwest::Client;

#[derive(Default)]
struct PostDataBuilder {
    data: serde_json::Map<String, serde_json::Value>,
}

impl PostDataBuilder {
    fn from_existing(value: &serde_json::Value) -> Self {
        Self {
            data: value
                .as_object()
                .map(|obj| obj.to_owned())
                .unwrap_or_default(),
        }
    }

    fn merge_with(&mut self, json: &serde_json::Value) -> &Self {
        if let Some(obj) = json.as_object() {
            for (key, value) in obj {
                if !value.is_null() {
                    self.data.insert(key.clone(), value.clone());
                }
            }
        }
        self
    }

    fn build(&self, key: &str) -> serde_json::Value {
        serde_json::json!({ key: [self.data] })
    }
}

/// Criteria for listing posts. Every field that is set must match.
#[derive(Debug, Default, Clone)]
pub struct PostFilter {
    /// `draft`, `published`, `scheduled` or `sent`. All of them when `None`.
    pub status: Option<String>,
    pub tag: Option<String>,
    pub author: Option<String>,
    pub featured: Option<bool>,
    pub visibility: Option<String>,
    pub slug: Option<String>,
    /// Raw Ghost NQL, ANDed with the other criteria.
    pub raw: Option<String>,
    /// Maximum number of posts to return.
    pub limit: Option<usize>,
}

impl PostFilter {
    /// Builds the Ghost NQL `filter` query value.
    pub fn to_nql(&self) -> String {
        let mut parts = vec![match &self.status {
            Some(status) => format!("status:{}", quote(status)),
            // Without an explicit status Ghost only returns drafts.
            None => "status:[draft,published,scheduled,sent]".to_string(),
        }];
        if let Some(tag) = &self.tag {
            parts.push(format!("tag:{}", quote(tag)));
        }
        if let Some(author) = &self.author {
            parts.push(format!("author:{}", quote(author)));
        }
        if let Some(featured) = self.featured {
            parts.push(format!("featured:{featured}"));
        }
        if let Some(visibility) = &self.visibility {
            parts.push(format!("visibility:{}", quote(visibility)));
        }
        if let Some(slug) = &self.slug {
            parts.push(format!("slug:{}", quote(slug)));
        }
        if let Some(raw) = &self.raw {
            parts.push(format!("({raw})"));
        }
        parts.join("+")
    }
}

/// Single-quote a value so slugs with special characters stay literal.
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Ghost keeps posts and pages behind identical endpoints with different names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Posts,
    Pages,
}

impl Resource {
    /// The endpoint, and the key of the objects in requests and responses.
    fn name(self) -> &'static str {
        match self {
            Resource::Posts => "posts",
            Resource::Pages => "pages",
        }
    }
}

pub struct GhostPostsApi<'a> {
    options: &'a GhostContentApiOptions<'a>,
    client: &'a Client,
    resource: Resource,
}

impl<'a> GhostPostsApi<'a> {
    pub fn new(
        options: &'a GhostContentApiOptions,
        client: &'a Client,
        resource: Resource,
    ) -> Self {
        GhostPostsApi {
            options,
            client,
            resource,
        }
    }

    /// Marks what came back as a page when this is the pages API.
    fn tag(&self, mut post: Post) -> Post {
        post.page = self.resource == Resource::Pages;
        post
    }

    fn generate_token(&self) -> Result<String, error::GhostError> {
        auth::generate_token(self.options.id, self.options.secret)
    }

    async fn make_request(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<reqwest::Response, error::GhostError> {
        self.make_request_with_query(method, endpoint, body, &[])
            .await
    }

    async fn make_request_with_query(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<&serde_json::Value>,
        query: &[(&str, &str)],
    ) -> Result<reqwest::Response, error::GhostError> {
        let host = self.options.url.as_str().trim_end_matches('/');
        let url = format!("{}/ghost/api/admin/{}/", host, endpoint);
        let token = self.generate_token()?;

        debug!("Making {} request to: {}", method, url);
        trace!("Authorization: Ghost {}", token);

        let mut request = self
            .client
            .request(method.parse().unwrap(), &url)
            .header("Accept-Version", self.options.version.as_ref())
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Ghost {}", token))
            .header("User-Agent", self.options.user_agent)
            .query(query);

        if let Some(json) = body {
            trace!("Request body:\n{}", serde_json::to_string_pretty(&json)?);
            request = request.json(&json);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let mut body = response.text().await?;

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                body = serde_json::to_string_pretty(&json)?;
            }
            trace!("Error body: {}", body);

            let error_message =
                if let Ok(error_response) = serde_json::from_str::<GhostErrorResponse>(&body) {
                    error_response
                        .errors
                        .into_iter()
                        .map(|e| format!("{:?}", e))
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    body
                };

            error!("API request failed: {} - {}", status, error_message);
            return Err(ApiError(format!("{} - {}", status, error_message)));
        }

        Ok(response)
    }

    /// Lists posts matching `filter`, fetching every page (or up to
    /// `filter.limit` posts).
    pub async fn browse(&self, filter: &PostFilter) -> Result<Vec<Post>, error::GhostError> {
        let filter_param = filter.to_nql();
        // Ghost caps the page size at 100 (`limit=all` included), so walk the pages.
        let mut posts = Vec::new();
        let mut page = 1;
        loop {
            let page_param = page.to_string();
            let response = self
                .make_request_with_query(
                    "GET",
                    self.resource.name(),
                    None,
                    &[
                        ("filter", filter_param.as_str()),
                        ("limit", "100"),
                        ("page", page_param.as_str()),
                    ],
                )
                .await?;
            let data: PostsResponse = response.json().await?;
            posts.extend(data.posts.into_iter().map(|post| self.tag(post)));
            if let Some(limit) = filter.limit {
                if posts.len() >= limit {
                    posts.truncate(limit);
                    return Ok(posts);
                }
            }
            match data.meta.and_then(|m| m.pagination).and_then(|p| p.next) {
                Some(next) => page = next,
                None => return Ok(posts),
            }
        }
    }

    pub async fn read(&self, post_id: &str) -> Result<Post, error::GhostError> {
        let response = self
            .make_request(
                "GET",
                &format!("{}/{}", self.resource.name(), post_id),
                None,
            )
            .await?;

        let data: PostsResponse = response.json().await?;
        Ok(self.tag(data.posts.into_iter().next().unwrap()))
    }

    pub async fn edit(&self, post_id: &str, partial: &Post) -> Result<Post, error::GhostError> {
        let existing_post = self.read(post_id).await?;
        let existing_post = &serde_json::to_value(existing_post)?;
        let mut existing_post = PostDataBuilder::from_existing(existing_post);

        existing_post.merge_with(&serde_json::to_value(partial)?);
        if partial.lexical.is_some() {
            // New lexical content supersedes whatever the post was stored as
            // before; sending the old renderings back would conflict with it.
            existing_post.data.remove("mobiledoc");
            existing_post.data.remove("html");
        }
        let updated_post = existing_post.build(self.resource.name());

        let response = self
            .make_request(
                "PUT",
                &format!("{}/{}", self.resource.name(), post_id),
                Some(&updated_post),
            )
            .await?;

        let data: PostsResponse = response.json().await?;
        Ok(self.tag(data.posts.into_iter().next().unwrap()))
    }

    pub async fn add(&self, partial: &Post) -> Result<Post, error::GhostError> {
        let mut builder = PostDataBuilder::default();
        builder.merge_with(&serde_json::to_value(partial)?);

        let response = self
            .make_request(
                "POST",
                self.resource.name(),
                Some(&builder.build(self.resource.name())),
            )
            .await?;

        let data: PostsResponse = response.json().await?;
        Ok(self.tag(data.posts.into_iter().next().unwrap()))
    }

    pub async fn delete(&self, post_id: &str) -> Result<(), error::GhostError> {
        let _response = self
            .make_request(
                "DELETE",
                &format!("{}/{}", self.resource.name(), post_id),
                None,
            )
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn default_filter_includes_every_status() {
        assert_eq!(
            PostFilter::default().to_nql(),
            "status:[draft,published,scheduled,sent]"
        );
    }

    #[test]
    fn criteria_are_anded_and_quoted() {
        let filter = PostFilter {
            status: Some("published".into()),
            tag: Some("rust".into()),
            author: Some("o'neil".into()),
            featured: Some(true),
            visibility: Some("public".into()),
            slug: Some("hello".into()),
            raw: Some("title:~'wasm',slug:foo".into()),
            limit: None,
        };
        assert_eq!(
            filter.to_nql(),
            "status:'published'+tag:'rust'+author:'o\\'neil'+featured:true+visibility:'public'+slug:'hello'+(title:~'wasm',slug:foo)"
        );
    }
}
