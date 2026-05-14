# Session Event Fan-out

## Thinking

The daemon manager now owns session IDs and input leases, while the actor owns ordered PTY output, terminal state, resize state, and exit handling. Event fan-out should follow that ownership: actor-generated events for output, snapshots, and exits; manager-generated events for lease transitions. Keeping this in-process avoids locking the API to a transport format before the local TUI and IPC adapters exist.

## Plan

1. Extend `argus_core::session::SessionApi` with an in-process subscription receiver for `SessionEvent`.
2. Add subscriber registration and broadcast support to `SessionActor`.
3. Broadcast output, resize snapshot, shutdown exit, and lease-change events from the daemon manager path.
4. Add focused tests for multiple subscribers receiving output and lease-change events.
5. Run focused daemon tests, workspace checks, formatting, and clippy before committing.
