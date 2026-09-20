---
phase: 98-deploy-stack-ts-regeneration-guard-config-driven-metadata
plan: 03
subsystem: cargo-pmcp / deployment config-driven stack.ts metadata
tags: [deploy, stack-ts, metadata, snapshot-baked, server-type, DSTK-02, DSTK-03]
requires:
  - "cargo_pmcp::deployment::config::MetadataConfig (server_type, snapshot_baked) — Plan 98-01"
  - "cargo_pmcp::deployment::config::DeployConfig.metadata field — Plan 98-01"
  - "render_stack_ts_for_deploy / InitCommand::render_stack_ts (bin-only renderer, threaded only IAM before this plan)"
  - "McpMetadata + to_cdk_context (synth-context seam, hardcoded server_type='custom')"
provides:
  - "cargo_pmcp::deployment::metadata::McpMetadata.snapshot_baked: bool (serde skip-when-false)"
  - "McpMetadata::apply_config_overrides(&MetadataConfig) — server_type override + snapshot_baked"
  - "McpMetadata::to_cdk_context emits -c 'mcp:snapshotBaked=true' when opted in"
  - "init::StackMetadata carrier + StackMetadata::from_config; render_stack_ts(_for_deploy) carry it"
  - "render_aws_lambda_metadata_block (bin-only) — bakes metadata literals into the aws-lambda template"
affects:
  - "Plan 98-04 (docs + CLI acceptance) — documents [metadata] block; decides on a lib-public render entry point to flip Test C live"
tech-stack:
  added: []
  patterns:
    - "explicit metadata carrier struct threaded through the renderer (per CONTEXT.md DSTK-02 — NOT widening the throwaway-InitCommand hack)"
    - "skip_serializing_if = is_false for the bool snapshot_baked (byte-identity for non-opting servers, mirrors the IamConfig/MetadataConfig D-05 contract)"
    - "conditional template seam ('' when absent) for additive metadata lines, mirroring the {iam_block} seam"
    - "config override applied to extracted McpMetadata at the synth call site (pmcp-run) BEFORE to_cdk_context"
    - "bin-only renderer proven via in-crate tests; external integration Test C stays #[ignore] (lib-boundary handoff to 98-04)"
key-files:
  created: []
  modified:
    - cargo-pmcp/src/deployment/metadata.rs
    - cargo-pmcp/src/commands/deploy/init.rs
    - cargo-pmcp/src/commands/deploy/deploy.rs
    - cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs
    - cargo-pmcp/tests/deploy_stack_ts_guard.rs
decisions:
  - "snapshot_baked is a plain bool with skip_serializing_if=is_false (not Option<bool>) on McpMetadata — the config knob is Option<bool> in MetadataConfig, but the extracted metadata only needs the resolved value; false elides for byte-identity."
  - "aws-lambda bakes the literals directly into the template (no tryGetContext default) because run_cdk_deploy passes NO -c context flags — confirmed by reading run_cdk_deploy. pmcp-run reaches the template via CDK synth context (-c 'mcp:serverType=…'/'mcp:snapshotBaked=…') AND carries the baked default, so both targets reproduce the literals."
  - "render_aws_lambda_metadata_block returns '' when [metadata] is absent → the aws-lambda template gains NO metadata block (it had none before), keeping non-opting output byte-identical."
  - "Test C stays #[ignore] (the renderer is bin-only pub(crate), unreachable from the external tests/ crate) and DSTK-02/03 is instead proven by 3 in-crate render tests + 3 in-crate cdk-context tests — the same lib-boundary fallback 98-01/98-02 used for Tests A/C; 98-04 owns the lib-public-surface decision."
metrics:
  duration: ~40min
  completed: 2026-06-16
---

# Phase 98 Plan 03: Config-Driven stack.ts Metadata (DSTK-02 + DSTK-03) Summary

Closed DSTK-02 + DSTK-03: curated template metadata (`mcp:serverType`, `mcp:snapshotBaked`) is now reproducible-from-config so a `stack.ts` regeneration is SAFE. A `[metadata]` block with `server_type = "graph-rag"`, `snapshot_baked = true` in `.pmcp/deploy.toml` now flows through `McpMetadata` → `to_cdk_context` → both render-template branches, surfacing `mcp:serverType:'graph-rag'` + `mcp:snapshotBaked:'true'` into the synthesized stack — instead of the previously hardcoded `'custom'` with no `snapshotBaked` representation. Absent `[metadata]` ⇒ rendered output byte-identical (backward-compat proven by the unchanged golden tests).

## What Was Built

