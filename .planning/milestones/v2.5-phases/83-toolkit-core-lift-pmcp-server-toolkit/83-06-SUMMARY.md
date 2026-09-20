---
phase: 83-toolkit-core-lift-pmcp-server-toolkit
plan: 06
subsystem: toolkit
tags:
  - toolkit
  - code-mode
  - hmac
  - policy
  - re-exports
  - r1-preflight
  - r6-secret-type
  - r9-inline-secret-rejection

# Dependency graph
requires:
  - phase: 83-toolkit-core-lift-pmcp-server-toolkit
    provides: "ServerConfig + CodeModeSection + CodeModeLimits (Plan 04, R4 dep); SecretValue + From<SecretValue> for pmcp_code_mode::TokenSecret (Plan 02, R6 dep); ConfigValidationError enum (Plan 04, extended here with InlineSecretRejected per R9)"
  - phase: 67-1-code-mode-hardening
    provides: "pmcp-code-mode crate — CodeExecutor trait, ValidationPipeline, HmacTokenGenerator, TokenSecret, NoopPolicyEvaluator (Plan 06 re-exports verbatim per D-16; NO duplicate impl)"
provides:
  - "pmcp_server_toolkit::code_mode module (#[cfg(feature = \"code-mode\")])"
  - "pub use pmcp_code_mode::{CodeExecutor, ValidationPipeline, ValidationContext, TokenSecret, HmacTokenGenerator, TokenGenerator, ApprovalToken, NoopPolicyEvaluator, PolicyEvaluator, AuthorizationDecision, CodeModeConfig, canonicalize_code, compute_context_hash, hash_code}"
  - "pub use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator} under #[cfg(feature = \"avp\")]"
  - "pub fn validation_pipeline_from_config(&ServerConfig) -> Result<ValidationPipeline>"
  - "pub fn code_mode_tools_from_executor(Box<dyn CodeExecutor>, &ServerConfig) -> Result<Box<dyn CodeExecutor>>"
  - "pub fn register_code_mode_tools(pmcp::ServerBuilder, &ServerConfig) -> Result<pmcp::ServerBuilder> — tolerant of config.code_mode = None"
  - "CodeModeSection.allow_inline_token_secret_for_dev: bool (R9 dev escape hatch; default false)"
  - "ConfigValidationError::InlineSecretRejected variant (R9 rejection path)"
  - "Toolkit Cargo.toml: `code-mode` feature now forwards `pmcp-code-mode/sql-code-mode` so SC-3 anchor is available under --features code-mode"
affects:
  - "83-07 (skills server can rely on code_mode re-exports for the authoring vocabulary)"
  - "83-08 (ServerBuilderExt::code_mode_from_config wraps register_code_mode_tools)"
  - "84 (SQL connectors inject CodeExecutor backends via code_mode_tools_from_executor)"
  - "85 (Shape A pmcp-sql-server resolves [code_mode] config without per-server Rust glue)"
  - "87 (pmcp-config-helper surfaces the [code_mode] TOML vocabulary including allow_inline_token_secret_for_dev semantics)"

# Tech tracking
tech-stack:
  added:
    - "pmcp-code-mode/sql-code-mode forwarding from the toolkit's code-mode feature (per CODE_MODE_API_NOTES.md Section 5 — required for ValidationPipeline::validate_sql_query)"
  patterns:
    - "Preflight artifact (CODE_MODE_API_NOTES.md, 256 lines) documenting every pmcp-code-mode public signature Plan 06 depends on, BEFORE writing wiring code (review R1 enforcement)"
    - "R1 split — validation_pipeline_from_config + code_mode_tools_from_executor — chosen because pmcp-code-mode's CodeExecutor trait requires backend injection; no config-only constructor exists"
    - "Default-deny inline secret rejection (R9) — InlineSecretRejected error variant + allow_inline_token_secret_for_dev escape hatch with tracing::warn"
    - "Toolkit-owned SecretValue → pmcp_code_mode::TokenSecret conversion at the HMAC boundary (R6 — preserves --no-default-features stability)"
    - "Explicit field-by-field mapping in build_cm_config (PATTERNS §10 boundary-explicit) — no silent serde aliasing between the toolkit's unprefixed field names (allow_writes) and pmcp_code_mode's sql_-prefixed CodeModeConfig fields"
    - "Documented gap-notes for fields without a pmcp_code_mode counterpart (require_limit, [code_mode.limits].max_tables_per_query / max_join_depth / max_subquery_depth) — not silently dropped"

