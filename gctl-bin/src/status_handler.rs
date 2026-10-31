use std::collections::HashSet;
use std::path::PathBuf;

use bon::Builder;
use gctl_ghost::{GhostContentApi, PostFilter};
use tracing::info;

use crate::cli::StatusArgs;
use crate::diff_handler::{compare_file, include_root_for, Compared, Comparison};
use crate::error::GctlCliError;
use crate::output::{format_sync_results, SyncRow};
use crate::push_handler::{display_name, find_content_files};

#[derive(Builder)]
pub struct StatusCommandHandler<'a> {
    api: &'a GhostContentApi<'a>,
}

fn row(file: String, result: &str, status: String, detail: String) -> SyncRow {
    SyncRow {
        file,
        result: result.into(),
        status,
        detail,
    }
}

impl<'a> StatusCommandHandler<'a> {
    pub async fn handle_command(&self, args: StatusArgs) -> Result<(), GctlCliError> {
        let root = PathBuf::from(&args.path);
        let files = find_content_files(&root)?;
        let include_root = include_root_for(&root, args.include_root.as_deref());

        info!("Comparing {} file(s)...", files.len());
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        let mut failures = 0;
        for file in &files {
            let name = display_name(&root, file);
            match compare_file(self.api, file, &include_root).await {
                Ok(Compared {
                    comparison,
                    post_id,
                    status,
                }) => {
                    seen.extend(post_id);
                    rows.push(match comparison {
                        Comparison::Same => row(name, "in sync", status, String::new()),
                        Comparison::Differs(_) => row(name, "modified", status, String::new()),
                        Comparison::New => row(name, "new", String::new(), String::new()),
                        Comparison::Skipped(reason) => row(name, "skipped", status, reason),
                    });
                }
                Err(err) => {
                    failures += 1;
                    rows.push(row(name, "failed", String::new(), err.to_string()));
                }
            }
        }

        // Only a complete picture of the local files can say what is remote only.
        if !args.no_remote && failures == 0 && root.is_dir() {
            info!("Fetching posts...");
            let mut remote = self.api.posts().browse(&PostFilter::default()).await?;
            remote.extend(self.api.pages().browse(&PostFilter::default()).await?);
            for post in remote {
                if post.id.as_ref().is_some_and(|id| seen.contains(id)) {
                    continue;
                }
                rows.push(row(
                    post.slug.or(post.id).unwrap_or_default(),
                    "remote only",
                    post.status.unwrap_or_default(),
                    post.title.unwrap_or_default(),
                ));
            }
        }

        if rows.is_empty() && !args.json {
            println!("Nothing to compare.");
        } else {
            format_sync_results(rows, args.json)?;
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
