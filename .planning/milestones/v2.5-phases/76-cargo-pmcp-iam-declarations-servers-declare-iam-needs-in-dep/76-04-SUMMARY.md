---
phase: 76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep
plan: 04
subsystem: infra
tags: [cargo-pmcp, iam, validator, cli, footgun, fail-closed, anyhow, regex]

# Dependency graph
requires:
  - phase: 76
    provides: "Full IamConfig schema (Wave 2) + deployment/iam.rs renderer + render_iam_block (Wave 3)"
provides:
  - "pub fn validate(&IamConfig) -> anyhow::Result<Vec<Warning>> enforcing 6 hard-error rules + 2 warning classes (deployment/iam.rs)"
  - "pub struct Warning { message: String } — non-blocking CLI finding shape"
  - "KNOWN_SERVICE_PREFIXES — curated list of 40 AWS service prefixes for Warning 7"
  - "cargo pmcp validate deploy — new subcommand in ValidateCommand enum, dispatched to validate_deploy(server, verbose)"
  - "DeployExecutor::execute fail-closed gate — validator runs between DeployConfig::load and any AWS API call"
  - "T-76-02 wildcard-escalation mitigation at 3 levels: in-module unit test, in-crate integration test, end-to-end CLI-gate integration test"
affects: [76-05-fuzz-example]

# Tech tracking
tech-stack:
  added: [none — uses regex 1.x + anyhow 1.x already in cargo-pmcp deps; tempfile 3.x for test fixtures was already a runtime dep]
  patterns:
    - "Private-helper factoring to stay under cognitive-complexity 25: validate → validate_tables + validate_buckets + validate_statements; validate_statements → check_statement_effect_and_shape + check_statement_wildcard_escalation + check_statement_actions + collect_cross_account_warnings. Each helper owns a single loop or rule cluster."
    - "Vec<Warning> return type (not () / not anyhow::Warning) so tests can assert warning counts deterministically — intentional divergence from the 'Err or Ok(())' idiom documented in the plan."
    - "const ACTION_REGEX + runtime Regex::new(...).expect(...) pattern — regex constructed once per validate call with .expect on a static known-good pattern, avoiding lazy_static / OnceLock complexity."
    - "const KNOWN_SERVICE_PREFIXES as a &[&str] — curated, sorted alphabetically, annotated as 'Updated annually or on user report'."
    - "Shared fixture_with_iam(iam_section) helper in deploy_validate_gate_tests — cuts TOML duplication across 4 tests by inlining only the per-test IAM section onto a COMMON_FIXTURE_HEADER."

key-files:
  created:
    - .planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-04-SUMMARY.md
  modified:
    - cargo-pmcp/src/deployment/iam.rs  # +validate + Warning + 7 helpers + KNOWN_SERVICE_PREFIXES + ACTION_REGEX + VALID_SUGAR + validate_tests (14) + validate_integration_tests (11)
    - cargo-pmcp/src/commands/validate.rs  # +ValidateCommand::Deploy variant + pub fn validate_deploy + deploy_validate_gate_tests (4)
    - cargo-pmcp/src/commands/deploy/deploy.rs  # +validator call after DeployConfig::load, before any AWS API

