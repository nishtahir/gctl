use crate::{error, options::GhostContentApiOptions};
use tracing::{debug, error, trace};

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Our claims struct, it needs to derive `Serialize` and/or `Deserialize`
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    exp: usize,
    iat: usize,
    aud: String,
}

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
    pub primary_author: Author,
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

pub struct GhostPostsApi<'a> {
    options: &'a GhostContentApiOptions,
    client: &'a Client,
}

impl<'a> GhostPostsApi<'a> {
    pub fn new(options: &'a GhostContentApiOptions, client: &'a Client) -> Self {
        GhostPostsApi { options, client }
    }

    fn generate_token(&self) -> Result<String, error::GhostError> {
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("JWT".to_string());
        header.kid = Some(self.options.id.clone());

        let now = SystemTime::now();
        let timestamp = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

        let claims = Claims {
            iat: timestamp.as_secs() as usize,
            exp: timestamp.as_secs() as usize + 300,
            aud: "/admin/".to_string(),
        };

        let bytes = hex::decode(&self.options.secret)
            .map_err(|e| error::GhostError::ParseError(e.to_string()))?;
        let key = EncodingKey::from_secret(&bytes);
        debug!("Generated JWT token");
        Ok(encode(&header, &claims, &key)?)
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
            trace!("Request body: {}", &json);
            request = request.json(&json);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;

            trace!("Error body: {}", body);

            let error_message =
                if let Ok(error_response) = serde_json::from_str::<GhostErrorResponse>(&body) {
                    error_response
                        .errors
                        .into_iter()
                        .map(|e| format!("{} ({})", e.message, e.error_type.unwrap_or_default()))
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

    fn add_string_field(
        obj: &mut serde_json::Map<String, serde_json::Value>,
        key: &str,
        value: &str,
    ) {
        obj.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }

    fn create_post_fields(
        &self,
        post_data: &mut serde_json::Map<String, serde_json::Value>,
        title: Option<&str>,
        content: Option<&str>,
        status: Option<&str>,
    ) {
        if !post_data.contains_key("updated_at") {
            let now = chrono::Utc::now();
            Self::add_string_field(post_data, "updated_at", &now.to_rfc3339());
        }

        if let Some(title) = title {
            Self::add_string_field(post_data, "title", title);
        }
        if let Some(content) = content {
            Self::add_string_field(post_data, "markdown", content);
        }
        if let Some(status) = status {
            Self::add_string_field(post_data, "status", status);
        }
    }

    fn wrap_post_data(post_data: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
        serde_json::json!({ "posts": [post_data] })
    }

    pub async fn edit(
        &self,
        post_id: &str,
        title: Option<&str>,
        content: Option<&str>,
        json_data: Option<serde_json::Value>,
        status: Option<&str>,
    ) -> Result<Post, error::GhostError> {
        let mut post_data = if let Some(mut json) = json_data {
            json.as_object_mut()
                .map(|obj| {
                    obj.retain(|_, v| !v.is_null());
                    obj.to_owned()
                })
                .unwrap_or_default()
        } else {
            serde_json::Map::new()
        };

        self.create_post_fields(&mut post_data, title, content, status);
        let request_body = Self::wrap_post_data(post_data);

        trace!(
            "Request body: {}",
            serde_json::to_string_pretty(&request_body).unwrap()
        );

        let response = self
            .make_request("PUT", &format!("posts/{}", post_id), Some(request_body))
            .await?;
        let data: PostsResponse = response.json().await?;
        trace!(
            "Response body: {}",
            serde_json::to_string_pretty(&data).unwrap()
        );

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
        let mut post_data = if let Some(mut json) = json_data {
            json.as_object_mut()
                .map(|obj| {
                    obj.retain(|_, v| !v.is_null());
                    obj.to_owned()
                })
                .unwrap_or_default()
        } else {
            serde_json::Map::new()
        };

        self.create_post_fields(&mut post_data, Some(title), content, status);
        let request_body = Self::wrap_post_data(post_data);

        trace!(
            "Request body: {}",
            serde_json::to_string_pretty(&request_body).unwrap()
        );

        let response = self
            .make_request("POST", "posts", Some(request_body))
            .await?;
        let data: PostsResponse = response.json().await?;
        trace!(
            "Response body: {}",
            serde_json::to_string_pretty(&data).unwrap()
        );

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
