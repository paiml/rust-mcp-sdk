---
phase: 90-openapi-built-in-server
plan: 02
subsystem: config
tags: [openapi, http, config, toml, backend, tool-detection, deny-unknown-fields, fuzz]

# Dependency graph
requires:
  - phase: 90-openapi-built-in-server
    plan: 01
    provides: "http::auth::AuthConfig (six modes) + http::client::HttpConfig — re-exported by [backend.auth] / [backend.http]"
  - phase: 83-toolkit-core-lift
    provides: "backend-agnostic ServerConfig + ToolDecl + deny_unknown_fields discipline (D-13)"
provides:
  - "ServerConfig.backend: Option<BackendSection> (http feature) — [backend]/[backend.auth]/[backend.http] additive section"
  - "BackendSection { base_url, auth: AuthConfig, http: HttpConfig } re-exporting Plan 90-01 http types (H3)"
  - "ToolDecl path/method/base_url/script additive fields (D-01 two-kind detection)"
  - "ToolDecl::is_script_tool() detection rule (single source of truth for Plan 03/05 synthesizers)"
  - "ConfigValidationError::AmbiguousToolKind + validate()-time mutual-exclusivity check (T-90-02-04)"
  - "config-parser fuzz corpus seed-backend.toml"
affects: [90-03-synthesizer, 90-04-code-mode-executor, 90-05-script-tools, 90-06-binary-dispatch]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Feature-gated config section (#[cfg(feature=\"http\")] on both the struct AND the ServerConfig field) — no dead stub type in a no-http build"
    - "Detection rule lives in exactly one inherent method (ToolDecl::is_script_tool) so all synthesizers branch consistently"
    - "Ambiguity is a validate()-time typed error, never a silent precedence rule"

key-files:
  created:
    - crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-backend.toml
  modified:
    - crates/pmcp-server-toolkit/src/config.rs
    - crates/pmcp-server-toolkit/src/http/auth.rs
    - crates/pmcp-server-toolkit/src/http/client.rs
    - crates/pmcp-server-toolkit/src/error.rs
    - crates/pmcp-server-toolkit/tests/reference_configs.rs

key-decisions:
  - "Added PartialEq/Eq to AuthConfig (auth.rs) and PartialEq/Eq + deny_unknown_fields to HttpConfig (client.rs) — required because ServerConfig derives PartialEq (so its new backend field must be PartialEq) and because T-90-02-01 mitigation needs deny_unknown_fields to reject an unknown [backend.http] key (Rule 2)"
  - "BackendSection + ServerConfig.backend BOTH gated #[cfg(feature=\"http\")] — a no-http build has no OpenAPI backend, so a stub would be misleading (per plan read_first H3)"
  - "AuthConfig/HttpConfig re-exported from config (pub use crate::http::...) rather than redefined, per H3 ownership in Plan 90-01"
  - "Ambiguity check uses a private declared_kind_count() helper (cog well under 25); script+path/method and sql+script both rejected as AmbiguousToolKind"

patterns-established:
  - "Pattern: an http-feature-conditional fuzz seed is exempted in the reference_configs seed-parse smoke test via an exact-name match (name == \"seed-backend.toml\") + #[cfg(feature=\"http\")] parse-or-no-panic split"

requirements-completed: [OAPI-02a, OAPI-03]

# Metrics
duration: 6min
completed: 2026-05-29
---

# Phase 90 Plan 02: Backend-Agnostic Config Types Summary

**Extended the shared Phase 83 `ServerConfig` additively with a feature-gated `[backend]` / `[backend.auth]` / `[backend.http]` section (re-exporting Plan 90-01's `AuthConfig`/`HttpConfig`) and added the D-01 two-kind `ToolDecl` fields (`path`/`method`/`base_url`/`script`) with a single-source `is_script_tool()` detection rule and validate()-time ambiguity rejection — all under preserved `deny_unknown_fields`, with SQL configs parsing unchanged.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-05-29T18:01:27Z
- **Completed:** 2026-05-29T18:07Z
- **Tasks:** 2
- **Files modified:** 6 (1 created, 5 modified)

## Accomplishments

- `ServerConfig.backend: Option<BackendSection>` (http feature) — a full `[backend]` + `[backend.auth] type="api_key"` + `[backend.http]` TOML round-trips into `backend.is_some()` (OAPI-02a / D-06).
- `BackendSection { base_url, auth: AuthConfig, http: HttpConfig }` re-exports the Plan 90-01 http types (H3 ownership) instead of redefining them; both the struct and the `ServerConfig` field are `#[cfg(feature = "http")]` so a no-http build carries no dead stub.
- `ToolDecl` gained additive `path` / `method` / `base_url` / `script` `Option<String>` fields on the SHARED tool decl (not http-gated — `None` for SQL configs), plus `is_script_tool()` (the single detection rule Plan 03/05 branch on) — D-01.
- Mutual-exclusivity validation (Codex MEDIUM): `validate()` rejects an entry that mixes `sql` / `path`+`method` / `script` with the new `ConfigValidationError::AmbiguousToolKind(index)` — ambiguity is surfaced, never resolved by a silent "script wins" (T-90-02-04).
- `deny_unknown_fields` preserved everywhere: an unknown key under `[backend.http]` is a hard parse error (T-90-02-01); this required adding `deny_unknown_fields` to the lifted `HttpConfig`.
- Fuzz corpus seed `seed-backend.toml` added (a `[backend]` + single-call `[[tools]]` block); the reference_configs seed-parse smoke test treats it as http-feature-conditional.
- 44 config tests green under `--features http`; full default-features suite green (121 lib + integration) — additive proof.

## Task Commits

1. **Task 1: Additive [backend]/[backend.auth]/[backend.http] on ServerConfig (D-06)** - `015f5ab0` (feat)
2. **Task 2: ToolDecl path/method/base_url/script fields (D-01 two-kind detection)** - `a6658cda` (feat)

_TDD note: this plan's frontmatter is `type: execute`; each task carried `tdd="true"`. Tests and implementation were committed together per task because every assertion targets net-new fields/sections on existing types (no prior passing behaviour to protect)._

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/config.rs` - `ServerConfig.backend` field + `BackendSection` struct + `AuthConfig`/`HttpConfig` re-exports; `ToolDecl` path/method/base_url/script fields + `is_script_tool()`/`declared_kind_count()`; `validate()` ambiguity check; 4 backend tests + 6 tooldecl tests + 1 backend-doctest.
- `crates/pmcp-server-toolkit/src/http/auth.rs` - added `PartialEq, Eq` to `AuthConfig`.
- `crates/pmcp-server-toolkit/src/http/client.rs` - added `PartialEq, Eq` + `#[serde(deny_unknown_fields)]` to `HttpConfig`.
- `crates/pmcp-server-toolkit/src/error.rs` - `ConfigValidationError::AmbiguousToolKind(usize)`.
- `crates/pmcp-server-toolkit/tests/reference_configs.rs` - http-feature-conditional handling of `seed-backend.toml` in the fuzz-seed smoke test.
- `crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-backend.toml` - new named fuzz seed (`[backend]` + single-call tool).

## Decisions Made

- **PartialEq/Eq added to http types** so `ServerConfig`'s `PartialEq` derive holds with the new `backend` field (the field transitively contains `AuthConfig`/`HttpConfig`).
- **deny_unknown_fields added to HttpConfig** to satisfy the T-90-02-01 mitigation — without it an unknown `[backend.http]` key was silently ignored (caught by the negative test). This is the threat-register correctness requirement, applied as Rule 2.
- **Feature-gating both struct and field** keeps a no-http build free of an unusable OpenAPI backend type.
- **Ambiguity rejected, not precedence-resolved** — `declared_kind_count() > 1` ⇒ `AmbiguousToolKind`, covering both script+path/method and sql+script.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] `HttpConfig` lacked `deny_unknown_fields`**
- **Found during:** Task 1 (`test_backend_unknown_field_rejected` failed)
- **Issue:** The plan and threat register (T-90-02-01) require an unknown key under `[backend.http]` to be rejected. The lifted `HttpConfig` (Plan 90-01) did not carry `#[serde(deny_unknown_fields)]`, so `foo = 1` was silently ignored — the negative test caught it.
- **Fix:** Added `#[serde(deny_unknown_fields)]` to `HttpConfig` (and `PartialEq, Eq` to both `HttpConfig` and `AuthConfig`, needed for the `ServerConfig` `PartialEq` derive).
- **Files modified:** crates/pmcp-server-toolkit/src/http/client.rs, crates/pmcp-server-toolkit/src/http/auth.rs
- **Verification:** `test_backend_unknown_field_rejected` now passes; full `--features http` suite green.
- **Committed in:** `015f5ab0` (Task 1 commit)

**2. [Rule 3 - Blocking] `seed-backend.toml` broke the default-features fuzz-seed smoke test**
- **Found during:** Task 2 (full default-features suite run)
- **Issue:** `tests/reference_configs.rs::fuzz_corpus_seeds_parse_or_explicitly_fail` runs under default features (no `http`) and requires every non-adversarial `seed-*.toml` to parse via `from_toml`. The new `seed-backend.toml` uses `[backend]`, which is `#[cfg(feature = "http")]` — under default features it is an unknown section and correctly fails to parse, tripping the test.
- **Fix:** Treated `seed-backend.toml` (exact-name match, NOT the broad `contains("backend")` which would wrongly catch `seed-postgres-backend.toml` etc.) as http-feature-conditional: it MUST parse under `--features http` and need only not-panic under default features.
- **Files modified:** crates/pmcp-server-toolkit/tests/reference_configs.rs
- **Verification:** default-features suite green; `--features http --test reference_configs` 7/7 green (backend seed asserted to parse).
- **Committed in:** `a6658cda` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 missing-critical/Rule 2, 1 blocking/Rule 3)
**Impact on plan:** Both were necessary for the plan's own acceptance criteria (deny_unknown_fields negative test; additive-proof default suite). No scope creep — public surface matches the plan's `artifacts` and `must_haves`.

