---
phase: 76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep
plan: 03
subsystem: cargo-pmcp
tags: [cargo-pmcp, iam, translation, codegen, proptest, stack-ts, toyota-way]

# Dependency graph
requires:
  - phase: 76
    plan: 01
    provides: "Wave-1 McpRoleArn CfnOutput + render_stack_ts(server_name, &IamConfig) seam + aws-iam module import + golden-file backward-compat guards."
  - phase: 76
    plan: 02
    provides: "Full IamConfig schema (tables/buckets/statements three-vector shape) + sub-structs (TablePermission, BucketPermission, IamStatement) + is_empty refined to AND of all three vectors."
provides:
  - "`cargo_pmcp::deployment::iam::render_iam_block(&IamConfig) -> String` — top-level renderer that emits a 4-space-indented TS fragment of `mcpFunction.addToRolePolicy(...)` calls."
  - "Three private sub-renderers: `render_table`, `render_bucket`, `render_statement` — each producing one `addToRolePolicy` call."
  - "Four pure helpers: `table_actions` / `bucket_actions` (sugar-keyword expansion), `render_table_resources` (base + optional /index/* ARN), `format_single_quoted_array` (TS single-quoted array emission)."
  - "D-02 translation rules locked: read → GetItem/Query/Scan/BatchGetItem (4); write → PutItem/UpdateItem/DeleteItem/BatchWriteItem (4); readwrite → 8-action union; S3 read → GetObject; write → PutObject/DeleteObject."
  - "`render_stack_ts` now threads `crate::deployment::render_iam_block(iam)` into BOTH template branches via a single `{iam_block}` named placeholder, spliced directly after the preceding platform-composition `}}));` closer — empty-config path collapses to byte-identical output."
  - "Named-capture `format!` refactor: both branches now use zero-positional-arg form with `{server_name}` / `{iam_block}` interpolation (previously `'{}'` + positional `server_name`)."
  - "10 property tests (128 cases each) in `deployment::iam::proptests` locking D-02 invariants: one addToRolePolicy per declaration, empty-renders-empty, 4-read-action invariant, 4-write-action invariant, include_indexes biconditional, bucket action mapping, bucket object-level ARN, toml roundtrip, balanced braces, preserved effect."
  - "13 unit tests + 1 ordering test in `deployment::iam::tests` covering every translation rule enumerated in the plan's behavior clauses."
  - "3 wave3 in-crate tests in `commands::deploy::init::wave1_stack_ts_tests` locking the splice-point ordering: operator IAM appears after platform-composition IAM and before `// Outputs` in both branches."
affects: [76-04-validator, 76-05-fuzz-example]

# Tech tracking
tech-stack:
  added: [none — reuses proptest 1.x already in cargo-pmcp dev-dependencies]
  patterns:
    - "Named-capture format! with interpolated identifiers (Rust 2021): `format!(r#\"... {server_name} ... {iam_block} ...\"#)` with zero positional args — cleaner than mixed positional + named."
    - "Empty-output collapse for backward-compat: renderer returns `\"\"` on the default-input path, placeholder abuts preceding closer with no surrounding whitespace in the template, so the empty case is byte-identical to the pre-renderer version (D-05 invariant preservation)."
    - "Sub-renderer composition pattern: three private `render_*` functions each emit one `addToRolePolicy` call; top-level `render_iam_block` concatenates them in locked tables→buckets→statements order."
    - "In-crate proptest submodule: `#[cfg(test)] mod proptests` alongside the implementation, importing proptest via dev-deps. Same lib-surface-constrained context as Wave-1/Wave-2 in-crate test precedent."

key-files:
  created:
    - cargo-pmcp/src/deployment/iam.rs
    - .planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-03-SUMMARY.md
  modified:
    - cargo-pmcp/src/deployment/mod.rs  # pub mod iam; + pub use iam::render_iam_block
    - cargo-pmcp/src/commands/deploy/init.rs  # render_stack_ts iam wiring + 3 wave3 in-crate tests

