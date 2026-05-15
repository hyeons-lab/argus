# feat/daemon-first-tui

## Agent
- Codex

## Intent
- Make the local TUI daemon-backed by default now that the Unix socket session API has landed.
- Keep the embedded session manager path available only as an explicit development mode.

## Decisions
- Avoid silent fallback from daemon startup errors because it can hide daemon/socket regressions and split session ownership between two managers.
- Use a small built-in CLI parser for this slice instead of adding a command-line dependency.

## What Changed
- Made the TUI connect to the daemon Unix socket by default instead of falling back to an embedded manager on connection or startup errors.
- Added explicit `--embedded` and `--socket <path>` startup options with parser coverage.
- Marked the local Unix socket adapter complete in the design roadmap.
- Fixed PR review follow-ups so `--help` exits successfully, non-Unix TUI startup defaults to embedded mode as the supported workaround, and both ambiguous `--embedded`/`--socket` orderings are covered.

## Commits
- 472806b — feat: make TUI daemon-first
- HEAD — fix: clean up TUI startup options

## Progress
- 2026-05-14T18:00-0700 — Created `feat/daemon-first-tui` worktree from `origin/main`, unset the accidental upstream, and inspected the current TUI socket fallback.
- 2026-05-14T18:03-0700 — Implemented daemon-first TUI startup, kept embedded mode opt-in, updated roadmap status, and completed local validation.
- 2026-05-14T18:03-0700 — Validation passed: `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test -p argus-tui`, `/home/dberrios/.cargo/bin/cargo test --workspace`, `cargo clippy --all-targets --all-features -- -D warnings`, and `git diff --check`.
- 2026-05-14T18:18-0700 — Addressed PR review feedback: clean help exit, Windows/non-Unix embedded default workaround, and extra parser coverage for reverse ambiguous option ordering.
- 2026-05-14T18:18-0700 — Validation passed after review fixes: `cargo run -q -p argus-tui -- --help`, `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test -p argus-tui`, `/home/dberrios/.cargo/bin/cargo test --workspace`, `cargo clippy --all-targets --all-features -- -D warnings`, and `git diff --check`.
- 2026-05-14T18:25-0700 — Fixed the Windows CI regression by making the default-startup parser test assert the platform-specific default: daemon socket on Unix and embedded mode on non-Unix.
