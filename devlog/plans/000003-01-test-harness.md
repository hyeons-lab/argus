# Phase 0 — Test Harness

## Thinking

The roadmap calls for golden renderer snapshots and virtual terminal fixtures before TUI and PTY code exist. That is the right sequencing: terminal behavior is expensive to retrofit tests onto, and the upcoming engine acceptance spike needs deterministic fixtures immediately.

The harness should be reusable infrastructure, not a one-off test buried in `argus-core`. A dedicated workspace crate keeps test utilities visible and available to future crates while avoiding runtime dependencies in the product crates.

### Scope

- Renderer-style snapshot helpers for deterministic multi-line frames.
- A VT byte-stream fixture that records a transcript from raw bytes.
- Baseline tests and checked-in `insta` snapshots proving the harness works in CI.
- README documentation pointing future work at the shared harness.

### Non-goals

- Choosing the canonical terminal engine. That remains Phase 1.
- PTY spawning. That remains daemon session core work.
- Ratatui-specific rendering. The TUI crate has no UI yet, so this branch only provides generic snapshot normalization.

## Plan

1. Add workspace dependencies:
   - `insta = "1"` for golden snapshots.
   - `vte = "0.15"` for ANSI/VT byte-stream parsing in fixtures.

2. Add `crates/argus-test-support`:
   - Non-published workspace member.
   - `snapshots` module with newline normalization and frame construction helpers.
   - `vt` module with a stateful `TerminalFixture` built on `vte::Parser`.

3. Add harness tests:
   - Snapshot a sample renderer frame.
   - Snapshot a VT transcript containing printable text, CR/LF, and SGR CSI sequences.
   - Assert text normalization is platform-stable.

4. Document the harness in `README.md`.

5. Validate with:
   - `cargo fmt --all`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo check --workspace`
   - `cargo test --workspace`

6. Update this branch devlog with results and commit:
   - `test: add reusable test harness`
