use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use console::style;

use crate::cli::{LintArgs, DEFAULT_DIR};
use crate::diff_handler::include_root_for;
use crate::error::GctlCliError;
use crate::images::{is_local, path_part};
use crate::markdown::{compile_markdown_with, Source};
use crate::push_handler::{display_name, find_content_files, slug_from_path};

#[derive(Debug, PartialEq, Eq)]
enum Severity {
    Error,
    Warning,
}

#[derive(Debug, PartialEq, Eq)]
struct Issue {
    severity: Severity,
    message: String,
}

fn error(message: impl Into<String>) -> Issue {
    Issue {
        severity: Severity::Error,
        message: message.into(),
    }
}

fn warning(message: impl Into<String>) -> Issue {
    Issue {
        severity: Severity::Warning,
        message: message.into(),
    }
}

/// What is wrong with one file, and the slug it would be pushed with (when it
/// could be worked out).
fn lint_file(file: &Path, include_root: &Path) -> (Vec<Issue>, Option<String>) {
    let content = match fs::read_to_string(file) {
        Ok(content) => content,
        Err(e) => return (vec![error(format!("cannot read the file: {e}"))], None),
    };
    let mut document = match gctl_markdown::parse(&content) {
        Ok(document) => document,
        Err(e) => return (vec![error(e.to_string())], None),
    };
    let mut issues = Vec::new();
    let slug = document
        .front_matter
        .slug
        .clone()
        .unwrap_or_else(|| slug_from_path(file));

    if document
        .front_matter
        .title
        .as_deref()
        .is_none_or(|t| t.trim().is_empty())
    {
        issues.push(error("front matter is missing a `title`"));
    }

    let dir = file.parent().unwrap_or(Path::new("."));
    let mut srcs = Vec::new();
    document.visit_images_mut(&mut |src| srcs.push(src.clone()));
    srcs.extend(document.front_matter.feature_image.clone());
    srcs.dedup();
    for src in srcs.iter().filter(|src| is_local(src)) {
        if !dir.join(path_part(src)).is_file() {
            issues.push(error(format!("image `{src}` not found")));
        }
    }

    let source = Source {
        file,
        root: include_root,
    };
    match compile_markdown_with(&content, Some(&source), &HashMap::new()) {
        Ok(compiled) => {
            if !compiled.lossy.is_empty() {
                issues.push(error(format!(
                    "pulled with approximations ({}); push refuses it until the `lossy:` line is removed",
                    compiled.lossy.join(", ")
                )));
            }
            let asked_for = document.front_matter.status.as_deref();
            if compiled.post.status.as_deref() == Some("scheduled")
                && asked_for != Some("scheduled")
            {
                issues.push(warning(format!(
                    "dated in the future ({}), so it will be scheduled, not published",
                    compiled.post.published_at.unwrap_or_default()
                )));
            }
        }
        Err(e) => issues.push(error(e.to_string())),
    }
    (issues, Some(slug))
}

pub fn handle_command(args: LintArgs) -> Result<(), GctlCliError> {
    let root = PathBuf::from(args.path.as_deref().unwrap_or(DEFAULT_DIR));
    let files = find_content_files(&root)?;
    if files.is_empty() {
        return Err(GctlCliError::Push(format!(
            "No markdown files found in {}",
            root.display()
        )));
    }
    let include_root = include_root_for(&root, args.include_root.as_deref());

    let mut slugs: HashMap<String, String> = HashMap::new();
    let (mut errors, mut warnings) = (0, 0);
    for file in &files {
        let name = display_name(&root, file);
        let (mut issues, slug) = lint_file(file, &include_root);
        if let Some(slug) = slug {
            match slugs.get(&slug) {
                Some(other) => issues.push(error(format!("slug `{slug}` is also used by {other}"))),
                None => {
                    slugs.insert(slug, name.clone());
                }
            }
        }
        for issue in issues {
            let label = match issue.severity {
                Severity::Error => {
                    errors += 1;
                    style("error").red()
                }
                Severity::Warning => {
                    warnings += 1;
                    style("warning").yellow()
                }
            };
            println!("{name}: {label}: {}", issue.message);
        }
    }

    if errors == 0 && warnings == 0 {
        println!("{} file(s) checked, no problems found.", files.len());
    } else {
        println!(
            "\n{} file(s) checked: {errors} error(s), {warnings} warning(s).",
            files.len()
        );
    }
    if errors > 0 {
        return Err(GctlCliError::Push(format!(
            "{errors} error(s) found in {} file(s)",
            files.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    fn lint(files: &[(&str, &str)], target: &str) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!(
            "gctl-lint-{}-{}",
            std::process::id(),
            target.replace('/', "_")
        ));
        let _ = fs::remove_dir_all(&dir);
        for (name, content) in files {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let dir = fs::canonicalize(&dir).unwrap();
        let (issues, _) = lint_file(&dir.join(target), &dir);
        fs::remove_dir_all(&dir).unwrap();
        issues
            .into_iter()
            .map(|i| {
                format!(
                    "{:?}: {}",
                    i.severity,
                    i.message.replace(&dir.display().to_string(), "<dir>")
                )
            })
            .collect()
    }

    #[test]
    fn reports_problems() {
        let cases: Vec<(&str, Vec<String>)> = vec![
            ("clean", lint(&[("a.md", "---\ntitle: A\n---\n\nHi\n")], "a.md")),
            ("no title", lint(&[("a.md", "---\nslug: a\n---\n")], "a.md")),
            ("bad yaml", lint(&[("a.md", "---\n: [\n---\n")], "a.md")),
            (
                "missing include",
                lint(
                    &[("a.md", "---\ntitle: A\n---\n\n{{< include \"_x.md\" >}}\n")],
                    "a.md",
                ),
            ),
            (
                "images",
                lint(
                    &[
                        ("img/ok.png", "x"),
                        (
                            "a.md",
                            "---\ntitle: A\nimage: nope.png\n---\n\n![](img/ok.png)\n\n![](img/gone.png)\n\n![](https://x.test/r.png)\n",
                        ),
                    ],
                    "a.md",
                ),
            ),
            (
                "contradiction",
                lint(&[("a.md", "---\ntitle: A\nstatus: draft\ndraft: false\n---\n")], "a.md"),
            ),
            (
                "future date is scheduled",
                lint(&[("a.md", "---\ntitle: A\nstatus: published\ndate: 2099-01-01\n---\n")], "a.md"),
            ),
            (
                "scheduled on purpose",
                lint(&[("a.md", "---\ntitle: A\nstatus: scheduled\ndate: 2099-01-01\n---\n")], "a.md"),
            ),
            (
                "lossy",
                lint(&[("a.md", "---\ntitle: A\nlossy: [callout]\n---\n")], "a.md"),
            ),
        ];
        insta::assert_debug_snapshot!(cases);
    }
}
