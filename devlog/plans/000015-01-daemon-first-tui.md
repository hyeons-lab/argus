# Daemon-First TUI

## Thinking

The local Unix socket API is merged and the TUI already knows how to create sessions through it. The remaining product decision is startup behavior: silently falling back to an embedded `SessionManager` makes manual demos convenient, but it also hides daemon/socket failures and can create a second owner of session state. That conflicts with the design rule that the daemon owns PTYs, logs, terminal state, and client attachment.

The next slice should make the contract explicit without starting a larger CLI redesign. Default TUI startup should connect to the daemon socket and fail clearly when unavailable. The embedded manager remains useful for tests and isolated development, but it should be opt-in.

## Plan

1. Replace the Unix TUI startup fallback with daemon-socket startup by default.
2. Add explicit `--embedded` and `--socket <path>` startup options with focused parser tests.
3. Keep the existing test-only embedded constructors and local app tests intact.
4. Mark the local Unix socket adapter complete in the design roadmap.
5. Format, run targeted TUI tests, workspace tests, clippy, and diff checks before committing.
