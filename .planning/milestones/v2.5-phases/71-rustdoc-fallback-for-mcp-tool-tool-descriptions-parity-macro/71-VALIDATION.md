---
phase: 71
slug: rustdoc-fallback-for-mcp-tool-tool-descriptions-parity-macro
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-17
last_updated: 2026-04-17
revision: 2
revision_reason: "Replan incorporating Codex cross-AI review (71-REVIEWS.md) — HIGH-1 proc-macro crate restriction resolved via new `pmcp-macros-support` sibling crate (Option A); HIGH-2 workspace ripple audit added; MEDIUM-1 shared resolver; MEDIUM-2 non-empty-args trybuild fixture; MEDIUM-3 Limitations subsection + `description = \"\"` test; MEDIUM-4 minor bump 2.3.0→2.4.0; LOW-3 fuzz mixed-attr shapes. Plan set restructured from 3 plans to 4 plans / 4 waves / 12 tasks."
---

# Phase 71 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + trybuild + proptest + cargo fuzz |
| **Config file** | `crates/pmcp-macros-support/Cargo.toml` (proptest dev-dep), `pmcp-macros/Cargo.toml` (trybuild dev-dep), `pmcp-macros/tests/ui/` (trybuild snapshots), `fuzz/fuzz_targets/` (cargo fuzz) |
| **Quick run command** | `cargo test -p pmcp-macros -p pmcp-macros-support` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | quick: ~60s, full: ~5-10min |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-macros -p pmcp-macros-support` (quick, both crates)
- **After every plan wave:** Run `cargo check --workspace --examples && cargo test -p pmcp-macros -p pmcp-macros-support` (confirms no call-site regressions)
- **Before `/gsd-verify-work`:** `make quality-gate` must be green workspace-wide
- **Max feedback latency:** 90 seconds for quick, ~5-10min for full

Per research (Section 5: Call-Site Blast Radius), population is 25 `#[mcp_tool]` call sites across 6 files. 100% inspection is feasible (not Nyquist-sampled). The `cargo check --workspace --examples` command is the exhaustive verifier.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 71-01-T1 | 01 | 1 | PARITY-MACRO-01 | — | New workspace crate `pmcp-macros-support` scaffolded (Cargo.toml + src/lib.rs stubs + README.md) and registered as workspace member; root Cargo.toml `members` updated | integration | `cargo check --workspace` | ❌ W0 (Plan 01 creates) | ⬜ pending |
| 71-01-T2 | 01 | 1 | PARITY-MACRO-01 | — | `extract_doc_description` + `reference_normalize` implementations land in `pmcp-macros-support`; 10 normalization vectors + 3 unsupported-form tests + 3 ref oracle sanity tests pass | unit | `cargo test -p pmcp-macros-support --lib` | ❌ W0 (Plan 01 creates) | ⬜ pending |
| 71-01-T3 | 01 | 1 | PARITY-MACRO-01 | — | ≥4 proptest invariants (reference-equivalence, determinism, no-panic, mixed-attr robustness per LOW-3) green at 1000 cases each — NO feature gate | property | `cargo test -p pmcp-macros-support --test property_tests` | ❌ W0 (Plan 01 creates) | ⬜ pending |
| 71-02-T1 | 02 | 2 | PARITY-MACRO-01 | — | `pmcp-macros` depends on `pmcp-macros-support` (path dep); shared `resolve_tool_args` resolver + `MCP_TOOL_MISSING_DESCRIPTION_ERROR` const + internal unit tests (incl. `description = ""` semantic lock per MEDIUM-3) land in `mcp_common.rs` | unit | `cargo test -p pmcp-macros --lib --features full -- rustdoc_fallback_tests` | ❌ W0 (Plan 02 creates) | ⬜ pending |
| 71-02-T2 | 02 | 2 | PARITY-MACRO-01 | — | Both parse sites (mcp_tool.rs::expand_mcp_tool + mcp_server.rs::parse_mcp_tool_attr) reduced to a single call to `resolve_tool_args` — eliminates MEDIUM-1 drift risk; pre-existing 17 tests still green | unit+integration | `cargo test -p pmcp-macros --features full --lib` | ❌ W0 (Plan 02 modifies) | ⬜ pending |
| 71-02-T3 | 02 | 2 | PARITY-MACRO-01 | — | 4 new integration tests: `test_rustdoc_only_description`, `test_attribute_wins_over_rustdoc`, `test_multiline_rustdoc_normalization`, `test_impl_block_rustdoc_harvest`; 3 s23 coexistence sites byte-unchanged | integration | `cargo test -p pmcp-macros --features full -- test_rustdoc_only_description test_attribute_wins_over_rustdoc test_multiline_rustdoc_normalization test_impl_block_rustdoc_harvest` | ❌ W0 (Plan 02 creates) | ⬜ pending |
| 71-03-T1 | 03 | 3 | PARITY-MACRO-01 | — | THREE trybuild snapshots lock the missing-description error: regenerated `mcp_tool_missing_description.stderr` + new `mcp_tool_missing_description_and_rustdoc.rs/.stderr` (empty-args) + new `mcp_tool_nonempty_args_missing_description_and_rustdoc.rs/.stderr` (non-empty-args per MEDIUM-2) | trybuild | `cargo test -p pmcp-macros --features full -- compile_fail_tests` | ❌ W0 (Plan 03 creates) | ⬜ pending |
| 71-03-T2 | 03 | 3 | PARITY-MACRO-01 | — | README migration subsection + `rust,no_run` doctest (ALWAYS EXAMPLE) + `#### Limitations` subsection enumerating unsupported forms per MEDIUM-3 | doctest | `cargo test --doc -p pmcp-macros --features full` | ✅ README exists; new subsections added | ⬜ pending |
| 71-03-T3 | 03 | 3 | PARITY-MACRO-01 | — | Fuzz target `rustdoc_normalize` compiles via `cargo build`; consumes `pmcp-macros-support` (NO `__fuzz` feature gate); exercises mixed attr shapes per LOW-3 | fuzz | `cd fuzz && cargo build --bin rustdoc_normalize` (smoke); `cd fuzz && timeout 30s cargo fuzz run rustdoc_normalize -- -max_total_time=20` (full, if cargo-fuzz installed) | ❌ W0 (Plan 03 creates) | ⬜ pending |
| 71-04-T1 | 04 | 4 | PARITY-MACRO-01 | — | Workspace `pmcp`-dep ripple audit executed (HIGH-2); version bumps applied: pmcp-macros 0.5.0→0.6.0, root 2.3.0→2.4.0 (MINOR per MEDIUM-4), 2 root pmcp-macros dep pins to 0.6.0; no non-caret pmcp pins found requiring concurrent bumps; 25 call sites compile | integration | `cargo check --workspace --examples --features full` | ✅ | ⬜ pending |
| 71-04-T2 | 04 | 4 | PARITY-MACRO-01 | — | CHANGELOG entry for 2.4.0 citing PARITY-MACRO-01 + all three crate versions (pmcp 2.4.0, pmcp-macros 0.6.0, pmcp-macros-support 0.1.0); REQUIREMENTS.md line 56 ticked + row 145 Complete + footer updated | docs | `grep -c 'PARITY-MACRO-01 \| Phase 71 \| Complete' .planning/REQUIREMENTS.md` → 1 | ✅ files exist | ⬜ pending |
| 71-04-T3 | 04 | 4 | PARITY-MACRO-01 | — | `make quality-gate` green workspace-wide (fmt, clippy pedantic+nursery, build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always); updated publish order (including `pmcp-macros-support`) recorded in SUMMARY | quality-gate | `make quality-gate` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `crates/pmcp-macros-support/Cargo.toml` + `src/lib.rs` + `README.md` — new workspace crate scaffolding — **created by Plan 01 Task 1**
- [x] `crates/pmcp-macros-support/src/lib.rs` real implementation + internal unit tests for 10 vectors + unsupported forms — **created by Plan 01 Task 2**
- [x] `crates/pmcp-macros-support/tests/property_tests.rs` — 4 proptest invariants at 1000 cases each — **created by Plan 01 Task 3**
- [x] `pmcp-macros/src/mcp_common.rs` shared resolver `resolve_tool_args` + `MCP_TOOL_MISSING_DESCRIPTION_ERROR` const + `description = ""` unit test — **created by Plan 02 Task 1**
- [x] `pmcp-macros/tests/ui/mcp_tool_missing_description_and_rustdoc.rs` + `.stderr` — empty-args compile-fail fixture — **created by Plan 03 Task 1**
- [x] `pmcp-macros/tests/ui/mcp_tool_nonempty_args_missing_description_and_rustdoc.rs` + `.stderr` — non-empty-args compile-fail fixture (MEDIUM-2) — **created by Plan 03 Task 1**
- [x] `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` — regenerated with new wording — **updated by Plan 03 Task 1 via TRYBUILD=overwrite**
- [x] `fuzz/fuzz_targets/rustdoc_normalize.rs` — cargo-fuzz target with mixed-attr-shape variation per LOW-3 — **created by Plan 03 Task 3**
- [x] `fuzz/Cargo.toml` — new `[[bin]]` entry + `pmcp-macros-support` path dep (no feature gate) — **updated by Plan 03 Task 3**

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| docs.rs rendering of rustdoc-derived tool descriptions | PARITY-MACRO-01 | Only surfaces post-publish; CI doc build is a proxy but not the real rendering | After merge + crates.io publish, visit `docs.rs/pmcp-macros/0.6.0` and confirm the new "Rustdoc-derived descriptions" + "Limitations" sections render correctly |
| Byte-for-byte precedence on 3 known pre-existing rustdoc+attribute sites in `examples/s23_mcp_tool_macro.rs` (lines 48, 56, 63) | PARITY-MACRO-01 | Automated check confirms no regression; visual spot-check adds confidence | Run `cargo run --example s23_mcp_tool_macro` and inspect the first `tools/list` response — confirm attribute-provided descriptions are intact |

