use std::{fmt, path::Path, sync::Arc};

use gpui::{Entity, EventEmitter, FocusHandle, Focusable, StatefulInteractiveElement as _};
use project::{ProjectPath, Worktree};
use settings::Settings as _;
use ui::{
    App, Button, ButtonCommon, ButtonStyle, Clickable, Context, Disableable, FluentBuilder,
    InteractiveElement, KeyBinding, Label, LabelCommon, LabelSize, ParentElement, Render,
    SharedString, Styled as _, Window, h_flex, v_flex,
};
use util::ResultExt;
use zed_actions::workspace::OpenWithSystem;

use crate::{Item, ItemNavHistory, Workspace};

/// A view to display when a certain buffer/image/other item fails to open.
pub struct InvalidItemView {
    /// Which path was attempted to open.
    pub abs_path: Arc<Path>,
    /// An error message, happened when opening the item.
    pub error: SharedString,
    is_local: bool,
    preview_page: Option<crate::binary_files::PreviewPage>,
    preview_task: Option<gpui::Task<()>>,
    preview_subscription: Option<gpui::Subscription>,
    requested_offset: u64,
    preview_error: Option<SharedString>,
    focus_handle: FocusHandle,
    project_identity: Option<(Entity<Worktree>, ProjectPath)>,
    nav_history: Option<ItemNavHistory>,
}

impl InvalidItemView {
    fn load_preview_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        self.requested_offset = offset;
        self.preview_error = None;
        cx.notify();
        let options = project::project_settings::ProjectSettings::get_global(cx)
            .binary_file_options
            .clone();
        let task = crate::binary_files::present_file(&self.abs_path, offset, "view", options, cx);
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(page) => this.preview_page = Some(page),
                    Err(error) => this.preview_error = Some(error.to_string().into()),
                }
                cx.notify();
            })
            .log_err();
        }));
    }

    fn external_button_label(&self) -> &str {
        self.preview_page
            .as_ref()
            .and_then(|page| {
                page.presentation.actions.iter().find(|choice| {
                    matches!(
                        choice.action,
                        zed_editor_command_api::FilePreviewAction::OpenWithSystem
                    )
                })
            })
            .map_or("Open in Default App", |choice| choice.label.as_str())
    }

    fn retry_preview(&mut self, cx: &mut Context<Self>) {
        if self.preview_error.is_some() {
            self.load_preview_page(self.requested_offset, cx);
        }
    }

    pub fn new(
        abs_path: &Path,
        is_local: bool,
        e: &anyhow::Error,
        _: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self {
            is_local,
            preview_page: None,
            preview_task: None,
            preview_subscription: None,
            preview_error: None,
            requested_offset: 0,
            abs_path: Arc::from(abs_path),
            error: format!("{}", e.root_cause()).into(),
            focus_handle: cx.focus_handle(),
            project_identity: None,
            nav_history: None,
        }
    }

    pub(crate) fn with_project_path(
        mut self,
        worktree: Entity<Worktree>,
        project_path: ProjectPath,
    ) -> Self {
        self.project_identity = Some((worktree, project_path));
        self
    }

    pub(crate) fn has_deleted_file(&self, cx: &App) -> bool {
        self.project_identity
            .as_ref()
            .is_some_and(|(worktree, project_path)| {
                Workspace::history_path_is_removed(worktree.read(cx), &project_path.path, cx)
            })
    }
}

impl fmt::Debug for InvalidItemView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InvalidItemView")
            .field("abs_path", &self.abs_path)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Item for InvalidItemView {
    type Event = ();

    fn tab_content_text(&self, mut detail: usize, _: &App) -> SharedString {
        // Ensure we always render at least the filename.
        detail += 1;

        let path = self.abs_path.as_ref();

        let mut prefix = path;
        while detail > 0 {
            if let Some(parent) = prefix.parent() {
                prefix = parent;
                detail -= 1;
            } else {
                break;
            }
        }

        let path = if detail > 0 {
            path
        } else {
            path.strip_prefix(prefix).unwrap_or(path)
        };

        SharedString::new(path.to_string_lossy())
    }

    fn active_project_path(&self, _: &App) -> Option<ProjectPath> {
        self.project_identity
            .as_ref()
            .map(|(_, project_path)| project_path.clone())
    }

    fn set_nav_history(&mut self, history: ItemNavHistory, _: &mut Window, _: &mut Context<Self>) {
        self.nav_history = Some(history);
    }

    fn deactivated(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, project_path)) = &self.project_identity
            && let Some(history) = &mut self.nav_history
        {
            history.push_path(project_path.clone(), self.abs_path.to_path_buf(), cx);
        }
    }
}

impl EventEmitter<()> for InvalidItemView {}

