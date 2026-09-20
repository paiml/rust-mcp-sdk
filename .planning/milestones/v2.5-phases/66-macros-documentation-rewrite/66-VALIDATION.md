---
phase: 66
slug: macros-documentation-rewrite
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-11
updated: 2026-04-11
---

# Phase 66 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> **Revision note (2026-04-11):** This document was revised via `/gsd-plan-phase 66 --reviews`
> to incorporate Gemini's external plan review feedback. Rows 04-T1 and 05-T2 were updated
> to reflect new grep-based acceptance checks folded into the corresponding plans. No waves,
> dependencies, or plan structure were changed.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo doc + make quality-gate |
| **Config file** | `Cargo.toml` (workspace), `pmcp-macros/Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp-macros --doc` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~15s quick, ~5min full |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-macros --doc` (validates README + per-macro `///` doctests compile)
- **After every plan wave:** Run `cargo build -p pmcp-macros && cargo test -p pmcp-macros`
- **Before `/gsd-verify-work`:** `make quality-gate` must be green (matches CI exactly)
- **Max feedback latency:** ~15 seconds for README doctest iteration

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-T1 | 66-01 | 0 | MACR-03 | T-66-01, T-66-02 | N/A — docs phase | doctest + grep | `cargo test -p pmcp-macros --doc && grep -c '^```rust,no_run' pmcp-macros/README.md && head -3 pmcp-macros/src/lib.rs \| grep -F '#![doc = include_str!("../README.md")]'` | ❌ W0 creates both | ⬜ pending |
| 02-T1 | 66-02 | 1 | MACR-01 | T-66-03, T-66-04, T-66-05 | N/A — deletion phase | file absence + build | `! test -f pmcp-macros/src/tool.rs && ! test -f pmcp-macros/src/tool_router.rs && ! test -f pmcp-macros/tests/tool_tests.rs && ! test -f pmcp-macros/tests/tool_router_tests.rs && ! test -f pmcp-macros/tests/ui/tool_missing_description.rs && ! test -f pmcp-macros/tests/ui/tool_missing_description.stderr` | ❌ (files to be deleted) | ⬜ pending |
| 02-T2 | 66-02 | 1 | MACR-01 | T-66-03, T-66-04, T-66-05 | N/A — deletion phase | grep + cargo build/test | `! grep -n '^//! ' pmcp-macros/src/lib.rs && ! grep -E '^mod tool(_router)?;$' pmcp-macros/src/lib.rs && ! grep 'pub fn tool(' pmcp-macros/src/lib.rs && ! grep 'pub fn tool_router(' pmcp-macros/src/lib.rs && ! grep 'pub fn prompt(' pmcp-macros/src/lib.rs && ! grep 'pub fn resource(' pmcp-macros/src/lib.rs && ! grep 'tool_router_dev' pmcp-macros/Cargo.toml && cargo build -p pmcp-macros && cargo test -p pmcp-macros && cargo test -p pmcp-macros --doc` | ✓ (edits existing) | ⬜ pending |
| 03-T1 | 66-03 | 1 | MACR-01 | T-66-06 | N/A — docs phase | grep | `! grep -rn '#\[tool(' pmcp-course/src/part1-foundations/ pmcp-course/src/part5-security/ && grep -c '#\[mcp_tool(' pmcp-course/src/part5-security/ch13-oauth.md` | ✓ (edits existing) | ⬜ pending |
| 03-T2 | 66-03 | 1 | MACR-01 | T-66-06 | N/A — docs phase | grep | `! grep '#\[tool_router\]' docs/advanced/migration-from-typescript.md && ! grep '#\[tool(' docs/advanced/migration-from-typescript.md && grep -q '#\[mcp_server\]' docs/advanced/migration-from-typescript.md && grep -q '#\[mcp_tool(' docs/advanced/migration-from-typescript.md` | ✓ (edits existing) | ⬜ pending |
| 04-T1 | 66-04 | 2 | MACR-01, MACR-03 | T-66-07, T-66-08, T-66-09 | N/A — docs phase | doctest + multi-grep + URI-template grep | `[ $(wc -l < pmcp-macros/README.md) -ge 180 ] && [ $(grep -c '^```rust,no_run' pmcp-macros/README.md) -ge 4 ] && ! grep -q '^```rust,ignore' pmcp-macros/README.md && ! grep -qE 'pmcp *= *"1\.' pmcp-macros/README.md && ! grep -qi 'migration' pmcp-macros/README.md && ! grep -qF '#[tool(' pmcp-macros/README.md && grep -q 'URI template' pmcp-macros/README.md && grep -qE '\{[a-z_]+\}' pmcp-macros/README.md && cargo test -p pmcp-macros --doc` | ✓ (overwrites POC from Wave 0) | ⬜ pending |
| 04-T2 | 66-04 | 2 | MACR-01 | T-66-07 | N/A — docs phase | doctest + grep | `! grep -q 'rust,ignore' pmcp-macros/src/lib.rs && [ $(grep -c 'rust,no_run' pmcp-macros/src/lib.rs) -ge 4 ] && grep -q 's23_mcp_tool_macro' pmcp-macros/src/lib.rs && ! grep -q '63_mcp_tool_macro' pmcp-macros/src/lib.rs && grep -q 's24_mcp_prompt_macro' pmcp-macros/src/lib.rs && ! grep -q '64_mcp_prompt_macro' pmcp-macros/src/lib.rs && grep -q 'cargo run --example s23_mcp_tool_macro' examples/s23_mcp_tool_macro.rs && ! grep -q '63_mcp_tool_macro' examples/s23_mcp_tool_macro.rs && cargo test -p pmcp-macros --doc && cargo build --example s23_mcp_tool_macro --features full && cargo build --example s24_mcp_prompt_macro --features full` | ✓ (edits existing) | ⬜ pending |
| 05-T1 | 66-05 | 3 | MACR-02 | T-66-13 | N/A — docs phase | file existence + grep | `test -f pmcp-macros/CHANGELOG.md && head -1 pmcp-macros/CHANGELOG.md \| grep -q '^# Changelog' && grep -q '^## \[0\.5\.0\]' pmcp-macros/CHANGELOG.md && grep -q '^### Removed' pmcp-macros/CHANGELOG.md && grep -qF '#[mcp_tool]' pmcp-macros/CHANGELOG.md && grep -q 'Migration from 0\.4' pmcp-macros/CHANGELOG.md && grep -q '^## \[2\.3\.0\]' CHANGELOG.md && grep -q '\`pmcp-macros\` 0\.5\.0' CHANGELOG.md` | ❌ pmcp-macros/CHANGELOG.md is NEW | ⬜ pending |
| 05-T2 | 66-05 | 3 | MACR-02 | T-66-10 | N/A — version bump | grep + sweep audit + build | `grep -q 'version = "0.5.0"' pmcp-macros/Cargo.toml && ! grep -q 'version = "0.4.1"' pmcp-macros/Cargo.toml && grep -qE '^version = "2\.3\.0"' Cargo.toml && grep -q 'pmcp-macros = { version = "0\.5\.0", path = "pmcp-macros", optional = true }' Cargo.toml && grep -q 'pmcp-macros = { version = "0\.5\.0", path = "pmcp-macros" }' Cargo.toml && ! grep -q '63_mcp_tool_macro' Cargo.toml && ! grep -qE '[0-9]+_mcp_[a-z_]+_macro' Cargo.toml && cargo build -p pmcp-macros && cargo test -p pmcp-macros --doc` | ✓ (edits existing) | ⬜ pending |
| 05-T3 | 66-05 | 3 | MACR-01, MACR-02, MACR-03 | T-66-11, T-66-12 | N/A — phase gate | make target | `make quality-gate && grep -q 'nyquist_compliant: true' .planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md && grep -q 'status: approved' .planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md && cargo test -p pmcp-macros --doc` | ✓ (edits validation) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Proof-of-concept: verify `#![doc = include_str!("../README.md")]` + same-crate proc-macro doctest imports compile (~2 min task, de-risks assumptions A2 and A7 from research)
  - Plan: `66-01-poc-include-str-gate`
  - Task: `01-T1`
  - Must pass before Wave 1 starts
