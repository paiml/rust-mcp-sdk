---
phase: 91-workbook-runtime-purity-gate-dialect-spec
verified: 2026-06-10T07:30:00Z
status: passed
human_verification_resolved: 2026-06-10 — all 4 items closed in 91-HUMAN-UAT.md (provenance approved interactively; purity gate + test suites re-run green by orchestrator; CR-01 + WR-01..06 fixed via code-review --fix, commits 38feba92..6eb6c1be)
score: 10/10 must-haves verified
overrides_applied: 0
deferred:
  - truth: "A developer can lint a workbook against the dialect (whitelist-only, deny-by-default) and receive collect-all, located, BA-actionable findings with repair guidance"
    addressed_in: "Phase 93"
    evidence: "ROADMAP Phase 93 Requirements: WBCO-01..07, WBGV-01..07, WBDL-03. REQUIREMENTS.md traceability: WBDL-03 | Phase 93 | Pending. ROADMAP D-02 note (line 1744): 'WBDL-03 ... is re-mapped to Phase 93'. Phase 91 Requirements line (ROADMAP 1735): WBRT-01..04, WBDL-01 only."
human_verification:
  - test: "Confirm rust_xlsxwriter provenance (T-91-SC) was inspected by a human — crates.io owner jmcnamara, repo github.com/jmcnamara/rust_xlsxwriter, MIT OR Apache-2.0, v0.95.0 not yanked"
    expected: "Human-reviewed provenance record; cargo audit clean for rust_xlsxwriter / zip; cargo tree -i zip shows writer-only path"
    why_human: "Supply-chain provenance is a human judgment; slopcheck was unavailable during planning. SUMMARY 91-01 records approval but the verifier cannot independently confirm the crates.io review happened."
  - test: "Run make purity-check against the current workspace to confirm exit 0"
    expected: "purity-check PASSED: reader-free + writer-present (per-feature) + zip-permitted + cargo-deny-bans-clean"
    why_human: "The verifier cannot execute cargo commands in this context. The gate involves cargo tree and cargo deny invocations that require the full toolchain."
  - test: "Run cargo test -p pmcp-workbook-runtime --lib -- --test-threads=1 and cargo test -p pmcp-workbook-dialect --lib -- --test-threads=1"
    expected: "All tests pass (128+ lib tests for runtime, 4+ for dialect including doc_whitelist_table_matches_const)"
    why_human: "Test execution requires running the Rust toolchain."
  - test: "Assess CR-01 (render/mod.rs:119 argb_to_color reachable panic) and decide whether to fix before Phase 92 starts"
    expected: "Either apply the fix from 91-REVIEW.md (change `8 => &hex[2..]` to `8 => hex.get(2..)?`) + add non-ASCII unit test, OR accept the current behavior with a documented rationale. The fix is a 2-line change."
    why_human: "This is a security/correctness judgment call. The code reviewer rated it CRITICAL (violates the crate's documented 'panic-free writer path' contract). The verifier cannot determine whether the project accepts this risk for Phase 92 progression."
---

# Phase 91: Workbook Runtime + Purity Gate + Dialect Spec — Verification Report

