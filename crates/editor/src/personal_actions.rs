use super::*;

impl Editor {
    pub fn create_x_file(&mut self, _: &CreateXFile, window: &mut Window, cx: &mut Context<Self>) {
        let target = self
            .project
            .as_ref()
            .filter(|project| project.read(cx).is_local())
            .and_then(|project| {
                let buffer = self.buffer.read(cx).as_singleton()?;
                let file = buffer.read(cx).file()?;
                let path = file.as_local()?.abs_path(cx);
                Some((
                    project.read(cx).fs().clone(),
                    path.clone(),
                    x_file_path(&path)?,
                ))
            });
        let (Some((fs, source, path)), Some(workspace)) = (target, self.workspace()) else {
            self.notify_personal_error(
                "Open a saved local file in a single-file editor to create its X file.",
                window,
                cx,
            );
            return;
        };
        if source == path {
            return;
        }
        let task = cx.spawn_in(window, async move |_, cx| {
            anyhow::ensure!(
                fs.is_file(&source).await,
                "Save the source file before creating its X file."
            );
            if let Some(metadata) = fs.metadata(&path).await? {
                anyhow::ensure!(
                    !metadata.is_dir,
                    "X file path is a directory: {}",
                    path.display()
                );
            } else {
                fs.create_file(
                    &path,
                    fs::CreateOptions {
                        overwrite: false,
                        ignore_if_exists: true,
                    },
                )
                .await?;
            }
            workspace
                .update_in(cx, |workspace, window, cx| {
                    workspace.open_abs_path(path, workspace::OpenOptions::default(), window, cx)
                })?
                .await?;
            anyhow::Ok(())
        });
        self.detach_and_notify_err(task, window, cx);
    }

    pub fn sort_lines_like_vs_code(
        &mut self,
        action: &SortLinesLikeVsCode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = if action.descending {
            "refactor.rewrite.myext0.sortDescending"
        } else {
            "refactor.rewrite.myext0.sortAscending"
        };
        self.run_code_action_kind(kind.into(), true, window, cx);
    }

    pub fn execute_code_action(
        &mut self,
        action: &ExecuteCodeAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_code_action_kind(action.kind.clone().into(), action.select_lines, window, cx);
    }

    fn run_code_action_kind(
        &mut self,
        kind: CodeActionKind,
        select_lines: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only(cx) {
            return;
        }
        if self.selections.count() != 1 {
            self.notify_personal_error("Code actions require one selection.", window, cx);
            return;
        }
        let Some(project) = self.project.clone() else {
            self.notify_personal_error(
                "Open a project with a language server for this code action.",
                window,
                cx,
            );
            return;
        };
        let display = self.display_snapshot(cx);
        let selection = self.selections.newest_adjusted(&display);
        let multibuffer = self.buffer.read(cx);
        let Some(buffer) = multibuffer.as_singleton() else {
            self.notify_personal_error("Run code actions in a single-file editor.", window, cx);
            return;
        };
        let Some((_, start)) = multibuffer.text_anchor_for_position(selection.start, cx) else {
            return;
        };
        let Some((_, end)) = multibuffer.text_anchor_for_position(selection.end, cx) else {
            return;
        };
        // LSP edits are diffed into smaller edits by Zed. Keep anchors around the
        // whole line range so sorting does not turn one selection into many.
        let source = buffer.read(cx);
        let first_row = start.to_point(source).row;
        let end_point = end.to_point(source);
        let last_row = if end_point.column == 0 && end_point.row > first_row {
            end_point.row - 1
        } else {
            end_point.row
        };
        let sorted_range = source.anchor_before(Point::new(first_row, 0))
            ..source.anchor_after(Point::new(last_row, source.line_len(last_row)));
        let version = buffer.read(cx).version();
        let request = project.update(cx, |project, cx| {
            project.code_actions(&buffer, start..end, Some(vec![kind.clone()]), cx)
        });
        let task = cx.spawn_in(window, async move |editor, cx| {
            let mut actions = request
                .await?
                .unwrap_or_default()
                .into_iter()
                .filter(|action| {
                    matches!(&action.lsp_action, project::LspAction::Action(action)
                    if action.kind.as_ref() == Some(&kind) && action.disabled.is_none())
                });
            let Some(action) = actions.next() else {
                return Ok(());
            };
            anyhow::ensure!(
                actions.next().is_none(),
                "More than one language server returned the requested code action"
            );
            editor.update(cx, |editor, cx| {
                anyhow::ensure!(!editor.read_only(cx), "The editor became read-only");
                anyhow::ensure!(
                    buffer.read(cx).version() == version,
                    "The document changed while running the code action; try again"
                );
                Ok::<_, anyhow::Error>(())
            })??;
            project
                .update(cx, |project, cx| {
                    project.apply_code_action(buffer.clone(), action, true, cx)
                })
                .await?;
            if select_lines {
                editor.update_in(cx, |editor, window, cx| {
                    let source = buffer.read(cx);
                    let range =
                        sorted_range.start.to_point(source)..sorted_range.end.to_point(source);
                    editor.change_selections(Default::default(), window, cx, |selections| {
                        selections.select_ranges([range]);
                    });
                })?;
            }
            Ok(())
        });
        self.detach_and_notify_err(task, window, cx);
    }

