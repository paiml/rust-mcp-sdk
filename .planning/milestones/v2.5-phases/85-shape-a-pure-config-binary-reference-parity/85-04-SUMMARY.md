---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 04
subsystem: cli-dispatch
tags: [pmcp-sql-server, shape-a, clap, dispatch, sql-connector, athena-offline, credential-redaction, novel-seam]

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift
    provides: "ServerConfig (DatabaseSection.backend_type/file_path/url/workgroup/output_location/database), SqlConnector trait + ConnectorError, sqlite SqliteConnector"
  - phase: 84-sql-connectors
    provides: "PostgresConnector::connect (lazy pool) / MysqlConnector::connect (connect_lazy) / AthenaConnector::from_config (SDK client, lazy creds) — all offline-safe constructors"
  - phase: 85-shape-a-pure-config-binary-reference-parity
    provides: "Plan 85-03 scaffolded crates/pmcp-sql-server (lib/main split, feature-gated 4-connector manifest); Plan 85-01 superset config fields"
provides:
  - "crates/pmcp-sql-server/src/cli.rs — clap Args { config: PathBuf, schema: PathBuf, http: String=127.0.0.1:8080 }, re-exported at crate root as pmcp_sql_server::Args"
  - "crates/pmcp-sql-server/src/dispatch.rs — NOVEL dispatch(cfg) -> Arc<dyn SqlConnector> + DispatchError (FeatureMissing/UnknownBackend/MissingType/MissingField/SqliteOpen/Connector)"
  - "Compiled-out-backend errors (D-08) naming the missing feature with rebuild guidance"
  - "Credential-safe DispatchError Display (V7/T-85-04-01): path-free SqliteOpen, no url/file_path echo"
  - "Offline-safe Athena dispatch arm (T-85-04-04): explicit region, no IMDS/provider-chain probe at construction"
affects: [85-05, 85-06, 86-shapes-bcd]

# Tech tracking
tech-stack:
  added: [thiserror]
  patterns:
    - "Per-backend dispatch arm split into a feature-gated helper fn pair (dispatch_<backend> under #[cfg(feature)] + a #[cfg(not(feature))] FeatureMissing stub) — keeps the dispatch() match cog trivial and each helper cog <=25"
    - "Connector errors that may echo a path/value (SQLite open) are mapped to a path-free DispatchError variant at the dispatch boundary; URL backends rely on the connector's own sanitize_url/strip_aws_credentials redaction"
    - "Athena offline-safety = explicit region (AWS_REGION/AWS_DEFAULT_REGION, static fallback) so aws_config::load() never probes IMDS for a region; credentials stay lazy (resolved on first API call, which dispatch never makes)"

key-files:
  created:
    - crates/pmcp-sql-server/src/cli.rs
    - crates/pmcp-sql-server/src/dispatch.rs
    - crates/pmcp-sql-server/tests/dispatch.rs
    - crates/pmcp-sql-server/tests/dispatch.proptest-regressions
  modified:
    - crates/pmcp-sql-server/src/lib.rs
    - crates/pmcp-sql-server/Cargo.toml

decisions:
  - "DispatchError::SqliteOpen is a dedicated path-free variant (T-85-04-01): rusqlite's open error text echoes the file path verbatim ('unable to open database file: /secret/path'), so the SQLite arm map_err's it to SqliteOpen rather than forwarding the raw ConnectorError. URL backends (Postgres/MySQL/Athena) already redact at source so they keep the wrapping Connector(#[from]) variant."
  - "Athena region resolved from AWS_REGION/AWS_DEFAULT_REGION with a static 'us-east-1' fallback — DatabaseSection has no `region` field (the Athena reference config sources region only via ${AWS_REGION} inside output_location). An explicit region is the offline-safety lever: it stops aws_config::load() from probing IMDS for one."
  - "Athena from_config CONFIRMED offline-safe at construction (REVIEW FIX): the offline test dispatches an athena config with NO AWS creds env set and completes in 0.43s inside a 10s timeout — no hang, no network. aws-config 1.x resolves credentials lazily (LazyCredentialsCache) on first API call; dispatch never calls execute()/schema_text(), so construction touches neither IMDS nor the network given the explicit region."
  - "thiserror added as a direct dependency (Rule 3 blocking fix) — DispatchError derives thiserror::Error to match the connector crates' ConnectorError style; it was not previously a dependency of pmcp-sql-server."

