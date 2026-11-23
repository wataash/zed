// SPDX-FileCopyrightText: Copyright (c) 2026 Wataru Ashihara <wataash0607@gmail.com>
// SPDX-License-Identifier: Apache-2.0
//! Optional editing commands for custom Zed builds, independent of the official extension ABI.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandInput {
    /// Selected text in selection order, including empty cursors.
    pub selections: Vec<String>,
    /// Invocation time in RFC 3339 format with the host's local UTC offset.
    pub local_time: String,
    pub arguments: serde_json::Value,
    pub context: EditorContext,
    pub host_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandOutput {
    /// Used only by the file preview hook, never as an editing effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_presentation: Option<FilePresentation>,
    /// One replacement per input selection. None leaves that selection untouched.
    pub replacements: Vec<Option<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<TextEdit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selections: Option<Vec<TextRange>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

/// Bounded bytes of the file the user is opening. Delivered as command arguments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilePreviewInput {
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub bytes: Vec<u8>,
    /// "view" requests read-only text; "link" requests an opening decision.
    pub reason: String,
    pub options: serde_json::Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FilePresentation {
    pub title: String,
    pub text: String,
    pub actions: Vec<FilePreviewChoice>,
    /// None displays actions as a menu; Some executes the selected action.
    pub action: Option<FilePreviewAction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilePreviewChoice {
    pub label: String,
    pub action: FilePreviewAction,
}

/// Actions are restricted to the user-selected file, not extension-supplied paths.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilePreviewAction {
    OpenView,
    OpenWithSystem,
}

impl FilePresentation {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.len() > 1024 || self.text.len() > 1024 * 1024 || self.actions.len() > 16 {
            return Err("File presentation exceeds host limits".into());
        }
        if self
            .actions
            .iter()
            .any(|choice| choice.label.is_empty() || choice.label.len() > 256)
        {
            return Err("Invalid file action label".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ContextRequest {
    pub document: bool,
    pub clipboard: bool,
    pub clipboard_image: bool,
    pub omit_selection_text: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorContext {
    pub document: Option<String>,
    pub path: Option<String>,
    pub language: Option<String>,
    pub roots: Vec<String>,
    pub read_only: bool,
    pub selections: Vec<TextRange>,
    pub clipboard: Option<String>,
    pub clipboard_image: Option<ClipboardImage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardImage {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextRange {
    #[serde(default)]
    pub reversed: bool,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: TextRange,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    /// Changes only the current buffer's read-only Git diff baseline. None clears it.
    GitDiffSource {
        source: Option<GitDiffSource>,
    },
    /// Saves an image beside the Markdown document and inserts its link.
    InsertImage {
        file_name: String,
        mime_type: String,
        bytes: Vec<u8>,
    },
    ClipboardImage {
        mime_type: String,
        bytes: Vec<u8>,
    },
    Clipboard {
        text: String,
    },
    CreateFile {
        path: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        text: String,
    },
    OpenFile {
        path: String,
    },
    OpenUrl {
        url: String,
    },
    DispatchAction {
        name: String,
        #[serde(default)]
        arguments: serde_json::Value,
    },
    SetIndentation {
        tab_size: u32,
        hard_tabs: bool,
    },
    Notify {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GitDiffSource {
    /// Absolute local Git repository root.
    pub repository: String,
    /// Repository-relative file path; no parent components.
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CommandDefinition {
    pub title: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub arguments: serde_json::Value,
    pub context: ContextRequest,
}

pub const HOST_CAPABILITIES: &[&str] = &[
    "git_diff_source",
    "on_file_open",
    "document",
    "byte_edits",
    "selections",
    "arguments",
    "clipboard_text",
    "clipboard_image",
    "insert_image",
    "create_file",
    "create_file_text",
    "open_file",
    "open_url",
    "dispatch_action",
    "indentation",
    "notify",
    "command_palette",
];

impl CommandOutput {
    /// File hooks cannot mix read-only presentation with editing operations.
    pub fn into_file_presentation(self) -> Result<FilePresentation, String> {
        if !self.replacements.is_empty()
            || !self.edits.is_empty()
            || self.selections.is_some()
            || !self.effects.is_empty()
        {
            return Err("File previews cannot edit documents or execute effects".into());
        }
        let presentation = self
            .file_presentation
            .ok_or("The extension returned no file presentation")?;
        presentation.validate()?;
        Ok(presentation)
    }

    /// Validate all text ranges against UTF-8 bytes and the complete resulting document.
    /// Effects are deliberately not executed here.
    pub fn validate_edits(&self, text: &str) -> Result<String, String> {
        if !self.edits.is_empty() && !self.replacements.is_empty() {
            return Err("Use either edits or replacements, not both".into());
        }
        let mut edits: Vec<_> = self.edits.iter().collect();
        edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
        let mut previous: Option<&TextEdit> = None;
        for edit in &edits {
            if text.get(edit.range.start..edit.range.end).is_none() {
                return Err("Invalid UTF-8 edit range".into());
            }
            if previous.is_some_and(|previous| {
                previous.range.end > edit.range.start || previous.range.start == edit.range.start
            }) {
                return Err("Overlapping edit ranges".into());
            }
            previous = Some(edit);
        }
        let mut result = text.to_owned();
        for edit in edits.into_iter().rev() {
            result.replace_range(edit.range.start..edit.range.end, &edit.text);
        }
        if let Some(selections) = &self.selections {
            if selections.is_empty()
                || selections
                    .iter()
                    .any(|range| result.get(range.start..range.end).is_none())
            {
                return Err("Invalid result selection range".into());
            }
        }
        Ok(result)
    }
}

pub use serde_json;

#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub mod bindings {
    wit_bindgen::generate!({ path: "wit", pub_export_macro: true });
}

/// Export a handler taking a command name and CommandInput, returning CommandOutput or an error.
#[macro_export]
macro_rules! register_editor_commands {
    ($handler:path) => {
        #[cfg(target_arch = "wasm32")]
        mod editor_command_export {
            struct Commands;
            impl $crate::bindings::Guest for Commands {
                fn run_editor_command_v2(command: String, input: String) -> Result<String, String> {
                    let input = $crate::serde_json::from_str(&input).map_err(|e| e.to_string())?;
                    let output: $crate::CommandOutput = $handler(&command, input)?;
                    $crate::serde_json::to_string(&output).map_err(|e| e.to_string())
                }
                fn run_editor_command(command: String, input: String) -> Result<String, String> {
                    Self::run_editor_command_v2(command, input)
                }
            }
            $crate::bindings::export!(Commands with_types_in $crate::bindings);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edit(start: usize, end: usize, text: &str) -> TextEdit {
        TextEdit {
            range: TextRange {
                start,
                end,
                ..Default::default()
            },
            text: text.into(),
        }
    }
    #[test]
    fn validate_utf8_edits_and_result_selections() {
        let mut output = CommandOutput {
            edits: vec![edit(0, 3, "abc"), edit(4, 5, "x")],
            selections: Some(vec![TextRange {
                start: 0,
                end: 3,
                reversed: true,
            }]),
            ..Default::default()
        };
        assert_eq!(
            output.validate_edits("日 a").expect("valid fixture"),
            "abc x"
        );
        output.edits = vec![edit(1, 3, "x")];
        assert!(output.validate_edits("日 a").is_err());
        output.edits = vec![edit(0, 3, "x"), edit(2, 4, "y")];
        assert!(output.validate_edits("abcd").is_err());
        output.edits = vec![edit(0, 0, "x"), edit(0, 0, "y")];
        assert!(output.validate_edits("abcd").is_err());
        output.edits.clear();
        output.selections = Some(vec![]);
        assert!(output.validate_edits("abc").is_err());
    }
    #[test]
    fn protocol_is_additive_and_preserves_v1_output() {
        let input: CommandInput =
            serde_json::from_str(r#"{"selections":["a"],"local_time":"now","future_field":1}"#)
                .expect("valid fixture");
        assert_eq!(input.selections, ["a"]);
        let output = CommandOutput {
            replacements: vec![Some("b".into())],
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&output).expect("valid fixture"),
            serde_json::json!({"replacements":["b"]})
        );
        assert!(serde_json::from_str::<Effect>(r#"{"kind":"unknown"}"#).is_err());
    }
}

#[cfg(test)]
mod file_preview_tests {
    use super::*;
    #[test]
    fn rejects_editing_effects_and_oversized_presentations() {
        let mut output = CommandOutput {
            file_presentation: Some(FilePresentation::default()),
            effects: vec![Effect::OpenFile {
                path: "/other.bin".into(),
            }],
            ..Default::default()
        };
        assert!(output.clone().into_file_presentation().is_err());
        output.effects.clear();
        assert!(output.clone().into_file_presentation().is_ok());
        output.edits.push(TextEdit {
            range: TextRange::default(),
            text: "x".into(),
        });
        assert!(output.into_file_presentation().is_err());
        assert!(
            FilePresentation {
                text: "x".repeat(1024 * 1024 + 1),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            FilePresentation {
                actions: vec![FilePreviewChoice {
                    label: "".into(),
                    action: FilePreviewAction::OpenView
                }],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn file_preview_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let request = FilePreviewInput {
            path: "/sample.bin".into(),
            offset: 16,
            size: 18,
            bytes: vec![0, 255],
            reason: "view".into(),
            options: serde_json::json!({"columns":8}),
        };
        let value = serde_json::to_value(&request)?;
        let decoded: FilePreviewInput = serde_json::from_value(value)?;
        assert_eq!(decoded.bytes, [0, 255]);
        assert_eq!(decoded.offset, 16);
        let output = CommandOutput {
            file_presentation: Some(FilePresentation {
                title: "Custom viewer".into(),
                text: "00 ff".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let output: CommandOutput = serde_json::from_str(&serde_json::to_string(&output)?)?;
        assert_eq!(output.into_file_presentation()?.title, "Custom viewer");
        Ok(())
    }
}
