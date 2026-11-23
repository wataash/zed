# Rust

- Prefer existing files unless adding a new logical component. Use `src/name.rs`, not `src/name/mod.rs`; for new crates, prefer a descriptive `[lib] path` over `lib.rs`.
- Avoid `unwrap()` and unchecked indexing. Propagate fallible results with `?`, log with `.log_err()`, or handle explicitly; do not silently discard them with `let _ =`. Surface async failures in the UI.
- Comments should explain non-obvious reasons, not restate the code. Use full words in variable names.
- Clone captures into a shadowed binding immediately around the async closure that needs them.

# GPUI pitfalls

- Parameter order is `window`, `cx`, then callbacks. `Context<T>` dereferences to `App`; async contexts may cross await points.
- Inside entity update closures, use the inner `cx`. Updating an entity recursively panics. Use `WeakEntity<T>` to avoid reference cycles; accessing a dropped entity returns an error.
- Entity access and rendering stay on the foreground thread. Use `cx.spawn(async move |cx| ...)`, or `cx.spawn(async move |this, cx| ...)` from `Context<T>` (`this` is weak). Use `cx.background_spawn(async move { ... })` for background work.
- Dropping a `Task` cancels it: await, retain, or explicitly detach it (`detach_and_log_err(cx)` for fallible work). Dropping a `Subscription` unregisters it; retain subscriptions that must stay active.
- Call `cx.notify()` after state changes affecting rendering; this also triggers entity observers.
- Use `cx.listener` for handlers that update the current entity. Define data-free actions with `actions!`, otherwise derive `Action`; action doc comments are user-visible.
- `RenderOnce` consumes `self` and takes `&mut App`, unlike `Render`, which takes `&mut self` and `&mut Context<Self>`.
- In GPUI tests, use `cx.background_executor().timer(duration).await` (`cx.background_executor.timer(...)` in `TestAppContext`). `smol::Timer` is not tracked by GPUI's scheduler and can break `run_until_parked()` with "nothing left to run".

# Validation

- Use `./script/clippy` instead of `cargo clippy`.

# 長時間処理の結果待ち

- ビルド・リンク・テストなどの長時間処理では、Codex usage を節約するため、モデルを何度も再開してポーリングしない。待機自体と、結果確認のためのモデル実行を区別する。
- 開始前にログの保存先と終了コードの記録方法を用意する。依存する検証はスクリプトで順に実行し、失敗時に停止する。既に通過した段階は、変更や失敗の原因が関係しない限り再実行しない。
- 利用可能なら、完了通知またはツール側の監視ループで結果を待ち、完了・失敗・ユーザーの介入が必要になった時点でモデルに制御を戻す。短い待機ツール呼び出しをモデル側で繰り返す方式は避ける。
- このチャットで開始したビルドは、バックグラウンドシェルまたはツールの実行セッションで終了を待ち、同じエージェントが結果確認・必要な修正・コミット・失敗段階からの再開を行う。終了待ちが必要な間は最終回答でターンを閉じない。別の `codex exec` を起動する監視は使わない。
- ポーリングが必要なら、処理時間の見込みに応じて間隔を延ばす。数十分以上かかる処理では原則5〜10分間隔とし、変化がなければ最大15分まで延ばす。上位指示に待機時間や進捗通知の制約がある場合はそれを守り、可能ならツール側で通知する。
- 変化のないログや同じ進捗説明を繰り返し出さない。モデルで確認する際は、終了状態・新しいエラー・必要なログ末尾だけを取得し、ログ全体や会話全体の再読を避ける。
- 低負荷指定では並列数・CPU・I/O優先度を抑える。出力が止まっただけで再起動せず、想定時間を超えた場合に限ってプロセスの稼働状況を確認する。
- バックグラウンド実行や完了通知が利用できない場合、その制約を明示する。利用できない監視機能や、ターン終了後の自動確認を約束しない。

# Upstream pull requests

- Use an imperative title without conventional commit prefixes or trailing punctuation; a crate prefix is optional.
- For GPUI changes, prefix the release note bullet with `[GPUI]`, including changes used only by library consumers.
- End the body with `Release Notes:`, a blank line, and one `- Added ...`, `- Fixed ...`, `- Improved ...`, or `- N/A` bullet.

# Crash investigation references

- Prompts: `.factory/prompts/crash/{investigate,fix}.md`.
- Tools: `script/sentry-fetch <issue-id>`, `script/crash-to-prompt <issue-id>`.
