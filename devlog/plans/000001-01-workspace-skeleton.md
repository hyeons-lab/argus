# Phase 0 — Step 1: Cargo Workspace Skeleton

## Thinking

Phase 0 of the design-doc roadmap has four items: workspace crates, `tracing` wiring, TOML config parser, and test harness. We're splitting each into its own plan + commit on `chore/initial-scaffold`, so review and CI are scoped per artifact.

**This plan covers only the workspace skeleton** — a buildable, empty workspace. The five crates are scaffolded with placeholder content; real implementations land in later phases (Phase 1 for PTY, Phase 2 for daemon/IPC, Phase 3 for TUI, etc.).

### Crate shape

| Crate | Target | Role |
| --- | --- | --- |
| `argus-core` | lib | Session trait + shared types (defined in Phase 2; empty placeholder now). Every other crate depends on this. |
| `argus-daemon` | bin | Long-running process; owns all session state. Implements the session trait. |
| `argus-tui` | bin | Ratatui local client (terminal-mode). Default `argus` binary in co-located mode. |
| `argus-web` | lib | WebSocket transport adapter, server-side. Consumed by `argus-daemon`. |
| `argus-mcp` | bin | MCP server (stdio + optional TCP). |

The Flutter app at `flutter/argus_client/` is Phase 4 work — not touched here.

### Rationale notes

- **`argus-web` is a library, not a binary.** It's a transport adapter the daemon mounts as a sub-system, not a standalone process. Same pattern will apply for a future `argus-grpc`.
- **`argus-core` is a library, used by every other crate.** Defines the session trait + types once. Empty for now — its actual API is Phase 2.
- **Toolchain pinning.** Pin to stable in `rust-toolchain.toml`. Revisit only if we need nightly features (we won't for Phase 0).
- **Lints.** Workspace-wide `clippy::all = "warn"` to start. `pedantic` would drown empty crates in noise; revisit once real code lands.
- **Edition.** Use `edition = "2024"` and `resolver = "3"` — latest stable for new Rust projects in 2026.

### Open questions

None blocking. Tracked for later:
- Workspace-level `[workspace.dependencies]` block — defer until at least two crates share a dep.
- License headers per file — Apache 2.0 doesn't require them on every file. Skip for now; can add if we onboard contributors.

## Plan

1. **Write workspace `Cargo.toml`** at the worktree root:
   - `[workspace]` with `members = ["crates/*"]` and `resolver = "3"`.
   - `[workspace.package]`: `edition = "2024"`, `version = "0.0.0"`, `license = "Apache-2.0"`, `repository = "https://github.com/hyeons-lab/argus"`, `authors = ["Hyeons' Lab"]`.
   - `[workspace.lints.clippy]`: `all = "warn"`.

2. **Scaffold each crate** under `crates/`:
   - `argus-core/Cargo.toml` + `src/lib.rs` (empty lib).
   - `argus-daemon/Cargo.toml` + `src/main.rs` (placeholder `println!`).
   - `argus-tui/Cargo.toml` + `src/main.rs` (placeholder `println!`).
   - `argus-web/Cargo.toml` + `src/lib.rs` (empty lib).
   - `argus-mcp/Cargo.toml` + `src/main.rs` (placeholder `println!`).
   
   Each `Cargo.toml` inherits version / edition / license / repository / authors from `[workspace.package]` via `package.* = { workspace = true }` keys.

3. **Add `rust-toolchain.toml`** pinning to stable, with `components = ["rustfmt", "clippy"]`.

4. **Verify** with `cargo check --workspace` — clean, no warnings.

5. **Update branch devlog** — flip the "Cargo workspace skeleton" checkbox, add `What Changed` entries for the new files, record the commit in `Commits` (as `HEAD — chore: scaffold cargo workspace`).

6. **Commit** with message:
   ```
   chore: scaffold cargo workspace
   
   Five member crates: argus-core (lib, shared types), argus-daemon (bin,
   state owner), argus-tui (bin, Ratatui local client), argus-web (lib,
   WebSocket transport adapter), argus-mcp (bin, MCP server). cargo check
   --workspace passes clean.
   ```

7. **Commit `CLAUDE.md`** separately with `docs: add project CLAUDE.md` so each commit stays single-purpose.

8. **Push** `chore/initial-scaffold` to origin (`git push -u origin chore/initial-scaffold`) and open a **draft** PR via `gh pr create --draft`. Single push for both commits — minimizes CI runs per the workflow.

## Out of scope (follow-up plans on this branch)

- `tracing` wiring to `~/.local/state/argus/argus.log` → `devlog/plans/000001-02-tracing.md`
- TOML config parser per design-doc §9 → `devlog/plans/000001-03-config.md`
- Test harness — `insta` snapshots + `vte` virtual terminal → `devlog/plans/000001-04-test-harness.md`
- CI — fmt + clippy + test on Linux and macOS → `devlog/plans/000001-05-ci.md`

## Amendment — 2026-05-11

After the initial scaffold PR merged, the design roadmap was tightened around the highest-risk architecture questions:

- Session input now requires an explicit one-writer lease model instead of "all clients can inject" semantics.
- The daemon is the canonical owner of VT state; remote renderers attach from daemon snapshots before consuming incremental output.
- Local and remote trust boundaries are split. Remote network transports require auth and encryption; in-process, owner-only Unix sockets, and stdio MCP inherit local process trust.
- The roadmap now prioritizes local daemon/TUI usability, then MCP, then browser remote access, then native mobile.
- CI is no longer future-only in the roadmap; it belongs in Phase 0 and should grow with project surfaces as they become real.
