use gctl_ghost::{ImageData, Post};
use tabled::{Table, Tabled};

#[derive(Tabled)]
struct PostRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "URL")]
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

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Table,
    Json,
}

pub fn format_posts(posts: Vec<Post>, format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            let rows: Vec<PostRow> = posts
                .into_iter()
                .map(|p| PostRow {
                    id: p.id,
                    title: p.title,
                    status: p.status,
                    url: p.url,
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
                id: post.id,
                title: post.title,
                status: post.status,
                url: post.url,
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
