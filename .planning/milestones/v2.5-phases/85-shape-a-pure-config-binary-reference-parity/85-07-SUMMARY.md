---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 07
subsystem: code-mode
gap_closure: true
tags: [sql-code-mode, validation, authorization, require-limit, gap-closure]
requires:
  - "pmcp-code-mode CodeModeConfig + check_sql_config_authorization (Phase 83/85)"
  - "pmcp-server-toolkit build_cm_config / CodeModeSection.require_limit (Phase 83)"
provides:
  - "sql_require_limit enforced flag on CodeModeConfig (additive, default false)"
  - "missing_limit policy violation on bare read-only SELECT when sql_require_limit=true"
  - "toolkit require_limit -> sql_require_limit live mapping (no longer discarded)"
affects:
  - "SC-3 code-mode policy parity (regains correctness foundation for Gap 1)"
  - "Plan 85-08 (Gap 2) asserts against this enforcement"
tech-stack:
  added: []
  patterns:
    - "Additive #[serde(default, alias=...)] field on a published crate (non-breaking)"
    - "Single shared SQL authorization path (check_sql_config_authorization) covers both validate_sql_query sync + async"
key-files:
  created: []
  modified:
    - crates/pmcp-code-mode/src/config.rs
    - crates/pmcp-code-mode/src/validation.rs
    - crates/pmcp-server-toolkit/src/code_mode.rs
decisions:
  - "sql_require_limit lives only in the Select arm — write/DDL gating stays independent (require_limit is a read-only-query guard)"
  - "Used #[serde(default)] (bool default false) with alias require_limit — no default_* fn needed; configs that omit it are unchanged"
  - "Enforcement rule string is missing_limit (matches the planned reference observable Plan 85-08 asserts on)"
metrics:
  duration: 4m
  completed: 2026-05-27
  tasks: 2
  files: 3
---

# Phase 85 Plan 07: sql_require_limit Gap Closure Summary

Closed VERIFICATION Gap 1 (HIGH, correctness): the `[code_mode] require_limit` policy is now
enforced end-to-end. Added an additive, opt-in `sql_require_limit` flag to pmcp-code-mode's
`CodeModeConfig`, enforced it in the single shared SQL authorization path
(`check_sql_config_authorization`, used by BOTH `validate_sql_query` and
`validate_sql_query_async`), and replaced the toolkit's discarded `_require_limit_gap` with a
live `cfg.sql_require_limit = section.require_limit;` mapping. A bare `SELECT * FROM Artist`
under `require_limit=true` is now rejected (rule `missing_limit`) independent of the
`sql_max_rows` row-estimate heuristic; a `LIMIT`ed read and write/DDL statements are unaffected.

## What Was Built

### Task 1 — Add + enforce sql_require_limit in pmcp-code-mode (commit 74871aae)

- `crates/pmcp-code-mode/src/config.rs`: added
  `#[serde(default, alias = "require_limit")] pub sql_require_limit: bool` to `CodeModeConfig`
  (immediately after `sql_require_where_on_writes`), plus `sql_require_limit: false,` in
  `impl Default`. Additive and non-breaking for the published crate (T-85-07-03 accepted).
- `crates/pmcp-code-mode/src/validation.rs`: extended the `SqlStatementType::Select` arm of
  `check_sql_config_authorization` so that, after the existing `sql_reads_enabled` check, it
  returns a `ValidationResult::failure` with a `missing_limit` `PolicyViolation`
  (`.with_suggestion("Add a LIMIT clause (e.g. \`LIMIT 100\`).")`) when
  `self.config.sql_require_limit && !info.has_limit`. The branch lives only in the Select arm,
  so a single edit covers both the sync and async surfaces (both delegate to this function).
- 5 unit tests in `sql_tests`: rejects bare SELECT (`missing_limit`), accepts LIMITed SELECT,
  default (false) accepts bare SELECT (no regression), require_limit does not reject a write,
  serde round-trip (omitted → false, `require_limit = true` → true).

### Task 2 — Map toolkit require_limit -> sql_require_limit (commit 05c3f55e)

- `crates/pmcp-server-toolkit/src/code_mode.rs` `build_cm_config`: replaced
  `let _require_limit_gap = section.require_limit;` with
  `cfg.sql_require_limit = section.require_limit;` (with a comment documenting the Gap-1 closure).
- 2 `build_cm_config` mapping tests (require_limit true/false → sql_require_limit).
- 2 `#[cfg(all(test, feature = "sqlite"))]` executor tests: a new
  `read_only_executor_with_require_limit()` fixture (config sets `require_limit: true`); a bare
  `SELECT * FROM Artist` execute returns `Err(ExecutionError::BackendError(_))` (rejected on
  re-validation before the connector — row count unchanged at 1, proving it is the
  require_limit policy and NOT a row-count failure), and a `SELECT ... LIMIT 5` execute returns
  Ok with the seeded row. Proves the toolkit → pmcp-code-mode wiring is live through the same
  re-validation path `validate_code`/`execute_code` use.

## Verification

- `cargo test -p pmcp-code-mode --features sql-code-mode -- --test-threads=1` — 133 passed, 5
  ignored (includes the 5 new require_limit tests).
- `cargo test -p pmcp-server-toolkit --features "code-mode sqlite" -- --test-threads=1` — 191
  passed (includes the 4 new require_limit tests).
- `cargo clippy -p pmcp-code-mode --features sql-code-mode -- -D warnings` — No issues found.
- `rustfmt --check` clean on all three touched files.
- `grep -n "_require_limit_gap" crates/pmcp-server-toolkit/src/code_mode.rs` — 0 matches (discard line removed).
- `grep -n "sql_require_limit = section.require_limit"` — 1 match.

## Deviations from Plan

None — plan executed exactly as written. No Rule 1–3 auto-fixes were required.

## Deferred Issues

### `clippy::field_reassign_with_default` on `build_cm_config` (PRE-EXISTING, out of scope)

`cargo clippy -p pmcp-server-toolkit --features "code-mode sqlite" -- -D warnings` reports a
single `clippy::field_reassign_with_default` error at `code_mode.rs:471-472` — the
`let mut cfg = CodeModeConfig::default();` + `cfg.enabled = section.enabled;` opener of
`build_cm_config`. This is a PRE-EXISTING rust-1.95.0-vs-CI lint already logged in
`.planning/phases/85-shape-a-pure-config-binary-reference-parity/deferred-items.md` (Phase 83
code; line numbers shifted as the function grew). Plan 85-07's mapping line
(`cfg.sql_require_limit = section.require_limit;`) REUSES the identical existing `cfg.field = …`
reassignment pattern the whole function already uses (`cfg.enabled`, `cfg.sql_allow_writes`,
`cfg.sql_max_rows`, …) — it introduces no new lint category. Removing the lint would require
rewriting the entire `build_cm_config` body to a struct-literal initializer, which is out of
scope per the 85-07 PLAN verification note ("pre-existing toolkit clippy/fmt diffs … are NOT in
scope; verify the FILES THIS PLAN TOUCHES are fmt-clean"). The deferred-items.md note was
extended with a Plan 85-07 entry. Fix when the CI toolchain advances.

## Threat Surface

The two `mitigate` dispositions in the plan's threat register (T-85-07-01 EoP, T-85-07-02 DoS)
are now realized: an unbounded read-only statement is rejected at validation time, before the
connector executes it, independent of the row-estimate heuristic. T-85-07-03 (additive serde
field on a published crate) holds — `sql_require_limit` is `#[serde(default)]`, default false,
no rename. No NEW security surface beyond the planned threat model was introduced.

## Self-Check: PASSED
