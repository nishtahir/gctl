use chrono::{DateTime, Utc};
use gctl_ghost::{Post, Tag};
use gctl_markdown::{Document, FrontMatter};

use crate::error::GctlCliError;

/// A content file ready to publish.
pub struct CompiledPost {
    pub post: Post,
}

/// Compiles a markdown file with YAML front matter into a Ghost post whose
/// body is Lexical.
pub fn compile_markdown(content: &str) -> Result<CompiledPost, GctlCliError> {
    compile_markdown_at(content, Utc::now())
}

fn compile_markdown_at(content: &str, now: DateTime<Utc>) -> Result<CompiledPost, GctlCliError> {
    let document = gctl_markdown::parse(content)?;
    let post = to_post(&document, now)?;
    Ok(CompiledPost { post })
}

fn to_post(document: &Document, now: DateTime<Utc>) -> Result<Post, GctlCliError> {
    let fm = &document.front_matter;
    let published_at = fm.published_at()?;

    let status = resolve_status(fm, published_at, now)?;

    let tags = fm.all_tags();
    Ok(Post {
        id: fm.id.clone(),
        title: fm.title.clone(),
        slug: fm.slug.clone(),
        lexical: Some(document.to_lexical_string()),
        status: status.map(str::to_string),
        published_at: published_at.map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        custom_excerpt: fm.description.clone(),
        feature_image: fm.feature_image.clone(),
        featured: fm.featured,
        visibility: fm.visibility.clone(),
        canonical_url: fm.canonical_url.clone(),
        tags: (!tags.is_empty()).then(|| {
            tags.into_iter()
                .map(|name| Tag {
                    id: None,
                    name: Some(name),
                    slug: None,
                    description: None,
                    feature_image: None,
                    visibility: None,
                    url: None,
                })
                .collect()
        }),
        ..Post::default()
    })
}

/// The post's status comes from its front matter: `status:` if given, else
/// `draft: true|false`. A published post dated in the future is scheduled
/// instead. `None` means the file doesn't say; the caller decides (a new post
/// becomes a draft, an existing one keeps the status it has).
fn resolve_status(
    fm: &FrontMatter,
    published_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<Option<&'static str>, GctlCliError> {
    if fm.status.is_none() && fm.draft.is_none() {
        return Ok(None);
    }
    let wants_published = match (fm.status.as_deref(), fm.draft) {
        (Some("draft"), Some(false)) | (Some("published" | "scheduled"), Some(true)) => {
            return Err(GctlCliError::ParseError(
                "front matter `status` and `draft` contradict each other".into(),
            ))
        }
        (Some("draft"), _) | (None, Some(true) | None) => false,
        (Some("published" | "scheduled"), _) | (None, Some(false)) => true,
        (Some(other), _) => {
            return Err(GctlCliError::ParseError(format!(
                "invalid `status` '{other}': expected draft, published or scheduled"
            )))
        }
    };

    let in_future = published_at.is_some_and(|date| date > now);
    match (wants_published, fm.status.as_deref()) {
        (false, _) => Ok(Some("draft")),
        (true, _) if in_future => Ok(Some("scheduled")),
        (true, Some("scheduled")) => Err(GctlCliError::ParseError(
            "`status: scheduled` needs a `date` in the future".into(),
        )),
        (true, _) => Ok(Some("published")),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn at(date: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(date)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn compile(content: &str) -> CompiledPost {
        compile_markdown_at(content, at("2024-06-01T00:00:00Z")).unwrap()
    }

    #[test]
    fn maps_front_matter_to_post() {
        let compiled = compile(
            "---\ntitle: Hello World\nslug: hello-world\nid: foo\ndate: 2024-01-31\ntags: [a, b]\ndescription: Short\n---\n\nSome *content*\n",
        );
        let mut post = compiled.post;
        // The lexical body is covered by the markdown crate's snapshot.
        assert!(post.lexical.take().unwrap().contains("Some "));
        insta::assert_json_snapshot!(post);
    }

    #[test]
    fn status_comes_from_front_matter() {
        let status = |fm: &str| {
            let result =
                compile_markdown_at(&format!("---\n{fm}\n---\n"), at("2024-06-01T00:00:00Z"))
                    .map(|compiled| compiled.post.status)
                    .map_err(|e| e.to_string());
            (fm.to_string(), result)
        };
        insta::assert_debug_snapshot!([
            // Nothing specified: left to the caller (draft on create, unchanged on update).
            status("title: a"),
            status("draft: true"),
            status("draft: false"),
            status("status: draft"),
            status("status: published"),
            // Future dates schedule instead of publishing.
            status("draft: false\ndate: 2024-01-01"),
            status("draft: false\ndate: 2025-01-01"),
            status("status: published\ndate: 2025-01-01"),
            status("status: scheduled\ndate: 2025-01-01"),
            status("status: draft\ndate: 2025-01-01"),
            status("date: 2025-01-01"),
            // Problems.
            status("status: scheduled"),
            status("status: scheduled\ndate: 2024-01-01"),
            status("status: live"),
            status("status: draft\ndraft: false"),
            status("status: published\ndraft: true"),
        ]);
    }

    #[test]
    fn rejects_invalid_front_matter() {
        let errors: Vec<_> = ["---\ndate: nope\n---\n", "---\n: [\n---\n"]
            .iter()
            .map(|input| compile_markdown(input).err().map(|e| e.to_string()))
            .collect();
        insta::assert_debug_snapshot!(errors);
    }
}
