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
    /// One replacement per input selection. None leaves that selection untouched.
    pub replacements: Vec<Option<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<TextEdit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selections: Option<Vec<TextRange>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
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
