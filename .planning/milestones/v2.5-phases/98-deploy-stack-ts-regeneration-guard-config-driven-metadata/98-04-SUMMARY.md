---
phase: 98-deploy-stack-ts-regeneration-guard-config-driven-metadata
plan: 04
subsystem: cargo-pmcp / deploy guard + config-driven metadata — ALWAYS coverage + merge bar
tags: [deploy, stack-ts, metadata, proptest, fuzz, golden, example, docs, DSTK-04]
requires:
  - "cargo_pmcp::deployment::config::{DeployConfig, MetadataConfig} (lib-public, Plan 98-01)"
  - "McpMetadata::apply_config_overrides + to_cdk_context (bin-only synth seam, Plan 98-03)"
  - "DeployConfig.regenerate_stack runtime carrier + write_stack_ts_guarded contract (Plan 98-02)"
provides:
  - "in-crate proptest: config_metadata_survives_into_cdk_context (config -> apply_config_overrides -> to_cdk_context, arbitrary inputs)"
  - "external proptest: metadata_config_survives_toml_round_trip (lib-public carrier fidelity)"
  - "cargo-pmcp/fuzz/fuzz_targets/fuzz_metadata_config.rs (+ registered [[bin]]) — [metadata]/DeployConfig TOML parse fuzz (T-98-01)"
  - "cargo-pmcp/examples/deploy_stack_metadata.rs (+ fixtures/graph-rag.deploy.toml) — runnable guard + config-metadata walkthrough"
  - "cargo-pmcp/docs/commands/deploy.md — --regenerate-stack/--force + [metadata] documentation"
affects:
  - "Phase 98 mergeability — DSTK-04 closes the ALWAYS mandate; make quality-gate green"
tech-stack:
  added: []
  patterns:
    - "in-crate proptest at the real (bin-only pub) synth seam + complementary external proptest at the lib-public carrier — same lib-boundary split 98-01/02/03 used for Tests A/C"
    - "fuzz target mirrors fuzz_iam_config: utf8 guard -> toml::from_str::<DeployConfig> -> post-parse accessor (is_empty), never panics"
    - "example uses lib-public surface only; mirrors write_stack_ts_guarded's path.exists() && !regenerate predicate since the real pub(crate) helper is unreachable externally"
key-files:
  created:
    - cargo-pmcp/fuzz/fuzz_targets/fuzz_metadata_config.rs
    - cargo-pmcp/examples/deploy_stack_metadata.rs
    - cargo-pmcp/examples/fixtures/graph-rag.deploy.toml
  modified:
    - cargo-pmcp/src/deployment/metadata.rs
    - cargo-pmcp/tests/deploy_stack_ts_guard.rs
    - cargo-pmcp/fuzz/Cargo.toml
    - cargo-pmcp/docs/commands/deploy.md
decisions:
  - "Golden files UNCHANGED and that is the CORRECT outcome: 98-03 made the mcp:snapshotBaked line conditional (emitted only when [metadata] opts in), so the EMPTY-metadata golden corpus is byte-identical. UPDATE_GOLDEN=1 produced a zero-diff; the golden tests (in-crate golden_* + external backward_compat_stack_ts) are green. A non-zero diff on the empty case would have signalled a broken conditional-add — there was none, so no golden file was committed."
  - "The plan's property-test key_link (to render_stack_ts/to_cdk_context) is satisfied IN-CRATE (metadata.rs dstk04_proptests) because that synth seam is bin-only pub(crate)/unreachable from the external tests/ crate — the same lib boundary all three prior plans hit. The external tests/deploy_stack_ts_guard.rs carries the COMPLEMENTARY carrier-fidelity proptest (matching the proptest|prop_ pattern the plan requires in that file). Together they cover config -> carrier -> render."
  - "Did NOT expose a lib-public render/guard entry point to un-ignore Tests A/C. Doing so would require mounting the bin-only commands::deploy::init / deployment::targets::* tree (which references crate::commands::*) into the lib view — an architectural change (Rule 4) the prior plans deliberately avoided. The behaviors are fully proven by the in-crate render/cdk-context tests + the new in-crate proptest; Tests A/C remain documented #[ignore] reproductions."
metrics:
  duration: ~30min
  completed: 2026-06-16
---

# Phase 98 Plan 04: ALWAYS Coverage + Merge Bar (DSTK-04) Summary

