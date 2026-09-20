---
phase: 76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep
plan: 02
subsystem: cargo-pmcp
tags: [cargo-pmcp, iam, config, serde, schema, toml, toyota-way]

# Dependency graph
requires:
  - phase: 76
    plan: 01
    provides: "Wave-1 `IamConfig` zero-sized stub, `#[serde(default, skip_serializing_if = \"IamConfig::is_empty\")]` wiring on `DeployConfig::iam`, `iam_wave1_tests` invariants, `McpRoleArn` CfnOutput + `render_stack_ts` seam, D-05 golden-file guard."
provides:
  - "Full `IamConfig` schema: `tables: Vec<TablePermission>` + `buckets: Vec<BucketPermission>` + `statements: Vec<IamStatement>` with `#[serde(default)]` on each vector."
  - "`TablePermission { name, actions, include_indexes: bool (default false) }` — sugar-keyword subset of DynamoDB read/write/readwrite, expanded in Wave 3."
  - "`BucketPermission { name, actions }` — S3 object-level-ARN sugar, expanded in Wave 3."
  - "`IamStatement { effect, actions, resources }` — raw PolicyStatement passthrough; validated in Wave 4."
  - "`IamConfig::is_empty` refined to `tables.is_empty() && buckets.is_empty() && statements.is_empty()` — preserves D-05 byte-identity when every vector is empty."
  - "`iam_wave2_tests` (in-crate) — 7 struct-level tests covering: default shape, is_empty toggling, cost-coach TOML parse, include_indexes defaulting, lossless roundtrip, D-05 header elision, sub-struct public constructability."
  - "`cargo-pmcp/tests/iam_config.rs` — 6 integration tests covering the `[iam]` TOML surface via `toml::Value` reflection."
affects: [76-03-render-iam-block, 76-04-validator, 76-05-fuzz-example]

# Tech tracking
tech-stack:
  added: [none — additive schema change, no new crate-level dependencies]
  patterns:
    - "Three-vector IAM schema matching 76-CONTEXT.md §Scope / CLI_IAM_CHANGE_REQUEST.md (DynamoDB tables + S3 buckets + raw statements)."
    - "Default via `#[derive(Default)]` when all fields have trivial defaults (empty Vec); `clippy::derivable_impls` enforces. Hand-written `impl Default` reserved for structs with non-trivial seeded defaults (e.g. `AssetsConfig::exclude = [\"**/*.tmp\", ...]`)."
    - "Fixture pattern for parse tests: build valid baseline via `DeployConfig::default_for_server` → `toml::to_string` → append `[[iam.*]]` blocks, rather than hand-crafting every required non-IAM field."
    - "Integration-test-without-struct-imports: external crate observes TOML-schema invariants via `toml::Value` reflection (top-level key presence, nested array lengths, re-serialisation header absence)."

key-files:
  created:
    - cargo-pmcp/tests/iam_config.rs
  modified:
    - cargo-pmcp/src/deployment/config.rs  # IamConfig stub → full three-vector schema + 3 sub-structs + iam_wave2_tests module

key-decisions:
  - "Task-1 schema replacement landed as a RED/GREEN pair (commits 32cbba4b + 76700abb) so the Wave-2 test surface was wired before the struct body; Task 2 RED would have been trivially compilable under a valid Task 1 GREEN, so it shipped as a single commit (d57adbc3) alongside its fixture authoring."
  - "Rule-3 deviation (same class as Wave-1's Rule-3 #1): `cargo_pmcp::deployment::config` is NOT re-exported from `src/lib.rs` (surface = `loadtest`, `pentest`, `test_support_cache`). Struct-level integration tests therefore can't do `use cargo_pmcp::deployment::config::{IamConfig, TablePermission, …}`. Mitigation: full struct-level coverage moved in-crate as `iam_wave2_tests`; `tests/iam_config.rs` provides TOML-schema-surface coverage via `toml::Value` reflection — genuine integration coverage of what an operator's editor observes, without needing to expand the lib surface."
  - "Rule-1 deviation: plan's verbatim `impl Default for IamConfig` tripped `clippy::derivable_impls` (all three fields are trivial `Vec::default()`). Replaced with `#[derive(Default)]` + source comment explaining why (the plan's must_have forbids `derive(Default)` for non-trivially-defaulted fields — IamConfig's fields ARE trivially defaulted, so the must_have is satisfied)."
  - "Fixture construction changed from plan's hand-crafted minimal TOML to `default_for_server` + text append. Reason: `ServerConfig`, `ObservabilityConfig`, `TargetConfig` all have required non-`serde(default)` fields; hand-crafting a 4-line TOML produces `missing field 'memory_mb'` errors. Building on top of a known-valid baseline is both more robust and forward-compatible to any future required-field additions in neighbouring structs."
  - "`#[serde(skip_serializing_if = \"IamConfig::is_empty\")]` on `DeployConfig::iam` means the parent field is elided entirely when `is_empty` is true. For non-empty `IamConfig`, TOML emits `[[iam.tables]]` / `[[iam.buckets]]` / `[[iam.statements]]` directly — never a bare `[iam]` header — so D-05's byte-identity is preserved and populated configs produce the CR-specified output format."

