# feat/session-event-fanout

## Agent

Codex

## Intent

Add daemon session event fan-out so attached clients can observe live output, lease changes, snapshots, and exits through the shared session API before any transport-specific adapter is introduced.

## Decisions

- Keep the first subscription surface in-process with `std::sync::mpsc` receivers; Unix socket, WebSocket, TUI, and MCP adapters can translate the same event stream later.
- Emit output and lifecycle events from `SessionActor`, where ordered PTY bytes and terminal snapshots are already serialized.
- Emit lease-change events from `SessionManager`, where input ownership state is maintained.
- Pin Junie to the documented stable CLI version instead of `latest`; the nightly runner installed by `latest` authenticated but failed before producing its JSON result file.
- Use bounded subscriber queues and disconnect stalled event subscribers instead of allowing unbounded event buffering.

## What Changed

- Added `SessionApi::subscribe_session_events` and an in-process `SessionEventReceiver` type.
- Added managed actor event subscribers for PTY output, resize snapshots, and completed-session exit events.
- Added manager broadcasts for input lease acquisition, takeover, and release.
- Added daemon coverage for two subscribers observing the same lease and output events, plus snapshot and exit events.
- Addressed PR feedback by polling for session exit after output closes, making output-marker tests tolerate split PTY chunks, and asserting snapshot/exit fan-out for both subscribers.

## Validation

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `git diff --check`

## Progress

- 2026-05-13T21:21-0700 — Created `feat/session-event-fanout` worktree from `origin/main` and unset the accidental upstream.
- 2026-05-13T21:38-0700 — Added in-process session event fan-out and validated the workspace.
- 2026-05-13T21:52-0700 — Investigated Junie run `25842326501`; pinned Junie away from nightly after `latest` installed CLI `1699.1` and failed with `Failed to build 'issue.md.junie_standalone'`.
- 2026-05-13T22:06-0700 — Addressed PR feedback around bounded subscriber queues, exit polling after PTY output closes, and fan-out test coverage.

## Next Steps

- Bind the event stream to the first local API transport after the in-process surface settles.
- Decide whether late subscribers should receive the latest completed-session event or rely on attach snapshots for already-exited sessions.

## Commits

- f4083c9 — feat: add session event fanout
- c5e52f1 — ci: pin Junie runner version
- HEAD — fix: address session event fanout review
