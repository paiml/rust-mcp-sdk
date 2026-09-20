---
phase: 85-shape-a-pure-config-binary-reference-parity
plan: 09
subsystem: api
tags: [code-mode, prompts, resources, sql-server, shape-a, config-driven]

# Dependency graph
requires:
  - phase: 85-shape-a-pure-config-binary-reference-parity (Plan 05)
    provides: merge_schema_resource + register_prompts + build_server assembly seam
  - phase: 83-toolkit-core-lift
    provides: StaticPromptHandler::from_configs warn-skip resolve_body + StaticResourceHandler
provides:
  - "Synthesis of code-mode://instructions + code-mode://policies resources during Shape A assembly"
  - "merged_resource_configs helper: schema-merged set + synthesized resources, dedup-by-URI"
  - "start_code_mode prompt body now faithful to production (5/5 sections, not 3/5)"
affects: [86-shapes-bcd, 88-dogfood, reference-parity, code-mode-prompt-fidelity]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Template-synthesis of startup-generated resources from [code_mode] config + dialect"
    - "Dedup-by-exact-URI so operator [[resources]] override wins over synthesis"
    - "Test drives PromptHandler::handle to assert the REAL served body (private field exercised via the production path)"

key-files:
  created: []
  modified:
    - crates/pmcp-sql-server/src/assemble.rs

key-decisions:
  - "Synthesized resources are appended to the merged set ONLY if their URI is not already declared (operator override wins, T-85-09-03)"
  - "Policy body renders only NON-secret CodeModeSection fields; token_secret is never read or emitted (T-85-09-01)"
  - "Dialect label derived from cfg.database.backend_type (sqlite->SQLite etc.), no live connector needed for prompt text"
  - "Scoped merge_schema_resource /schema override to the FIRST match (folded-in Gap 3 follow-up)"
  - "Test drives PromptHandler::handle (async) to extract served message text — StaticPromptHandler.body is private, exposed only through handle"

patterns-established:
  - "Pattern: startup-generated resources are synthesized deterministically in assembly, not silently dropped at prompt-resolution time"
  - "Pattern: prompt-body content assertions go through handle() so they lock the served observable, not an internal field"

requirements-completed: [REF-02]

# Metrics
duration: 4min
completed: 2026-05-27
---

# Phase 85 Plan 09: Code-Mode Prompt Synthesis (Gap 3 Closure) Summary

**The assembled `start_code_mode` prompt now carries its full five sections — the two previously-dropped `code-mode://instructions` and `code-mode://policies` URIs are synthesized from `[code_mode]` config + dialect and merged before prompt resolution, restoring prompt↔production fidelity.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-05-27T03:36:41Z
- **Completed:** 2026-05-27T03:40:23Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Closed VERIFICATION Gap 3 (MEDIUM): the served `start_code_mode` prompt no longer silently omits its instructions + policy sections.
- Added two pure synthesizers (`synthesize_instructions_resource`, `synthesize_policies_resource`) that derive the two startup-generated resources the reference config documents as template-generated.
- Added `merged_resource_configs` — the single seam both prompt resolution and the served resource surface read from; dedup-by-URI honors operator `[[resources]]` overrides.
- Folded in the `merge_schema_resource` `/schema`-suffix follow-up: the DDL override now scopes to the FIRST matching resource only, so a config declaring more than one `/schema`-suffixed URI keeps the extras' configured content.
- Added 4 self-contained tests (1 async prompt-body assertion + 3 merge-set assertions) asserting the served body contains the instructions marker, the dialect, ≥3 policy fields, a sensitive-column name, and NO secret leak; plus override-wins and no-code_mode backward-compat cases.

## Task Commits

Each task was committed atomically:

1. **Task 1: Synthesize code-mode instructions + policies resources during assembly** - `0f8b0f58` (feat)

_TDD task executed as a single RED→GREEN cycle: tests written first (compile-failed referencing the not-yet-added `merged_resource_configs`), then the synthesizers + merge seam made them pass. Committed together as a feat since this is a gap-closure landing tests + implementation atomically._