key-files:
  created:
    - ".planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/CODE_MODE_API_NOTES.md — Task 0 preflight artifact (256 lines, 8 sections)"
    - "crates/pmcp-server-toolkit/tests/code_mode_wiring.rs — 6 integration tests covering SC-3 anchor + R9 enforcement + tolerant no-op behaviour"
  modified:
    - "crates/pmcp-server-toolkit/src/code_mode.rs — full module replacing Plan 01 stub (470+ lines): re-exports + validation_pipeline_from_config + code_mode_tools_from_executor + register_code_mode_tools + build_cm_config + resolve_token_secret + map_auto_approve_levels + 6 unit tests"
    - "crates/pmcp-server-toolkit/src/config.rs — CodeModeSection gains allow_inline_token_secret_for_dev field"
    - "crates/pmcp-server-toolkit/src/error.rs — ConfigValidationError gains InlineSecretRejected variant"
    - "crates/pmcp-server-toolkit/src/lib.rs — _CODE_MODE_REEXPORT_SMOKE compile-only assertion gated on feature = code-mode"
    - "crates/pmcp-server-toolkit/Cargo.toml — code-mode feature forwards pmcp-code-mode/sql-code-mode"

key-decisions:
  - "R1 split selected (validation_pipeline_from_config + code_mode_tools_from_executor) — NOT the single-function executor_from_config path. CODE_MODE_API_NOTES.md Section 6 records the rationale: pmcp-code-mode's CodeExecutor concrete impls (JsCodeExecutor, SdkCodeExecutor, McpCodeExecutor) all require backend injection (HttpExecutor / SdkExecutor / McpExecutor); a config-only executor constructor does not exist and creating one in the toolkit would invert the dependency graph by forcing the toolkit to depend on every Phase 84 connector crate."
  - "Toolkit's `code-mode` feature now forwards `pmcp-code-mode/sql-code-mode` — required for ValidationPipeline::validate_sql_query (the SC-3 anchor). Documented in CODE_MODE_API_NOTES.md Section 5. The transitive cost is one extra dep (sqlparser 0.62) which is already pulled in by Phase 84 connectors anyway."
  - "register_code_mode_tools is intentionally tolerant of config.code_mode = None (returns the builder unchanged). This lets Plan 08's `code_mode_from_config(&cfg)` builder extension be invoked unconditionally without an `if cfg.code_mode.is_some()` ceremony at every Shape A/C call site. The R9 enforcement gate still fires when [code_mode] IS present."
  - "build_cm_config does explicit field-by-field translation, NOT serde aliasing. The toolkit's unprefixed CodeModeSection field names (allow_writes, allow_deletes, ...) are deliberately stable across pmcp-code-mode versions; coupling them via serde alias would make every pmcp-code-mode field rename a breaking change for toolkit consumers. PATTERNS §10 + D-13."
  - "Fields documented as gaps (not silently dropped): CodeModeSection.require_limit (pmcp-code-mode has no direct counterpart — closest semantic is sql_max_rows); CodeModeLimits.max_tables_per_query / max_join_depth / max_subquery_depth (forward-compat for Phase 84 SQL connector enforcement; not silently mapped). Threat T-83-06-04 mitigation."
  - "auto_approve_levels parsing: unrecognised strings emit a tracing::debug log and are SKIPPED (not erroring). Rationale: the level vocabulary is open-ended; an operator typo should surface as 'nothing auto-approved' rather than a hard parse failure that blocks the server from starting. Hard-parse-error would also force Plan 04's config parser to know about pmcp-code-mode's RiskLevel enum, undesirable coupling."
  - "5 unit tests + 6 integration tests + 1 doctest — TEST-02 / TEST-03 coverage. Total Plan 06 tests: 12 new (6 unit, 6 integration). Toolkit total: 80 tests under --features code-mode."

