# Phase 0 — Config Parser

## Thinking

The checked-in README plan keeps config parsing in Phase 0 before PTY and TUI work. That is the right place for it: every upcoming component needs a shared interpretation of defaults, keybindings, network binds, attention thresholds, and agent pattern configuration.

The parser should be real enough to support the design-doc TOML shape, but not overbuilt. It does not need filesystem watching, config migration, theme loading, or runtime reload. It does need typed structs, defaults, validation, and tests that make future parser changes safe.

### Placement

Put config in `argus-core` as `argus_core::config`. The daemon, TUI, MCP server, and future transport adapters all depend on core and should not each parse TOML differently.

### Shape

Mirror the documented config sections:

- `general.default_shell`
- `ui.theme`, `ui.sidebar_width_percent`, `ui.group_by`
- `attention.idle_threshold_ms`, `attention.notify_on_awaiting`, `attention.notify_sound`
- `agents.known`, `agents.custom_pack.process_names`, `agents.custom_pack.prompt_patterns`
- `remote.bind`, `remote.require_pairing`, `remote.tls_cert`, `remote.tls_key`
- `mcp.tcp_bind`
- `grpc.enabled`, `grpc.bind`
- `approval.patterns`
- `keybindings.*`

### Validation

Start with checks that prevent obviously broken config from entering the daemon:

- `ui.sidebar_width_percent` must be `1..=80`.
- `ui.group_by` is an enum: `repo`, `worktree`, or `flat`.
- `attention.idle_threshold_ms` must be greater than zero.
- `remote.bind`, `mcp.tcp_bind`, and optional `grpc.bind` must parse as `SocketAddr`.
- `remote.tls_cert` and `remote.tls_key` must both be set or both be unset.
- `agents.known`, custom process names, prompt patterns, approval patterns, and keybinding values must not contain empty strings.

### Dependencies

Add:

- `serde` with `derive`
- `toml`

No `dirs` crate yet. Loading from the default config path can use `$HOME/.config/argus/config.toml`, failing loudly if `HOME` is unavailable. Path expansion for user-provided fields can remain a later concern; store cert/key strings as provided.

## Plan

1. Add workspace dependencies for `serde` and `toml`; inherit them in `argus-core`.
2. Add `argus-core/src/config.rs` with typed config structs, defaults, `from_toml_str`, `load_from_path`, `default_path`, and validation.
3. Expose `pub mod config;` from `argus-core/src/lib.rs`.
4. Add unit tests for defaults, sparse TOML merge behavior, full documented TOML, invalid enum, invalid bind address, invalid sidebar width, missing paired TLS key/cert, and loading from a file path.
5. Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check --workspace`, and `cargo test --workspace`.
6. Update the branch devlog with results.
