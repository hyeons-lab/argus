# Argus — Project Conventions for Coding Agents

Argus is an attention-routing terminal supervisor: a long-running Rust daemon, a Ratatui local TUI, a Flutter remote client (web / iOS / Android / optional desktop), and an MCP server, all sharing one session API. The full design doc lives at `devlog/argus-design-doc.md` (local-only — see below).

## Repository layout

- `crates/` — Cargo workspace
  - `argus-core` — session trait and shared types (every other crate depends on this)
  - `argus-daemon` — long-running process; owns all session state
  - `argus-tui` — Ratatui local client (terminal-mode)
  - `argus-web` — WebSocket transport adapter, server-side (consumed by Flutter clients)
  - `argus-mcp` — MCP server (stdio + optional TCP)
- `flutter/argus_client/` — Flutter app (scaffolded in Phase 4)
- `devlog/` — **local-only**, gitignored. Branch devlogs and per-task plans.
- `worktrees/` — gitignored.

## Devlog conventions

Devlogs are local-only in this project. Write devlog files at the repo root (`/path/to/argus/devlog/`), **not** inside worktrees — worktrees are independent working trees and untracked files don't propagate between them.

- **Branch devlog:** `devlog/NNNNNN-<branch-name>.md` — one file per branch. `NNNNNN` is a zero-padded 6-digit sequence (check the highest in `devlog/` and increment). `<branch-name>` is the git branch with `/` replaced by `-`.
- **Plan file:** `devlog/plans/NNNNNN-NN-<description>.md`. `NN` is the per-branch plan sequence (01, 02, ...). Plan files use `## Thinking` then `## Plan` sections; plans are append-only.
- Branch devlogs use these sections (omit if empty): **Agent**, **Intent**, **What Changed**, **Decisions**, **Issues**, **Commits**, **Progress**, **Research & Discoveries**, **Lessons Learned**, **Next Steps**.
- Timestamps are ISO 8601 with UTC offset, e.g. `2026-05-11T12:58-0700`. Get the real time with `date "+%Y-%m-%dT%H:%M%z"`. Never fabricate or use placeholders like `00:00`.
- Track *why*, not just *what* — capture reasoning, not file diffs.
- Append-only across sessions: append to existing sections; don't rewrite or split into per-session subsections.
- Never log secrets. Use placeholders like `<API_KEY>`.

### Commits section — HEAD rule

The latest commit on the branch is always recorded as `HEAD — message`. **Never replace `HEAD` with the real hash.** When the *next* commit is made, the previous `HEAD` entry is updated to its real hash (with `git log --format="%h" -2 HEAD | tail -1`) as part of preparing that new commit, and the new commit becomes `HEAD`. Recording the hash on the same commit it refers to would require amending, which changes the hash — a self-reference loop.

## Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]
```

Common types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `ci`, `chore`, `build`, `perf`.

## Branching and worktrees

- Never commit directly to `main`. All changes go through PRs.
- Every feature uses a git worktree — no direct branch switching in the main checkout.
- The main checkout stays on `main` and is used only for worktree creation and housekeeping.
- Create a worktree:
  ```bash
  # From the main checkout (on main branch)
  git fetch origin
  git worktree add worktrees/<branch-name> -b <type>/<branch-name> origin/main
  cd worktrees/<branch-name>
  git branch --unset-upstream
  ```
  `--unset-upstream` is required because git auto-tracks `origin/main` when branching from a remote ref — a push without it would target `main`. The correct upstream is set on the first `git push -u origin <type>/<branch-name>`.
- After PR merges, clean up from the main checkout:
  ```bash
  git worktree remove worktrees/<branch-name>
  git branch -d <type>/<branch-name>
  git pull origin main
  ```

## Plan-first workflow

Before writing code on a new branch:

1. Create the worktree (above).
2. Create a branch devlog: `devlog/NNNNNN-<branch-name>.md`.
3. Create a plan file: `devlog/plans/NNNNNN-NN-<description>.md`.
4. Write code, format, validate, update the devlog, then commit and push.
5. Open a PR via `gh pr create` (use `--draft` when work isn't review-ready).
6. Update the PR description to match the final commit body.

## Minimizing CI pushes

Every push to GitHub triggers CI. CI runs are expensive — minimize waste:

- Update the devlog before committing — including the Commits section — so it reflects the commit before it lands.
- Batch related commits before pushing. Don't push after every commit.
- Prefer amending or fixup before pushing if you catch a mistake before push.
- Exception: push immediately when you need CI feedback on a specific change (e.g., testing a CI fix). Still bundle with any pending local commits.

## Build and test commands

- `cargo fmt --all -- --check` — format check (use `cargo fmt --all` to apply)
- `cargo clippy --all-targets --all-features -- -D warnings` — lint with warnings denied
- `cargo check --workspace` — type-check the whole workspace
- `cargo test --workspace` — run all tests
- `cargo run -p argus-daemon` — start the daemon (writes to `$HOME/.local/state/argus/argus.log`)

## Style

- No AI slop. Keep prose neutral and factual — no pitch-deck framing, epigraphs, or second-person scene-setting.
- Only add code comments when the *why* is non-obvious. Don't narrate what the code does.
- Don't add features, abstractions, or error handling beyond what the task requires.
