# Terminal Engine Compatibility Matrix

## Candidates

| Candidate | Source | Packaging | Notes |
| --- | --- | --- | --- |
| `wezterm-term` | `https://github.com/wezterm/wezterm`, `term/` crate | Git-pinned dependency unless/until a current crates.io release is available | Upstream terminal core. Provides terminal parsing, screen cells, scrollback, input encoding, optional serde, `advance_bytes`, and stable row index concepts that match Argus daemon needs. |
| `alacritty_terminal` | `https://github.com/alacritty/alacritty` | crates.io `0.26.0` | Mature terminal emulator library with `Term`, resize, alt-screen, scrollback, damage tracking, and serde-enabled grid data. Public API is optimized around Alacritty's frontend/event model. |

`cargo search wezterm-term` did not return a crate literally named `wezterm-term` on 2026-05-12. The upstream repo does define `term/Cargo.toml` with `name = "wezterm-term"` and version `0.1.0`, so this candidate should be evaluated as an upstream Git dependency rather than replaced by the Tattoy fork.

Both candidates pass a minimal compile spike in `argus-test-support`:

- `wezterm_engine_spike.rs` instantiates upstream Git-pinned `wezterm-term`, feeds chunked PTY bytes with `advance_bytes`, and verifies size/sequence/screen state access.
- `alacritty_engine_spike.rs` instantiates crates.io `alacritty_terminal`, feeds the same style of chunked PTY bytes through `vte::ansi::Processor`, and verifies grid dimensions/display state.

The upstream WezTerm Git path is materially heavier: adding the dependency locked 201 packages and fetched WezTerm submodules including zlib, libpng, freetype, and harfbuzz. That is not a semantic blocker, but it is a build and CI cost that Phase 2 should revisit before making WezTerm a production dependency.

## Compatibility Matrix

| Capability | Fixture | `wezterm-term` | `alacritty_terminal` | Phase 1 Decision |
| --- | --- | --- | --- | --- |
| Resize | `resize_window_report` | Strong fit. Upstream exposes `TerminalSize`, `Terminal::resize`, and screen resize logic with scrollback-aware row identity. | Supported via `Term::resize`; resize resets the scroll region and integrates with damage tracking. | Either can model resize; prefer the engine whose row identity supports attach/replay cleanly. |
| Late attach | `late_attach_screen_reset` | Strong fit. Screen and scrollback model includes visible rows, scrollback rows, stable row indexes, and optional serde. | Viable. `RenderableContent`, grid, display offset, and serde-enabled grid can create snapshots, but attach contracts must be Argus-owned. | Daemon must expose snapshot plus ordered tail, not raw-log replay only. |
| Reconnect | `reconnect_alt_screen_boundary` | Strong fit. Main/alternate screen and sequence numbers line up with checkpointed reconnect. | Viable. Active/inactive grids cover alternate screen; reconnect still needs Argus sequence numbers outside the engine. | Reconnect uses daemon sequence numbers and a bounded byte/event tail. |
| Alt-screen | `alt_screen_enter_exit` | Strong fit. Explicit normal/alternate screen model. | Strong fit. Active and inactive grids model normal/alternate screen behavior. | Required for vim, htop, lazygit, and terminal UIs. |
| Bracketed paste | `bracketed_paste_round_trip` | Strong fit for parsing and input encoding. Paste policy still belongs to the daemon lease/input layer. | Strong fit for terminal mode tracking. Paste policy still belongs to the daemon lease/input layer. | Engine tracks mode; daemon controls who can write paste bytes. |
| Mouse reporting | `sgr_mouse_press_release` | Strong fit. Upstream includes keyboard and mouse input encoding plus terminal mode handling. | Strong fit. Terminal mode handling supports mouse protocol state. | Engine tracks mode; clients encode events through a daemon-approved input path. |
| Scroll regions | `scroll_region_reset` | Strong fit. Screen API has scroll-region and stable row concepts. | Strong fit. `Term` tracks scroll regions and scroll-up/down behavior. | Required; no candidate blocker. |
| Replay | `chunked_replay_matches_contiguous_stream` | Strong fit. `advance_bytes` accepts caller-provided byte chunks and parser state lives in the terminal model. | Viable. Parser state lives in `Term`; Argus still owns byte ordering and checkpoints. | Argus stores monotonically sequenced raw chunks and validates chunked replay equivalence. |
| Log tee | `log_tee_preserves_raw_bytes` | Neutral. Engine should not own raw logs. | Neutral. Engine should not own raw logs. | Daemon tees raw PTY bytes before parsing. |

## Recommended State Model

Use a daemon-owned terminal session state with these layers:

1. Raw PTY byte log tee, stored as ordered chunks with monotonically increasing sequence numbers.
2. Parsed terminal state owned by one engine instance in the daemon.
3. Daemon metadata beside the engine: session id, PTY size, last resize sequence, attach generation, active input lease, foreground process metadata, and mode flags needed for policy.
4. Snapshot contract for clients: engine snapshot of visible state and scrollback window, daemon metadata, last included sequence number, then an ordered byte/event tail after that sequence.
5. Replay contract for tests: feeding one contiguous byte stream and feeding the same bytes in PTY-sized chunks must produce equivalent parsed events and equivalent raw log bytes.

## Current Recommendation

Prefer upstream `wezterm-term` for the canonical daemon VT state, evaluated as a Git-pinned dependency from `https://github.com/wezterm/wezterm` unless a current crates.io package becomes available. Its public model is closer to Argus's daemon requirements: terminal core without GUI or PTY ownership, `advance_bytes` ingestion, screen plus scrollback abstractions, stable row indexes, sequence-number concepts, input encoding, and optional serde.

Keep `alacritty_terminal` as the fallback candidate if Git-pinning WezTerm's workspace dependencies proves too heavy or unstable for normal development and CI. It is mature and packaged cleanly on crates.io, but Argus would need more daemon-owned wrapper code around snapshot identity, attach/reconnect semantics, and event-policy boundaries.

## Follow-Up Proof Points

- Decide whether direct Git-pinned `wezterm-term` is acceptable in default CI or whether Argus should vendor, fork, or use a lighter published terminal-core package.
- Evaluate a fork artifact workflow for `wezterm-term` packaging. Preferred artifact shape is a reproducible source/package artifact, such as a vendored crate tarball or sparse-registry package, produced by the fork's CI and downloaded by Argus CI before `cargo` runs. Avoid depending on precompiled Rust build outputs except as cache acceleration, because `.rlib`/incremental outputs are tied to rustc version, target triple, enabled features, and dependency hashes.
- Extend candidate tests from simple chunk ingestion to visible-cell snapshots for the full acceptance scenario list.
- Decide whether serialized engine snapshots are needed in Phase 2 or whether Argus can rebuild snapshots from periodic checkpoints plus raw byte tails.

## Fork Artifact Option

A fork can reduce risk if upstream `wezterm-term` remains unpublished or too heavy as a direct Git dependency:

1. Fork `wezterm/wezterm`.
2. Add a workflow in the fork that extracts the terminal-core crates needed by Argus and produces a reproducible source artifact.
3. Upload that artifact from the fork workflow.
4. In Argus CI, download the artifact before `cargo` runs and point Cargo at it through one of:
   - a vendored path dependency checked out under `vendor/`;
   - `[patch]` entries pointing at the downloaded source tree;
   - a private/sparse registry package if the fork publishes one.

This is better than downloading compiled artifacts at compile time. Cargo's build model is source-oriented, and compiled artifacts are only safe as a cache when the cache key includes rustc version, target, features, lockfile, and relevant environment inputs.
