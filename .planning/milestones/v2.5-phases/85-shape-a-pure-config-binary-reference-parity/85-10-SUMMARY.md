---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 10
subsystem: api
tags: [code-mode, sql-server, shape-a, config-driven, robustness, gap-closure]

# Dependency graph
requires:
  - phase: 85-shape-a-pure-config-binary-reference-parity (Plan 07)
    provides: require_limit mapping in build_cm_config (code_mode.rs co-edit base)
  - phase: 85-shape-a-pure-config-binary-reference-parity (Plan 04)
    provides: dispatch_sqlite + DispatchError::MissingField/SqliteOpen
  - phase: 84-sql-connectors
    provides: SqlConnector::execute(&[(String, Value)]) named-param binding
provides:
  - "execute_code variables input is BOUND as named params (no silent drop)"
  - "SqlCodeExecutor caches its ValidationPipeline at construction (token_secret resolved once)"
  - "extract_named_params treats explicit JSON null like missing → applies declared default"
  - "set-but-empty token_secret / AWS_REGION env vars treated as UNSET"
  - "serve-task JoinError propagated as RunError::Serving → non-zero exit"
  - "sqlite database = \":memory:\"/<path> fallback per DatabaseSection docs"
affects: [86-shapes-bcd, 88-dogfood, reference-parity, operator-robustness]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Schema-advertised inputs are honored (bound) — never silently dropped (a dropped input invites unsafe string-interpolation)"
    - "Resolve secrets/config ONCE at construction; cache; do not re-read process env per request"
    - "Set-but-empty/whitespace env value is treated as UNSET (non_empty_env / trim().is_empty()), not as a present empty value"
    - "JoinError on the long-lived serve task is propagated (non-zero exit), not discarded"

key-files:
  created: []
  modified:
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/src/tools.rs
    - crates/pmcp-server-toolkit/src/builder_ext.rs
    - crates/pmcp-sql-server/src/lib.rs
    - crates/pmcp-sql-server/src/dispatch.rs
    - crates/pmcp-sql-server/tests/dispatch.rs

key-decisions:
  - "WR-02 DESIGN: HONOR the execute_code variables (bind them) rather than reject — variables_to_params() maps a JSON object to (name, value) pairs, stripping a leading ':' to match the connector keying; None/non-object → empty so the parity scenario (None) is unaffected"
  - "IN-01: SqlCodeExecutor::new became fallible (Result<Self>) so the pipeline is built ONCE and cached; the single construction site (builder_ext) and the test constructors propagate the Result; this also resolves token_secret once at builder time"
  - "null-default: a .filter(|v| !v.is_null()) before .cloned() makes explicit JSON null fall through to the declared default (no LIMIT NULL bind)"
  - "empty token_secret: a shared resolve_secret_env_var() helper backs both env: and ${VAR} forms; an empty/whitespace value is a clear 'set but empty' CodeMode error"
  - "JoinError: added RunError::Serving(#[source] JoinError); run() now does handle.await.map_err(RunError::Serving)? so main.rs surfaces a non-zero exit"
  - "sqlite database fallback: file_path.or(database) with file_path precedence; MissingField field string names both ('file_path` or `database')"
  - "empty AWS_REGION: non_empty_env() (athena-gated, alongside resolve_athena_region) treats empty/whitespace as None so the AWS_DEFAULT_REGION/static-default fallback fires"

patterns-established:
  - "Pattern: a schema-advertised tool input is never a silent no-op — bind it or reject with a clear error"
  - "Pattern: cache the validation pipeline / resolved secret at construction; per-request env reads are a robustness hazard"
  - "Pattern: env-var presence checks reject set-but-empty values (trim().is_empty()) to avoid degenerate config"

requirements-completed: [SHAP-A-01, REF-02]

# Metrics
duration: 9min
completed: 2026-05-26
---

# Phase 85 Plan 10: Secondary Gap Closure (WR-02 + IN-01/IN-04 robustness) Summary

**Six lower-severity contract/robustness gaps are closed: `execute_code`'s `variables` input is now BOUND as named params (never silently dropped), the validation pipeline is built once and cached (token_secret resolved a single time), an explicit JSON `null` applies the declared default, set-but-empty `token_secret`/`AWS_REGION` env vars are treated as unset, a serve-task panic propagates as a non-zero exit, and the documented `database = ":memory:"` SQLite form is accepted — all unit-tested with no parity regression.**

## Performance

- `cargo test -p pmcp-server-toolkit --features "code-mode sqlite" -- --test-threads=1`: 201 passed.
- `cargo test -p pmcp-sql-server --no-default-features --features sqlite -- --test-threads=1`: 39 passed (incl. parity_chinook, dispatch).
- `cargo test -p pmcp-sql-server --no-default-features --features athena --lib` (region/serving units): 4 passed.

## What Changed

### Task 1 — Toolkit fixes (commit d962051e)

1. **WR-02 (variables, T-85-10-01).** `SqlCodeExecutor::execute` previously bound `&[]`, silently dropping the schema-advertised `variables` input. A new `variables_to_params(Option<&Value>) -> Vec<(String, Value)>` maps a JSON object's entries to `(name, value)` pairs (stripping a leading `:` to match the connector's `translate_placeholders` keying), and `execute` now binds them. `None`/non-object → empty slice, so the parity `execute_code` scenario (passes `None`) is byte-for-byte unaffected. Binding (not interpolation) preserves parameterized-query safety.
2. **IN-01 (cached pipeline, T-85-10-03).** `SqlCodeExecutor` gained a `pipeline: Arc<ValidationPipeline>` field built ONCE in `new` (now `Result<Self, ToolkitError>`). `revalidate` reuses it instead of rebuilding + re-reading the `token_secret` env var per call. The single construction site (`builder_ext::try_code_mode_from_config_with_connector`) and the two test constructors propagate the `Result`.
3. **null-default (tools.rs).** `extract_named_params` now `.filter(|v| !v.is_null())` before `.cloned()`, so `{"limit": null}` falls through to the declared default (`20`) instead of binding `LIMIT NULL`.
4. **empty token_secret (T-85-10-03).** A shared `resolve_secret_env_var` helper backs both the `env:VAR` and `${VAR}` branches; an empty/whitespace value returns a clear `CodeMode("env var '…' is set but empty for token_secret")` error rather than wrapping a degenerate `SecretValue`.