key-decisions:
  - "Rule-3 deviation (consistent with Wave-1 #1 / Wave-2 #1): Task 3's property-test file landed IN-CRATE as `deployment::iam::proptests` rather than at the plan-specified path `cargo-pmcp/tests/iam_translation_props.rs`. Reason: `cargo_pmcp::deployment` is not re-exported from `cargo-pmcp/src/lib.rs` (lib surface intentionally minimal at `loadtest`/`pentest`/`test_support_cache`). Expanding lib visibility would transitively drag in the CognitoConfig + templates tree for very little over the in-crate coverage that works against the same `render_iam_block` via `super::*`. All 10 proptests + 128 cases each still deliver the full VALIDATION.md T-76-01 discharge — just in a different file."
  - "Rule-2 deviation (correctness): `pub use iam::render_iam_block` in `deployment/mod.rs` triggered an `unused_imports` warning in the bin target because the original `init.rs` call site used the full path `crate::deployment::iam::render_iam_block`. Fixed by calling through the facade `crate::deployment::render_iam_block` in `render_stack_ts` so the re-export is reachable and the plan's artifact contract (line 32 of 76-03-PLAN.md) is honoured. Consequence: zero new clippy warnings from Wave-3 changes."
  - "Splice-point layout choice: `{iam_block}` placeholder sits at the END of the line `    }}));` (no surrounding newline in the template) so the empty-output case collapses to byte-identity. The renderer's non-empty output begins with `\\n` + a 3-line banner comment, giving clean visual separation from platform-composition IAM calls without template-side whitespace handling. This is path (a) from 76-03-PLAN.md §Task 2 — the preferred approach. Wave 1 goldens re-pass unchanged; no `UPDATE_GOLDEN=1` regeneration needed."
  - "Effect normaliser policy: `render_statement` treats any effect string that is NOT case-insensitively `\"Deny\"` as `iam.Effect.ALLOW`. This deliberately keeps render and validate separable (per the must_have on line 165 of 76-03-PLAN.md: 'Unrecognized sugar keyword is RENDERED (no silent drop) — validation happens in Wave 4'). Wave 4's validator rejects effect strings outside {Allow, Deny} before `render_iam_block` runs, so the default-ALLOW branch corresponds to canonical `\"Allow\"` in the supported path. The docstring in `render_statement` explicitly cross-references this policy."
  - "Unit-test count: 13 explicit behavior tests + 1 `ordering_is_tables_then_buckets_then_statements` sanity test = 14 unit tests; plan asked for ≥13. Proptests: 10 × 128 cases (plan asked for ≥9). Wave 3 delivers 24 tests against `deployment::iam` on top of the 3 new `wave3_stack_ts_tests` in `init.rs` = 27 new tests in Wave 3."

patterns-established:
  - "In-crate proptest module (`#[cfg(test)] mod proptests { ... proptest! { ... } }`) as the go-to pattern for bin-heavy crates with minimal lib surface. First instance in cargo-pmcp: sets the pattern for future waves (Wave 4 validator tests, Wave 5 fuzz integration)."
  - "Empty-output placeholder-collapse for backward-compatibility: when introducing an optional rendered fragment into an existing template, abut the placeholder directly against the neighbouring character so the empty case is a no-op. Prefer this over template whitespace juggling."
  - "Named-capture `format!` refactor when adding a second placeholder: convert existing `format!(..., positional)` to `format!(r#\"... {named} ... {other} ...\"#)` with zero positional args. Rust 2021 interpolated identifier form is strictly cleaner than mixing positional + named args."

requirements-completed: [PART-2]

# Metrics
duration: ~15min
completed: 2026-04-23
---

# Phase 76 Plan 03: Wave 3 — IAM → TypeScript Translation Layer Summary

**`render_iam_block` ships as a pure-string renderer producing D-02-locked `addToRolePolicy` calls; `render_stack_ts` now threads it into both template branches at a single named placeholder; D-05 byte-identity invariant preserved for the empty-config path; 27 new tests (14 unit + 10 proptest × 128 cases + 3 integration-level wiring guards) lock every translation rule.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-04-23T04:45:12Z
- **Completed:** 2026-04-23T05:00:40Z (approx)
- **Tasks:** 3/3 (each as RED → GREEN commit pair for Tasks 1 + 2; single commit for Task 3 which adds tests against an already-passing implementation)
- **Files created:** 1 source (`deployment/iam.rs`)
- **Files modified:** 2 source (`deployment/mod.rs`, `commands/deploy/init.rs`)

## Accomplishments

