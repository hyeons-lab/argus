# Daemon Session Actor Plan

## Thinking

The previous branch proved that daemon-owned PTY bytes can be logged and ingested into the canonical terminal engine. The next missing behavior is process lifetime control. Argus needs a session object that stays alive while clients ask it to write input, resize the PTY and terminal model, read snapshots, and shut down.

This branch should not define the final transport API yet. The useful slice is a daemon-local actor with a narrow handle. That lets the next TUI/API work call one surface without every caller owning PTY handles or terminal state directly.

Sequence numbers should be part of this branch because attach/reconnect, scrollback cursors, and transport diffs all depend on stable ordering. The first version can use a single monotonically increasing output sequence that advances per PTY chunk and is included in snapshots.

## Plan

1. Refactor `PtySession` so it can clone a PTY writer, resize the master PTY, and expose snapshots without consuming the session.
2. Add a daemon-local `SessionActor` handle that owns a worker thread, command channel, and response channels for input, resize, snapshot, and shutdown.
3. Have the worker drain PTY output opportunistically, tee bytes to the raw log, advance the terminal, and increment an output sequence number.
4. Add focused tests around a long-running shell: send input, observe ordered snapshots, resize the terminal, and shut down cleanly.
5. Update the devlog with decisions, validation, and remaining follow-up work.
