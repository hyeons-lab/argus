# Local Unix Socket API

## Thinking

The local TUI now proves that the session API can drive multiple shells, but it still embeds the daemon `SessionManager` in the TUI process. The next useful transport step is owner-local Unix socket IPC because it forces a real client/server boundary while staying within the local trust model described in the design doc.

This branch should avoid remote access, pairing, MCP tooling, and browser concerns. The goal is a small protocol adapter around the existing session API, with enough coverage to move the TUI off direct daemon internals in a later slice if the full swap proves too large for one branch.

## Plan

1. Inspect the current `SessionApi`, `SessionManager`, and `LocalSessionApp` boundaries.
2. Add a transport-neutral wire request/response shape for the current TUI operations.
3. Implement an owner-local Unix socket server around `SessionManager`.
4. Implement a Unix socket client adapter that exposes the same session operations to local clients.
5. Add focused tests for request handling, protocol errors, and a basic session lifecycle over the socket.
6. Run formatting, clippy, check, workspace tests, and update the branch devlog before committing.
