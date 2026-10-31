use console::Term;
use gctl_ghost::Post;
use serde::Serialize;
use tabled::builder::Builder;
use tabled::settings::peaker::PriorityMax;
use tabled::settings::{Style, Width};
use tabled::{Table, Tabled};

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

pub fn format_posts(posts: Vec<Post>, json: bool) -> Result<(), serde_json::Error> {
    if json {
        let summaries: Vec<_> = posts.into_iter().map(PostSummary::from).collect();
        println!("{}", serde_json::to_string_pretty(&summaries)?);
    } else {
        println!("{}", render_posts(posts, terminal_width()));
    }
    Ok(())
}

/// The fields of a post worth listing: no content, so the output stays small.
#[derive(Serialize)]
struct PostSummary {
    id: Option<String>,
    title: Option<String>,
    slug: Option<String>,
    status: Option<String>,
    updated_at: Option<String>,
    url: Option<String>,
}

impl From<Post> for PostSummary {
    fn from(post: Post) -> Self {
        PostSummary {
            id: post.id,
            title: post.title,
            slug: post.slug,
            status: post.status,
            updated_at: post.updated_at,
            url: post.url,
        }
    }
}

fn render_posts(posts: Vec<Post>, width: Option<usize>) -> Table {
    {
        {
            // Below these widths, drop the lowest-value columns so titles stay readable.
            let show_url = width.is_none_or(|w| w >= 110);
            let show_id = width.is_none_or(|w| w >= 70);

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

#[derive(Tabled, Serialize)]
pub struct SyncRow {
    #[tabled(rename = "File")]
    pub file: String,
    #[tabled(rename = "Result")]
    pub result: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Detail")]
    pub detail: String,
}

pub fn format_sync_results(rows: Vec<SyncRow>, json: bool) -> Result<(), serde_json::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    let mut table = Table::new(rows);
    fit(&mut table, terminal_width());
    println!("{}", table);
    Ok(())
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
    fn posts_json_has_only_the_summary_fields() {
        let summaries: Vec<_> = posts().into_iter().map(PostSummary::from).collect();
        let json = serde_json::to_value(&summaries).unwrap();
        let keys: Vec<_> = json[0].as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["id", "slug", "status", "title", "updated_at", "url"]);
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
}