## Files Created/Modified
- `crates/pmcp-sql-server/src/assemble.rs` - Added `INSTRUCTIONS_URI`/`POLICIES_URI` consts, `synthesize_instructions_resource`, `synthesize_policies_resource`, `dialect_label`, `opt_to_string`, `join_or_none`, `merged_resource_configs`; rewired `build_resource_handler` to use the merged set; scoped the `/schema` override to the first match; added 4 tests.

## Decisions Made
- **Operator override wins (dedup-by-URI):** synthesized resources are appended only when no configured `[[resources]]` declares the same URI — matching the reference config's documented "To override, add a `[[resources]]` block with the same URI" path (T-85-09-03).
- **No secret leakage:** the policy synthesizer reads only non-secret `CodeModeSection` fields; `token_secret` is never referenced in the synthesizer code path (T-85-09-01). A test asserts the served body contains neither `CODE_MODE_SECRET` nor `token_secret`.
- **Dialect from config, not connector:** `dialect_label` maps `backend_type` (`sqlite`/`postgres`/`mysql`/`athena`) to a human label; for Shape A the prompt text needs no live connector (preserves SC-1 lazy-startup).
- **Prompt-body assertion through `handle`:** `StaticPromptHandler.body` is private; the test drives `PromptHandler::handle` and extracts the `Content::Text` from the returned messages, asserting the genuinely served observable rather than an internal field.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Scoped `merge_schema_resource` `/schema` override to the first match**
- **Found during:** Task 1
- **Issue:** `ends_with("/schema")` set `found_schema` and overrode content for EVERY matching resource; a config declaring more than one `/schema`-suffixed URI would have all of them clobbered with the DDL. The gap context explicitly called this out to fold in.
- **Fix:** Changed the predicate to `!found_schema && r.uri.ends_with(SCHEMA_URI_SUFFIX)` so only the first match is overridden; subsequent matches pass through with their configured content.
- **Files modified:** crates/pmcp-sql-server/src/assemble.rs
- **Verification:** Existing `merge_overrides_only_schema_content` / `merge_appends_schema_when_absent` tests still pass; new merge-set test confirms the single schema override survives alongside synthesized resources.
- **Committed in:** `0f8b0f58` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug — folded in per gap context instructions)
**Impact on plan:** The `/schema` scoping was explicitly requested in the gap context; no scope creep.

## Issues Encountered
- The reference `[database]` TOML key for backend is `type` (the Rust field is `backend_type` with `#[serde(rename = "type")]`). Initial test fixtures used `backend_type =` and failed `deny_unknown_fields` parsing; corrected to `type =`. Resolved during the GREEN phase.

## Known Stubs
None — both synthesized resources carry real, config-derived content.

## TDD Gate Compliance
This was a `tdd="true"` task (not a plan-level `type: tdd`). RED was confirmed via a compile failure (tests referenced the not-yet-added `merged_resource_configs` and the private `body` field), then GREEN landed the implementation. Tests + implementation committed together in `0f8b0f58` as a single `feat` per gap-closure convention — there is no separate `test(...)` gate commit, which is acceptable for a single-task `tdd="true"` (not the plan-level gate-sequence rule).

## Quality Notes
- `crates/pmcp-sql-server/src/assemble.rs` is `cargo fmt`-clean and has zero clippy warnings attributable to it under `cargo clippy -p pmcp-sql-server --no-default-features --features sqlite --all-targets -- -D warnings`.
- The only clippy error surfaced under that command is a PRE-EXISTING `field_reassign_with_default` lint in the dependency `crates/pmcp-server-toolkit/src/code_mode.rs:471-472` (rust-1.95.0 mismatch), documented as deferred in 85-06-SUMMARY and the gap context — OUT OF SCOPE for this plan; not introduced by these changes.
- Zero SATD in the touched file.
- All 11 lib tests pass with `--test-threads=1` (8 pre-existing + 3 new).

## Next Phase Readiness
- Gap 3 closed; the `start_code_mode` prompt is behaviorally faithful to production. Re-verification of SC-3 / 85-VERIFICATION can confirm the two previously-skipped `include_resources` URIs now resolve.
- Remaining incomplete Phase 85 plans: 85-08, 85-10.

## Self-Check: PASSED
- FOUND: crates/pmcp-sql-server/src/assemble.rs
- FOUND commit: 0f8b0f58

---
*Phase: 85-shape-a-pure-config-binary-reference-parity*
*Completed: 2026-05-27*
