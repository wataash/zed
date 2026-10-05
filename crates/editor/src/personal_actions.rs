use super::*;

impl Editor {
    pub fn calculate_selection(
        &mut self,
        _: &CalculateSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.transform_personal_selections(
            |text| selection_tools::calculate(text).map(|result| result.0),
            window,
            cx,
        );
    }

    pub fn precise_calculation(
        &mut self,
        _: &PreciseCalculation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.transform_personal_selections(
            |text| selection_tools::calculate(text).map(|result| result.1),
            window,
            cx,
        );
    }

    pub fn uniq_selection(
        &mut self,
        _: &UniqSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.transform_personal_selections(
            |text| Some(selection_tools::unique_lines(text)),
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

    fn transform_personal_selections(
        &mut self,
        transform: impl Fn(&str) -> Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only(cx) {
            return;
        }
        let snapshot = self.buffer.read(cx).snapshot(cx);
        let selections = self
            .selections
            .all::<MultiBufferOffset>(&self.display_snapshot(cx));
        // Validate every selection before editing so that an invalid one leaves the buffer untouched.
        let replacements: Option<Vec<_>> = selections
            .iter()
            .map(|selection| {
                if selection.is_empty() {
                    return None;
                }
                let text = snapshot
                    .text_for_range(selection.range())
                    .collect::<String>();
                let replacement = transform(&text)?;
                Some((replacement != text).then_some((selection.range(), replacement)))
            })
            .collect();
        let Some(replacements) = replacements else {
            self.notify_personal_error("Select valid text in every selection.", window, cx);
            return;
        };
        // Unchanged selections are left out of the edit so they neither dirty the buffer nor
        // add an undo entry; their selection is kept, while changed ones collapse after the result.
        let anchors: Vec<_> = selections
            .iter()
            .zip(&replacements)
            .map(|(selection, replacement)| {
                if replacement.is_some() {
                    let anchor = snapshot.anchor_after(selection.end);
                    selection.map(|_| anchor)
                } else {
                    selection.map(|offset| snapshot.anchor_before(offset))
                }
            })
            .collect();
        let replacements: Vec<_> = replacements.into_iter().flatten().collect();
        if replacements.is_empty() {
            return;
        }
        self.transact(window, cx, |this, window, cx| {
            this.edit(replacements, cx);
            this.change_selections(Default::default(), window, cx, |selections| {
                selections.select_anchors(anchors)
            });
        });
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
