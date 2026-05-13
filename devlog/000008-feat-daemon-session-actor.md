# Daemon Session Actor

## Agent

Codex

## Intent

Extend the daemon PTY session core from a one-shot drain helper into a long-running session actor that can accept input, resize, snapshot, and shut down while preserving daemon-owned terminal state.

## Decisions

- Keep the actor inside `argus-daemon` for this branch. The public session API can move into `argus-core` once the command and event shapes are proven locally.
- Use daemon-owned sequence numbers for output ingestion and snapshots so later attach/reconnect APIs have a monotonic ordering primitive.
- Keep input leases and multi-client attach semantics out of scope for this branch; the actor will expose a single control surface that later lease logic can wrap.

## Progress

- 2026-05-12T22:41-0700 — Created `feat/daemon-session-actor` worktree from `origin/main` and unset the accidental upstream.
- 2026-05-12T22:45-0700 — Added a daemon-local session actor handle with PTY input, resize, snapshot, shutdown, and daemon-owned output sequence tracking.
- 2026-05-12T22:45-0700 — Verified the actor against a long-running shell that accepts injected input, updates visible terminal rows, reports resized dimensions, and shuts down cleanly.

## What Changed

- Added `SessionActor` as a daemon-local control surface over a PTY-backed session.
- Routed PTY output through a worker-owned terminal state path that tees raw bytes to the session log and increments an output sequence per ingested chunk.
- Added `SessionSnapshot` with output sequence, logged byte count, size, visible rows, and exit state.
- Kept the previous one-shot `PtySession::drain_until_exit` path available for narrow drain tests.
- Added a live actor test that sends input to a long-running shell, waits for the marker in visible rows, resizes the session, and validates final log bytes on shutdown.

## Validation

- `cargo fmt --all -- --check`
- `cargo test -p argus-daemon`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Next Steps

- Define the shared session API in `argus-core` once the daemon-local actor surface is ready to bind to TUI, web, and MCP transports.
- Add attach modes and input lease/takeover semantics around the actor input path.
- Add Windows-specific ConPTY lifecycle coverage for actor shutdown and EOF behavior.

## Commits

- HEAD — feat: add daemon session actor
