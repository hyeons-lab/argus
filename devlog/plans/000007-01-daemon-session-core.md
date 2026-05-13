# Daemon Session Core

## Thinking

Phase 1 established that Argus should use upstream `wezterm-term` for daemon-owned terminal state. Phase 2 should now prove that real PTY output can enter the same state model without clients interpreting raw bytes independently.

The first implementation should be deliberately narrow. It does not need multi-client attach, input leases, resize broadcast, persistence, or transport APIs yet. It does need one reliable path where a daemon-owned session spawns a command, reads PTY bytes, writes those exact bytes to a log, and feeds the same ordered chunks into the terminal engine.

Keeping this slice in `argus-daemon` is appropriate because the daemon owns process lifetime, PTY handles, raw logs, and canonical VT state. Shared API types can move into `argus-core` once the attach/transport boundary is clearer.

## Plan

1. Add `portable-pty` and daemon `wezterm-term` dependencies.
2. Add a daemon session module with config, PTY spawn, output drain, raw log tee, and visible terminal rows.
3. Add a focused integration-style unit test that runs a short shell command through a real PTY and verifies raw log bytes plus visible terminal state.
4. Update the design checklist to mark Phase 1 complete and Phase 2 started.
5. Validate with format, package tests, and workspace checks.

