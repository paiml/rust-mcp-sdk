# Phase 118: Conformance Against the Official Suite - Context

**Gathered:** 2026-08-09
**Status:** Ready for planning

<domain>
## Phase Boundary

The dual-version claim is validated **by construction**: the official
`@modelcontextprotocol/conformance` suite and the extended Phase-109 Rust harness both run
against what the dual-version binary actually does, with v1 fixtures kept green and the
deprecated capabilities verified still-functional under v2.

Three requirements:

- **CONF-01** — the official suite, pinned, runs in CI against a dual-version pmcp server
  example over real HTTP.
- **CONF-02** — the Phase-109 Rust harness gains era-v2 fixtures while v1 fixtures stay green,
  verified with a **dev-dependency-free build** so feature unification cannot produce a false green.
- **CONF-03** — deprecated Roots/Sampling/Logging remain fully functional under v2 negotiation
  (advisory-only, 12-month window).

This phase runs LAST, over the union of 112–117. It **adds no protocol behaviour**. If a plan
finds itself changing what the server does rather than what CI measures, that is out of scope.

**The through-line:** one mechanism — the era matrix (D-07) — discharges CONF-02 and CONF-03
together. Do not build a second parallel mechanism for CONF-03.

</domain>

<decisions>
## Implementation Decisions

### Official-suite CI integration (CONF-01)

- **D-01: A committed `package.json` + `package-lock.json` in a subdirectory, run with `npm ci`.**
  This is the first Node manifest in a Rust repo, accepted deliberately. Chosen over `npx` because
  lockfile integrity hashes give commit-level reproducibility and survive a yanked or re-published
  version; over a git submodule because submodule UX cost buys nothing `npm ci` does not already
  give; and over a container because the rest of this repo's CI carries no image-build pipeline.
  **Ledger context:** `project_ci_purity_gate_unpinned_tooling_drift` records the Purity Gate
  bit-rotting from exactly this failure mode (an unpinned cargo-deny CLI plus a gitignored
  `Cargo.lock`). Pin it or it rots.

  **Two traps the planner MUST handle:** (a) the Node manifest must be EXCLUDED from the published
  crate — `115-REVIEW.md` CR-01 was precisely this defect, a test file that shipped in the tarball
  and panicked, caught only by `cargo package --list --allow-dirty`; verify the same way. (b) The
  `package-content` and `Purity Gate` CI jobs both inspect the tree — confirm neither trips on a
  Node manifest before declaring the job green.

- **D-02: The job is BLOCKING and wired into `gate` in all THREE places** — `needs:`, the `env:`
  binding, and the `if` chain. Phase 117's `v1-severance` job is the exact precedent to copy, and
  `tests/ci_severance_gate_wiring.rs` is the precedent for proving the wiring structurally rather
  than asserting it. A conformance claim that cannot fail is not a claim.

  **Read this before assuming "in CI" means "gates":** `gate.needs` does NOT currently list
  `security_audit` or `workspace-test`. Both were RED on PR #319 while `gate` was GREEN. Adding a
  job to `ci.yml` does not make it block. See `<deferred>`.

- **D-03: All server-side tests must pass. NO known-fail allowlist.** If a test genuinely does not
  apply, that is a conversation at re-pin time, not a standing exemption. Matches the house stance
  — 117-13 shipped `WHOLE_BODY_ALLOWLIST` as a literally EMPTY const so nothing could hide in it.

- **D-04: The suite runs TWICE against one running server** — once negotiating `2025-11-25`, once
  negotiating `2026-07-28`. Both blocking. This is the only shape that proves the milestone
  headline ("one pmcp server binary transparently serves both"); a single v2 run would leave the
  official suite silent about half the dual claim. Research must confirm the suite accepts a target
  protocol version — if it cannot, that is a plan-blocking finding, not something to work around
  quietly.

### The dual-version target server (CONF-01)

