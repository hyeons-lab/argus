# chore/code-review-graph-files

## Agent
- Codex

## Intent
- Check in the local code-review-graph agent/editor integration files used during development.
- Keep CI independent from a code-review-graph installation by adding only configuration and instruction files.

## Decisions
- Do not add a CI installation step for code-review-graph in this branch.
- Keep `.code-review-graph/` ignored so local graph databases are not committed.

## What Changed
- Added code-review-graph MCP instructions for the agent/editor surfaces used in this repo.
- Added local MCP, hook, and skill configuration files for Claude, Gemini, Kiro, OpenCode, Qoder, Cursor, and Windsurf-style tools.
- Updated `.gitignore` to ignore the local `.code-review-graph/` data directory.
- Split Claude and Qoder SessionStart fallback handling so only non-git directories print `Not a git repo, skipping`; missing or failing `code-review-graph` stays quiet.

## Progress
- 2026-05-16T08:16-0700 — Created the PR worktree from `origin/main` and copied the local code-review-graph integration files into it.
- 2026-05-16T08:18-0700 — Replaced local absolute repo paths with relative `.` paths, omitted the generated Gemini backup file, and validated JSON/whitespace formatting.
- 2026-05-16T08:33-0700 — Addressed review feedback about misleading Claude and Qoder SessionStart fallback text when `code-review-graph` is absent or returns a non-zero status.

## Commits
- 4bce632 — chore: add code-review-graph agent files
- HEAD — fix: clarify graph hook fallback handling