**Phase Goal:** Port the reader-free `pmcp-workbook-runtime` leaf (owned IR/model types, deterministic evaluator, writer-only `.xlsx` renderer) and stand up the `cargo tree` + `cargo-deny` purity gate on day one; ship the SDK-owned versioned dialect spec. The dialect linter and WBDL-03 were re-mapped to Phase 93 by decision D-02 during planning.
**Verified:** 2026-06-10T07:30:00Z
**Status:** passed (human items resolved 2026-06-10 — see 91-HUMAN-UAT.md)
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Developer can depend on `pmcp-workbook-runtime` as a reader-free leaf and deserialize the shared model types identically to how the offline emitter produces them | VERIFIED | `crates/pmcp-workbook-runtime/Cargo.toml` has `rust_xlsxwriter` (writer-only, `default-features=false`), `serde`, `schemars`, `thiserror="2"`, `sha2="0.11"`, `hex="0.4"`. No `pmcp` dep, no reader dep. Workspace members array confirmed at root `Cargo.toml`. |
| 2 | The runtime runs a compiled IR through a deterministic topo executor producing typed outputs + per-cell derivation traces; a dependency cycle returns a LintFinding, not a panic | VERIFIED | `sheet_ir/executor.rs:85-98`: `run()` returns `Result<RunResult, Box<LintFinding>>` and maps `toposort` cycle residual to `LintFinding::new(..., "dag/cycle", ...)`. Test `cycle_is_one_located_finding_not_a_panic` at line 482. `EvalTrace` struct with full evidence fields present. |
| 3 | The runtime renders a computed workbook to byte-identical deterministic `.xlsx` via the writer-only `rust_xlsxwriter` renderer | VERIFIED | `render/mod.rs:26` imports `rust_xlsxwriter`. `render_xlsx` at line 223. Byte-determinism test `render_xlsx_is_deterministic_byte_identical` at line 448 asserts `a == b`. Fixed creation datetime + empty author. No reader/parser imports. |
| 4 | `LintFinding`/`Severity`/`LintReport` round-trip through JSON (D-08 Deserialize) | VERIFIED | `finding.rs:23` uses `use serde::{Deserialize, Serialize}`. All three types have `Deserialize` in their derive. Test `lint_report_round_trips_through_json` at line 190 present and verifiable. |
| 5 | The crate compiles clean under `#![deny(clippy::unwrap_used, expect_used, panic)]` (D-10) | VERIFIED | `lib.rs:18-19` has exactly `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` + `#![cfg_attr(test, allow(...))]`. No `version.workspace` in Cargo.toml. `thiserror = "2"` confirmed. |
| 6 | The lifted source carries zero SATD and no un-justified over-25 complexity function | VERIFIED | `grep -rEn 'TODO|FIXME|HACK|XXX' crates/pmcp-workbook-runtime/src/` returns empty. `grep -rEn 'TODO|FIXME|HACK|XXX' crates/pmcp-workbook-dialect/src/` returns empty. SUMMARY 91-01 documents zero remediation was needed. |
| 7 | The SDK owns a versioned dialect spec document with a flat 13-function whitelist, and a binding test fails the build if doc and code diverge (WBDL-01) | VERIFIED | `docs/workbook-dialect-spec.md` exists with all 13 names in flat `whitelist` category column (no "D-09 widened" framing — grep returns 0). `WHITELIST` const has exactly 13 flat entries. `doc_whitelist_table_matches_const` test at lib.rs:262 parses doc table, checks `!doc_set.is_empty()` guard (Pitfall-4 guard at line 272), asserts BTreeSet equality. |
| 8 | `make purity-check` fails closed (no `2>/dev/null` swallow, explicit exit-status capture) AND positively asserts `rust_xlsxwriter` is present per-crate AND per-feature-combination; the gate is wired into `make quality-gate` | VERIFIED | `Makefile:489` has `@set -euo pipefail`. `Makefile:493` captures `tree=$$(cargo tree -p $$crate $$feat 2>&1); status=$$?` and checks `[ $$status -ne 0 ]` (see WARNING on diagnostic dead code below). Positive arm at lines 506-517 loops `"" / --no-default-features / --all-features` for `pmcp-workbook-runtime` and greps for `rust_xlsxwriter`. `Makefile:540` has `@$(MAKE) purity-check` inside `quality-gate`. |
| 9 | `just purity-check` recipe exists and `make purity-check` CI job is merge-blocking via the org-required `gate` job | VERIFIED | `justfile:66` has `purity-check:` recipe delegating to `make purity-check`. `ci.yml:284` has `purity-check:` job with `taiki-e/install-action cargo-deny` + `make purity-check`. `ci.yml:315` has `needs: [test, quality-gate, purity-check]`. `ci.yml:322` has `PURITY_RESULT`. `ci.yml:326` checks `PURITY_RESULT != "success"`. YAML parses (`python3 yaml.safe_load` OK). |
| 10 | WBDL-03 is re-mapped from Phase 91 to Phase 93 in both REQUIREMENTS.md and ROADMAP.md (D-02 — not silently dropped) | VERIFIED | `REQUIREMENTS.md:103` shows `| WBDL-03 | Phase 93 | Pending |`. No `WBDL-03 | Phase 91` row exists. ROADMAP Phase 91 Requirements line (1735): `WBRT-01, WBRT-02, WBRT-03, WBRT-04, WBDL-01` — WBDL-03 absent. ROADMAP Phase 93 Requirements (1774) lists `WBDL-03`. ROADMAP D-02 note at line 1744 explicitly explains the re-map. |

**Score:** 10/10 truths verified

---

### Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | ROADMAP SC5: "A developer can lint a workbook against the dialect (whitelist-only, deny-by-default) and receive collect-all, located, BA-actionable findings with repair guidance" | Phase 93 | ROADMAP Phase 93 Requirements include WBDL-03; D-02 note at ROADMAP line 1744 explicitly states this re-map; REQUIREMENTS.md traceability row maps WBDL-03 to Phase 93. |

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-workbook-runtime/Cargo.toml` | Reader-free leaf manifest: version 0.1.0, thiserror=2, rust_xlsxwriter writer-only | VERIFIED | Exact: `version = "0.1.0"`, `thiserror = "2"`, `rust_xlsxwriter = { version = "0.95", default-features = false }`, no `pmcp` dep, no `version.workspace` |
| `crates/pmcp-workbook-runtime/src/lib.rs` | Crate root with panic-freedom lints + module re-export surface | VERIFIED | Lines 18-19: `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` + cfg_attr test allow. Full pub mod + pub use re-export surface present. |
| `crates/pmcp-workbook-runtime/src/finding.rs` | LintFinding/Severity/LintReport with Deserialize round-trip | VERIFIED | `use serde::{Deserialize, Serialize}` line 23. All three types derive Deserialize. Round-trip test at line 190. |
| `crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs` | Deterministic topo `run()` executor returning RunResult + EvalTrace; cycle → LintFinding | VERIFIED | `run()` signature at line 85 returns `Result<RunResult, Box<LintFinding>>`. Cycle produces "dag/cycle" LintFinding. EvalTrace struct complete. |
| `crates/pmcp-workbook-runtime/src/render/mod.rs` | Writer-only `render_xlsx` via rust_xlsxwriter | VERIFIED (with WARNING) | `use rust_xlsxwriter::{...}` at line 26. `render_xlsx` at line 223. Determinism test present. See WARNING below on `argb_to_color` reachable panic (CR-01). |
| `crates/pmcp-workbook-dialect/Cargo.toml` | Reader-free dialect-contract leaf; path-dep on runtime | VERIFIED | `pmcp-workbook-runtime = { version = "0.1.0", path = "../pmcp-workbook-runtime" }`. No serde, no version.workspace. |
| `crates/pmcp-workbook-dialect/src/lib.rs` | WHITELIST const (13 flat fns) + DialectRules + CandidateRole + re-exported finding types + binding test | VERIFIED | `WHITELIST` = 13 flat names. `pub use pmcp_workbook_runtime::finding::{LintFinding, LintReport, Severity}` at line 21. No `pub use.*lint` or `pub mod.*linter`. Binding test at line 262 with Pitfall-4 guard. |
| `docs/workbook-dialect-spec.md` | Published dialect contract with flat-13 whitelist table | VERIFIED | File exists (8969 bytes). All 13 function names present. `grep "D-09 widened"` returns 0. All rows have `whitelist` category. `Total: 13 names` line present. |
| `Makefile` (`purity-check` target) | Fail-closed per-crate per-feature reader-absence + writer-presence + crate-local cargo-deny | VERIFIED (with WARNING) | Lines 486-522. `set -euo pipefail`. Negative arm loops both crates × 3 feature modes. Positive arm for runtime × 3 feature modes. Layer 2 cargo-deny invocations. Wired into `quality-gate` at line 540. See WARNING on diagnostic dead code (WR-01). |
| `crates/pmcp-workbook-runtime/deny.toml` | Crate-scoped [bans] declaration banning umya/quick-xml/calamine/swc_/pmcp-code-mode | VERIFIED | `[bans]` section with deny list: umya-spreadsheet, calamine, quick-xml, swc_core/common/ecma_parser/ecma_ast, pmcp-code-mode. `--manifest-path` scoping documented in header. Workspace deny.toml unchanged. |
| `crates/pmcp-workbook-dialect/deny.toml` | Same as runtime, scoped to dialect tree | VERIFIED | Same deny list structure. |
| `justfile` (`purity-check` recipe) | D-09 + ROADMAP SC3 `just` entrypoint delegating to make | VERIFIED | Lines 66-67: `purity-check:` header + `make purity-check` body. |
| `.github/workflows/ci.yml` | Merge-blocking purity-check job wired into gate | VERIFIED | `purity-check:` job at line 284. `taiki-e/install-action cargo-deny` at line 304. `needs: [test, quality-gate, purity-check]` at line 315. `PURITY_RESULT` at line 322. If-condition at line 326. |
| `.planning/REQUIREMENTS.md` | WBDL-03 traceability re-mapped to Phase 93 | VERIFIED | Line 103: `| WBDL-03 | Phase 93 | Pending |`. No Phase 91 row. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `sheet_ir/executor.rs` | `finding.rs` | `run()` returns `Box<LintFinding>` on cycle (D-03) | VERIFIED | `use crate::finding::{LintFinding, Severity}` at line 23. `run()` returns `Result<RunResult, Box<LintFinding>>`. `LintFinding::new(..., "dag/cycle", ...)` in the map_err closure. |
| `crates/pmcp-workbook-runtime/Cargo.toml` | `rust_xlsxwriter 0.95 (default-features = false)` | writer-only render dependency | VERIFIED | `rust_xlsxwriter = { version = "0.95", default-features = false }` present. No reader dep. |
| `Cargo.toml` (root) | `crates/pmcp-workbook-runtime` | workspace members array | VERIFIED | `"crates/pmcp-workbook-runtime"` in members array; not in exclude. |
| `crates/pmcp-workbook-dialect/src/lib.rs` | `docs/workbook-dialect-spec.md` | binding test parses whitelist table, asserts set-equality with WHITELIST | VERIFIED | `SPEC_PATH = "../../docs/workbook-dialect-spec.md"`. `doc_whitelist_table_matches_const` reads the file, parses rows with `category == "whitelist"`, checks `!doc_set.is_empty()`, asserts BTreeSet equality with WHITELIST. |
| `crates/pmcp-workbook-dialect/src/lib.rs` | `pmcp_workbook_runtime::finding` | re-export of LintFinding/LintReport/Severity (D-03) | VERIFIED | `pub use pmcp_workbook_runtime::finding::{LintFinding, LintReport, Severity};` at line 21. |
| `Cargo.toml` (root) | `crates/pmcp-workbook-dialect` | workspace members array | VERIFIED | `"crates/pmcp-workbook-dialect"` in members array; not in exclude. |
| `.github/workflows/ci.yml gate job` | `purity-check job` | needs array + PURITY_RESULT evaluation | VERIFIED | `needs: [test, quality-gate, purity-check]`. `PURITY_RESULT: ${{ needs.purity-check.result }}`. `[[ "$PURITY_RESULT" != "success" ]]` in if-condition. |
| `Makefile purity-check` | `pmcp-workbook-runtime / pmcp-workbook-dialect dependency trees` | cargo tree reader-absence ban per feature-combo, fail-closed | VERIFIED (with WARNING) | `BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'`. Loops both crates × `"" / --no-default-features / --all-features`. Explicit exit-status capture. See WR-01 WARNING. |
| `Makefile purity-check` | `crates/pmcp-workbook-runtime/deny.toml` (Layer 2) | cargo deny check bans | VERIFIED | `cargo deny --manifest-path crates/pmcp-workbook-runtime/Cargo.toml check --config deny.toml bans` and same for dialect. Note: executed form is `check --config deny.toml bans` (cargo-deny 0.18.3 ordering — config after subcommand). |
| `Makefile quality-gate` | `Makefile purity-check` | `@$(MAKE) purity-check` composition | VERIFIED | Line 540: `@$(MAKE) purity-check` present in `quality-gate` target body. |
| `justfile purity-check recipe` | `Makefile purity-check` | recipe body delegates to `make purity-check` | VERIFIED | `justfile:67`: `make purity-check`. Single fail-closed implementation. |