    pub fn calculate_selection(
        &mut self,
        _: &CalculateSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_extension_command("myext0", "calculate", window, cx);
    }

    pub fn precise_calculation(
        &mut self,
        _: &PreciseCalculation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_extension_command("myext0", "calculate-precise", window, cx);
    }

    pub fn uniq_selection(
        &mut self,
        _: &UniqSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_extension_command("myext0", "unique-lines", window, cx);
    }

    pub fn execute_extension_command(
        &mut self,
        action: &ExecuteExtensionCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let definition = cx
            .try_global::<extension::editor_commands::EditorCommands>()
            .and_then(|commands| commands.1.get(action.extension.as_str()))
            .and_then(|commands| commands.get(action.command.as_str()))
            .cloned()
            .unwrap_or_default();
        let context = if action.context.is_null() {
            Ok(definition.context)
        } else {
            serde_json::from_value(action.context.clone())
        };
        let context = match context {
            Ok(context) => context,
            Err(error) => {
                self.detach_and_notify_err(
                    Task::ready(Err::<(), _>(anyhow::Error::from(error))),
                    window,
                    cx,
                );
                return;
            }
        };
        let arguments = if action.arguments.is_null() {
            definition.arguments
        } else {
            action.arguments.clone()
        };
        self.run_extension_command_with_context(
            &action.extension,
            &action.command,
            arguments,
            context,
            window,
            cx,
        );
    }

    pub(crate) fn run_extension_command(
        &mut self,
        extension: &str,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_extension_command(
            &ExecuteExtensionCommand {
                extension: extension.into(),
                command: command.into(),
                ..Default::default()
            },
            window,
            cx,
        );
    }

    pub(crate) fn notify_personal_error(
        &self,
        message: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.detach_and_notify_err(
            Task::ready(Err::<(), _>(anyhow::anyhow!(message))),
            window,
            cx,
        );
    }

    pub fn sort_lines_descending(
        &mut self,
        _: &SortLinesDescending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.manipulate_immutable_lines(window, cx, |lines| {
            lines.sort_by(|left, right| right.cmp(left))
        });
    }

    pub fn insert_clipboard_file_uri(
        &mut self,
        _: &InsertClipboardFileUri,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only(cx) {
            return;
        }
        let text = cx.read_from_clipboard().and_then(|item| {
            item.entries().iter().find_map(|entry| match entry {
                ClipboardEntry::String(text) => Some(text.text().to_owned()),
                _ => None,
            })
        });
        if let Some(text) = text {
            self.insert(&format!("file://{text}"), window, cx);
        } else {
            self.notify_personal_error("The clipboard does not contain text.", window, cx);
        }
    }

