# Zed (myhack0)

Personal fork of [Zed](https://github.com/zed-industries/zed) with `myhack0::*` actions for editing, indentation, and command history.

- [Usage and keybindings](https://github.com/wataash/zed-myext0#native-actions-custom-zed-build)
- [Build](#rebuild) and [installation / rollback](#install)
- Licenses: [GPL-3.0-or-later](LICENSE-GPL), [Apache-2.0](LICENSE-APACHE) where marked.

## HACK一覧

上流 v1.21.0 に対する独自変更。アクション名はキーバインド設定で使用する。

| 機能・アクション | 内容 |
| --- | --- |
| `myhack0::CalculateSelection` | 選択した数式を計算し、入力の精度に応じて丸めた結果に置換する。複数選択に対応。 |
| `myhack0::PreciseCalculation` | 選択した数式を計算し、追加の丸めをせず結果に置換する。 |
| `myhack0::UniqSelection` | 選択範囲の重複行を、最初に出現した順序を保って除去する。 |
| `myhack0::SortLinesDescending` | 選択行を大文字・小文字を区別して降順にソートする。 |
| `myhack0::InsertClipboardFileUri` | クリップボードの文字列に `file://` を付けて挿入する。 |
| `myhack0::CopyCurrentCodeBlock` | カーソル位置の Markdown コードブロックの内容を、フェンスを除いてコピーする。 |
| `myhack0::OpenDirectoryInTilix` | 現在のローカルファイルのディレクトリを Tilix で開く。ファイルがなければプロジェクトの先頭のルートを使う。 |
| `myhack0::SetIndentation` | 現在のバッファのインデント幅とタブ使用を変更する。引数は `tab_size` と `hard_tabs`。本文や設定ファイルは変更せず、バッファを閉じるまで有効。 |
| `myhack0::InsertDate` | ローカル日時から `YYYY-MM-DD 曜日`（英語の略称）を挿入する。 |
| `myhack0::InsertDateTime` | ローカル日時から `YYYY-MM-DD 曜日 HH:MM:SS` を挿入する。 |
| `myhack0::ShowCommandHistory` | アクションの実行要求を記録し、回数と直近200件の履歴を表示する。ハンドラーがないアクションも記録対象。 |
| `myhack0::ExportCommandHistory` | コマンド履歴を JSONL にエクスポートして開く。 |
| `myhack0::ConfigureCommandHistory` | 履歴の有効・無効と除外アクションを設定する。変更の反映には Zed の再起動が必要。 |
| Markdown 画像の貼り付け | 画像名を `文書名.YYMMDDhhmmss.拡張子`（UTC）にする。同名があれば `_1` などを付けて上書きを避け、リンク先の特殊文字をエスケープする。 |
| Ctrl+P の絶対パス検索 | プロジェクト外のファイルも絶対パスから候補に表示して開けるようにする。`~/` で始まるパスはホームディレクトリからの絶対パスとして扱う。プロジェクトパネルのルートは追加しない。 |
| Ctrl+P の相対パス完全一致 | ワークツリーのルートからの相対パスが既存ファイルと完全一致すれば、ignore されて未読み込みのディレクトリ内でも候補に表示する。`include_ignored` が `"indexed"` のときは表示しない。 |
| `file_finder.include_ignored: "zall"` | Ctrl+P を開くたびに、未読み込みの ignore 済みディレクトリをバックグラウンドで走査して候補に加える（最大20万ファイル）。ワークツリーとプロジェクトパネルは変更しない。設定の移行処理も `"zall"` を有効な値として扱う。 |
| `file_finder.exclusions` / `file_finder::ExcludeSelected` | glob に一致するパスを Ctrl+P の候補と履歴からだけ除外し、`"zall"` の走査対象からも外す。`shift-backspace` で選択中のファイルを `**/<パス>` としてグローバル設定に追記する。 |
| `file://` リンクを開く | Ctrl+クリックや `editor::OpenUrl` で、OS の既定アプリへ渡さず現在のワークスペースで開く。パーセントエンコードされたパスと行番号フラグメントに対応。 |
| ワークスペースの色 | 先頭のプロジェクトルートの絶対パスから安定した色相を決め、タイトルバーを着色する。リモートでは接続種別とホストも含める。ファイル・ブランチ切り替えでは変色せず、非アクティブ時も色相を維持する。明るさ・透明度はテーマに従う。`title_bar.workspace_color: false` で無効化できる。 |

コマンド履歴の設定は `~/.config/wataash/zed/command-history/config.json`、記録は `~/.local/share/wataash/zed/command-history/` に保存する。`XDG_CONFIG_HOME` / `XDG_DATA_HOME` が指定されていればそちらを使う。既定では記録が有効で、`excluded_actions` にはアクション名や `vim::*` のような名前空間を指定できる。

## Custom build

The checkout at `~/fork/zed/` uses branch `main` of `https://github.com/wataash/zed.git` (`origin`), with the official repository as `upstream`. It is based on the official `v1.21.0` tag. `crates/zed/RELEASE_CHANNEL` is `stable`; the Cargo build profile independently controls compiler optimization.

Keys are maintained in [zed-myext0/config/keymap.json](https://github.com/wataash/zed-myext0/blob/main/config/keymap.json).

## Rebuild

Install `mold` and the system dependencies listed by Zed's `script/linux` and the Rust toolchain pinned in `rust-toolchain.toml`. The build and test commands below work in fish and bash. License generation may download `cargo-about` and license metadata; the first build also downloads Cargo dependencies and native build assets.

```sh
cd ~/fork/zed/
# Keep custom binaries from being replaced by an official automatic update.
# Use the same environment for tests and the application build.
nice -n 15 ionice -c 3 script/generate-licenses
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo test --profile release-quick --config .cargo/release-quick.toml --locked -j 4 --config profile.release-quick.package.editor.opt-level=0 --config profile.release-quick.package.editor.codegen-units=16 -p editor --lib -- test_set_indentation test_personal_ selection_tools::tests test_insert_date test_tilix_directory clipboard::tests:: test_paste_image test_paste_multiple_images
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo test --profile release-quick --config .cargo/release-quick.toml --locked -j 4 -p language --lib test_indentation_override
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo test --profile release-quick --config .cargo/release-quick.toml --locked -j 4 -p gpui --lib test_action_dispatch_observer
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo test --profile release-quick --config .cargo/release-quick.toml --locked -j 4 --config profile.release-quick.package.zed.opt-level=1 --config profile.release-quick.package.zed.codegen-units=16 -p zed --bin zed command_history::tests
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo test --profile release-quick --config .cargo/release-quick.toml --locked -j 4 -p file_finder --lib
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo test --profile release-quick --config .cargo/release-quick.toml --locked -j 4 -p migrator --lib test_make_file_finder_include_ignored_an_enum
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo test --profile release-quick --config .cargo/release-quick.toml --locked -j 4 --config profile.release-quick.package.editor.opt-level=0 --config profile.release-quick.package.editor.codegen-units=16 -p editor --lib file_url
time ZED_UPDATE_EXPLANATION='Fetch and rebuild the custom Zed checkout to update.' CC=clang CXX=clang++ nice -n 15 ionice -c 3 cargo build --profile release-quick --config .cargo/release-quick.toml --locked -j 4 -p zed -p cli
```

The editor tests cover selection editing, Undo/Redo, unchanged selections, date insertion, indentation, and Tilix directory selection. The other tests cover buffer-local indentation, action dispatch observation, command history persistence and configuration, file finding, settings migration, and opening file URLs in the current workspace. `CC=clang CXX=clang++` matches upstream Linux CI, which builds the C++ in `webrtc-sys` only with clang. `.cargo/release-quick.toml` defines the `release-quick` profile (`release-fast` with limited debug info and incremental compilation) and links with mold instead of the default `rust-lld`. The test-only optimization overrides reduce compilation cost. Outputs are in `target/release-quick/`. Keep the profile and build environment consistent to reuse cached artifacts. Four Cargo jobs, CPU niceness 15, and idle I/O priority reduce desktop contention; individual compiler/linker processes can still use substantial resources.

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

Each snapshot contains the **newly installed build**, so rollback means choosing a snapshot from an earlier successful installation. Rebuilding only changes `target/`; it does not overwrite installed applications or snapshots. Before replacing an older installation that has no snapshot, preserve it separately with `cp -a`.

To roll back, quit Zed and replace `yymmdd.HHMM` below with the desired earlier snapshot's timestamp. The destination must be removed first so `cp` restores the application directory rather than nesting it. `trash` must be installed. These commands work in fish and bash:

```sh
test -x ~/.local/zed-wataash.app.bak.yymmdd.HHMM/bin/zed && test -x ~/.local/zed-wataash.app.bak.yymmdd.HHMM/libexec/zed-editor && trash ~/.local/zed-wataash.app/ && cp -a ~/.local/zed-wataash.app.bak.yymmdd.HHMM/ ~/.local/zed-wataash.app/
~/.local/bin/zed
```

Also update `~/.local/share/applications/dev.zed.Zed.desktop`: `TryExec` and both `Exec` entries should use `~/.local/zed-wataash.app/bin/zed`, and `Icon` should use `~/.local/zed-wataash.app/share/icons/zed.png`. Expand `~` to the absolute home path in the desktop file. Merge `~/src/zed-myext0/config/keymap.json` into the active Zed keymap and its synchronized copy.

Official binaries stay in `~/.local/zed.app/`. The two builds share the existing Zed settings and extensions. Quit and restart Zed after switching builds.

To use the official application again, remove or rebind the `myhack0::*` keybindings and launch `~/.local/zed.app/bin/zed`. The extension provides syntax highlighting and a Hello World action, but no equivalent editing Code Actions. Quit the running Zed first: both builds use the stable application identity, so an already running instance may receive the launch request.

Command history files under `${XDG_CONFIG_HOME:-~/.config}/wataash/zed/command-history/` and `${XDG_DATA_HOME:-~/.local/share}/wataash/zed/command-history/` are only written by the custom build; the official build ignores them. Renaming actions does not touch these files: earlier events keep the names they were recorded under, and new events use `myhack0::*` (see [usage documentation](https://github.com/wataash/zed-myext0#command-history)).

## Updates and AI

Merge or rebase upstream stable releases onto `main`, resolve any conflicts, then run the tests and rebuild. Keep the local changes squashed into a few commits on top of the release tag. Do not enable official automatic updates for the custom installation.

Edit Prediction uses the normal provider configuration and authentication. This patch does not change AI providers, service URLs, authentication, or request formats. Actual service connectivity must be checked after launch; editor tests do not verify it.