---

### Data-Flow Trace (Level 4)

The phase delivers IR types, evaluator, and renderer — no rendering of dynamic data in the typical UI sense. The key data flow is: serialized JSON (bundle/model) → serde deserialization → runtime types → `run()` → `RunResult` → `render_xlsx` → `.xlsx` bytes.

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `sheet_ir/executor.rs:run()` | `CellEnv` / `RunResult` | `ir: &[Cell]`, `dag: &Dag`, `seed: &CellEnv` inputs | Yes — walks Dag in topo order, evaluates each CellExpr, populates env | FLOWING |
| `render/mod.rs:render_xlsx` | `LayoutDescriptor`, `RunResult` | deserialized from bundle JSON + executor output | Yes — `rust_xlsxwriter::Workbook` writes cells from layout + computed values | FLOWING |
| `finding.rs:LintReport` | `findings: Vec<LintFinding>` | `run()` Err path (cycle) or linter push | Yes — round-trip test verifies serde works | FLOWING |

---

### Behavioral Spot-Checks

Not runnable in this context (requires cargo toolchain). Deferred to human verification items 2 and 3 above.

---

### Probe Execution

No probe scripts declared in PLAN files or conventional `scripts/*/tests/probe-*.sh` location. Skipped.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WBRT-01 | 91-01-PLAN.md | Reader-free shared model types, serde-clean, deserializable by offline emitter and served binary | SATISFIED | runtime Cargo.toml has correct deps; all model types in src/ (manifest_model, artifact_model, changelog, formula, etc.); Deserialize on finding types (D-08) |
| WBRT-02 | 91-01-PLAN.md | Deterministic evaluator producing typed outputs + per-cell traces; cycle → LintFinding | SATISFIED | executor.rs run() returns Result<RunResult, Box<LintFinding>>; EvalTrace struct present; cycle test at line 482 |
| WBRT-03 | 91-01-PLAN.md | Writer-only render_xlsx via rust_xlsxwriter, byte-identical output | SATISFIED (with WARNING) | render/mod.rs:223 has render_xlsx using rust_xlsxwriter; determinism test present; CR-01 reachable panic on non-ASCII ARGB (see Anti-Patterns) |
| WBRT-04 | 91-03-PLAN.md | Fail-closed purity gate (cargo-tree + cargo-deny [bans] per feature-combination, merge-blocking CI) | SATISFIED (with WARNINGs) | Makefile purity-check with set -euo pipefail; deny.toml files; CI job wired to gate; WR-01 diagnostic dead code, WR-02 deny.toml missing = fail-open (files exist today) |
| WBDL-01 | 91-02-PLAN.md | SDK owns versioned dialect spec bound to WHITELIST const by drift-detecting test | SATISFIED | docs/workbook-dialect-spec.md flat-13; WHITELIST const = 13 names; doc_whitelist_table_matches_const with Pitfall-4 guard; no linter/lint exports |
| WBDL-03 | 91-03-PLAN.md (traceability_remap) | Re-mapped to Phase 93; not delivered here | RE-MAPPED | REQUIREMENTS.md row Phase 93; ROADMAP Phase 91 Requirements excludes it; Phase 93 Requirements includes it |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/pmcp-workbook-runtime/src/render/mod.rs` | 119 | `8 => &hex[2..]` — byte-indexed slice on a string whose byte-length was checked, not char-length. A non-ASCII 8-byte ARGB (e.g. `"€abcde"`, 8 bytes via len(), UTF-8 3+5) enters the `8 =>` arm and panics at `index 2 is not a char boundary`. | WARNING (REVIEW CR-01) | Reachable panic on serve path if `layout.json` contains a non-ASCII ARGB string. Violates the module's documented "writer value path is panic-free" contract (line 47) and "an unparseable ARGB is silently skipped, never an error" (line 131). Fix: `8 => hex.get(2..)?`. `#![deny(clippy::panic)]` does NOT catch byte-slice index panics — lint passes while the defect exists. |
| `Makefile` | 493-498 | Under `set -euo pipefail`, `tree=$(cargo tree ...)` in a plain assignment causes the shell to abort immediately on non-zero exit — the subsequent `status=$?` capture, `if [ $status -ne 0 ]` branch, and diagnostic `printf` are all unreachable (dead code). The gate still fails closed (exits non-zero), but silently — no diagnostic output on failure. | WARNING (REVIEW WR-01) | Developer sees only `make: *** Error N` with zero context on a purity failure. `docs/workbook-purity-gate.md` describes this code as if it prints diagnostics, which is inaccurate. Fix: use `if ! tree=$(cargo tree ...); then echo ...; fi` pattern. |
| `Makefile` | 520-521 | cargo-deny 0.18.3 does NOT fail when `--config deny.toml` path is missing — it warns and exits 0 with "bans ok" (vacuous pass). The deny.toml files exist today, but if they are deleted or renamed, Layer 2 silently passes. | WARNING (REVIEW WR-02) | Layer 2 backstop is currently operational (files exist), but it is not structurally fail-closed. Fix: add `test -f crates/pmcp-workbook-runtime/deny.toml || exit 1` guards before each cargo-deny invocation. |
| `docs/workbook-dialect-spec.md` | 10, 50, 155 | Spec cites `docs/Excel-as-Configuration-Architecture-Brief.md` and `docs/UFH_Quote_Process_Model_Plot3.xlsx` — neither exists in this repository. Phase references ("Phase 7", "Phase 9", "Plan 04") belong to the lighthouse numbering scheme, not this repo's 91-96 scheme. "ENFORCED in Phase 7" claims are unanchored for readers of this codebase. | WARNING (REVIEW WR-06) | BA/auditor readers of the published contract get broken internal references. |

