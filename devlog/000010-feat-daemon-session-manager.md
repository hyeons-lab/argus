# Daemon Session Manager

## Agent

Codex

## Intent

Bind the daemon session actor to the shared `argus-core` session API with daemon-owned session IDs, per-session actors, and input lease enforcement before PTY writes.

## Decisions

- Keep the first manager in-process and synchronous so local IPC/TUI adapters can bind to the same trait later.
- Allocate daemon-local session IDs and log paths inside `argus-daemon`; transports provide commands, attach mode, input, resize, and shutdown requests.
- Gate `write_input` on the current input lease holder instead of giving observers an implicit write path.
- Use the existing `SessionActor` as the execution boundary and keep broadcast/event fan-out out of this slice.

## Progress

- 2026-05-13T20:21-0700 — Created `feat/daemon-session-manager` worktree from `origin/main` and unset the accidental upstream.
- 2026-05-13T20:25-0700 — Added an in-process daemon session manager implementing the shared session API over `SessionActor`.
- 2026-05-13T20:25-0700 — Added daemon coverage for observer write rejection, controller lease acquisition, agent takeover, release, managed snapshot polling, log tee, and shutdown.

## What Changed

- Added `SessionManagerConfig` and `SessionManager` in `argus-daemon`.
- Implemented `SessionApi` for the daemon manager: start, attach, acquire/release lease, write, resize, snapshot, and shutdown.
- Added daemon-owned session ID allocation and per-session log path creation under the configured log directory.
- Enforced input lease ownership before forwarding input bytes to the PTY actor.
- Added a managed-session integration test around attach/write/takeover/release/shutdown behavior.

## Validation

- `cargo fmt --all -- --check`
- `cargo test -p argus-daemon`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `git diff --check`

## Next Steps

- Add event fan-out so output, lease changes, snapshots, and exits can be broadcast to attached clients.
- Bind the manager to the first local API transport after the in-process surface settles.

## Commits

- HEAD — feat: add daemon session manager
