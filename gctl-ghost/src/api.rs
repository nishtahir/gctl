// Copilot note - We're porting a Javascript API to Rust

use reqwest::Client;
use url::Url;

use crate::{
    images::GhostImageApi,
    options::{GhostContentApiOptions, GhostVersion},
    posts::GhostPostsApi,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("API error: {0}")]
    ApiError(String),
    // ... other variants ...
}

pub struct GhostContentApi {
    options: GhostContentApiOptions,
    client: Client,
}

impl GhostContentApi {
    pub fn new(url: Url, key: String) -> GhostContentApi {
        let components: Vec<&str> = key.split(":").collect::<Vec<&str>>();
        if components.len() != 2 {
            panic!("Invalid key format");
        }

        let id = components[0];
        let secret = components[1];

        let options = GhostContentApiOptions {
            url,
            id: id.to_owned(),
            secret: secret.to_owned(),
            version: GhostVersion::V5,
            user_agent:
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:64.0) Gecko/20100101 Firefox/64.0"
                    .to_owned(),
            ghost_path: "ghost".to_owned(),
        };

        let client = Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        GhostContentApi { options, client }
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
        GhostPostsApi::new(&self.options, &self.client)
    }

    pub fn images(&self) -> GhostImageApi {
        GhostImageApi::new(&self.options, &self.client)
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TEST_KEY: &str =
        "672fca652213be00013adf1d:6110e87ab802d66423ce13756ea7b9708b5017cacaefd758c1523f047fac410a";

    async fn setup_test_api(mock_server: &MockServer) -> GhostContentApi {
        let url = Url::parse(&mock_server.uri()).unwrap();
        GhostContentApi::new(url, TEST_KEY.to_string()).with_ghost_path("ghost".to_string())
    }

    fn init() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Trace)
            .is_test(true)
            .try_init();
    }

    const BROWSE_POSTS_RESPONSE: &str =
        include_str!("../tests/fixtures/browse_posts_response.json");
    const READ_POST_RESPONSE: &str = include_str!("../tests/fixtures/read_post_response.json");
    const ADD_POST_RESPONSE: &str = include_str!("../tests/fixtures/add_post_response.json");
    const UPDATE_POST_RESPONSE: &str = include_str!("../tests/fixtures/update_post_response.json");
    const UPLOAD_IMAGE_RESPONSE: &str =
        include_str!("../tests/fixtures/upload_image_response.json");

    #[tokio::test]
    async fn test_browse_posts() {
        init();
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/ghost/api/admin/posts/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .respond_with(ResponseTemplate::new(200).set_body_string(BROWSE_POSTS_RESPONSE))
            .expect(1)
            .mount(&mock_server)
            .await;

        let api = setup_test_api(&mock_server).await;
        let posts = api.posts().browse().await.unwrap();

        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].title, "Coming soon");
        assert_eq!(posts[0].status, "published");
    }

    #[tokio::test]
    async fn test_read_post() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/ghost/api/admin/posts/6759fd1c9ddd22000110d2d4/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .respond_with(ResponseTemplate::new(200).set_body_string(READ_POST_RESPONSE))
            .expect(1)
            .mount(&mock_server)
            .await;

        let api = setup_test_api(&mock_server).await;
        let post = api.posts().read("6759fd1c9ddd22000110d2d4").await.unwrap();

        assert_eq!(post.id, "6759fd1c9ddd22000110d2d4");
        assert_eq!(post.title, "Coming soon");
        assert_eq!(post.status, "published");
    }

    #[tokio::test]
    async fn test_add_post() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/ghost/api/admin/posts/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .and(body_json(serde_json::json!({
                "posts": [{
                    "title": "Test Title",
                    "markdown": "Test Content"
                }]
            })))
            .respond_with(ResponseTemplate::new(201).set_body_string(ADD_POST_RESPONSE))
            .expect(1)
            .mount(&mock_server)
            .await;

        let api = setup_test_api(&mock_server).await;
        let post = api
            .posts()
            .add("Test Title", Some("Test Content"), None, None)
            .await
            .unwrap();

        assert_eq!(post.id, "test-post-id");
        assert_eq!(post.title, "Test Title");
        assert_eq!(post.markdown.as_deref(), Some("Test Content"));
        assert_eq!(post.status, "draft");
    }

    #[tokio::test]
    async fn test_delete_post() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/ghost/api/admin/posts/test-post-id/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&mock_server)
            .await;

        let server_url = mock_server.uri();
        let url = Url::parse(&server_url).unwrap();
        let key = "672fca652213be00013adf1d:6110e87ab802d66423ce13756ea7b9708b5017cacaefd758c1523f047fac410a".to_string();
        let api = GhostContentApi::new(url, key).with_ghost_path("ghost".to_string());
        let result = api.posts().delete("test-post-id").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_post() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/ghost/api/admin/posts/test-post-id/"))
            .and(header("Content-Type", "application/json"))
            .and(header("Accept-Version", "v5.0"))
            .and(body_json(serde_json::json!({
              "posts": [
                {
                  "authors": [],
                  "comment_id": "5f5b1f3b4f0f4b0001f3b4f0",
                  "created_at": "2024-12-11T20:59:08.000Z",
                  "featured": false,
                  "id": "test-post-id",
                  "slug": "test-post-id",
                  "status": "draft",
                  "tags": [],
                  "title": "Updated Title",
                  "updated_at": "2024-12-11T21:00:03.000Z",
                  "url": "http://localhost:2368/p/3e257b92-54fc-4f0e-94a2-d2d735c2ed9f/",
                  "uuid": "test-post-id",
                  "visibility": "public"
                }
              ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(UPDATE_POST_RESPONSE))
            .expect(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/ghost/api/admin/posts/test-post-id/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "posts": [{
                    "uuid": "test-post-id",
                    "id": "test-post-id",
                    "slug": "test-post-id",
                    "title": "Updated Title",
                    "status": "draft",
                    "comment_id": "5f5b1f3b4f0f4b0001f3b4f0",
                    "featured": false,
                    "visibility": "public",
                    "created_at": "2024-12-11T20:59:08.000Z",
                    "updated_at": "2024-12-11T21:00:03.000Z",
                    "tags": [],
                    "tiers": [],
                    "authors": [],
                    "url": "http://localhost:2368/p/3e257b92-54fc-4f0e-94a2-d2d735c2ed9f/",
                }]
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let server_url = mock_server.uri();
        let url = Url::parse(&server_url).unwrap();
        let key = "672fca652213be00013adf1d:6110e87ab802d66423ce13756ea7b9708b5017cacaefd758c1523f047fac410a".to_string();
        let api = GhostContentApi::new(url, key).with_ghost_path("ghost".to_string());
        let post = api
            .posts()
            .edit(
                "test-post-id",
                Some("Updated Title"),
                None,
                None,
                Some("draft"),
            )
            .await
            .unwrap();

        assert_eq!(post.id, "test-post-id");
        assert_eq!(post.title, "Updated Title");
        assert_eq!(post.status, "draft");
    }

    #[tokio::test]
    async fn test_upload_image() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/ghost/api/admin/images/upload/"))
            .and(header("Accept-Version", "v5.0"))
            .respond_with(ResponseTemplate::new(201).set_body_string(UPLOAD_IMAGE_RESPONSE))
            .expect(1)
            .mount(&mock_server)
            .await;

        let api = setup_test_api(&mock_server).await;
        let image = api
            .images()
            .upload(Path::new("tests/fixtures/test-image.png"))
            .await
            .unwrap();

        assert_eq!(
            image.url,
            "https://example.com/content/images/2024/01/test-image.png"
        );
        assert_eq!(image.image_type.as_deref(), Some("image/png"));
    }
}
