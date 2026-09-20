---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 01
subsystem: config
tags: [pmcp-server-toolkit, serde, toml, env-expansion, hmac, code-mode, ref-01]

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift
    provides: ServerConfig + sub-sections, deny_unknown_fields strict-parse discipline, resolve_token_secret env: branch, validation_pipeline_from_config
  - phase: 84-sql-connectors
    provides: DatabaseSection.url field, SQLite connector consuming the database file path
provides:
  - Additive REF-01 superset fields DatabaseSection.file_path, ServerSection.is_reference, ServerConfig.shared_policy_store + SharedPolicyStoreSection struct
  - ${VAR} env-expansion branch in resolve_token_secret (coexists with env:VAR), scoped to token_secret only
  - config_superset.rs regression gate (all 4 reference configs + renames_rejected + ${VAR}-verbatim)
  - env_expansion.rs gate (resolve/missing-err/no-regression/proptest no-panic)
  - Vendored Chinook reference-config.toml fixture
affects: [85-02-shape-a-binary, 85-03, 86-shapes-bcd, reference-parity]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Additive superset field — always ADD the missing field under #[serde(default)], never loosen deny_unknown_fields (config.rs:24, RESEARCH Pitfall 1)"
    - "expand_braced_var(&str) -> Option<&str> exact-shape matcher keeps ${VAR} expansion scoped to token_secret; substrings/malformed forms fall through to inline-secret handling (preserves R9)"
    - "Secret-resolution forms (env:VAR / ${VAR}) coexist as ordered branches in resolve_token_secret; missing var -> Err, never panic, never weak fallback (T-85-01-01)"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/config_superset.rs
    - crates/pmcp-server-toolkit/tests/env_expansion.rs
    - crates/pmcp-server-toolkit/tests/fixtures/reference-config.toml
  modified:
    - crates/pmcp-server-toolkit/src/config.rs
    - crates/pmcp-server-toolkit/src/code_mode.rs

decisions:
  - "${VAR} expansion intentionally scoped to token_secret only (Codex MEDIUM #6); non-secret fields like Athena output_location keep ${...} verbatim — general from_toml_with_env_expansion deferred, not a Phase 85 blocker"
  - "env_expansion.rs drives ${VAR} through the public validation_pipeline_from_config because resolve_token_secret is private; proptest no-panic invariant runs through the same public seam"
  - "renames_rejected uses a `filepath` typo (no underscore) to prove deny_unknown_fields stays strict for the new field"

metrics:
  duration: 14m
  completed: 2026-05-27
  tasks: 2
  files: 5
---

# Phase 85 Plan 01: REF-01 Superset + ${VAR} Token-Secret Expansion Summary

**SQLite Chinook reference config now parses: added additive `file_path` / `is_reference` / `[shared_policy_store]` fields and a `${VAR}` token-secret env-expansion branch alongside `env:VAR`, with two regression gates covering all four reference configs.**

## Performance

- **Duration:** ~14 min
- **Completed:** 2026-05-27
- **Tasks:** 2
- **Files created/modified:** 5

## Accomplishments

