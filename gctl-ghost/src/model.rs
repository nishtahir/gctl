use bon::Builder;
use serde::{Deserialize, Serialize};

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Builder, Default)]
pub struct Post {
    pub id: Option<String>,
    pub uuid: Option<String>,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub html: Option<String>,
    pub comment_id: String,
    pub feature_image: Option<String>,
    pub featured: Option<bool>,
    pub visibility: Option<String>,
    pub mobiledoc: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
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
    pub url: Option<String>,
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
    pub status: Option<String>,
    pub markdown: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub feature_image: Option<String>,
    pub visibility: Option<String>,
    pub url: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct Author {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub profile_image: Option<String>,
    pub cover_image: Option<String>,
    pub bio: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub facebook: Option<String>,
    pub twitter: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub url: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize)]
pub struct PostsResponse {
    pub posts: Vec<Post>,
}

#[derive(Debug, Deserialize)]
pub struct GhostErrorDetails {
    pub message: Option<String>,
    pub context: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhostErrorResponse {
    pub errors: Vec<GhostErrorDetails>,
}
