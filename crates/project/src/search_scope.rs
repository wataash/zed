//! Shared scope for file finding and project search. Additional roots remain
//! ordinary worktrees, so editing and Git status use the normal project model.
use std::path::{Path, PathBuf};

use anyhow::Result;
use gpui::{App, Context, Task};
use settings::{Settings, SettingsLocation};
use util::rel_path::RelPath;

use crate::{Project, ProjectSettings, WorktreeId};

#[derive(Clone, Default)]
pub struct SearchScope {
    pub expanded: bool,
    roots: Vec<PathBuf>,
}

impl SearchScope {
    pub fn new(project: &Project, cx: &App) -> Self {
        let mut roots = Vec::new();
        if project.is_local() {
            for worktree in project.visible_worktrees(cx) {
                let worktree = worktree.read(cx);
                let settings = ProjectSettings::get(
                    Some(SettingsLocation {
                        worktree_id: worktree.id(),
                        path: RelPath::empty(),
                    }),
                    cx,
                );
                for path in &settings.additional_search_roots {
                    let path = if let Some(rest) = path.strip_prefix("~/") {
                        util::paths::home_dir().join(rest)
                    } else {
                        worktree.abs_path().join(path)
                    };
                    let path = util::paths::normalize_path(&path);
                    if !roots.contains(&path) && path != worktree.abs_path().as_ref() {
                        roots.push(path);
                    }
                }
            }
        }
        Self {
            expanded: false,
            roots,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.roots.is_empty()
    }

    pub fn includes_path(&self, path: &Path) -> bool {
        self.expanded || !self.roots.iter().any(|root| path.starts_with(root))
    }

    pub fn excluded_worktrees(&self, project: &Project, cx: &App) -> Vec<WorktreeId> {
        project
            .worktrees(cx)
            .filter_map(|worktree| {
                let worktree = worktree.read(cx);
                (!self.includes_path(worktree.abs_path().as_ref())).then_some(worktree.id())
            })
            .collect()
    }

    pub fn label(&self, project: &Project, cx: &App) -> String {
        project
            .visible_worktrees(cx)
            .filter_map(|worktree| {
                let worktree = worktree.read(cx);
                self.includes_path(worktree.abs_path().as_ref())
                    .then(|| worktree.root_name().as_unix_str().to_string())
            })
            .collect::<Vec<_>>()
            .join(" + ")
    }

    pub fn prepare(&self, project: &mut Project, cx: &mut Context<Project>) -> Task<Result<()>> {
        let roots = self.roots.clone();
        let fs = project.fs().clone();
        cx.spawn(async move |project, cx| {
            for path in roots {
                anyhow::ensure!(
                    fs.is_dir(&path).await,
                    "Additional search root is not a directory: {}",
                    path.display()
                );
                project
                    .update(cx, |project, cx| {
                        project.find_or_create_worktree(&path, true, cx)
                    })?
                    .await?;
            }
            Ok(())
        })
    }
}