- Closed RESEARCH Gap #1 (REF-01 superset): the SQLite Chinook `reference-config.toml` — which previously FAILED on `unknown field 'file_path'` — now parses + validates via `ServerConfig::from_toml_strict_validated`. All four reference configs (open-images, imdb, msr-vtt, Chinook) parse + validate.
- Closed RESEARCH Gap #3 + threat T-85-01: `token_secret = "${CODE_MODE_SECRET}"` (the form every reference config emits) now resolves from the named env var; a missing/unset var returns `Err(ToolkitError::CodeMode)` cleanly, never a panic and never a weak/empty-secret fallback.
- Preserved REF-01 invariant: `deny_unknown_fields` was NOT loosened anywhere (struct count 20 → 21, additive only); a typo'd field (`filepath`) is still rejected.
- Preserved SC-2 parse-only (REVIEW FIX #6): Athena `${VAR}` in `output_location` parses verbatim — `${VAR}` expansion is scoped to `token_secret` only, so no config-load-time expansion is attempted on non-secret fields.

## Task Commits

Each task was committed atomically (TDD: failing test → implementation in one feat commit per task):

1. **Task 1: Add REF-01 superset fields (file_path / is_reference / [shared_policy_store])** - `566ca0a1` (feat)
2. **Task 2: Add ${VAR} env-expansion for token_secret** - `54efb538` (feat)

_TDD note: each task wrote the failing test first (RED confirmed via compile-error for Task 1, assertion-failure for Task 2), then the additive implementation made it green. RED and GREEN landed in one `feat` commit per task since the test and the additive field/branch form a single feature unit; the RED state is documented here and reproducible by reverting the source half of each commit._

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/config.rs` - Added `DatabaseSection.file_path: Option<String>`, `ServerSection.is_reference: bool`, `ServerConfig.shared_policy_store: Option<SharedPolicyStoreSection>`, and a new `SharedPolicyStoreSection` struct (`creates_shared_store`/`export_to_ssm`/`ssm_path`/`templates`), all `#[serde(default)]` with `deny_unknown_fields`. Updated the module-doc REF-01 superset enumeration.
- `crates/pmcp-server-toolkit/src/code_mode.rs` - Added `expand_braced_var(&str) -> Option<&str>` helper and a `${VAR}` branch in `resolve_token_secret` after the `env:` branch (coexists; missing var → `Err(ToolkitError::CodeMode)`).
- `crates/pmcp-server-toolkit/tests/config_superset.rs` - 6 tests: Chinook parse+validate (asserts `file_path`/`backend_type`/`is_reference`/`[shared_policy_store]`), 3 existing-config no-regression checks, `renames_rejected`, `var_in_output_location_parses_verbatim`.
- `crates/pmcp-server-toolkit/tests/env_expansion.rs` - 5 tests: `${VAR}` resolves, missing-var errors without panic, `env:` no-regression, R9 inline rejection unchanged, proptest no-panic invariant over arbitrary `${name}` strings.
- `crates/pmcp-server-toolkit/tests/fixtures/reference-config.toml` - Vendored verbatim from `pmcp-run/built-in/sql-api/reference/config.toml` (the Chinook reference snapshot).

## Decisions Made

- **`${VAR}` scoped to `token_secret` only** (Codex MEDIUM #6): non-secret config fields (Athena `output_location`/`region`/`workgroup`) keep `${...}` verbatim. A general `from_toml_with_env_expansion` raw-TOML pre-pass is a deferred forward-compatible enhancement, NOT a Phase 85 blocker. `var_in_output_location_parses_verbatim` pins this.
- **Test seam via public API:** `resolve_token_secret` is private, so `env_expansion.rs` drives `${VAR}` resolution (and the proptest no-panic invariant) through the public `validation_pipeline_from_config` entry point that consumes it.
- **`expand_braced_var` exact-shape matcher:** matches only `${NAME}` with non-empty `NAME`; substrings containing `${` or malformed `${` fall through to the inline-secret handling, preserving the R9 guarantee and keeping cognitive complexity ≤ 25.

## Deviations from Plan

None - plan executed exactly as written. Both tasks landed their planned fields/branch and all six acceptance criteria across the two tasks are green.

## Deferred Issues

Out-of-scope pre-existing clippy lints surfaced under the local Rust toolchain (NOT introduced by this plan; consistent with the STATE.md note about pre-existing rust-1.95.0 pedantic lints in the workspace):

- `crates/pmcp-server-toolkit/src/code_mode.rs:207-208` — `field assignment outside of initializer` in `build_cm_config` (Phase 83 code, untouched by this plan).
- `crates/pmcp-server-toolkit/src/builder_ext.rs:284` — `unneeded return statement` (untouched by this plan).
- `cargo fmt --check` diffs in `src/sql/sqlite.rs` and `tests/synthesizer_structured_content.rs` (untouched by this plan).

These are not fixed here per the SCOPE BOUNDARY rule (only auto-fix issues directly caused by this plan's changes). My five touched files are fmt-clean and clippy-clean.

## Issues Encountered

None. RED/GREEN cycle was clean for both tasks: Task 1 RED was a compile error (fields absent), Task 2 RED was an assertion failure (`${VAR}` falling through to `InlineSecretRejected`); both went green on the first implementation pass.

## User Setup Required

None - no external service configuration required. `${VAR}`/`env:VAR` token-secret resolution reads process env at server-build time; production deployments must set `CODE_MODE_SECRET` (≥16 bytes) but that is a deployment concern, not a Plan 85-01 setup step.

## Verification

```
cargo test -p pmcp-server-toolkit --features code-mode \
  --test config_superset --test env_expansion --test reference_configs --test code_mode_wiring \
  -- --test-threads=1
```
→ 24 passed (4 suites). `deny_unknown_fields` count 20 → 21 (additive, no removal). `expand_braced_var` + `resolve_token_secret` cognitive complexity ≤ 25 (PMAT: no violations). `env:` branch preserved (grep matches).

## Next Phase Readiness

REF-01 SC-2 (superset, no renames) and Gap #3 (`${VAR}`) are closed — the foundation gate for Wave 2's `pmcp-sql-server` binary is in place. The binary can now build a `pmcp::Server` from the Chinook reference config (parse + validate + token-secret resolution all succeed). No blockers for Plan 85-02.

---
*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Completed: 2026-05-27*

## Self-Check: PASSED

All 5 created/modified source+test+fixture files present; both task commits (`566ca0a1`, `54efb538`) exist in git history.