**Task 1 — McpMetadata carries snapshot_baked + config server_type override (commit `0e62563c`)**
- Added `pub snapshot_baked: bool` to `McpMetadata`, annotated `#[serde(default, skip_serializing_if = "is_false")]` with a new `is_false(&bool)` serde predicate, so non-opting JSON / CloudFormation metadata is byte-identical (DSTK-03 backward-compat).
- Initialized `snapshot_baked: false` at all 4 construction sites (builtin-manifest branch, both `"custom"` branches in `from_pmcp_toml`, `default_from_cargo`) + the 2 in-module test fixtures.
- Added `McpMetadata::apply_config_overrides(&mut self, &MetadataConfig)`: `Some(server_type)` REPLACES `self.server_type` (DSTK-02 — overrides hardcoded `'custom'`); `Some(snapshot_baked)` sets `self.snapshot_baked` (DSTK-03). Absent config fields leave the extracted values untouched.
- `to_cdk_context()` conditionally pushes `-c 'mcp:snapshotBaked=true'` ONLY when `snapshot_baked` is true (single-quoted, mirroring existing args). `to_cloudformation_metadata()` mirrors the conditional `mcp:snapshotBaked` entry.
- 4 unit tests: override replace-and-untouched, conditional cdk-context emission, server_type override surfacing in cdk-context for a custom server, JSON elision of `false`.

**Task 2 — render threading + synth override + aws-lambda baking (commit `d33a06c9`)**
- New `StackMetadata` carrier (`server_type: Option<String>`, `snapshot_baked: Option<bool>`) + `StackMetadata::from_config(&MetadataConfig)` in `init.rs` (the explicit metadata struct CONTEXT.md DSTK-02 mandated — NOT a widening of the throwaway-`InitCommand` hack).
- `InitCommand::render_stack_ts` and `render_stack_ts_for_deploy` now carry the metadata; updated all internal call sites (`create_stack_ts`, 9 wave1 tests) and both production callers (`DeployExecutor::regenerate_stack_ts` in `commands/deploy/deploy.rs`, `validate_and_regenerate_stack_ts` in `targets/pmcp_run/deploy.rs`) to pass `&config.metadata`.
- pmcp-run template: `mcpServerType` default literal is now the config override (`|| '{server_type_default}'`); a conditional `{snapshot_baked_block}` adds `metadata['mcp:snapshotBaked'] = … || 'true';` only when opted in (`""` otherwise → byte-identical).
- aws-lambda template: new `render_aws_lambda_metadata_block` bakes a `templateOptions.metadata` block with `'mcp:serverType': '<override>'` and (when opted in) `metadata['mcp:snapshotBaked'] = 'true'` — injected ONLY when `[metadata]` is set (the aws-lambda template had no metadata block before; `cdk deploy` passes no `-c` context, confirmed in `run_cdk_deploy`).
- pmcp-run synth path: after `extract_metadata_with_log`, applies `apply_config_overrides(&config.metadata)` before `to_cdk_context`, so the operator's `server_type`/`snapshot_baked` reach the synth context.
- 3 in-crate render tests (`phase98_metadata_render_tests`): pmcp-run reproduces literals, aws-lambda bakes literals, absent-metadata leaves both targets unchanged. Test C in `tests/deploy_stack_ts_guard.rs` documentation updated to DSTK-02/03-satisfied with the in-crate proof list + 98-04 handoff.

## Verification

- `cargo build -p cargo-pmcp` — 0 `error[`, clean.
- `deployment::metadata::tests` (bin) — 8 passed (incl. the 4 new DSTK-02/03 unit tests).
- `commands::deploy::init::phase98_metadata_render_tests` (bin) — 3 passed (pmcp-run reproduce, aws-lambda bake, absent-metadata unchanged).
- `commands::deploy::init::wave1_stack_ts_tests` (bin) — 9 passed, including `wave3_empty_iam_still_byte_identical_to_golden` + `golden_*` (default `StackMetadata` renders byte-identically — backward-compat proven).
- `tests/backward_compat_stack_ts.rs` — 5 passed (committed goldens unchanged).
- `tests/deploy_stack_ts_guard.rs` — 2 passed, 2 ignored (Test A + Test C, both by design with documented lib-boundary handoffs).
- `make quality-gate` — PASSED (full Toyota Way: fmt --all, clippy pedantic+nursery, build, test, audit, ALWAYS requirements, Phase 91–95 purity gates). No `--no-verify` used; pre-commit hook ran on both task commits.
- PMAT cognitive-complexity check on touched files (`init.rs`, `metadata.rs`, `pmcp_run/deploy.rs`, `deploy/deploy.rs`) — no violations (cog ≤25). Zero SATD.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — blocking lib boundary] Test C kept `#[ignore]`; DSTK-02/03 proven by in-crate tests instead.**
- **Found during:** Task 2.
- **Issue:** The plan's Task 2 `<action>` says to "un-ignore [Test C] and adjust its assertion target." But the render entry points (`render_stack_ts_for_deploy`, `InitCommand::render_stack_ts`) are `pub(crate)` in the bin-only `commands::deploy::init` tree that `lib.rs` intentionally does not re-export — the external `tests/` crate genuinely cannot reach the real renderer (the same boundary that kept Tests A/C ignored in 98-01/98-02 and documented in `backward_compat_stack_ts.rs`).
- **Fix:** Proved DSTK-02/03 with 3 in-crate render tests (`phase98_metadata_render_tests`) against the real `render_stack_ts_for_deploy`, plus 3 in-crate `to_cdk_context` tests in `metadata.rs`. Updated Test C's doc + `#[ignore]` reason to "DSTK-02/03 satisfied in 98-03 via in-crate tests" with the explicit proof list and the 98-04 lib-public-surface handoff. This is the plan's stated fallback ("or, if the render entry points are pub(crate)-only and unreachable from the external test, prove DSTK-02/03 via in-crate unit tests and document the handoff, exactly as 98-02 did for its Test A"), not a gap.
- **Files modified:** `cargo-pmcp/tests/deploy_stack_ts_guard.rs`, `cargo-pmcp/src/commands/deploy/init.rs`, `cargo-pmcp/src/deployment/metadata.rs`.
- **Commits:** `0e62563c`, `d33a06c9`.