- **D-05: A NEW purpose-built dual-version example**, beside the existing `t04`/`t05`
  streamable-HTTP examples. Chosen over reusing `t05_streamable_http_stateless` because an
  overloaded example lets a change made for a docs or transport reason silently alter what
  conformance measures — and stateless-only may not exercise the v1 session path D-04's matrix
  needs. Chosen over a Shape-A binary because a backend failure would read as a conformance failure.

  **Dual-purpose, deliberately:** this example is also the runnable v2 example DOCS-06 needs in
  Phase 119. Build it so Phase 119 can cite it rather than write a second one.

- **D-06: ONE process, per-request era negotiation.** Start the server once; each suite run
  negotiates its own era via `MCP-Protocol-Version`. Two processes would prove "pmcp can serve v1"
  and "pmcp can serve v2" separately — not that one binary does both, which is the actual claim.
  A single process also catches cross-era state bleed that isolated processes hide.

### Era-v2 fixtures for the Rust harness (CONF-02)

- **D-07: MATRIX — every fixture is replayed under BOTH eras**, with an expected-difference
  baseline recording where the eras legitimately diverge. Chosen over parallel `v1/`+`v2/`
  directories (duplicating 33 fixtures invites exactly the mirror-drift defect 115 fought four
  rounds over) and over an optional per-fixture `era` field (absence is invisible, so v2 coverage
  would never be forced to grow).

  **Reuse, do not reinvent:** 117-08 already built this shape for `mcp-tester` — a reviewable
  14-entry YAML expected-difference baseline with a non-vacuity tripwire, added with no new
  dependency. Read it before designing a new one.

  **The baseline is bidirectional:** an unlisted difference is a finding, AND a listed difference
  that no longer reproduces is also a finding. A baseline that only catches one direction rots.

- **D-08: Rename the fixture format so bare "v2" always means MCP era.** `runner.rs:43` currently
  reads `Fixture schema v2`, and `FixtureKind`'s rustdoc says "a v2 fixture case" — that is format
  revision 2, NOT era v2. Rename the format (e.g. `fixture format rev 2`) and reserve `v2` for the
  era throughout. A mechanical rename now beats downstream agents conflating the two for the rest
  of the milestone.

- **D-09 (from CONF-02, not negotiable): verify with a DEV-DEPENDENCY-FREE build.**
  `project_cargo_feature_severance_false_greens` records the mechanism: `cargo test` sees dev-deps
  and re-enables the feature you are severing, so the run reports `0 tests` and exits 0. Assert a
  **NONZERO test count**, and use `cargo build` (not `cargo test`) for the severance check.
  Related: `project_nextest_selector_binary_not_test` — `-E 'test(/foo/)'` silently selects ZERO
  tests and does not fail. Use `binary(foo)`. This bit Phase 114 seven times.

### Deprecated-capability evidence (CONF-03)

- **D-10: Roots/Sampling/Logging get fixtures IN the matrix**, so D-07 replays them under both eras
  and CONF-03 is discharged by the same mechanism as CONF-02. Chosen over dedicated Rust tests (a
  second mechanism that rots independently) and over relying on the official suite (coverage we do
  not control — a re-pin could drop it and CONF-03 would silently stop being proven).

- **D-11: No runtime signal. The capabilities keep working and say nothing.** Advisory-only
  deprecation over a 12-month window means no behavioural change and no new output; a warn on a
  still-supported capability trains users to ignore warnings and would fire for a year. Deprecation
  is documented, not emitted. Extend `docs/v1-sunset-policy.md` (created by 117-13, which already
  carries a table of items deliberately NOT severed) with the three capabilities and their window.

  Supporting evidence for "no new warn": this session's `oauth_store_wiring` flake shows
  warn-capture assertions carry their own maintenance burden.

### Post-research decisions (2026-08-09, resolved with user)

