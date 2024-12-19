use crate::{error, options::GhostContentApiOptions};
use tracing::{debug, error, trace};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::auth;

/// Our claims struct, it needs to derive `Serialize` and/or `Deserialize`
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    exp: usize,
    iat: usize,
    aud: String,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub uuid: String,
    pub title: String,
    pub slug: String,
    pub html: Option<String>,
    pub comment_id: String,
    pub feature_image: Option<String>,
    pub featured: bool,
    pub visibility: String,
    pub mobiledoc: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub custom_excerpt: Option<String>,
    pub codeinjection_head: Option<String>,
    pub codeinjection_foot: Option<String>,
    pub custom_template: Option<String>,
    pub canonical_url: Option<String>,
    pub tags: Vec<Tag>,
    pub authors: Vec<Author>,
    pub primary_author: Option<Author>,
    pub primary_tag: Option<Tag>,
    pub url: String,
    pub excerpt: Option<String>,
    pub reading_time: Option<i32>,
    pub access: Option<bool>,
    pub comments: Option<bool>,
    pub og_image: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub twitter_image: Option<String>,
    pub twitter_title: Option<String>,
    pub twitter_description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub email_subject: Option<String>,
    pub frontmatter: Option<String>,
    pub feature_image_alt: Option<String>,
    pub feature_image_caption: Option<String>,
    pub status: String,
    pub markdown: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub feature_image: Option<String>,
    pub visibility: String,
    pub url: String,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct Author {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub profile_image: Option<String>,
    pub cover_image: Option<String>,
    pub bio: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub facebook: Option<String>,
    pub twitter: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub url: String,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct PostsResponse {
    posts: Vec<Post>,
}

#[derive(Debug, Deserialize)]
struct GhostErrorDetails {
    message: String,
    context: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhostErrorResponse {
    errors: Vec<GhostErrorDetails>,
}

#[derive(Default)]
struct PostBuilder {
    data: serde_json::Map<String, serde_json::Value>,
}

impl PostBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn from_existing(value: &serde_json::Value) -> Self {
        Self {
            data: value.as_object().map(|obj| obj.to_owned()).unwrap_or_default(),
        }
    }

    fn merge_json(&mut self, json: Option<serde_json::Value>) {
        if let Some(json) = json {
            if let Some(obj) = json.as_object() {
                for (key, value) in obj {
                    if !value.is_null() {
                        self.data.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }

    fn add_fields(&mut self, title: Option<&str>, content: Option<&str>, status: Option<&str>) {
        if let Some(title) = title {
            self.data.insert("title".into(), title.into());
        }
        if let Some(content) = content {
            self.data.insert("markdown".into(), content.into());
        }
        if let Some(status) = status {
            self.data.insert("status".into(), status.into());
        }
    }

    fn build(self) -> serde_json::Value {
        serde_json::json!({ "posts": [self.data] })
    }
}

pub struct GhostPostsApi<'a> {
    options: &'a GhostContentApiOptions,
    client: &'a Client,
}

impl<'a> GhostPostsApi<'a> {
    pub fn new(options: &'a GhostContentApiOptions, client: &'a Client) -> Self {
        GhostPostsApi { options, client }
    }

    fn generate_token(&self) -> Result<String, error::GhostError> {
        auth::generate_token(&self.options.id, &self.options.secret)
    }

    async fn make_request(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<serde_json::Value>,
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
            .header("User-Agent", &self.options.user_agent);

        if let Some(json) = body {
            trace!("Request body:\n{}", serde_json::to_string_pretty(&json)?);
            request = request.json(&json);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                trace!("Error body:\n{}", serde_json::to_string_pretty(&json)?);
            } else {
                trace!("Error body: {}", body);
            }

            let error_message = if let Ok(error_response) = serde_json::from_str::<GhostErrorResponse>(&body) {
                error_response
                    .errors
                    .into_iter()
                    .map(|e| {
                        let context = e.context.unwrap_or_default();
                        let code = e.code.unwrap_or_default();
                        format!("{} (type: {}, context: {}, code: {})", 
                            e.message, 
                            e.error_type.unwrap_or_default(),
                            context,
                            code
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                body
            };

            error!("API request failed: {} - {}", status, error_message);
            return Err(error::GhostError::ApiError(format!(
                "{} - {}",
                status, error_message
            )));
        }

        Ok(response)
    }

    pub async fn browse(&self) -> Result<Vec<Post>, error::GhostError> {
        let response = self.make_request("GET", "posts", None).await?;
        let data: PostsResponse = response.json().await?;
        Ok(data.posts)
    }

    pub async fn read(&self, post_id: &str) -> Result<Post, error::GhostError> {
        let response = self
            .make_request("GET", &format!("posts/{}", post_id), None)
            .await?;
        let data: PostsResponse = response.json().await?;
        data.posts
            .into_iter()
            .next()
            .ok_or_else(|| error::GhostError::ApiError("No post found".to_string()))
    }

    pub async fn edit(
        &self,
        post_id: &str,
        title: Option<&str>,
        content: Option<&str>,
        json_data: Option<serde_json::Value>,
        status: Option<&str>,
    ) -> Result<Post, error::GhostError> {
        let existing_post = self.read(post_id).await?;
        let mut builder = PostBuilder::from_existing(&serde_json::to_value(existing_post)?);
        builder.merge_json(json_data);
        builder.add_fields(title, content, status);

        let response = self
            .make_request("PUT", &format!("posts/{}", post_id), Some(builder.build()))
            .await?;
        let data: PostsResponse = response.json().await?;

        data.posts
            .into_iter()
            .next()
            .ok_or_else(|| error::GhostError::ApiError("Failed to update post".to_string()))
    }

    pub async fn add(
        &self,
        title: &str,
        content: Option<&str>,
        json_data: Option<serde_json::Value>,
        status: Option<&str>,
    ) -> Result<Post, error::GhostError> {
        let mut builder = PostBuilder::new();
        builder.merge_json(json_data);
        builder.add_fields(Some(title), content, status);

        let response = self
            .make_request("POST", "posts", Some(builder.build()))
            .await?;
        let data: PostsResponse = response.json().await?;

        data.posts
            .into_iter()
            .next()
            .ok_or_else(|| error::GhostError::ApiError("Failed to create post".to_string()))
    }

    pub async fn delete(&self, post_id: &str) -> Result<(), error::GhostError> {
        let _response = self
            .make_request("DELETE", &format!("posts/{}", post_id), None)
            .await?;
        Ok(())
    }
}
