# Zed (myhack0)

Personal fork of [Zed](https://github.com/zed-industries/zed) with `myhack0::*` actions for editing, indentation, and command history.

- [Usage and keybindings](https://github.com/wataash/zed-myext0#native-actions-custom-zed-build)
- [Build](#rebuild) and [installation / rollback](#install)
- Licenses: [GPL-3.0-or-later](LICENSE-GPL), [Apache-2.0](LICENSE-APACHE) where marked.

## HACK一覧

上流 v1.26.2 に対する独自変更。アクション名はキーバインド設定で使用する。

| 機能・アクション | 内容 |
| --- | --- |
| `myhack0::CalculateSelection` | 選択した数式を計算し、入力の精度に応じて丸めた結果に置換する。複数選択に対応。 |
| `myhack0::PreciseCalculation` | 選択した数式を計算し、追加の丸めをせず結果に置換する。 |
| `myhack0::UniqSelection` | 選択範囲の重複行を、最初に出現した順序を保って除去する。 |
| `myhack0::SortLinesLikeVsCode` | 選択行を VS Code 互換でソートする。`descending: true` で降順。`myext0-sort` LSP が必要。 |
| `myhack0::InsertClipboardFileUri` | クリップボードの文字列に `file://` を付けて挿入する。 |
| `myhack0::CopyCurrentCodeBlock` | カーソル位置の Markdown コードブロックの内容を、フェンスを除いてコピーする。 |
| `myhack0::OpenDirectoryInTilix` | 現在のローカルファイルのディレクトリを Tilix で開く。ファイルがなければプロジェクトの先頭のルートを使う。 |
| `myhack0::SetIndentation` | 現在のバッファのインデント幅とタブ使用を変更する。引数は `tab_size` と `hard_tabs`。本文や設定ファイルは変更せず、バッファを閉じるまで有効。 |
| `myhack0::InsertDate` | ローカル日時から `YYYY-MM-DD 曜日`（英語の略称）を挿入する。 |
| `myhack0::InsertDateTime` | ローカル日時から `YYYY-MM-DD 曜日 HH:MM:SS` を挿入する。 |
| `myhack0::ShowCommandHistory` | アクションの実行要求を記録し、回数と直近200件の履歴を表示する。ハンドラーがないアクションも記録対象。 |
| `myhack0::ExportCommandHistory` | コマンド履歴を JSONL にエクスポートして開く。 |
| `myhack0::ConfigureCommandHistory` | 履歴の有効・無効と除外アクションを設定する。変更の反映には Zed の再起動が必要。 |
| Markdown 画像の貼り付け | `zhack.markdown.image_paste.filename_template` で命名を設定（既定 `文書名.YYMMDDhhmmss.拡張子`、ローカル時刻）。解釈は拡張が担当。新規画像とリンクをまとめて Undo/Redo し、変更された画像や同名ファイルは保護する。 |
| Ctrl+P の絶対パス検索 | プロジェクト外のファイルも絶対パスから候補に表示して開けるようにする。`~/` で始まるパスはホームディレクトリからの絶対パスとして扱う。プロジェクトパネルのルートは追加しない。 |
| Ctrl+P の相対パス完全一致 | ワークツリーのルートからの相対パスが既存ファイルと完全一致すれば、ignore されて未読み込みのディレクトリ内でも候補に表示する。`include_ignored` が `"indexed"` のときは表示しない。 |
| `myhack0::CreateXFile` | コマンドパレットから現在のローカルファイルの `.x` companion を作成して開く。`music-sessions.md` → `music-sessions.x.md`、拡張子なしなら `notes.x`。新規ファイルは空、既存ファイルは上書きせず開く。既に `.x` 形式なら何もしない。未保存・リモート・複数ファイルのエディターには対応しない。 |
| `zhack.search.all_files: true` | Ctrl+P を開くたびに、未読み込みの ignore 済みディレクトリをバックグラウンドで走査して候補に加える（最大20万ファイル）。ワークツリーとプロジェクトパネルは変更しない。 |
| `zhack.search.exclusions` / `file_finder::ExcludeSelected` | glob に一致するパスを Ctrl+P の候補と履歴、プロジェクト検索の対象から除外し、未読み込みの ignore 済みディレクトリの走査対象からも外す。`shift-backspace` で選択中のファイルを `**/<パス>` としてグローバル設定に追記する。 |
| `zhack.watch_exclusions` | Linux のネイティブ監視から glob に一致するディレクトリと配下を除外する。表示・スキャンは維持する。Git 更新に必要な最小限のディレクトリは監視する。 |
| `zhack.on_file_open` | 保存済みのローカルファイルを開いたときに拡張コマンドを実行する。Git 差分の参照先など、ファイルに応じた動作を拡張側で設定できる。 |
| `file://` リンクを開く | Ctrl+クリックや `editor::OpenUrl` で、OS の既定アプリへ渡さず現在のワークスペースで開く。パーセントエンコードされたパスと行番号フラグメントに対応。 |
| `enable_ai_in_directories` / `disable_ai_in_directories` | ユーザー設定で、指定したディレクトリ（配列）の `disable_ai` を上書きし、AI を有効・無効にする。最も深いディレクトリが優先され、両方に同じディレクトリがあれば無効を優先する。開いているローカルのワークツリーのどれかが無効になるディレクトリの中にあるか、それを含む間は、`.zed/settings.json` の `disable_ai` と同様に全ウィンドウで AI を無効にする。シンボリックリンクはワークツリーのルートだけ実体パスでも判定する。 |
| ワークスペースの色 | 先頭のプロジェクトルートの絶対パスから安定した色相を決め、タイトルバーを着色する。リモートでは接続種別とホストも含める。ファイル・ブランチ切り替えでは変色せず、非アクティブ時も色相を維持する。明るさ・透明度はテーマに従う。`title_bar.workspace_color: false` で無効化できる。 |

コマンド履歴の設定は `~/.config/wataash/zed/command-history/config.json`、記録は `~/.local/share/wataash/zed/command-history/` に保存する。`XDG_CONFIG_HOME` / `XDG_DATA_HOME` が指定されていればそちらを使う。既定では記録が有効で、`excluded_actions` にはアクション名や `vim::*` のような名前空間を指定できる。

## Custom build

The checkout at `~/fork/zed/` uses branch `main` of `https://github.com/wataash/zed.git` (`origin`), with the official repository as `upstream`. It is based on the official `v1.26.2` tag. `crates/zed/RELEASE_CHANNEL` is `stable`; the Cargo build profile independently controls compiler optimization.

### File-open extension hooks and Git diff sources

`zhack.on_file_open` invokes extension editing commands for saved local files.
It runs on open/rename, when hook settings change, and after the configured
extension reloads. Commands wait until their extension loads; unchanged
settings and provider registrations do not rerun them. Document and clipboard
access follow the same opt-in context rules as manually invoked commands.

The `myext0` extension uses this generic hook and the `git_diff_source` effect to
compare `c3/FILE.x.EXT` with `x3/FILE.x.EXT` in the latter's Git repository:

```json
{
  "zhack": {
    "on_file_open": [
      {
        "extension": "myext0",
        "command": "configure-git-diff",
        "arguments": {
          "roots": [{"source": "c3", "repository": "x3", "include": ["*.x.*"]}]
        }
      }
    ]
  }
}
```

Mapping and glob rules live in the extension, not Zed. See its README for
the arguments. The host accepts an absolute local repository root and a
repository-relative file path, or null to clear the override. No repository
is initialized and no remote is contacted. These hooks do not run for remote
projects or multi-buffer editors. Updating mapping logic requires only an
extension rebuild/reload; changing arguments requires only a settings edit.

The gutter compares the current buffer, including unsaved edits, with HEAD and
the index of the history repository. It follows HEAD/index updates there and
uses HEAD even if the normal gutter uses a default-branch diff base. The
original file remains the editing and saving destination; Git ownership and
ignore rules do not change. Hunk staging, unstaging and restore controls are
disabled for these comparison-only diffs. Commit the synchronized file in its
history repository. The history root is loaded without adding a visible
workspace folder. Removing a hook does not undo its earlier effects; return a
null diff source from the extension or reopen the project to clear an override.

The protocol is documented in [the editing-command API](crates/editor_command_api/README.md).
Regression tests: `test_alternate_git_diff_root` (project) and
`test_personal_file_open_hook_git_diff_source` (editor).

Local video file links open in the system's default application, including Ctrl-click and Markdown file links. Relative paths and percent-encoded `file://` URLs are supported. Remote files and text files retain normal editor behavior.

VS Code-compatible sorting uses `myhack0::SortLinesLikeVsCode` with a `descending` boolean (default `false`). Ctrl+M Ctrl+S requests `refactor.rewrite.myext0.sortAscending`; Ctrl+M Ctrl+R requests `refactor.rewrite.myext0.sortDescending`. The implementation in [zed-myext0/server/sort.mjs](https://github.com/wataash/zed-myext0/blob/main/server/sort.mjs) is the comparison source of truth. The bridge requests the exact kind for one selection, rejects ambiguous responses and changed documents, and applies the LSP edit through normal Undo history. It requires a project and the `myext0-sort` language server. Multiple selections are rejected without editing. Built-in sorts and the unbound legacy descending action retain their original comparison rules.

Calculation, unique-line filtering and date/time formatting are implemented by
`zed-myext0` through the [optional editing-command API](crates/editor_command_api/README.md).
Their existing `myhack0::*` actions and keybindings remain aliases. These commands
require the updated extension and custom binary, but work in unsaved documents
without a project or language server. Extension replies are applied as one Undo
transaction only if the document and selections have not changed. Changing the
command implementations requires rebuilding/reloading the extension, not Zed.

The editing-command API also provides optional full-document and clipboard context,
JSON arguments, arbitrary byte-range edits and resulting selections, clipboard
writes, nonoverwriting file creation, file/URL opening, existing action dispatch,
buffer indentation and notifications. Extension `editor_commands` manifest entries
add palette commands without adding native action types. See the SDK specification
for context opt-ins, v1/v2 compatibility, validation and effect ordering.

Keys are maintained in [zed-myext0/config/keymap.json](https://github.com/wataash/zed-myext0/blob/main/config/keymap.json).

Custom settings use the `zhack` object in the user settings:

```json
{
  "file_scan_exclusions": [],
  "zhack": {
    "watch_exclusions": ["**/.git"],
    "search": {
      "all_files": true,
      "exclusions": ["**/node_modules", "**/target"]
    }
  }
}
```

Both exclusion lists default to `[]` and support `"..."` to extend inherited values. `search.all_files` defaults to `false`; when enabled, Ctrl+P initially includes ignored files, and its existing toggle can still turn them off. Search exclusions also apply to history and project search, while `file_scan_exclusions` still takes precedence. The `all_files` and `exclusions` search settings are global, matching the existing custom search behavior. `watch_exclusions` also supports project settings, matches ancestors, and checks resolved symlink targets. External Git directories can be matched with absolute paths or globs such as `"**/.git"`.

Watch exclusions apply only to native Linux watches. For Git repositories, metadata roots and the parent directories of the current symbolic ref chain remain watched even when excluded, so commits and index changes refresh immediately. Branch switches replace the ref watches; unborn branches watch the nearest existing ancestor. A normal repository on `main` needs two directories. Linked worktrees also watch their separate metadata root, and reftable directories are watched when present. Objects, logs, and unrelated refs remain excluded. Recursive watchers on other platforms and polling watchers continue watching as before. Excluded files remain visible, but other external changes may not update automatically. Parent-directory events can still trigger a rescan. Changing the setting restarts the worktree scanner and replaces its watches.

Settings migration moves `file_finder.include_ignored: "zall"` to `zhack.search.all_files: true` and `file_finder.exclusions` to `zhack.search.exclusions`. Explicit values in the new settings take precedence.

### Additional search roots

Set `zhack.search.additional_roots` in a project's `.zed/settings.json` to search
related directories in the same window:

```json
{
  "zhack": {
    "search": {
      "additional_roots": ["../archive/", "../reference/"]
    }
  }
}
```

Paths may be absolute, start with `~/`, or be relative to the worktree containing
these settings. This feature is local-only. The directories are attached as normal
visible worktrees when either search interface is first opened; they remain in the
project for editing, Git status, and subsequent searches. Missing directories show
an error. The initial search excludes these worktrees, including their open files
and file-finder history. Existing `zhack.search.exclusions` apply in both scopes.

Press Ctrl+P again while the file finder is open, or Ctrl+Shift+F again while the
project search input is focused, to toggle between normal roots and all roots.
The query and search options are retained, and the included root names appear in
the search interface. There is no double-tap timeout or shortcut prefix delay.
Closing and reopening the finder/search starts with normal roots. Returning from
search results to the query only focuses the input; the next press changes scope.
Projects without additional roots retain their existing shortcut behavior.

### Capturing search UI tests

The optional `visual-test-capture` feature for `file_finder` and `search` renders
the real test windows using WGPU, system text shaping, and Zed's assets. Set
`ZED_TEST_CAPTURE_DIR` to save PNGs at normal, expanded, and reset scopes in
`test_additional_search_roots_toggle`. Enable both features when testing both crates:
`--features file_finder/visual-test-capture,search/visual-test-capture`.
This compiles test binaries, not the Zed application. Normal tests do not initialize
a renderer or load system fonts. Captures use fixture files rather than personal
notes. A video assembled from these PNGs shows the tested states, rather than a
real-time desktop recording.

## Rebuild

Install `mold` and the system dependencies listed by Zed's `script/linux` and the Rust toolchain pinned in `rust-toolchain.toml`. The Linux runner below works in fish and bash. License generation may download `cargo-about` and license metadata; the first build also needs Cargo dependencies and native build assets.

```sh
cd ~/fork/zed/
# Generate licenses and build the application and CLI using cached dependencies.
script/low-load
# Run relevant tests, clippy, license generation, and the application build in order.
script/low-load all
# Inspect commands without compiling, or resume explicitly at a failed phase.
script/low-load --dry-run all
script/low-load --from clippy
# Build alone, inspect the last run, or allow dependency downloads.
script/low-load build
script/low-load --status
env ZED_LOW_LOAD_OFFLINE=0 script/low-load all
# Override resource limits when needed (CPU numbers must be allowed by taskset).
env ZED_LOW_LOAD_JOBS=2 ZED_LOW_LOAD_CPUS=12,13 script/low-load build
# From another terminal, adjust the latest active runner in this checkout.
script/low-load --load high
script/low-load --load medium
script/low-load --load low
# Automatic control: enable on a new build, or attach from another terminal.
script/low-load --auto all
script/low-load --load auto
```

For builds started in an agent chat, the default is to wait for completion in a background shell or tool execution session within the same turn. The same agent then checks the result, fixes and commits any necessary changes, and restarts from the failed phase. Keep the turn open while waiting; after a final response, a shell cannot automatically resume that chat. Avoid repeated log reads and unchanged progress messages.

The runner defaults to a fixed Cargo job count from `nproc` at startup (before applying CPU limits), the last two CPUs in its allowed affinity, niceness 19, idle I/O priority, and one test thread. `ZED_LOW_LOAD_JOBS` overrides the fixed job count. It runs phases sequentially, stops on failure, and prevents concurrent copies in the same checkout. Logs, phase exit codes, and final status are saved under `${XDG_DATA_HOME:-~/.local/share}/wataash/zed-low-load/`; `ZED_LOW_LOAD_DATA_DIR` overrides this location. `--status` reads the latest run without polling. `--from` reruns the named phase and every later phase; it does not infer whether cached results still apply. Use `--help` to list phase names. The runner does not install binaries.

`--load low|medium|high` changes CPU affinity immediately for the active runner and all its existing descendants and threads. `low` uses up to two originally allowed CPUs, `medium` up to four, and `high` all of them. New processes inherit the changed affinity. Cargo's job count remains fixed across all load levels and phases, so increasing the CPU allocation lets already running compiler jobs use the extra CPUs immediately without restarting Cargo. Dependency bottlenecks can still limit concurrency. The large job count can consume substantial memory even at `low`; reduce `ZED_LOW_LOAD_JOBS` at startup if needed. CPU niceness 19 and idle I/O priority remain constant, allowing repeated increases and decreases without root privileges. The controller verifies the checkout and runner PID start time before changing processes, rejects expired runs, and records changes in `load-changes.tsv`. This requires a run started with the current runner; older ad-hoc build scripts do not have its control metadata. Existing runs retain their original job count.

Automatic control requires Python 3.10+ (standard library only), `gdbus`, GNOME's `org.gnome.Mutter.IdleMonitor` session-bus interface, and Linux PSI. `--auto` starts the monitor with a new runner. `--load auto` attaches a foreground monitor to the latest existing runner without restarting Cargo; it stays bound to that run even if another checkout changes `latest`. A manual `--load low|medium|high` disables the monitor and takes precedence. Duplicate monitors are rejected. The monitor exits when its runner ends; stopping it while the runner is active restores `low`. Status and measurements appear in `auto-status.json` and `--status`.

The monitor samples once per second. After **five minutes (300 seconds) without input**, it selects `high`. On input resumption it selects `low` on the next sample. **The default maximum while active is `low`**, because a fast GNOME query can coexist with perceptibly slow desktop operation. Set `--active-max medium` or `--active-max high` explicitly to enable increasing load while active. Within that cap, two consecutive response delays of at least 100 ms lower the load by one level; 30 seconds of responses below 50 ms raise it by one level. Responses between these thresholds reset recovery. These are initial tuning values, not universal perceptual thresholds. The delay measurement includes the GNOME idle-query round trip (including process startup) and monitor scheduling lateness. It is a responsiveness proxy, **not a Sysprof frame-time measurement**, and cannot reliably detect GPU or application-specific stalls. Sysprof remains useful for correlating the proxy with actual frame timing. Idle time also includes reading or watching videos without input.

Memory PSI `some avg10 >= 1%`, I/O PSI `some avg10 >= 5%`, or available memory below 10% force `low`, even while idle. Missing measurements or an unavailable GNOME session also force `low`; they are not interpreted as idle. These guards only limit CPU affinity and do not impose a memory quota or reduce the number of already running compiler jobs. For tuning, invoke `python3 script/low-load-auto.py -q monitor --run-directory PATH --idle-seconds 300 --interval 1 --latency-ms 100 --recovery-seconds 30`; `--dry-run` prints the external commands without invoking them. Tests: `python3 script/test_low_load_auto.py` and `script/test-low-load-auto` (mocked measurements and compiler, no real build).

The editor tests cover selection editing, Undo/Redo, unchanged selections, date insertion, indentation, and Tilix directory selection. The other tests cover buffer-local indentation, action dispatch observation, command history persistence and configuration, file finding, file finder exclusions in project search, settings migration, worktree settings and watch exclusions, disabling AI by directory, and opening file URLs in the current workspace. `CC=clang CXX=clang++` matches upstream Linux CI, which builds the C++ in `webrtc-sys` only with clang; existing compiler environment overrides are respected. The runner also sets `ZED_UPDATE_EXPLANATION` to prevent official automatic updates from replacing the custom binary. `.cargo/release-quick.toml` defines the `release-quick` profile (`release-fast` with limited debug info and incremental compilation) and links with mold instead of the default `rust-lld`. The test-only optimization overrides reduce compilation cost. Clippy uses `./script/clippy`, with a Cargo adapter replacing its forced `--release` with `release-quick`. Outputs are in `target/release-quick/`. Keep the profile and build environment consistent to reuse cached artifacts. The CPU limits also apply to compiler/linker subprocesses, but do not limit their memory use.

These builds can be run with `sbx`, granting write access only to the checkout and Cargo/Rust toolchain directories, read access to `/etc/alternatives/` for compiler symlinks, and network access for dependency downloads. Set `CARGO_HOME`, `RUSTUP_HOME`, and `PATH` explicitly because `sbx` clears the environment and uses a temporary home directory.

## Install

After the build and relevant tests succeed, install the new build in both the active directory and a timestamped snapshot. Run this block in **bash** (enter `bash` first if using fish):

```bash
(
set -eu
cd ~/fork/zed/
# Use the local time at installation, and never overwrite an existing snapshot.
zed_snapshot="$HOME/.local/zed-wataash.app.bak.$(date +%y%m%d.%H%M)"
if [ -e "$zed_snapshot" ] || [ -L "$zed_snapshot" ]; then
    echo "Snapshot already exists: $zed_snapshot" >&2
    exit 1
fi
install -D -m 755 target/release-quick/cli ~/.local/zed-wataash.app/bin/zed.new
install -D -m 755 target/release-quick/zed ~/.local/zed-wataash.app/libexec/zed-editor.new
mv -f ~/.local/zed-wataash.app/bin/zed.new ~/.local/zed-wataash.app/bin/zed
mv -f ~/.local/zed-wataash.app/libexec/zed-editor.new ~/.local/zed-wataash.app/libexec/zed-editor
install -D -m 644 crates/zed/resources/app-icon.png ~/.local/zed-wataash.app/share/icons/zed.png
cp -a ~/.local/zed-wataash.app/ "$zed_snapshot/"
mkdir -p ~/.local/bin
ln -sfn ~/.local/zed-wataash.app/bin/zed ~/.local/bin/zed
printf 'Installed snapshot: %s\n' "$zed_snapshot"
)
```

Each snapshot contains the **newly installed build**, so rollback means choosing a snapshot from an earlier successful installation. Rebuilding only changes `target/`; it does not overwrite installed applications or snapshots.

To roll back, quit Zed and replace `yymmdd.HHMM` below with the desired earlier snapshot's timestamp. The destination must be removed first so `cp` restores the application directory rather than nesting it. `trash` must be installed. These commands work in fish and bash:

```sh
test -x ~/.local/zed-wataash.app.bak.yymmdd.HHMM/bin/zed && test -x ~/.local/zed-wataash.app.bak.yymmdd.HHMM/libexec/zed-editor && trash ~/.local/zed-wataash.app/ && cp -a ~/.local/zed-wataash.app.bak.yymmdd.HHMM/ ~/.local/zed-wataash.app/
~/.local/bin/zed
```

Also update `~/.local/share/applications/dev.zed.Zed.desktop`: `TryExec` and both `Exec` entries should use `~/.local/zed-wataash.app/bin/zed`, and `Icon` should use `~/.local/zed-wataash.app/share/icons/zed.png`. Expand `~` to the absolute home path in the desktop file. Merge `~/d3/c3/zed-myext0/config/keymap.json` into the active Zed keymap and its synchronized copy.

Official binaries stay in `~/.local/zed.app/`. The two builds share the existing Zed settings and extensions. Quit and restart Zed after switching builds.

To use the official application again, remove or rebind the `myhack0::*` keybindings and launch `~/.local/zed.app/bin/zed`. The extension provides syntax highlighting and a Hello World action, but no equivalent editing Code Actions. Quit the running Zed first: both builds use the stable application identity, so an already running instance may receive the launch request.

Command history files under `${XDG_CONFIG_HOME:-~/.config}/wataash/zed/command-history/` and `${XDG_DATA_HOME:-~/.local/share}/wataash/zed/command-history/` are only written by the custom build; the official build ignores them. Renaming actions does not touch these files: earlier events keep the names they were recorded under, and new events use `myhack0::*` (see [usage documentation](https://github.com/wataash/zed-myext0#command-history)).

## Updates and AI

Merge or rebase upstream stable releases onto `main`, resolve any conflicts, then run the tests and rebuild. Keep the local changes squashed into a few commits on top of the release tag. Do not enable official automatic updates for the custom installation.

Edit Prediction uses the normal provider configuration and authentication. This patch does not change AI providers, service URLs, authentication, or request formats. Actual service connectivity must be checked after launch; editor tests do not verify it.

### Markdown image paste

Configure `zhack.markdown.image_paste.filename_template` in user or project settings:

```json
{"zhack":{"markdown":{"image_paste":{"filename_template":"${documentBaseName}.${localTime:%y%m%d%H%M%S}.${fileExtName}"}}}}
```

The current file's settings are read at each paste and passed to the extension.
Saving settings is sufficient; rebuilding Zed and reloading extensions are not
needed after installing a host with this setting. Template variables are described
in the `zed-myext0` README, "Markdown image filenames". Missing settings use the
extension's default. Without a provider, native naming uses local time.

Image pastes are isolated Undo transactions. Undo removes their newly created
files, and Redo restores them without overwriting existing paths. Images modified
or replaced since creation are preserved and an error is reported. Filesystem
operations are asynchronous and serialized per image. History is shared by views
of the buffer, kept in memory, and discarded when that buffer closes. This is not
a general cleanup of unreferenced images and does not undo unrelated file effects.
Tests: `editor_tests::test_personal_extension_image_paste` and `test_paste_image*`.