Research (118-RESEARCH.md) surfaced three plan-blocking open questions. All three were
resolved with the user before planning. These are LOCKED decisions with the same force as
D-01..D-11.

- **D-12 (resolves Open Question 1): CONF-03 asserts reading (a) — capability reachable via
  its v2 mechanism.** Under v2 negotiation, Logging is exercised via the
  `_meta` `io.modelcontextprotocol/logLevel` key, and Sampling/Roots via `InputRequiredResult`
  (SEP-2322). The v1 RPC shapes (`logging/setLevel`, etc.) stay green under **v1** negotiation
  only; under v2 the official suite's expectation (removed methods → 404 + -32601) stands.
  Fixtures in the D-07 matrix encode this via the expected-difference baseline. Reading (b)
  — v1 RPCs answering under v2 — is rejected; it would contradict the official suite's
  `server-stateless` scenario.

- **D-13 (resolves Open Question 2): relax the `Mcp-Name` rule to name-bearing methods only.**
  The user explicitly authorizes this ONE server-behaviour change as an exception to the
  "adds no protocol behaviour" phase boundary: pmcp's v2 header validation
  (`require_three_headers`, `streamable_http_server.rs`) changes from "Mcp-Name present on
  every v2 request" to "Mcp-Name required exactly where the method carries a name, with the
  strict name/body cross-check kept where it is present". This deliberately reverses the
  Phase-113 DRIFT-1 adjudication to align with the spec and the official suite; the rustdoc
  recording the old adjudication must be updated to record the reversal and why.
  **Follow-up (from Open Question 5):** after this relaxation lands, re-measure the v2
  conformance profile before finalizing the example's tool/resource/prompt surface — the
  pre-relaxation numbers (62/91) are a lower bound, not a baseline.

- **D-14 (resolves Open Question 3): D-03's boundary is the suite's SCORED set.** `extension`
  (all 10 Tasks scenarios) and `pending` checks run and report but cannot fail the job — the
  suite's own SEP-1730-derived design. The `-o results/` artifact is uploaded so not-scored
  results stay reviewable at re-pin time; that visibility is what D-03's no-allowlist spirit
  protects. The Tasks extension is NOT implemented in the example.

- **D-15 (adopts research recommendation, Open Question 4): CONF-02's Rust-harness execution
  is wired into the NEW blocking conformance job (or a blocking sibling), not into
  `workspace-test`.** Today no CI job runs `crates/pmcp-team-servers/tests/`; widening
  `workspace-test` would touch the explicitly-deferred `gate.needs` item. The new job owns
  the gate for the fixtures it proves.

### Post-review corrections (locked 2026-08-09 after `/gsd-review --codex --gemini`)

These SUPERSEDE the mechanism (not the intent) of D-06/D-07 where they conflict. Rationale and
source citations: `118-REVIEWS.md`. All three CONF-02/CONF-03 blockers below were confirmed
against repository source, not inferred from plan text.

