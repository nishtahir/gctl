use std::fs;
use std::path::{Path, PathBuf};

use console::style;
use serde::Serialize;

use crate::cli::{NewArgs, DEFAULT_DIR};
use crate::error::GctlCliError;
use crate::push_handler::slugify;

#[derive(Serialize)]
struct NewFrontMatter<'a> {
    title: &'a str,
    status: &'a str,
}

/// The text of a new draft: front matter and nothing else.
fn render(title: &str) -> Result<String, GctlCliError> {
    let yaml = serde_yaml::to_string(&NewFrontMatter {
        title,
        status: "draft",
    })
    .map_err(|e| GctlCliError::Parse(e.to_string()))?;
    Ok(format!("---\n{yaml}---\n\n"))
}

/// Writes `<dir>/<slug>.md` and returns its path. The slug is the file name,
/// which is also where `gctl push` takes it from.
fn create(dir: &Path, title: &str, slug: Option<&str>) -> Result<PathBuf, GctlCliError> {
    if title.trim().is_empty() {
        return Err(GctlCliError::Parse("the title can't be empty".into()));
    }
    let slug = slugify(slug.unwrap_or(title));
    if slug.is_empty() {
        return Err(GctlCliError::Parse(
            "no slug could be made from the title; pass one with --slug".into(),
        ));
    }
    let path = dir.join(format!("{slug}.md"));
    if path.exists() {
        return Err(GctlCliError::Push(format!(
            "{} already exists",
            path.display()
        )));
    }
    fs::create_dir_all(dir)?;
    fs::write(&path, render(title.trim())?)?;
    Ok(path)
}

pub fn handle_command(args: NewArgs) -> Result<(), GctlCliError> {
    let path = create(
        Path::new(args.path.as_deref().unwrap_or(DEFAULT_DIR)),
        &args.title,
        args.slug.as_deref(),
    )?;
    println!("{} {}", style("Created").green(), path.display());
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gctl-new-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn creates_a_draft_named_after_the_title() {
        let dir = temp_dir("create");
        let path = create(&dir, "Hello: World", None).unwrap();
        let file = fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        fs::remove_dir_all(&dir).unwrap();
        insta::assert_debug_snapshot!((name, file));
    }

    #[test]
    fn refuses_to_overwrite_and_to_guess_a_slug() {
        let dir = temp_dir("refuse");
        create(&dir, "Hello", None).unwrap();
        let results: Vec<String> = [
            create(&dir, "Hello", None),
            create(&dir, "  ", None),
            create(&dir, "日本語", None),
            create(&dir, "日本語", Some("nihongo")),
        ]
        .into_iter()
        .map(|r| match r {
            Ok(path) => format!("ok {}", path.file_name().unwrap().to_string_lossy()),
            Err(e) => e.to_string().replace(&dir.display().to_string(), "<dir>"),
        })
        .collect();
        fs::remove_dir_all(&dir).unwrap();
        insta::assert_debug_snapshot!(results);
    }
}