## Issues Encountered

- The first `contains("backend")` exemption draft was too broad (it would have matched the three SQL `seed-*-backend.toml` seeds, which legitimately parse under default features). Narrowed to an exact filename match.

## TDD Gate Compliance

This plan's frontmatter is `type: execute` (not `type: tdd`); each task carried `tdd="true"`. Tests and implementation were committed together per task rather than as separate RED/GREEN commits, because every assertion targets net-new fields/sections on existing types (no prior passing behaviour to protect). All behaviours listed in each task's `<behavior>` block have passing tests.

## Known Stubs

None — the `[backend]` section and `ToolDecl` two-kind fields are fully parsed and (for the detection rule) wired. Consumption of these fields by the synthesizer and code-mode executor is Plan 03/04/05 scope, as designed; this plan is the config-type layer.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 03** (synthesizer) can read `ServerConfig.backend` + `ToolDecl.path`/`method`/`base_url` for single-call tool synthesis and branch on `ToolDecl::is_script_tool()`.
- **Plan 04/05** (code-mode executor / script tools) read `ToolDecl.script` (detected via `is_script_tool()`) and bind `[[tools.parameters]]` to `args`.
- `AuthConfig`/`HttpConfig` reachable via `pmcp_server_toolkit::config::{AuthConfig, HttpConfig}` (re-export) or the `http` module path.
- No blockers.

## Self-Check: PASSED

- Created file present: `crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-backend.toml`.
- Both commits (015f5ab0, a6658cda) present in git history.
- Acceptance greps match: `pub backend: Option<BackendSection>`, `pub script/path/method/base_url: Option<String>`.

---
*Phase: 90-openapi-built-in-server*
*Completed: 2026-05-29*
