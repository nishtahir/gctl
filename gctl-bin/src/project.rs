use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli::GctlCommands;
use crate::error::GctlCliError;

/// Name of the project file, looked up in the working directory and its parents.
pub const PROJECT_FILE: &str = "gctl.toml";

/// Defaults for a project, so they don't have to be repeated on every command.
/// Paths are relative to the file. The command line always wins.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    /// Named site to use, as saved with `gctl auth login --site`.
    pub site: Option<String>,
    /// Directory of markdown files (default: `content`).
    pub path: Option<String>,
    /// Directory `{{< include >}}` may read from.
    pub include_root: Option<String>,
}

impl ProjectConfig {
    /// The nearest `gctl.toml` from `start` upward, and the directory it is in.
    pub fn find(start: &Path) -> Result<Option<(PathBuf, ProjectConfig)>, GctlCliError> {
        for dir in start.ancestors() {
            let file = dir.join(PROJECT_FILE);
            if !file.is_file() {
                continue;
            }
            let text = fs::read_to_string(&file)?;
            let config = toml::from_str(&text).map_err(|e| {
                GctlCliError::Config(format!("{}: {}", file.display(), e.message()))
            })?;
            return Ok(Some((dir.to_path_buf(), config)));
        }
        Ok(None)
    }

    /// Fills in what `command` leaves out. `dir` is where the file is.
    pub fn apply(&self, dir: &Path, command: &mut GctlCommands) {
        let resolve = |value: &Option<String>| {
            value
                .as_ref()
                .map(|v| dir.join(v).to_string_lossy().into_owned())
        };
        let (path, include_root) = match command {
            GctlCommands::Push(args) => (&mut args.path, Some(&mut args.include_root)),
            GctlCommands::Diff(args) => (&mut args.path, Some(&mut args.include_root)),
            GctlCommands::Lint(args) => (&mut args.path, Some(&mut args.include_root)),
            GctlCommands::Status(args) => (&mut args.path, Some(&mut args.include_root)),
            GctlCommands::Pull(args) => (&mut args.path, None),
            GctlCommands::New(args) => (&mut args.path, None),
            _ => return,
        };
        if path.is_none() {
            *path = resolve(&self.path);
        }
        if let Some(include_root) = include_root {
            if include_root.is_none() {
                *include_root = resolve(&self.include_root);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cli::{GctlCli, GctlCommands};
    use clap::Parser;

    fn parse(args: &[&str]) -> GctlCommands {
        GctlCli::try_parse_from(args).unwrap().command
    }

    #[test]
    fn fills_in_what_the_command_leaves_out() {
        let config = ProjectConfig {
            site: None,
            path: Some("posts".into()),
            include_root: Some("shared".into()),
        };
        let dir = Path::new("/project");
        let outcome = |args: &[&str]| {
            let mut command = parse(args);
            config.apply(dir, &mut command);
            match command {
                GctlCommands::Push(a) => (a.path, a.include_root),
                GctlCommands::Pull(a) => (a.path, None),
                _ => unreachable!(),
            }
        };
        let results = [
            outcome(&["gctl", "push"]),
            outcome(&["gctl", "push", "other"]),
            outcome(&["gctl", "push", "--include-root", "mine"]),
            outcome(&["gctl", "pull"]),
        ];
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn reads_the_nearest_file_and_rejects_typos() {
        let root = std::env::temp_dir().join(format!("gctl-project-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(
            root.join(PROJECT_FILE),
            "site = \"prod\"\npath = \"posts\"\n",
        )
        .unwrap();

        let found = ProjectConfig::find(&root.join("a/b")).unwrap().unwrap();
        let nothing = ProjectConfig::find(Path::new("/")).unwrap();

        fs::write(root.join(PROJECT_FILE), "sight = \"prod\"\n").unwrap();
        let typo = ProjectConfig::find(&root.join("a"))
            .unwrap_err()
            .to_string();
        let typo = typo.replace(&root.display().to_string(), "<dir>");
        let relative = (found.0 == root, found.1);
        fs::remove_dir_all(&root).unwrap();

        insta::assert_debug_snapshot!((relative, nothing, typo));
    }
}
