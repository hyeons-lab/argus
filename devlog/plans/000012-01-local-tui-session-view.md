# Local TUI Session View

## Thinking

Phase 3 needs the first local client binding. The daemon already owns PTY sessions, snapshots, input leases, and event fan-out, while `argus-tui` is still an empty binary. The most useful next slice is not a socket transport yet; it is a local TUI that uses the existing API in-process and exposes the awkward parts early: terminal sizing, event refresh, keyboard passthrough, lease ownership, and shutdown.

This branch should keep the surface intentionally narrow. A single managed shell session is enough to prove the client path. The UI still needs the shape users expect from a session supervisor: a sidebar/list area, a main terminal pane, current session metadata, and an obvious status line. The state update logic should be testable without entering raw terminal mode.

## Plan

1. Add TUI dependencies and crate wiring:
   - Add `argus-core`, `argus-daemon`, `anyhow`, `crossterm`, and `ratatui` to `argus-tui`.
   - Keep terminal control in the binary and session/view state in a library module.

2. Build the local session app model:
   - Start one shell session through `SessionManager`.
   - Attach as `InteractiveController` with a stable local client ID.
   - Subscribe to session events and keep the latest `SessionSnapshot`.
   - Expose methods for resize, input bytes, event draining, and shutdown.

3. Render the first usable view:
   - Draw a fixed sidebar listing the active session and lease/exited status.
   - Draw the main terminal rows from `SessionSnapshot.visible_rows`.
   - Draw a compact status/footer with output sequence and logged bytes.

4. Handle terminal interaction:
   - Enter raw mode and alternate screen.
   - Forward printable keys, Enter, Backspace, Tab, arrows, and common control keys to the PTY.
   - Use `q`/Esc to exit only when not treating the key as session input for this first branch.
   - Resize the session when the terminal changes size.

5. Validate:
   - Unit-test state updates and input mapping where possible.
   - Run `cargo fmt --all -- --check`, targeted TUI tests, `cargo check --workspace`, and clippy if dependency resolution allows.
