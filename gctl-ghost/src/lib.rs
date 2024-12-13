//! # gctl-ghost
//! 
//! A Rust library for interacting with the Ghost Admin API.
//! 
//! ## Features
//! 
//! - Post management (create, read, update, delete)
//! - JWT authentication
//! - Support for Ghost Admin API v5.0
//! - Error handling with custom error types
//! 
//! ## Example
//! 
//! ```rust,no_run
//! use gctl_ghost::GhostContentApi;
//! use url::Url;
//! 
//! // Initialize the API client
//! let url = Url::parse("https://yourblog.ghost.io").unwrap();
//! let api = GhostContentApi::new(url, "your_admin_key".to_string());
//! 
//! // List all posts
//! let posts = api.posts().browse().unwrap();
//! for post in posts {
//!     println!("{}: {}", post.id, post.title);
//! }
//! 
//! // Create a new post
//! let new_post = api.posts()
//!     .create("My New Post", "Post content here")
//!     .unwrap();
//! 
//! // Update a post
//! let updated_post = api.posts()
//!     .edit(
//!         &new_post.id,
//!         Some("Updated Title"),
//!         Some("Updated content"),
//!     )
//!     .unwrap();
//! ```

#![recursion_limit = "256"]

mod api;
mod auth;
mod error;
mod images;
mod options;
mod posts;

pub use api::GhostContentApi;
pub use error::GhostError;
pub use images::{GhostImageApi, ImageData};
pub use options::{GhostContentApiOptions, GhostVersion};
pub use posts::{Post, GhostPostsApi};