impl Focusable for InvalidItemView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InvalidItemView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let abs_path = self.abs_path.clone();
        if self.is_local && self.error.as_ref() == "Binary files are not supported" {
            if self.preview_subscription.is_none() {
                self.preview_subscription = Some(
                    cx.observe_global::<extension::editor_commands::EditorCommands>(|this, cx| {
                        this.load_preview_page(this.requested_offset, cx);
                    }),
                );
            }
            if self.preview_task.is_none() {
                self.load_preview_page(0, cx);
            }
            let offset = self.preview_page.as_ref().map_or(0, |page| page.offset);
            let has_next = self.preview_page.as_ref().is_some_and(|page| {
                page.offset + (crate::binary_files::PAGE_SIZE as u64) < page.size
            });
            return v_flex()
                .size_full()
                .track_focus(&self.focus_handle)
                .key_context("InvalidItem")
                .child(
                    h_flex()
                        .p_2()
                        .gap_2()
                        .child(Label::new(
                            self.preview_page
                                .as_ref()
                                .map_or("File Preview · Read only".to_owned(), |page| {
                                    format!("{} · Read only", page.presentation.title)
                                }),
                        ))
                        .child(
                            Button::new(
                                "preview-external",
                                self.external_button_label().to_owned(),
                            )
                            .on_click(move |_, _, cx| cx.open_with_system(&abs_path))
                            .key_binding(KeyBinding::for_action(&OpenWithSystem, cx)),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .p_2()
                        .child(
                            Button::new("hex-previous", "Previous page")
                                .disabled(offset == 0)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.load_preview_page(
                                        offset
                                            .saturating_sub(crate::binary_files::PAGE_SIZE as u64),
                                        cx,
                                    )
                                })),
                        )
                        .child(
                            Button::new("hex-next", "Next page")
                                .disabled(!has_next)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.load_preview_page(
                                        offset + crate::binary_files::PAGE_SIZE as u64,
                                        cx,
                                    )
                                })),
                        )
                        .when_some(self.preview_page.as_ref(), |view, page| {
                            view.child(Label::new(format!(
                                "Offset {:#x} / {} bytes",
                                page.offset, page.size
                            )))
                        })
                        .when(
                            self.preview_page.is_none() && self.preview_error.is_none(),
                            |view| view.child(Label::new("Loading…")),
                        ),
                )
                .when_some(self.preview_error.clone(), |view, error| {
                    view.child(Label::new(error)).child(
                        Button::new("preview-retry", "Retry preview")
                            .on_click(cx.listener(|this, _, _, cx| this.retry_preview(cx))),
                    )
                })
                .child(
                    ui::div()
                        .id("hex-scroll")
                        .min_h_0()
                        .flex_1()
                        .overflow_scroll()
                        .p_2()
                        .child(
                            ui::div()
                                .font_family("monospace")
                                .whitespace_nowrap()
                                .child(self.preview_page.as_ref().map_or_else(
                                    || SharedString::from(""),
                                    |page| page.presentation.text.clone().into(),
                                )),
                        ),
                );
        }
        v_flex()
            .size_full()
            .track_focus(&self.focus_handle(cx))
            .flex_none()
            .justify_center()
            .overflow_hidden()
            .key_context("InvalidItem")
            .child(
                h_flex().size_full().justify_center().child(
                    v_flex()
                        .justify_center()
                        .gap_2()
                        .child(h_flex().justify_center().child("Could not open file"))
                        .child(
                            h_flex()
                                .justify_center()
                                .child(Label::new(self.error.clone()).size(LabelSize::Small)),
                        )
                        .when(self.is_local, |contents| {
                            contents.child(
                                h_flex().justify_center().child(
                                    Button::new("open-with-system", "Open in Default App")
                                        .on_click(move |_, _, cx| {
                                            cx.open_with_system(&abs_path);
                                        })
                                        .style(ButtonStyle::Outlined)
                                        .key_binding(KeyBinding::for_action(&OpenWithSystem, cx)),
                                ),
                            )
                        }),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn test_retry_preview_preserves_failed_offset(cx: &mut TestAppContext) {
        crate::tests::init_test(cx);
        let file = tempfile::NamedTempFile::new().expect("preview file");
        let offset = crate::binary_files::PAGE_SIZE as u64;
        file.as_file().set_len(offset + 1).expect("two pages");
        let (view, cx) = cx.add_window_view(|window, cx| {
            InvalidItemView::new(
                file.path(),
                true,
                &anyhow::anyhow!("Binary files are not supported"),
                window,
                cx,
            )
        });
        // No provider is registered, so both requests fail before reading a file.
        view.update(cx, |view, cx| view.load_preview_page(offset, cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.retry_preview(cx);
            assert!(view.preview_error.is_none());
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| assert!(view.preview_error.is_some()));
        cx.update(|_, cx| {
            let registry = cx.default_global::<extension::editor_commands::EditorCommands>();
            registry.1.entry("preview".into()).or_default()
                .insert("preview-file".into(), Default::default());
            registry.0.insert("preview".into(), Arc::new(|_, _| Box::pin(async {
                Ok(r#"{"file_presentation":{"title":"Hex","text":"page","actions":[]},"replacements":[]}"#.into())
            })));
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(view.preview_error.is_none(), "{:?}", view.preview_error);
            assert_eq!(view.external_button_label(), "Open in Default App");
            assert_eq!(
                view.preview_page.as_ref().map(|page| page.offset),
                Some(offset)
            );
        });
    }
}
