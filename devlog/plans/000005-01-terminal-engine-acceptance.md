# Terminal Engine Acceptance

## Thinking

Phase 1 is a gate before daemon session work. The daemon needs one canonical terminal state model that can support local TUI rendering, browser reconnect, late attach, session replay, and future MCP observation without each client owning a divergent interpretation of PTY bytes.

The spike should produce checked-in evidence instead of an informal preference. The current test-support crate already has byte-stream fixtures and snapshot normalization, so the lowest-risk path is to extend that test-support surface with reusable acceptance scenarios and a written compatibility matrix. The branch should avoid PTY process management until Phase 2; terminal engine behavior can be evaluated with deterministic byte streams first.

Candidate engines:

- `wezterm-term`: likely stronger for reusable terminal-state semantics and serialization-style thinking because it is extracted from a mature terminal emulator.
- `alacritty_terminal`: likely strong and battle-tested for fast terminal emulation, but its public surface may be shaped around Alacritty's frontend integration rather than daemon snapshot/replay needs.

The decision criteria should favor daemon semantics over renderer convenience:

- Can the daemon ingest PTY bytes once and expose stable snapshots/events to many clients?
- Can a late client attach without replaying unbounded raw logs?
- Can reconnect combine a snapshot with a bounded byte tail?
- Are alt-screen, scroll regions, bracketed paste, and mouse modes observable enough for policy and transport?
- Can resize state be represented cleanly and replayed deterministically?

## Plan

1. Add deterministic terminal acceptance scenarios to `argus-test-support`.
   - Keep them engine-neutral: scenario name, byte stream, expected text/control behavior, and the Phase 1 capability each scenario covers.
   - Cover resize marker/state, late attach/reconnect replay boundaries, alt-screen enter/exit, bracketed paste enable/disable, mouse reporting enable/disable, scroll regions, replay ordering, and log tee byte preservation.

2. Add candidate comparison notes.
   - Create a compatibility matrix under `devlog/`.
   - Evaluate `wezterm-term` and `alacritty_terminal` against each capability.
   - Record unknowns explicitly when an API must be proven by a follow-up compile/test spike.

3. Decide the daemon VT state model.
   - Document which engine, if any, owns canonical screen state.
   - Define what the daemon stores: raw byte log, parsed terminal state, scrollback sequence numbers, mode flags, resize metadata, and replay checkpoints.
   - Define the attach contract: initial snapshot plus ordered byte/event tail.

4. Validate the branch.
   - Run formatting and workspace tests.
   - Update the branch devlog with evidence and the final decision.
   - Commit using a conventional commit message and open a PR when ready.
