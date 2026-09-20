---
phase: 116-auth-hardening-seps
plan: 01
subsystem: infra
tags: [contracts, provable-contracts, pmat, baselines, semver-checks, nextest, wasm32, rustdoc, oauth, rfc9207, rfc8414, sep-2351, sep-2352]

# Dependency graph
requires:
  - phase: 113-stateless-http-multi-round-trip-elicitation
    provides: "D-113-V (the deferred four-file unbounded-read population) and tests/v2_bounded_reads_tripwire.rs, the scanner this plan pointed at those files"
  - phase: 115-json-schema-2020-12-structured-output-caching-hints
    provides: "tests/phase115_contract_bindings.rs — the contracts/binding.yaml resolver, and the `status: planned` convention this plan reuses"
provides:
  - "116-BASELINES.md — the phase's evidence anchor: seven dated, command-recorded phase-base measurements at b2bf9157"
  - "The doc-check ACCEPTED BASELINE DELTA ANCHOR (28 errors, per-file table, zero in this phase's four auth files)"
  - "Three OAuth contract equations authored BEFORE any implementation, discharging CLAUDE.md's contract-first mandate"
  - "Eight `status: planned` bindings 116-15 flips to `implemented`"
  - "The phase's single written PMAT quality-proxy write workflow (clauses a/b/c) every source-touching plan references"
  - "The D-15 pre-fix violation list: 33 unbounded reads + 7 unreviewed accumulations = 40 sites 116-14 must drive to zero"
  - "The standard non-zero-count nextest verification form every plan in this phase cites"
affects: [116-02, 116-04, 116-05, 116-13, 116-14, 116-15, 116-16]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Contract-first: equations + `status: planned` bindings authored in wave 1, flipped to `implemented` in the closing plan"
    - "Accepted-baseline-delta gating: a red gate is bookable as `<= N` against a dated per-file table, never against zero"
    - "Observe-then-fix: a fence is widened only after its failure has been captured verbatim"
    - "Second enumeration, not widening: a new phase's `planned` permission is a separate named constant joined by a helper"

key-files:
  created:
    - .planning/phases/116-auth-hardening-seps/116-BASELINES.md
  modified:
    - contracts/mcp-protocol-sdk-v1.yaml
    - contracts/binding.yaml
    - tests/phase115_contract_bindings.rs

key-decisions:
  - "Contracts are authored into the in-repo contracts/ tree, not ../provable-contracts/contracts/pmcp/ — the latter does not exist on this machine and no gate resolves it"
  - "doc-check is an ACCEPTED BASELINE DELTA gate at 28 errors, not a required-green gate — this resolves the Codex HIGH contradiction in 116-15"
  - "PMAT quality-proxy clause (a) is INACTIVE: pmat 3.15.0 has no mcp-server subcommand and no --enable-quality-proxy flag. Clause (b) is the active enforcement"
  - "116-13 must NOT list Cargo.lock among modified files — it is gitignored at .gitignore:3, correcting a Codex MEDIUM"
  - "D-15 closure is 40 reported sites (33 reads + 7 accumulations), not the 33 D-113-V implies"
  - "The Phase 116 `planned` permission is a SECOND enumeration (PHASE_116_EQUATIONS) joined by planned_is_permitted, not a widening of PHASE_115_EQUATIONS"

patterns-established:
  - "Every phase-end number is a delta against a dated, command-recorded baseline; `[x]` without a citable delta is not bookable (T-116-EV)"
  - "Every nextest verification parses `Summary [...] N tests run` with a leading non-zero digit — a selector matching nothing exits 0 having run nothing"
  - "Negative controls are run for gate edits: the Phase 116 `planned` permission was proven non-blanket by flipping an unrelated binding and observing the failure"

requirements-completed: []

# Metrics
duration: 78min
completed: 2026-08-03
---

# Phase 116 Plan 01: Phase Baselines and Contract-First Authoring Summary

**Three OAuth contract equations and eight `planned` bindings authored before a single `src/` line exists, plus seven dated phase-base measurements at `b2bf9157` — including the doc-check 28-error delta anchor, the `full`-vs-`full,oauth` 0/5/8 A/B that proves `make quality-gate` compiles none of this phase's code, and the OBSERVED 40-site D-15 violation list.**

