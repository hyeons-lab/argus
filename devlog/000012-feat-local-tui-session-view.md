# feat/local-tui-session-view

## Agent

- Codex

## Intent

- Build the first local TUI surface against the existing in-process `SessionManager`.
- Prove the shared session API from a real client before adding the Unix socket transport.

## Decisions

- Start with an embedded daemon manager in `argus-tui` instead of the Unix socket adapter. This keeps the first UI slice focused on session rendering, event consumption, input lease behavior, and resize handling.
- Treat this branch as a minimal usable terminal client: one managed shell session, a session list/sidebar, visible terminal rows, keyboard input passthrough, and clean terminal teardown.
- Use Esc/Ctrl-Q as local client exit keys and forward regular printable keys, including `q`, to the PTY. Plain `q` is valid shell input and should not be reserved by the client.

## What Changed

- Added Ratatui and Crossterm workspace dependencies for the local terminal client.
- Added `argus-tui` dependencies on `argus-core`, `argus-daemon`, `anyhow`, `crossterm`, and `ratatui`.
- Added a testable local TUI app layer that starts one in-process `SessionManager` shell session, attaches as the interactive controller, subscribes to events, refreshes snapshots, resizes the PTY, forwards input, and shuts the session down.
- Replaced the empty TUI binary with a raw-mode alternate-screen Ratatui interface containing a session sidebar, terminal row view, and status/error footer.
- Added unit tests for event-to-view state updates and key routing.
- Fixed TUI startup order so the managed shell session is spawned before the outer terminal enters raw mode. Spawning after raw mode could leave the child PTY with echo disabled, which made typed input invisible during smoke testing.
- Added a regression test that starts the local app, writes a marker command, drains events, and verifies the marker reaches `visible_rows`.
- Wrapped the Unix shell startup in `stty sane echo; exec "${SHELL:-/bin/sh}"` so the child PTY explicitly restores echo and canonical terminal behavior even when inherited terminal settings are not useful.
- Added a pre-Enter echo regression test, explicit terminal text foreground styling, and footer counters for accepted input bytes and observed PTY output events to diagnose live terminal input/rendering behavior.
- Matched the PTY size to the rendered terminal pane's inner area rather than the full app terminal. The previous sizing let the shell write into rows hidden by the sidebar/footer/borders, so the active prompt/input line could be clipped even while input and output counters advanced.

## Commits

- HEAD — feat: add local TUI session view

## Progress

- 2026-05-13T22:30-0700 — Created the feature worktree from `origin/main`, unset branch upstream, inspected the session API, daemon manager, and empty TUI crate.
- 2026-05-13T22:35-0700 — Implemented the first in-process local TUI session view and validated formatting, type-checking, clippy, workspace tests, and diff whitespace.
- 2026-05-13T22:43-0700 — Investigated smoke-test input not echoing, found the raw-mode startup ordering issue, fixed it, and reran validation.
- 2026-05-13T22:46-0700 — Made Unix shell startup explicitly restore sane echo behavior and reran validation.
- 2026-05-13T22:51-0700 — Added live TUI diagnostics after manual smoke testing still showed no typed text, and verified the app state sees pre-Enter input echo in tests.
- 2026-05-13T22:55-0700 — Fixed PTY sizing to account for footer, sidebar, and terminal borders after live diagnostics showed both input and output were flowing.

## Issues

- `cargo test -p argus-tui` initially needed network access to fetch the new Ratatui/Crossterm dependency graph. After dependency resolution, local validation completed normally.

## Validation

- `cargo fmt --all -- --check`
- `cargo test -p argus-tui`
- `cargo check --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- `git diff --check`

## Next Steps

- Run a manual interactive smoke test in a real terminal and refine viewport/input behavior from observed use.
- Add multi-session creation/selection after the single-session local path is proven.
- Bind this client path to the Unix socket API once the local UI flow settles.
