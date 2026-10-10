use console::Term;
use gctl_ghost::{ImageData, Post};
use tabled::builder::Builder;
use tabled::settings::peaker::PriorityMax;
use tabled::settings::{Style, Width};
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
struct ImageRow {
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Ref")]
    ref_: String,
    #[tabled(rename = "Type")]
    image_type: String,
}

/// Terminal width in columns, or `None` when stdout isn't a terminal.
fn terminal_width() -> Option<usize> {
    Term::stdout().size_checked().map(|(_, cols)| cols as usize)
}

/// Fit a table into `width` columns by shrinking the widest column first.
fn fit(table: &mut Table, width: Option<usize>) {
    table.with(Style::rounded());
    if let Some(width) = width {
        table.with(Width::truncate(width).suffix("…").priority::<PriorityMax>());
    }
}

/// `2024-05-01T10:00:00.000Z` -> `2024-05-01`
fn short_date(date: Option<String>) -> String {
    date.map(|d| d.chars().take(10).collect())
        .unwrap_or_default()
}

pub fn format_posts(posts: Vec<Post>, format: OutputFormat) {
    match format {
        OutputFormat::Table => println!("{}", render_posts(posts, terminal_width())),
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&posts).unwrap());
        }
    }
}

fn render_posts(posts: Vec<Post>, width: Option<usize>) -> Table {
    {
        {
            // Below these widths, drop the lowest-value columns so titles stay readable.
            let show_url = width.map_or(true, |w| w >= 110);
            let show_id = width.map_or(true, |w| w >= 70);

            let mut builder = Builder::default();
            let mut header = vec!["Title", "Status", "Updated"];
            if show_id {
                header.insert(0, "Id");
            }
            if show_url {
                header.push("Url");
            }
            builder.push_record(header);

            for p in posts {
                let mut row = vec![
                    p.title.unwrap_or_default(),
                    p.status.unwrap_or_default(),
                    short_date(p.updated_at),
                ];
                if show_id {
                    row.insert(0, p.id.unwrap_or_default());
                }
                if show_url {
                    row.push(p.url.unwrap_or_default());
                }
                builder.push_record(row);
            }

            let mut table = builder.build();
            fit(&mut table, width);
            table
        }
    }
}

pub fn format_post(post: Post, format: OutputFormat) {
    match format {
        OutputFormat::Table => println!("{}", render_post(post, terminal_width())),
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&post).unwrap());
        }
    }
}

fn render_post(post: Post, width: Option<usize>) -> Table {
    {
        {
            let join = |names: Vec<String>| names.join(", ");
            let fields = [
                ("Id", post.id),
                ("Title", post.title),
                ("Slug", post.slug),
                ("Status", post.status),
                ("Visibility", post.visibility),
                ("Featured", post.featured.map(|f| f.to_string())),
                (
                    "Tags",
                    post.tags
                        .map(|t| join(t.into_iter().filter_map(|t| t.name).collect())),
                ),
                (
                    "Authors",
                    post.authors
                        .map(|a| join(a.into_iter().filter_map(|a| a.name).collect())),
                ),
                ("Published", short_date(post.published_at).into()),
                ("Updated", short_date(post.updated_at).into()),
                (
                    "Reading time",
                    post.reading_time.map(|m| format!("{m} min")),
                ),
                ("Url", post.url),
                (
                    "Excerpt",
                    post.excerpt.map(|e| match e.char_indices().nth(200) {
                        Some((i, _)) => format!("{}…", e[..i].trim_end()),
                        None => e,
                    }),
                ),
            ];

            // Vertical layout: a long list of fields reads better than a wide row.
            let mut builder = Builder::default();
            for (name, value) in fields {
                if let Some(value) = value.filter(|v| !v.is_empty()) {
                    builder.push_record([name.to_string(), value.replace('\n', " ")]);
                }
            }
            let mut table = builder.build();
            table.with(Style::rounded());
            if let Some(width) = width {
                // Keep the label column intact and wrap long values instead of cutting them.
                table.with(Width::wrap(width).keep_words().priority::<PriorityMax>());
            }
            table
        }
    }
}

#[derive(Tabled, serde::Serialize)]
pub struct PublishRow {
    #[tabled(rename = "File")]
    pub file: String,
    #[tabled(rename = "Result")]
    pub result: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Detail")]
    pub detail: String,
}

pub fn format_publish_results(rows: Vec<PublishRow>, format: OutputFormat) {
    match format {
        OutputFormat::Table => {
            let mut table = Table::new(rows);
            fit(&mut table, terminal_width());
            println!("{}", table);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&rows).unwrap());
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
            let mut table = Table::new(rows);
            fit(&mut table, terminal_width());
            println!("{}", table);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&images).unwrap());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn posts() -> Vec<Post> {
        let json = include_str!("../../gctl-ghost/tests/fixtures/browse_posts_response.json");
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        serde_json::from_value(v["posts"].clone()).unwrap()
    }

    #[test]
    fn posts_table_fits_terminal_width() {
        for width in [40, 60, 80, 120] {
            let out = render_posts(posts(), Some(width)).to_string();
            let max = out.lines().map(|l| l.chars().count()).max().unwrap();
            assert!(max <= width, "width {width}: got {max}\n{out}");
            println!("--- {width}\n{out}");
        }
    }

    #[test]
    fn post_table_fits_terminal_width() {
        for width in [40, 80] {
            let out = render_post(posts().remove(0), Some(width)).to_string();
            let max = out.lines().map(|l| l.chars().count()).max().unwrap();
            assert!(max <= width, "width {width}: got {max}\n{out}");
            println!("--- {width}\n{out}");
        }
    }
}