patterns-established:
  - "Preflight artifact pattern: when wiring across crate boundaries to a feature-gated API surface, ship a CODE_MODE_API_NOTES.md-style document with verbatim signatures BEFORE writing the wiring. Eliminates execution-time discovery + hallucinated constructors (R1)."
  - "R1 split pattern: when the dependency exposes a trait but not a concrete config-only constructor, ship `<thing>_from_config` for the config-driven portion + `<thing>_tools_from_executor` for the dependency-injection portion. The toolkit gives Shape A/C consumers the config-driven half; Plan 08 hands the executor over."
  - "R9 default-deny pattern: a config field that can carry a secret MUST reject inline literals by default with a typed error (InlineSecretRejected). Provide an explicit dev-only opt-in flag (allow_inline_token_secret_for_dev) with a loud tracing::warn so the operator's choice is auditable."
  - "Gap-note pattern: every CodeModeSection field without a pmcp_code_mode::CodeModeConfig counterpart is acknowledged in build_cm_config via a `_gap_*` local + inline comment (NOT silently dropped). Reviewer can grep `_gap_` to see every untranslated field at a glance."

requirements-completed:
  - TKIT-06
  - TKIT-09

# Metrics
duration: 35min
completed: 2026-05-18
---

# Phase 83 Plan 06: code-mode wiring + HMAC re-export (TKIT-06 + TKIT-09) Summary

**`pmcp_server_toolkit::code_mode` re-exports every HMAC / policy / executor / validation surface from `pmcp-code-mode` (D-16, no duplicate impl) and ships `validation_pipeline_from_config(&ServerConfig) -> Result<ValidationPipeline>` plus `code_mode_tools_from_executor` (R1 split — backend injection required) with default-deny inline-secret rejection (R9) and toolkit-owned `SecretValue` → `TokenSecret` conversion at the HMAC boundary (R6).**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-05-18T21:30Z (approximate — STATE.md last_updated from Plan 05 close)
- **Completed:** 2026-05-18T22:05Z (approximate)
- **Tasks:** 4 (Task 0 preflight + Task 1 module + Task 2 integration tests + Task 3 quality gate)
- **Files created:** 2 (`CODE_MODE_API_NOTES.md`, `tests/code_mode_wiring.rs`)
- **Files modified:** 5 (`src/code_mode.rs`, `src/config.rs`, `src/error.rs`, `src/lib.rs`, `Cargo.toml`)

## Wiring strategy selected (per CODE_MODE_API_NOTES.md Section 6)

**R1 split** — `validation_pipeline_from_config` + `code_mode_tools_from_executor`. NOT the single-function `executor_from_config` path.

**Rationale from CODE_MODE_API_NOTES.md Section 6:**

> Section 1's `CodeExecutor` trait requires backend injection; the toolkit
> cannot manufacture a backend from config alone without dragging in Phase
> 84's SQL connectors (which would invert the dependency graph and force a
> circular dep through the deploy story). The R1 split keeps Plan 06's
> deliverable focused on the validation pipeline (config-driven, no backend
> needed) while leaving the executor-tying surface for callers that already
> own a backend.

Concretely:

- `validation_pipeline_from_config(&ServerConfig)` builds a fully-configured
  `ValidationPipeline` (HMAC keyed by the resolved `TokenSecret`, all policy
  + limit fields mapped through `build_cm_config`).
- `code_mode_tools_from_executor(executor, &ServerConfig)` accepts a
  caller-supplied `Box<dyn CodeExecutor>` and surfaces R9 enforcement errors
  before handing the executor back to the caller. Plan 08's
  `ServerBuilderExt::code_mode_from_config` is where the actual tool
  registration on `pmcp::ServerBuilder` lands.
- `register_code_mode_tools(builder, &ServerConfig)` is the tolerant builder
  entry point — no-op when `[code_mode]` is absent, R9 enforcement gate when
  present.

## R1 verification — no `todo!()` survives

