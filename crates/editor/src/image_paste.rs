// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
use super::*;
use futures::{StreamExt as _, channel::mpsc};

#[derive(Default)]
struct ImagePasteHistories(HashMap<gpui::EntityId, Vec<ImagePasteHistory>>);
impl Global for ImagePasteHistories {}

struct ImagePasteHistory {
    transaction: TransactionId,
    _events: Subscription,
    _release: Subscription,
}

pub(super) fn track(
    buffer: &Entity<Buffer>,
    transaction: TransactionId,
    fs: Arc<dyn fs::Fs>,
    path: PathBuf,
    bytes: Vec<u8>,
    identity: fs::Metadata,
    window: &mut Window,
    cx: &mut Context<Editor>,
) -> Result<()> {
    let edit = buffer
        .read(cx)
        .get_transaction(transaction)
        .and_then(|transaction| transaction.edit_ids.first())
        .copied()
        .context("Image insertion did not create a text edit")?;
    let (sender, mut receiver) = mpsc::unbounded();
    let mut undone = false;
    let events = App::subscribe(cx, buffer, move |buffer, event, cx| {
        if let language::BufferEvent::Operation {
            operation: language::Operation::Buffer(text::Operation::Undo(operation)),
            is_local: true,
        } = event
            && let Some(count) = operation.counts.get(&edit)
        {
            undone = count % 2 != 0;
            if !sender.is_closed() {
                sender.unbounded_send(!undone).log_err();
            }
        } else if undone
            && matches!(
                event,
                language::BufferEvent::Operation {
                    operation: language::Operation::Buffer(text::Operation::Edit(_)),
                    is_local: true,
                }
            )
            && !buffer.read(cx).has_history_edit(edit)
        {
            let histories = &mut cx.default_global::<ImagePasteHistories>().0;
            if let Some(entries) = histories.get_mut(&buffer.entity_id()) {
                entries.retain(|entry| entry.transaction != transaction);
                if entries.is_empty() {
                    histories.remove(&buffer.entity_id());
                }
            }
        }
    });
    cx.spawn_in(window, async move |editor, cx| {
        let mut present = true;
        let mut identity = identity;
        while let Some(restore) = receiver.next().await {
            if restore == present {
                continue;
            }
            let result = async {
                if restore {
                    fs.create_file_exclusive(&path, &bytes)
                        .await
                        .with_context(|| {
                            format!("Cannot redo image paste without overwriting {path:?}")
                        })?;
                    identity = fs
                        .metadata(&path)
                        .await?
                        .context("Restored image disappeared")?;
                } else {
                    let held =
                        path.with_file_name(format!(".zed-image-undo-{}", uuid::Uuid::new_v4()));
                    if let Err(error) = fs.rename_exclusive(&path, &held).await {
                        if fs.metadata(&path).await?.is_none() {
                            return Ok(());
                        }
                        return Err(error);
                    }
                    finish_image_undo(fs.as_ref(), &path, &held, &identity, &bytes).await?;
                }
                anyhow::Ok(())
            }
            .await;
            if let Err(error) = result {
                editor
                    .update_in(cx, |editor, window, cx| {
                        editor.detach_and_notify_err(Task::ready(Err::<(), _>(error)), window, cx);
                    })
                    .log_err();
                break;
            }
            present = restore;
        }
    })
    .detach(); // Dropping the event sender at buffer release drains queued file operations.
    let id = buffer.entity_id();
    let release = App::observe_release(cx, buffer, move |_, cx| {
        cx.default_global::<ImagePasteHistories>().0.remove(&id);
    });
    cx.default_global::<ImagePasteHistories>()
        .0
        .entry(id)
        .or_default()
        .push(ImagePasteHistory {
            transaction,
            _events: events,
            _release: release,
        });
    Ok(())
}

async fn finish_image_undo(
    fs: &dyn fs::Fs,
    path: &std::path::Path,
    held: &std::path::Path,
    identity: &fs::Metadata,
    bytes: &[u8],
) -> Result<()> {
    let result = async {
        let current = fs
            .metadata(held)
            .await?
            .context("Pasted image disappeared")?;
        anyhow::ensure!(
            !current.is_symlink
                && !current.is_dir
                && !current.is_fifo
                && current.inode == identity.inode
                && current.mtime == identity.mtime
                && current.len == bytes.len() as u64
                && fs.load_bytes(held).await? == bytes,
            "Image changed after pasting: {path:?}"
        );
        fs.remove_file(held, fs::RemoveOptions::default()).await
    }
    .await;
    if let Err(error) = result {
        fs.rename_exclusive(held, path).await.with_context(|| {
            format!("{error:#}; could not restore {path:?}; recovery path: {held:?}")
        })?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
#[gpui::test]
async fn image_undo_keeps_both_replacements(cx: &mut gpui::TestAppContext) {
    use fs::Fs as _;
    let fs = fs::FakeFs::new(cx.executor());
    let path = std::path::Path::new(util::path!("/image.png"));
    let held = path.with_extension("held");
    fs.insert_file(path, b"original".to_vec()).await;
    let identity = fs.metadata(path).await.unwrap().unwrap();
    fs.insert_file(path, b"replacement one".to_vec()).await;
    fs.rename_exclusive(path, &held).await.unwrap();
    fs.insert_file(path, b"replacement two".to_vec()).await;
    let error = finish_image_undo(fs.as_ref(), path, &held, &identity, b"original")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("recovery path"));
    assert_eq!(fs.load_bytes(path).await.unwrap(), b"replacement two");
    assert_eq!(fs.load_bytes(&held).await.unwrap(), b"replacement one");
}

#[cfg(test)]
pub(super) fn tracked_images(buffer: &Entity<Buffer>, cx: &App) -> usize {
    cx.try_global::<ImagePasteHistories>()
        .and_then(|histories| histories.0.get(&buffer.entity_id()))
        .map_or(0, Vec::len)
}