    pub fn copy_current_code_block(
        &mut self,
        _: &CopyCurrentCodeBlock,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.buffer.read(cx).snapshot(cx);
        if snapshot.as_singleton().is_none() {
            self.notify_personal_error(
                "Code blocks can only be copied from a single-file editor.",
                window,
                cx,
            );
            return;
        }
        let selection = self.selections.newest::<Point>(&self.display_snapshot(cx));
        if let Some(text) =
            selection_tools::code_block(&snapshot.text(), selection.head().row as usize)
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        } else {
            self.notify_personal_error("The cursor is not inside a closed code block.", window, cx);
        }
    }

    pub fn open_directory_in_tilix(
        &mut self,
        _: &OpenDirectoryInTilix,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = self.tilix_directory(cx);
        self.detach_and_notify_err(Self::launch_tilix(directory, cx), window, cx);
    }

    /// The directory of the editor's local file, falling back to
    /// [`Editor::tilix_directory_for_project`]. Remote projects yield `None`.
    pub fn tilix_directory(&self, cx: &App) -> Option<PathBuf> {
        match self.project.as_ref() {
            Some(project) => {
                let project = project.read(cx);
                project
                    .is_local()
                    .then(|| self.working_directory(cx))
                    .flatten()
                    .or_else(|| Self::tilix_directory_for_project(project, cx))
            }
            None => self.working_directory(cx),
        }
    }

    /// Returns the directory Tilix should open when no local file is focused: the first
    /// visible worktree of a local project. Single-file worktrees (`zed file.txt`) have the
    /// file itself as their root, so their parent directory is used instead.
    pub fn tilix_directory_for_project(project: &Project, cx: &App) -> Option<PathBuf> {
        if !project.is_local() {
            return None;
        }
        let worktree = project.visible_worktrees(cx).next()?;
        let worktree = worktree.read(cx);
        let path = worktree.abs_path();
        if worktree.is_single_file() {
            path.parent().map(Path::to_path_buf)
        } else {
            Some(path.to_path_buf())
        }
    }

    pub fn launch_tilix(directory: Option<PathBuf>, cx: &App) -> Task<anyhow::Result<()>> {
        cx.background_spawn(async move {
            let directory =
                directory.ok_or_else(|| anyhow::anyhow!("No local directory is open."))?;
            let mut child = util::command::new_command("tilix")
                .arg("--working-directory")
                .arg(directory)
                .stdin(util::command::Stdio::null())
                .stdout(util::command::Stdio::null())
                .stderr(util::command::Stdio::null())
                .spawn()?;
            let status = child.status().await?;
            anyhow::ensure!(status.success(), "Tilix exited with {status}");
            Ok::<(), anyhow::Error>(())
        })
    }
}

fn x_file_path(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?;
    let name = path.file_name()?.as_encoded_bytes();
    if name.ends_with(b".x") || name.windows(3).any(|part| part == b".x.") {
        return Some(path.to_path_buf());
    }
    let mut name = stem.to_os_string();
    name.push(".x");
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    Some(path.with_file_name(name))
}

#[cfg(test)]
mod x_file_tests {
    use super::x_file_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_personal_x_file_names() {
        for (source, target) in [
            ("/notes/music-sessions.md", "/notes/music-sessions.x.md"),
            ("/notes/演奏.md", "/notes/演奏.x.md"),
            ("/notes/notes", "/notes/notes.x"),
            ("/notes/.env", "/notes/.env.x"),
            ("/notes/archive.tar.gz", "/notes/archive.tar.x.gz"),
            ("/notes/music.x.md", "/notes/music.x.md"),
            ("/notes/notes.x", "/notes/notes.x"),
            ("/notes/archive.x.tar.gz", "/notes/archive.x.tar.gz"),
            ("/notes/x.md", "/notes/x.x.md"),
        ] {
            assert_eq!(x_file_path(Path::new(source)), Some(PathBuf::from(target)));
        }
        assert_eq!(x_file_path(Path::new("/")), None);
    }
}
