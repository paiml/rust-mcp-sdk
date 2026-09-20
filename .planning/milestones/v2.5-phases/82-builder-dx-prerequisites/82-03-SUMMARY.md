---
phase: 82-builder-dx-prerequisites
plan: 03
subsystem: planning-artifacts + book
tags:
  - documentation
  - requirements-update
  - roadmap-update
  - book-chapter
  - reconciliation
requirements:
  - BLDR-03
  - BLDR-04
dependency_graph:
  requires:
    - "Wave 1 (Plan 82-01) — already partially updated REQUIREMENTS.md and ROADMAP.md"
  provides:
    - "REQUIREMENTS.md BLDR-03 reworded with get_tool accessor symmetry per D-08"
    - "REQUIREMENTS.md BLDR-04 enriched with spike-004 framing per D-06"
    - "ROADMAP.md Phase 82 SC4 amended to hand off version bump to release workflow"
    - "ch15-testing.md ## Handler-Level Testing Pattern (In-Process) section with D-03 callout + Tasks cross-link"
  affects:
    - "Phase 83+ toolkit authors who follow the documented handler-level testing pattern"
    - "v2.2.x release branch (now explicitly named as owner of version-bump bullet)"
tech_stack:
  added: []
  patterns:
    - "Reconciliation execution (Wave 1 partially landed targeted files; this plan closed only the remaining gaps without duplicating)"
key_files:
  created:
    - .planning/phases/82-builder-dx-prerequisites/82-03-SUMMARY.md
  modified:
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - pmcp-book/src/ch15-testing.md
key_decisions:
  - "Reconciliation, not duplication: Wave 1 had already (a) added BLDR-04 marked [x] Complete, (b) added the BLDR-04 traceability row, (c) updated the ROADMAP Phase 82 Requirements line to all 4 IDs, (d) added ROADMAP Success Criteria bullets 5 and 6, (e) made the Plans line concrete. This plan closed only what remained: BLDR-03 reword (D-08), BLDR-04 enrichment with spike-004 framing (full D-06 text), ROADMAP SC4 amendment to cite Release & Publish Workflow, and the book section."
  - "BLDR-04 traceability row left as 'Complete' (Wave 1 set it) — the plan's must_have allowed 'Pending or Complete'; Complete reflects that Plan 82-01 already shipped the code."
  - "Doctest example in ch15-testing.md tagged `rust,no_run` (not `rust`) — consistent with the chapter's other code samples that demonstrate API surface without executing during book-doctest runs."
metrics:
  duration_minutes: ~25
  tasks_completed: 3
  commits_created: 3
  files_modified: 3
  test_pass_count: "3 successive `make quality-gate` runs all pass (ALL TOYOTA WAY QUALITY CHECKS PASSED); mdbook build clean"
completed: 2026-05-17
---

# Phase 82 Plan 03: Documentation Reconciliation Summary

Closes the requirements-spec gap (BLDR-04 umbrella requirement + BLDR-03 reword), amends ROADMAP Phase 82 Success Criterion 4 so the version-bump bullet is no longer unowned, and adds the third documentation surface — a handler-level testing pattern section under `ch15-testing.md` — that pairs with the doctests from Plan 01 and the integration test from Plan 02.

## Objective Recap

After this plan, every locked decision in CONTEXT.md has either landing code (Plan 01), landing tests (Plan 02), or landing docs (this plan), and every Phase 82 success criterion has an owning task or an explicit release-workflow handoff.

## Reconciliation Map (Wave 1 vs This Plan)