## Performance

- **Duration:** ~78 min
- **Started:** 2026-08-03T14:55Z
- **Completed:** 2026-08-03T16:13Z
- **Tasks:** 3
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments

- **The contract-first mandate is DISCHARGED, not argued away.** Three equations —
  `oauth_authorization_response_validation` (10 invariants), `oauth_discovery_anchor` (7) and
  `oauth_credential_binding` (7) — authored into `contracts/mcp-protocol-sdk-v1.yaml`, every
  invariant derived from the cited RFC/SEP clause because there is no implementation to transcribe
  from. Eight `status: planned` bindings carry the DESIGNED signatures from each owning plan's
  `<interfaces>` block. Additions only: `git diff --numstat` 126/0 and 225/0, zero removed lines.
- **The measured proof that `make quality-gate` is not evidence for this phase.**
  `cargo nextest list --features full -E 'binary(oauth_dcr_integration)'` selects **0** tests and
  exits **0**; with `full,oauth` it selects **5**. Row (c) — `--features full -E 'binary(/oauth/)'` —
  measured **8**, not RESEARCH's predicted 0, and the 8 come from three unrelated binaries, making it
  a trap rather than a null result.
- **A doc-check anchor a phase-end booking can actually cite:** 28 `^error` lines, exit 2, with a
  per-file distribution table and **zero** errors in this phase's four auth files. This resolves the
  Codex HIGH contradiction ("cannot claim both 'every gate green' and 'doc-check remains red'") by
  naming it an ACCEPTED BASELINE DELTA gate.
- **The D-15 fence was OBSERVED to fail, and it fails in TWO tests, not one.** 33 unbounded whole-body
  reads (reconciling exactly with `D-113-V`'s reviewed column) **plus 7 unreviewed `push_str(`
  accumulation sites that `D-113-V` never mentions**. Closure is 40 sites.
- **Two open review items closed with measurements rather than opinions:** `Cargo.lock` is NOT
  git-tracked (so `116-13` must not list it), and `REQUIRED_FILES`'s base-name form is genuinely
  ambiguous — **nine** tracked repo paths end in `/auth.rs`, two of them under `src/`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Author the OAuth contract equations and their planned bindings** — `ea1d2d68` (docs)
2. **Task 2: Record the phase-base gate measurements and the PMAT write workflow** — `ab9b965e` (docs)
3. **Task 3: Observe the D-15 tripwire failing on the four auth files, then revert** — `5247f0a9` (docs)

## Files Created/Modified

- `.planning/phases/116-auth-hardening-seps/116-BASELINES.md` (**created**, 817 lines) — the phase's
  evidence anchor. Four sections: `## Contract-First Finding`, `## Phase-Base Measurements` (seven
  numbered subsections), `## PMAT Quality-Proxy Write Workflow`, `## D-15 Pre-Fix Violation Baseline`.
- `contracts/mcp-protocol-sdk-v1.yaml` (+225/-0) — three new `equations:` entries at `:492-708`
  (equation count 13 → 16), each with `formula:`, `domain:`, `codomain:`, `invariants:`,
  `preconditions:`, `postconditions:`, `lean_theorem:`.
- `contracts/binding.yaml` (+126/-0) — `# === OAuth Client Hardening (Phase 116) ===` at `:828-952`
  (record count 64 → 72), eight `status: planned` entries with the 116-15 hand-off written into the
  section comment.
- `tests/phase115_contract_bindings.rs` (+45/-9) — **[Rule 3 deviation, see below]** a second
  enumeration `PHASE_116_EQUATIONS` plus a `planned_is_permitted` join, and the matching
  `phase_116_records >= 8` anti-vacuity floor.

## Decisions Made

- **Author into in-repo `contracts/`, not `../provable-contracts/contracts/pmcp/`.** CLAUDE.md names
  the latter; it does not exist on this machine and no gate resolves it. `make comply`
  (`Makefile:842-849`) resolves the in-repo tree. Recorded as a CLAUDE.md documentation gap, not as a
  licence to skip the mandate.
- **`doc-check` is an accepted-baseline-delta gate at 28, with a per-file table.** The bookable claim
  is `^error count <= 28 AND no error attributed to a file this phase touched`. `make quality-gate`
  does NOT chain `doc-check` (`Makefile:673-694`), so the two are independent gates and every plan
  must run both.
- **PMAT quality-proxy clause (a) is inactive; clause (b) is the enforcement.** `pmat` IS installed at
  3.15.0 (the CI-pinned version) but has no `mcp-server` subcommand and no `--enable-quality-proxy`
  flag in any subcommand's help. The mandate's intent is discharged by a per-task
  `pmat quality-gate --fail-on-violation --checks complexity` plus `make lint`'s pedantic/nursery
  clippy set run under `--features full,oauth`.
- **Bound the eight bindings to exactly the eight functions the plan named**, not a larger set, even
  though the equations' formulas also reference `validate_issuer_url`, `same_origin` and
  `CredentialSnapshot::to_bytes`. `116-15` is written to flip *eight* entries; adding more would put
  this plan out of step with its own closing plan.
- **RESEARCH assumption A2 (`make quality-gate` exits 0) is carried verbatim as an OPEN item for
  `116-15`, not re-measured.** The full gate exceeds this plan's budget. Its `lint`, `fmt-check` and
  `comply` sub-gates WERE measured, all exit 0, and are citable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] `status: planned` is rejected by a second binding-file reader the plan did not know about**

