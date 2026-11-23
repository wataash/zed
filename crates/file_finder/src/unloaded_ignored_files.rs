//! Walks ignored directories that worktrees have not loaded, for `zhack.search.all_files`.

use std::{path::Path, sync::Arc};

use futures::StreamExt as _;
use fuzzy_nucleo::PathMatchCandidate;
use gpui::{App, AppContext as _, Entity, Task};
use project::{Fs, Project, WorktreeId, WorktreeSettings};
use settings::{Settings as _, SettingsLocation};
use util::{
    paths::{PathMatcher, PathStyle},
    rel_path::RelPath,
};

/// Stops the walk so a huge ignored tree cannot exhaust memory.
const MAX_WALKED_ENTRIES: usize = 200_000;

pub(crate) struct UnloadedIgnoredFiles {
    pub(crate) worktree_id: WorktreeId,
    pub(crate) files: Vec<Arc<RelPath>>,
    roots: Vec<Arc<RelPath>>,
}

impl UnloadedIgnoredFiles {
    pub(crate) fn contains_path_or_descendant(&self, path: &RelPath) -> bool {
        // RelPath orders by component, so descendants immediately follow their parent.
        let index = self.roots.partition_point(|root| root.as_ref() < path);
        self.roots
            .get(index)
            .is_some_and(|root| root.starts_with(path))
            || path.ancestors().any(|ancestor| {
                self.roots
                    .binary_search_by(|root| root.as_ref().cmp(ancestor))
                    .is_ok()
            })
    }
}

struct WorktreeWalk {
    abs_path: Arc<Path>,
    path_style: PathStyle,
    scan_exclusions: WorktreeSettings,
    unloaded_dirs: Vec<Arc<RelPath>>,
}

pub(crate) fn is_excluded(exclusions: &PathMatcher, path: &RelPath) -> bool {
    path.ancestors()
        .any(|ancestor| exclusions.is_match(ancestor))
}

pub(crate) fn walk_unloaded_ignored_files(
    project: &Entity<Project>,
    exclusions: PathMatcher,
    scope: &project::search_scope::SearchScope,
    cx: &App,
) -> Task<Vec<Arc<UnloadedIgnoredFiles>>> {
    let project = project.read(cx);
    let fs = project.fs().clone();
    let snapshots = project
        .visible_worktrees(cx)
        .filter_map(|worktree| {
            let worktree = worktree.read(cx);
            if worktree.is_single_file()
                || !worktree.is_local()
                || !scope.includes_path(worktree.abs_path().as_ref())
            {
                return None;
            }
            let scan_exclusions = WorktreeSettings::get(
                Some(SettingsLocation {
                    worktree_id: worktree.id(),
                    path: RelPath::empty(),
                }),
                cx,
            )
            .clone();
            Some((worktree.snapshot(), scan_exclusions))
        })
        .collect::<Vec<_>>();

    cx.background_spawn(async move {
        let mut remaining = MAX_WALKED_ENTRIES;
        let mut results = Vec::new();
        for (snapshot, scan_exclusions) in snapshots {
            let mut unloaded_dirs = snapshot
                .entries(true, 0)
                .filter(|entry| entry.kind == project::EntryKind::UnloadedDir)
                // Apply the same symlink boundary to walk roots as to their children.
                .filter(|entry| !entry.is_external && entry.canonical_path.is_none())
                .filter(|entry| !is_excluded(&exclusions, &entry.path))
                .take(remaining)
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            if unloaded_dirs.is_empty() {
                continue;
            }
            unloaded_dirs.sort_unstable();
            let walk = WorktreeWalk {
                abs_path: snapshot.abs_path().clone(),
                path_style: snapshot.path_style(),
                scan_exclusions,
                unloaded_dirs: unloaded_dirs.clone(),
            };
            let files = walk_worktree(fs.as_ref(), walk, &exclusions, &mut remaining).await;
            results.push(Arc::new(UnloadedIgnoredFiles {
                worktree_id: snapshot.id(),
                files,
                roots: unloaded_dirs,
            }));
            if remaining == 0 {
                log::warn!(
                    "Stopped walking ignored directories for the file finder after {MAX_WALKED_ENTRIES} entries"
                );
                break;
            }
        }
        results
    })
}

