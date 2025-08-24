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

    fn build(&self) -> serde_json::Value {
        serde_json::json!({ "posts": [self.data] })
    }
}

pub struct GhostPostsApi<'a> {
    options: &'a GhostContentApiOptions<'a>,
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
        body: Option<&serde_json::Value>,
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
            .header("User-Agent", self.options.user_agent);

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
        Ok(data.posts.into_iter().next().unwrap())
    }

    pub async fn edit(&self, post_id: &str, partial: &Post) -> Result<Post, error::GhostError> {
        let existing_post = self.read(post_id).await?;
        let existing_post = &serde_json::to_value(existing_post)?;
        let mut existing_post = PostDataBuilder::from_existing(existing_post);

        let updated_post = existing_post.merge_with(&serde_json::to_value(partial)?);
        let updated_post = updated_post.build();

        let response = self
            .make_request("PUT", &format!("posts/{}", post_id), Some(&updated_post))
            .await?;

        let data: PostsResponse = response.json().await?;
        Ok(data.posts.into_iter().next().unwrap())
    }

    pub async fn add(&self, partial: &Post) -> Result<Post, error::GhostError> {
        let mut builder = PostDataBuilder::default();
        builder.merge_with(&serde_json::to_value(partial)?);

        let response = self
            .make_request("POST", "posts", Some(&builder.build()))
            .await?;

        let data: PostsResponse = response.json().await?;
        Ok(data.posts.into_iter().next().unwrap())
    }

    pub async fn delete(&self, post_id: &str) -> Result<(), error::GhostError> {
        let _response = self
            .make_request("DELETE", &format!("posts/{}", post_id), None)
            .await?;

        Ok(())
    }
}
