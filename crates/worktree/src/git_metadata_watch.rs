use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use collections::HashSet;
use fs::Fs;

/// Directories needed for HEAD/index changes and the current symbolic ref chain.
/// Watching parents survives Git's atomic replacement of individual files.
pub(super) async fn directories(
    fs: &dyn Fs,
    directory: &Path,
    common: &Path,
) -> Result<HashSet<PathBuf>> {
    let mut paths = HashSet::from_iter([directory.to_path_buf(), common.to_path_buf()]);
    for root in [directory, common] {
        let reftable = root.join("reftable");
        if fs.is_dir(&reftable).await {
            paths.insert(reftable);
        }
    }
    let mut head = directory.join("HEAD");
    for _ in 0..16 {
        if !fs.is_file(&head).await {
            break;
        }
        let contents = fs.load(&head).await?;
        let Some(reference) = contents.trim().strip_prefix("ref: ") else {
            break;
        };
        let reference = Path::new(reference);
        if !reference.starts_with("refs")
            || reference
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            break;
        }
        // These namespaces are private to a linked worktree; other refs are shared.
        let root = if ["refs/bisect", "refs/worktree", "refs/rewritten"]
            .iter()
            .any(|prefix| reference.starts_with(prefix))
        {
            directory
        } else {
            common
        };
        head = root.join(reference);
        // An unborn branch or a newly created namespace may not have a parent
        // directory yet. Watch its nearest existing ancestor until it appears.
        if let Some(mut parent) = head.parent() {
            while parent != root && !fs.is_dir(parent).await {
                let Some(ancestor) = parent.parent() else {
                    break;
                };
                parent = ancestor;
            }
            paths.insert(parent.to_path_buf());
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs::FakeFs;
    use gpui::TestAppContext;
    use serde_json::json;

    #[gpui::test]
    async fn test_minimal_git_watch_directories(cx: &mut TestAppContext) {
        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree("/git", json!({"HEAD": "ref: refs/heads/main\n", "refs": {"heads": {"main": "commit"}}, "objects": {}, "logs": {}})).await;
        let root = Path::new("/git");
        assert_eq!(
            directories(fs.as_ref(), root, root)
                .await
                .expect("watch directories"),
            HashSet::from_iter([PathBuf::from("/git"), PathBuf::from("/git/refs/heads")])
        );
        fs.atomic_write(root.join("HEAD"), "ref: refs/heads/topic/new\n".into())
            .await
            .expect("HEAD");
        assert_eq!(
            directories(fs.as_ref(), root, root)
                .await
                .expect("unborn branch"),
            HashSet::from_iter([PathBuf::from("/git"), PathBuf::from("/git/refs/heads")])
        );
        fs.atomic_write(root.join("refs/heads/topic/new"), "commit".into())
            .await
            .expect("ref");
        assert_eq!(
            directories(fs.as_ref(), root, root)
                .await
                .expect("nested branch"),
            HashSet::from_iter([
                PathBuf::from("/git"),
                PathBuf::from("/git/refs/heads/topic")
            ])
        );
        fs.atomic_write(root.join("HEAD"), "detached commit".into())
            .await
            .expect("HEAD");
        assert_eq!(
            directories(fs.as_ref(), root, root)
                .await
                .expect("detached HEAD"),
            HashSet::from_iter([root.to_path_buf()])
        );
    }
    #[gpui::test]
    async fn test_linked_worktree_and_reftable(cx: &mut TestAppContext) {
        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            "/git",
            json!({
                "worktrees": {"linked": {"HEAD": "ref: refs/heads/main\n"}},
                "refs": {"heads": {"main": "commit"}}, "reftable": {}
            }),
        )
        .await;
        assert_eq!(
            directories(
                fs.as_ref(),
                Path::new("/git/worktrees/linked"),
                Path::new("/git")
            )
            .await
            .expect("linked worktree"),
            HashSet::from_iter([
                PathBuf::from("/git/worktrees/linked"),
                PathBuf::from("/git"),
                PathBuf::from("/git/refs/heads"),
                PathBuf::from("/git/reftable"),
            ])
        );
    }
}
