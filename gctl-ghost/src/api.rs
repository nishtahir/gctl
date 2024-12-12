// Copilot note - We're porting a Javascript API to Rust

use url::Url;

use crate::{
    options::{GhostContentApiOptions, GhostVersion},
    posts::GhostPostsApi,
};

pub struct GhostContentApi {
    options: GhostContentApiOptions,
}

impl GhostContentApi {
    pub fn new(url: Url, key: String) -> GhostContentApi {
        let components: Vec<&str> = key.split(":").collect::<Vec<&str>>();
        if components.len() != 2 {
            panic!("Invalid key format");
        }

        let id = components[0];
        let secret = components[1];

        GhostContentApi {
            options: GhostContentApiOptions {
                url,
                id: id.to_owned(),
                secret: secret.to_owned(),
                version: GhostVersion::V5,
                user_agent:
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:64.0) Gecko/20100101 Firefox/64.0"
                        .to_owned(),
                ghost_path: "ghost".to_owned(),
            },
        }
    }

    pub fn with_version(mut self, version: GhostVersion) -> GhostContentApi {
        self.options.version = version;
        self
    }
    pub fn with_user_agent(mut self, user_agent: String) -> GhostContentApi {
        self.options.user_agent = user_agent;
        self
    }
    pub fn with_ghost_path(mut self, ghost_path: String) -> GhostContentApi {
        self.options.ghost_path = ghost_path;
        self
    }

    pub fn posts(&self) -> GhostPostsApi {
        GhostPostsApi::new(&self.options)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const BROWSE_POSTS_RESPONSE: &str =
        include_str!("../tests/fixtures/browse_posts_response.json");
    const READ_POST_RESPONSE: &str = include_str!("../tests/fixtures/read_post_response.json");

    async fn setup_mock_api() -> (GhostContentApi, MockServer) {
        let mock_server = MockServer::start().await;
        println!("Mock server started at: {}", mock_server.uri());

        let url = Url::parse(&mock_server.uri()).unwrap();
        let key = "672fca652213be00013adf1d:6110e87ab802d66423ce13756ea7b9708b5017cacaefd758c1523f047fac410a".to_string();
        let api = GhostContentApi::new(url, key).with_ghost_path("ghost".to_string());

        (api, mock_server)
    }

    #[tokio::test]
    async fn test_browse_posts() {
        let (api, mock_server) = setup_mock_api().await;

        // Ghost Admin API requires trailing slashes
        Mock::given(method("GET"))
            .and(path("/ghost/api/admin/posts/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .respond_with(ResponseTemplate::new(200).set_body_string(BROWSE_POSTS_RESPONSE))
            .expect(1)
            .mount(&mock_server)
            .await;

        let posts = api.posts().browse().unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].title, "Coming soon");
        assert_eq!(posts[0].status, "published");
    }

    #[tokio::test]
    async fn test_read_post() {
        let (api, mock_server) = setup_mock_api().await;

        // Ghost Admin API requires trailing slashes
        Mock::given(method("GET"))
            .and(path("/ghost/api/admin/posts/6759fd1c9ddd22000110d2d4/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .respond_with(ResponseTemplate::new(200).set_body_string(READ_POST_RESPONSE))
            .expect(1)
            .mount(&mock_server)
            .await;

        let post = api.posts().read("6759fd1c9ddd22000110d2d4").unwrap();
        assert_eq!(post.id, "6759fd1c9ddd22000110d2d4");
        assert_eq!(post.title, "Coming soon");
        assert_eq!(post.status, "published");
    }

    #[tokio::test]
    async fn test_add_post() {
        let (api, mock_server) = setup_mock_api().await;

        // Ghost Admin API requires trailing slashes
        Mock::given(method("POST"))
            .and(path("/ghost/api/admin/posts/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .and(body_json(serde_json::json!({
                "posts": [{
                    "title": "Test Title",
                    "markdown": "Test Content",
                    "status": "draft"
                }]
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "posts": [
                    {
                        "id": "new-post-id",
                        "uuid": "1b7d4f90-b9c1-4ef8-8f0d-7aa1bcfc1e3e",
                        "title": "Test Title",
                        "slug": "test-title",
                        "html": "<p>Test Content</p>",
                        "comment_id": "1",
                        "feature_image": null,
                        "featured": false,
                        "visibility": "public",
                        "created_at": "2023-01-01T00:00:00.000Z",
                        "updated_at": "2023-01-01T00:00:00.000Z",
                        "published_at": null,
                        "custom_excerpt": null,
                        "codeinjection_head": null,
                        "codeinjection_foot": null,
                        "custom_template": null,
                        "canonical_url": null,
                        "tags": [],
                        "authors": [],
                        "primary_author": {
                            "id": "1",
                            "name": "Test Author",
                            "slug": "test-author",
                            "profile_image": null,
                            "cover_image": null,
                            "bio": null,
                            "website": null,
                            "location": null,
                            "facebook": null,
                            "twitter": null,
                            "meta_title": null,
                            "meta_description": null,
                            "url": "/author/test-author"
                        },
                        "primary_tag": null,
                        "url": "/test-title",
                        "excerpt": "Test Content",
                        "reading_time": 1,
                        "access": true,
                        "comments": false,
                        "og_image": null,
                        "og_title": null,
                        "og_description": null,
                        "twitter_image": null,
                        "twitter_title": null,
                        "twitter_description": null,
                        "meta_title": null,
                        "meta_description": null,
                        "email_subject": null,
                        "frontmatter": null,
                        "feature_image_alt": null,
                        "feature_image_caption": null,
                        "status": "draft",
                        "markdown": "Test Content"
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let post = api.posts().add("Test Title", Some("Test Content")).unwrap();
        assert_eq!(post.id, "new-post-id");
        assert_eq!(post.title, "Test Title");
        assert_eq!(post.status, "draft");
    }

    #[tokio::test]
    async fn test_edit_post() {
        let (api, mock_server) = setup_mock_api().await;

        // Ghost Admin API requires trailing slashes
        Mock::given(method("PUT"))
            .and(path("/ghost/api/admin/posts/test-post-id/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .and(body_json(serde_json::json!({
                "posts": [{
                    "title": "Updated Title",
                    "markdown": "Updated Content"
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "posts": [
                    {
                        "id": "test-post-id",
                        "uuid": "1b7d4f90-b9c1-4ef8-8f0d-7aa1bcfc1e3e",
                        "title": "Updated Title",
                        "slug": "updated-title",
                        "html": "<p>Updated Content</p>",
                        "comment_id": "1",
                        "feature_image": null,
                        "featured": false,
                        "visibility": "public",
                        "created_at": "2023-01-01T00:00:00.000Z",
                        "updated_at": "2023-01-01T00:00:00.000Z",
                        "published_at": null,
                        "custom_excerpt": null,
                        "codeinjection_head": null,
                        "codeinjection_foot": null,
                        "custom_template": null,
                        "canonical_url": null,
                        "tags": [],
                        "authors": [],
                        "primary_author": {
                            "id": "1",
                            "name": "Test Author",
                            "slug": "test-author",
                            "profile_image": null,
                            "cover_image": null,
                            "bio": null,
                            "website": null,
                            "location": null,
                            "facebook": null,
                            "twitter": null,
                            "meta_title": null,
                            "meta_description": null,
                            "url": "/author/test-author"
                        },
                        "primary_tag": null,
                        "url": "/updated-title",
                        "excerpt": "Updated Content",
                        "reading_time": 1,
                        "access": true,
                        "comments": false,
                        "og_image": null,
                        "og_title": null,
                        "og_description": null,
                        "twitter_image": null,
                        "twitter_title": null,
                        "twitter_description": null,
                        "meta_title": null,
                        "meta_description": null,
                        "email_subject": null,
                        "frontmatter": null,
                        "feature_image_alt": null,
                        "feature_image_caption": null,
                        "status": "draft",
                        "markdown": "Updated Content"
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let post = api
            .posts()
            .edit(
                "test-post-id",
                Some("Updated Title"),
                Some("Updated Content"),
            )
            .unwrap();
        assert_eq!(post.id, "test-post-id");
        assert_eq!(post.title, "Updated Title");
        assert_eq!(post.status, "draft");
    }

    #[tokio::test]
    async fn test_delete_post() {
        let (api, mock_server) = setup_mock_api().await;

        // Ghost Admin API requires trailing slashes
        Mock::given(method("DELETE"))
            .and(path("/ghost/api/admin/posts/test-post-id/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = api.posts().delete("test-post-id");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_post() {
        let (api, mock_server) = setup_mock_api().await;

        // Ghost Admin API requires trailing slashes
        Mock::given(method("PUT"))
            .and(path("/ghost/api/admin/posts/test-post-id/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .and(body_json(serde_json::json!({
                "posts": [{
                    "title": "Updated Title",
                    "markdown": "Updated Content"
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "posts": [
                    {
                        "id": "test-post-id",
                        "uuid": "1b7d4f90-b9c1-4ef8-8f0d-7aa1bcfc1e3e",
                        "title": "Updated Title",
                        "slug": "updated-title",
                        "html": "<p>Updated Content</p>",
                        "comment_id": "1",
                        "feature_image": null,
                        "featured": false,
                        "visibility": "public",
                        "created_at": "2023-01-01T00:00:00.000Z",
                        "updated_at": "2023-01-01T00:00:00.000Z",
                        "published_at": null,
                        "custom_excerpt": null,
                        "codeinjection_head": null,
                        "codeinjection_foot": null,
                        "custom_template": null,
                        "canonical_url": null,
                        "tags": [],
                        "authors": [],
                        "primary_author": {
                            "id": "1",
                            "name": "Test Author",
                            "slug": "test-author",
                            "profile_image": null,
                            "cover_image": null,
                            "bio": null,
                            "website": null,
                            "location": null,
                            "facebook": null,
                            "twitter": null,
                            "meta_title": null,
                            "meta_description": null,
                            "url": "/author/test-author"
                        },
                        "primary_tag": null,
                        "url": "/updated-title",
                        "excerpt": "Updated Content",
                        "reading_time": 1,
                        "access": true,
                        "comments": false,
                        "og_image": null,
                        "og_title": null,
                        "og_description": null,
                        "twitter_image": null,
                        "twitter_title": null,
                        "twitter_description": null,
                        "meta_title": null,
                        "meta_description": null,
                        "email_subject": null,
                        "frontmatter": null,
                        "feature_image_alt": null,
                        "feature_image_caption": null,
                        "status": "draft",
                        "markdown": "Updated Content"
                    }
                ]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let post = api
            .posts()
            .edit(
                "test-post-id",
                Some("Updated Title"),
                Some("Updated Content"),
            )
            .unwrap();
        assert_eq!(post.id, "test-post-id");
        assert_eq!(post.title, "Updated Title");
        assert_eq!(post.markdown.as_deref(), Some("Updated Content"));
        assert_eq!(post.status, "draft");
    }
}
