---
phase: 109-team-reference-servers
plan: 07
subsystem: team-servers
tags: [pmcp-team-servers, conformance, fixtures-v2, ConformanceTarget, wire-level, deterministic-replay, advertised-equals-enforced]

# Dependency graph
requires:
  - phase: 109-team-reference-servers
    plan: 00
    provides: "Client::call_tool_with_meta / call_tool_with_task_and_meta low-level _meta APIs the runner sends fixture _meta through"
  - phase: 109-team-reference-servers
    plan: 01
    provides: "conformance module seam, public DuplexTransport, four server builders' features"
  - phase: 109-team-reference-servers
    plan: 03
    provides: "InMemoryMemoryBackend::deterministic() (mem-001…) id seam"
  - phase: 109-team-reference-servers
    plan: 04
    provides: "ApprovalRepository::deterministic() (appr-001…) id seam"
  - phase: 109-team-reference-servers
    plan: 05
    provides: "team-mcp guards + MemberHandle + build_team_mcp_server + RELATED_TASK_META_KEY related-task semantics"
provides:
  - "Exportable wire-level fixture runner (run_fixtures) over a ConformanceTarget abstraction (in-memory Client over DuplexTransport + HTTP skeleton), gated behind the conformance feature"
  - "Fixture schema v2: kind (tools_list | tool_call), ordered scenario/setup grouping, deterministic seed/clock/id injection, capture+substitution, expected per-tool input schemas, wildcard/predicate assertions"
  - "v2 fixtures for all four reference servers with deterministic ids: every tool + every guard + exact tools_list surface with schemas"
  - "tests/conformance.rs drives all four fresh deterministic servers at zero failures + every-tool/every-guard coverage + an ignored tools_list generator"
  - "Negative harness tests (in runner.rs) proving the runner FAILS on extra tool / schema drift / missing guard / malformed fixture"
affects: [109-08-binding-finalize]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ConformanceTarget trait abstracts the runner over transport (in-memory Client over DuplexTransport AND HTTP) so the platform can point it at an endpoint (D-19)"
    - "Ignored fixture-generator test freezes the EXACT advertised surface + per-tool inputSchema of each live server into tools_list.json; normal tests REPLAY it so any drift fails"
    - "Independent cases run against a FRESH deterministic target per case; ordered scenario groups share one target with capture/substitution for stateful sequences"
    - "Target-owned temp-dir lifetime: FsTarget wraps ClientTarget + TempDir so the team-fs workspace/review roots outlive the client hop"
    - "@contains: predicate matching on stable deterministic substrings tolerates generated fields while asserting real content"

key-files:
  created:
    - "crates/pmcp-team-servers/tests/conformance.rs"
    - "contracts/team-servers/fixtures/team-fs/ (12 v2 fixtures + tools_list)"
    - "contracts/team-servers/fixtures/mem-mcp/ (7 v2 fixtures + tools_list)"
    - "contracts/team-servers/fixtures/approval-mcp/ (6 v2 fixtures + tools_list)"
    - "contracts/team-servers/fixtures/team-mcp/ (7 v2 fixtures + tools_list)"
  modified:
    - "crates/pmcp-team-servers/src/conformance/runner.rs (rustfmt touch-up on the Task-1 committed runner)"
    - "tests/team_contracts_conformance.rs (root structural gate updated v1 → v2)"

decisions:
  - "tools_list fixtures are GENERATED from live servers by an ignored generator test, then replayed — the fixture is the frozen, reviewed surface; drift fails the suite, intentional change requires regeneration"
  - "Error fixtures assert outcome + error message SUBSTRING (not a brittle numeric code), because the high-level Server flattens guard codes; message carries the guard's semantic identity"
  - "team-mcp unknown-member is realized as pmcp tool-not-found (unadvertised member), matching 109-05; the fixture asserts a 'not found' error, never a panic"
  - "team-mcp invalid-args is a structurally-invalid (non-integer numeric 1.5) depth _meta rejected by strict parsing — a distinct wire input complementary to the string malformed-depth case"
  - "Root tests/team_contracts_conformance.rs updated to v2 (Rule 3 blocking): regenerating fixtures to v2 broke its hard-coded schema_version==\"1\" asserts; the plan's own acceptance requires it stay green"

requirements-completed: [TEAM-06]

# Metrics
duration: 45min
completed: 2026-07-18
---

# Phase 109 Plan 07: Wire-Level Conformance Harness (TEAM-06) Summary

**Ships the exportable wire-level conformance harness: fixture schema v2 (kind / ordered scenarios / deterministic seed-clock-id injection / capture+substitution / expected per-tool input schemas / wildcard+predicate assertions) driven by a `run_fixtures` runner abstracted over a `ConformanceTarget` (in-memory `Client` over `DuplexTransport` AND an HTTP skeleton — D-19); tool_call fixtures send `_meta` via the 109-00 low-level client API and assert related-task SEMANTICALLY via `CallToolResult::related_task()` under `RELATED_TASK_META_KEY`; independent cases replay against FRESH deterministic server instances while stateful sequences use ordered scenarios with capture/substitution; NEGATIVE harness tests prove the runner fails on extra tools / schema drift / a missing guard / a malformed fixture; and v2 fixtures are regenerated with deterministic ids covering every tool + every guard for all four reference servers, proven conformant at zero failures.**

