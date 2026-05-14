# TUI Session Controls

## Thinking

The merged local TUI can start one embedded shell and forward input. The next useful pressure test is multiple local sessions because it exercises session creation, event subscriptions, input leases, resizing, shutdown, and rendering without adding the Unix socket adapter yet.

The current app still has temporary smoke-test counters in the visible status line. Those counters were useful while fixing the invisible-input issue, but the next branch should replace them with user-facing session controls.

## Plan

1. Refactor `LocalSessionApp` from one session view plus one event receiver into a selected list of local session runtimes.
2. Add app methods for creating a new session, closing the selected session, selecting next/previous sessions, writing input to the selected session, resizing active sessions, and shutting down all remaining sessions.
3. Update the Ratatui rendering so the sidebar shows all local sessions and highlights the selected one.
4. Add keyboard commands for new session, close selected session, and session switching while preserving normal printable and arrow-key shell input.
5. Remove the temporary input/output diagnostic counters from the visible status line.
6. Add focused unit tests for event routing and keyboard command handling, then run the repository validation commands.