Per Plan 06 review R1, the verify step grep-rejects any surviving `todo!()`
or `unimplemented!()` calls:

```bash
$ grep -nE "todo!\(|unimplemented!\(" \
    crates/pmcp-server-toolkit/src/code_mode.rs \
    crates/pmcp-server-toolkit/tests/code_mode_wiring.rs
# (empty output — clean)
$ echo $?
1
```

The Task 0 preflight (`CODE_MODE_API_NOTES.md`) supplied the exact verbatim
signatures so Task 1's body uses real constructor calls
(`ValidationPipeline::from_token_secret(cm_config, &token_secret)`) rather
than `todo!()` placeholders. R1 satisfied.

## CodeModeSection field gaps (per Plan 06 output spec)

The `<output>` block specifies: "Whether any field on `CodeModeSection` had
no `CodeModeConfig` counterpart (documented dropped field — gap note, not
silent drop)."

| `CodeModeSection` field | `CodeModeConfig` counterpart | Action |
|---|---|---|
| `enabled` | `enabled` | mapped |
| `server_id` | `server_id` | mapped |
| `allow_writes` | `sql_allow_writes` | mapped (unprefixed → prefixed) |
| `allow_deletes` | `sql_allow_deletes` | mapped |
| `allow_ddl` | `sql_allow_ddl` | mapped |
| `blocked_tables` | `sql_blocked_tables` | mapped (Vec → HashSet) |
| `sensitive_columns` | `sql_blocked_columns` | mapped (Vec → HashSet) — semantic merge: toolkit's sensitive_columns becomes pmcp-code-mode's blocked_columns since the latter is the only `column`-level enforcement surface available |
| `max_limit` | `sql_max_rows` | mapped |
| `token_ttl_seconds` | `token_ttl_seconds` | mapped (Option<u64> → saturating-to-i64) |
| `auto_approve_levels` | `auto_approve_levels` | mapped (Vec<String> → Vec<RiskLevel> via best-effort parse) |
| `token_secret` | (consumed via HMAC, not stored in CodeModeConfig) | mapped (separate codepath: `resolve_token_secret` → `SecretValue` → `TokenSecret` → `ValidationPipeline::from_token_secret`) |
| `allow_inline_token_secret_for_dev` | (toolkit-only — R9 enforcement flag) | not mapped (toolkit-internal) |
| `require_limit` | (no direct counterpart) | **GAP DOCUMENTED.** The closest pmcp-code-mode field is `sql_max_rows`, which is a hard cap rather than a "must declare LIMIT" requirement. Plan 06 acknowledges the gap in an inline `_require_limit_gap` local. Phase 84's SQL connector enforcement may add an explicit LIMIT-requiring check; not silently mapped here. |
| `limits.max_tables_per_query` | (no direct counterpart) | **GAP DOCUMENTED** (`_gap_max_tables`). pmcp-code-mode's `max_field_count` is GraphQL-flavoured and not semantically equivalent. Forward-compat for Phase 84. |
| `limits.max_join_depth` | (no direct counterpart) | **GAP DOCUMENTED** (`_gap_max_join`). pmcp-code-mode's `sql_max_joins` would be the natural mapping but is not exposed via the CodeModeConfig fields visible to the toolkit at this layer; revisit if/when pmcp-code-mode exposes it more directly. |
| `limits.max_subquery_depth` | (no direct counterpart) | **GAP DOCUMENTED** (`_gap_max_subquery`). pmcp-code-mode has `max_depth` (GraphQL field depth) but not a subquery-specific equivalent. Forward-compat for Phase 84. |

Threat T-83-06-04 (silent field-drop) mitigated by inline `_gap_*` locals
plus this table — reviewers can grep `_gap_` in `code_mode.rs` to audit
every untranslated field.

## R6 verification — feature-independence

```bash
$ cargo build -p pmcp-server-toolkit --no-default-features
   Compiling pmcp-server-toolkit v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.93s
```

`SecretValue` lives in `crate::secrets` (Plan 02) and does NOT depend on
`code-mode`. The `From<SecretValue> for pmcp_code_mode::TokenSecret` impl is
itself `#[cfg(feature = "code-mode")]`-gated so the conversion exists only
when the consumer actually opts in. `--no-default-features` compiles clean
and all 67 non-code-mode tests pass.

