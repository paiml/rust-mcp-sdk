---
phase: 69
plan: 01
subsystem: research / gsd
tags: [rmcp-parity, ergonomics, research, gap-matrix]
dependency_graph:
  requires:
    - 69-CONTEXT.md (locked decisions D-01..D-19)
  provides:
    - 69-RESEARCH.md (evidence-backed gap matrix with stable Row IDs)
  affects:
    - 69-02-PLAN.md (derives follow-on phase proposals from High-severity rows)
    - 69-03-PLAN.md (quality gate: High-row-ID bijection with proposal citations)
tech_stack:
  added: []  # docs-only phase
  patterns:
    - "Stable <SURFACE>-NN Row IDs for cross-document reference"
    - "GitHub blob URLs with #L<N> for rmcp source citations (≥80% bar)"
    - "pmcp baseline tags ([v2.3.0] / [main]) for line-number reproducibility"
key_files:
  created:
    - .planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-RESEARCH.md
    - .planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-01-SUMMARY.md
  modified: []
decisions:
  - "Pinned rmcp to 1.5.0 (latest stable on crates.io as of 2026-04-16, tag rmcp-v1.5.0)"
  - "Pinned pmcp baseline to v2.3.0 + feat/sql-code-mode at commit dbaee6cc"
  - "Substituted canonical source paths for 4 files not found at HEAD (see RESEARCH.md Source file decisions)"
  - "4 High-severity gaps identified — concentrated on Handler Signatures and Client-Side Ergonomics"
metrics:
  duration: "~90 min"
  completed: 2026-04-16
---

# Phase 69 Plan 01: rmcp Parity Research — Gap Matrix Summary

Produced `69-RESEARCH.md`, a 32-row evidence-backed ergonomics gap matrix comparing pmcp v2.3.0 (+ `feat/sql-code-mode` main) against rmcp 1.5.0 across six surfaces, with 4 High-severity gaps surfaced for Plan 02 consumption.

## Baselines Pinned

| Dimension | Value |
|-----------|-------|
| rmcp crates.io version | 1.5.0 |
| rmcp git tag | https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v1.5.0 |
| rmcp source tree path | `crates/rmcp/src/...` + `crates/rmcp-macros/src/...` |
| pmcp baseline | v2.3.0 + `feat/sql-code-mode` @ commit `dbaee6cc` |
| Research date | 2026-04-16 |

## Row Counts Per Surface

| Surface | Row-ID prefix | Rows | Minimum required |
|---------|---------------|------|------------------|
| Tool Macros | MACRO | 7 | 5 |
| Builder APIs | BUILDER | 5 | 4 |
| Typed Tool / Prompt Wrappers | TYPED | 5 | 4 |
| Handler Signatures + State / Extra Injection | HANDLER | 5 | 4 |
| Client-Side Ergonomics | CLIENT | 5 | 4 |
| Error Types + Result Wrappers | ERR | 5 | 4 |
| **Total** | — | **32** | **25** |

All minimums met or exceeded.

## Severity Distribution

| Severity | Count |
|----------|-------|
| High | 4 |
| Medium | 7 |
| Low (incl. Parity + Strength-to-preserve) | 21 |

## High-Severity Row IDs (Plan 02 Proposal Seeds)

- **MACRO-02** — Tool description should fall back to rustdoc when the `description` attribute is omitted (rmcp already does this; pmcp requires explicit string).
- **HANDLER-02** — Add a request-scoped `Extensions` typemap to `RequestHandlerExtra` (rmcp's `RequestContext` has one; pmcp does not).
- **HANDLER-05** — Add a peer handle to `RequestHandlerExtra` so handlers can initiate server-to-client RPCs (sampling, list_roots, progress) from inside the handler body.
- **CLIENT-02** — Add typed `call_tool_typed<T: Serialize>` convenience and `list_all_tools`/`list_all_prompts`/`list_all_resources` auto-pagination helpers on `Client`.

## Citation Quality Metrics

| Metric | Target | Actual |
|--------|--------|--------|
| rmcp GitHub blob URL ratio (vs docs.rs) | ≥80% | 100% |
| pmcp citations ending with baseline tag (`[v2.3.0]` / `[main]`) | ≥80% | 97% (67/69) |
| Unique Row IDs across document | 100% | 100% (32/32) |

## Source File Substitutions Recorded

Four canonical source paths named in the plan's `<read_first>` did not exist at HEAD `dbaee6cc`; substitutes were recorded in the `### Source file decisions` subsection of the document header:

1. `src/client/core.rs` → `src/client/mod.rs` (client module is flat)
2. `examples/09_client_with_tool.rs` → `examples/c02_client_tools.rs` (Phase 65 role-prefix convention)
3. `src/error.rs` → `src/error/mod.rs` (error is a directory module)
4. `src/types/error.rs` → not present; error lives only in `src/error/mod.rs`

## Deviations from Plan

None — the plan executed exactly as written. Three sequential tasks (skeleton → populate → summary + methodology) were each committed individually. Task 2 min-row-count verification passed on first attempt; no auto-fixes required.

## Handoff to Plan 02

**4 High-severity gaps identified — Row IDs: MACRO-02, HANDLER-02, HANDLER-05, CLIENT-02; see `69-RESEARCH.md` §"Tool Macros", §"Handler Signatures + State / Extra Injection", and §"Client-Side Ergonomics" rows for evidence.**

Plan 02 should produce one follow-on phase proposal per High-severity Row ID, citing the Row ID verbatim in each proposal's `### Rationale`. Plan 03's quality gate will check bijection between High Row IDs and proposal citations.

## Self-Check: PASSED

**Files verified:**
- FOUND: `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-RESEARCH.md`
- FOUND: `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-01-SUMMARY.md`

**Commits verified:**
- FOUND: `f37d4b02` (Task 1 — skeleton + baselines pinned)
- FOUND: `618d5e2a` (Task 2 — matrix populated)
- FOUND: `b5ab906f` (Task 3 — executive summary + methodology notes)
