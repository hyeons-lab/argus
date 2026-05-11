# Argus

A terminal multiplexer with attention routing and remote access. A long-running daemon owns all session state; the Ratatui TUI, a Flutter client (web / iOS / Android / optional desktop), and an MCP server are clients of the same session API.

## Planned scope

- Sessions grouped by repo and worktree, inferred from the working directory of each session's foreground process.
- Per-session status (running, idle, awaiting input, error) with hotkeys to cycle sessions that need a response.
- AI agent prompt detection via MCP integration, idle/pattern heuristics, and bundled pattern packs for Claude Code, Aider, Codex, Cline, and Continue.
- Remote access from any paired device over WebSocket + TLS, with QR-based pairing.
- Optional gRPC + mTLS surface for AI agents running on a different machine.

## Status

Early development. Not yet usable.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