async fn walk_worktree(
    fs: &dyn Fs,
    walk: WorktreeWalk,
    exclusions: &PathMatcher,
    remaining: &mut usize,
) -> Vec<Arc<RelPath>> {
    let mut files = Vec::new();
    let mut pending_dirs = walk.unloaded_dirs;
    while *remaining > 0
        && let Some(dir) = pending_dirs.pop()
    {
        *remaining -= 1;
        let abs_dir = walk.abs_path.join(dir.as_std_path());
        let mut children = match fs.read_dir(&abs_dir).await {
            Ok(children) => children,
            Err(error) => {
                log::debug!("Failed to read {abs_dir:?} for the file finder: {error:#}");
                continue;
            }
        };
        while *remaining > 0
            && let Some(child_abs_path) = children.next().await
        {
            *remaining -= 1;
            let child_abs_path = match child_abs_path {
                Ok(child_abs_path) => child_abs_path,
                Err(error) => {
                    log::debug!("Failed to read an entry of {abs_dir:?}: {error:#}");
                    continue;
                }
            };
            let Some(file_name) = child_abs_path.file_name() else {
                continue;
            };
            let Ok(file_name) = RelPath::new(Path::new(file_name), walk.path_style) else {
                continue;
            };
            let child_path: Arc<RelPath> = dir.join(&file_name).into();
            if exclusions.is_match(&child_path)
                || walk.scan_exclusions.is_path_excluded(&child_path)
            {
                continue;
            }
            let metadata = match fs.metadata(&child_abs_path).await {
                Ok(Some(metadata)) => metadata,
                Ok(None) => continue,
                Err(error) => {
                    log::debug!("Failed to stat {child_abs_path:?}: {error:#}");
                    continue;
                }
            };
            if metadata.is_dir {
                // Symlinked directories can form cycles.
                if !metadata.is_symlink {
                    pending_dirs.push(child_path);
                }
            } else {
                files.push(child_path);
            }
        }
    }
    files
}

pub(crate) struct UnloadedIgnoredCandidateSet {
    pub(crate) files: Arc<UnloadedIgnoredFiles>,
    pub(crate) prefix: Arc<RelPath>,
    pub(crate) path_style: PathStyle,
}

pub(crate) struct UnloadedIgnoredCandidates<'a> {
    files: std::slice::Iter<'a, Arc<RelPath>>,
    prefix: &'a RelPath,
}

impl<'a> Iterator for UnloadedIgnoredCandidates<'a> {
    type Item = PathMatchCandidate<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.files
            .next()
            .map(|path| PathMatchCandidate::new(path, false, Some(self.prefix)))
    }
}

impl<'a> fuzzy_nucleo::PathMatchCandidateSet<'a> for UnloadedIgnoredCandidateSet {
    type Candidates = UnloadedIgnoredCandidates<'a>;

    fn id(&self) -> usize {
        self.files.worktree_id.to_usize()
    }

    fn len(&self) -> usize {
        self.files.files.len()
    }

    fn root_is_file(&self) -> bool {
        false
    }

    fn prefix(&self) -> Arc<RelPath> {
        self.prefix.clone()
    }

    fn candidates(&'a self, start: usize) -> Self::Candidates {
        UnloadedIgnoredCandidates {
            files: self.files.files.get(start..).unwrap_or_default().iter(),
            prefix: &self.prefix,
        }
    }

    fn path_style(&self) -> PathStyle {
        self.path_style
    }
}

#[cfg(test)]
#[gpui::test]
async fn directory_only_walk_exhausts_budget(cx: &mut gpui::TestAppContext) {
    let app = crate::file_finder_tests::init_test(cx);
    let fs = app.fs.as_fake();
    let root = Path::new(util::path!("/root"));
    fs.insert_tree(
        root,
        serde_json::json!({"a": {}, "b": {}, "c": {}, "d": {}}),
    )
    .await;
    let walk = WorktreeWalk {
        abs_path: root.into(),
        path_style: PathStyle::local(),
        scan_exclusions: cx.read(|cx| WorktreeSettings::get_global(cx).clone()),
        unloaded_dirs: vec![RelPath::empty_arc()],
    };
    let exclusions = PathMatcher::new(&[] as &[&str], PathStyle::local()).unwrap();
    let calls = fs.read_dir_call_count();
    let mut remaining = 3;
    let files = walk_worktree(fs.as_ref(), walk, &exclusions, &mut remaining).await;
    assert!(files.is_empty());
    assert_eq!(remaining, 0);
    assert_eq!(fs.read_dir_call_count() - calls, 1);
}