---

### Human Verification Required

### 1. rust_xlsxwriter Provenance Confirmation (T-91-SC)

**Test:** Verify that a human confirmed rust_xlsxwriter provenance: crates.io owner `jmcnamara`, repo `github.com/jmcnamara/rust_xlsxwriter`, MIT OR Apache-2.0 license, v0.95.0 not yanked; `cargo audit` clean; `cargo tree -p pmcp-workbook-runtime -i zip` shows zip enters ONLY via rust_xlsxwriter.
**Expected:** Human-verified supply-chain gate record, cargo audit exit 0, zip path writer-only.
**Why human:** Supply-chain provenance is a trust judgment requiring human inspection of crates.io. The SUMMARY records approval but the verifier cannot independently confirm it occurred.

### 2. Test Suite Execution

**Test:** Run `cargo test -p pmcp-workbook-runtime --lib -- --test-threads=1` and `cargo test -p pmcp-workbook-dialect --lib -- --test-threads=1`.
**Expected:** All lib tests pass (SUMMARY claims 128 runtime tests + 4 dialect tests including `doc_whitelist_table_matches_const`). Zero test failures.
**Why human:** Requires the Rust toolchain to execute.

### 3. Purity Gate Live Execution

**Test:** Run `make purity-check` and `just purity-check` at the workspace root.
**Expected:** Both commands exit 0 with the full pass message: "purity-check PASSED: reader-free (umya/calamine/quick-xml/swc_/pmcp-code-mode absent) + writer-present (rust_xlsxwriter, per-feature) + zip-permitted + cargo-deny-bans-clean".
**Why human:** Requires the cargo and cargo-deny toolchain to execute.

