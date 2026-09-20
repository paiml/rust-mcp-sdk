# Phase 75: Fix PMAT issues - Context

**Gathered:** 2026-04-22
**Status:** Ready for planning
**Patched:** 2026-04-23 (post-cross-AI-review — added D-10/D-11 conditional decisions
on PMAT-allow suppression behavior + badge/gate semantic alignment)

<domain>
## Phase Boundary

Restore the auto-generated `Quality Gate: passing` badge on the README by
remediating the PMAT findings surfaced after PR #246 fixed the `pmat-cli` →
`pmat` install bug and the `quality-badges.yml` workflow began running real
analysis instead of falling back to sentinel values.

**In scope:**
- Reduce `pmat quality-gate --fail-on-violation` to exit 0 (the badge signal).
- Remediate the 94 cognitive-complexity-over-25 functions (the gating dimension).
- Best-effort improvements to SATD (33), duplicate (439), entropy (4), and
  README sections (2 detected) within the same waves, where they fall out of
  the complexity work or are cheap to address.
- Tune `.pmat/project.toml` (or equivalent) to exclude `examples/`, `tests/`,
  and `benches/` from duplicate detection where appropriate.
- Add a CI check that runs `pmat quality-gate --fail-on-violation` on PRs and
  blocks merge on regression.

**Out of scope (deferred to other phases or backlog):**
- Driving SATD, duplicate, entropy, or sections to absolute zero — those are
  best-effort here, not gating.
- Rewriting hotspot files for reasons other than complexity (architecture
  cleanup, public-API changes, performance work).
- Net-new features in `pmcp-code-mode/`, `pentest/`, `deployment/`, etc. —
  this phase only touches them to fix complexity violations.

**Baseline (worktrees excluded, from commit 28db59a, 2026-04-22):**

| Dimension  | Count | Notes |
|------------|-------|-------|
| complexity | 94    | cognitive complexity > 25 |
| duplicate  | 439   | includes examples/tests/benches noise |
| satd       | 33    | TODO/FIXME/HACK/XXX comments |
| entropy    | 4     | |
| sections   | 2     | only README Installation + Usage detected |

**Hotspots (from commit message):** `crates/pmcp-code-mode/`,
`cargo-pmcp/src/pentest/`, `cargo-pmcp/src/deployment/`,
`src/server/streamable_http_server.rs`, `pmcp-macros/`.

</domain>

<decisions>
## Implementation Decisions

### Definition of Done
- **D-01:** "Phase 75 done" = `pmat quality-gate --fail-on-violation` exits 0
  and the auto-generated README badge flips back to `Quality Gate: passing`.
  Complexity is the gating dimension; SATD, duplicate, entropy, and sections
  are best-effort improvements within the same waves but do NOT block phase
  closure.

### Complexity Remediation Strategy
- **D-02:** Default approach is pragmatic per-function refactor — extract
  helper methods, introduce intermediate types (state structs, command enums),
  decompose deeply-nested matches. For functions that are irreducibly complex
  (parsers, AST walkers, protocol-message dispatch, state machines), use
  `#[allow(clippy::cognitive_complexity)]` with a one-line `// Why:` comment
  immediately above the attribute justifying why decomposition would harm
  clarity. Every `#[allow]` must have a `Why:` justification — no bare allows.

- **D-03:** Hard ceiling on `#[allow(clippy::cognitive_complexity)]` use:
  even with the allow, a function should aim for ≤35 cognitive complexity
  and MUST NOT exceed 50. Functions above 50 require refactor regardless of
  how good the justification is. Signals that the escape hatch is for
  "legitimate but bounded" complexity, not "gave up".

### SATD + Duplicate Policy
- **D-04:** SATD triage is per-comment, three-way:
  - **Trivial / obsolete** (the comment refers to code that no longer exists,
    or the concern was addressed elsewhere) → delete the comment (and any dead
    code attached).
  - **Real follow-up work needed** → file a GitHub issue in `paiml/rust-mcp-sdk`,
    replace the SATD marker with a normal comment that references the issue
    number (e.g. `// See #NNN — defer to streaming response phase`).
  - **Cheap to fix now (<30 min)** → just fix it in this phase.
  Do not blanket-convert all 33 to issues without triage. Do not blanket-fix
  all 33 in this phase.