key-decisions:
  - "Rule-3 deviation (consistent with Wave 1/2/3 precedent): both integration test files (plan specified `cargo-pmcp/tests/iam_validate.rs` and `cargo-pmcp/tests/deploy_validate_gate.rs`) landed IN-CRATE as `#[cfg(test)] mod validate_integration_tests` inside `iam.rs` and `#[cfg(test)] mod deploy_validate_gate_tests` inside `commands/validate.rs` respectively. Reason: `cargo_pmcp::deployment` and `cargo_pmcp::commands` are NOT re-exported from `cargo-pmcp/src/lib.rs` — the lib surface is intentionally kept at `loadtest` / `pentest` / `test_support_cache`. Expanding it to expose `deployment::iam::{validate, Warning}` + `deployment::config::*` would transitively drag in the CognitoConfig + templates tree (via `utils::config::WorkspaceConfig::load` called inside `DeployConfig::load`) for very little benefit. All 11 + 4 = 15 integration tests reach the public API through `super::*` and deliver the full CR-gate coverage."
  - "Vec<Warning> return type (NOT unit-return crate-norm): `validate` returns `Result<Vec<Warning>>` — intentional divergence from the 'Err or Ok(())' idiom used elsewhere in cargo-pmcp. This lets tests deterministically assert warning counts (e.g. `warnings.is_empty()`, `warnings.iter().any(|w| w.message.contains(...))`) rather than parsing stderr. Documented on the `validate` rustdoc."
  - "Bare '*' action is permitted only when Rule 1 wouldn't trip. `check_statement_actions` treats an action that is literally `\"*\"` as a pass-through (skips the regex check) so that e.g. `effect=\"Deny\" actions=[\"*\"] resources=[\"s3:bucket/secret/*\"]` remains declarable. Rule 1 still rejects the Allow+*+* combination as a hard error, so this doesn't create a new footgun. Without this branch the ACTION_REGEX `^[a-z0-9-]+:[A-Za-z0-9*]+$` would also match bare `*` only if we widened the prefix portion to include `*`, which would weaken the regex."
  - "Factored `validate` into 3 helpers (+ 4 nested helpers inside `validate_statements`) per W-02 cognitive-complexity guidance in 76-04-PLAN.md. Each top-level helper owns one loop; `validate_statements` owns the 4-rule-per-statement pipeline. Cognitive complexity for every function sits comfortably below the 25 threshold."
  - "Cross-account ARN detection is best-effort (Rule 8): `extract_account_from_arn` parses `arn:partition:service:region:account:resource` and only flags when the account segment is exactly 12 ASCII digits. For S3 ARNs like `arn:aws:s3:::bucket/key`, the account segment is empty and no warning fires — matching real-world ARN semantics. A full cross-account check would require passing the deploy-target `aws.account_id` through to `validate`; deferred to an optional overload per the plan."
  - "Deploy gate wires via `console::style(\"warning:\")` in `deploy.rs` (the crate already uses `console` across other CLI surfaces) — keeps stderr formatting consistent between `cargo pmcp validate deploy` and `cargo pmcp deploy`."
  - "Test counts: 14 in-module validate_tests + 11 in-crate validate_integration_tests + 4 in-crate deploy_validate_gate_tests = 29 new tests (matches plan-specified 29). All pre-existing 359 tests continue to pass for a total of 388 tests in cargo-pmcp."

patterns-established:
  - "Validator-as-gate pattern: a standalone `validate(&Config) -> Result<Vec<Warning>>` function is the single source of truth; BOTH the dedicated validate CLI subcommand AND the real deploy flow call into it. Eliminates drift between 'what validate says' and 'what deploy actually rejects'. Applied here to IAM; pattern generalises to any config subsystem that needs a pre-flight gate."
  - "Private-helper factoring for cognitive complexity: when a single function enforces N rules via nested loops, factor each rule (or rule cluster) into a `check_*` / `validate_*` private helper that returns `Result<()>` and accepts a mutable warning accumulator. The top-level function chains them via `?` and returns the accumulated warnings."
  - "Shared fixture header + per-test IAM section: for TOML-fixture-based integration tests that all share a heavy `[target]` / `[aws]` / `[server]` / `[auth]` / `[observability]` prefix, define one COMMON_FIXTURE_HEADER const and a `fixture_with_iam(iam_section)` helper that format!s the per-test variant. Cuts fixture code by ~60% vs. writing 4 complete TOML blocks."

requirements-completed: [PART-2]

# Metrics
duration: ~25min
completed: 2026-04-22
---

# Phase 76 Plan 04: IamConfig Validator + Fail-Closed CLI Gate Summary

**CR-locked IAM validator with 6 hard-error rules + 2 warning classes, wired into both `cargo pmcp validate deploy` (pre-flight) AND `cargo pmcp deploy` (fail-closed gate before any AWS call).**

## Performance

- **Duration:** ~25 min
- **Tasks:** 2
- **Files modified:** 3 (iam.rs, validate.rs, deploy/deploy.rs)
- **New tests:** 29 (14 unit + 11 in-crate integration + 4 in-crate CLI-gate integration)
- **All-crate test total after Wave 4:** 388 passed (prior 359 + 29 new)

## Accomplishments

