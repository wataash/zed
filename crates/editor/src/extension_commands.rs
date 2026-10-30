// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
use super::*;
use zed_editor_command_api::{
    CommandInput, CommandOutput, ContextRequest, EditorContext, Effect, TextEdit, TextRange,
};

pub(super) type FileOpenHookRun = (
    PathBuf,
    settings::ZhackEditorHook,
    extension::editor_commands::CommandProvider,
);

impl Editor {
    pub(super) fn run_file_open_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.mode.is_full()
            || !self
                .project
                .as_ref()
                .is_some_and(|project| project.read(cx).is_local())
        {
            return;
        }
        let Some(buffer) = self.buffer.read(cx).as_singleton() else {
            return;
        };
        // Diff base and preview buffers can carry file metadata without being
        // actual project files. They must not run file-open commands.
        if !self.project.as_ref().is_some_and(|project| {
            project
                .read(cx)
                .buffer_store()
                .read(cx)
                .get(buffer.read(cx).remote_id())
                .as_ref()
                == Some(&buffer)
        }) {
            return;
        }
        let Some(file) = buffer.read(cx).file() else {
            return;
        };
        let Some(local) = file.as_local() else {
            return;
        };
        let path = local.abs_path(cx);
        let hooks = project::project_settings::ProjectSettings::get(
            Some(settings::SettingsLocation {
                worktree_id: file.worktree_id(cx),
                path: file.path(),
            }),
            cx,
        )
        .on_file_open
        .clone();
        let hooks = hooks
            .into_iter()
            .filter_map(|hook| {
                let provider = cx
                    .try_global::<extension::editor_commands::EditorCommands>()?
                    .0
                    .get(hook.extension.as_str())?
                    .clone();
                Some((hook, provider))
            })
            .collect::<Vec<_>>();
        self.file_open_hooks_run
            .retain(|(previous_path, previous_hook, previous_provider)| {
                *previous_path == path
                    && hooks.iter().any(|(hook, provider)| {
                        hook == previous_hook && Arc::ptr_eq(provider, previous_provider)
                    })
            });
        for (hook, provider) in hooks {
            if self
                .file_open_hooks_run
                .iter()
                .any(|(_, previous_hook, _)| previous_hook == &hook)
            {
                continue;
            }
            self.file_open_hooks_run
                .push((path.clone(), hook.clone(), provider));
            self.execute_extension_command(
                &ExecuteExtensionCommand {
                    extension: hook.extension,
                    command: hook.command,
                    arguments: hook.arguments,
                    context: hook.context,
                },
                window,
                cx,
            );
        }
    }

    pub(crate) fn run_extension_command_with_context(
        &mut self,
        extension: &str,
        command: &str,
        arguments: serde_json::Value,
        request: ContextRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(provider) = cx
            .try_global::<extension::editor_commands::EditorCommands>()
            .and_then(|commands| commands.0.get(extension))
            .cloned()
        else {
            self.notify_personal_error("The editing-command extension is not loaded.", window, cx);
            return;
        };
        let buffer = self.buffer.clone();
        let snapshot = buffer.read(cx).snapshot(cx);
        let selections = self
            .selections
            .all::<MultiBufferOffset>(&self.display_snapshot(cx));
        let singleton = buffer.read(cx).as_singleton();
        let path = singleton.as_ref().and_then(|buffer| {
            buffer
                .read(cx)
                .file()?
                .as_local()
                .and_then(|file| file.abs_path(cx).to_str().map(ToOwned::to_owned))
        });
        let language = singleton.as_ref().and_then(|buffer| {
            buffer
                .read(cx)
                .language()
                .map(|language| language.name().to_string())
        });
        let project = self
            .project
            .clone()
            .filter(|project| project.read(cx).is_local());
        let roots = project
            .as_ref()
            .map(|project| {
                project
                    .read(cx)
                    .visible_worktrees(cx)
                    .filter_map(|worktree| {
                        worktree.read(cx).abs_path().to_str().map(ToOwned::to_owned)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let input = CommandInput {
            selections: selections
                .iter()
                .map(|selection| {
                    if request.omit_selection_text {
                        String::new()
                    } else {
                        snapshot.text_for_range(selection.range()).collect()
                    }
                })
                .collect(),
            local_time: chrono::Local::now().to_rfc3339(),
            arguments,
            context: EditorContext {
                document: request.document.then(|| snapshot.text()),
                path,
                language,
                roots,
                read_only: self.read_only(cx),
                selections: selections
                    .iter()
                    .map(|selection| TextRange {
                        start: selection.start.0,
                        end: selection.end.0,
                        reversed: selection.reversed,
                    })
                    .collect(),
                clipboard: if request.clipboard {
                    cx.read_from_clipboard().and_then(|item| item.text())
                } else {
                    None
                },
                clipboard_image: if request.clipboard_image {
                    cx.read_from_clipboard().and_then(|item| {
                        item.entries().iter().find_map(|entry| {
                            if let ClipboardEntry::Image(image) = entry {
                                Some(zed_editor_command_api::ClipboardImage {
                                    mime_type: image.format.mime_type().into(),
                                    bytes: image.bytes.clone(),
                                })
                            } else {
                                None
                            }
                        })
                    })
                } else {
                    None
                },
            },
            host_capabilities: zed_editor_command_api::HOST_CAPABILITIES
                .iter()
                .map(|value| value.to_string())
                .collect(),
        };
        let command = command.to_owned();
        let workspace = self.workspace();
        let task = cx.spawn_in(window, async move |editor, cx| {
            let output = provider(command, serde_json::to_string(&input)?).await?;
            let mut output: CommandOutput = serde_json::from_str(&output)?;
            anyhow::ensure!(
                output.replacements.is_empty() || output.replacements.len() == selections.len(),
                "The extension returned an incorrect number of replacements"
            );
            anyhow::ensure!(
                output.edits.is_empty() || output.replacements.is_empty(),
                "Use edits or replacements, not both"
            );
            for (selection, replacement) in selections
                .iter()
                .zip(std::mem::take(&mut output.replacements))
            {
                if let Some(text) = replacement {
                    output.edits.push(TextEdit {
                        range: TextRange {
                            start: selection.start.0,
                            end: selection.end.0,
                            reversed: false,
                        },
                        text,
                    });
                }
            }
            output
                .edits
                .sort_by_key(|edit| (edit.range.start, edit.range.end));
            let source_text = snapshot.text();
            let result_text = output
                .validate_edits(&source_text)
                .map_err(anyhow::Error::msg)?;
            drop(result_text);
            output.edits.retain(|edit| {
                source_text.get(edit.range.start..edit.range.end) != Some(edit.text.as_str())
            });
            // Validate all synchronous operations before applying any edit or effect.
            editor.update(cx, |editor, cx| {
                let current = editor.buffer.read(cx).snapshot(cx);
                let metadata_only = output.edits.is_empty() && output.selections.is_none() && output.effects.iter().all(|effect| matches!(effect, Effect::GitDiffSource { .. }));
                let same_path = buffer.read(cx).as_singleton().and_then(|buffer| buffer.read(cx).file()?.as_local()?.abs_path(cx).to_str().map(ToOwned::to_owned)) == input.context.path;
                anyhow::ensure!(
                    editor.buffer == buffer
                        && same_path
                        && (metadata_only || (current.edit_count() == snapshot.edit_count()
                        && current.excerpts().eq(snapshot.excerpts())
                        && editor
                            .selections
                            .all::<MultiBufferOffset>(&editor.display_snapshot(cx))
                            == selections)),
                    "The document or selection changed while running the command; try again"
                );
                anyhow::ensure!(
                    output.edits.is_empty() || !editor.read_only(cx),
                    "The editor is read-only"
                );
                for effect in &output.effects {
                    match effect {
                        Effect::GitDiffSource { source } => {
                            anyhow::ensure!(project.is_some() && singleton.is_some(), "Git diff sources require a local single-file editor");
                            if let Some(source) = source {
                                anyhow::ensure!(Path::new(&source.repository).is_absolute() && !source.path.is_empty() && Path::new(&source.path).components().all(|component| matches!(component, std::path::Component::Normal(_))), "Expected an absolute Git repository and a relative file path without parent components");
                            }
                        }
                        Effect::InsertImage {
                            file_name,
                            mime_type,
                            bytes,
                        } => {
                            anyhow::ensure!(!editor.read_only(cx), "The editor is read-only");
                            anyhow::ensure!(
                                project.is_some()
                                    && input.context.path.is_some()
                                    && input.context.language.as_deref() == Some("Markdown"),
                                "Image insertion requires a local Markdown document"
                            );
                            anyhow::ensure!(
                                !file_name.is_empty()
                                    && !file_name.contains(['/', '\\', '\0'])
                                    && Path::new(file_name).file_stem().is_some()
                                    && Path::new(file_name).extension().is_some(),
                                "Expected an image filename without directories"
                            );
                            anyhow::ensure!(
                                gpui::ImageFormat::from_mime_type(mime_type).is_some()
                                    && !bytes.is_empty(),
                                "Invalid image data"
                            );
                        }
                        Effect::DispatchAction { name, arguments } => {
                            cx.build_action(
                                name,
                                (!arguments.is_null()).then(|| arguments.clone()),
                            )?;
                        }
                        Effect::SetIndentation { tab_size, .. } => {
                            anyhow::ensure!(
                                *tab_size > 0 && buffer.read(cx).as_singleton().is_some(),
                                "Indentation requires a positive width and single-file editor"
                            );
                        }
                        Effect::CreateFile { path, .. } | Effect::OpenFile { path } => {
                            anyhow::ensure!(
                                Path::new(path).is_absolute()
                                    && project.is_some()
                                    && workspace.is_some(),
                                "File operations require an absolute local path and a workspace"
                            );
                        }
                        Effect::OpenUrl { url } => {
                            url::Url::parse(url)?;
                        }
                        Effect::Notify { .. } => {
                            anyhow::ensure!(
                                workspace.is_some(),
                                "Notifications require a workspace"
                            );
                        }
                        Effect::ClipboardImage { mime_type, bytes } => {
                            anyhow::ensure!(
                                gpui::ImageFormat::from_mime_type(mime_type).is_some()
                                    && !bytes.is_empty(),
                                "Invalid clipboard image format or empty data"
                            );
                        }
                        Effect::Clipboard { .. } => {}
                    }
                }
                anyhow::Ok(())
            })??;
            editor.update_in(cx, |editor, window, cx| {
                if !output.edits.is_empty() || output.selections.is_some() {
                    editor.transact(window, cx, |editor, window, cx| {
                        let anchors: Vec<_> = selections
                            .iter()
                            .map(|selection| {
                                let changed = output.edits.iter().any(|edit| {
                                    edit.range.start <= selection.end.0
                                        && edit.range.end >= selection.start.0
                                });
                                if changed {
                                    selection.map(|_| snapshot.anchor_after(selection.end))
                                } else {
                                    selection.map(|offset| snapshot.anchor_before(offset))
                                }
                            })
                            .collect();
                        editor.edit(
                            output.edits.iter().map(|edit| {
                                (
                                    MultiBufferOffset(edit.range.start)
                                        ..MultiBufferOffset(edit.range.end),
                                    edit.text.clone(),
                                )
                            }),
                            cx,
                        );
                        editor.change_selections(Default::default(), window, cx, |selections| {
                            if let Some(ranges) = &output.selections {
                                selections.select_ranges(ranges.iter().map(|range| {
                                    if range.reversed {
                                        MultiBufferOffset(range.end)..MultiBufferOffset(range.start)
                                    } else {
                                        MultiBufferOffset(range.start)..MultiBufferOffset(range.end)
                                    }
                                }));
                            } else {
                                selections.select_anchors(anchors);
                            }
                        });
                    });
                }
            })?;
            // Effects execute in order after text validation. Files/clipboard/UI actions are not
            // an atomic transaction; a later I/O failure does not roll back earlier effects.
            for effect in output.effects {
                match effect {
                    Effect::GitDiffSource { source } => {
                        let project = project.as_ref().context("No local project")?;
                        let source_buffer = singleton.as_ref().context("No single file")?;
                        project.update(cx, |project, cx| {
                            let buffer_id = source_buffer.read(cx).remote_id();
                            project.git_store().update(cx, |store, cx| store.set_buffer_diff_source(buffer_id, source.map(|source| project::git_store::GitDiffSource { repository: source.repository.into(), path: source.path.into() }), cx))
                        })?;
                    }
                    Effect::InsertImage {
                        file_name,
                        mime_type,
                        bytes,
                    } => {
                        let format = gpui::ImageFormat::from_mime_type(&mime_type)
                            .context("Unknown image format")?;
                        editor.update_in(cx, |editor, window, cx| {
                            editor.paste_item_with_image_name(
                                &ClipboardItem::new_image(&gpui::Image::from_bytes(format, bytes)),
                                Some(&file_name),
                                window,
                                cx,
                            );
                        })?;
                    }
                    Effect::Clipboard { text } => {
                        cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(text)))?
                    }
                    Effect::ClipboardImage { mime_type, bytes } => {
                        let format = gpui::ImageFormat::from_mime_type(&mime_type)
                            .context("Unknown image format")?;
                        cx.update(|_, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_image(
                                &gpui::Image::from_bytes(format, bytes),
                            ))
                        })?;
                    }
                    Effect::DispatchAction { name, arguments } => cx.update(|window, cx| {
                        let action =
                            cx.build_action(&name, (!arguments.is_null()).then_some(arguments))?;
                        window.dispatch_action(action, cx);
                        anyhow::Ok(())
                    })??,
                    Effect::SetIndentation {
                        tab_size,
                        hard_tabs,
                    } => editor.update_in(cx, |editor, window, cx| {
                        editor.set_indentation(
                            &SetIndentation {
                                tab_size,
                                hard_tabs,
                            },
                            window,
                            cx,
                        )
                    })?,
                    Effect::OpenUrl { url } => cx.update(|_, cx| cx.open_url(&url))?,
                    Effect::Notify { message } => {
                        let workspace = workspace.as_ref().context("Workspace closed")?;
                        workspace.update(cx, |workspace, cx| {
                            workspace.show_toast(
                                workspace::Toast::new(
                                    workspace::notifications::NotificationId::named(
                                        "extension-command".into(),
                                    ),
                                    message,
                                ),
                                cx,
                            )
                        });
                    }
                    Effect::CreateFile { path, text } => {
                        let project = project.as_ref().context("No local project")?;
                        let fs = project.read_with(cx, |project, _| project.fs().clone());
                        let path = PathBuf::from(path);
                        if let Some(metadata) = fs.metadata(&path).await? {
                            anyhow::ensure!(!metadata.is_dir, "The file path is a directory");
                        } else {
                            fs.create_file_exclusive(&path, text.as_bytes()).await?;
                        }
                    }
                    Effect::OpenFile { path } => {
                        workspace
                            .as_ref()
                            .context("Workspace closed")?
                            .update_in(cx, |workspace, window, cx| {
                                workspace.open_abs_path(
                                    PathBuf::from(path),
                                    workspace::OpenOptions::default(),
                                    window,
                                    cx,
                                )
                            })?
                            .await?;
                    }
                }
            }

            anyhow::Ok(())
        });
        self.detach_and_notify_err(task, window, cx);
    }
}