**2. [Rule 2 — missing critical functionality] aws-lambda template baked metadata directly (no tryGetContext default reliance).**
- **Found during:** Task 2.
- **Issue:** The plan noted the aws-lambda path "uses template-baked metadata, NOT to_cdk_context" and asked to confirm via `run_cdk_deploy`. Confirmed: `run_cdk_deploy` passes only env vars, NO `-c` context flags — so a `tryGetContext('mcp:serverType')` in the aws-lambda template would always fall through to its default. The aws-lambda template also had NO metadata block at all before this plan.
- **Fix:** Added `render_aws_lambda_metadata_block` that bakes the literals directly from the config carrier, injected only when `[metadata]` is set (returns `""` otherwise → byte-identical for non-opting servers). This makes DSTK-02/03 actually reproducible on aws-lambda, not just pmcp-run.
- **Files modified:** `cargo-pmcp/src/commands/deploy/init.rs`.
- **Commit:** `d33a06c9`.

## Deferred Issues

- Pre-existing lib proptest failure `commands::auth_cmd::cache::proptests::normalize_round_trip_idempotent` (`cargo-pmcp/src/commands/auth_cmd/cache.rs:419`) — already logged to `deferred-items.md` in Plan 98-02; reproduced on the bin test runner with this plan's changes set aside, unrelated to the deploy/stack.ts metadata work. Left untouched per SCOPE BOUNDARY. (`make quality-gate` passes — this proptest is not in the gate's test set.)
- Several bin test failures under parallel `--test-threads` (configure/show, doctor widget checks, deploy manifest-root guard) are CWD/test-isolation flakes that pass under `--test-threads=1` (the CI convention per CLAUDE.md). Not introduced by this plan.

## Known Stubs

- `render_stack_ts_with_metadata` in `tests/deploy_stack_ts_guard.rs` remains a `String::new()` placeholder for the still-`#[ignore]`d Test C — the renderer is bin-only `pub(crate)` so the external test cannot call the real one. The live DSTK-02/03 proof is the in-crate `phase98_metadata_render_tests`; not a product-path stub. 98-04 decides whether to expose a lib-public surface and flip Test C live.

## Threat Flags

None beyond the plan's registered threats. The change keeps the existing shell-safety convention (T-98-06): `to_cdk_context` single-quotes `mcp:snapshotBaked` (a bool literal) and `mcp:serverType` (an operator-controlled string from their own deploy.toml). The aws-lambda template bakes the same operator-controlled values as TS string literals. T-98-07 (info-disclosure for non-opting servers) is mitigated as planned: the conditional template seam + serde skip-when-false keep non-opting output byte-identical (golden tests enforce).

## Self-Check: PASSED

- FOUND: `cargo-pmcp/src/deployment/metadata.rs` (snapshot_baked field + apply_config_overrides + conditional to_cdk_context/to_cloudformation_metadata)
- FOUND: `cargo-pmcp/src/commands/deploy/init.rs` (StackMetadata + render threading + both template branches + render_aws_lambda_metadata_block + phase98_metadata_render_tests)
- FOUND: `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` (apply_config_overrides in synth path + config.metadata into render)
- FOUND: `cargo-pmcp/src/commands/deploy/deploy.rs` (config.metadata into render)
- FOUND: `cargo-pmcp/tests/deploy_stack_ts_guard.rs` (Test C doc updated)
- FOUND commit `0e62563c` (Task 1)
- FOUND commit `d33a06c9` (Task 2)

## Handoff

- **Plan 98-04 (docs + CLI acceptance):** document the `[metadata]` block (`server_type`, `snapshot_baked`) alongside `--regenerate-stack`/`--force` in `cargo-pmcp/docs/commands/deploy.md`. Decide whether to expose a lib-public render entry point (e.g. re-export `render_stack_ts_for_deploy`) so the external `tests/deploy_stack_ts_guard.rs` Test C (and Test A) can flip from `#[ignore]` to live black-box assertions. The full DSTK-02/03 behavior is already proven by the in-crate `phase98_metadata_render_tests` + `deployment::metadata::tests`; 98-04 only owns the external-surface ergonomics + docs.