- `pub fn validate(iam: &IamConfig) -> anyhow::Result<Vec<Warning>>` enforces the CR's 6 hard-error rules (wildcard-escalation footgun T-76-02, bad effect, empty actions/resources, malformed action, invalid sugar, empty name) and emits 2 warning classes (unknown service prefix, pinned 12-digit account ARN).
- T-76-02 (Allow + `actions=["*"]` + `resources=["*"]`) is explicitly hard-errored with the message `"wildcard escalation footgun — refuse to deploy"`, tested at 3 levels: in-module unit test, in-crate integration test, end-to-end CLI-gate integration test.
- `cargo pmcp validate deploy` subcommand added to `ValidateCommand`; dispatches to `pub fn validate_deploy(server, verbose)` which loads `.pmcp/deploy.toml`, calls `validate`, prints warnings to stderr with yellow `warning:` prefix, and returns non-zero on hard errors.
- `DeployExecutor::execute` runs the validator immediately after `DeployConfig::load` — deploy blocks BEFORE any AWS API call on hard errors, matching 76-CONTEXT.md D-04 (fail-closed semantics).
- Curated list of 40 AWS service prefixes (`KNOWN_SERVICE_PREFIXES`) drives the unknown-prefix warning.
- Zero clippy warnings in new code; `cargo fmt --all -- --check` passes.

## Task Commits

Each task committed atomically:

1. **Task 1 — RED (failing tests):** `f1e0b2a2` (test)
   - 14 in-module validate_tests + 11 in-crate validate_integration_tests
   - Tests fail to compile because `validate` and `Warning` don't exist yet (classic TDD RED gate)
2. **Task 1 — GREEN (implementation):** `9be1e151` (feat)
   - `pub fn validate` + `pub struct Warning` + 7 private helpers + 40-entry `KNOWN_SERVICE_PREFIXES` + `ACTION_REGEX` + `VALID_SUGAR`
   - All 25 new validator tests pass
3. **Task 2 Part A — validate_deploy CLI handler:** `b02d6509` (feat)
   - `ValidateCommand::Deploy { server }` variant + dispatch arm + `pub fn validate_deploy(server, verbose)`
   - 4 in-crate deploy_validate_gate_tests (accepts valid, rejects wildcard, rejects bad bucket sugar, returns Ok on unknown prefix warning)
4. **Task 2 Part B — DeployExecutor fail-closed wiring:** `3362ad88` (feat)
   - `crate::deployment::iam::validate(&config.iam)?` between `DeployConfig::load` and `BinaryBuilder::build`
   - Warnings printed to stderr via `console::style("warning:").yellow()`

_Note: Task 2 was not full TDD — implementation and tests shipped together because validate_deploy is a thin wrapper and the behaviour was already locked by Task 1's validator tests. The 4 deploy_validate_gate_tests still cover end-to-end CLI-gate behaviour._

## Files Created/Modified

- `cargo-pmcp/src/deployment/iam.rs` — added validation block (Phase 76 Wave 4 section) with `validate` + `Warning` + 7 private helpers + 3 constants (`ACTION_REGEX`, `VALID_SUGAR`, `KNOWN_SERVICE_PREFIXES`) + 2 new test modules (14 + 11 tests)
- `cargo-pmcp/src/commands/validate.rs` — extended `ValidateCommand` enum with `Deploy { server }` variant; added dispatch arm to `execute`; added `pub fn validate_deploy(server, verbose)` handler; added `deploy_validate_gate_tests` module with 4 in-crate CLI-gate tests backed by a shared `fixture_with_iam` helper
- `cargo-pmcp/src/commands/deploy/deploy.rs` — inserted 4-line fail-closed gate between `DeployConfig::load` and `BinaryBuilder::build`; warnings surface via `console::style("warning:").yellow()`

## Decisions Made

See `key-decisions` in frontmatter. Highlights:

- **Rule-3 deviation:** both integration-test files stayed in-crate (consistent with Wave 1/2/3 precedent) because `cargo_pmcp::deployment` and `cargo_pmcp::commands` are NOT re-exported from `lib.rs`. Expanding the lib surface would drag in the CognitoConfig + templates tree.
- **Vec<Warning> return type:** intentional divergence from the crate's `Ok(())` norm so tests can assert warning counts deterministically.
- **Bare `"*"` action passthrough:** permitted when combined with a non-`*` resources list; Rule 1 still rejects Allow+*+*.
- **Best-effort cross-account ARN detection:** parser flags only when the account segment is exactly 12 ASCII digits, matching real-world ARN semantics.
- **Helper factoring:** `validate` splits into `validate_tables` / `validate_buckets` / `validate_statements`; `validate_statements` further splits into 4 per-rule helpers. Keeps every function comfortably under cognitive-complexity 25.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Integration tests moved in-crate (consistent with Wave 1/2/3 precedent)**

