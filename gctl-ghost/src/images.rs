use crate::auth;
use crate::{error, options::GhostContentApiOptions};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tracing::{debug, error, trace};

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageUploadResponse {
    pub images: Vec<ImageData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageData {
    pub url: String,
    pub ref_: Option<String>,
    #[serde(rename = "type")]
    pub image_type: Option<String>,
}

pub struct GhostImageApi<'a> {
    options: &'a GhostContentApiOptions,
    client: &'a Client,
}

impl<'a> GhostImageApi<'a> {
    pub fn new(options: &'a GhostContentApiOptions, client: &'a Client) -> Self {
        GhostImageApi { options, client }
    }

    pub async fn upload(&self, file_path: &Path) -> Result<ImageData, error::GhostError> {
        let file = File::open(file_path).await?;
        let form =
            reqwest::multipart::Form::new().part("file", reqwest::multipart::Part::stream(file));

        let host = self.options.url.as_str().trim_end_matches('/');
        let url = format!("{}/ghost/api/admin/images/upload/", host);
        let token = self.generate_token()?;

        debug!("Making POST request to: {}", url);
        trace!("Authorization: Ghost {}", token);

        let response = self
            .client
            .post(&url)
            .header("Accept-Version", self.options.version.as_ref())
            .header("Authorization", format!("Ghost {}", token))
            .header("User-Agent", &self.options.user_agent)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            error!("API request failed: {} - {}", status, body);
            return Err(error::GhostError::ApiError(format!(
                "API request failed: {} - {}",
                status, body
            )));
        }

        let data: ImageUploadResponse = response.json().await?;
        data.images
            .into_iter()
            .next()
            .ok_or_else(|| error::GhostError::ApiError("No image data in response".to_string()))
    }

    fn generate_token(&self) -> Result<String, error::GhostError> {
        auth::generate_token(&self.options.id, &self.options.secret)
    }
}