- **Found during:** Task 1 (Author the OAuth contract equations and their planned bindings)
- **Issue:** The plan reasoned that `status: planned` was safe because `comply-bindings-check`
  (`Makefile:818-834`) resolves `function:` values only in `contracts/team-servers/binding.yaml` and
  `pmat comply check --path .` is informational per D-07. **Both halves are correct and were
  confirmed. The reasoning was incomplete.** Phase 115 added
  `tests/phase115_contract_bindings.rs` — "the missing resolver", written precisely because before it
  *nothing in this repository read `contracts/binding.yaml` at all* — and its test
  `phase115_contract_bindings_planned_entries_are_scoped_to_phase_115` confines `planned` to a named
  equation list so it cannot become a universal escape hatch. Committing the bindings without this
  fix would have left `make quality-gate`'s `test-all` step RED.
- **Fix:** Exactly the edit that test's own failure message demands ("extend ... deliberately in this
  file — that edit is the conversation this test exists to force"). Added a SECOND enumeration
  `PHASE_116_EQUATIONS` naming the three Phase 116 equations, plus a `planned_is_permitted` helper
  joining the two lists — not a widening of `PHASE_115_EQUATIONS`, and not a predicate over
  `contract:` or a filename, so a fourth equation cannot join by accident. Added the matching
  anti-vacuity floor `phase_116_records >= 8` so deleting the whole Phase 116 section cannot make the
  new permission cover nothing and pass silently. Module doc updated to record why.
- **Files modified:** `tests/phase115_contract_bindings.rs`
- **Verification:** **The failure was OBSERVED before the fix**, naming all eight entries —
  `target/116-verify/phase115_contract_bindings.OBSERVED-RED.log`, `4 passed, 1 failed`. After the
  fix: `5 tests run: 5 passed`. **Negative control:** temporarily flipping the unrelated
  `jsonrpc_framing` / `JSONRPCRequest::validate` binding to `status: planned` still fails with
  "`status: planned` was used outside the enumerated contract-first phases" — so the permission is
  not blanket. The control's edit was reverted and verified with `shasum -a 256 -c` → `OK`.
  `make lint` exit 0 over the edited file.
- **Committed in:** `ea1d2d68` (part of the Task 1 commit — the bindings and the gate extension MUST
  land together, since either alone leaves the suite red)

---

**Total deviations:** 1 auto-fixed (1 × Rule 3).
**Impact on plan:** Necessary for correctness — without it the repository's own test suite would be
red at the end of wave 1. It also strengthens the phase: the `planned` permission is now enumerated
per-phase with a measured negative control, and `116-15` has a written obligation to remove it. No
scope creep: zero `src/` bytes changed, and the only other `tests/` touch (the D-15 measurement) was
reverted byte-for-byte.

