// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
use super::*;
use futures::{StreamExt as _, channel::mpsc};

#[derive(Default)]
struct ImagePasteHistories(HashMap<gpui::EntityId, Vec<ImagePasteHistory>>);
impl Global for ImagePasteHistories {}

struct ImagePasteHistory {
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
    let events = App::subscribe(cx, buffer, move |_, event, _| {
        if let language::BufferEvent::Operation {
            operation: language::Operation::Buffer(text::Operation::Undo(operation)),
            is_local: true,
        } = event
            && let Some(count) = operation.counts.get(&edit)
        {
            if !sender.is_closed() {
                sender.unbounded_send(count % 2 == 0).log_err();
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
                    let current = fs
                        .metadata(&path)
                        .await?
                        .context("Pasted image no longer exists")?;
                    anyhow::ensure!(
                        !current.is_symlink
                            && !current.is_dir
                            && current.inode == identity.inode
                            && current.mtime == identity.mtime
                            && fs.load_bytes(&path).await? == bytes,
                        "Image changed after pasting; leaving it untouched: {path:?}"
                    );
                    fs.remove_file(&path, fs::RemoveOptions::default()).await?;
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
            _events: events,
            _release: release,
        });
    Ok(())
}
