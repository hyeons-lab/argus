# WezTerm Term Packaging Spike

## Agent

Codex

## Intent

Prove the least complex packaging path for using upstream `wezterm-term` directly before deciding whether Argus needs a fork or artifact workflow.

## Decisions

- Keep the current upstream commit pin from the terminal-engine acceptance spike.
- Target the upstream workspace package by dependency name; Cargo does not allow combining `git` and `path` in one dependency specification.
- Use HTTPS for the public upstream dependency so default CI can fetch it without requiring SSH deploy keys.

## Progress

- 2026-05-12T17:39-0700 — Created `spike/wezterm-term-packaging` worktree from `origin/main`.
- 2026-05-12T17:39-0700 — Tested `git` plus `path = "term"` in the dependency declaration; Cargo rejected it as ambiguous because only one of `git` or `path` is allowed.
- 2026-05-12T17:39-0700 — Normalized the upstream Git dependency URL to `https://github.com/wezterm/wezterm.git` and confirmed Cargo still resolves the `wezterm-term` package from the upstream workspace.
- 2026-05-12T18:06-0700 — Addressed PR review feedback by rewording the plan to say direct Git dependency and explicit package name instead of direct path dependency.

## What Changed

- Updated the feature-gated `wezterm-term` dependency URL to the canonical Git URL with `.git` suffix.
- Updated `Cargo.lock` source entries for the pinned WezTerm workspace packages to match the canonical URL.
- Added a plan and devlog for the packaging proof.

## Validation

- `cargo fmt --all -- --check`
- `cargo test -p argus-test-support --features engine-spikes`
- `cargo test -p argus-test-support --features engine-spikes --locked`

## Commits

- 605b2ea — test: normalize wezterm-term git dependency
- HEAD — docs: clarify wezterm-term dependency wording

## Next Steps

- Decide whether the direct Git-pinned dependency is acceptable for Phase 2 or whether to continue into a fork/artifact workflow.