| Must-have | Wave 1 (Plan 82-01) | This Plan (82-03) | Final State |
|---|---|---|---|
| `REQUIREMENTS.md` BLDR-04 umbrella requirement covering 4 extra `_arc` methods | Added (marked `[x]` Complete; bare 1-sentence form) | Enriched with the spike-004 "API surface discovery 1" framing + 20-line shim sentence per CONTEXT.md D-06 full text | Full umbrella text, `[x]` Complete |
| `REQUIREMENTS.md` BLDR-03 explicitly mentions `Server::get_tool(name) accessor symmetric with get_prompt` | Touched (added "Includes `Server::get_tool(name)` accessor symmetric with `Server::get_prompt`" suffix) | Replaced with the plan-specified reword per D-08 (single-option committed wording, drops the dual-OR phrasing, names the doctests and `tests/in_process_handler_pattern.rs` reference test, cites the rejected `Server::dispatch`/`MemoryTransport` alternative) | Full reword with explicit accessor + doctest + integration test references |
| `REQUIREMENTS.md` BLDR-04 traceability row | Added (set to `Complete` since Plan 01 shipped the code) | Left untouched | `\| BLDR-04 \| Phase 82 \| Complete \|` — plan allowed "Pending or Complete"; Complete reflects reality |
| `ROADMAP.md` Phase 82 Requirements line lists all 4 IDs | Done | Left untouched | `**Requirements**: BLDR-01, BLDR-02, BLDR-03, BLDR-04` |
| `ROADMAP.md` Phase 82 Success Criteria bullets 5 (`_arc` parity) and 6 (`get_tool` accessor) | Done | Left untouched | 6 numbered bullets present |
| `ROADMAP.md` Phase 82 Plans line is concrete (no longer TBD) | Done | Left untouched | Three-plan enumeration with status checkboxes |
| `ROADMAP.md` Phase 82 SC4 amended to cite Release & Publish Workflow (closes review item 7) | Not done | Amended per plan-specified text — bullet 4 now cites `CLAUDE.md` §"Release & Publish Workflow" and clarifies the bump itself happens in the v2.2.x release branch, not in Phase 82 implementation plans | Bullet 4 no longer unowned |
| `pmcp-book/src/ch15-testing.md` new section with D-03 callout + Tasks cross-link | Not done | Added: `## Handler-Level Testing Pattern (In-Process)` between Testing Philosophy and Official MCP Inspector | 87-line section with runnable example, D-03 "What this pattern skips" callout, production-vs-testing-shortcut distinction, references to `tests/in_process_handler_pattern.rs` and `ch12-7-tasks.md` |

## Exact Edits

### `.planning/REQUIREMENTS.md` (commit `122e2080`)

Line 122 — BLDR-03 reword (replaces single line):

> `- [ ] **BLDR-03**: Officially documented handler-level testing pattern so external toolkit integration tests can drive request flow without poking at private \`Server::handle_request\` — delivered via \`Server::get_tool(name)\` accessor symmetric with \`get_prompt\`, plus a comprehensive doctest on each accessor and a reference integration test under \`tests/in_process_handler_pattern.rs\` (rejecting the \`Server::dispatch\`/\`MemoryTransport\` alternative as out-of-scope for this phase per CONTEXT.md D-01).`

