# Daemon Session Core

## Agent

Codex

## Intent

Start Phase 2 by adding the first daemon-owned PTY session core: spawn a process in a PTY, tee raw output to a per-session log, and feed the same bytes into the canonical terminal engine.

## Decisions

- Use upstream Git-pinned `wezterm-term` as the daemon canonical VT engine.
- Use `portable-pty` for PTY allocation so the first session core is not tied to one platform API.
- Keep this branch to the smallest useful session slice: PTY spawn, output ingestion, raw log tee, terminal state update, and visible snapshot access.

## Progress

- 2026-05-12T18:32-0700 — Created `feat/daemon-session-core` worktree from `origin/main`.
- 2026-05-12T19:08-0700 — Added the first daemon PTY session module with `portable-pty` spawn, raw output log tee, and `wezterm-term` ingestion.
- 2026-05-12T19:08-0700 — Verified the daemon session test can spawn a short shell command through a real PTY and observe the marker in both raw logged bytes and visible terminal rows.
- 2026-05-12T19:08-0700 — Left the real PTY drain test ignored on Windows after GitHub Windows CI hung waiting for ConPTY EOF; Windows PTY lifecycle needs a dedicated follow-up instead of blocking this Unix/macOS session-core slice.
- 2026-05-12T20:38-0700 — Addressed PR review feedback by truncating session logs on open, replacing unbounded raw byte accumulation with a byte count, naming the terminal input sink, treating Unix PTY `EIO` as normal closure, and making the shell test non-login.
- 2026-05-12T20:38-0700 — Applied follow-up review feedback to simplify visible row trimming without cloning before truncation.

## What Changed

- Added `argus_daemon::session` with session config, checked PTY sizing, PTY spawn, output drain, raw log tee, and visible row extraction.
- Added a real PTY output test for Unix/macOS and kept it ignored on Windows until ConPTY EOF/shutdown behavior is handled deliberately.
- Opened session logs as fresh truncated files so each log contains only the current session's bytes.
- Returned logged byte count instead of retaining all raw PTY bytes in memory.
- Added `portable-pty` as a workspace dependency and reused the pinned workspace `wezterm-term` dependency for daemon state.
- Updated the design roadmap to mark Phase 1 complete and Phase 2 PTY spawn/log tee started.

## Validation

- `cargo fmt --all -- --check`
- `cargo test -p argus-daemon`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Commits

- HEAD — feat: add daemon PTY session core

## Next Steps

- Extend the session core from one-shot drain into a long-running actor with commands for resize, input, snapshot, and shutdown.
- Add daemon-owned sequence numbers around output chunks and snapshots.
- Add a Windows-specific ConPTY lifecycle test that can terminate without relying on blocking reader EOF.
