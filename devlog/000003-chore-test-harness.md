# chore/test-harness

## Agent

Codex

## Intent

Add the Phase 0 test harness before terminal engine and PTY work. The first harness should make renderer golden snapshots and VT byte-stream fixtures reusable across future daemon, TUI, and transport acceptance tests.

## What Changed

- Created `argus-test-support` as a workspace-only, non-published crate.
- Added snapshot helpers for deterministic multi-line renderer goldens.
- Added a `vte`-backed terminal byte-stream fixture that records printable text, controls, CSI, ESC, OSC, DCS hook/put/unhook events.
- Added baseline tests and checked-in `insta` snapshots proving the harness shape.
- Documented the harness in `README.md`.

## Decisions

- Keep harness code in a dedicated crate instead of per-crate `tests/support` modules so future crates share the same fixture behavior.
- Depend on `vte` for low-level byte classification now; the later engine spike can still choose `wezterm-term` or `alacritty_terminal` for canonical daemon VT state.
- Use `insta` snapshots with checked-in `.snap` files so CI catches renderer and transcript regressions without requiring snapshot updates.

## Commits

- e532567 — test: add reusable test harness
- 22b1a92 — test: preserve terminal fixture fidelity
- 8ea3979 — test: address harness review feedback

## Progress

- 2026-05-11T22:12-0700 — Started `chore/test-harness` from `origin/main`.
- 2026-05-11T22:12-0700 — Implemented the initial `argus-test-support` crate with snapshot helpers, VT fixtures, and baseline snapshots.
- 2026-05-11T22:12-0700 — Validation passed: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --workspace`, and `cargo test --workspace`.
- 2026-05-11T22:48-0700 — Addressed review findings: snapshot normalization now preserves intentional trailing blank rows and VT plain text preserves carriage returns.
- 2026-05-11T22:48-0700 — Revalidated with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --workspace`, and `cargo test --workspace`.
- 2026-05-11T22:56-0700 — Addressed Junie and Copilot review threads: `frame` accepts `AsRef<str>` inputs without an intermediate `Vec`, and `argus-test-support` now has a package description.
- 2026-05-11T22:56-0700 — Revalidated with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --workspace`, and `cargo test --workspace`.

## Next Steps

- Push the branch and open a draft PR.
