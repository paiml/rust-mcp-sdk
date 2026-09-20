---
phase: 98-deploy-stack-ts-regeneration-guard-config-driven-metadata
plan: 01
subsystem: cargo-pmcp / deployment config
tags: [deploy, config, stack-ts, metadata, regression-test, DSTK-02]
requires:
  - "cargo_pmcp::deployment::config::DeployConfig (existing serde-field patterns: IamConfig::is_empty, #[serde(skip)] project_root)"
provides:
  - "cargo_pmcp::deployment::config::MetadataConfig (server_type, snapshot_baked)"
  - "DeployConfig.metadata field (skip_serializing_if = MetadataConfig::is_empty)"
  - "DeployConfig.regenerate_stack non-persisted runtime flag (#[serde(skip)])"
  - "tests/deploy_stack_ts_guard.rs RED regression suite (A/B/C + B-companion)"
affects:
  - "Plan 98-02 (DSTK-01 exists-guard + --regenerate-stack flag) — reads regenerate_stack, un-ignores Test A"
  - "Plan 98-03 (DSTK-02/03 metadata→render plumbing) — reads metadata, un-ignores Test C"
tech-stack:
  added: []
  patterns:
    - "skip_serializing_if = is_empty for optional opt-in blocks (mirrors IamConfig D-05 byte-identity)"
    - "#[serde(skip)] for non-persisted runtime flags (mirrors project_root)"
    - "#[ignore]-with-un-ignore-plan for committed-RED integration tests blocked by the bin-only lib boundary"
key-files:
  created:
    - cargo-pmcp/tests/deploy_stack_ts_guard.rs
  modified:
    - cargo-pmcp/src/deployment/config.rs
decisions:
  - "Tests A and C committed RED but #[ignore]-gated because the regenerate/render entry points are pub(crate) in the bin-only commands::*/deployment::targets::* tree the lib does not re-export; Test B (config parse) is lib-reachable and runs GREEN now."
  - "Added a 4th test (Test B companion) asserting absent-[metadata] byte-identity AND that regenerate_stack never serialises — locks both halves of the DSTK-02 backward-compat contract."
metrics:
  duration: ~12min
  completed: 2026-06-16
---

# Phase 98 Plan 01: stack.ts Regeneration Guard + Config-Driven Metadata — Config Contract + RED Regression Summary

Locked the Phase 98 shared config contract (`[metadata]` block + non-persisted `regenerate_stack` flag on `DeployConfig`) and committed a RED regression suite reproducing both reported defects (silent stack.ts overwrite + config-unreproducible metadata) before any fix lands.

## What Was Built

**Task 1 — config contract (`config.rs`, commit `9559e832`)**
- New `MetadataConfig { server_type: Option<String>, snapshot_baked: Option<bool> }`, `#[derive(Debug, Clone, Default, Serialize, Deserialize)]`, each field `#[serde(default, skip_serializing_if = "Option::is_none")]`, plus `is_empty(&self) -> bool` (both None).
- `DeployConfig.metadata: MetadataConfig` annotated `#[serde(default, skip_serializing_if = "MetadataConfig::is_empty")]` — absent `[metadata]` blocks round-trip byte-identically (DSTK-02 backward-compat, mirrors the `IamConfig` D-05 contract).
- `DeployConfig.regenerate_stack: bool` annotated `#[serde(skip)]` — the runtime opt-out carrier the `--regenerate-stack`/`--force` flag sets in Plan 98-02; never serialised (mirrors `project_root`).
- Both `DeployConfig` struct-literal constructors (`default_for_server`, `default_for_cloud_run_server`) initialise `metadata: MetadataConfig::default()` + `regenerate_stack: false`. These are the only two struct-literal sites in the crate; all other `DeployConfig` constructions go through these helpers.

**Task 2 — RED regression suite (`tests/deploy_stack_ts_guard.rs`, commit `7c2051f8`)**
- **Test A** (`curated_stack_ts_is_preserved_without_regenerate_flag`) — `#[ignore]`, un-ignore in 98-02 (DSTK-01). Reproduces the unconditional-overwrite defect: curated `stack.ts` must survive a no-flag deploy.
- **Test B** (`deploy_toml_metadata_block_parses_into_config`) — GREEN now (DSTK-02). `[metadata] server_type="graph-rag", snapshot_baked=true` parses to `Some("graph-rag")` / `Some(true)`.
- **Test B companion** (`absent_metadata_block_round_trips_byte_identically`) — GREEN now. Absent `[metadata]` serialises with no `[metadata]` header and no `regenerate_stack` key; round-trips to empty metadata + `regenerate_stack == false`.
- **Test C** (`config_metadata_survives_into_rendered_stack_ts`) — `#[ignore]`, un-ignore in 98-03 (DSTK-02/03). Reproduces the metadata-unreproducible-from-config defect: rendered `stack.ts` must advertise both literals from config.
- Curated literals `mcp:serverType:'graph-rag'` / `mcp:snapshotBaked:'true'` match `.planning/debug/deploy-overwrites-stack-ts.md` 1:1.

