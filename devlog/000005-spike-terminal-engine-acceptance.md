# Terminal Engine Acceptance Spike

## Agent

Codex

## Intent

Start Phase 1 by comparing terminal engine candidates and deciding the daemon-owned virtual terminal state model before implementing PTY session actors.

## Decisions

- Keep this branch scoped to acceptance evidence, fixtures, and architecture notes. PTY spawning and session actors remain Phase 2.
- Treat upstream `wezterm-term` as a Git-pinned candidate from `https://github.com/wezterm/wezterm`, not as replaced by the published Tattoy fork.
- Prefer upstream `wezterm-term` for the daemon canonical VT state unless the follow-up compile spike shows Git-pinned workspace dependencies are too heavy or unstable.

## Progress

- 2026-05-12T07:38-0700 — Created `spike/terminal-engine-acceptance` worktree from `origin/main`.
- 2026-05-12T07:38-0700 — Started Phase 1 devlog and plan.
- 2026-05-12T07:56-0700 — Added engine-neutral terminal acceptance fixtures covering the Phase 1 behavior checklist.
- 2026-05-12T07:56-0700 — Verified support-crate acceptance tests pass after checking in the first terminal acceptance snapshot.
- 2026-05-12T07:56-0700 — Confirmed upstream WezTerm defines `term/` as `wezterm-term`; documented it as a Git-pinned candidate.
- 2026-05-12T09:14-0700 — Added compile spikes for upstream Git-pinned `wezterm-term` and crates.io `alacritty_terminal`; both candidates accept chunked PTY byte ingestion in tests.
- 2026-05-12T09:32-0700 — Gated heavyweight engine compile spikes behind the explicit `engine-spikes` feature so default support tests stay lightweight.
- 2026-05-12T10:59-0700 — Addressed PR feedback by pinning `wezterm-term` to the reviewed upstream commit and clarifying the observed dependency cost in the matrix.

## What Changed

- Added reusable, engine-neutral terminal acceptance scenarios to `argus-test-support`.
- Added snapshots for resize, late attach, reconnect, alt-screen, bracketed paste, mouse reporting, scroll regions, replay, and log tee parser behavior.
- Added feature-gated compile spikes for upstream Git-pinned `wezterm-term` and crates.io `alacritty_terminal`.
- Added `devlog/terminal-engine-compatibility-matrix.md` with candidate packaging, acceptance matrix, recommended daemon VT state model, and fork artifact option.

## Research & Discoveries

- `cargo search wezterm-term` did not return an upstream crate named `wezterm-term` on 2026-05-12, but `https://github.com/wezterm/wezterm/blob/main/term/Cargo.toml` defines the crate name as `wezterm-term`.
- `alacritty_terminal` is available from crates.io as `0.26.0` with Rust version `1.85.0` and default serde support.
- The WezTerm terminal core exposes concepts that map directly to daemon attach/reconnect requirements: `advance_bytes`, screen state, scrollback, stable row indexes, sequence numbers, input encoding, and optional serde.
- Directly adding upstream `wezterm-term` as a Git dependency works, but it locked 201 packages. The initial manual Git fetch also reported updating upstream submodules; Cargo's locked dependency graph is the concrete CI cost. That build and maintenance cost should be treated as a packaging risk, not ignored.
- `alacritty_terminal` compiles from crates.io with a smaller dependency increment and can parse chunked bytes through `vte::ansi::Processor`.
- A WezTerm fork can publish a source/package artifact for Argus CI to download before compiling. That is safer than relying on precompiled Rust build outputs, which are sensitive to rustc version, target triple, features, and dependency hashes.

## Validation

- `cargo fmt --all --check`
- `cargo test -p argus-test-support`
- `cargo test -p argus-test-support --features engine-spikes`
- `cargo check -p argus-test-support --features engine-spikes`

## Commits

- 6ba7ebf — test: add terminal engine acceptance spikes
- HEAD — docs: address terminal engine review feedback

## Next Steps

- Extend the engine-specific spikes to produce visible-cell snapshots for every engine-neutral acceptance scenario.
- Decide whether direct Git-pinned upstream `wezterm-term` belongs in default CI or should be replaced by a vendored/forked/published package artifact path before Phase 2.
- Decide whether Phase 2 needs serialized engine snapshots or checkpoint-plus-tail rebuilds.
