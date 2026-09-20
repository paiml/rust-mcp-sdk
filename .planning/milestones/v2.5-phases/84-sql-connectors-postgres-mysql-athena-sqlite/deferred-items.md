# Phase 84 — Deferred / Out-of-Scope Items

Items discovered during execution that fall outside the current plan's scope
(SCOPE BOUNDARY rule: only auto-fix issues directly caused by the current task's
changes). Logged here, NOT fixed.

## Discovered during Plan 84-01 (Wave 1, SqlConnector.execute + ConnectorError)

### Pre-existing clippy lints surfaced by local toolchain (rust-1.95.0)

The local clippy toolchain (rust-1.95.0) is newer than CI's pinned
`dtolnay/rust-toolchain@stable`, so several lints fire locally that did not gate
the Phase 83 / Wave 0 commits when they were written. None of these are in the
file Plan 84-01 modified (`src/sql/mod.rs` is clippy-clean at `-D warnings`).

| File | Lines | Lint | Owner / disposition |
|------|-------|------|--------------------|
| `crates/pmcp-server-toolkit/src/builder_ext.rs` | 178 | `clippy::needless_return` | Phase 83 code; fix in a Phase 83 follow-up or whenever CI toolchain advances |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | 207-208 | `clippy::field_reassign_with_default` | Phase 83 code; same disposition |
| `crates/pmcp-server-toolkit/src/sql/translate.rs` | 76, 79, 89, 99, 109, 119 | various (RED proptest scaffold) | Wave 0 (84-00) RED proptest shell; resolves in Plan 84-02 when `translate_placeholders` lands GREEN |
| `crates/pmcp-widget-utils/src/lib.rs` | 46, 50 | `clippy::uninlined_format_args` (pedantic) | unrelated dependency crate; pedantic-only, fix when CI toolchain advances |

### Pre-existing RED tests (intentional, Plan 84-02 territory)

`src/sql/translate::proptests::*` (5 proptests) panic with `RED — Plan 02
implements`. These were committed RED in Wave 0 (84-00, commit `c11f0962`) and
turn GREEN in Plan 84-02 when the `SqlWalker` state machine ships. Plan 84-01
deliberately does not touch them (per the executor context note).
