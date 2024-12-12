use crate::{error, options::GhostContentApiOptions};

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
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
    pub excerpt: String,
    pub reading_time: i32,
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

pub struct GhostPostsApi<'a> {
    options: &'a GhostContentApiOptions,
}

impl<'a> GhostPostsApi<'a> {
    pub fn new(options: &'a GhostContentApiOptions) -> GhostPostsApi<'a> {
        GhostPostsApi { options }
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
        Ok(encode(&header, &claims, &key)?)
    }

    pub fn browse(&self) -> Result<Vec<Post>, error::GhostError> {
        let mut url = self.options.url.clone();
        let token = self.generate_token()?;

        url.path_segments_mut()
            .unwrap()
            .push(&self.options.ghost_path)
            .push("api")
            .push("admin")
            .push("posts")
            .push("");

        eprintln!("Making request to: {}", url.as_str());
        eprintln!("Authorization: Ghost {}", token);

        let res = ureq::get(url.as_str())
            .set("Content-Type", "application/json")
            .set("Authorization", format!("Ghost {}", token).as_str())
            .set("Accept-Version", self.options.version.as_ref())
            .call()?;

        let body = res.into_string()?;
        eprintln!("Response body: {}", body);
        let response: PostsResponse = serde_json::from_str(&body)?;
        Ok(response.posts)
    }

    pub fn read(&self, post_id: &str) -> Result<Post, error::GhostError> {
        let mut url = self.options.url.clone();
        let token = self.generate_token()?;

        url.path_segments_mut()
            .unwrap()
            .push(&self.options.ghost_path)
            .push("api")
            .push("admin")
            .push("posts")
            .push(post_id)
            .push("");

        eprintln!("Making request to: {}", url.as_str());
        eprintln!("Authorization: Ghost {}", token);

        let res = ureq::get(url.as_str())
            .set("Content-Type", "application/json")
            .set("Authorization", format!("Ghost {}", token).as_str())
            .set("Accept-Version", self.options.version.as_ref())
            .call()?;

        let body = res.into_string()?;
        eprintln!("Response body: {}", body);
        let response: PostsResponse = serde_json::from_str(&body)?;
        response
            .posts
            .into_iter()
            .next()
            .ok_or_else(|| error::GhostError::ApiError("No post found".to_string()))
    }

    pub fn add(&self, title: &str, content: &str) -> Result<Post, error::GhostError> {
        let mut url = self.options.url.clone();
        let token = self.generate_token()?;

        url.path_segments_mut()
            .unwrap()
            .push(&self.options.ghost_path)
            .push("api")
            .push("admin")
            .push("posts")
            .push("");

        let post_data = serde_json::json!({
            "posts": [{
                "title": title,
                "markdown": content,
                "status": "draft"
            }]
        });

        eprintln!("Making request to: {}", url.as_str());
        eprintln!("Authorization: Ghost {}", token);
        eprintln!(
            "Request body: {}",
            serde_json::to_string_pretty(&post_data).unwrap()
        );

        let res = ureq::post(url.as_str())
            .set("Content-Type", "application/json")
            .set("Authorization", format!("Ghost {}", token).as_str())
            .set("Accept-Version", self.options.version.as_ref())
            .send_json(post_data)?;

        let body = res.into_string()?;
        eprintln!("Response body: {}", body);
        let response: PostsResponse = serde_json::from_str(&body)?;
        response
            .posts
            .into_iter()
            .next()
            .ok_or_else(|| error::GhostError::ApiError("Failed to create post".to_string()))
    }

    pub fn edit(
        &self,
        post_id: &str,
        title: Option<&str>,
        content: Option<&str>,
    ) -> Result<Post, error::GhostError> {
        let mut url = self.options.url.clone();
        let token = self.generate_token()?;

        url.path_segments_mut()
            .unwrap()
            .push(&self.options.ghost_path)
            .push("api")
            .push("admin")
            .push("posts")
            .push(post_id)
            .push("");

        let mut post_update = serde_json::Map::new();
        if let Some(title) = title {
            post_update.insert(
                "title".to_string(),
                serde_json::Value::String(title.to_string()),
            );
        }
        if let Some(content) = content {
            post_update.insert(
                "markdown".to_string(),
                serde_json::Value::String(content.to_string()),
            );
        }

        let post_data = serde_json::json!({
            "posts": [post_update]
        });

        let res = ureq::put(url.as_str())
            .set("Content-Type", "application/json")
            .set("Authorization", format!("Ghost {}", token).as_str())
            .set("Accept-Version", self.options.version.as_ref())
            .send_json(post_data)?;

        let body = res.into_string()?;
        eprintln!("Response body: {}", body);
        let response: PostsResponse = serde_json::from_str(&body)?;
        response
            .posts
            .into_iter()
            .next()
            .ok_or_else(|| error::GhostError::ApiError("Failed to update post".to_string()))
    }

    pub fn delete(&self, post_id: &str) -> Result<(), error::GhostError> {
        let mut url = self.options.url.clone();
        let token = self.generate_token()?;

        url.path_segments_mut()
            .unwrap()
            .push(&self.options.ghost_path)
            .push("api")
            .push("admin")
            .push("posts")
            .push(post_id)
            .push("");

        let _res = ureq::delete(url.as_str())
            .set("Content-Type", "application/json")
            .set("Authorization", format!("Ghost {}", token).as_str())
            .set("Accept-Version", self.options.version.as_ref())
            .call()?;

        Ok(())
    }
}
