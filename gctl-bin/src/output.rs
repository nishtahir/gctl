use gctl_ghost::Post;
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