## Thinking

The previous branch put the session API in `argus-core` and left daemon binding as the next step. The narrowest useful daemon slice is not IPC yet; it is an in-process manager that owns many `SessionActor` handles, assigns `SessionId`s, maintains `InputLeaseState`, and implements `SessionApi`.

The manager should keep PTY internals hidden. `StartSessionRequest` becomes `SessionConfig` at the boundary, with a daemon-selected log path. Attach as observer should return a snapshot and leave the lease alone. Attach as an interactive or agent controller should acquire or take over the lease and return the new state. `write_input` must reject clients that do not currently hold the lease.

## Plan

1. Add a daemon `SessionManager` and configuration type in `crates/argus-daemon/src/session.rs`.
2. Implement `argus_core::session::SessionApi` for the manager using the existing `SessionActor`.
3. Add focused daemon tests for session creation, observer write rejection, controller lease acquisition, takeover, release, resize, snapshot, and shutdown.
4. Format, run daemon/core/workspace validation, update the devlog, commit, push, and open a draft PR.

