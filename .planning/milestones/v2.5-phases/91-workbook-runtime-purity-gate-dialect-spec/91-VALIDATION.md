---
phase: 91
slug: workbook-runtime-purity-gate-dialect-spec
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-09
---

# Phase 91 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `91-RESEARCH.md` § Validation Architecture (live-verified against the lighthouse source, 2026-06-09).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`#[cfg(test)] mod tests` + `#[test]`); lighthouse runtime already ships unit tests in every module |
| **Config file** | none — cargo-native |
| **Quick run command** | `cargo test -p pmcp-workbook-runtime --lib -- --test-threads=1` / `cargo test -p pmcp-workbook-dialect --lib -- --test-threads=1` |
| **Full suite command** | `make quality-gate` (fmt --all + pedantic/nursery clippy + build + test + audit) **plus** `make purity-check` |
| **Estimated runtime** | ~60–120 seconds (lib tests) + ~30s per-feature purity matrix |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <crate> --lib -- --test-threads=1`
- **After every plan wave:** Run `make quality-gate` + `make purity-check`
- **Before `/gsd:verify-work`:** Full suite green AND purity gate green (per-feature-combination)
- **Max feedback latency:** ~120 seconds

The **purity gate is the load-bearing sampling point** (WBRT-04): per feature-combination it must measure reader-absence (`umya`/`quick-xml`/`swc_*`/`pmcp-code-mode` ∉ tree), writer-presence (`rust_xlsxwriter` ∈ tree, `zip` permitted), determinism of `render_xlsx`, and doc↔const non-drift.

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| WBRT-01 | Model types serde round-trip (incl. new finding `Deserialize`) | unit | `cargo test -p pmcp-workbook-runtime finding -- --test-threads=1` | ⚠️ lift exists; ADD round-trip test for `Deserialize` (D-08) |
| WBRT-02 | `run()` evaluator determinism + per-cell traces; dependency cycle → `LintFinding` | unit | `cargo test -p pmcp-workbook-runtime sheet_ir -- --test-threads=1` | ✅ lifted from lighthouse |
| WBRT-03 | `render_xlsx` produces byte-identical deterministic output | unit | `cargo test -p pmcp-workbook-runtime render -- --test-threads=1` | ✅ lighthouse has a two-render byte-equal determinism test |
| WBRT-04 | Purity gate fails on reader presence; passes reader-free; per-feature matrix (Layer 1 cargo-tree) | CI/script | `make purity-check` (new) + per-feature CI matrix job | ✅ delivered (plan 91-03 Task 2/3) |
| WBRT-04 | Layer 2 cargo-deny `[bans]` backstop (crate-local, `--manifest-path`-scoped — workspace `deny.toml` untouched, Phase 93 unaffected) | CI/script | canonical `cargo deny --manifest-path crates/pmcp-workbook-runtime/Cargo.toml --config deny.toml check bans` (cargo-deny 0.18.3 executes the equivalent `check --config deny.toml bans` ordering; same for the `pmcp-workbook-dialect` crate) — invoked by `make purity-check` | ✅ delivered (plan 91-03 Task 2) |
| WBDL-01 | doc↔`WHITELIST` binding (drift fails build) | unit | `cargo test -p pmcp-workbook-dialect doc_whitelist_table_matches_const` | ⚠️ lift exists; adapt for flat-13 (D-05) |
| WBDL-03 | **RE-MAPPED to Phase 93** (D-02) — linter execution + `WorkbookMap` not delivered here | n/a | REQUIREMENTS.md line 103 must change `Phase 91 → Phase 93` (blocking doc edit) | ❌ mechanical doc fix, planner Task 1 |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-workbook-runtime/src/finding.rs` — ADD a `Deserialize` round-trip test (D-08 delta).
- [ ] `make purity-check` target + `.github/workflows/ci.yml` `purity-check` job appended to the `gate` job's `needs:` array (WBRT-04).
- [ ] Adapt `pmcp-workbook-dialect` binding test for the flat-13 whitelist table format (D-05).
- [ ] Verify `thiserror` 1→2 bump compiles clean under pedantic clippy.

---

## Manual-Only Verifications

_None._ The Layer 2 cargo-deny `[bans]` backstop that was previously listed here as
deferred/manual is now **delivered automatically** by plan 91-03 (Task 2) via
**crate-local** `deny.toml` files invoked with `--manifest-path` — this scopes the
ban to each workbook crate's tree as its sole graph root, so the infra-managed
workspace-global `deny.toml` is never touched and Phase 93's compiler (a separate,
non-dependent member) is unaffected. It is now an automated row in the Per-Task
Verification Map under WBRT-04.

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (finding Deserialize test, purity-check recipe+CI, flat-13 binding test)
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
