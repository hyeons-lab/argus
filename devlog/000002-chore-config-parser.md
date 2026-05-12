## Agent

Codex @ argus branch chore/config-parser

## Intent

Implement the checked-in roadmap's next Phase 0 item: a TOML config parser for Argus, grounded in the revised plan that prioritizes local daemon/TUI behavior before remote/mobile work.

## Progress

- [x] `chore/config-parser` worktree created at `worktrees/config-parser`
- [x] Checked-in README plan updated to reflect the critical roadmap review
- [x] Phase 0 config parser plan (`devlog/plans/000002-01-config-parser.md`)
- [x] TOML config parser in `argus-core`
- [x] Tests for defaults, valid TOML, validation failures, and path loading
- [x] Format, clippy, check, and tests

## What Changed

2026-05-11T17:46-0700 README.md, CLAUDE.md — updated checked-in planning surface. README now records the revised architecture constraints and implementation order; CLAUDE.md no longer refers to Flutter as Phase 4.
2026-05-11T18:05-0700 Cargo.toml, Cargo.lock, crates/argus-core/{Cargo.toml,src/lib.rs,src/config.rs} — added typed TOML config parsing in `argus-core`. Config supports documented sections with defaults, validation, socket address parsing helpers, loading from a path, and tests for default/sparse/full/invalid configs.

## Decisions

2026-05-11T17:46-0700 Config parser belongs in `argus-core` — Every binary and transport needs the same config shape. Keeping parsing and validation in core prevents daemon/TUI/MCP from growing separate interpretations.

2026-05-11T17:46-0700 Use typed TOML deserialization with defaults — The config file is user-authored and should accept sparse files. Validation should happen after deserialize so defaults and explicit values go through the same checks.

2026-05-11T18:05-0700 TLS cert/key defaults are unset — The design-doc sample shows cert/key fields, but defaulting both fields to concrete path strings prevents validation from catching a config that explicitly sets only one of them. Store unset defaults for now; when remote TLS lands, the daemon can generate or resolve default cert material deliberately.

## Issues

2026-05-11T18:05-0700 First validation run found two issues: Clippy rejected manual `Default` impls that could be derived, and the TLS pairing test exposed that concrete cert/key defaults masked partial user config. Both fixed.

## Verification

2026-05-11T18:05-0700 `cargo fmt --all -- --check` — passed.
2026-05-11T18:05-0700 `cargo check --workspace` — passed.
2026-05-11T18:05-0700 `cargo clippy --all-targets --all-features -- -D warnings` — passed.
2026-05-11T18:05-0700 `cargo test --workspace` — passed; 9 config tests.

## Commits

768466b — chore: add config parser
dc26ed3 — chore: address config review