## Verification

- `cargo build -p cargo-pmcp` — succeeds, zero errors.
- `cargo test -p cargo-pmcp --lib deployment::config` — 24 passed, 1 ignored; includes `empty_azure_serialises_byte_identical_to_golden`, proving the committed golden `deploy-toml-no-azure.golden.toml` is unchanged (the new fields elide cleanly).
- `cargo test -p cargo-pmcp --test deploy_stack_ts_guard` — 2 passed, 2 ignored (suite GREEN).
- `cargo fmt --all -- --check` — clean.
- Pre-commit quality-gate hook ran on BOTH commits (no `--no-verify`).

## Deviations from Plan

**1. [Rule 2 — missing critical functionality] Added a 4th test (Test B companion) for the backward-compat / no-serialise half of DSTK-02.**
- **Found during:** Task 2.
- **Issue:** The plan's `<behavior>` requires both that `[metadata]` parses AND that absent-`[metadata]` configs round-trip byte-identically and `regenerate_stack` never serialises — but only the parse direction was an enumerated test (Test B). The byte-identity + skip-serialise direction is the actual backward-compat correctness guarantee.
- **Fix:** Added `absent_metadata_block_round_trips_byte_identically` asserting no `[metadata]` header, no `regenerate_stack` key, and clean round-trip to empty/false defaults.
- **Files modified:** `cargo-pmcp/tests/deploy_stack_ts_guard.rs`.
- **Commit:** `7c2051f8`.

**2. [Rule 3 — blocking lib boundary] Tests A and C committed as `#[ignore]`-gated rather than live.**
- **Found during:** Task 2.
- **Issue:** The regenerate/render entry points (`render_stack_ts_for_deploy`, `regenerate_stack_ts`, `validate_and_regenerate_stack_ts`) are `pub(crate)` inside the bin-only `commands::*`/`deployment::targets::*` tree that `cargo-pmcp/src/lib.rs` intentionally does not re-export (same constraint documented in `backward_compat_stack_ts.rs`). An integration test cannot reach them yet.
- **Fix:** Committed A and C as the reproduction with `#[ignore]` + inline un-ignore-plan reason (98-02 for A, 98-03 for C) and a documented module-level handoff. This keeps the suite GREEN for this plan while preserving the committed reproduction — exactly the plan's stated fallback in Task 2 `<action>`.
- **Files modified:** `cargo-pmcp/tests/deploy_stack_ts_guard.rs`.
- **Commit:** `7c2051f8`.

## Known Stubs

- `render_stack_ts_with_metadata` in `tests/deploy_stack_ts_guard.rs` returns `String::new()` — an intentional placeholder for the call shape of Test C, which is `#[ignore]`d until Plan 98-03 wires the real (then-reachable) renderer and un-ignores the test. Documented inline and in the module doc. Not a product-path stub; it lives only in the RED-handoff integration test.

## Threat Flags

None — no new network endpoints, auth paths, file access, or schema changes at a trust boundary beyond the planned `[metadata]` parse (already in the plan's `<threat_model>` as T-98-01, mitigated by serde Option/skip defaults; malformed input yields a parse Err, never a panic).

## Self-Check: PASSED

- FOUND: `cargo-pmcp/src/deployment/config.rs` (contains `struct MetadataConfig` at line 150)
- FOUND: `cargo-pmcp/tests/deploy_stack_ts_guard.rs`
- FOUND commit `9559e832` (Task 1)
- FOUND commit `7c2051f8` (Task 2)

## Handoff

- **Plan 98-02 (DSTK-01):** read `DeployConfig.regenerate_stack`; add the exists-guard + `--regenerate-stack`/`--force` flag on both write sites (`commands/deploy/deploy.rs:~87`, `targets/pmcp_run/deploy.rs:~742`); wire and un-ignore Test A.
- **Plan 98-03 (DSTK-02/DSTK-03):** read `DeployConfig.metadata`; thread `MetadataConfig` through `render_stack_ts_for_deploy` → `render_stack_ts` → `McpMetadata` → `to_cdk_context` → template (`mcp:serverType` override + new `mcp:snapshotBaked`); wire and un-ignore Test C; update the `backward_compat_stack_ts.rs` golden for the additive `mcp:snapshotBaked` line only when metadata is set.
