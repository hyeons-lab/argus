# feat/session-event-fanout

## Agent

Codex

## Intent

Add daemon session event fan-out so attached clients can observe live output, lease changes, snapshots, and exits through the shared session API before any transport-specific adapter is introduced.

## Decisions

- Keep the first subscription surface in-process with `std::sync::mpsc` receivers; Unix socket, WebSocket, TUI, and MCP adapters can translate the same event stream later.
- Emit output and lifecycle events from `SessionActor`, where ordered PTY bytes and terminal snapshots are already serialized.
- Emit lease-change events from `SessionManager`, where input ownership state is maintained.

## What Changed

- Added `SessionApi::subscribe_session_events` and an in-process `SessionEventReceiver` type.
- Added managed actor event subscribers for PTY output, resize snapshots, and completed-session exit events.
- Added manager broadcasts for input lease acquisition, takeover, and release.
- Added daemon coverage for two subscribers observing the same lease and output events, plus snapshot and exit events.

## Validation

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Progress

- 2026-05-13T21:21-0700 — Created `feat/session-event-fanout` worktree from `origin/main` and unset the accidental upstream.
- 2026-05-13T21:38-0700 — Added in-process session event fan-out and validated the workspace.

## Next Steps

- Bind the event stream to the first local API transport after the in-process surface settles.
- Decide whether late subscribers should receive the latest completed-session event or rely on attach snapshots for already-exited sessions.

## Commits

- HEAD — feat: add session event fanout