- **D-16: The era comparison runs on PORTED Phase-117 probe machinery over REAL streamable HTTP,
  both eras on the same transport.** The planned in-process v2 arm is architecturally dead:
  `DuplexTransport` (`crates/pmcp-team-servers/src/transport.rs:47`) implements only
  `send`/`receive`/`close`/`is_connected`/`transport_type`, never overriding
  `supports_negotiated_protocol_version` (trait default `false`,
  `src/shared/transport.rs:351`) nor `send_raw`. `ClientBuilder::build`
  (`src/client/mod.rs:5213`) explicitly warns such a v2 selection is **INERT** — the matrix would
  compare v1 against v1 and report green having measured nothing.

  This is not a new design. D-07 already said "reuse, do not reinvent," and Phase 117 shipped the
  exact machinery in `mcp-tester`; plans 118-03/118-06 copied only the baseline YAML and rebuilt
  observations from fixture pass/fail. **Port both halves:**

  * `crates/mcp-tester/src/era_observations.rs` — `ObservationId`, `ObservedValue`,
    `EraObservations`, `PROBE_REGISTRY`, and `pub async fn observe(target, era) -> EraObservations`.
    Observations come from EXPLICIT PROBE CODE, never inferred from a bool.
  * `crates/mcp-tester/src/era_diff.rs` — `compare_eras`, `DualRunReport`,
    `DifferenceClass::{Unexpected, Missing}`, `EraDelta`, `EraBaseline`, `parse_baseline`,
    `load_default_baseline`.

    **CORRECTION (measured 2026-08-09, `era_diff.rs:551-603`):** an earlier draft of this decision
    claimed `compare_eras` exempts `provisional` entries from `Missing`. It does NOT. The
    `(false, Some(d), _)` arm classifies `Missing` unconditionally; `provisional` is carried only
    as a field on `ClassifiedDifference` and affects RENDERING (`[PROVISIONAL]`) alone. Put the
    exemption in the CONSUMER, not the comparator, and assert it is dead by construction once the
    seeded rows are measured.

  `CaseResult` (`runner.rs:306`) carries only `{case_id, passed, detail}` and MUST NOT be the
  observation substrate — two eras both passing the same expected response emit no observation at
  all. The 33 existing fixtures stay in-process and v1-only as a **regression guard**; they are no
  longer the era-comparison surface.

- **D-17: CONF-03 deprecated capabilities are proved by PROBES, not by fixtures.** The fixture
  grammar supports only `tools_list` and a single `tool_call` — it cannot express a preceding
  `logging/setLevel`, host-handler installation, a server→client `sampling/createMessage` or
  `roots/list` exchange, or an MRTR gather/resend. Under D-16 this problem dissolves: add
  Roots/Sampling/Logging observation IDs to `PROBE_REGISTRY` and prove both eras through the same
  probe surface. **Do not extend the fixture format** — that option is now closed.

- **D-18: `is_name_bearing_method` MUST delegate to `name_bearing_key`, not `logical_name_key`.**
  `logical_name_key` (`src/types/mrtr.rs:297`) covers only tools/prompts/resources;
  `name_bearing_key` (`src/types/mrtr.rs:313`) adds `tasks/get`, `tasks/update`, `tasks/cancel`.
  The SDK's own rustdoc at `mrtr.rs:290-297` names `name_bearing_key` as "the function the
  `Mcp-Name` EMITTER resolves through." As planned, 118-01 would let the client emit `Mcp-Name`
  for `tasks/*` while the server never requires or cross-checks it — an emitter/validator
  asymmetry contradicting D-13's own principle ("required exactly where a method carries a routing
  name"). Add explicit wire tests for all three `tasks/*` methods, and keep at least one literal
  contract test so the property test's oracle is not the predicate under test.

- **D-19: No verification command may mask the exit code of the thing it verifies, and a lint
  enforces it.** Confirmed masked sites: 118-01 T2, 118-02 T2, 118-03 T1/T3, 118-06 T1/T2/T3,
  118-07 T1/T2, 118-09 T3. `cargo test … | tee … | tail` returns `tail`'s status; worse,
  `118-02:222` runs `cargo package … | grep …` and treats `$? -eq 1` as PASS, so a FAILING
  `cargo package` reports success. Use `bash -o pipefail -c '… | tee "$log"'` then assert against
  `$log`, or capture-then-assert. Additionally ship a plan-lint that FAILS when a verification
  command pipes a build/test invocation into another command without `pipefail`, so this cannot
  recur across the rest of the milestone.

