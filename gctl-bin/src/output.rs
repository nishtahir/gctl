use gctl_ghost::{ImageData, Post};
use tabled::{Table, Tabled};

use crate::error::GctlCliError;

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Table,
    Json,
}

impl TryFrom<&str> for OutputFormat {
    type Error = GctlCliError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            _ => Err(GctlCliError::ConfigError(format!(
                "Invalid output format {}",
                value
            ))),
        }
    }
}

#[derive(Tabled)]
struct PostRow {
    #[tabled(rename = "Id")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Url")]
    url: String,
}

#[derive(Tabled)]
struct ImageRow {
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Ref")]
    ref_: String,
    #[tabled(rename = "Type")]
    image_type: String,
}

pub fn format_posts(posts: Vec<Post>, format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            let rows: Vec<PostRow> = posts
                .into_iter()
                .map(|p| PostRow {
                    id: p.id.unwrap_or_default(),
                    title: p.title.unwrap_or_default(),
                    status: p.status.unwrap_or_default(),
                    url: p.url.unwrap_or_default(),
                })
                .collect();
            let table = Table::new(rows);
            println!("{}", table);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&posts).unwrap());
        }
    }
}

pub fn format_post(post: Post, format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            let rows = vec![PostRow {
                id: post.id.unwrap_or_default(),
                title: post.title.unwrap_or_default(),
                status: post.status.unwrap_or_default(),
                url: post.url.unwrap_or_default(),
            }];
            let table = Table::new(rows);
            println!("{}", table);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&post).unwrap());
        }
    }
}

pub fn format_images(images: Vec<ImageData>, format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            let rows = images.into_iter().map(|image| ImageRow {
                url: image.url,
                ref_: image.ref_.unwrap_or_default(),
                image_type: image.image_type.unwrap_or_default(),
            });
            let table = Table::new(rows);
            println!("{}", table);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&images).unwrap());
        }
    }
}
