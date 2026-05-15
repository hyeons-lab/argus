# feat/local-unix-socket-api

## Agent
- Codex

## Intent
- Add the first owner-local Unix socket transport for the shared session API.
- Move the local TUI toward daemon-backed sessions without starting the remote WebSocket or MCP surfaces yet.

## Decisions
- Keep the first transport slice focused on the operations the current TUI already needs.
- Preserve the in-process manager path until the socket adapter is proven enough to replace it cleanly.
- Use newline-delimited JSON for the first Unix socket protocol so request/response handling stays inspectable and transport-specific.
- Keep event subscription as the only long-lived socket request; normal session API calls use one connection per call.
- Refuse to bind over a live socket, but remove stale socket files after connection refusal.
- Let the TUI connect to the default daemon socket when available and fall back to the embedded manager when no daemon is running.
- Keep the daemon IPC accept loop alive after individual accept or handler-spawn failures.
- Send periodic subscription heartbeat frames so idle disconnected subscribers are cleaned up on the next heartbeat write.
- Use a user-scoped temp fallback socket directory when `XDG_RUNTIME_DIR` is unset.

## What Changed
- Added `argus_daemon::ipc` with owner-local Unix socket server and client adapters for the existing `SessionApi`.
- Added a daemon binary serve path that binds the default socket at `$XDG_RUNTIME_DIR/argus/argus.sock` or the temp directory fallback.
- Refactored `LocalSessionApp` to own a boxed `SessionApi` backend and added a Unix socket connection constructor.
- Updated the TUI binary to prefer the daemon socket path and keep the current embedded behavior as fallback.
- Added socket transport coverage for server error propagation and a real PTY lifecycle with event streaming.
- Addressed PR feedback on accept-loop resilience, idle subscription cleanup, and temp fallback socket scoping.
- Cleaned up daemon-created TUI sessions when startup fails after `start_session`, and made the IPC event-stream assertion wait deterministically instead of racing the reader thread.

## Commits
- 52a1148 — feat: add local Unix socket session API
- HEAD — fix: clean up TUI daemon startup failures

## Progress
- 2026-05-14T10:00-0700 — Created `feat/local-unix-socket-api` worktree from `origin/main` and unset the accidental upstream.
- 2026-05-14T10:06-0700 — Implemented the first Unix socket session API adapter, wired the daemon serve path and TUI socket preference, and completed validation.
- 2026-05-14T10:06-0700 — Validation passed: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --workspace`, `/home/dberrios/.cargo/bin/cargo test --workspace`, and `git diff --check`.
- 2026-05-14T10:28-0700 — Smoke tested `argus-daemon` and `argus-tui` together with temp runtime/state directories; the first sandboxed TUI run fell back to the embedded manager, then the unsandboxed TUI run connected to the daemon socket and produced a daemon-owned session log at `<tmp>/argus/sessions/session-1.log`.
- 2026-05-14T16:23-0700 — Addressed Copilot PR comments for resilient accept handling, idle subscriber cleanup, and user-scoped temp socket fallback.
- 2026-05-14T16:23-0700 — Validation passed after review fixes: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --workspace`, `/home/dberrios/.cargo/bin/cargo test --workspace`, and `git diff --check`.
- 2026-05-14T17:02-0700 — Fixed critical-review follow-ups for partial TUI daemon startup cleanup and the racy IPC output-event assertion.

## Next Steps
- Decide whether the next slice should make the TUI require the daemon socket or keep embedded fallback as an explicit development mode.
