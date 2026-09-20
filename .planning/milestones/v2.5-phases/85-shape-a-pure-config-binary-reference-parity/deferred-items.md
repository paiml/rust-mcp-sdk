# Phase 85 — Deferred / Out-of-Scope Items

Items discovered during execution that fall outside the current plan's scope
(SCOPE BOUNDARY rule: only auto-fix issues directly caused by the current task's
changes). Logged here, NOT fixed.

## Discovered during Plan 85-03 (Wave 1, scaffold pmcp-sql-server + vendor fixtures)

### Pre-existing clippy lints surfaced by local toolchain (rust-1.95.0)

`cargo clippy -p pmcp-sql-server --all-features -- -D warnings` escalates
warnings across the whole compilation graph and so surfaces lints in the
`pmcp-server-toolkit` dependency, NOT in the new `pmcp-sql-server` crate. These
are the same pre-existing rust-1.95.0 lints already logged in Phase 84's
`deferred-items.md` (local toolchain newer than CI's pinned stable):

| File | Lines | Lint | Owner / disposition |
|------|-------|------|--------------------|
| `crates/pmcp-server-toolkit/src/builder_ext.rs` | 284 | `clippy::needless_return` | Phase 83 code; fix when CI toolchain advances |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | 207-208 | `clippy::field_reassign_with_default` | Phase 83 code; same disposition |

`crates/pmcp-sql-server/src/*` is clippy-clean at `-D warnings` (0 warnings
attributable to this crate's sources). The deferral matches the Phase 84
precedent and the STATE.md note that broad `make quality-gate` is blocked by
pre-existing unrelated rust-1.95.0 pedantic lints in workspace dependencies.

## Discovered during Plan 85-07 (gap closure, sql_require_limit wiring)

### `clippy::field_reassign_with_default` on `build_cm_config` (PRE-EXISTING)

`cargo clippy -p pmcp-server-toolkit --features "code-mode sqlite" -- -D warnings`
reports a single `clippy::field_reassign_with_default` error at
`crates/pmcp-server-toolkit/src/code_mode.rs:471-472` — the
`let mut cfg = CodeModeConfig::default();` + `cfg.enabled = section.enabled;`
opener of `build_cm_config`. This is the SAME pre-existing lint already logged
above (the line numbers shifted as the function grew; it is the same Phase 83
`build_cm_config` mapping body). Plan 85-07's mapping line
(`cfg.sql_require_limit = section.require_limit;`) REUSES the identical existing
`cfg.field = …` reassignment pattern that the entire function already uses for
`cfg.enabled`, `cfg.sql_allow_writes`, `cfg.sql_max_rows`, etc. — it does NOT
introduce a new lint category, and removing the lint would require rewriting the
whole `build_cm_config` body (out of scope, untouched-pattern, deferred per the
85-07 PLAN verification note "pre-existing toolkit clippy/fmt diffs … are NOT in
scope; verify the FILES THIS PLAN TOUCHES are fmt-clean"). The touched file is
`rustfmt --check`-clean. Fix when the CI toolchain advances and the whole
`build_cm_config` body is refactored to a struct-literal initializer.
