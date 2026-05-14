# Session API Plan

## Thinking

The daemon now has a local actor that can control one PTY-backed session. The next boundary should be the shape all clients and transports will call, not another transport-specific implementation. The shared API belongs in `argus-core` because every crate already depends on it or is expected to bind to it.

This branch should avoid moving daemon internals. PTY runtime setup, worker threads, terminal engine state, and raw log files are implementation details. The useful slice is a transport-neutral contract: stable IDs, snapshots, lifecycle summaries, attach modes, lease ownership, session events, and a trait with synchronous methods that daemon adapters can implement first.

Input lease semantics should be explicit now because they shape every client. Observers can attach without input rights. Interactive and agent controllers require a lease, and takeover must be a visible state change rather than a silent overwrite.

## Plan

1. Add an `argus_core::session` module with transport-neutral session identifiers, sizes, snapshots, completion summaries, attach modes, lease state, events, and request/result structs.
2. Define a synchronous `SessionApi` trait with start, attach, write, resize, snapshot, shutdown, and lease methods.
3. Re-export the session module from `argus-core`.
4. Add focused unit tests for default attach behavior, lease ownership transitions, and snapshot/completed-session shape.
5. Update the branch devlog and validate with formatting, tests, check, and clippy.
