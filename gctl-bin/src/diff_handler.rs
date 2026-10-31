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
enum Comparison {
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
        let include_root = match &args.include_root {
            Some(dir) => PathBuf::from(dir),
            None if root.is_dir() => root.clone(),
            None => root
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
        };

        info!("Comparing {} file(s)...", files.len());
        let (mut differing, mut failures) = (0, 0);
        for file in &files {
            let name = display_name(&root, file);
            match self.compare_file(file, &include_root).await {
                Ok(Comparison::Same) => {}
                Ok(Comparison::Differs(diff)) => {
                    differing += 1;
                    println!("{} {}", style("modified:").yellow(), name);
                    println!("{}\n", colored(&diff));
                }
                Ok(Comparison::New) => {
                    differing += 1;
                    println!("{} {}\n", style("new:").green(), name);
                }
                Ok(Comparison::Skipped(reason)) => {
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

    async fn compare_file(
        &self,
        file: &Path,
        include_root: &Path,
    ) -> Result<Comparison, GctlCliError> {
        let content = fs::read_to_string(file)?;
        let front_matter = gctl_markdown::parse(&content)?.front_matter;
        let post = match front_matter.id {
            Some(id) => Some(self.api.posts().read(&id).await?),
            None => {
                let slug = front_matter.slug.unwrap_or_else(|| slug_from_path(file));
                let filter = PostFilter {
                    slug: Some(slug),
                    limit: Some(1),
                    ..PostFilter::default()
                };
                self.api.posts().browse(&filter).await?.into_iter().next()
            }
        };
        let Some(post) = post else {
            return Ok(Comparison::New);
        };
        let source = Source {
            file,
            root: include_root,
        };
        compare(&content, &source, &post)
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