Closed DSTK-04: completed the CLAUDE.md ALWAYS mandate (property + fuzz + example + docs, on top of 98-01..03's unit tests) for the stack.ts regeneration guard and config-driven `[metadata]` path, confirmed the additive `mcp:snapshotBaked` template change is golden-stable for non-opting servers, and brought `make quality-gate` green — the phase is mergeable.

## What Was Built

**Task 1 — property test + fuzz target (commit `56b63ace`)**
- **In-crate proptest** (`src/deployment/metadata.rs`, `tests::dstk04_proptests`): for an arbitrary ASCII-alnum+dash `server_type` and arbitrary `snapshot_baked` bool, `McpMetadata::apply_config_overrides` → `to_cdk_context` always advertises `mcp:serverType={server_type}` and advertises `mcp:snapshotBaked=true` **iff** opted in (non-opting renders emit no `mcp:snapshotBaked` arg at all). This is the genuine config-survives-render property at the real synth seam. A second property asserts absent config fields leave the metadata untouched.
- **External proptest** (`tests/deploy_stack_ts_guard.rs`, `dstk04_config_survives_render`): for arbitrary inputs, a `DeployConfig` carrying that `[metadata]` block survives a full TOML serialize → parse round-trip with values intact — the lib-public carrier-fidelity precondition the in-crate render property depends on. Satisfies the plan's `proptest|prop_` key-link pattern for that file.
- **Fuzz target** `cargo-pmcp/fuzz/fuzz_targets/fuzz_metadata_config.rs`: mirrors `fuzz_iam_config`/`pmcp_config_toml_parser` — utf8 guard → `toml::from_str::<DeployConfig>` (covers the `[metadata]` sub-struct) → on success calls the lib-public `MetadataConfig::is_empty()`; never panics (T-98-01). Registered as a `[[bin]]` in `fuzz/Cargo.toml` (`test=false, doc=false, bench=false`). Compiles under `cargo +nightly check --bin fuzz_metadata_config`.

**Task 2 — golden, example, docs, merge bar (commit `87389ad9`)**
- **Goldens:** `UPDATE_GOLDEN=1 cargo test -p cargo-pmcp -- golden` produced a **zero diff** — the empty-metadata golden corpus is byte-identical because 98-03's `mcp:snapshotBaked` line is conditional. Golden tests (in-crate `golden_*` + `wave3_empty_iam_still_byte_identical_to_golden`, external `backward_compat_stack_ts` 5/5) are green. No golden file changed — the additive template change is correctly invisible to non-opting servers (the STOP-and-flag condition did NOT trigger).
- **Example** `cargo-pmcp/examples/deploy_stack_metadata.rs` (+ `examples/fixtures/graph-rag.deploy.toml`): runs to completion demonstrating (1) a `[metadata] server_type="graph-rag", snapshot_baked=true` deploy.toml parsing into `DeployConfig` and the literals that the regenerated stack advertises, (2) backward-compat that a config without `[metadata]` serialises no header and never leaks `regenerate_stack`, (3) the exists-guard preserving a curated `stack.ts` when `regenerate_stack=false` and overwriting it when `true`. Lib-public surface only; mirrors the `write_stack_ts_guarded` `path.exists() && !regenerate` predicate (the real helper is `pub(crate)`).
- **Docs** `cargo-pmcp/docs/commands/deploy.md`: added a `--regenerate-stack` (alias `--force`) row to Deploy Options, a regeneration-guard note in the deploy Flow (preserved-by-default, `--regenerate-stack` to overwrite, missing file scaffolded flag-free), and a `### Config-driven stack metadata ([metadata])` subsection documenting `server_type`/`snapshot_baked`, their effect, the backward-compat (absent block = byte-identical), and a pointer to the runnable example.

## Verification

- `cargo test -p cargo-pmcp --test deploy_stack_ts_guard` — 3 passed, 2 ignored (Tests A/C by design; the new external proptest is among the 3).
- `cargo test -p cargo-pmcp --bin cargo-pmcp dstk04` — 2 passed (in-crate proptests).
- `cargo +nightly check --bin fuzz_metadata_config` (in `cargo-pmcp/fuzz/`) — compiles clean.
- `cargo run -p cargo-pmcp --example deploy_stack_metadata` — runs to completion, all inline asserts pass.
- `cargo test -p cargo-pmcp --test backward_compat_stack_ts` — 5 passed; in-crate `golden_*` 5 passed (goldens byte-identical, zero diff after `UPDATE_GOLDEN=1`).
- `cargo fmt --all -- --check` — clean.
- **`make quality-gate` — PASSED** (full Toyota Way: fmt --all, clippy pedantic+nursery, build, test, audit, ALWAYS requirements incl. example build, Phase 91–95 purity gates). No `--no-verify`; pre-commit hook ran on both task commits.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — bug] proptest format-string compile error (mixed inline/positional args).**
- **Found during:** Task 1.
- **Issue:** The in-crate `prop_assert_eq!` failure message used an inline `{snapshot_baked}` capture alongside a positional `{:?}` — `prop_assert_eq!`'s format expansion rejected the named capture ("no argument named `snapshot_baked`").
- **Fix:** Switched the `snapshot_baked` interpolation to a positional `{}` with an explicit argument.
- **Files modified:** `cargo-pmcp/src/deployment/metadata.rs`.
- **Commit:** `56b63ace`.

