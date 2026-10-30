# Optional editor commands

This SDK provides an internal WASM API for the custom Zed build. It creates no
TCP listener, Unix socket, HTTP endpoint or CLI request. Calls run through the
existing in-process extension host. Only invoke commands from trusted extensions.

Implement `fn(&str, CommandInput) -> Result<CommandOutput, String>` and export it
with `zed_editor_command_api::register_editor_commands!(crate::handler)`.
The host supports official extension ABI versions 0.6/0.7 and 0.8, subject to
Zed's existing release-channel restrictions. The optional `run-editor-command-v2`
export is preferred. The original export remains available; the host strips v2
input fields when calling an older provider, and ordinary replacement replies
from newer providers retain the v1 shape.

## Registration and configuration

Declare commands in the extension's `extension.toml`:

```toml
[editor_commands.create-companion]
title = "myext0: create companion file"
arguments = { suffix = ".x" }
context = { omit_selection_text = true }
```

A nonempty title creates a command-palette entry on the next palette opening.
An empty title supplies defaults for an existing native alias or keybinding
without adding a palette entry. Reloading/unloading the extension replaces/removes
its entries. Commands execute in an editor context, including an unsaved document
without a project. File operations require a local project and workspace.

Bind any command without creating a native action:

```json
["editor::ExecuteExtensionCommand", {
  "extension": "myext0",
  "command": "format-document",
  "arguments": {"style": "compact"},
  "context": {"document": true, "clipboard": false}
}]
```

Null/omitted arguments and context use the manifest defaults. An explicit object
replaces those defaults. Adding commands or changing their algorithms, arguments,
titles, or supported effect combinations requires only an extension rebuild/reload.

## Input

- `selections`: selected strings in editor order, including empty cursors.
  `context.omit_selection_text` replaces them with empty strings without changing
  their count. This is useful for date insertion or path-only commands.
- `local_time`: invocation time, RFC 3339 with the host's local UTC offset.
- `arguments`: arbitrary JSON supplied by the manifest or keybinding.
- `context`: selection ranges, read-only state, optional local UTF-8 file path,
  language and visible local project roots. No disk path is provided for an
  untitled document, multibuffer, remote file or non-UTF-8 path.
- `context.document`: full snapshot text, only when `context.document = true`.
- `context.clipboard`: clipboard text, only when `context.clipboard = true`.
- `context.clipboard_image`: encoded image MIME type and bytes, only when
  `context.clipboard_image = true`. Image data is transported without decoding.
- `host_capabilities`: supported operations. Check this before relying on newer
  functionality. Future additive context fields may be ignored by older SDKs.

Ranges use UTF-8 **byte offsets**, not UTF-16 or character counts. `start <= end`;
`reversed` indicates that the selection head is at the start. The text is the
editor snapshot, which may include multiple excerpts, rather than disk contents.

## Output

All fields default to empty/absent; return an error to reject the whole reply.

- `replacements`: one nullable string per input selection, or an empty list for
  no replacements. Null/identical replacements preserve that selection.
- `edits`: arbitrary `{range, text}` replacements in the original snapshot.
  Do not combine these with `replacements`. Ranges must be valid UTF-8 boundaries
  and must not overlap; duplicate insertion positions are rejected.
- `selections`: optional resulting selection ranges, measured **after** editing.
- `effects`: ordered operations, described below.

The host rejects stale document/excerpt/selection snapshots, malformed replies,
invalid ranges, unknown effects, and text edits in read-only editors. All text
edits use one Undo transaction. Explicit resulting selections override the normal
collapse-after-replacement behavior. Copying and other nonediting operations can
run in read-only editors.

## Effects

Each effect has a `kind` plus these fields:

| kind | fields | behavior |
| --- | --- | --- |
| `clipboard` | `text` | Write clipboard text. |
| `clipboard_image` | `mime_type`, `bytes` | Write encoded image data in a supported format. |
| `create_file` | `path`, optional `text` | Create a file with initial UTF-8 text (default empty); preserve an existing file, reject a directory. |
| `open_file` | `path` | Open a file in the current workspace. |
| `open_url` | `url` | Open an absolute URL with the existing OS URL handler. |
| `dispatch_action` | `name`, optional JSON `arguments` | Run an existing Zed action in the window, including parameterized actions. |
| `set_indentation` | positive `tab_size`, `hard_tabs` | Change indentation of the invoking single-file editor. |
| `notify` | `message` | Show a workspace toast. |

File paths must be absolute UTF-8 paths. Derive sibling paths from `context.path`;
use project roots for project-relative operations. Create-file does not overwrite
content, create parent directories, save dirty buffers, or delete anything.
Initial text requires the `create_file_text` host capability. The file is written
and flushed before subsequent effects run. A following `open_file` and
`dispatch_action` can open it and position the cursor. Existing contents are
never replaced, including when the same command is repeated.

All synchronous validation precedes mutations. Text edits happen first; effects
then execute in list order. Effects are **not atomic**: an I/O failure does not
roll back preceding text edits or effects. File/clipboard operations are outside
text Undo. Action dispatch happens outside the editor's entity update to avoid
recursive updates. A dispatched action may change focus or close an editor, so
order effects accordingly. Text edits, result selections and `set_indentation` target the invoking editor; dispatched actions target the window’s current focus.

