# Code Review Graph Files

## Thinking

The repository already uses code-review-graph locally, but the generated agent/editor integration files are only present in the working tree. Checking them in lets local development tools discover the graph instructions and MCP configuration without requiring CI to install or execute code-review-graph.

This branch should stay configuration-only. The current CI workflow runs Rust formatting, linting, docs, and tests; adding these files should not change those commands or introduce a new runtime dependency in CI. The local graph database stays ignored.

## Plan

1. Create a dedicated branch worktree from `origin/main`.
2. Copy only the code-review-graph instruction, MCP, hook, skill, and editor integration files from the local checkout.
3. Add a devlog and plan for the branch.
4. Validate whitespace and confirm the CI workflow remains unchanged.
5. Commit, push, and open a draft PR.
