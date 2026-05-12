## Agent

Claude Code (claude-opus-4-7) @ argus branch chore/initial-scaffold

## Intent

Bootstrap the Argus repository and execute Phase 0 of the design-doc roadmap: Cargo workspace skeleton (`argus-core`, `argus-daemon`, `argus-tui`, `argus-web`, `argus-mcp`), `tracing` wiring to `~/.local/state/argus/argus.log`, TOML config parser per §9 of the design doc, and a test harness (insta snapshots + `vte`-based virtual terminal).

## Progress

- [x] Repo created (`hyeons-lab/argus`, public, Apache 2.0)
- [x] Devlog placement initially set to local-only (historical; changed by `docs/check-in-devlogs`)
- [x] Design doc moved to `devlog/argus-design-doc.md`; Phase 0 line updated for the new crate shape
- [x] `chore/initial-scaffold` worktree created at `worktrees/chore-initial-scaffold`
- [x] Project `CLAUDE.md` drafted
- [x] Phase 0 step 1 plan (`devlog/plans/000001-01-workspace-skeleton.md`)
- [x] Cargo workspace skeleton (5 crates, `cargo check` clean)
- [x] Phase 0 step 2 plan (`devlog/plans/000001-02-tracing.md`)
- [x] `tracing` wiring → `~/.local/state/argus/argus.log`
- [x] Self-review + 11 fixes + rebase to 2 logical commits
- [ ] TOML config parser per design-doc §9 (later plan `devlog/plans/000002-01-config-parser.md`)
- [ ] Test harness — `insta` snapshots + `vte` virtual terminal (later plan `devlog/plans/000003-01-test-harness.md`)
- [ ] CI workflow — fmt + clippy + test on Linux and macOS (landed in PR #3; no checked-in plan file)

## What Changed

2026-05-11T12:58-0700 README.md, LICENSE, .gitignore — initial commit on main. License adapted from sibling repo prism; at the time, gitignore made `devlog/` and `worktrees/` local-only. The `devlog/` policy later changed in `docs/check-in-devlogs`.
2026-05-11T12:58-0700 devlog/argus-design-doc.md — moved from a local planning document; Phase 0 crate line updated to add `argus-core` and clarify `argus-web` as the server-side WebSocket transport adapter, with the Flutter app at `flutter/argus_client/` outside the workspace.
2026-05-11T15:30-0700 CLAUDE.md — project conventions for coding agents (layout, devlog rules, Conventional Commits, worktree flow, plan-first flow, CI minimization, build/test commands, style guardrails).
2026-05-11T15:30-0700 Cargo.toml, Cargo.lock, rust-toolchain.toml, crates/* — five-crate workspace bootstrap. Edition 2024, resolver 3, Apache-2.0 + per-crate descriptions, authors with email. Workspace lints: `unsafe_code = "deny"` (rust) and `clippy::all = "warn"`. Toolchain pinned to 1.94.1 with rustfmt + clippy.
2026-05-11T15:30-0700 crates/argus-core/src/{lib.rs,logging.rs}, crates/argus-daemon/src/main.rs — tracing wired via `argus-core::logging`. Per-layer filters (file + console) with `RUST_LOG` overriding both; `default_config()` returns Err if HOME is unset; `init` returns a `WorkerGuard` that the daemon binds to `_flush_guard` with a load-bearing comment.
2026-05-11T17:36-0700 devlog/argus-design-doc.md, devlog/plans/* — critical roadmap review applied locally. Added explicit session input lease semantics, daemon-canonical VT state, split local vs remote trust boundaries, stronger context inference wording, realistic persistence scope, and reordered phases around local daemon/TUI usability before MCP, browser remote access, and native mobile.

## Decisions

2026-05-11T12:58-0700 Devlogs are local-only — Historical policy from the initial scaffold. User directive at the time: "keep devlogs local." Reason: this project's devlog content initially stayed outside git. Superseded by `docs/check-in-devlogs`, which makes `devlog/` checked-in repository content.

2026-05-11T12:58-0700 Workspace adds `argus-core` — Design doc commits to "one API, many transports" (§1) and a Rust trait + types for the session interface (§2). `argus-daemon` implements that trait; `argus-tui` / `argus-web` / `argus-mcp` consume it. Empty crate now means the dep graph is correct from day one.

2026-05-11T12:58-0700 `argus-web` is the WebSocket transport adapter, not the web client — Web client moved to Flutter (`flutter/argus_client/`, Phase 4). `argus-web` is the *server-side* crate that binds the session trait to WebSocket + JSON + TLS, mirroring `argus-mcp` for MCP and a future `argus-grpc` for gRPC.

2026-05-11T12:58-0700 Flutter app outside the Cargo workspace — Dart + pubspec.yaml is a different build chain; mixing it inside `crates/` would pollute `cargo metadata`.

2026-05-11T12:58-0700 First branch is `chore/initial-scaffold` — Bootstrap is project housekeeping, not user-facing functionality. Conventional Commits convention.

2026-05-11T15:38-0700 Rebased to two logical commits — Bundled scaffold + tracing into one `chore` commit; kept `docs` (CLAUDE.md) separate. Reason: empty-workspace-without-real-functionality isn't a meaningful review intermediate; the right review boundary is "conventions vs implementation." How to apply: future Phase 0 work (TOML config, test harness, CI) still lands as separate logical commits on this branch.

2026-05-11T15:38-0700 Applied review fixes 1-11 — HOME silent fallback → fail loud; single filter on both layers → per-layer `file_filter` + `console_filter`; `_guard` → `_flush_guard` with load-bearing comment; workspace `unsafe_code = "deny"`; author with email; per-crate descriptions; filter `&'static str` → `String`; rust toolchain pinned to 1.94.1; CLAUDE.md build commands promoted from TBD; stray `argus-web` newline gone via fresh history; devlog What Changed format tightened.

2026-05-11T17:36-0700 Roadmap tightened around session semantics first — The original roadmap put multi-client behavior behind a broad "all can inject" statement and deferred several hard questions until remote/mobile phases. New rule: one interactive input lease per session, daemon-owned canonical terminal state, explicit attach/reconnect semantics, and local product validation before remote/mobile distribution work.

## Issues

2026-05-11T15:38-0700 Self-review surfaced 11 issues across the first three commits (HOME silent fallback, single filter on both layers, ambiguous `_guard` binding, missing `unsafe_code` lint, malformed `authors` string, missing per-crate descriptions, `&'static str` filter, unpinned toolchain, stray newline rolled into tracing commit, CLAUDE.md build commands stale, prose-y What Changed entries). All 11 fixed; branch rebased onto main as two logical commits and force-pushed with `--force-with-lease`.

2026-05-11T17:36-0700 Critical plan review found architectural risk in multi-writer sessions, split-brain terminal rendering, over-broad auth wording, transient foreground-cwd inference, and process-resurrection persistence promises. Design doc updated locally to narrow and sequence those risks.

## Commits

112e3bd — docs: add project CLAUDE.md
d87c4b7 — chore: bootstrap workspace and wire tracing

*(history rebased: original commits b85fb28, 6a7ad3d, 469edfa replaced with the two above)*