Line 123 — BLDR-04 enriched (replaces Wave 1's bare form with the full plan text):

> `- [x] **BLDR-04**: \`pmcp::ServerBuilder\` gains \`_arc\` variants for the remaining four handler types (\`resources_arc\`, \`sampling_arc\`, \`auth_provider_arc\`, \`tool_authorizer_arc\`) so all impl-or-Arc handler-registration paths reach parity with \`ServerCoreBuilder\`. This closes the umbrella Arc-symmetry gap surfaced in spike 004's "API surface discovery 1" — no external toolkit author should need a 20-line delegating wrapper shim for any handler type.`

Diff scope: exactly 2 lines changed (BLDR-03 and BLDR-04 statements); no other lines touched. BLDR-01/-02 wording byte-identical pre/post; traceability table untouched; milestone-totals line untouched.

### `.planning/ROADMAP.md` (commit `8a99ac28`)

Line 1246 — Phase 82 Success Criterion 4 amended:

Before:
> `  4. The new builder methods are additive (no existing builder method signatures change) and ship in a minor \`pmcp\` version bump`

After:
> `  4. The new builder methods are additive (no existing builder method signatures change) and ship as part of a minor \`pmcp\` version bump — the actual \`Cargo.toml\` version change and \`CHANGELOG.md\` entry are produced by the v2.2.x release branch per \`CLAUDE.md\` §"Release & Publish Workflow", NOT by Phase 82's implementation plans. (Phase 82 closes when its three plans land; the release that ships them is tagged separately.)`

Diff scope: single hunk at line 1246, strictly within the Phase 82 block. Phase 81 and Phase 83 blocks byte-identical pre/post.

### `pmcp-book/src/ch15-testing.md` (commit `13834d45`)

Insertion point: between the existing `## Testing Philosophy` section (which ends at the testing-pyramid code fence at line 46) and the existing `## Official MCP Inspector` section. New section starts at line 48.

New section heading: `## Handler-Level Testing Pattern (In-Process)`

Section structure (87 lines total):

1. Two-paragraph intro explaining the pattern (`get_tool` / `get_prompt` accessors, the `tool_arc` / `prompt_arc` Arc-sharing path).
2. `### Example` — runnable `rust,no_run` code block demonstrating the full lifecycle:
   - Define an `EchoTool` implementing `ToolHandler`.
   - Build a `Server` with `tool_arc("echo", Arc::clone(&handler))`.
   - Retrieve via `server.get_tool("echo")`.
   - Invoke via `tool.handle(json!({...}), RequestHandlerExtra::default()).await`.
   - Assert on the returned `Value`.
   - Notes the symmetric `prompt_arc` / `get_prompt` shape and points readers at `tests/in_process_handler_pattern.rs`.
3. `### What this pattern skips` — the D-03 callout naming `auth_provider`, `tool_authorizer`, `tool_middleware` as bypassed by the handler-level path, and explicitly stating that in production, real MCP clients hit `Server::handle_request` over a transport (`stdio`, `streamable-http`, `websocket`, `SSE`) and the full pipeline runs; the handler-level pattern is a deliberate testing shortcut.
4. `### When to use a full transport instead` — points readers at `pmcp::Client` over `stdio` (`Server::run_stdio()` + `StdioTransport`) or `streamable-http` (`pmcp::StreamableHttpServer` + `pmcp::Client::connect_streamable_http(...)`).
5. `### Pattern summary` — 5-row table mapping each step (register, build, retrieve, invoke, assert) to its code snippet.
6. `> **See also:** [Long-Running Operations (Tasks)](./ch12-7-tasks.md)` cross-link blockquote — closes cross-AI review item 12.

Five literal D-03 tokens present in section body (verified via stateful awk extraction): `auth_provider`, `tool_authorizer`, `tool_middleware`, `pmcp::Client`, `stdio` (and `streamable-http` also present — 7 matches total in the body extraction). Tasks cross-link `ch12-7-tasks.md` present.

`SUMMARY.md` is unchanged — this is a same-file section addition, not a new chapter.

## CONTEXT.md Decision Confirmations

- **D-06 (BLDR-04 umbrella requirement covering all four extra `_arc` lifts):** confirmed — `grep -E 'BLDR-04' .planning/REQUIREMENTS.md | grep -cE 'resources_arc.*sampling_arc.*auth_provider_arc.*tool_authorizer_arc'` returns 1 (all four method names appear on a single line in order).
- **D-08 (BLDR-03 reword to mention `Server::get_tool` accessor symmetric with `get_prompt`):** confirmed — the reworded BLDR-03 text contains the literal sequence `Server::get_tool(name)` + backtick + ` accessor symmetric with ` + backtick + `get_prompt`.
- **Closes cross-AI review item 7 (unowned version-bump criterion):** confirmed — `awk '/^### Phase 82:/,/^### Phase 83:/' .planning/ROADMAP.md | grep -c 'Release & Publish Workflow'` returns 1.
- **Closes cross-AI review item 12 (Tasks-chapter cross-link in book section):** confirmed — the new section body contains `[Long-Running Operations (Tasks)](./ch12-7-tasks.md)`.
- **D-02 part (c) (book section is the third documentation surface):** confirmed — the new `## Handler-Level Testing Pattern (In-Process)` section exists with all D-03 tokens, references both `tool_arc` and `prompt_arc`, references `tests/in_process_handler_pattern.rs`, and explicitly contrasts production vs testing shortcut.

## Requirement IDs Closed by This Plan

- **BLDR-03** (closed via documentation surface — book-section portion of D-02 part (c) lands here; doctest portion landed in Plan 01; integration-test portion landed in Plan 02). Wording also reworded per D-08. Traceability row remains `Pending` until Phase 82's final close-out (Plan 02 + 03 both wrap up the BLDR-03 surface).
- **BLDR-04** (full requirement-spec creation per D-06 — Wave 1 added the bare form, this plan enriched it with the full spike-004 framing; the code itself shipped in Plan 01 and Wave 1's traceability row already shows `Complete`).

## Verification Results

| Step | Command | Result |
|------|---------|--------|
| BLDR-04 entry present | `grep -E '^\- \[[ x]\] \*\*BLDR-04\*\*:' .planning/REQUIREMENTS.md` | exit 0 (1 match) |
| BLDR-04 has all 4 method names in order | `grep -E 'BLDR-04' .planning/REQUIREMENTS.md \| grep -cE 'resources_arc.*sampling_arc.*auth_provider_arc.*tool_authorizer_arc'` | 1 |
| BLDR-03 names get_tool accessor symmetry | `grep -F 'Server::get_tool(name)\` accessor symmetric with \`get_prompt' .planning/REQUIREMENTS.md` | 1 match |
| BLDR-04 traceability row | `grep -c '^\| BLDR-04 \| Phase 82 \| ' .planning/REQUIREMENTS.md` | 1 |
| ROADMAP Requirements line | `grep -cE '^\*\*Requirements\*\*: BLDR-01, BLDR-02, BLDR-03, BLDR-04$' .planning/ROADMAP.md` | 1 |
| Phase 82 numbered success-criteria bullets | `awk '/^### Phase 82:/,/^### Phase 83:/' .planning/ROADMAP.md \| grep -cE '^\s+[1-6]\.'` | 6 |
| Phase 82 all six method names in order | `awk … \| grep -cE 'tool_arc.*prompt_arc.*resources_arc.*sampling_arc.*auth_provider_arc.*tool_authorizer_arc'` | 1 |
| Phase 82 `get_tool` signature substring | `awk … \| grep -cF 'get_tool(name) -> Option<&Arc<dyn ToolHandler>>'` | 1 |
| Phase 82 cites Release & Publish Workflow | `awk … \| grep -c 'Release & Publish Workflow'` | 1 |
| New section heading | `grep -c '^## Handler-Level Testing Pattern (In-Process)$' pmcp-book/src/ch15-testing.md` | 1 |
| D-03 tokens in section body (≥5) | stateful awk + `grep -cE 'auth_provider\|tool_authorizer\|tool_middleware\|pmcp::Client\|stdio\|streamable-http'` | 7 |
| `tool_arc` in section body | stateful awk + `grep -c 'tool_arc'` | 5 |
| `prompt_arc` in section body | stateful awk + `grep -c 'prompt_arc'` | 4 |
| "production" in section body | stateful awk + `grep -c 'production'` | 1 |
| "testing shortcut" in section body | stateful awk + `grep -c 'testing shortcut'` | 1 |
| `tests/in_process_handler_pattern.rs` reference | stateful awk + `grep -c 'tests/in_process_handler_pattern.rs'` | 1 |
| Tasks cross-link present | stateful awk + `grep -c 'ch12-7-tasks\.md'` | 1 |
| mdbook build | `cd pmcp-book && mdbook build` | exit 0 — clean |
| **Quality gate** (Task 1) | `make quality-gate` | exit 0 — ALL TOYOTA WAY QUALITY CHECKS PASSED |
| **Quality gate** (Task 2) | `make quality-gate` | exit 0 — ALL TOYOTA WAY QUALITY CHECKS PASSED |
| **Quality gate** (Task 3) | `make quality-gate` | exit 0 — ALL TOYOTA WAY QUALITY CHECKS PASSED |
| Targeted diffs only | `git diff -U0 .planning/ROADMAP.md \| grep '^@@'` | single hunk at line 1246; `pmcp-book/src/SUMMARY.md` unchanged |

## Commits

| # | Hash | Subject |
|---|------|---------|
| 1 | `122e2080` | `task(82-03-01): reword BLDR-03 with get_tool accessor, enrich BLDR-04 with spike-004 framing` |
| 2 | `8a99ac28` | `task(82-03-02): amend ROADMAP Phase 82 SC4 to hand off version bump to release workflow` |
| 3 | `13834d45` | `task(82-03-03): add handler-level testing pattern section to ch15-testing.md` |

## Deviations from Plan

### Reconciliation deviations (Wave 1 had already done part of the plan's work)

The plan was authored before Wave 1 (Plan 82-01) ran, but by the time this Wave 2 plan executed, Wave 1 had already partially landed several of the must_have truths:

| Plan Task | Already Done by Wave 1 | Newly Done This Plan |
|---|---|---|
| Task 1 (REQUIREMENTS.md — 3 edits) | (a) BLDR-04 inserted (bare 1-sentence form, marked `[x]`); (b) BLDR-03 suffix added (`Includes \`Server::get_tool(name)\` accessor symmetric with \`Server::get_prompt\`.`); (c) BLDR-04 traceability row added (set to `Complete`) | (a) BLDR-03 fully reworded per plan-specified text (single-committed-option phrasing per D-08); (b) BLDR-04 enriched with spike-004 framing + 20-line shim sentence per full D-06 text |
| Task 2 (ROADMAP.md — 4 edits) | (1) Requirements line updated to 4 IDs; (3) bullets 5 and 6 added; (4) Plans line made concrete | (2) **SC4 amended to cite Release & Publish Workflow** (this was the only Task 2 edit Wave 1 had not done — closes the unowned-criterion gap from cross-AI review item 7) |
| Task 3 (ch15-testing.md — new section) | Nothing | Full section added (87 lines): heading + intro + example + D-03 callout + production-vs-testing distinction + transport guidance + summary table + Tasks cross-link |

No work was duplicated. The plan's reconciliation contract — "follow the must_haves, not the literal task wording" — was honored throughout.

### Documentation-only nuance

- **BLDR-04 traceability row status:** the plan's literal task 1 step 3 said "insert `| BLDR-04 | Phase 82 | Pending |`", but the plan's `read_first` and `must_have` text both explicitly allow "Pending or Complete depending on must_have wording — follow the plan". Wave 1 had set it to `Complete` because Plan 82-01 actually shipped the code; that's a more accurate reflection of reality and was left untouched.
- **Doctest fence:** the ch15-testing.md example uses `rust,no_run` rather than bare `rust`. This is consistent with the chapter's other API-surface code samples that demonstrate shape without executing during book-doctest runs (the example builds a server and would require an async runtime to actually invoke). mdbook build still validates the code's syntactic correctness.

### Auto-fixed Issues

None.

### Auth gates / human-action checkpoints

None.

## Threat Flags

None. This plan touches three documentation files; no new runtime code, no new public API surface, no new auth or trust-boundary changes. The plan's `<threat_model>` register entry T-82-05 (informational tampering — readers drawing incorrect security conclusions from incomplete documentation) is mitigated by the literal D-03 token-set callout in both the doctests (Plan 01) and the new book section (this plan), with traceability between REQUIREMENTS.md BLDR-03 wording and the doctest/book-section text preventing silent drift.

## Self-Check: PASSED

- [x] `.planning/REQUIREMENTS.md` modified (file exists; contains reworded BLDR-03 and enriched BLDR-04).
- [x] `.planning/ROADMAP.md` modified (file exists; Phase 82 SC4 cites `Release & Publish Workflow`).
- [x] `pmcp-book/src/ch15-testing.md` modified (file exists; new `## Handler-Level Testing Pattern (In-Process)` section at line 48).
- [x] Commit `122e2080` exists in `git log` (task 01).
- [x] Commit `8a99ac28` exists in `git log` (task 02).
- [x] Commit `13834d45` exists in `git log` (task 03).
- [x] `make quality-gate` ran exit 0 with `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner after each task.
- [x] `mdbook build` ran exit 0 with no warnings about the new section.
- [x] `SUMMARY.md` unchanged (no new chapter, just a same-file section addition).
- [x] No files outside the plan's three-file scope were modified by this plan.