patterns-established:
  - "RED-GREEN commit separation for Wave 2 schema-replacement: pairs the failing compile-state test commit with the feature-complete refactor, giving CI history a clear 'these tests require the feature' marker even when both commits land within seconds of each other."
  - "`toml::Value`-reflective integration-test pattern for crates with minimal lib surface: use `toml::from_str::<toml::Value>` + `.get().as_array().as_table()` chains to assert TOML shape invariants without importing internal struct types. Suitable wherever D-05-style byte-identity / textual-contract invariants are load-bearing."

requirements-completed: [PART-2]

# Metrics
duration: ~25min
completed: 2026-04-23
---

# Phase 76 Plan 02: Wave 2 — Full `IamConfig` Schema Summary

**Three-vector `IamConfig` schema (tables + buckets + statements) replaces the Wave-1 zero-sized stub; refined `is_empty` preserves D-05 byte-identity; 13 new tests lock the struct-level and TOML-surface invariants.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-23T04:15:00Z (approx — right after Wave 1 handoff)
- **Completed:** 2026-04-23T04:40:57Z
- **Tasks:** 2/2 + 1 fmt fix-up (applied in same commit as Task 2)
- **Files:** 1 modified, 1 created

## Accomplishments

- **Part 2 of the CR shipped:** `IamConfig` now carries the three repeated-table fields specified in 76-CONTEXT.md §Scope and `CLI_IAM_CHANGE_REQUEST.md`. All three sub-structs (`TablePermission`, `BucketPermission`, `IamStatement`) are `pub` with `pub` fields, directly addressable by Wave 3 (`render_iam_block`) and Wave 4 (validator).
- **D-05 byte-identity preserved:** The refined `is_empty` returns `true` only when every vector is empty; `#[serde(skip_serializing_if = "IamConfig::is_empty")]` elides the `iam` field entirely in that case. Wave-1's golden files (`tests/golden/pmcp-run-empty.ts`, `tests/golden/aws-lambda-empty.ts`) are BYTE-IDENTICAL post-Wave-2 (`git diff HEAD cargo-pmcp/tests/golden/` produces zero output).
- **`include_indexes` defaults to `false`:** verified at both Rust level (`TablePermission::default` via derive-trivial semantics) and TOML level (omitting the key in `.pmcp/deploy.toml` produces a parsed struct with `include_indexes = false`).
- **Full schema documentation:** each of the four public types carries rustdoc covering:
  - Which Wave's code consumes it (Wave 3 translation, Wave 4 validation)
  - The D-02 sugar-keyword expansion spec (`read` → 4 actions, `readwrite` → 8 actions)
  - The 76-CONTEXT.md invariants they're bound to (D-05 backward-compat, CR scope boundaries like S3 bucket-level ops routing through `[[iam.statements]]`)
- **13 new tests green:** 7 `iam_wave2_tests` (in-crate struct-level) + 6 `tests/iam_config.rs` (integration TOML-surface). Plus Wave-1's 14 tests remain unchanged and green.

## Task Commits

Each task committed atomically:

1. **Task 1 RED: Add `iam_wave2_tests` targeting full schema** — `32cbba4b` (test) — 224 insertions, 0 deletions. Tests fail to compile under the Wave-1 stub (no `tables`/`buckets`/`statements` fields, no sub-struct types); 47 compile errors.
2. **Task 1 GREEN: Replace stub with three-vector schema + sub-structs** — `76700abb` (feat) — 141 insertions, 46 deletions. All 7 Wave-2 tests pass; Wave-1's 3 `iam_wave1_tests` + 6 `wave1_stack_ts_tests` + 5 `backward_compat_stack_ts` remain unchanged green.
3. **Task 2: Add `tests/iam_config.rs` TOML-surface integration guards + fmt-rewrap** — `d57adbc3` (test) — 302 insertions, 13 deletions. 6 new integration tests; `cargo fmt --all -- --check` clean.

