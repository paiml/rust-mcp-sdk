---
phase: 95-shape-a-binary-pmcp-workbook-server
plan: 02
subsystem: pmcp-workbook-server
tags: [shape-a, binary, workbook, tests, parity, purity-gate]
requires:
  - pmcp-workbook-server crate (lib + bin) from plan 95-01
  - mcp-tester (dev-dependency parity harness only)
  - golden fixture crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0
provides:
  - integration test trio (assemble surface, ephemeral-port HTTP initialize smoke, mcp-tester parity through run_serving)
  - property/fuzz coverage of the --bundle-id fail-closed assertion
  - Makefile purity-check assertion for the pmcp-workbook-server served cone (reader-free)
  - CLAUDE.md slot-9a publish-order entry
affects:
  - Makefile (purity-check target)
  - CLAUDE.md (Release & Publish Workflow)
tech-stack:
  added:
    - proptest 1 (dev — bundle_id property test)
    - mcp-tester 0.7.0 (dev — parity harness, never a runtime dep)
    - tempfile 3 / serde_json 1 / url 2.5 (dev — http/parity harness)
  patterns:
    - field-for-field re-skin of pmcp-sql-server's test trio, swapping the SQL seam for the workbook bundle
    - live wire-surface assertion (tools/list) before tool invocation
    - fail-closed cargo-tree negative assertion (no reader/JS deps in served cone)
key-files:
  created:
    - crates/pmcp-workbook-server/tests/assemble.rs
    - crates/pmcp-workbook-server/tests/http_smoke.rs
    - crates/pmcp-workbook-server/tests/parity_workbook.rs
    - crates/pmcp-workbook-server/tests/bundle_id_props.rs
  modified:
    - crates/pmcp-workbook-server/Cargo.toml (dev-dependencies)
    - Makefile (purity-check served-cone assertion)
    - CLAUDE.md (slot-9a publish order)
    - .planning/phases/95-shape-a-binary-pmcp-workbook-server/deferred-items.md
decisions:
  - "in-process five-tool surface asserted via Server::get_tool in assemble.rs; live tools/list wire surface asserted in parity_workbook.rs (Codex MEDIUM #4)"
  - "http_smoke asserts the initialize RESPONSE (server actually listening), not just a bound socket (Gemini #10)"
  - "purity-check BAN list is intentionally broad + fail-closed; false positives are resolved by narrowing, never weakening the gate (Codex MEDIUM #6)"
  - "mcp-tester is [dev-dependencies] only — parity harness, not a published dependency"
metrics:
  duration: ~20m (executor) + orchestrator-authored SUMMARY after executor died pre-SUMMARY
  completed: 2026-06-14
requirements: [WBCL-06]
---

# Phase 95 Plan 02: Test Trio + Property/Fuzz + Purity Gate Summary

Added the full verification layer for the Shape A `pmcp-workbook-server` binary —
the integration test trio mirroring `pmcp-sql-server` (server-surface assemble,
ephemeral-port HTTP initialize smoke, mcp-tester parity through the real
`run_serving` path), the ALWAYS property/fuzz coverage of the binary-owned
`--bundle-id` fail-closed assertion, the reader-free purity gate over the served
cone, and the CLAUDE.md slot-9a publish-order wiring.

## Execution Note

The executor agent implemented and committed all three tasks (commits below) but
died on an API socket error **before** writing SUMMARY.md. The orchestrator merged
the four completed commits, re-ran the full verification suite (all green — see
below), and authored this SUMMARY from the committed work and verified results. No
code was changed by the orchestrator; only this SUMMARY was added. (`SendMessage`
to resume the original agent was unavailable in this harness.)

## What Was Built

### Task 1 — Integration test trio (commit e94db773)
- `tests/assemble.rs` — `golden_bundle_dir()` resolves `tax-calc@1.1.0` from
  `CARGO_MANIFEST_DIR`; builds the server via `build_server` and asserts all five
  workbook tools (`calculate`/`explain`/`get_manifest`/`diff_version`/`render_workbook`)
  via the stable `Server::get_tool` inspection API + the `workbook://` resource.
