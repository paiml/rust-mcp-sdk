---
document: addendum
phase: "75"
supersedes_in_part: ["75-01-PLAN.md", "75-02-PLAN.md", "75-03-PLAN.md", "75-04-PLAN.md", "75-CONTEXT.md D-02"]
created: "2026-04-23"
trigger: Wave 0 D-10 spike resolution (D-10-B) + user decision to split phase
---

# Phase 75 Addendum — D-10-B Scope Adjustment

## Background

Wave 0 Plan 75-00 Task 4 empirically verified that **PMAT 3.15.0 ignores `#[allow(clippy::cognitive_complexity)]`** (see `.planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md`). The P5 technique described in `75-CONTEXT.md` D-02 — add `#[allow]` + `// Why:` and continue — **does not suppress PMAT's complexity gate**.

User decision (2026-04-23): split the P5-dependent work into sibling Phase 75.5 rather than absorbing it into Phase 75. See `.planning/phases/75.5-pmat-ex-p5-refactor-backlog/75.5-CONTEXT.md`.

## Rules that override the PLAN.md files

**These rules take precedence over any `#[allow]` / P5 / retro-justification instructions in 75-01-PLAN.md, 75-02-PLAN.md, 75-03-PLAN.md, and 75-04-PLAN.md. Read this addendum before executing any Wave 1+ task.**

### Rule 1 — Skip Task 1a-C entirely

**Plan 75-01 Task 1a-C** ("Retro-justify 13 pre-existing bare `#[allow]` sites in src/") is **SKIPPED** in Phase 75. The 13 sites are migrated wholesale to Phase 75.5 (see `75.5-CONTEXT.md` Category A). gsd-executors running 75-01 must NOT open or edit any of these 10 files for Task 1a-C purposes:

- `src/server/elicitation.rs`
- `src/server/notification_debouncer.rs`
- `src/server/resource_watcher.rs`
- `src/server/transport/websocket_enhanced.rs`
- `src/server/mod.rs`
- `src/shared/sse_optimized.rs`
- `src/shared/connection_pool.rs`
- `src/shared/logging.rs`
- `src/client/mod.rs`
- `src/client/http_logging_middleware.rs`

(These files MAY still be touched by other 75-01 tasks — e.g., Task 1a-A refactors `src/server/streamable_http_server.rs`. Rule 1 only retires the retro-justification work.)

Acceptance criteria in 75-01-PLAN.md that reference Task 1a-C (e.g., "13 pre-existing bare allows in src/ retro-justified per D-02") are **dropped**. The SUMMARY.md for 75-01 should explicitly note "Task 1a-C deferred to Phase 75.5 per D-10-B".

### Rule 2 — P5 fallback branches become "extract more or defer"

Anywhere a PLAN.md says "if residual cog is 26-50 after extraction, apply P5 with `// Why:`" (there are several such branches in Tasks 1a-A, 1b-A, 1b-B, and analogous tasks in 75-02/03/04), the revised decision tree is:

1. **First try harder extraction**: apply an additional P1/P2/P3/P4 pass, revisiting the natural sections. PATTERNS.md suggests cog ≤25 is reachable for most hotspots.
2. **If still residual >25**: log the function to `.planning/phases/75.5-pmat-ex-p5-refactor-backlog/75.5-ESCAPEES.md` (using the format block at the top of that file) and leave the function unchanged in Phase 75. Do NOT add `#[allow(clippy::cognitive_complexity)]` — it's ineffective.
3. **D-03 ≤50 ceiling still applies**: any function measured >50 must be reduced to ≤50 regardless (via real extraction), even if that still leaves it >25. A >50 function deferred to 75.5 should be noted as "≤50 achieved, cog 25 deferred".

### Rule 3 — Specific named P5 candidates from PLAN.md

These functions were explicitly flagged as P5 candidates in their plan bodies. Under the new rule, they follow the Rule 2 decision tree:

- `pmcp-macros/src/mcp_server.rs::collect_resource_methods` (cog 80) — Task 1b-A item 4
- Any other P5-tagged function in 75-02/03/04 (executors identify these by searching for "P5" mentions in their task body)

If an executor cannot reach ≤25 on one of these after a full P1-P4 pass, append to `75.5-ESCAPEES.md` and move on.

### Rule 4 — PMAT total complexity violation count expectation

Because P5 no longer suppresses, 75-01's expected violation reduction changes:
- **Old acceptance**: "PMAT total complexity violation count drops by at least 20" (named hotspots' contribution; retro-justifications don't change the count).
- **New acceptance**: "PMAT total complexity violation count drops by at least the number of hotspot functions this plan actually refactored to ≤25" (no retro-justifications to factor in; 75.5 will handle the retro-justification sites separately).

Executors report actual delta in each plan SUMMARY.md.

### Rule 5 — Wave 5 gets one extra task (D-11-B)

Wave 0 Task 5 resolved D-11-B: the bare `pmat quality-gate --fail-on-violation` that `.github/workflows/quality-badges.yml` runs fails on 5 dimensions, so the badge will NOT flip even if complexity reaches 0 unless the badge workflow is patched to check only complexity.

Plan 75-05 must add a task (or extend its CI-gate task) to patch `.github/workflows/quality-badges.yml` at ~line 72: change the bare `pmat quality-gate --fail-on-violation` to `pmat quality-gate --fail-on-violation --checks complexity`. See `.planning/phases/75-fix-pmat-issues/badge-vs-gate-verification.md` for the worked diff.

## Unchanged

- All P1/P2/P3/P4 extraction techniques (real refactors).
- D-01 (complexity is the gating dimension), D-03 (≤50 hard cap, ≤25 target), D-04 (SATD triage), D-05..D-08.
- Wave ordering, wave 1/2/3/4/5 scope boundaries, dependency chain.
- Snapshot + semantic-regression safety nets from Wave 0.

## For executors — quick checklist

Before executing any Phase 75 task (Wave 1+):

- [ ] Read this addendum.
- [ ] If your task is 75-01 Task 1a-C → skip entirely, write a one-sentence note in the plan SUMMARY.md.
- [ ] If your task body says "apply P5 with `// Why:`" → treat it as "extract harder; if still >25 after P1-P4, defer to 75.5-ESCAPEES.md".
- [ ] Never add a fresh `#[allow(clippy::cognitive_complexity)]` attribute — it's ineffective and will fail the Phase 75 gate check anyway.
- [ ] Measure cog with `pmat analyze complexity --top-files 0 --format json | jq ...` (same query syntax as PLAN.md tasks).