## R9 verification — inline-secret rejection

Three layered enforcement points cover the R9 invariant:

1. **`resolve_token_secret` (unit-tested):**
   - `token_secret = "env:VAR_NAME"` resolves via env (happy path).
   - `token_secret = "raw-string"` + `allow_inline_token_secret_for_dev =
     false` (default) → `Err(ToolkitError::Validation(InlineSecretRejected))`.
   - `token_secret = "raw-string"` + `allow_inline_token_secret_for_dev =
     true` → `Ok(SecretValue)` with a `tracing::warn!` audit log.

2. **`validation_pipeline_from_config` (integration-tested):**
   - Drives `resolve_token_secret`, so any R9 rejection bubbles up as
     `ToolkitError::Validation(InlineSecretRejected)` before the HMAC
     `TokenSecret` is ever materialised.

3. **`register_code_mode_tools` (integration-tested):**
   - The builder-extension entry point — used by Plan 08's
     `code_mode_from_config`. Tolerant of `config.code_mode = None`
     (no-op); when present, runs `validation_pipeline_from_config` to
     surface R9 errors at builder-time, NOT at first request.

Integration test
`inline_token_secret_without_dev_flag_rejected_by_register` exercises path
(3) end-to-end: a `ServerConfig` parsed from a TOML with `token_secret =
"raw-string-that-should-be-rejected"` is fed into
`register_code_mode_tools(pmcp::Server::builder().name("t").version("0.1.0"),
&cfg)` and the result MUST be
`Err(ToolkitError::Validation(ConfigValidationError::InlineSecretRejected))`.
The companion test
`inline_token_secret_with_dev_flag_passes_register` confirms the dev-flag
escape hatch works as designed.

## SC-3 anchor — `allow_writes = false` rejects INSERT

```rust
// tests/code_mode_wiring.rs::allow_writes_false_rejects_insert
let cfg = ServerConfig::from_toml_strict_validated(CONFIG_WRITES_DISALLOWED)
    .expect("config parses + validates");
let pipeline = validation_pipeline_from_config(&cfg).expect("pipeline builds");
let ctx = ValidationContext::new("test-user", "test-session", "schema-hash", "perms-hash");
let result = pipeline
    .validate_sql_query("INSERT INTO foo VALUES (1, 2, 3);", &ctx)
    .expect("validation runs (returns failure, not Err)");
assert!(!result.is_valid);
assert!(result.violations.iter().any(|v| v.rule == "writes_disabled"));
```

This is the ROADMAP SC-3 anchor — a Shape A consumer writing a
`config.toml` with `[code_mode] allow_writes = false` gets the runtime
enforcement WITHOUT writing any Rust glue. Sibling test
`allow_writes_true_permits_insert` confirms the inverse, and
`select_is_always_permitted_under_default_config` proves the pipeline is
not blanket-rejecting.

## Quality gates

| Check | Result |
|---|---|
| `cargo build -p pmcp-server-toolkit --no-default-features` (R6) | ok |
| `cargo build -p pmcp-server-toolkit --features code-mode` | ok |
| `cargo build -p pmcp-server-toolkit --features code-mode,avp` | ok |
| `cargo build -p pmcp-server-toolkit --all-features` | ok |
| `cargo test code_mode::tests --features code-mode` | 6 passed |
| `cargo test --test code_mode_wiring --features code-mode` | 6 passed |
| `cargo test --doc code_mode --features code-mode` | 1 passed |
| `cargo test -p pmcp-server-toolkit --features code-mode` | 80 passed |
| `cargo test -p pmcp-server-toolkit --no-default-features` | 67 passed |
| `make quality-gate` | exit 0 (fmt + clippy + build + test + audit all green) |
| No surviving `todo!()` / `unimplemented!()` | grep-clean |

## Commits