- [ ] Existing infrastructure (`pmcp-macros/Cargo.toml:27` already has `pmcp` dev-dependency) is sufficient; no framework install required

*The Wave 0 POC validates research assumptions A2 (use pmcp_macros::mcp_resource compiles in a README doctest included via #[doc = include_str!]) and A7 (#[cfg(doctest)] ReadmeDoctests pattern gates correctly).*

---

## Dependency Wave Structure

| Wave | Plans | Can Run In Parallel? | Rationale |
|------|-------|----------------------|-----------|
| 0 | 66-01 | N/A (single plan) | POC gate blocking Wave 1 |
| 1 | 66-02, 66-03 | YES (disjoint files_modified) | 66-02 edits pmcp-macros/; 66-03 edits pmcp-course/ + docs/ |
| 2 | 66-04 | N/A (single plan, depends on 66-01 + 66-02) | Must wait for 66-02 to clean up lib.rs before rewriting per-macro `///` docs |
| 3 | 66-05 | N/A (single plan, depends on 66-02, 66-03, 66-04) | CHANGELOG describes completed work; version bumps trigger CI |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `docs.rs/pmcp-macros` renders rewritten README as crate-level docs | MACR-02 | Requires publishing to docs.rs (post-release) | After v0.5.0 publish, visit `https://docs.rs/pmcp-macros/0.5.0/` and confirm README content is visible on the landing page |
| docs.rs auto-generated feature badges appear correctly | N/A (phase 67 territory) | Requires published crate | Post-release: check feature flag rendering on docs.rs |
| GitHub rendering of the rewritten `pmcp-macros/README.md` | MACR-01 | Requires push to GitHub | After push: visit `https://github.com/paiml/pmcp/blob/main/pmcp-macros/README.md` to confirm markdown renders cleanly (no broken links, tables display, fences highlight correctly) |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers the `include_str!` + doctest POC
- [x] No watch-mode flags
- [x] Feedback latency < 30s (cargo test -p pmcp-macros --doc is ~5-15s)
- [x] `nyquist_compliant: true` set in frontmatter
- [x] Revised 2026-04-11 via `--reviews` mode incorporating Gemini feedback:
  - Row 04-T1 now checks for `URI template` literal + `{variable_name}` token in README
  - Row 05-T2 now includes the explicit `NN_mcp_*_macro` sweep audit check
  - Plan 02 gained an executor-safety `<notes>` block for the intermediate broken-build window
- [x] `wave_0_complete: true` — flipped after Plan 01 passed (POC gate green)
- [x] `status: approved` — flipped by executor in Plan 05 Task 3 after `make quality-gate` passed (285s, all ALWAYS requirements validated)

**Approval:** approved (Plan 05 Task 3 green, 2026-04-11)
