---
phase: 106-client-host-surface
plan: 03
subsystem: docs
tags: [pmcp-book, sampling, hosting, docs, host-06]

# Dependency graph
requires:
  - phase: 106-client-host-surface
    plan: 01
    provides: "pmcp::client::host module + HOST-06 rustdoc naming the LLM-server pattern"
provides:
  - "pmcp-book Sampling & Hosting page (ch17-04) disambiguating spec host sampling from the LLM-server pattern"
  - "Chapter 17 TOC entry linking the new page"
affects: [111-sampling-hosting-docs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Short Chapter 17 book page using real public trait paths only, verified against src/"

key-files:
  created:
    - pmcp-book/src/ch17-04-sampling-hosting.md
  modified:
    - pmcp-book/src/SUMMARY.md

key-decisions:
  - "Reworded the LLM-server path note to avoid the literal string 'server::traits' so the acceptance grep (! grep -q 'server::traits') stays green while still steering readers away from any internal-module variant"
  - "Described on_sampling_approval as a preflight gate that denies before the LLM runs (plus optional result-review), matching the locked HOST-06 wording, without overclaiming invocation internals"

requirements-completed: [HOST-06]

# Metrics
duration: 6min
completed: 2026-07-17
---

# Phase 106 Plan 03: Sampling & Hosting Book Page Summary

**The pmcp-book now has a Sampling & Hosting page (ch17-04) that unambiguously contrasts spec host sampling (server->client, answered by `pmcp::client::host::HostSamplingHandler`, gated by a preflight `on_sampling_approval` hook) against the legacy LLM-server pattern (client->server, answered by `pmcp::SamplingHandler`), with a direction/entry-point/trait contrast table and the nested-flow limitation, all using verified real public trait paths and linked into the Chapter 17 TOC.**

## Performance

- **Duration:** ~6 min
- **Tasks:** 2
- **Files created:** 1  **Files modified:** 1

## Accomplishments
- Wrote `pmcp-book/src/ch17-04-sampling-hosting.md` ("Sampling & Hosting"): an intro that MCP sampling runs in two opposite directions; a "Spec host sampling (server asks the client)" section covering `ClientBuilder::on_sampling`/`on_elicitation`/`on_roots`, the preflight `on_sampling_approval` deny-before-LLM gate and optional `on_sampling_result_review`, and the nested-flow / idle-host + `Server::run` (D-106-A) limitation; an "LLM-server pattern (client asks a server)" section for the kept-not-deprecated `Client::create_message` path answered by `pmcp::SamplingHandler` (a.k.a. `pmcp::server::SamplingHandler`); a four-column contrast table (Direction | Caller entry point | Answering trait | Use case), one row per pattern; and a pointer to `cargo run --example s49_sampling_host` plus a durable "full chapter lands in Phase 111" note.
- Verified every cited API against the worktree source before writing: `pmcp::SamplingHandler` re-export at `src/lib.rs:80`; `HostSamplingHandler`/`HostElicitationHandler`/`RootsProvider`/`ApprovalDecision`/`PreflightApproval`/`SamplingResultReview` exported from `src/client/host/mod.rs`; the five `on_*` builder methods at `src/client/mod.rs:2611-2660`.
- Linked the page into `pmcp-book/src/SUMMARY.md` as a two-space-indented Chapter 17 sub-entry immediately after `ch17-03-sampling-tools.md`; no existing entries reordered or removed.
- `mdbook build pmcp-book` runs clean (exit 0, HTML backend, no broken-link warning), executed unpiped per CLAUDE.md.

## Task Commits

1. **Task 1: Write the Sampling & Hosting disambiguation page** - `8e066f0e` (docs)
2. **Task 2: Link the page into SUMMARY.md + build the book** - `85ba059d` (docs)

## Files Created/Modified
- `pmcp-book/src/ch17-04-sampling-hosting.md` - New disambiguation page (created).
- `pmcp-book/src/SUMMARY.md` - Added Chapter 17 sub-entry linking the new page (modified).

## Decisions Made
- **Avoided the literal `server::traits` string:** the plan's acceptance grep asserts `server::traits` is ABSENT. The initial draft mentioned it inside a "there is no such path" negation, which tripped the gate. Reworded to steer readers away from "any internal-module variant" while naming only the two real public paths — prose intent preserved, gate green.
- **Approval described as a completed preflight gate:** per the locked HOST-06 wording (106-CONTEXT §Legacy path disambiguation and the plan objective), the page presents `on_sampling_approval` as a preflight deny-before-LLM hook with an optional post-generation `on_sampling_result_review`, not as a deferred type.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Draft prose contained the literal `server::traits`, failing the acceptance grep**
- **Found during:** Task 1 verification
- **Issue:** The page warned readers away from `server::traits::SamplingHandler` by naming it, but the acceptance criterion is `! grep -q 'server::traits'` — the negation itself contained the forbidden string.
- **Fix:** Reworded the callout to state only the two real public paths (`pmcp::SamplingHandler` / `pmcp::server::SamplingHandler`) and warn against "any internal-module variant", removing the literal string.
- **Files modified:** pmcp-book/src/ch17-04-sampling-hosting.md
- **Verification:** Task 1 automated grep chain returns VERIFY PASS.
- **Committed in:** 8e066f0e (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug). **Impact:** None on scope — documentation-only, both acceptance gates green.

## Issues Encountered
- None beyond Deviation 1.

## Threat Flags
None — documentation-only change; no new trust boundary, no code, no packages installed (matches the plan's `<threat_model>`: T-106-09 mitigated by constraining prose to the verified interfaces and the passing acceptance greps).

## Self-Check: PASSED

- `pmcp-book/src/ch17-04-sampling-hosting.md` exists on disk.
- `pmcp-book/src/SUMMARY.md` contains `ch17-04-sampling-hosting.md`.
- Both commits (8e066f0e, 85ba059d) present in git history.
- Task 1 grep chain (LLM-server pattern + HostSamplingHandler + pmcp::SamplingHandler present, server::traits absent) passes.
- `mdbook build pmcp-book` exits 0 with no broken-link warning.

---
*Phase: 106-client-host-surface*
*Completed: 2026-07-17*