| Task | Subject | Commit |
|---|---|---|
| 0 | docs(83-06): preflight pmcp-code-mode API surface (R1 Task 0) | `183bd0ba` |
| 1 | feat(83-06): re-export pmcp-code-mode + validation_pipeline_from_config (R1 split) | `76c6e17a` |
| 2 | test(83-06): integration tests for [code_mode] policy enforcement (SC-3 anchor) | `798a5d06` |
| 3 | chore(83-06): cargo fmt + code_mode reexport smoke const | `d68ac794` |

## Review compliance summary

- **R1 (preflight artifact):** `CODE_MODE_API_NOTES.md` exists (256 lines, 8 sections). No `todo!()` survives in `code_mode.rs` or `tests/code_mode_wiring.rs`.
- **R3 (crate-root re-exports):** the `code_mode::*` path resolves under `pmcp_server_toolkit::code_mode::{CodeExecutor, ValidationPipeline, validation_pipeline_from_config, ...}` per D-15/D-16. Proven by `_CODE_MODE_REEXPORT_SMOKE` compile-only const.
- **R4 (depends_on):** plan frontmatter declared `[02, 04]`; the wiring consumes `SecretValue` (02) + `ServerConfig`/`CodeModeSection`/`ConfigValidationError` (04).
- **R6 (toolkit-owned secret type):** `--no-default-features` build succeeds; `From<SecretValue> for pmcp_code_mode::TokenSecret` is feature-gated; `SecretValue` itself is feature-independent (defined in Plan 02).
- **R9 (inline-secret rejection):** `allow_inline_token_secret_for_dev: bool` field + `InlineSecretRejected` error variant + 2 unit tests + 2 integration tests covering reject + dev-flag-accept paths.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical feature] Toolkit `code-mode` feature must forward `pmcp-code-mode/sql-code-mode`**

- **Found during:** Task 0 preflight (CODE_MODE_API_NOTES.md Section 5).
- **Issue:** The plan's verify step uses `cargo test --features code-mode`, but `ValidationPipeline::validate_sql_query` (the SC-3 anchor) is gated behind `#[cfg(feature = "sql-code-mode")]` in pmcp-code-mode. Without forwarding, the integration test does not compile.
- **Fix:** `crates/pmcp-server-toolkit/Cargo.toml` — `code-mode = ["dep:pmcp-code-mode", "pmcp-code-mode/sql-code-mode"]`. Documented in CODE_MODE_API_NOTES.md Section 8 + the Cargo.toml comment block.
- **Commit:** `76c6e17a`

**2. [Rule 1 - Bug] `SecretValue` has no `Debug` impl — `expect_err` on `Result<SecretValue, _>` doesn't compile**

- **Found during:** Task 1 first test run.
- **Issue:** The plan's example test sketch used `.expect_err(...)` on `Result<SecretValue, ToolkitError>`. `SecretValue` intentionally does NOT implement `Debug` (R5 invariant from Plan 02), so `expect_err` cannot format the `Ok` payload.
- **Fix:** Rewrote the affected tests to use a `match` arm pattern instead of `expect_err`. The test logic is identical; only the panic-on-unexpected-Ok wording moved into the explicit panic arm.
- **Commit:** `76c6e17a`

**3. [Rule 1 - Bug] `ValidationResult` field is `is_valid`, not `valid`**

- **Found during:** Task 2 first test run.
- **Issue:** The plan's example assertions used `result.valid`. The actual `pmcp_code_mode::ValidationResult` field is `is_valid` (`crates/pmcp-code-mode/src/types.rs:161`).
- **Fix:** Renamed all `result.valid` references in `tests/code_mode_wiring.rs` to `result.is_valid`.
- **Commit:** `798a5d06`

These are all minor sketch-vs-reality deltas — the plan's pseudocode said
"shape" but did not bind to the verbatim field names. The preflight artifact
(R1) caught the bigger structural issues (R1 split, sql-code-mode
forwarding) before any wiring was written.

### Out-of-Scope Discoveries

None. The `cargo-fuzz` builds-against-nightly errors visible in the
`make quality-gate` log are pre-existing (the host toolchain is stable, and
fuzz targets are documented as nightly-only — Makefile treats them as
advisory). Not a Plan 06 regression.

## Self-Check: PASSED
