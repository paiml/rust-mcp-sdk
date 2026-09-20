---
phase: 107-contracts-package-format
plan: 03
subsystem: testing
tags: [provable-contracts, conformance-fixtures, team-servers, yaml, serde_json, TEAM-06]

# Dependency graph
requires:
  - phase: 104-task-augmented-tool-results-dx
    provides: "SEP-1686 ToolOutput::Result surface (pmcp 2.12.0) — top-level _meta[related_task] shape encoded in team-mcp/complete_task fixtures"
provides:
  - "contracts/team-servers-v1.yaml — 4-equation provable-contract for the team-fs/mem-mcp/approval-mcp/team-mcp tool surfaces (namespaced provisional PMCP extension, storage-agnostic)"
  - "contracts/team-servers/fixtures/** — versioned (schema_version=1) per-server request/expect conformance fixtures (TEAM-06 shared artifact)"
  - "tests/team_contracts_conformance.rs — dependency-free schema-aware gate cross-referencing every fixture tool against the contract"
affects: [phase-108, phase-109, team-servers, TEAM-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Versioned conformance-fixture schema (schema_version/case_id/server/request/expect{outcome,match,response})"
    - "One equation per server surface in the provable-contracts house format, per-tool detail inside formula/invariants"
    - "CARGO_MANIFEST_DIR-resolved contract + fixtures for location-independent conformance testing"

key-files:
  created:
    - contracts/team-servers-v1.yaml
    - contracts/team-servers/fixtures/team-fs/fs__list.success.json
    - contracts/team-servers/fixtures/team-fs/fs__complete_task.success.json
    - contracts/team-servers/fixtures/team-fs/fs__write.invalid-args.error.json
    - contracts/team-servers/fixtures/mem-mcp/mem__add.success.json
    - contracts/team-servers/fixtures/mem-mcp/mem__search.success.json
    - contracts/team-servers/fixtures/approval-mcp/resolve_approval.success.json
    - contracts/team-servers/fixtures/approval-mcp/get_approval.success.json
    - contracts/team-servers/fixtures/approval-mcp/team_approval__ask.success.json
    - contracts/team-servers/fixtures/team-mcp/team_mcp__member.success.json
    - contracts/team-servers/fixtures/team-mcp/team_mcp__unknown-member.error.json
    - contracts/team-servers/fixtures/team-mcp/team_mcp__malformed-depth.error.json
    - contracts/team-servers/fixtures/team-mcp/team_mcp__excessive-depth.error.json
    - contracts/team-servers/fixtures/team-mcp/team_mcp__self-call.error.json
    - tests/team_contracts_conformance.rs
  modified: []

key-decisions:
  - "One equation per server surface (4 equations), matching the mcp-protocol-sdk-v1.yaml house grouping"
  - "Omitted lean_theorem entirely (no proof obligations wired this phase; scalar-only house format)"
  - "No binding.yaml authored — bindings + pmat comply deferred to Phase 109 (target functions live in crates/pmcp-team-servers)"
  - "Represented the x-pmcp-team-depth header inside fixture request._meta (extra fields allowed by the schema)"

patterns-established:
  - "Versioned conformance-fixture schema: SDK + platform harnesses interpret request/expect identically (TEAM-06)"
  - "Dynamic tool families cross-referenced by prefix (team_mcp__, team_approval__ask_); static tools by exact name"

requirements-completed: [PKG-03]

# Metrics
duration: 12min
completed: 2026-07-17
---

# Phase 107 Plan 03: Team-Server Tool Contracts + Conformance Fixtures Summary

**Provable-contracts YAML for the four team-server tool surfaces (19 static + 2 dynamic tool families) plus 13 versioned request/expect conformance fixtures and a dependency-free schema-aware gate — the TEAM-06 shared-fixture foundation.**

## Performance

- **Duration:** ~12 min
- **Completed:** 2026-07-17
- **Tasks:** 2
- **Files modified:** 15 created

## Accomplishments
- `contracts/team-servers-v1.yaml`: four equations (`fs_tool_surface`, `mem_tool_surface`, `approval_tool_surface`, `team_dispatch_surface`) enumerating all 11 `fs__*`, 6 `mem__*`, the two unnamespaced legacy names (`resolve_approval`, `get_approval`), and the two dynamic families (`team_approval__ask_<member>`, `team_mcp__<member>`). Marked as a namespaced provisional PMCP extension, storage-agnostic ("stable configured member ID", no DynamoDB/DDB leak), no `lean_theorem`, no `binding.yaml`.
- team-mcp dispatch invariants captured correctly (NOT the obsolete bypass): lookup by stable member ID, `ToolOutput::Result` owns the full `CallToolResult` incl. top-level `_meta[related_task]`, strict `x-pmcp-team-depth` parse, self-call guard on ids, ancestor-cycle guard, advertised==enforced schema.
- 13 versioned fixtures across 4 server directories: positive coverage per family + both dynamic families + `related_task` under top-level `_meta`, plus 5 negative/security cases (invalid arguments, unknown member, malformed depth, excessive depth, self-call).
- `tests/team_contracts_conformance.rs`: 5 tests validating the fixture schema, cross-referencing every fixture tool against the contract, asserting per-server coverage and ≥4 negatives — resolving both contract and fixtures via `CARGO_MANIFEST_DIR`, using only `serde_json` + `std`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Author contracts/team-servers-v1.yaml** - `e8496d83` (feat)
2. **Task 2: Versioned conformance fixtures + schema-aware test** - `1f2bfc35` (test)

## Files Created/Modified
- `contracts/team-servers-v1.yaml` - 4-equation provable-contract for the team-server tool surfaces
- `contracts/team-servers/fixtures/team-fs/*.json` - fs__list, fs__complete_task (top-level _meta.related_task), fs__write invalid-args negative
- `contracts/team-servers/fixtures/mem-mcp/*.json` - mem__add, mem__search positives
- `contracts/team-servers/fixtures/approval-mcp/*.json` - resolve_approval, get_approval, team_approval__ask_<member>
- `contracts/team-servers/fixtures/team-mcp/*.json` - team_mcp__<member> positive + 4 negative/security cases
- `tests/team_contracts_conformance.rs` - schema-aware conformance gate (5 tests)

## Decisions Made
- One equation per server surface (4 total), per-tool detail inside `formula`/`invariants` — mirrors the `mcp-protocol-sdk-v1.yaml` house grouping.
- Omitted `lean_theorem` (house format uses scalar theorem identifiers; no proof obligations wired this phase).
- No `binding.yaml` — bindings + `pmat comply` are a Phase-109 obligation (target functions do not exist in-repo; pv tooling absent per RESEARCH.md).
- Modeled the `x-pmcp-team-depth` header inside fixture `request._meta` — the versioned schema permits extra fields; the test only pins `schema_version`, `case_id`, `server`, `request.name`, `expect{outcome,match,response}`.

## Deviations from Plan

None - plan executed exactly as written. Both task verifications passed on first run; `cargo fmt` reflowed the test file (formatting only, no logic change) and `cargo clippy --features full --lib --tests` is clean.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Scope Note (deferred to Phase 109 / TEAM-06)
This plan ships the versioned fixture SCHEMA + representative positive coverage per tool family + both dynamic families + a handful of high-value negative/security cases. Exhaustive per-tool positive fixtures and the full adversarial matrix (ancestor cycle, advertised≠enforced schema, unauthorized approval resolution, missing approval) are deferred to Phase 109, where the reference servers actually EXECUTE the suite. `binding.yaml` / `pmat comply` are likewise a Phase-109 obligation.

## Next Phase Readiness
- The contract precedes the Phase 108/109 implementations (contract-first house rule satisfied).
- `contracts/team-servers/fixtures/**` is the single shared artifact both the Phase 109 reference servers and the platform servers verify against (TEAM-06). Location is a documented cross-repo coordination point.

## Self-Check: PASSED

- FOUND: contracts/team-servers-v1.yaml
- FOUND: tests/team_contracts_conformance.rs (5 tests passing)
- FOUND: contracts/team-servers/fixtures/** (13 fixtures)
- FOUND commits: e8496d83 (Task 1), 1f2bfc35 (Task 2), e5f445eb (SUMMARY)

---
*Phase: 107-contracts-package-format*
*Completed: 2026-07-17*