requirements-completed: [SHAP-A-01]

# Metrics
duration: 6min
completed: 2026-05-27
tasks: 2
files: 6
---

# Phase 85 Plan 04: CLI Surface + Backend Dispatch Summary

**Built the two binary-side seams for the Shape A server: the clap `Args { config, schema, http }` CLI surface and the NOVEL `[database] type` → `Arc<dyn SqlConnector>` dispatch — with compiled-out-backend errors that name the missing feature (D-08), credential-safe error Display (V7), and a confirmed-offline Athena construction arm (REVIEW FIX, T-85-04-04).**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-05-27T01:14:52Z
- **Completed:** 2026-05-27T01:21:06Z
- **Tasks:** 2
- **Files created/modified:** 6 (4 created, 2 modified)

## Accomplishments

- **CLI surface (`cli.rs`):** `#[derive(clap::Parser)] Args` with required `--config`/`--schema` `PathBuf` args and `--http` `String` defaulting to `127.0.0.1:8080` (loopback so the out-of-the-box binary exposes no public listener). Re-exported at the crate root as `pmcp_sql_server::Args` so `main.rs` (Plan 05) and the test suite reach it via one path. 4 unit tests cover the default, the `--http` override, and the two missing-required usage errors.
- **NOVEL dispatch (`dispatch.rs`):** `pub async fn dispatch(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError>` matches on `cfg.database.backend_type` and constructs the matching connector behind its `#[cfg(feature)]` gate. The match body is trivial; each backend arm is a `dispatch_<backend>` helper (cog ≤ 25 each, PMAT-verified) paired with a `#[cfg(not(feature))]` stub returning `FeatureMissing`.
- **D-08 compiled-out errors:** under `--no-default-features --features sqlite`, an `athena` config returns `DispatchError::FeatureMissing("athena")` whose Display reads `config requires backend 'athena' but this binary was built without the 'athena' feature; rebuild with --features athena`. No silent fallback to a wrong backend.
- **V7 credential safety (T-85-04-01):** `DispatchError` Display names the backend/feature only. The proptest caught a REAL leak — `SqliteConnector::open` propagates `rusqlite`'s path-bearing error — and the fix maps any SQLite open failure to a path-free `DispatchError::SqliteOpen`. A property test feeds adversarial secret-bearing URLs and paths and asserts they never appear in the rendered error.
- **Offline-safe Athena (REVIEW FIX, T-85-04-04):** the Athena arm passes an explicit region so `aws_config::load()` never probes IMDS for one; credentials stay lazy. The offline test dispatches an Athena config with NO creds and completes in 0.43s under a 10s timeout guard — confirming construction is network-free.

## Task Commits

Each task was committed atomically (TDD: failing test written first, then implementation):

1. **Task 1: clap CLI surface (Args { config, schema, http })** — `7f65909a` (feat)
2. **Task 2: Backend dispatch (type → Arc<dyn SqlConnector>) + compiled-out error + offline Athena** — `4b63d1cd` (feat)

_TDD note: Task 1's 4 cli tests and Task 2's dispatch test were authored alongside the implementation; the RED state for Task 2's credential-leak property test was REAL and observable — the first `cargo test` run failed with `DispatchError leaked the file path token` (the SQLite path), driving the `SqliteOpen` redaction fix to GREEN. The proptest-regressions seed locks that exact shrunk input (`token = "0AAAaAa000AA"`)._

## Files Created/Modified