### Task 2 — pmcp-sql-server fixes (commit 25c58962)

1. **JoinError propagation (T-85-10-02).** Added `RunError::Serving(#[source] tokio::task::JoinError)`; `run()` replaced `let _ = handle.await;` with `handle.await.map_err(RunError::Serving)?`. A serve-task panic now surfaces as a non-zero process exit (main.rs returns `run()`'s `Result`), letting a supervisor restart a crashed listener.
2. **sqlite database fallback.** `dispatch_sqlite` resolves `file_path.or(database)` (file_path precedence) per the `DatabaseSection` docs, so `database = ":memory:"` / `database = "<path>"` now opens the backend. `MissingField` names both fields (`file_path` or `database`).
3. **empty AWS_REGION.** A `non_empty_env` helper (athena-gated) treats empty/whitespace as `None`, so an empty `AWS_REGION` falls through to `AWS_DEFAULT_REGION` and then the static `us-east-1` default instead of yielding an empty region.

## Tests Added

- `tools::tests`: `extract_named_params_applies_default_when_absent`, `_explicit_null_applies_default`, `_explicit_value_overrides_default`.
- `code_mode::tests`: `resolve_token_secret_empty_env_var_is_set_but_empty_error`, `_whitespace_env_var_…`, `variables_to_params_maps_object_stripping_colon_prefix`, `_none_or_non_object_is_empty`.
- `code_mode::sql_code_executor_tests`: `execute_binds_variables_input`, `execute_empty_variables_is_unaffected`, `pipeline_cached_at_construction_not_reread_per_execute`.
- `dispatch::region_tests` (athena-gated): `non_empty_env_treats_empty_and_whitespace_as_unset`, `empty_aws_region_falls_through_to_default_region`, `both_region_vars_empty_use_static_default`, `aws_region_wins_when_set_non_empty`.
- `lib::tests`: `serving_task_panic_maps_to_run_error_serving`, `run_error_serving_display_is_descriptive`.
- `tests/dispatch.rs`: `sqlite_database_memory_form_opens_in_memory`, `sqlite_database_file_path_form_opens_file`, `sqlite_file_path_takes_precedence_over_database`; renamed/updated `sqlite_without_file_path_or_database_reports_missing_field`.

## Deviations from Plan

None for the substance of the fixes — all six findings implemented as specified, with the plan's recommended designs (honor variables; eager-construct cached pipeline via fallible `new`; `is_null` filter; shared empty-env helper; `RunError::Serving`; `file_path.or(database)`; `non_empty_env`).

**[Rule 3 - Blocking] Updated the existing `sqlite_without_file_path_reports_missing_field` integration test.** The `MissingField` `field` string changed from `"file_path"` to `"file_path` or `database"`, which would have broken the pre-existing assertion. Renamed it to `sqlite_without_file_path_or_database_reports_missing_field` and updated the matched field string. Necessary to keep the suite green after the documented field-rename in the fix.

## Deferred / Out of Scope

- **IN-04 (`merge_schema_resource` `ends_with("/schema")` over-broad match)** is owned by Plan 85-09 (assemble.rs) and was folded there — NOT duplicated here to keep file ownership clean (confirmed in 85-09-SUMMARY: "Scoped merge_schema_resource /schema override to the FIRST match").

## Known Stubs

None. All six fixes are wired end-to-end (bound variables reach the connector; the cached pipeline drives revalidation; the JoinError reaches the process exit; the sqlite fallback reaches `SqliteConnector::open*`).

## Pre-existing / Deferred Lint (NOT introduced here)

`cargo clippy -p pmcp-server-toolkit --features "code-mode sqlite"` reports `clippy::field_reassign_with_default` on `build_cm_config` (code_mode.rs ~line 520, `let mut cfg = CodeModeConfig::default(); cfg.enabled = …`). This is the documented deferred pmcp-server-toolkit pedantic lint (rust-1.95.0) — it is in a function NOT touched by this plan (confirmed absent from the diff). All files this plan modified are clippy-clean and rustfmt-clean (`cargo fmt --check` passes for both crates).

## Threat Coverage

| Threat ID | Disposition | Status |
|-----------|-------------|--------|
| T-85-10-01 (execute_code variables silent drop) | mitigate | DONE — variables bound as named params |
| T-85-10-02 (serve-task panic exits 0) | mitigate | DONE — RunError::Serving → non-zero exit |
| T-85-10-03 (empty token_secret / per-request env reread) | mitigate | DONE — cached pipeline + 'set but empty' error |
| T-85-10-04 (sqlite fallback error info disclosure) | accept | unchanged — MissingField names fields only; SqliteOpen path-free |

## Self-Check: PASSED

- All 6 modified files exist on disk.
- Both task commits (d962051e, 25c58962) exist in git history.
- Toolkit suite: 201 passed; sql-server (sqlite) suite: 39 passed; athena region/serving units: 4 passed.
