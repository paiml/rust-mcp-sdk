---
phase: 84-sql-connectors-postgres-mysql-athena-sqlite
plan: 02
subsystem: database
tags: [sql, placeholder-translation, state-machine, postgres, mysql, athena, sqlite, proptest, tdd]

# Dependency graph
requires:
  - phase: 84-00
    provides: "translate.rs shell + TranslatedSql struct + 5 RED proptest stubs"
  - phase: 84-01
    provides: "SqlConnector::execute() 3-method trait that consumes translate_placeholders"
provides:
  - "Full SqlWalker char-by-char state machine implementing translate_placeholders (CONN-03)"
  - "Dialect-aware emission: $1.. (Postgres), ? (MySQL/Athena), :name identity (SQLite) with bind-order preservation"
  - "REVIEWS H7 colon-precedence: ::text casts, := session-vars, :1bad malformed all emitted verbatim"
  - "5 GREEN property tests + 26 edge-case unit tests (incl. 4 mandatory H7 named tests)"
affects: [84-04, 84-05, 84-06, 84-07, 85, 86]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Split-helper state machine (Pattern G): SqlWalker with run/handle_* helpers each under PMAT cog 25"
    - "Peekable<Chars> lookahead for colon-precedence + comment/literal boundary detection"

key-files:
  created: []
  modified:
    - "crates/pmcp-server-toolkit/src/sql/translate.rs — SqlWalker state machine + GREEN tests"

key-decisions:
  - "CastTypeName transitional state swallows the type identifier after `::` so no placeholder lookup happens mid-cast (handles 1::int, :id::text, 'foo'::text uniformly)"
  - "Placeholder state is only entered when peek() is [A-Za-z_], so pending_name is never empty — no empty-name flush path exists"
  - "format!() strings built into locals before prop_assert! because `${i}` inside the concat!-expanded macro cannot capture surrounding variables"

patterns-established:
  - "Pattern G split-helper decomposition keeps every SqlWalker method at cog <= 25 with zero #[allow(clippy::cognitive_complexity)]"
  - "is_ident_start / is_ident_continue free functions encode the [A-Za-z_][A-Za-z0-9_]* identifier grammar shared by dispatch_colon + handle_placeholder + handle_cast_type_name"

requirements-completed: [CONN-03]

# Metrics
duration: 12min
completed: 2026-05-26
---

# Phase 84 Plan 02: translate_placeholders SqlWalker State Machine Summary

**Dialect-aware `:name` placeholder translation via a 6-state SqlWalker char scanner — $1.. for Postgres, ? for MySQL/Athena, :name identity for SQLite — with bind-order preservation and REVIEWS H7 guards against ::text-cast, :=-session-var, and :1bad mis-translation.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-05-26T20:53:00Z (approx)
- **Completed:** 2026-05-26T21:06:00Z
- **Tasks:** 1 (GREEN gate of a `type: tdd` plan; RED gate was committed in Plan 00)
- **Files modified:** 1

## Accomplishments

- Replaced the Wave 0 verbatim-passthrough stub of `translate_placeholders` with the full `SqlWalker` state machine over `Peekable<Chars>`, tracking 6 states (`Normal`, `StringLiteral(char)`, `LineComment`, `BlockComment(usize)`, `Placeholder`, `CastTypeName`).
- Turned the 5 RED proptests GREEN: idempotence (no-placeholder identity for all dialects), bind-order preservation, Postgres contiguous positional indexing, SQLite SQL identity, and no-panic on arbitrary `String` input.
- Implemented the REVIEWS H7 colon-precedence rule: a `:` only begins a placeholder when the next char is `[A-Za-z_]`; `::` is a verbatim Postgres cast (the following type identifier is swallowed by `CastTypeName`), `:=` is a MySQL session-var, and `:1bad` is malformed — all emit the bare `:` verbatim and stay in `Normal`.
- Added 26 edge-case unit tests including the 4 mandatory H7-named tests, string-literal/line-comment/block-comment (nested) skipping, doubled-quote escape, repeated-name fresh-index, unterminated literal/comment no-panic, and lone-colon-at-EOF.
- Verified PMAT reports zero functions exceeding cognitive complexity 25 in `translate.rs`, with no `#[allow(clippy::cognitive_complexity)]` anywhere.

## Task Commits

This is a `type: tdd` plan. The RED gate landed in Plan 00; this plan supplies the GREEN gate:

1. **RED gate (Plan 00 baseline):** `c11f0962` — `test(84-00): add translate.rs shell + 5 RED property tests`
2. **GREEN gate (this plan):** `e9a894e4` — `feat(84-02): implement SqlWalker state machine with split-helper decomposition + ::text cast handling`

