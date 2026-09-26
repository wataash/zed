//! Walks ignored directories that worktrees have not loaded, for `include_ignored: "zall"`.

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
const MAX_WALKED_FILES: usize = 200_000;

pub(crate) struct UnloadedIgnoredFiles {
    pub(crate) worktree_id: WorktreeId,
    pub(crate) files: Vec<Arc<RelPath>>,
}

struct WorktreeWalk {
    worktree_id: WorktreeId,
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
    cx: &App,
) -> Task<Vec<Arc<UnloadedIgnoredFiles>>> {
    let project = project.read(cx);
    let fs = project.fs().clone();
    let walks = project
        .visible_worktrees(cx)
        .filter_map(|worktree| {
            let worktree = worktree.read(cx);
            if worktree.is_single_file() || !worktree.is_local() {
                return None;
            }
            let snapshot = worktree.snapshot();
            let unloaded_dirs = snapshot
                .entries(true, 0)
                .filter(|entry| entry.kind == project::EntryKind::UnloadedDir)
                .filter(|entry| !is_excluded(&exclusions, &entry.path))
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>();
            if unloaded_dirs.is_empty() {
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
            Some(WorktreeWalk {
                worktree_id: worktree.id(),
                abs_path: worktree.abs_path(),
                path_style: snapshot.path_style(),
                scan_exclusions,
                unloaded_dirs,
            })
        })
        .collect::<Vec<_>>();

    cx.background_spawn(async move {
        let mut remaining = MAX_WALKED_FILES;
        let mut results = Vec::new();
        for walk in walks {
            let files = walk_worktree(fs.as_ref(), &walk, &exclusions, &mut remaining).await;
            results.push(Arc::new(UnloadedIgnoredFiles {
                worktree_id: walk.worktree_id,
                files,
            }));
            if remaining == 0 {
                log::warn!(
                    "Stopped walking ignored directories for the file finder after {MAX_WALKED_FILES} files"
                );
                break;
            }
        }
        results
    })
}

async fn walk_worktree(
    fs: &dyn Fs,
    walk: &WorktreeWalk,
    exclusions: &PathMatcher,
    remaining: &mut usize,
) -> Vec<Arc<RelPath>> {
    let mut files = Vec::new();
    let mut pending_dirs = walk.unloaded_dirs.clone();
    while let Some(dir) = pending_dirs.pop() {
        let abs_dir = walk.abs_path.join(dir.as_std_path());
        let mut children = match fs.read_dir(&abs_dir).await {
            Ok(children) => children,
            Err(error) => {
                log::debug!("Failed to read {abs_dir:?} for the file finder: {error:#}");
                continue;
            }
        };
        while let Some(child_abs_path) = children.next().await {
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
                *remaining -= 1;
                if *remaining == 0 {
                    return files;
                }
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