## Issues Encountered

- **RESEARCH's row-(c) A/B prediction was wrong (0, measured 8).** Recorded as a correction rather
  than silently adopted. The correction sharpens rather than weakens the conclusion: a plan running
  `binary(/oauth/)` under `--features full` would see 8 tests pass from three unrelated binaries and
  could report "the oauth suite is green" having compiled zero lines of `oauth_dcr_integration`.
- **`--features oauth` alone fails more broadly than RESEARCH recorded.** Under `--all-targets` it is
  9 diagnostics across 4 failing targets, of which RESEARCH's "4 errors in
  `examples/s51_v2_tasks_agent.rs`" is one row. Conclusion unchanged; the exact command is recorded so
  the number is reproducible.
- **A bare `cargo nextest run` (no features) does not compile in this repo** — examples fail on
  feature-gated imports. Every verification in this phase must pass `--features full,oauth`.
- **`D-113-V` under-describes the D-15 job.** Its "widening the fence is SUFFICIENT — the needle set
  needs no change" is true of the READS but silent on the accumulation change detector, which the same
  widening trips with 7 new `push_str(` sites. Recorded prominently for `116-14`.

## User Setup Required

None — no external service configuration required. This plan installed zero packages
(threat `T-116-SC`: the inverse fence at § Phase-Base Measurements item 6 records `Cargo.toml`
byte-identical to `b2bf9157`), so no package-legitimacy checkpoint applies.

## Next Phase Readiness

**Wave 2 (`116-02`, `116-03`) is unblocked.** Every downstream plan now has:

- a dated number to diff against instead of an assertion to repeat (`116-BASELINES.md`);
- one written PMAT write workflow to reference by name rather than restate;
- the exact verification snippet (§ item 7) that cannot report success on a zero-selection run;
- an authored contract whose invariants CONSTRAIN the implementation rather than describe it.

**Carried obligations, each with a named owner:**

| Owner | Obligation |
|---|---|
| `116-02` | `src/error/mod.rs` already carries 1 of the 28 doc-check errors; must not add a second |
| `116-13` | Must NOT list `Cargo.lock` among modified files (gitignored at `.gitignore:3`) |
| `116-14` | Drive **40** sites to zero (33 reads + 7 accumulations); convert `REQUIRED_FILES` to full relative paths AND change the `:128-130` matcher in the same edit; do not grow `WHOLE_BODY_ALLOWLIST` |
| `116-15` | Close RESEARCH A2 (`make quality-gate`); flip the eight bindings to `implemented` after resolving each `function:` by hand (`CredentialKey::new` resolves through a non-unique `fn new` needle); then remove the three equations from `PHASE_116_EQUATIONS` or leave a written reason |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: .planning/phases/116-auth-hardening-seps/116-BASELINES.md   (817 lines, min_lines 90 ✓)
FOUND: contracts/mcp-protocol-sdk-v1.yaml
FOUND: contracts/binding.yaml
FOUND: tests/phase115_contract_bindings.rs
```

Commits claimed, verified in `git log`:

```
FOUND: ea1d2d68  docs(116-01): author the OAuth contract equations ahead of any implementation
FOUND: ab9b965e  docs(116-01): record the phase-base gate measurements and the PMAT write workflow
FOUND: 5247f0a9  docs(116-01): observe the D-15 tripwire on the four auth files, then revert
```

Plan-level verification block:

```
✓ 116-BASELINES.md exists with all four sections and is committed
✓ contracts/*.yaml parse (yaml.safe_load exit 0); make comply exit 0
✓ git status --porcelain src/ tests/ Cargo.toml is EMPTY
✓ every recorded measurement names its exact command
✓ must_haves artifacts: `oauth_authorization_response_validation` present in BOTH contract files
✓ key_links: `b2bf9157` appears 6× in 116-BASELINES.md; `validate_authorization_response` present in contracts/binding.yaml
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-03*
