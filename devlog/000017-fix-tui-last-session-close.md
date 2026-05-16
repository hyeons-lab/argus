# fix/tui-last-session-close

## Agent

Codex

## Intent

Allow a confirmed close action on the only terminal session to close that session and exit the TUI instead of blocking with a last-session error.

## What Changed

- Changed `LocalSessionApp::close_selected_session` to return whether the TUI should exit after the selected session is successfully shut down.
- Removed the last-session close guard so a confirmed close can shut down the final session.
- Updated the TUI event loop to exit immediately after a confirmed close removes the final session.
- Added a daemon-backed regression test for closing the only attached session.
- Replaced the ambiguous close `bool` with `CloseSessionOutcome` and made no-selection close attempts non-panicking.

## Decisions

- Kept the behavior tied to the existing close-session confirmation flow; exiting only happens after the user confirms the close command.
- Used devlog sequence `000017` because `000016` is already reserved by the open code-review-graph PR.

## Progress

- 2026-05-16T08:33-0700 — Created `fix/tui-last-session-close` from current `origin/main` and implemented the last-session close behavior.
- 2026-05-16T08:42-0700 — Addressed PR review feedback by adding an explicit close outcome enum and a no-selected-session guard.

## Commits

- 004da05 — fix: exit TUI after closing last session
- HEAD — fix: clarify TUI close session outcome
