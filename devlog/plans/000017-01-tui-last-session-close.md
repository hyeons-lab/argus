# TUI Last Session Close

## Thinking

The TUI already asks for confirmation before closing a session. The bug is the app-level guard that rejects closing when only one session remains, even though a user-confirmed close of the only terminal naturally means the TUI should exit. The fix should keep the confirmation flow in `main.rs`, keep session shutdown owned by `LocalSessionApp`, and avoid leaving the app in an empty-session state except on the direct path back out of the run loop.

## Plan

1. Change the selected-session close API to signal when the final session was shut down.
2. Have the TUI run loop return after a confirmed close removes the final session.
3. Add a regression test for daemon-backed single-session close behavior.
4. Run focused TUI tests, format checks, and diff checks before committing.
