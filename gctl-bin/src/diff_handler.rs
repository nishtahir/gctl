use std::fs;
use std::path::{Path, PathBuf};

use bon::Builder;
use console::style;
use gctl_ghost::{GhostContentApi, Post, PostFilter};
use similar::TextDiff;
use tracing::info;

use crate::cli::DiffArgs;
use crate::error::GctlCliError;
use crate::markdown::Source;
use crate::pull_handler::{decide, render_post, Action};
use crate::push_handler::{display_name, find_content_files, slug_from_path};

#[derive(Builder)]
pub struct DiffCommandHandler<'a> {
    api: &'a GhostContentApi<'a>,
}

/// How one local file compares with its post.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Comparison {
    Same,
    Differs(String),
    /// No post on Ghost for this file; a push would create one.
    New,
    /// The post can't be rendered as markdown.
    Skipped(String),
}

/// Compares `local` with the post as `gctl pull` would write it.
fn compare(local: &str, source: &Source, post: &Post) -> Result<Comparison, GctlCliError> {
    let Some(rendered) = render_post(post)? else {
        return Ok(Comparison::Skipped("no Lexical content on Ghost".into()));
    };
    if decide(Some(local), Some(source), &rendered.file, false) == Action::Unchanged {
        return Ok(Comparison::Same);
    }
    Ok(Comparison::Differs(unified_diff(&rendered.file, local)))
}

/// A unified diff from `ghost` to `local`: `+` lines are what a push adds.
fn unified_diff(ghost: &str, local: &str) -> String {
    TextDiff::from_lines(ghost, local)
        .unified_diff()
        .context_radius(3)
        .header("ghost", "local")
        .to_string()
}

fn colored(diff: &str) -> String {
    diff.lines()
        .map(|line| match line.chars().next() {
            Some('+') if !line.starts_with("+++") => style(line).green().to_string(),
            Some('-') if !line.starts_with("---") => style(line).red().to_string(),
            Some('@') => style(line).cyan().to_string(),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A local file's comparison, and the id of the post it maps to.
pub(crate) struct Compared {
    pub comparison: Comparison,
    pub post_id: Option<String>,
    pub status: String,
}

/// Where `{{< include >}}` may read from: the override, else the directory
/// being compared (or the folder of a single file).
pub(crate) fn include_root_for(root: &Path, include_root: Option<&str>) -> PathBuf {
    match include_root {
        Some(dir) => PathBuf::from(dir),
        None if root.is_dir() => root.to_path_buf(),
        None => root
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
    }
}

/// Finds the post a file maps to (by id, else slug) and compares them.
pub(crate) async fn compare_file(
    api: &GhostContentApi<'_>,
    file: &Path,
    include_root: &Path,
) -> Result<Compared, GctlCliError> {
    let content = fs::read_to_string(file)?;
    let front_matter = gctl_markdown::parse(&content)?.front_matter;
    let post = match front_matter.id {
        Some(id) => Some(api.posts().read(&id).await?),
        None => {
            let slug = front_matter.slug.unwrap_or_else(|| slug_from_path(file));
            let filter = PostFilter {
                slug: Some(slug),
                limit: Some(1),
                ..PostFilter::default()
            };
            api.posts().browse(&filter).await?.into_iter().next()
        }
    };
    let Some(post) = post else {
        return Ok(Compared {
            comparison: Comparison::New,
            post_id: None,
            status: String::new(),
        });
    };
    let source = Source {
        file,
        root: include_root,
    };
    Ok(Compared {
        comparison: compare(&content, &source, &post)?,
        post_id: post.id.clone(),
        status: post.status.clone().unwrap_or_default(),
    })
}

impl<'a> DiffCommandHandler<'a> {
    pub async fn handle_command(&self, args: DiffArgs) -> Result<(), GctlCliError> {
        let root = PathBuf::from(&args.path);
        let files = find_content_files(&root)?;
        if files.is_empty() {
            return Err(GctlCliError::Push(format!(
                "No markdown files found in {}",
                root.display()
            )));
        }
        let include_root = include_root_for(&root, args.include_root.as_deref());

        info!("Comparing {} file(s)...", files.len());
        let (mut differing, mut failures) = (0, 0);
        for file in &files {
            let name = display_name(&root, file);
            match compare_file(self.api, file, &include_root).await {
                Ok(Compared {
                    comparison: Comparison::Same,
                    ..
                }) => {}
                Ok(Compared {
                    comparison: Comparison::Differs(diff),
                    ..
                }) => {
                    differing += 1;
                    println!("{} {}", style("modified:").yellow(), name);
                    println!("{}\n", colored(&diff));
                }
                Ok(Compared {
                    comparison: Comparison::New,
                    ..
                }) => {
                    differing += 1;
                    println!("{} {}\n", style("new:").green(), name);
                }
                Ok(Compared {
                    comparison: Comparison::Skipped(reason),
                    ..
                }) => {
                    println!("{} {} ({reason})\n", style("skipped:").dim(), name);
                }
                Err(err) => {
                    failures += 1;
                    println!("{} {}: {err}\n", style("failed:").red(), name);
                }
            }
        }

        if differing == 0 && failures == 0 {
            println!("Everything is in sync with Ghost.");
        }
        if failures > 0 {
            return Err(GctlCliError::Push(format!(
                "{failures} of {} file(s) could not be compared",
                files.len()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn post(text: &str) -> Post {
        Post {
            id: Some("abc".into()),
            title: Some("Hello".into()),
            slug: Some("hello".into()),
            status: Some("draft".into()),
            lexical: Some(
                gctl_markdown::parse(&format!("---\ntitle: Hello\n---\n\n{text}\n"))
                    .unwrap()
                    .to_lexical_string(),
            ),
            ..Post::default()
        }
    }

    #[test]
    fn diffs_are_from_ghost_to_local() {
        let dir = Path::new(".");
        let file = dir.join("post.md");
        let source = Source {
            file: &file,
            root: dir,
        };
        let remote = post("Hello world.");
        let rendered = render_post(&remote).unwrap().unwrap().file;

        let same = compare(&rendered, &source, &remote).unwrap();
        let edited = compare(&rendered.replace("world", "there"), &source, &remote).unwrap();
        let no_lexical = Post {
            lexical: None,
            ..post("Hello world.")
        };
        let skipped = compare(&rendered, &source, &no_lexical).unwrap();

        insta::assert_debug_snapshot!((same, edited, skipped));
    }
}
