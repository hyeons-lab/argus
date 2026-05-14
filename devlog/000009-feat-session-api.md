# Session API

## Agent

Codex

## Intent

Define the shared session API in `argus-core` so daemon, TUI, web, and MCP transports can bind to one set of session commands, snapshots, events, attach modes, and input lease semantics.

## Decisions

- Keep PTY handles, terminal engine state, raw logs, actor threads, and shutdown mechanics in `argus-daemon`.
- Put transport-neutral identifiers, sizes, snapshots, completed-session summaries, attach modes, lease state, events, and trait contracts in `argus-core`.
- Model input control as an explicit lease with observer-only attach as the default behavior.

## Progress

- 2026-05-13T19:14-0700 — Created `feat/session-api` worktree from `origin/main` and unset the accidental upstream.
- 2026-05-13T19:19-0700 — Added `argus-core::session` with shared session identifiers, size/snapshot/completion types, attach modes, input lease state, session events, request/response structs, and the synchronous session API trait.
- 2026-05-13T19:19-0700 — Updated `argus-daemon` to reuse the shared `SessionSize`, `SessionSnapshot`, and `CompletedSession` types while keeping PTY-specific config and conversion helpers local.

## What Changed

- Added transport-neutral session IDs, client IDs, session sizing, snapshots, completion summaries, attach modes, input controller kinds, lease state transitions, and session events in `argus-core`.
- Added request/response structs and a synchronous `SessionApi` trait covering start, attach, input lease, write, resize, snapshot, and shutdown operations.
- Added unit coverage for observer defaults, input lease acquire/takeover/release behavior, session size validation, and start request validation.
- Re-exported `argus_core::session` and removed duplicate daemon-local definitions for shared size/snapshot/completion types.

## Validation

- `cargo fmt --all -- --check`
- `cargo test -p argus-core`
- `cargo test -p argus-daemon`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Next Steps

- Bind the daemon-local `SessionActor` to the shared `argus-core` session API.
- Add a daemon session manager that owns multiple actors by `SessionId` and applies input lease decisions before writing PTY input.

## Commits

- HEAD — feat: define shared session API