- **D-05:** Duplicate triage is by file location:
  - Duplicates inside `src/`, `crates/*/src/`, `cargo-pmcp/src/` → real
    refactor (extract helpers, share via internal modules, use macros where
    appropriate).
  - Duplicates inside `examples/`, `tests/`, `benches/`, `fuzz/` → tune the
    PMAT config (`.pmat/project.toml` or `.pmatignore` — verify the actual
    config file PMAT 3.11.x reads) to exclude these paths from duplicate
    detection. Test fixtures and example boilerplate are intentionally
    duplicated for reader clarity and should not be flagged.
  - Goal is "duplicate count drops meaningfully", not "duplicate count = 0".

### Wave Structure
- **D-06:** Split work into waves by hotspot directory rather than by PMAT
  dimension. This lets each wave ship independently and probably flips the
  badge mid-phase as complexity violations clear:
  - **Wave 1:** `src/server/streamable_http_server.rs` + `pmcp-macros/`
    (cross-cutting impact; touching them affects downstream crates).
  - **Wave 2:** `cargo-pmcp/src/pentest/` + `cargo-pmcp/src/deployment/`
    (newer code, mostly self-contained inside cargo-pmcp).
  - **Wave 3:** `crates/pmcp-code-mode/` (largest hotspot, most contained;
    last so earlier waves don't perturb it).
  - SATD triage, duplicate triage, and PMAT config tuning happen *within* the
    relevant wave when the file is being touched anyway, OR in a parallel
    "housekeeping" plan if they don't naturally fall out.
  Planner is free to add a final "Wave 4: enforcement" plan for the CI gate
  (D-07) so it lands after the badge is already green.

### Going-Forward Enforcement
- **D-07:** After the badge flips green, add a CI check that runs
  `pmat quality-gate --fail-on-violation` on every PR and blocks merge if it
  fails. Likely lives in `.github/workflows/quality-badges.yml` (or a new
  workflow specifically for the gate so badges and gating are decoupled). Do
  NOT add to pre-commit hook in this phase — pre-commit already runs
  `make quality-gate` (fmt, clippy, build, doctests) and adding PMAT there
  risks slowing the dev loop too much. CI block alone is enough to prevent
  regression.

### Post-Research Refinements (added 2026-04-22 after research surfaced gaps)

- **D-08:** Wave map expands beyond the 5 originally-named hotspots.
  Researcher confirmed only 16 of 73 in-scope src/ violations live in the
  named directories; 57 more are scattered across `cargo-pmcp/src/commands/`,
  `cargo-pmcp/src/loadtest/`, `crates/mcp-tester/`, `crates/mcp-preview/`,
  `src/server/path_validation.rs`, `src/server/schema_utils.rs`,
  `src/utils/json_simd.rs`, `src/server/workflow/task_prompt_handler.rs`.
  D-06's wave structure stays (by hotspot directory) but the planner adds
  the additional directories to existing waves where they cluster
  thematically, or creates a "Wave 4: remaining hotspots sweep" that batches
  the rest. Definition of done from D-01 is binding — phase doesn't close
  until the badge can flip green.

- **D-09:** D-05 mechanism revised — `.pmatignore` and `[analysis]
  exclude_patterns` were verified non-functional in PMAT 3.15.0 (two live
  experiments). The CI gate from D-07 will use
  `pmat quality-gate --fail-on-violation --checks complexity` to scope the
  gate to the gating dimension only (consistent with D-01). For the 21
  examples/ violations: planner picks the cheapest path that lets the
  badge flip — preferred order: (a) verify whether `pmat quality-gate
  --include 'src/**'` or equivalent path filter actually works on PMAT
  3.15.0, (b) if yes, scope the gate to non-example code, (c) if no, bulk
  `#[allow(clippy::cognitive_complexity)]` on examples/ functions with a
  single per-file `// Why: illustrative demo code, not production` comment,
  (d) only refactor examples/ functions if neither (a) nor (c) is workable.
  D-05's path-exclusion goal for duplicates is downgraded to "best effort"
  — duplicates are not gating (D-01) so this is no longer a blocker.

### Post-Review Refinements (added 2026-04-23 after cross-AI review of plan set)

- **D-10:** PMAT-allow-suppression behavior is the load-bearing assumption
  for D-02/D-03; it MUST be empirically verified in Wave 0 before any
  refactor wave begins. A new Wave 0 task (75-00 Task 4) writes a tiny
  fixture function with cog ≥30, runs `pmat analyze complexity`, adds
  `#[allow(clippy::cognitive_complexity)]` per the D-02 `// Why:` template,
  re-runs PMAT, and records the result in
  `.planning/phases/75-fix-pmat-issues/pmat-allow-behavior.md`.

  **Branch on result:**
  - **(D-10-A) PMAT honors the allow** (function disappears from violations
    after annotation): proceed as planned. P5 (`#[allow]` with `// Why:`)
    remains a valid technique throughout Waves 1-4. D-03 still applies
    (cap 50 even with allow).
  - **(D-10-B) PMAT IGNORES the allow** (function still flagged after
    annotation): P5 is REMOVED from the refactor toolkit. All hotspot
    functions must REDUCE to ≤25 by extraction. D-02's `// Why:` template
    is retained for documentation hygiene only (it provides a paper trail
    even if it doesn't reduce the gate count). D-03's "cap 50" becomes
    irrelevant — every function must hit ≤25. The planner must re-budget
    every later wave's projected count drops (currently inflated under the
    P5-works assumption); the executor surfaces this as a SCOPE EXPANSION
    in 75-00-SUMMARY.md so the user can decide whether to split the phase
    or accept the additional refactor work.

  Plans 75-01 through 75-05 remain authored under the (D-10-A) "allow works"
  optimistic assumption; if Wave 0 returns (D-10-B), the executor pauses
  Wave 1 and surfaces a re-plan request via `/gsd-plan-phase 75 --gaps`.

- **D-11:** Badge/gate command alignment is mandatory for the phase to
  satisfy its stated goal. The README badge is set by
  `.github/workflows/quality-badges.yml` line ~70-82 running
  `pmat quality-gate --fail-on-violation` (no `--checks` filter). The
  Wave 5 CI gate runs `pmat quality-gate --fail-on-violation --checks
  complexity`. If `--checks complexity` exits 0 but bare
  `--fail-on-violation` exits non-zero (because duplicates/SATD/sections/
  entropy still fail), CI passes but the BADGE STAYS RED — phase goal
  unmet.

  A new Wave 0 verification task (75-00 Task 5) MUST run BOTH commands and
  determine which dimensions currently fail the bare gate. Branch on
  result:
  - **(D-11-A) only complexity currently fails** (other dimensions pass or
    are not failure-counted): the existing plan works. After complexity
    hits 0, `--fail-on-violation` will exit 0 and the badge flips. No
    `quality-badges.yml` edit is needed; Wave 5 only edits `ci.yml`.
  - **(D-11-B) other dimensions also fail today** (e.g. SATD count
    contributes to non-zero exit): Wave 5 Task 5-01 MUST ALSO update
    `quality-badges.yml` so the badge command matches the CI gate command.
    Two equivalent fix shapes:
    - Both badge and CI gate use `--checks complexity` (the simplest fix;
      the badge then reflects only complexity, which matches D-01's
      "complexity is the gating dimension")
    - OR both use a path filter (e.g. `--include 'src/**,...'`) consistent
      with the Wave 0 D-09 spike result

  Plan 75-05 is authored to handle BOTH branches; Wave 0 Task 5 narrows
  to one. The phase goal is satisfied only when the README displays
  `Quality Gate: passing` — not just when CI is green.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### PMAT and Quality Gates
- `.github/workflows/quality-badges.yml` — Defines the badge generation logic.
  The Quality Gate badge is set by `pmat quality-gate --fail-on-violation`
  exit code. Complexity, TDG, and Tech Debt badges have their own thresholds.
  Any CI gating change for D-07 modifies (or adds a sibling to) this file.
- `.pmat/project.toml` — Existing PMAT project config (currently records
  version 3.11.1 and last_compliance_check). Path exclusions for D-05 likely
  go here or in a sibling `.pmatignore` — agents should verify against
  `pmat --help` for the version actually installed.
- Commit `28db59aa` (`docs(75): add phase — Fix PMAT quality-gate debt`) —
  Records the baseline counts and hotspot directories. Authoritative source
  for what "fixing PMAT" means in this phase.

### Project Standards
- `CLAUDE.md` (project root) — Toyota Way zero-tolerance policy, the
  "complexity ≤25 per function" target that PMAT enforces, and the SATD
  policy (D-04 reconciles with this). Also defines `make quality-gate` as the
  pre-commit gate.
- `Makefile` — Defines `make quality-gate`, `make lint`, etc. Any new CI gate
  added in D-07 should be a thin wrapper around an existing make target where
  possible, not a parallel command path.

### Prior Phase Patterns
- `.planning/phases/74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token/74-PLAN.md`
  files (74-01, 74-02, 74-03) — Reference for plan structure, wave numbering,
  and acceptance-criteria format. Phase 74 used 3 waves with parallel files,
  which matches the wave structure decided in D-06.
- `.planning/phases/73-typed-client-helpers-list-all-pagination-parity-client-01/73-PLAN.md`
  files — Same reference for plan size and complexity.

### Code Hotspots (read first when planning each wave)
- `src/server/streamable_http_server.rs` — Wave 1 target. Big file, likely
  many >25 complexity functions in protocol dispatch.
- `pmcp-macros/` — Wave 1 target. Macro expansion code is naturally complex;
  expect some `#[allow]` with justification (D-02).
- `cargo-pmcp/src/pentest/` — Wave 2 target. Newer code; check for SATDs
  added during rapid development (D-04).
- `cargo-pmcp/src/deployment/` — Wave 2 target.
- `crates/pmcp-code-mode/` — Wave 3 target. Largest hotspot; AST/parser code
  likely irreducible — heavy `#[allow]` use expected, all bounded by D-03.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `make quality-gate` — Already runs fmt, clippy (pedantic + nursery), build,
  test, and audit. Phase 75 should leverage this as the local check before
  PMAT-specific work; only the CI gate (D-07) is new infrastructure.
- `.github/workflows/quality-badges.yml` — Existing workflow already installs
  PMAT and runs the gate command for badge generation. The CI gate (D-07)
  can either extend this workflow with a fail-on-violation job or add a
  sibling workflow that runs on PR events specifically.

### Established Patterns
- **Plan files are large (50–90KB) and detailed** — Phase 73/74 plans set the
  pattern. Phase 75 plans should match: each plan owns one wave with
  per-function task breakdown for the complexity work.
- **Atomic commits per task** — Project standard from CLAUDE.md and prior
  phase artifacts. Each refactored function should be its own commit so
  regressions can be bisected per-function.
- **Cargo workspace structure** — Root crate is `pmcp`; workspace members are
  `pmcp-macros`, `crates/mcp-tester`, `crates/mcp-preview`, `cargo-pmcp`,
  plus newer `crates/pmcp-code-mode`. Touching `pmcp-macros` rebuilds
  downstream — sequencing matters.

### Integration Points
- New CI gate workflow lives in `.github/workflows/`. Existing `quality-badges.yml`
  is the closest analog.
- PMAT config exclusions for D-05 land in `.pmat/project.toml` or a
  `.pmatignore`-style file (planner verifies against installed PMAT version).
- GitHub issues filed for SATDs (D-04) go in `paiml/rust-mcp-sdk`. The
  remaining code comment should reference the issue number, matching how the
  rest of the codebase tracks deferred work.

</code_context>

<specifics>
## Specific Ideas

- The Quality Gate badge is the single most visible signal — it's at the top
  of the README. The user explicitly framed this phase as "restore the
  badge", not "perfect the codebase". Plans should optimize for the badge
  flipping green as early as possible (likely after wave 1 or 2), not for
  reaching theoretical zero across all dimensions.
- Many of the hotspots (`pmcp-code-mode`, `pentest`, `deployment`) are
  newer code. SATDs there are more likely to be active follow-ups (file an
  issue) than obsolete (delete) — bias the D-04 triage accordingly.
- `streamable_http_server.rs` is mature code in `src/`. SATDs there are more
  likely to be obsolete or trivial — bias toward delete or fix-now.
- `pmcp-macros/` complexity violations are likely irreducible (proc-macro
  expansion is naturally branchy). Expect heavy `#[allow]` use bounded by
  D-03's ≤35 / hard cap 50 rule.

</specifics>

<deferred>
## Deferred Ideas

- **Drive SATD/duplicate/entropy/sections to absolute zero** — Out of scope
  for Phase 75 (D-01). If the team wants to chase the perfect PMAT report,
  that's Phase 76+ as separate per-dimension cleanup phases.
- **Add PMAT to pre-commit hook** — D-07 explicitly chose CI-only. If the
  CI gate proves insufficient (regressions still landing because PRs ignore
  red CI), revisit pre-commit integration in a future phase.
- **Raise the cognitive_complexity threshold from 25 to ~35** — Considered as
  an option but rejected. Stays at 25 to keep the CLAUDE.md "complexity ≤25
  per function" promise honest. The `#[allow]` escape hatch (D-02) handles
  the legitimate exceptions.
- **Whole-file rewrites of hotspot files** — Considered as an option but
  rejected. Function-level refactor is lower risk and ships faster. Whole-file
  rewrites can be a future phase if specific files prove unworkable.
- **Sections badge fix (only 2 README sections detected)** — If trivially
  fixed by adding section headers to README during housekeeping, do it.
  Otherwise defer — it's not on the gate path.

</deferred>

---

*Phase: 75-fix-pmat-issues*
*Context gathered: 2026-04-22 via /gsd-discuss-phase*
*Patched 2026-04-23 with D-10 + D-11 from cross-AI review pass*