### Plan-sanctioned outcomes (not gaps)

**2. Property test split across in-crate + external (lib boundary).** The plan's `key_link` points the property test at `render_stack_ts/to_cdk_context`. Those are bin-only `pub(crate)` and unreachable from the external `tests/` crate — the same boundary 98-01/02/03 documented. The end-to-end "config survives into `to_cdk_context`" property is therefore proven IN-CRATE (`metadata.rs::dstk04_proptests`), with the external file carrying the complementary lib-public carrier round-trip proptest (satisfying the `proptest|prop_`-in-`deploy_stack_ts_guard.rs` requirement). This is the plan's stated lib-boundary fallback, not a reduction in coverage.

**3. Goldens unchanged (zero diff = correct).** The plan anticipated an additive `mcp:snapshotBaked` golden diff. Because 98-03 made that line conditional on `[metadata]` opt-in, the EMPTY-metadata golden corpus is byte-identical and `UPDATE_GOLDEN=1` produced no change. Per the plan's STOP-and-flag rule, a *non-zero* diff on the empty case would have signalled a broken conditional-add; a zero diff confirms it is correct. No golden file was committed.

**4. Did not expose a lib-public render/guard surface to un-ignore Tests A/C.** The 98-02/98-03 handoff asked whether 98-04 should re-export the renderer/guard to flip Tests A/C live. Doing so requires mounting the bin-only `commands::deploy::init` / `deployment::targets::*` tree (which references `crate::commands::*`) into the lib view — an architectural change the prior plans deliberately avoided. The behaviors are fully proven by in-crate render + cdk-context tests + the new in-crate proptest; Tests A/C stay as documented `#[ignore]` reproductions.

## Deferred Issues

- Pre-existing lib proptest failure `commands::auth_cmd::cache::test_support_cache::proptests::normalize_round_trip_idempotent` (`cargo-pmcp/src/commands/auth_cmd/cache.rs:419`) — logged in 98-02/98-03's `deferred-items.md`, reproduces on clean HEAD independent of Phase 98, and is NOT in `make quality-gate`'s test set (the gate passed). NOT introduced by this phase; left untouched per SCOPE BOUNDARY.

## Known Stubs

- `render_stack_ts_with_metadata` in `tests/deploy_stack_ts_guard.rs` remains a `String::new()` placeholder for the still-`#[ignore]`d Test C (bin-only renderer unreachable externally; live DSTK-02/03 proof is the in-crate `phase98_metadata_render_tests` + the new `dstk04_proptests`). Not a product-path stub. Unchanged by this plan.

## Threat Flags

None beyond the plan's registered threats. T-98-01 (`[metadata]` parser DoS) is now mitigated by the landed `fuzz_metadata_config` target. T-98-08 (golden drift hiding a breaking template change) is mitigated by the zero-diff golden regeneration + the green golden tests. T-98-SC (package installs): no new packages — `proptest`/`libfuzzer-sys` were already dev/fuzz deps.

## Self-Check: PASSED

- FOUND: `cargo-pmcp/fuzz/fuzz_targets/fuzz_metadata_config.rs`
- FOUND: `cargo-pmcp/examples/deploy_stack_metadata.rs`
- FOUND: `cargo-pmcp/examples/fixtures/graph-rag.deploy.toml`
- FOUND: `cargo-pmcp/docs/commands/deploy.md` (contains `regenerate-stack` + `[metadata]`)
- FOUND commit `56b63ace` (Task 1)
- FOUND commit `87389ad9` (Task 2)

## Handoff

- **Phase 98 is mergeable.** DSTK-01/02/03/04 are all closed; `make quality-gate` is green modulo the documented pre-existing `test_support_cache` proptest (not in the gate's test set, not Phase-98-introduced).
- A future plan that wants live black-box Tests A/C must first decide to expose a lib-public render/guard surface (an architectural change deferred here); the behavior contracts are already fully covered by in-crate tests + proptests, so this is ergonomics-only.