_TDD applied plan-level for Task 1 (RED + GREEN as separate commits). Task 2 is test-only by nature; its "RED" would have been a trivially-compilable empty file, so it ships as a single test-adding commit._

## Files Created/Modified

### Created
- `cargo-pmcp/tests/iam_config.rs` — 6 integration tests using `toml::Value` reflection to lock the `[iam]` TOML-surface shape from outside the crate. Doc-comment header explains why struct-import tests live in-crate (lib-surface constraint).

### Modified
- `cargo-pmcp/src/deployment/config.rs`
  - Lines 729-765 (schema block): replaced zero-sized `IamConfig` stub with `IamConfig { tables, buckets, statements }` + `#[derive(Default)]` + refined `is_empty` method (16 → 33 lines).
  - Lines 766-905 (new sub-struct block): added `TablePermission`, `BucketPermission`, `IamStatement` struct definitions with per-field documentation (88 lines).
  - Lines 906-1140 (new test block): added `iam_wave2_tests` module with 7 tests (237 lines), exercising default shape, is_empty-toggling per-vector, cost-coach parse, include_indexes default, lossless TOML roundtrip, D-05 header elision, sub-struct constructability.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] Integration test cannot `use cargo_pmcp::deployment::config::*`**
- **Found during:** Task 2 planning, before writing `tests/iam_config.rs`.
- **Issue:** The plan's Task-2 action instructed `use cargo_pmcp::deployment::config::{BucketPermission, DeployConfig, IamConfig, IamStatement, TablePermission};`. None of those paths resolve — `cargo_pmcp`'s `src/lib.rs` re-exports only `loadtest`, `pentest`, and a `#[doc(hidden)] test_support_cache` / `test_support` seam (same lib-surface minimalism Wave 1 documented in its Rule-3 deviation #1). Re-exporting `deployment::config` would transitively drag in `templates::oauth::{authorizer, proxy}` and the full CognitoConfig/OidcConfig subtree — massive surface expansion for one test file.
- **Fix (same as Wave 1):** Struct-level integration tests live in-crate as `iam_wave2_tests` (`config.rs`); `tests/iam_config.rs` provides TOML-schema integration coverage via `toml::Value` reflection — locks what an external operator's editor observes, which IS the D-05 backward-compat contract. Total test count (13) matches the plan's intent (6 integration + 7 struct-level = at or above the plan's 6-test target in `tests/iam_config.rs`).
- **Alternative considered:** Expose a `#[doc(hidden)] pub mod test_support_config` in `lib.rs` via `#[path]` single-file include of `config.rs`. Rejected because `config.rs:482` references `crate::utils::config::WorkspaceConfig` (from the bin-only CLI tree), so a `#[path]` include fails to compile in the lib target — same structural reason Wave 1 rejected the same approach.
- **Files modified:** `cargo-pmcp/tests/iam_config.rs` (created — 6 TOML-reflective integration tests), `cargo-pmcp/src/deployment/config.rs` (added `iam_wave2_tests` in-crate — 7 struct-level tests).
- **Commits:** `76700abb` (in-crate tests ship with GREEN), `d57adbc3` (integration tests).

**2. [Rule 1 — Lint] `clippy::derivable_impls` on hand-written `impl Default for IamConfig`**
- **Found during:** post-Task-1 `cargo clippy -p cargo-pmcp --all-targets -- -D warnings`.
- **Issue:** The plan's verbatim code wrote `impl Default for IamConfig { fn default() -> Self { Self { tables: vec![], buckets: vec![], statements: vec![] } } }`. Clippy's `derivable_impls` rule rejects this because all three fields are `Vec<T>` whose `Default::default()` is already an empty vector — the hand-written impl is byte-equivalent to `#[derive(Default)]`.
- **Fix:** Replaced with `#[derive(Debug, Clone, Default, Serialize, Deserialize)]` + a source comment documenting why derive is the right choice here and how this is NOT in conflict with the plan's must_have (the must_have forbids `derive(Default)` for structs with non-trivial seeded defaults like `AssetsConfig::exclude = ["**/*.tmp", ...]`; `IamConfig`'s fields are all trivial defaults, so derive is correct).
- **Files modified:** `cargo-pmcp/src/deployment/config.rs`.
- **Commit:** `76700abb` (bundled into Task 1 GREEN).

**3. [Rule 1 — Test fixture bug] Hand-crafted minimal TOML fixtures missing required non-`serde(default)` fields**
- **Found during:** Task 1 initial RED-run of `iam_wave2_tests` — 3 of 7 tests failed with `Error { message: "missing field 'memory_mb'" }`.
- **Issue:** The plan-verbatim `COST_COACH_DEPLOY_TOML` fixture only included `[target] type`, `[aws] region`, `[server] name`, and the `[[iam.*]]` blocks. `ServerConfig` requires `memory_mb` + `timeout_seconds` (no `serde(default)`), `TargetConfig` requires `version`, `ObservabilityConfig` requires three fields, `AuthConfig` requires `enabled`. Hand-crafting every field is fragile and forward-incompatible.
- **Fix:** Built the cost-coach and `include_indexes`-default fixtures by calling `DeployConfig::default_for_server(...)` → `toml::to_string` → appending the `[[iam.*]]` blocks as text. The `tests/iam_config.rs` integration fixtures instead hand-craft every field (since they can't call `default_for_server`) but include all required keys. Both approaches tested and green.
- **Files modified:** `cargo-pmcp/src/deployment/config.rs` (iam_wave2_tests fixture function `cost_coach_deploy_toml()`), `cargo-pmcp/tests/iam_config.rs` (full-field fixture consts).
- **Commits:** `76700abb`, `d57adbc3`.

### Extra Tests Added Beyond Plan

- The plan targeted 6 tests in `tests/iam_config.rs` + implied 5-7 in `iam_wave2_tests` (the plan didn't explicitly specify Wave-2 in-crate test count). Delivered **13 total**: 7 in-crate (`iam_wave2_tests`) + 6 integration (`tests/iam_config.rs`). Coverage expanded beyond the plan because splitting struct-level and TOML-surface into separate suites let each stay focused (struct-level drives the Rust API; TOML-surface drives the D-05 invariant).

## Verification Results

- `cargo test -p cargo-pmcp -- --test-threads=1` → **662 passed, 8 suites, 0 failures** (Wave-1 baseline was 649; +13 from Wave 2).
  - `cargo-pmcp` bin tests: 295 passed (unchanged from Wave 1)
  - `cargo-pmcp` main tests: 332 passed (Wave 1 had 325; +7 from `iam_wave2_tests`)
  - `tests/auth_integration.rs`: 7 passed (unchanged)
  - `tests/backward_compat_stack_ts.rs`: 5 passed (unchanged — D-05 guarded)
  - `tests/engine_property_tests.rs`: 8 passed (unchanged)
  - `tests/iam_config.rs`: **6 passed (NEW)**
  - `tests/property_tests.rs`: 7 passed (unchanged)
  - auth_integration suite: 2 passed (unchanged)
- `cargo test -p cargo-pmcp iam_wave2_tests` → 7 passed.
- `cargo test -p cargo-pmcp iam_wave1_tests` → 3 passed (unchanged from Wave 1).
- `cargo test -p cargo-pmcp wave1_stack_ts_tests` → 6 passed (unchanged).
- `cargo test -p cargo-pmcp --test backward_compat_stack_ts` → 5 passed (D-05 invariant holds).
- `cargo test -p cargo-pmcp --test iam_config` → 6 passed.
- `cargo build -p cargo-pmcp` → clean; 12 pre-existing warnings in pentest/secrets (unchanged from Wave 1 baseline).
- `cargo fmt --all -- --check` → exit 0.
- `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` → 20 pre-existing errors (all in pentest/, secrets/, loadtest/init.rs, deployment/builder.rs, deployment/metadata.rs, deployment/config.rs:494 `auto_configure_template_assets`). **Zero clippy errors introduced by Wave 2 code.** Same list as Wave 1's deferred-items.md.
- `cargo doc -p cargo-pmcp --no-deps --document-private-items` → compiles; 6 pre-existing doc warnings (unchanged).
- Wave-1 golden files (`cargo-pmcp/tests/golden/pmcp-run-empty.ts`, `cargo-pmcp/tests/golden/aws-lambda-empty.ts`) — `git diff HEAD cargo-pmcp/tests/golden/` produces ZERO output → **BYTE-IDENTICAL to Wave 1 baseline, D-05 invariant holds**.

## TDD Gate Compliance

Plan type is `execute`, not `tdd`, so plan-level RED/GREEN/REFACTOR gates are not mandatory. However, Task 1 voluntarily applied task-level TDD:
- **RED gate:** `32cbba4b` — `test(76-02): RED …` with 47 compile errors
- **GREEN gate:** `76700abb` — `feat(76-02): GREEN …` with all 7 new + 14 prior Wave-2-related tests passing

Task 2 is purely test-adding, so RED/GREEN separation would have been noise.

## Deferred Issues

No new deferrals. The 20 pre-existing clippy errors documented in Wave 1's `deferred-items.md` remain untouched (scope boundary — files not modified by this plan). They will break `make quality-gate` at the phase boundary but stay Wave 5's or a pre-phase cleanup task's responsibility per Wave-1's disposition.

## CLAUDE.md / Toyota Way Compliance

- ✅ **Unit tests** — 7 in-crate struct-level (`iam_wave2_tests`), full branch coverage of `is_empty`, round-trip, defaulting.
- ✅ **Integration tests** — 6 in `tests/iam_config.rs` observing TOML surface from external crate.
- ✅ **Property-like tests** — `populated_iam_roundtrips_losslessly_through_toml` and `cost_coach_toml_roundtrips_losslessly_through_value` are morally property tests (the property being "parse → serialise → parse yields structural equality"). Actual `proptest`-driven property tests land in Wave 5 per phase design.
- ✅ **Zero SATD** in all new code.
- ✅ **Comprehensive rustdoc** on every new public type and method — rules reference 76-CONTEXT.md D-02/D-05 and the CR document.
- ✅ **Zero clippy errors** in Wave-2-touched code (verified via `-D warnings`).
- ✅ **Zero fmt drift** (`cargo fmt --all -- --check` green).
- ✅ **Cognitive complexity ≤25** — `is_empty` is O(3-AND), test functions are flat sequential assertions.
- ❌ **Fuzz testing** — deferred to Wave 5 per phase design (planner's slice: Wave 5 = fuzz + example).
- ❌ **`cargo run --example`** — deferred to Wave 5 per phase design (cost-coach-shaped `.pmcp/deploy.toml` example).
- ❌ **`make quality-gate`** — blocked by 20 pre-existing clippy errors (see Wave-1 deferred-items.md). Wave-2 code itself passes `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` if we filter to the new lines.

## Threat Flags

None. The plan's `<threat_model>` section catalogued T-76-01 (I — iam schema info disclosure), T-76-02 (E — raw statement elevation), T-76-03 (D — toml::from_str DoS). All three have "mitigate" dispositions whose mitigation plans explicitly schedule the protective layer (Wave 3 translation locks the sugar-keyword action-sets; Wave 4's validator rejects bad effect / actions / `*:*`+`*` / empty vectors; Wave 5's fuzz target exercises arbitrary UTF-8 through `toml::from_str::<DeployConfig>`). Wave 2's job is the data shape only — it introduces no new custom `Deserialize` impl, so T-76-03's `toml::from_str` attack surface is inherited directly from the audited `toml = "1.0"` crate.

## Self-Check: PASSED

- Created files exist:
  - `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/worktrees/agent-aea4bc6f/cargo-pmcp/tests/iam_config.rs` — FOUND (6 tests, 253 lines post-fmt)
- Modified files carry Wave-2 markers:
  - `cargo-pmcp/src/deployment/config.rs` contains `pub struct IamConfig` with `tables: Vec<TablePermission>` — FOUND
  - `cargo-pmcp/src/deployment/config.rs` contains `pub struct TablePermission` — FOUND
  - `cargo-pmcp/src/deployment/config.rs` contains `pub struct BucketPermission` — FOUND
  - `cargo-pmcp/src/deployment/config.rs` contains `pub struct IamStatement` — FOUND
  - `cargo-pmcp/src/deployment/config.rs` contains `pub include_indexes: bool` — FOUND
  - `cargo-pmcp/src/deployment/config.rs` contains `self.tables.is_empty() && self.buckets.is_empty() && self.statements.is_empty()` — FOUND
  - `cargo-pmcp/src/deployment/config.rs` contains `mod iam_wave2_tests` — FOUND
- Commits in git log:
  - `32cbba4b` (Task 1 RED) — FOUND
  - `76700abb` (Task 1 GREEN) — FOUND
  - `d57adbc3` (Task 2) — FOUND
- Goldens unchanged:
  - `git diff HEAD cargo-pmcp/tests/golden/` → zero output — FOUND (D-05 invariant holds)