- **Found during:** Task 1 planning (before any code written)
- **Issue:** Plan specified `cargo-pmcp/tests/iam_validate.rs` (and later `cargo-pmcp/tests/deploy_validate_gate.rs`) as integration-test files that `use cargo_pmcp::deployment::iam::{validate, Warning}` and `use cargo_pmcp::commands::validate::validate_deploy`. However, `cargo-pmcp/src/lib.rs` intentionally does NOT re-export `deployment` or `commands` (lib surface kept at `loadtest` / `pentest` / `test_support_cache`). Attempting the plan's path verbatim would either fail to compile or require expanding the lib surface by ~1,500 lines to drag in `deployment::config` / `utils::config::WorkspaceConfig` / the entire `commands` tree.
- **Fix:** Both integration-test files moved in-crate as `#[cfg(test)] mod validate_integration_tests` inside `iam.rs` and `#[cfg(test)] mod deploy_validate_gate_tests` inside `commands/validate.rs`. Tests are identical in intent — they exercise the public API through `super::*` rather than through a crate-root re-export.
- **Files modified:** `cargo-pmcp/src/deployment/iam.rs`, `cargo-pmcp/src/commands/validate.rs`
- **Verification:** 11 + 4 = 15 integration-style tests pass; zero clippy warnings; pattern matches Wave 3 `proptests` module precedent documented in `76-03-SUMMARY.md` §key-decisions[0].
- **Committed in:** `f1e0b2a2` (Task 1 RED, iam.rs mod) + `b02d6509` (Task 2, validate.rs mod)

**2. [Rule 1 - Bug] Fixture TOML required full DeployConfig schema, not the partial stub in the plan template**

- **Found during:** Task 2 Part A (deploy_validate_gate_tests fixture authoring)
- **Issue:** The plan's fixture TOML used shorthand like `[target]\ntype = "pmcp-run"` + `[auth]\ntype = "none"` without `version = "1.0.0"`, `memory_mb = 512`, `timeout_seconds = 30`, `enabled = false`, `log_retention_days = 30`, etc. `DeployConfig::load` would fail to deserialize because several fields lack `#[serde(default)]` or defaults in `impl Default`.
- **Fix:** Refactored fixtures to share a single `COMMON_FIXTURE_HEADER` const with all required fields populated + a `fixture_with_iam(iam_section)` helper. Per-test fixtures now supply only the varying `[iam.*]` section.
- **Files modified:** `cargo-pmcp/src/commands/validate.rs` (deploy_validate_gate_tests module)
- **Verification:** All 4 deploy-gate tests pass; fixture TOML now round-trips through real `DeployConfig::load`.
- **Committed in:** `b02d6509` (Task 2 Part A)

**3. [Rule 2 - Missing Critical] Bare `"*"` action permitted only when Rule 1 wouldn't trip**

- **Found during:** Task 1 GREEN (regex pattern review)
- **Issue:** The CR-locked action regex `^[a-z0-9-]+:[A-Za-z0-9*]+$` requires a `service:Action` shape. A bare `"*"` action (e.g. to accompany `effect="Deny"` with tightened `resources`) wouldn't match the regex and would be rejected as a Rule 4 violation — BUT the underlying IAM policy shape permits `Action: "*"` when scoped by resources, and rejecting it would block legitimate tightening use cases.
- **Fix:** In `check_statement_actions`, skip the regex check for an action that is literally `"*"`. Rule 1 (wildcard escalation) still rejects the `Allow + actions=["*"] + resources=["*"]` combination as a hard error, so this doesn't create a new footgun.
- **Files modified:** `cargo-pmcp/src/deployment/iam.rs` (`check_statement_actions`)
- **Verification:** Explicitly documented in the helper's rustdoc comment; Rule 1 wildcard-escalation test still passes.
- **Committed in:** `9be1e151` (Task 1 GREEN)

---

**Total deviations:** 3 auto-fixed (2 blocking-class, 1 correctness-class)
**Impact on plan:** All auto-fixes were necessary for compilation / correct semantics. No scope creep — all 29 CR-specified tests pass, both CLI entry points wired, T-76-02 mitigation live.

## Issues Encountered