**REFACTOR gate:** Not needed — PMAT was clean on first pass (every helper under cog 25), so no `refactor(84-02)` commit was created.

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/sql/translate.rs` — `SqlWalker<'a>` struct + `State` enum + split-helper methods (`new`, `run`, `handle_normal`, `dispatch_colon`, `handle_placeholder`, `handle_cast_type_name`, `emit_placeholder_from_pending`, `handle_string`, `handle_line_comment`, `handle_block_comment`, `into_translated`) + `is_ident_start`/`is_ident_continue` helpers; `mod proptests` (5 invariants) and `mod unit_tests` (26 cases). 585 lines.

## Decisions Made

- **CastTypeName as a one-shot swallow state:** instead of a countdown flag, a dedicated transitional state consumes the entire identifier run following `::`. This makes `1::int`, `:id::text`, and `'foo'::text` all fall out of the same rule without special-casing.
- **No empty-name flush path:** because `Placeholder` is only entered when `peek()` is an identifier-start char, `pending_name` is guaranteed non-empty at emission, simplifying `emit_placeholder_from_pending`.
- **Proptest format-string workaround:** `${i}` inside a `prop_assert!` (which expands through `concat!`) cannot capture a surrounding variable, so the `$N` token strings are built into `let` bindings first.

## Deviations from Plan

None — plan executed exactly as written. The plan's `<implementation>` proposed two helper-naming variants; the implemented `dispatch_colon` split (extracted out of `handle_normal`) is the variant the plan explicitly offered ("Split if needed: `fn dispatch_colon`"), so this is a planned option, not a deviation.

## Issues Encountered

- **Proptest macro + `$N` format strings:** the first compile failed because `prop_assert!(t.sql.contains(&format!("${i}")))` cannot capture `i` through the macro's `concat!` expansion. Resolved by building `let token = format!("${i}");` before the assertion. No behavior change.
- **`cargo fmt` reflow:** match arms in `handle_normal` / `emit_placeholder_from_pending` needed trailing commas per rustfmt; applied via `cargo fmt -p pmcp-server-toolkit` (only `translate.rs` affected).

## Verification Results

- `cargo test -p pmcp-server-toolkit --features sqlite --lib sql::translate::` — 31 passed (5 proptests + 26 unit tests), 0 failed.
- `cargo test --doc -p pmcp-server-toolkit --features sqlite` — 26 doctests passed (includes the 2 new `translate.rs` doctests).
- `cargo clippy -p pmcp-server-toolkit --features sqlite --all-targets -- -D warnings` — 0 warnings attributable to `translate.rs` (the only `-D warnings` failures are PRE-EXISTING toolchain-1.95.0 lints in `builder_ext.rs` + `code_mode.rs`, out of scope, already logged to `deferred-items.md`).
- `cargo fmt --all -- --check` — clean for `translate.rs`.
- `pmat analyze complexity --max-cognitive 25 --path crates/pmcp-server-toolkit/src/sql/translate.rs` — 0 functions exceeding cog 25.
- `git grep -n "#\[allow(clippy::cognitive_complexity)\]" crates/pmcp-server-toolkit/src/sql/translate.rs` — no matches.
- All 4 REVIEWS H7 named tests present (`postgres_double_colon_cast_preserves_text_identifier`, `postgres_double_colon_int_cast_no_placeholder`, `mysql_session_variable_assignment_not_a_placeholder`, `colon_followed_by_digit_emits_verbatim`).
- `cargo check --workspace` — green.

## Out-of-Scope Note (toolchain caveat)

The local toolchain is Rust 1.95.0, newer than CI's pinned stable, surfacing PRE-EXISTING clippy warnings in unrelated files (`builder_ext.rs`, `code_mode.rs`, `pmcp-widget-utils/lib.rs`). These were NOT touched — they are out of scope for CONN-03 and remain logged to `deferred-items.md`. The pre-commit quality-gate hook passed for this commit.

## Next Phase Readiness

- `translate_placeholders` is now production-ready and is the helper every Wave 2 per-backend `execute()` impl (84-05 Postgres, 84-06 MySQL, 84-07 Athena) and 84-04 (`SqliteConnector`) calls as `let TranslatedSql { sql, ordered_params } = translate_placeholders(canonical, dialect);`.
- No blockers introduced. The bind-order contract (`ordered_params` left-to-right, fresh positional index per appearance) is the stable interface those plans depend on.

## Self-Check: PASSED

- `crates/pmcp-server-toolkit/src/sql/translate.rs` exists and contains `struct SqlWalker`.
- `.planning/phases/84-sql-connectors-postgres-mysql-athena-sqlite/84-02-SUMMARY.md` exists.
- GREEN gate commit `e9a894e4` present in git history.

---
*Phase: 84-sql-connectors-postgres-mysql-athena-sqlite*
*Completed: 2026-05-26*