- **PART-2 codegen shipped:** `deployment/iam.rs` delivers the complete operator-declared IAM rendering pipeline. `render_iam_block` expands DynamoDB read/write/readwrite sugar into the 4-action D-02 lists (including `BatchGetItem` and `BatchWriteItem`), S3 read/write sugar into GetObject/PutObject/DeleteObject, and raw `[[iam.statements]]` into verbatim `iam.PolicyStatement` calls with ALLOW/DENY normalisation.
- **Seam wiring complete:** `render_stack_ts` drops the underscore on its `iam` parameter, computes the rendered block once, and splices it via a single `{iam_block}` named placeholder into BOTH template branches (pmcp-run: after the `lambda:InvokeFunction` platform call; aws-lambda: after the `ApiGatewayInvoke` permission). Insertion point locked by new in-crate tests.
- **D-05 byte-identity preserved:** All 5 `backward_compat_stack_ts.rs` tests + 2 `golden_*` in-crate byte-identity checks pass against the committed Wave-1 goldens unchanged. No `UPDATE_GOLDEN=1` regeneration needed — the renderer returns `""` for the default config and the template abuts `{iam_block}` to the preceding `}}));` closer.
- **27 new tests, all passing:** 14 unit tests + 10 proptests (128 cases each = 1,280 coverage points) in `deployment::iam`, plus 3 integration-level `wave3_*` in-crate tests in `commands::deploy::init::wave1_stack_ts_tests` locking the ordering and banner-presence invariants on populated configs.
- **Zero new clippy warnings:** Pre-existing baseline of 25 warnings in cargo-pmcp dropped to 23 (the dead-code warnings on `render_stack_ts`'s `_iam` parameter and a couple of reachability-dependent warnings cleared when `render_iam_block` became reachable via the facade). Wave 3 introduces zero net clippy regressions.
- **T-76-01 discharged:** The STRIDE threat register in 76-03-PLAN.md §threat_model flagged silent-privilege-expansion (Information Disclosure / Elevation) on the three renderers as the Wave-3 attack surface. Mitigation: per-rule unit tests (14) + property tests (10 × 128) lock the exact action sets emitted for each sugar keyword. A refactor adding or dropping an action breaks tests loud. Wave 1's golden-file guard additionally catches empty-path drift.

## Task Commits

Each task committed atomically with `--no-verify` (parallel-worktree mode):

1. **Task 1 RED — `test(76-03): RED — add deployment::iam stub + 13 unit tests`** — `73070a29`
   - Stub `render_iam_block` returning `""` unconditionally + 13 in-module unit tests; 12 tests fail (as designed; 2 pass trivially on the empty stub — `empty_iam_renders_empty_string` and `table_include_indexes_false_omits_index_resource`).
2. **Task 1 GREEN — `feat(76-03): GREEN — implement render_iam_block with three renderers`** — `a4766b36`
   - Three private renderers (`render_table`, `render_bucket`, `render_statement`) plus four helpers (`table_actions`, `bucket_actions`, `render_table_resources`, `format_single_quoted_array`). All 14 unit tests pass.
3. **Task 2 RED — `test(76-03): RED — add wave3_stack_ts tests for populated IamConfig rendering`** — `c8a4147f`
   - Three `wave3_*` in-crate tests in `init.rs`. 2 fail (the populated-IAM cases); 1 passes trivially (the empty case).
4. **Task 2 GREEN — `feat(76-03): GREEN — wire render_iam_block into render_stack_ts both branches`** — `63104bf4`
   - Renamed `_iam` → `iam`, added `let iam_block = crate::deployment::render_iam_block(iam);`, swapped `'{}'` → `'{server_name}'` at 3 sites (pmcp-run serverId, aws-lambda serverName, aws-lambda apiName), inserted `{iam_block}` at 2 sites (after each branch's last pre-Outputs call), dropped trailing positional arguments on both `format!` calls. All 3 wave3 tests + all 9 wave1 tests + all 5 backward-compat tests pass.
5. **Task 3 — `test(76-03): add proptests module for IAM translation rules`** — `7517fb80`
   - `#[cfg(test)] mod proptests` with 10 property tests × 128 cases. In-crate placement per Rule-3 deviation (see key-decisions). Full test count: 689 (up from 679).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] Property-test file location**

- **Found during:** Task 3 planning
- **Issue:** The plan specified `cargo-pmcp/tests/iam_translation_props.rs` (an integration test file) with `use cargo_pmcp::deployment::{config::*, iam::*};`. But `cargo-pmcp/src/lib.rs` does not re-export `deployment::*` (the lib surface is intentionally minimal: `loadtest`, `pentest`, `test_support_cache`). The plan's own Task-3 §Note (line 1017) acknowledged this: "If the build fails on import, promote `render_iam_block` visibility (it is already `pub` in iam.rs)." Promoting the function alone is insufficient — the parent `deployment` module is not `pub` outside the crate.
- **Fix:** Placed the proptest module in-crate as `cargo-pmcp/src/deployment/iam.rs::proptests` using `#[cfg(test)] mod proptests`. This follows the exact precedent set by Wave 1 (76-01-SUMMARY.md Rule-3 #1) and Wave 2 (76-02-SUMMARY.md Rule-3 deviation). The tests reach `render_iam_block` via `super::*` without requiring any lib-surface expansion. Same 10 proptests, same 128 cases each, same coverage.
- **Files modified:** `cargo-pmcp/src/deployment/iam.rs` (gained the proptest submodule).
- **Commit:** `7517fb80`

**2. [Rule 2 — Missing critical functionality] Facade re-export path usage**

- **Found during:** Task 2 verification
- **Issue:** After Task 2 wired `render_iam_block` via `crate::deployment::iam::render_iam_block(iam)`, the `pub use iam::render_iam_block;` re-export in `deployment/mod.rs` (specified in the plan's artifacts list at line 32) had no in-crate call site and generated an `unused_imports` warning. The plan's artifact contract says the re-export should exist.
- **Fix:** Changed the call site in `render_stack_ts` from `crate::deployment::iam::render_iam_block(iam)` to `crate::deployment::render_iam_block(iam)` — going through the facade. This both preserves the plan's explicit artifact contract AND removes the warning.
- **Files modified:** `cargo-pmcp/src/commands/deploy/init.rs`.
- **Commit:** `63104bf4` (same commit as the Task 2 GREEN wiring).

### Deferred Issues

None. All 3 tasks completed with zero fix-attempt retries.

## Authentication Gates

None encountered (pure-Rust refactor with no external service interaction).

## Self-Check: PASSED

### Created/modified files

- `cargo-pmcp/src/deployment/iam.rs` — FOUND (contains `pub fn render_iam_block`, 14 unit tests, 10 proptests)
- `cargo-pmcp/src/deployment/mod.rs` — FOUND (contains `pub mod iam;` and `pub use iam::render_iam_block;`)
- `cargo-pmcp/src/commands/deploy/init.rs` — FOUND (contains `let iam_block = crate::deployment::render_iam_block(iam);` + 2× `{iam_block}` placeholders + 3× `wave3_*` tests)
- `.planning/phases/76-cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep/76-03-SUMMARY.md` — FOUND (this file)

### Commits

- `73070a29` test(76-03): RED stub + unit tests — FOUND
- `a4766b36` feat(76-03): GREEN render_iam_block — FOUND
- `c8a4147f` test(76-03): RED wave3_stack_ts tests — FOUND
- `63104bf4` feat(76-03): GREEN wire render_iam_block into render_stack_ts — FOUND
- `7517fb80` test(76-03): proptests module — FOUND

### Verification checklist

- [x] All 3 tasks executed
- [x] Each task committed individually with `--no-verify` in parallel-worktree mode
- [x] `deployment/iam.rs` module with three renderers + top-level `render_iam_block` (+ 4 helpers)
- [x] `deployment/mod.rs` exports `iam` module via `pub mod iam;` and re-exports the renderer via `pub use iam::render_iam_block;`
- [x] `render_stack_ts` wired to emit `{iam_block}` in BOTH template branches
- [x] 10 property tests in `deployment::iam::proptests` (plan asked for ≥9)
- [x] D-05 goldens pass BYTE-IDENTICAL (backward_compat_stack_ts.rs: 5 passed; golden_* in-crate: 2 passed)
- [x] `cargo test -p cargo-pmcp` green — 689 passed (8 suites, 9.55s)
- [x] `cargo fmt --all -- --check` clean
- [x] No modifications to STATE.md or ROADMAP.md (orchestrator owns those writes)
- [x] SUMMARY.md created and will be committed with this plan's final docs commit