- Pre-existing clippy warnings in unrelated files (pentest, secrets, deployment/metadata) are out-of-scope per the execute-plan deviation-rules scope boundary — logged by running `make quality-gate` on main before this wave; NOT fixed in Wave 4.
- `console` crate is used in `commands/deploy/deploy.rs` as a crate-path reference (`console::style(...)`) rather than an imported symbol. This is consistent with how `console` is already used across the crate (no `use console::...` at file top).

## Threat Flags

None — no new security-relevant surface introduced outside the plan's `<threat_model>`. Wave 4 specifically DISCHARGES T-76-02 (wildcard escalation) at the `validate` gate, per threat-model disposition `mitigate (primary mitigation)`.

## TDD Gate Compliance

- **Task 1 RED gate:** `f1e0b2a2` (`test(76-04): RED — add validator unit + integration tests (failing)`)
- **Task 1 GREEN gate:** `9be1e151` (`feat(76-04): GREEN — implement validate + Warning + 6 hard-error rules`)
- **Task 1 REFACTOR gate:** not needed — GREEN already produced the 7-helper factoring per W-02 guidance; no further cleanup commit required.
- **Task 2 gates:** Part A (`b02d6509`) and Part B (`3362ad88`) shipped impl+tests together. validate_deploy is a thin wrapper and the behaviour was locked by Task 1's validator tests, so full RED/GREEN separation for Task 2 would produce noise commits with no gate-compliance value. The 4 deploy_validate_gate_tests provide end-to-end coverage and all pass.

## User Setup Required

None — no external service configuration required. The validator runs offline against the parsed `.pmcp/deploy.toml`.

## Next Phase Readiness

- Wave 4 delivers the full T-76-02 mitigation; operators cannot deploy a wildcard-escalation IAM policy via `cargo pmcp deploy`.
- Wave 5 (fuzz + example) can build on the stable `validate` API: `Vec<Warning>` return type is stable for proptest assertions, and `KNOWN_SERVICE_PREFIXES` is append-only for easy future additions.
- No blockers — all Wave 1/2/3 tests continue to pass (verified by running `cargo test -p cargo-pmcp --bin cargo-pmcp -- --test-threads=1` = 388 passed).

## Self-Check: PASSED

**Files verified present:**
- FOUND: cargo-pmcp/src/deployment/iam.rs (added validate + Warning + 7 helpers + 2 test modules)
- FOUND: cargo-pmcp/src/commands/validate.rs (added Deploy variant + validate_deploy + deploy_validate_gate_tests)
- FOUND: cargo-pmcp/src/commands/deploy/deploy.rs (added fail-closed gate)
- FOUND: .planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-04-SUMMARY.md (this file)

**Commits verified present in git log:**
- FOUND: f1e0b2a2 (Task 1 RED)
- FOUND: 9be1e151 (Task 1 GREEN)
- FOUND: b02d6509 (Task 2 Part A)
- FOUND: 3362ad88 (Task 2 Part B)

**Acceptance criteria verified:**
- `grep "pub fn validate" cargo-pmcp/src/deployment/iam.rs` → match at line 148 ✓
- `grep "pub struct Warning" cargo-pmcp/src/deployment/iam.rs` → match at line 52 ✓
- `grep "KNOWN_SERVICE_PREFIXES" cargo-pmcp/src/deployment/iam.rs` → 3 matches (declaration + doc + use site) ✓
- Curated prefix list entries: 40 (plan required ≥40) ✓
- `grep "wildcard escalation" cargo-pmcp/src/deployment/iam.rs` → 5 matches (doc + error + test assertion × 3) ✓
- `grep "ValidateCommand::Deploy" cargo-pmcp/src/commands/validate.rs` → 2 matches (variant at line 39, dispatch at line 52) ✓
- `grep "pub fn validate_deploy" cargo-pmcp/src/commands/validate.rs` → 1 match ✓
- `grep "crate::deployment::iam::validate" cargo-pmcp/src/commands/deploy/deploy.rs` → 1 match ✓
- `grep "IAM validation failed" cargo-pmcp/src/commands/deploy/deploy.rs` → 1 match ✓
- `cargo test -p cargo-pmcp --bin cargo-pmcp -- --test-threads=1` → 388 passed, 0 failed ✓
- `cargo fmt --all -- --check` → clean ✓
- Zero clippy warnings in plan-modified files (iam.rs, validate.rs, deploy.rs) ✓

---
*Phase: 76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep*
*Completed: 2026-04-22*