- `crates/pmcp-sql-server/src/cli.rs` — clap `Args` struct (`--config`/`--schema`/`--http`) + 4 unit tests + a crate-doc example.
- `crates/pmcp-sql-server/src/dispatch.rs` — `dispatch()` + `DispatchError` (6 variants) + 4 feature-gated `dispatch_<backend>` helper pairs + `resolve_athena_region()`.
- `crates/pmcp-sql-server/tests/dispatch.rs` — 8 tests: sqlite file-path, `:memory:`, missing-field, unknown-backend, missing-type, compiled-out-athena (cfg-gated off), athena offline-safety (cfg-gated on, timeout-guarded), and a `no_credential_leak` proptest module (postgres URL secret + sqlite path token).
- `crates/pmcp-sql-server/tests/dispatch.proptest-regressions` — regression seed locking the fixed SQLite path-leak case (the file's own header recommends checking it in; 8 such files are already tracked at the repo root).
- `crates/pmcp-sql-server/src/lib.rs` — appended `pub mod cli;` + `pub mod dispatch;` to the Plan 03 stub; re-exports `Args`, `dispatch`, `DispatchError`.
- `crates/pmcp-sql-server/Cargo.toml` — added `thiserror = "2"` (DispatchError derive).

## Decisions Made

- **`DispatchError::SqliteOpen` path-free variant (T-85-04-01):** the dispatch boundary is the right place to sanitize because `rusqlite`'s open error is path-bearing by design and the toolkit's `SqliteConnector` forwards it verbatim. URL backends keep the wrapping `Connector(#[from] ConnectorError)` variant since their connectors already redact (`sanitize_url` / `strip_aws_credentials`).
- **Athena region from env + static fallback:** `DatabaseSection` has no `region` field (the reference config only references `${AWS_REGION}` inside `output_location`). An explicit region — not a missing/None one — is the offline-safety lever, so the arm reads `AWS_REGION` → `AWS_DEFAULT_REGION` → `"us-east-1"`.
- **`from_config` offline-safety CONFIRMED, not just asserted:** PATTERNS claimed `from_config` "only builds an SDK client"; on inspection it calls `aws_config::defaults(...).region(...).load().await`. In aws-config 1.x that builds a `SdkConfig` with a lazy credentials cache and (because region is set explicitly) does no IMDS region probe — so construction is offline. The timeout-guarded no-creds test empirically proves it (0.43s, no hang).
- **`thiserror` added (Rule 3):** blocking — `DispatchError` could not derive `thiserror::Error` without it; it was absent from the crate's dependencies.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `thiserror` dependency to `pmcp-sql-server`**
- **Found during:** Task 2 (first compile failed: `cannot find module or crate thiserror`)
- **Issue:** `DispatchError` derives `thiserror::Error` (matching the connector crates) but `thiserror` was not a direct dependency of `pmcp-sql-server`.
- **Fix:** added `thiserror = "2"` to `[dependencies]` with a `# Why` comment.
- **Files modified:** `crates/pmcp-sql-server/Cargo.toml`
- **Commit:** `4b63d1cd`

**2. [Rule 2 - Missing critical functionality / security] Path-free `DispatchError::SqliteOpen`**
- **Found during:** Task 2 (the `no_credential_leak` proptest failed on the first run)
- **Issue:** `SqliteConnector::open` propagates `rusqlite`'s error text, which echoes the database file path (`unable to open database file: /nonexistent-.../db.sqlite`). Forwarding that as `DispatchError::Connector(_)` would violate V7/T-85-04-01 (no path/credential in dispatch errors).
- **Fix:** added the path-free `DispatchError::SqliteOpen` variant and `map_err(|_| DispatchError::SqliteOpen)` on both SQLite open paths. The operator finds the path in their own config / the binary's tracing logs, never in a client-facing error.
- **Files modified:** `crates/pmcp-sql-server/src/dispatch.rs`
- **Commit:** `4b63d1cd`

## Issues Encountered

- **`Arc<dyn SqlConnector>` is not `Debug`,** so the stdlib `Result::expect_err`/`unwrap_err` could not be used on dispatch results in tests. Added a small `expect_dispatch_err` helper that pattern-matches the `Err` arm, and switched the proptest to `if let Err(e)`.
- The credential-leak proptest's RED state (the SQLite path leak) was the most valuable signal in this plan — it is exactly the V7 threat the threat model flags, caught by the property test as designed.

## Deferred Issues

Out-of-scope pre-existing clippy lint surfaced under the local rust-1.95.0 toolchain (NOT introduced by this plan; same class already logged in STATE.md / `deferred-items.md`):

- `crates/pmcp-server-toolkit/src/code_mode.rs:460-461` — `field_reassign_with_default` in `build_cm_config` (Phase 83 code, untouched here). It is the only path `cargo clippy -p pmcp-sql-server --all-features -- -D warnings` flags. Per the SCOPE BOUNDARY rule it is NOT fixed (owned by Phase 83 code; local toolchain newer than CI's pinned stable).

My four touched source/test files (`cli.rs`, `dispatch.rs`, `tests/dispatch.rs`, the `lib.rs` additions) are fmt-clean and clippy-clean at `-D warnings`.

## User Setup Required

None — no external service configuration required. The Athena arm reads `AWS_REGION`/`AWS_DEFAULT_REGION` from the process env when present (production Athena deployments set them), but construction succeeds offline without them.

## Verification

```
cargo test -p pmcp-sql-server --no-default-features --features sqlite -- --test-threads=1
```
→ 21 passed (5 suites incl. doctests): 4 cli + 8 dispatch (sqlite-only, athena-feature-off compiled-out test active) + lib/main scaffold + 6 schema_fixture.

```
cargo test -p pmcp-sql-server --features athena,sqlite,postgres,mysql --test dispatch -- --test-threads=1
```
→ 8 passed including the athena-feature-ON offline-safety test (0.43s, no hang).

```
cargo clippy -p pmcp-sql-server --all-features --all-targets -- -D warnings
```
→ only the pre-existing `pmcp-server-toolkit/src/code_mode.rs` lint (out of scope); my files clean.

PMAT: `dispatch()` + all `dispatch_<backend>` helpers + `cli` cog ≤ 25 (0 violations on this plan's files).

## Threat Surface

No new threat surface beyond the plan's `<threat_model>`. All four registered threats are realized as planned:
- **T-85-04-01 (info disclosure):** mitigated — `DispatchError` Display names backend/feature only; the SQLite path leak the proptest caught is closed via `SqliteOpen`.
- **T-85-04-02 (DoS, malformed args):** accepted — clap derive fails missing `--config`/`--schema` with a usage error (2 tests).
- **T-85-04-03 (tampering, unknown/compiled-out backend):** mitigated — `UnknownBackend`/`FeatureMissing` are explicit and actionable; no silent fallback.
- **T-85-04-04 (DoS, Athena provider-chain):** mitigated — explicit region + lazy creds + no execute/schema_text at dispatch; confirmed offline by the timeout-guarded no-creds test.

## Next Phase Readiness

- **Plan 85-05 (Wave 2 assembly)** consumes both seams: `Args` from `cli.rs` feeds the `main.rs` shim, and `dispatch()`'s `Arc<dyn SqlConnector>` feeds the toolkit builder chain (`try_tools_from_config_with_connector` + the LOCKED Plan 02 `try_code_mode_from_config_with_connector`). The Athena arm's offline-safety is the precondition for SC-1's no-creds `tools/list` test (Plan 05 adds the startup timeout guard).
- No blockers. The deferred toolkit clippy lint persists (CI toolchain mismatch) but does not affect this crate's files.

---
*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Completed: 2026-05-27*

## Self-Check: PASSED

All 4 created files (`cli.rs`, `dispatch.rs`, `tests/dispatch.rs`, `tests/dispatch.proptest-regressions`) exist on disk; both task commits (`7f65909a`, `4b63d1cd`) are present in git history.
