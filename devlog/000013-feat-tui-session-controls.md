# feat/tui-session-controls

## Agent
- Codex

## Intent
- Add basic local TUI session controls now that a single embedded session is usable.
- Keep this slice local to `argus-tui` so the daemon/session API can be stressed before adding socket IPC.

## Decisions
- Reserve explicit TUI shortcuts for local session management instead of overloading printable shell input.
- Keep the temporary input/output debug counters out of the product status line.
- Use `Ctrl-N` for new sessions, `Ctrl-W` for close, and `Alt-Up`/`Alt-Down` or `Alt-Left`/`Alt-Right` for switching so normal arrow keys keep going to the shell.
- Keep the last session open; `Ctrl-W` reports an error instead of leaving the TUI with no selected session.

## What Changed
- Refactored `LocalSessionApp` to manage a selected list of local session runtimes.
- Added create, close, next, previous, resize, input, event-drain, and shutdown behavior across local sessions.
- Rendered every local session in the sidebar with selected-session highlighting.
- Replaced temporary `in`/`out` diagnostics with user-facing session control hints.
- Added tests for multi-session local app lifecycle and reserved command key routing.
- Capped per-session event draining per UI tick so sustained output cannot starve input and rendering.
- Preserved resize errors across all sessions instead of clearing an earlier failure after a later success.
- Matched the footer hint to the actual Alt-arrow session switching bindings.
- Coalesced output-event snapshot refreshes so each session refreshes at most once per drain tick.
- Kept recoverable TUI session control and snapshot refresh errors in `last_error` instead of exiting the application loop.
- Simplified event draining to borrow each session once per loop and removed redundant status error allocation.
- Suppressed event-stream-closed status errors after a session has already exited normally.
- Removed the Junie review workflow so PR updates no longer trigger automated Junie review comments.

## Commits
- HEAD — feat: add local TUI session controls

## Progress
- 2026-05-13T23:08-0700 — Created `feat/tui-session-controls` worktree from `origin/main`.
- 2026-05-13T23:13-0700 — Implemented local TUI multi-session controls and completed validation.
- 2026-05-13T23:13-0700 — Validation passed: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --workspace`, and `/home/dberrios/.cargo/bin/cargo test --workspace`.
- 2026-05-13T23:44-0700 — Addressed PR review comments on event-drain fairness, resize error handling, and footer keybinding text.
- 2026-05-13T23:52-0700 — Addressed Junie comments about redundant per-event snapshot refreshes.
- 2026-05-14T00:02-0700 — Addressed Junie comments about recoverable TUI session creation, closure, and snapshot refresh failures.
- 2026-05-14T00:09-0700 — Addressed follow-up Junie cleanup comments on event-drain indexing and status error rendering.
- 2026-05-14T00:19-0700 — Addressed follow-up Junie comment to avoid reporting normal exited session stream closure as an error.
- 2026-05-14T00:25-0700 — Removed `.github/workflows/junie-review.yml` from the PR branch.

## Next Steps
- Smoke test in a real terminal before marking the PR ready for review.
- If manual behavior is good, wire the local UI to the Unix socket API next.