---

## Review Findings Coverage Map

All 2 HIGH + 4 MEDIUM + 3 LOW findings from `71-REVIEWS.md` tracked to tasks:

| Finding | Severity | Resolved In | Evidence |
|---------|----------|-------------|----------|
| HIGH-1 | HIGH | Plan 01 (Option A — new support crate) | `test -d crates/pmcp-macros-support/` + `grep -c '__fuzz' .` returns 0 |
| HIGH-2 | HIGH | Plan 04 Task 1 | Grep output of `pmcp = ` pins recorded in SUMMARY; no non-caret pins |
| MEDIUM-1 | MEDIUM | Plan 02 Task 1 + Task 2 | `grep -c 'resolve_tool_args' pmcp-macros/src/mcp_tool.rs pmcp-macros/src/mcp_server.rs` = 2 (one call per site, no duplicated helper sequence) |
| MEDIUM-2 | MEDIUM | Plan 03 Task 1 | `test -f pmcp-macros/tests/ui/mcp_tool_nonempty_args_missing_description_and_rustdoc.rs` |
| MEDIUM-3 | MEDIUM | Plan 01 Task 2 + Plan 02 Task 1 + Plan 03 Task 2 | Unit tests for `include_str!` + `cfg_attr` unsupported + `description = ""` semantic lock + Limitations subsection |
| MEDIUM-4 | MEDIUM | Plan 04 Task 1 (2.3.0 → 2.4.0) | `grep -c '^version = "2.4.0"' Cargo.toml` = 1 |
| LOW-1 | LOW | Plan 03 Task 2 (verified) | README doctest uses `use pmcp::{mcp_tool, ServerBuilder, ServerCapabilities};` byte-identical to existing `### Example` |
| LOW-2 | LOW | Plan 02 Task 1 (explicit empty-string test) + Plan 03 Task 2 (Limitations) | `grep -c 'description = ""' pmcp-macros/src/mcp_common.rs` ≥ 1 |
| LOW-3 | LOW | Plan 01 Task 3 + Plan 03 Task 3 | Property `prop_mixed_attr_shapes_robust` at 1000 cases + fuzz target selector-based mixed shapes |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (support crate + helpers + property harness + trybuild snapshots + fuzz target + shared resolver)
- [x] No watch-mode flags
- [x] Feedback latency < 90s for quick loop, < 10min for full
- [x] `nyquist_compliant: true` — planner mapped all 12 tasks (3 per plan × 4 plans) to rows above
- [x] All 9 review findings mapped to resolving tasks in coverage map

**Approval:** planner sign-off 2026-04-17 (revision 2); pending execution sign-off on `make quality-gate` by 71-04-T3.
</content>
</invoke>