- `tests/http_smoke.rs` — builds via `build_server`, `serve`s to `127.0.0.1:0`,
  captures the REAL bound addr, drives a full MCP `initialize` round-trip over the
  SDK `StreamableHttpTransport`, and asserts the response id echoes (proving the
  server is actually listening, not merely bound — Gemini #10); `handle.abort()`.
- `tests/parity_workbook.rs` — invokes the real `pmcp_workbook_server::run_serving`
  path against the golden bundle on an ephemeral port, polls `test_initialize` with
  backoff, asserts a live `tools/list` exposes all five tools before invoking
  selected tools via the `mcp-tester` harness; `handle.abort()`. Synthetic golden
  only — zero customer material.
- `Cargo.toml` `[dev-dependencies]`: `mcp-tester` (path, harness-only), `tempfile`,
  `serde_json`, `url`, `proptest`. `mcp-tester` confirmed dev-only (not a runtime dep).

### Task 2 — Property/fuzz of the --bundle-id assertion (commit 34ed57a3)
- `tests/bundle_id_props.rs` — proptest generating arbitrary `bundle_id` strings:
  every non-`"tax-calc"` id yields `RunError::BundleIdMismatch` with **no** server
  constructed (fail-closed invariant); explicit cases assert `Some("tax-calc")` → Ok
  and `None` → Ok. Covers empty/whitespace/unicode/long-string edge cases without
  panic. Targets the one piece of genuinely-new binary-owned logic (the D-01 guard);
  boot-integrity fuzz is already owned by the Phase 91/92 toolkit suites.

### Task 3 — Purity gate + slot-9a wiring (commits 42a508fe, f18a1ae3)
- `Makefile` `purity-check` — a new fail-closed block ("Phase 95 — pmcp-workbook-server
  served cone reader-absence") mirroring the Phase 92 block: `cargo tree -p
  pmcp-workbook-server` captured with `|| status=$?` (never `2>/dev/null`), grepped
  against `BAN='umya|calamine|quick-xml|swc_|pmcp-code-mode'`, failing closed if any
  reader/JS dep enters the served binary tree. A comment documents that the broad BAN
  list is intentional and that false positives are resolved by narrowing, never
  weakening the gate (Codex MEDIUM #6). No `umya` positive assertion (the binary is
  not a writer crate). CI's `make purity-check` picks this up with no workflow edit.
- `CLAUDE.md` — `pmcp-workbook-server` inserted at slot 9a in the Release & Publish
  Workflow numbered list (after `pmcp-server-toolkit`/runtime, sibling to
  `pmcp-sql-server`; deps `pmcp-server-toolkit[workbook,http]` + `pmcp`; no
  inter-dependency with the SQL connector crates).
- `f18a1ae3` — `cargo fmt` normalization of `bundle_id_props.rs` (fmt --all --check).

## Verification (re-run post-merge by orchestrator — all green)

| Check | Result |
|-------|--------|
| `cargo test -p pmcp-workbook-server --test assemble` | passed |
| `cargo test -p pmcp-workbook-server --test http_smoke` | 1 passed (initialize round-trip) |
| `cargo test -p pmcp-workbook-server --test parity_workbook` | 1 passed (live tools/list + invoke via run_serving) |
| `cargo test -p pmcp-workbook-server --test bundle_id_props` | 3 passed (fail-closed property) |
| `cargo test -p pmcp-workbook-server` (lib+doc+integration) | all passed, 0 failed |
| `make purity-check` | PASSED — `pmcp-workbook-server reader-free` line present, exit 0 |
| grep: `pmcp-workbook-server` in Makefile / CLAUDE.md | present in both (Makefile ×6, CLAUDE.md ×1) |
| grep: `mcp-tester` under `[dev-dependencies]` only | confirmed |

## Deviations from Plan

None in the implemented work — all three tasks executed as written. The only
process deviation: SUMMARY.md was authored by the orchestrator (not the executor)
because the executor died on an API socket error after committing all task work but
before the SUMMARY step. Verification was re-run independently before this SUMMARY
was written.

## Out-of-Scope Observations (NOT fixed — SCOPE BOUNDARY)

The two pre-existing `pmcp-server-toolkit` warnings noted in 95-01-SUMMARY
(`code_mode.rs:557` unused import, `http/auth.rs:538` redundant guard) remain; they
are outside this binary-only phase and unrelated to the test/gate work here.

## Known Stubs

None — every test drives the real binary surface (`build_server` / `serve` /
`run_serving`) against the live golden bundle; the purity gate runs against the real
resolved dependency tree.

## Self-Check: PASSED

- FOUND: crates/pmcp-workbook-server/tests/assemble.rs
- FOUND: crates/pmcp-workbook-server/tests/http_smoke.rs
- FOUND: crates/pmcp-workbook-server/tests/parity_workbook.rs
- FOUND: crates/pmcp-workbook-server/tests/bundle_id_props.rs
- FOUND commit 42a508fe (Task 3), 34ed57a3 (Task 2), e94db773 (Task 1)
- VERIFIED: make purity-check exits 0 and reports pmcp-workbook-server reader-free