## Performance

- **Duration:** ~45 min
- **Completed:** 2026-07-18
- **Tasks:** 2 (Task 1 was already committed at HEAD on entry — `784fba13`; this run completed Task 2)
- **Files:** 43 changed (new tests/conformance.rs, 32 v2 fixtures, runner.rs fmt touch-up, root structural test updated)

## Accomplishments

- **Fixture schema v2 + ConformanceTarget runner (Task 1, `784fba13` — verified on entry):** `src/conformance/runner.rs` defines the v2 fixture types (serde), a `ConformanceTarget` trait with an in-memory `ClientTarget` over `DuplexTransport` plus an HTTP constructor behind the `http` feature, `run_fixtures`/`ConformanceReport`/`assert_conformant`, exact tools/list name-set + per-tool schema equality, `_meta` send via `call_tool_with_meta`/`call_tool_with_task_and_meta`, semantic related-task via `related_task()`, subset/exact/predicate (`@contains:`, `@nonempty`, `@string`, …) matching, capture/substitution, a minimal JSON selector that auto-descends into JSON-carrying `content[0].text`, and six in-file NEGATIVE harness tests. Re-exported from `lib.rs` behind the `conformance` feature. Verified: `cargo build --features conformance` + `conformance::runner` tests green.
- **v2 fixtures regenerated with deterministic ids (Task 2):**
  - **team-fs** — an ordered `fs-lifecycle` scenario covering all 10 storage tools (write→read→head→stat→list→append→create_directory→get_download_url→sync_to_review→sync_from_review) against one shared temp-dir-backed instance, plus an independent `fs__complete_task` related-task fixture (11th tool) and the exact 11-tool `tools_list` with schemas.
  - **mem-mcp** — an ordered `mem-lifecycle` scenario add→get→search→list_recent→delete capturing `mem_id` (`mem-001`) and substituting it, plus `mem__complete_task` and the exact 6-tool `tools_list`.
  - **approval-mcp** — an ordered `approval-lifecycle` scenario ask→get(pending)→resolve→get(resolved) capturing `approval_id` (`appr-001`), plus an unknown-role error (unadvertised ask ⇒ tool-not-found) and the exact `resolve_approval` + `get_approval` + 2 `team_approval__ask_<role>` `tools_list`.
  - **team-mcp** — `member.success` (related-task via `related_task()` under `RELATED_TASK_META_KEY`) plus EVERY guard error (unknown-member, self-call, malformed-depth, excessive-depth, ancestor-cycle, invalid-args), each sending guard `_meta` (`x-pmcp-team-depth`/`-caller`/`-ancestors`), plus the exact 1-member `tools_list`.
- **tests/conformance.rs:** builds each of the four reference servers with a FRESH deterministic instance (mem/approval id seams; an injected offline `FixedSource` member for team-mcp; a temp-dir `LocalDirBackend` for team-fs) and runs `run_fixtures(fresh_target, fixtures_dir/<server>)` asserting zero failures; adds every-tool + every-guard coverage assertions (a missing fixture fails the test); and ships an `#[ignore]`d `regenerate_tools_list_fixtures` generator that freezes each live server's exact advertised surface + per-tool input schema into `<server>/tools_list.json`.
- **Root structural gate kept green:** `tests/team_contracts_conformance.rs` updated from v1 → v2 (kind-discriminated validation, tools_list_schema surface check, related-task under `RELATED_TASK_META_KEY`).

## Task Commits

1. **Task 1: fixture schema v2 + ConformanceTarget + wire-level runner** — `784fba13` (feat) — *committed before this executor run; verified building + green on entry.*
2. **Task 2: regenerate v2 fixtures + wire-level conformance harness for all four servers** — `fb75c6dc` (test)

## Decisions Made