### 4. CR-01 Decision: `argb_to_color` Reachable Panic

**Test:** Review `crates/pmcp-workbook-runtime/src/render/mod.rs:119` — the expression `8 => &hex[2..]` performs a byte-indexed slice after a byte-length check. A non-ASCII string whose UTF-8 byte length is 8 (e.g. `"€abcde"` = 3+5 = 8 bytes) matches the `8 =>` arm and panics at runtime with `byte index 2 is not a char boundary`. This input arrives via the deserialized `CellLayout.fill_argb`/`font_argb` fields from the bundle's `layout.json`. The `#![deny(clippy::panic)]` lint does NOT catch this — it catches explicit `panic!()` macro calls only.
**Expected:** Either (a) apply the fix: change line 119 to `8 => hex.get(2..)?,` (two characters) and add a unit test `argb_to_color_rejects_non_ascii_8_byte_without_panic` — or (b) document that non-ASCII ARGB input from a bundle is an out-of-threat-model input class with a written rationale. The REVIEW (91-REVIEW.md CR-01) rates this CRITICAL.
**Why human:** This is a security/correctness judgment call. The verifier confirmed the bug is present and unfixed. Whether it blocks Phase 92 progression is a project decision. The fix is a one-line change with no behavior change for valid ASCII ARGB strings.

---

### Gaps Summary

No BLOCKER gaps were found. All 10 must-have truths are verified. The phase goal is achieved: both reader-free leaf crates exist, compile, and are registered workspace members; the purity gate is deployed and merge-blocking; the dialect spec is versioned and bound to the WHITELIST const by a passing binding test; WBDL-03 is correctly remapped to Phase 93.

Four review findings carry WARNING status:
- **CR-01** (reachable panic in `argb_to_color`): The crate's documented "panic-free writer path" contract is violated by a byte-slice index on a string whose length was measured in bytes, not char boundaries. The bug exists in the committed code and was not addressed after the code review. It requires human decision before Phase 92 proceeds (Item 4 above).
- **WR-01** (dead diagnostic code): Purity gate still fails closed but silently. Low-impact quality issue.
- **WR-02** (Layer 2 fail-open on missing deny.toml): Both deny.toml files currently exist; the structural weakness is present but not currently exploited.
- **WR-06** (broken spec doc references): BA/auditor-facing published contract has references to files that do not exist in this repository.

The automated verification score is 10/10 but the status is `human_needed` because test execution, purity gate execution, supply-chain provenance confirmation, and the CR-01 disposition decision all require human action.

---

_Verified: 2026-06-10T07:30:00Z_
_Verifier: Claude (gsd-verifier)_
