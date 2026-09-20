---
phase: 95
slug: shape-a-binary-pmcp-workbook-server
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-15
---

# Phase 95 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Reconstructed retroactively from PLAN/SUMMARY artifacts (State B) and verified
> against the live test suite on 2026-06-15.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in) + `proptest 1` (property) + `make purity-check` (dep-tree gate) |
| **Config file** | `crates/pmcp-workbook-server/Cargo.toml` (`[lib]`/`[[bin]]`/`[dev-dependencies]`); `Makefile` (`purity-check`) |
| **Quick run command** | `cargo test -p pmcp-workbook-server --lib` |
| **Full suite command** | `cargo test -p pmcp-workbook-server && make purity-check` |
| **Estimated runtime** | ~10 seconds (lib < 1s; integration ~1s; doctests ~1s; purity-check cargo-tree ~2s) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-workbook-server --lib`
- **After every plan wave:** Run `cargo test -p pmcp-workbook-server && make purity-check`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** ~10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 95-01-01 | 01 | 1 | WBCL-06 | — | CLI parses `--bundle-dir`/`--bundle-id`/`--http`; `--bundle-id` omitted → `None`; no `env()` overrides (D-03) | unit + doctest | `cargo test -p pmcp-workbook-server --lib` | ✅ `src/cli.rs` | ✅ green |
| 95-01-02 | 01 | 1 | WBCL-06 | — | `build_server` registers all 5 tools + `workbook://`; matching `--bundle-id` → Ok; mismatch → `BundleIdMismatch`; tampered/missing bundle → fail-closed `RunError::Bundle` | unit | `cargo test -p pmcp-workbook-server --lib assemble` | ✅ `src/assemble.rs` | ✅ green |
| 95-01-03 | 01 | 1 | WBCL-06 | — | `run`/`serve`/`run_serving` pipeline; invalid `--http` → `RunError::Addr` (serving never reached); serving-task panic surfaces as `RunError::Serving`; no unconstructible `Io` variant | unit + example + doctest | `cargo test -p pmcp-workbook-server --lib && cargo run -p pmcp-workbook-server --example workbook_server_min` | ✅ `src/lib.rs`, `examples/workbook_server_min.rs` | ✅ green |
| 95-02-01 | 02 | 2 | WBCL-06 | — | Live MCP server stands up from compiled bundle alone: 5-tool assemble surface; HTTP `initialize` round-trip echoes id (actually listening, not just bound); mcp-tester parity drives live `tools/list` + invoke through real `run_serving` | integration | `cargo test -p pmcp-workbook-server --test assemble --test http_smoke --test parity_workbook` | ✅ `tests/assemble.rs`, `tests/http_smoke.rs`, `tests/parity_workbook.rs` | ✅ green |
| 95-02-02 | 02 | 2 | WBCL-06 | — | Fail-closed `--bundle-id` invariant: every non-`tax-calc` id → `BundleIdMismatch` with **no** server constructed; `Some("tax-calc")`/`None` → Ok; edge cases (empty/whitespace/unicode/long) never panic | property/fuzz | `cargo test -p pmcp-workbook-server --test bundle_id_props` | ✅ `tests/bundle_id_props.rs` | ✅ green |
| 95-02-03 | 02 | 2 | WBCL-06 | — | "No user Rust" purity: served dependency cone is reader/JS-free (`umya`/`calamine`/`quick-xml`/`swc_`/`pmcp-code-mode` banned), fail-closed | dep-tree gate | `make purity-check` | ✅ `Makefile` (Phase 95 block) | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.* Rust's built-in
`cargo test`, `proptest` (dev-dependency), and the `make purity-check` Makefile
target were all in place; no test framework needed installing. All test files
listed in the Per-Task Map exist and run green.

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

The ALWAYS requirements (unit, property/fuzz, example, integration) are each
satisfied by an automated command above. The single requirement WBCL-06 ("a
pure-config binary stands up a live MCP server from a compiled bundle alone, no
user Rust") is fully decomposed into automated checks:
- **live server** → `http_smoke` (initialize round-trip) + `parity_workbook` (live `tools/list` + invoke via `run_serving`)
- **from a compiled bundle alone** → `assemble` integration + `workbook_server_min` example (golden `tax-calc@1.1.0`)
- **no user Rust / pure-config** → `make purity-check` reader-free served-cone gate
- **fail-closed boot integrity** → `bundle_id_props` property test + `assemble.rs` inline mismatch/tamper cases

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (none — existing infra sufficient)
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-06-15

---

## Validation Audit 2026-06-15

State B reconstruction from PLAN/SUMMARY artifacts, cross-referenced against the
live test suite (genchi genbutsu — all commands re-run, not trusted from SUMMARY).

| Metric | Count |
|--------|-------|
| Requirements | 1 (WBCL-06) |
| Tasks mapped | 6 (2 plans × 3 tasks) |
| Gaps found | 0 |
| Resolved | 0 (none needed) |
| Escalated | 0 |

**Live verification results (2026-06-15):**

| Check | Result |
|-------|--------|
| `cargo test -p pmcp-workbook-server --lib` | 13 passed, 0 failed |
| `cargo test -p pmcp-workbook-server --test assemble` | 2 passed |
| `cargo test -p pmcp-workbook-server --test bundle_id_props` | 3 passed |
| `cargo test -p pmcp-workbook-server --test http_smoke` | 1 passed |
| `cargo test -p pmcp-workbook-server --test parity_workbook` | 1 passed |
| `cargo test -p pmcp-workbook-server --doc` | 4 passed |
| `cargo run -p pmcp-workbook-server --example workbook_server_min` | exit 0 ("calculate present: true") |
| `make purity-check` (served cone) | reader-free PASS (no banned deps) |

**Total: 24 automated tests green + example + purity gate.** Phase 95 is
Nyquist-compliant — every facet of WBCL-06 has automated verification.