- **D-20: Carry the remaining verified review findings.** Each is confirmed and in scope:
  `results/` is not gitignored (`.gitignore:38` has only `test-results/`) and no plan owns
  `.gitignore` — write suite output under `target/conformance-results/` instead; 118-09 T3 is
  self-contradictory (requires documenting why PyYAML was rejected AND requires
  `grep -ciE 'python|pyyaml|yq '` to return 0 in the same file); 118-02 T1 expects
  `git ls-files conformance/` to list three files when Task 1 creates two and `git ls-files` does
  not list untracked files; `engines.node` does not make npm refuse Node 20 without a committed
  `.npmrc` `engine-strict=true`; `npm ci` runs transitive lifecycle scripts (prefer a verified
  `--ignore-scripts`); the suite's known zero-check scenarios must be reconciled ONCE and applied
  identically in 118-04, 118-05 and 118-08; add per-run and job-level timeouts and kill the
  process GROUP (trapping `cargo run`'s PID can orphan the server child); register
  `s54_v2_dual_conformance` as a `[[example]]` with `required-features` (adding `Cargo.toml` to
  118-04's `files_modified`); assert exact counts (`failed == 0`, total 33, exact per-directory)
  rather than the floors 11/6/5/7; reconcile `tests/v2_conformance_pin.rs`'s existing SHA pin with
  the new npm pin; add contract-first updates + `pmat comply check` around the D-13/D-18 behavior
  change; and drop 118-01's FUZZ exemption, which contradicts CLAUDE.md's ALWAYS rule.

  Drop the two review findings that were checked and found WRONG: the `provisional: true` cleanup
  Gemini wanted is already asserted at `118-06:384`, and `include_str!` with an absolute
  `CARGO_MANIFEST_DIR` path is a correct, standard idiom — only the prose at `118-03:217` claiming
  "no absolute path" needs fixing.

### Claude's Discretion

- The subdirectory name and layout for the Node manifest.
- The exact renamed identifier for the fixture format (D-08) — smallest diff that removes the
  ambiguity.
- Job naming, cache keys, and step ordering in `ci.yml`, following the `v1-severance` precedent.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` § `### Phase 118: Conformance Against the Official Suite` (line 2723) —
  goal, dependencies, the three success criteria
- `.planning/REQUIREMENTS.md` lines 924–926 — CONF-01/02/03 verbatim; line 991 carries the
  "Deprecated, not removed — 12-month advisory" decision behind CONF-03

### The Phase-109 Rust harness (CONF-02 / CONF-03)
- `crates/pmcp-team-servers/src/conformance/runner.rs` — the exportable runner. **`:43` reads
  `Fixture schema v2` — that is format revision 2, NOT MCP era v2 (see D-08).** `:180` documents
  the `ConformanceTarget` seam: in-process over `DuplexTransport` and, behind `http`, over the
  wire, "so the same fixtures prove conformance in-process and over the wire (D-19)"
- `crates/pmcp-team-servers/tests/conformance.rs` — the driver; `fixtures_root()` at `:69` resolves
  `contracts/team-servers/fixtures`; note the `regenerate_tools_list_fixtures` test
- `contracts/team-servers/fixtures/` — 33 existing fixtures across 4 reference servers
  (approval-mcp 6, mem-mcp 7, team-fs 12, team-mcp 8). These are the v1 corpus that must stay green

### The expected-difference baseline pattern to reuse (D-07)
- `.planning/phases/117-agents-tester-v1-severability/117-08-PLAN.md` + `117-08-SUMMARY.md` —
  the 14-entry reviewable YAML baseline, its non-vacuity tripwire, and the no-new-dependency
  constraint
- `.planning/phases/117-agents-tester-v1-severability/117-11-PLAN.md` — `--dual-run`, `run_dual`,
  era-observation probes, and the `DualRunReport` shape

### CI wiring precedent (D-02)
- `.github/workflows/ci.yml` — the `gate` job's `needs:` / `env:` / `if` chain, and the
  `v1-severance` job above it as the blocking-job template. **`gate.needs` omits `security_audit`
  and `workspace-test`** — see `<deferred>`
- `tests/ci_severance_gate_wiring.rs` — proves gate wiring structurally from the workflow file
  with a `serde_yaml` tripwire (no undeclared PyYAML); 8/8

### Deprecation surface (CONF-03)
- `docs/v1-sunset-policy.md` — created by 117-13; already carries a table of the seven items
  deliberately NOT severed. D-11 extends it

### Publishing / packaging traps (D-01)
- `Cargo.toml` `exclude` array — `115-REVIEW.md` CR-01 is the precedent: a test file shipped in the
  crates.io tarball and panicked. Verify with `cargo package --list --allow-dirty`

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`run_fixtures` / `assert_conformant` / `ConformanceTarget`** — the harness is already
  exportable and already has BOTH an in-process and an HTTP target. The era matrix (D-07) extends
  a working seam rather than building one.
- **117-08's YAML expected-difference baseline + non-vacuity tripwire** — directly transferable to
  D-07; it already solved "reviewable, no new dependency, cannot pass vacuously".
- **`t04_streamable_http_stateful` / `t05_streamable_http_stateless`** — already launched and
  validated in CI by mcp-tester on ports 8080/8081. The launch-and-probe CI pattern for D-05's new
  example already exists; copy it rather than inventing one.
- **`tests/v2_conformance_pin.rs`** — an existing v2 conformance pin worth reading before adding
  another.

### Established Patterns
- **Blocking CI jobs are proven structurally, not asserted** (`ci_severance_gate_wiring.rs`).
- **Allowlists ship empty or not at all** (117-13's `WHOLE_BODY_ALLOWLIST`).
- **Fences carry their OWN literals**, never derived from the artifact under test — the standing
  `D-115-AI(4)` rule. A conformance fence parameterised by the thing it checks cannot fire.
- **Anti-vacuity counts are hard-coded, not length-derived** (`115-REVIEW.md` WR-01; 115-20 applied
  this). Any "we ran N cases" assertion in the matrix must fail when the corpus shrinks.

### Integration Points
- `ci.yml` → a new conformance job + the three `gate` edits.
- `crates/pmcp-team-servers/src/conformance/runner.rs` → era dimension + baseline loading.
- `contracts/team-servers/fixtures/` → new Roots/Sampling/Logging cases (D-10).
- `examples/` → the new dual-version server (D-05), which Phase 119 will cite for DOCS-06.

</code_context>

<specifics>
## Specific Ideas

- The official suite is currently referenced **nowhere** in the repo and there is **no root
  `package.json`** — CONF-01 is greenfield, not an extension of something existing.
- The MCP 2026-07-28 spec is now final, so CONF-01's "re-pinned after the final spec" is
  actionable in this phase rather than a follow-up.
- The era matrix should make it cheap to answer "what actually differs between the eras for this
  server?" — the baseline is a deliverable a reader consults, not just a gate artifact.

</specifics>

<deferred>
## Deferred Ideas

- **Add `security_audit` and `workspace-test` to `gate.needs`.** Both were RED on PR #319 while
  `gate` was GREEN, so they would have merged silently. Thematically close to this phase (CI telling
  the truth) but concerns jobs unrelated to conformance — a separate capability, so out of scope
  per the phase-boundary rule. Owner unassigned. Recorded in
  `project_pr319_ci_findings`.
- **Root-cause the intermittent `oauth_store_wiring` DCR issuer-change test.** CI record is 3 pass
  / 2 fail on identical code; passes locally at every thread count. The tracing interest-cache and
  thread-migration hypotheses were both DISPROVEN by measurement. Its assertion is now
  self-diagnosing (`bf1c2261`) — read that output on the next failure rather than re-deriving.
  Belongs to Phase 116's surface, not conformance.
- **`SMPL-F1` — actual v1 removal.** A future pmcp 3.0, gated on public-client v2 adoption. v2.5
  only makes removal cheap. Carried forward from 117.

</deferred>

---

*Phase: 118-Conformance Against the Official Suite*
*Context gathered: 2026-08-09*