For exact-kind LSP transformations, dispatch `editor::ExecuteCodeAction` with
`{"kind":"refactor.rewrite.example", "select_lines":false}`. It requires one
selection in a project-backed single-file editor. It rejects ambiguous results
and stale documents, applies edits through normal Undo, and never chooses an
unrelated action by candidate order. The optional `select_lines` flag restores a
whole-line result selection, as used by the existing VS Code sort aliases.

The SDK intentionally does not expose GPUI entities, rendering hooks, native file
watchers, or an external automation server. Existing extension process APIs retain
their existing permission model. New rendering/search/watcher primitives can still
require host changes; this interface covers command-driven editing workflows.

## Development and validation

This directory is the SDK source of truth. `zed-myext0` vendors its `src/` and
`wit/` so it builds without a full Zed checkout. Keep the copies in sync.

Test SDK range/protocol validation, the extension's Rust tests and WASM build,
editor `test_personal_` / `test_insert_date`, and command palette
`test_extension_commands_in_palette`. With `ZED_EDITOR_COMMAND_TEST_WASM` pointing
to a built provider, explicitly run the ignored extension-host test
`test_optional_editor_command_wasm` to check the actual WASM boundary.

## User-configured palette commands

`command_palette.commands` maps display titles to `[action_name, arguments]` in
user settings. This allows personal paths and command arguments to stay outside
the extension manifest. Entries are rebuilt each time the palette opens; invalid
actions are logged and omitted. A configured entry replaces the default palette
entry for the same action and arguments, so native actions can also be renamed
without changing action identifiers or key bindings. Multiple configured titles
for the same action remain available. For example:

```json
{"command_palette":{"commands":{"notes: new note":["editor::ExecuteExtensionCommand",{"extension":"myext0","command":"create-timestamp-note","arguments":{"directory":"/absolute/notes/"}}]}}}
```

## Markdown clipboard images

`insert_image` capability adds the `InsertImage` effect (`kind: "insert_image"`):
`file_name`, `mime_type` and encoded `bytes`. It saves a sibling of the saved local
Markdown document and inserts its Markdown image snippet. Directory separators,
empty data, unknown formats and read-only editors are rejected. The host keeps
its existing filename-collision suffixes and Markdown URL escaping. Saving is
asynchronous, reports failures in the UI, and does not form an atomic transaction
with the other effects. Image creation follows the link's buffer transaction:
Undo removes the new image, and Redo restores it without overwriting another
file. Modified or replaced images are left untouched and an error is shown.
Undo/Redo requests are serialized per image; history lives until the buffer closes.

When exactly one loaded extension declares `editor_commands.paste-markdown-image`,
the Markdown image-paste handler invokes it with its manifest arguments/context.
`zhack.markdown.image_paste.filename_template` overrides the `template` argument
for the current file using normal user/project settings resolution. It is read
at each paste, so settings changes need no extension reload.
Declare `context = { clipboard_image = true, omit_selection_text = true }`.
The extension owns filename formatting; the host owns saving and snippet insertion.
With no unique provider, native paste behavior is retained. This hook adds no
socket or external endpoint and does not affect ordinary text pastes.

## File-open hooks and Git diff sources

`zhack.on_file_open` is an array of `{extension, command, arguments, context}`
entries, with arguments and context optional. It invokes the existing command
API for saved local single-file editors, including after a rename. Context
defaults come from the command's manifest, and document/clipboard text remain
opt-in. Commands wait for their extension to load. They run once per file,
hook definition and loaded provider instance; editing settings or reloading
an extension can therefore update behavior without rebuilding Zed. Hook
removal does not reverse previously applied effects. Hooks can use any normal
command output; text edits still require an unchanged document and selection.

The `git_diff_source` capability adds a metadata-only effect:

```json
{
  "effects": [{
    "kind": "git_diff_source",
    "source": {"repository": "/notes/private-history", "path": "note.private.md"}
  }]
}
```

`source: null` restores the buffer's ordinary Git baseline. The repository must
be an absolute existing local Git root. The file path must contain only normal
relative path components, without `..`. The host loads that repository without
adding a visible workspace folder and reads HEAD and index content through the
normal Git backend/watchers. Current buffer contents, including unsaved edits,
are compared with those bases. The original path, saving, Git ownership and
ignore rules do not change. These diffs expose no stage/unstage/restore controls,
and hunk index-write entry points reject them. No repository is initialized and
no remote is contacted.

Outputs containing only Git diff source effects remain valid if text or cursor
positions changed while the command ran, provided the editor and file identity
are unchanged. This lets automatic hooks initialize a baseline without racing
the user's first keystrokes. All other outputs retain normal stale-result checks.
The override lives with the buffer and is cleared on rename/close. It is not a
filesystem redirection or a public IPC API. Source-selection policy belongs in
the extension; the host has no directory-pair or filename-pattern rules.
