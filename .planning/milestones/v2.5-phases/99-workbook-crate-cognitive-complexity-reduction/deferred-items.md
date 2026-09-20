# Phase 99 — Deferred Items (out-of-scope discoveries)

Discovered during plan 99-10 execution. These are PRE-EXISTING clippy warnings in
files NOT touched by this plan (scope boundary: only auto-fix issues directly caused
by the current task). They predate commit a4d60cb6 and are unrelated to the
`workbook/input.rs::validate_input` refactor.

- `crates/pmcp-code-mode/src/avp.rs:59` — `clippy::derivable_impls`: manual
  `impl Default for AvpConfig` can be `#[derive(Default)]`. (rust-1.95 clippy.)
- `crates/pmcp-server-toolkit/src/http/auth.rs:538` — `clippy::redundant_guards`:
  `Some(name) if name.is_empty()` can be `Some("")`.
- `crates/pmcp-server-toolkit/...` test build — `unused import: pmcp_code_mode::CodeExecutor`
  (test-only, unrelated module).

NOTE: per project MEMORY, the real merge gate (`make lint` / CI `ci.yml`) lints only
the root `pmcp` crate with a generous allow-list; the toolkit/code-mode crates are not
`-D warnings` clippy-gated, so these do not block CI. Left untouched intentionally.
