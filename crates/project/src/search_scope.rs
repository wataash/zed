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
    primary_roots: Vec<PathBuf>,
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
        // Additional roots remain attached between searches. Only roots not
        // explicitly configured as additional are primary workspace roots.
        let primary_roots = project
            .visible_worktrees(cx)
            .map(|tree| tree.read(cx).abs_path().to_path_buf())
            .filter(|path| !roots.contains(path))
            .collect();
        Self {
            expanded: false,
            roots,
            primary_roots,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.roots.is_empty()
    }

    pub fn includes_path(&self, path: &Path) -> bool {
        self.expanded
            || self.primary_roots.iter().any(|root| path.starts_with(root))
            || !self.roots.iter().any(|root| path.starts_with(root))
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
            let mut errors = Vec::new();
            for path in roots {
                if !fs.is_dir(&path).await {
                    errors.push(format!(
                        "Additional search root is not a directory: {}",
                        path.display()
                    ));
                    continue;
                }
                let task = project.update(cx, |project, cx| {
                    project.find_or_create_worktree(&path, true, cx)
                })?;
                if let Err(error) = task.await {
                    errors.push(format!(
                        "Opening additional search root {}: {error:#}",
                        path.display()
                    ));
                }
            }
            anyhow::ensure!(errors.is_empty(), "{}", errors.join("\n"));
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    async fn prepares_available_roots_when_others_are_missing(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let settings = settings::SettingsStore::test(cx);
            cx.set_global(settings);
        });
        let fs = fs::FakeFs::new(cx.executor());
        let available = Path::new(util::path!("/archive"));
        fs.insert_tree(available, serde_json::json!({})).await;
        let project = Project::test(fs, [], cx).await;
        let missing = Path::new(util::path!("/missing"));
        let other_missing = Path::new(util::path!("/also-missing"));
        let scope = SearchScope {
            roots: vec![missing.into(), available.into(), other_missing.into()],
            ..Default::default()
        };
        let error = project
            .update(cx, |project, cx| scope.prepare(project, cx))
            .await
            .expect_err("unavailable roots should be reported")
            .to_string();
        assert!(error.contains(missing.to_str().unwrap()));
        assert!(error.contains(other_missing.to_str().unwrap()));
        project.read_with(cx, |project, cx| {
            assert!(
                project
                    .visible_worktrees(cx)
                    .any(|tree| tree.read(cx).abs_path().as_ref() == available)
            );
        });
    }

    #[test]
    fn ancestor_additional_root_preserves_primary_paths() {
        let mut scope = SearchScope {
            expanded: false,
            roots: vec!["/notes".into()],
            primary_roots: vec!["/notes/current".into()],
        };
        assert!(scope.includes_path(Path::new("/notes/current")));
        assert!(scope.includes_path(Path::new("/notes/current/memo.md")));
        assert!(!scope.includes_path(Path::new("/notes")));
        assert!(!scope.includes_path(Path::new("/notes/archive/old.md")));
        scope.expanded = true;
        assert!(scope.includes_path(Path::new("/notes/archive/old.md")));
    }
}