- **Generated-then-replayed tools_list fixtures.** The `tools_list.json` per-tool schema map is produced from the live servers by the ignored generator and then replayed by the normal tests. The committed, reviewed fixture is the frozen surface; any advertised-set or schema drift fails the suite, and an intentional surface change requires an explicit regeneration + review. This is the standard contract-freeze pattern and keeps the schemas byte-exact without hand-transcription error.
- **Error fixtures assert message substring, not numeric code.** The high-level `Server` flattens guard error codes, so the guard's semantic identity ("self-call rejected", "excessive team depth", …) lives in the message; the runner's `match_error` does substring containment.
- **unknown-member == tool-not-found.** Per 109-05, an unadvertised member yields pmcp's tool-not-found error (never a panic); the fixture asserts a `not found` error outcome.
- **invalid-args == non-integer numeric depth.** A structurally-invalid `1.5` depth `_meta` value is rejected by strict parsing — a distinct wire input complementary to the string `not-a-number` malformed-depth case.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated root `tests/team_contracts_conformance.rs` from v1 to v2**
- **Found during:** Task 2 (after regenerating fixtures)
- **Issue:** The root structural gate hard-asserts `schema_version == "1"` and v1-only shapes (`request.name` on every fixture, `response.error.code` numeric, `_meta.related_task`). Regenerating fixtures to v2 broke all three, but the plan's own acceptance criterion + verification require `cargo test --test team_contracts_conformance` to stay green.
- **Fix:** Rewrote the structural validation for schema v2 — `schema_version == "2"`, `kind`-discriminated cases (tools_list requires `expect.tools_list_schema`; tool_call requires `request.name` + outcome/response shape), tool-name capture across both kinds, and related-task under `RELATED_TASK_META_KEY`. Coverage/negative-count asserts unchanged in intent.
- **Files modified:** `tests/team_contracts_conformance.rs` (outside the plan's enumerated `files_modified`, but required by its verification block)
- **Verification:** `cargo test --test team_contracts_conformance` → 5 passed.
- **Committed in:** `fb75c6dc`

**2. [Rule 3 - Blocking] rustfmt touch-up on the Task-1 `runner.rs`**
- **Found during:** Task 2 (pre-commit fmt gate)
- **Issue:** The current local rustfmt flagged two lines in the already-committed `runner.rs`; `cargo fmt --all -- --check` would block any subsequent commit.
- **Fix:** Applied `cargo fmt` (two cosmetic line-wraps, no behavior change) and included it in the Task 2 commit.
- **Files modified:** `crates/pmcp-team-servers/src/conformance/runner.rs`
- **Committed in:** `fb75c6dc`

**Total deviations:** 2 auto-fixed (both Rule 3 blocking). Both are required to satisfy the plan's own verification/acceptance and introduce no behavior change.

## Threat Model Compliance

- **T-109-07-01 (advertised≠enforced surface drift):** EXACT tools/list name-set equality + per-tool input-schema equality (frozen tools_list fixtures) + every-tool fixtures + the in-runner negative harness (extra-tool / schema-drift) make any drift a failure.
- **T-109-07-02 (a guard silently unenforced):** every-guard error fixtures (unknown-member, self-call, malformed-depth, excessive-depth, ancestor-cycle, invalid-args) with sent `_meta` assert enforcement at the wire; the runner's missing-guard negative test proves the harness catches an unenforced guard.
- **T-109-07-03 (non-replayable fixtures):** deterministic id/clock seams + ordered scenarios + capture/substitution make fixtures replayable against fresh instances (mem-001/appr-001 replay exactly).
- **T-109-07-SC (dependency graph):** no new registry package; the runner stays dependency-light (`pretty_assertions` unused in the library path — `assert_conformant` builds its own diff string); test-only deps (`tempfile`, `pmcp-agent`, `pmcp-package`) already vendored.

## Known Stubs

None. The HTTP `ConformanceTarget` constructor is a real (feature-gated) `Client`-over-URL builder skeleton per D-19 — the platform supplies the running endpoint; the in-memory path is exercised end-to-end here.

## Threat Flags

None — no new network endpoint, auth path, or trust-boundary schema beyond the plan's registered threat model. The harness is in-repo test data + an in-process transport.

## Verification Performed

- `cargo build -p pmcp-team-servers --features conformance` → exit 0.
- `cargo test -p pmcp-team-servers --test conformance --all-features` → 8 passed, 1 ignored (4 zero-failure server runs + 4 coverage asserts; generator ignored).
- `cargo test -p pmcp-team-servers --all-features` → 128 passed, 1 ignored (no regression to fs/mem/approval/team/derive suites + doctests).
- `cargo test --test team_contracts_conformance` → 5 passed (root structural gate green on v2).
- `cargo fmt -p pmcp-team-servers -- --check` → clean; `cargo clippy -p pmcp-team-servers --all-targets --all-features -- -D warnings` → No issues found.
- Each per-task commit passed the repo's pre-commit `make quality-gate` (no `--no-verify`).

## Next Phase Readiness

- 109-08 can flip the four `binding.yaml` entries to `status: implemented` — the wire-level runner now proves advertised==enforced for all four surfaces, and the platform can import `run_fixtures` + `ConformanceTarget` behind the `conformance` feature to drive its own in-memory or HTTP servers against the v2 fixtures.
- No blockers.

## Self-Check: PASSED

- Files present: `crates/pmcp-team-servers/tests/conformance.rs`, all four `contracts/team-servers/fixtures/<server>/tools_list.json`, the v2 scenario + guard fixtures — all on disk.
- Commits present in git history: `784fba13` (Task 1), `fb75c6dc` (Task 2).

---
*Phase: 109-team-reference-servers*
*Completed: 2026-07-18*
