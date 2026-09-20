---
phase: 115-json-schema-2020-12-structured-output-caching-hints
verified: 2026-08-02T19:10:00Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
re_verification_round_5:
  re_verified: 2026-08-08
  previous_status: human_needed
  summary: >-
    Round 4's `human_needed` rested on TWO claims. Both are now false, one of them because it
    was already false when written.
  human_verification_items_resolved:
    - item: "Triage the round-4 review findings into `deferred-items.md`"
      resolution: ALREADY_DONE_WHEN_WRITTEN
      evidence: >-
        `D-115-AM` exists at `deferred-items.md:1464` — "the round-4 `115-REVIEW.md` findings,
        triaged on the owner's approval" — booked 2026-08-02 by the `/gsd:execute-phase 115`
        orchestrator at phase close-out on the owner's `approved` answer to items 3 and 4 of
        `115-HUMAN-UAT.md`. It covers all 11 findings: CR-01 recorded as CLOSED in `71a44f40`,
        WR-03/WR-01/WR-05/WR-04 each restated individually, and WR-02/WR-06/IN-01..IN-04 booked
        as read. This verification's claim that "no `D-115-AM` exists" was true at the moment
        the ledger was last read and false by the time the report was committed — the booking
        landed after it. A record-ordering artifact, not a missing decision.
    - item: "Decide the disposition of WR-03 (array-position schema descent)"
      resolution: DECIDED_AND_EXECUTED
      evidence: >-
        The owner's 2026-08-02 `approved` selected defer-and-book, recorded as `D-115-AM (2)`.
        On **2026-08-08 the owner re-dispositioned WR-03 to FIX**, which `115-20` executed —
        option (a) of the two this report offered, the gap-closure plan. `D-115-AM (2)` now
        carries a labelled STATUS CHANGE banner; its original text is preserved verbatim per
        this phase's rule on retracted dispositions.
  gap_closed_by_115_20: >-
    The measurement that defined the gap has been INVERTED and re-measured. This report
    recorded that deleting both `Value::Array` arms left `output_validation` 25/25, property
    21/21 and a clean fuzz `cargo check` — the array position was unfenced. With `115-20`
    landed, the same deletion FAILS at three layers: the unit grid fence (`8 of 8` array
    positions, each named), the `normalization_cases()` structural fence (borrow/own wrong on
    the `allOf` row), and corpus seed `16_all_of_embedded_legacy` run in TRUE isolation
    (invariant 5, exit 77, the seed reproduced verbatim in the panic's `Input was:` line). The
    contract's `walk:` clause, POSTCONDITION and embedded-resource invariant now state the
    array position, as do all three `contracts/binding.yaml` note heads. Verbatim failure text
    in `115-20-SUMMARY.md`.
  file_hash_superseded: >-
    `a97f5cb2…3192c`, the `src/server/output_validation.rs` hash rounds 1-4 recorded and
    re-confirmed, NO LONGER MATCHES — `115-20` added a fixture, a case row and a fence to that
    file. The post-115-20 value is
    `1f335399fa05af7bb6ddd31590ebc857e3a2aece458ab6e60d3ffe1f4c6ab6dc`, confirmed identical
    before and after that plan's negative control.
  still_residual: >-
    `D-115-AM` (3) WR-01, (4) WR-05, (5) WR-04 and (6) remain residual and unowned on the
    owner's original `approved` disposition. Only WR-03 was re-dispositioned. This phase is
    `passed` with those bookings standing, not with them closed.
  regressions: []
superseded_re_verification:
  previous_status: human_needed
  previous_score: 4/4
  gaps_closed:
    - "Round-3 review CR-01 (SUBSCHEMA_MAP_KEYWORDS omitted `dependencies`): closed by
       115-16..115-19. `dependencies` is now the sixth entry, established by a re-runnable
       DERIVATION over all 19 jsonschema-0.49.2 meta-schema documents (not a patch of the one
       case a reviewer found), and is byte-identical and ORDER-identical across all three
       restated copies (src/server/output_validation.rs:233-240, tests/property_tests.rs:1050-1057,
       fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:380-387) — INDEPENDENTLY CONFIRMED this session
       by direct read of all three, not by trusting any SUMMARY."
    - "Round-3 review WR-04 / Gap 2 (contract `formula:` equation head stated an unscoped total
       five lines above the correctly-scoped `walk:` clause): 115-19 rescoped the equation head,
       `walk:` clause, name-position invariant and POSTCONDITION to state ONE six-keyword scope,
       and rewrote three `contracts/binding.yaml` note heads. INDEPENDENTLY CONFIRMED by direct
       read of contracts/mcp-protocol-sdk-v1.yaml:243-402 this session: 'anywhere in s' / 'root or
       any depth' as unscoped totals are gone from the equation head; the historical retracted
       wording is preserved as a labelled quotation in the 115-19 SCOPE CORRECTION note, not
       silently deleted."
    - "The 3 negative controls this round required (mirror-drift both directions, lockstep
       removal caught only by the derivation-anchored assertion, extractor non-vacuity) were run
       and are recorded verbatim in 115-16..115-19's SUMMARYs with the failing line numbers
       named. Not re-run in full this session (would require re-mutating shared production
       files); accepted on the strength of the specific line numbers and failure text recorded,
       which is a materially stronger form of evidence than a pass/fail count."
  gaps_remaining:
    - "A NEW finding, not present in round 3: this round's OWN code review (115-REVIEW.md,
       committed at 67b2e8f1, AFTER the 115-16..115-19 commits it reviews) found 1 critical
       (already fixed in 71a44f40, independently confirmed this session) plus 6 warnings and 4
       info findings. NONE of the 6 warnings / 4 info findings have been triaged into
       deferred-items.md yet (confirmed: the ledger's highest entries are D-115-AK/D-115-AL, both
       filed by 115-19 for the ROUND-3 review; no D-115-AM exists for this round's OWN review).
       The centerpiece — WR-03, array-position (`allOf`/`anyOf`/`oneOf`/`prefixItems`) descent
       absent from the contract's SCHEMA POSITION definition and exercised by no test/property
       draw/corpus seed — was INDEPENDENTLY MEASURED this session by deleting both `Value::Array`
       arms and re-running the suite: 25/25 unit tests, 21/21 property tests and a clean fuzz
       `cargo check` all still passed, confirming the reviewer's claim exactly. Going one step
       further than the review: the SHIPPED code (verified unmodified, hash
       a97f5cb2…3192c, restored and re-confirmed) already performs array descent CORRECTLY and
       UNCONDITIONALLY — there is no name-filtering at the array position for a collision to
       hide behind, which is the specific shape that reopened this requirement in rounds 1, 2 and
       CR-01. This verification does NOT reopen SCHM-01 to FAILED on WR-03 — see reasoning below
       and in Human Verification. WR-01 (a genuinely tautological anti-vacuity assertion,
       independently confirmed by direct code read) and WR-05 (the rename-invariance fences run
       only under CI's `--all-features`, never under the mandated local `make quality-gate`,
       independently confirmed by tracing the Makefile) are the two next-most substantive and are
       bundled into the same disposition ask."
  regressions: []
gaps: []
human_verification:
  - test: "Decide the disposition of 115-REVIEW.md (round-4) WR-03: array-position schema descent
      (`allOf`/`anyOf`/`oneOf`/`prefixItems`) is absent from the contract's SCHEMA POSITION
      definition and exercised by no test, property draw or corpus seed, even though the shipped
      code correctly and unconditionally performs it"
    expected: "Either (a) a further gap-closure plan adds a positive `normalization_cases()` row
      exercising array descent, extends the contract `walk:` clause with the array rule already
      stated as rule 4 in the module rustdoc, and mirrors it into the POSTCONDITION and the three
      `binding.yaml` note heads (115-REVIEW.md's own suggested fix), or (b) the finding is
      formally booked to `deferred-items.md` as `D-115-AM`, continuing the ledger's own numbering
      instruction, with a stated rationale for why the coverage gap is accepted despite the
      code being correct today."
    why_human: "This verification's own measurement (deleting both `Value::Array` arms, re-running
      25/25 + 21/21 + a clean fuzz check) confirms the review's claim exactly: the array position
      is currently unfenced. But it independently diverges from the reviewer's framing on
      severity in one material respect — WR-03 is NOT a defect in currently-shipped behaviour (the
      code already does the right thing, unconditionally, with no name to collide against), which
      is categorically different from the BLOCKER-class defects of rounds 1 and 2 and from CR-01
      (all three involved a demonstrable NAME-dependent or verdict-flipping behaviour difference
      in the code as shipped at the time). Given the extraordinary scrutiny this exact requirement
      has already received across four rounds, and given the owner's own established preference
      (115-HUMAN-UAT.md, both items) to FIX rather than silently defer, this is a severity and
      disposition judgment for the owner to ratify, not one an automated re-run should resolve
      either way."
  - test: "Triage the remaining round-4 review findings (WR-01, WR-02, WR-04, WR-05, WR-06, IN-01,
      IN-02, IN-03, IN-04) into `deferred-items.md`, following this phase's own established
      convention that every review finding is either fixed or explicitly booked"
    expected: "A recorded decision for each of the 8 items, continuing the ledger at `D-115-AM`.
      WR-01 (the anti-vacuity assertion `assert_eq!(examined, containers.len() *
      DATA_ONLY_KEYWORDS.len(), ...)` is a tautology — independently confirmed by direct read of
      src/server/output_validation.rs:1417-1425, it recomputes the loop bound from the loop bound
      and cannot fail for any list contents) and WR-05 (the rename-invariance fences in
      tests/property_tests.rs are gated `feature = \"fuzzing\"`, which is in neither `default` nor
      `full`, and independently confirmed by tracing Makefile:216-231 that `test-unit`/
      `test-property` — the targets `make quality-gate` invokes — pass only `--features \"full\"`,
      never `fuzzing`; the module runs only under CI's `cargo test --all-features`, ci.yml:93) are
      the two with the clearest concrete fixes already stated in the review."
    why_human: "None of these represent a demonstrated bypass in shipped behaviour (independently
      checked for WR-01 and WR-05 this session), but all are real, and this phase's own convention
      — applied without exception across three prior rounds — is that a review finding is never
      left implicit. The round-4 review landed after 115-19 committed, so this is simply the next
      instance of the same triage step every prior round required, not a new class of concern."
---

# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints Verification Report

**Phase Goal:** Schema validation moves to an explicitly-pinned Draft 2020-12, v2 `structuredContent`
accepts any JSON value (relaxing the 2.15 object-only bridge), and the list/read results carry
additive caching hints — all wasm-clean and independent enough to parallelize with the HTTP/Tasks
track.

**Verified:** 2026-08-02
**Status:** human_needed
**Re-verification:** Yes — fourth pass. Round 1 found the root-only normalizer BLOCKER (closed by
115-12/13). Round 2 found that closure's own position-blind residual BLOCKER (closed by 115-14/15).
Round 3 found a new completeness gap (CR-01: `dependencies` omitted from `SUBSCHEMA_MAP_KEYWORDS`)
and a documentation inconsistency (WR-04), both routed to Human Verification; the owner selected FIX
for both on 2026-08-02, recorded in `115-HUMAN-UAT.md`. This round (115-16..115-19) closed both, plus
a source-text drift gate over all three keyword-list copies. This round's OWN code review then found
a fresh critical (already fixed) and six new warnings, the most substantive of which — WR-03, array
descent — this verification independently measures and rules on below.

## Goal Achievement

**SCHM-01's `dependencies` completeness gap (round-3 CR-01) is genuinely closed, independently
re-confirmed — not accepted from any SUMMARY.** All three copies of `SUBSCHEMA_MAP_KEYWORDS` — 
`src/server/output_validation.rs:233-240`, `tests/property_tests.rs:1050-1057`,
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:380-387` — were read directly this session and are
byte-identical, six entries, `dependencies` ordered last with the identical trailing comment. The new
`tests/keyword_list_mirrors.rs` drift gate (featureless, no crate import, confirmed by
`grep -c 'use pmcp'` → 0) compares all three as ordered sequences AND against a derivation-anchored
expectation sourced from none of them; re-run fresh this session: `cargo test --test
keyword_list_mirrors -- --test-threads=1` → **2 passed**.

**The packaging defect this round's own review found (CR-01, critical) is fixed and independently
re-confirmed.** `115-REVIEW.md` reported `tests/keyword_list_mirrors.rs` shipped in the published
crate while the `fuzz/` tree it reads at runtime was excluded, so `cargo test` on the crates.io
tarball would panic. Measured fresh this session: `cargo package --list --allow-dirty | grep -E
'^(tests/(keyword_list_mirrors|property_tests)\.rs|fuzz/)'` returns only `tests/property_tests.rs` —
`tests/keyword_list_mirrors.rs` and every `fuzz/**` path are absent. `Cargo.toml:50-54` carries the
exclusion with the same comment convention as the two neighbouring, identically-shaped exclusions.

**wasm-clean, re-confirmed fresh this session:** `cargo build --target wasm32-unknown-unknown
--no-default-features --features "wasm,validation"` → exit 0.

**The contract's scope statement is now internally consistent, independently re-confirmed.**
`contracts/mcp-protocol-sdk-v1.yaml:243-402` read directly: the `formula:` equation head, the
`walk:` clause, the name-position invariant and the POSTCONDITION all state the same six-keyword
scope; the retracted unscoped-total wording survives only as a labelled historical quotation inside
the `115-19 SCOPE CORRECTION` note, not as live prose. `grep -n 'anywhere in s'` / `'root or any
depth'` over the file → 0 hits each, confirmed.

**A NEW finding from this round's own review, independently measured rather than accepted: WR-03
(array-position schema descent is absent from the contract and untested).** `115-REVIEW.md` states
that both walkers' `Value::Array` arms (`first_legacy_dialect` at `:265`, `pin_dialect_in_place` at
`:325`) — the branch that reaches an embedded schema resource carried by `allOf` / `anyOf` / `oneOf`
/ `prefixItems`, the commonest real-world carrier of a subschema — are (a) absent from the contract's
`SCHEMA POSITION` definition, and (b) exercised by no test, no property draw and no corpus seed, such
that deleting both arms passes the entire suite. **This verification reproduced that claim by direct
measurement, not by trusting the review:** both `Value::Array` arms were commented out (confirmed by
`grep -n "WR-03 MEASUREMENT"` showing exactly two hits, one per function), and:

| Command | Result |
|---|---|
| `cargo test --lib --features "full fuzzing" output_validation -- --test-threads=1` | **25 passed / 0 failed** (unchanged from the array-descent-present baseline) |
| `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)' --test-threads=1` | **21 tests run: 21 passed** (unchanged) |
| `cargo check --manifest-path fuzz/Cargo.toml --bin fuzz_schema_draft_pin` | clean, exit 0 |
| exhaustive grep for `allOf`/`anyOf`/`oneOf`/`prefixItems` across `src/server/output_validation.rs`, `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, `tests/keyword_list_mirrors.rs`, and all 15 tracked corpus seeds | prose only in the reviewed files, zero fixtures — matches the review exactly |

The file was then restored from a pre-edit copy and verified byte-identical to the pre-experiment tree
(`shasum -a 256 -c` → OK against `a97f5cb2…3192c`, the same hash every prior round's SUMMARY records
for this file, and `cargo test --lib --features "full fuzzing" output_validation` re-run at 25/25
against the restored file). **The claim is confirmed: this position is genuinely unfenced.**

**This verification's own verdict, reached independently rather than deferred to the reviewer:
WR-03 does NOT reopen SCHM-01 to FAILED.** The reasoning: rounds 1 and 2, and round-3's CR-01, each
involved a defect that was live in the code AS SHIPPED at the time of discovery — round 1 measured an
actual `(Violates, Conforms)` verdict weakening; round 2 measured an actual `(Conforms, Conforms)`
accept-everything bypass; CR-01 measured a real name-dependent `Cow::Borrowed`-vs-`Owned` behaviour
difference (`dependencies.default` unrewritten vs `dependencies.Inner` rewritten), even though no
verdict flip was reproducible there either. **WR-03 has neither.** The shipped code, confirmed by this
session's own read of `src/server/output_validation.rs:265,325` and by the module's own traversal-rule
rustdoc (`normalize_schema_dialect`, rule 4, `:391`: "At an array node, recurse into every element.
Scalars terminate."), already descends into every array element UNCONDITIONALLY — there is no
author-chosen name for a `DATA_ONLY_KEYWORDS` collision to hide behind at this position, which is the
exact structural shape that reopened this requirement three times before. What is missing is (a) an
explicit statement of the array rule in the FORMAL contract file consulted by `pmat comply check`
(the rule already exists in the source-of-truth module rustdoc), and (b) regression protection: a
future edit that broke array descent would not currently be caught. That is a real, legitimate
coverage-and-documentation gap — not a currently-existing security bypass — and per this phase's own
established bar (a **demonstrated** bypass, not a theoretical one, is what has justified FAILED in
every prior round), it does not meet that bar. It is routed to Human Verification below, together with
the round's remaining un-triaged review findings, because this phase's own convention — every review
finding gets fixed or explicitly booked, never left implicit — has not yet been applied to this
round's own review, exactly the situation round 3 was in when this verification's predecessor ran.

**SCHM-02 and SCHM-03 are unmoved and are NOT reopened, per this task's explicit instruction; their
files are confirmed untouched by this round's diff rather than merely cited.** `git diff --stat
f9fad51c..HEAD -- src/types/tools.rs src/types/caching.rs` → empty (zero hunks). They were re-measured
and VERIFIED at 78/78 in round 3 and that evidence stands.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SCHM-01: Draft 2020-12 explicitly pinned, no `$schema` auto-detect, wasm-clean, SEP-2106-compliant | ✓ VERIFIED | `dependencies` completeness gap (round-3 CR-01) closed and independently re-confirmed: 6-entry `SUBSCHEMA_MAP_KEYWORDS` byte-identical across all 3 copies (direct read), `tests/keyword_list_mirrors.rs` drift gate 2/2 passed (re-run fresh), packaging defect (this round's own review CR-01) fixed and re-confirmed via `cargo package --list`, wasm build exit 0 (re-run fresh). WR-03 (array descent unfenced) independently measured and confirmed real, but ruled NOT to reopen this truth — see Goal Achievement narrative and Human Verification |
| 2 | SCHM-02: v2 `structuredContent` accepts any JSON value; v1 keeps object-shaped behavior | ✓ VERIFIED | Not reopened per task instruction; `src/types/tools.rs` confirmed untouched by this round's diff (`git diff --stat` empty). Round-3's 78/78 (including `structured_tool_output` 20/20) stands |
| 3 | SCHM-03: list/read results carry additive `ttlMs`/`cacheScope` | ✓ VERIFIED | Not reopened per task instruction; `src/types/caching.rs` confirmed untouched by this round's diff (`git diff --stat` empty). Round-3's 78/78 stands |
| 4 | v1 (`2025-11-25`) wire is behaviourally frozen | ✓ VERIFIED | `compile_for_era`'s `Era::V1` arm untouched by this round's diff (file not in the round-4 diff at all); round-3's `v1_lists_golden` byte-identical goldens stand |

**Score:** 4/4 truths verified. Status is `human_needed`, not `passed`, because this round's own code
review produced findings — most substantively WR-03 — that have not yet been triaged (fixed or
explicitly booked to `deferred-items.md`) per this phase's own established convention, and this
verification independently confirmed the most substantive of them (WR-03, WR-01, WR-05) rather than
accepting the review at face value.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/output_validation.rs` (`SUBSCHEMA_MAP_KEYWORDS`, both `*_in_member` dispatchers) | Six-entry derivation-anchored allow-list, position-aware traversal | ✓ VERIFIED | Six entries confirmed by direct read (`:233-240`); `dependencies` present with the D-115-03-C comment; hash `a97f5cb2…3192c` matches every prior round's recorded value |
| `tests/keyword_list_mirrors.rs` | Featureless source-text drift gate over all three copies, comparing ordered sequences and a derivation-anchored expectation; excluded from the published package | ✓ VERIFIED | `cargo test --test keyword_list_mirrors` → 2 passed (re-run fresh); `Cargo.toml:54` exclusion present and confirmed via `cargo package --list` (file absent from package) |
| `tests/property_tests.rs` (`SUBSCHEMA_MAP_KEYWORDS`, `CONTAINER_DRAW`, `arb_container`) | Six-entry mirror, gated equality against the shipped seam, six-way container draw | ✓ VERIFIED | `:1050-1057` byte-identical to `src/`; `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)'` → 21/21 passed (re-run fresh) |
| `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` (`SUBSCHEMA_MAP_KEYWORDS`) | Six-entry mirror, own container literal, corpus seed for the reproduction | ✓ VERIFIED | `:380-387` byte-identical to `src/` and `tests/`; `cargo check --manifest-path fuzz/Cargo.toml --bin fuzz_schema_draft_pin` clean (re-run fresh); seed `15_dependencies_named_default` present, tracked (`git ls-files` count 15) |
| `contracts/mcp-protocol-sdk-v1.yaml` (`output_schema_draft_pin`) | One stated six-keyword scope across the equation head, `walk:` clause, invariants and POSTCONDITION | ✓ VERIFIED (array position gap noted) | Direct read of `:243-402` confirms internal consistency on the map-keyword axis; `SCHEMA POSITION` as literally defined ("descending into every member value…") does not explicitly cover array ELEMENTS, which is WR-03's contract-side half — routed to Human Verification, not scored as a defect in this artifact's stated purpose (map-keyword scope) |
| `.planning/phases/.../deferred-items.md` | Every review finding triaged (fixed or explicitly booked) | ⚠️ PARTIAL | Round-3 review's ten findings (CR-01, WR-01..06, IN-01..03) fully triaged as `D-115-AK`/`D-115-AL` (confirmed by direct read). This round's OWN review (`115-REVIEW.md`, 1 critical + 6 warnings + 4 info) has NOT yet been triaged — confirmed by `grep` showing the ledger's highest entries are still `AK`/`AL`, no `AM` exists — routed to Human Verification |
| `.planning/REQUIREMENTS.md` (SCHM-01 booking) | Fourth booking, amend-not-delete preserved, evidence written after gates ran | ✓ VERIFIED | `[x]`, "FOURTH booking" language present at `:157`, `REOPENED` count unchanged at 1 (amend-not-delete guard intact) — confirmed by direct read |
| `.planning/ROADMAP.md` | Plan-progress bookkeeping only; phase marker left for this verification | ✓ VERIFIED | All four round-4 plan lines `[x]`; Phase 115 marker still `[~]` — confirmed by direct read |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `first_legacy_dialect_in_member` / `pin_dialect_in_member` | `SUBSCHEMA_MAP_KEYWORDS` (6 entries) | member-key dispatch | WIRED | Both walkers consult the same 6-entry list; confirmed by direct read |
| `first_legacy_dialect` / `pin_dialect_in_place` | array elements (`allOf`/`anyOf`/`oneOf`/`prefixItems` carriers) | `Value::Array(items) => items.iter()…` / `.iter_mut()…` | WIRED, but UNFENCED | Confirmed present and CORRECT by direct read and by the deletion experiment (removing it changes shipped behaviour with zero test signal) — this is WR-03's exact shape, routed to Human Verification |
| `tests/keyword_list_mirrors.rs` | all three `SUBSCHEMA_MAP_KEYWORDS`/`DATA_ONLY_KEYWORDS` copies + a derivation-anchored expectation | source-text extraction, no crate import | WIRED | `grep -c 'use pmcp'` → 0; 2/2 tests pass; confirmed running inside `make quality-gate`'s transcript per 115-19-SUMMARY.md (not re-run in full this session — see below) |
| `contracts/binding.yaml` | `output_schema_draft_pin` equation | `function: pin_dialect_in_place` / `first_legacy_dialect` bindings | WIRED | `phase115_contract_bindings` binary cited at 5/5 in 115-19's own re-run; not independently re-run this session (would require re-mutating the contract files) |
| `tests/property_tests.rs` `schema_dialect_normalization_properties` module | `make quality-gate` (local, mandatory per CLAUDE.md) | `#[cfg(all(test, feature = "fuzzing", feature = "validation"))]` | **NOT WIRED locally** | Independently confirmed by tracing `Makefile:216-231`: `test-unit` and `test-property` (the targets `quality-gate` → `test-all` invokes) pass only `--features "full"`, never `fuzzing`. The module — including the mirror-equality gate and the rename-invariance fences repeatedly described as this closure's PRIMARY independent instrument — runs only under CI's `cargo test --all-features` (`ci.yml:93`). This is WR-05, independently confirmed, routed to Human Verification |

### Data-Flow Trace (Level 4)

Not applicable — this phase's artifacts are validation/normalization logic and test/fuzz harnesses, not
UI components rendering fetched state. The equivalent question ("does the widened rule actually reach
the compiled validator") is answered by `compile_2020_12` calling `normalize_schema_dialect` before
`jsonschema::draft202012::new`, unchanged by this round (file not in the round-4 diff).

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Six-entry keyword list, all three copies byte-identical | direct read of all three `SUBSCHEMA_MAP_KEYWORDS` definitions | identical, six entries, `dependencies` last | ✓ PASS |
| Drift gate passes | `cargo test --test keyword_list_mirrors -- --test-threads=1` | 2 passed | ✓ PASS |
| Packaging fix (this round's review CR-01) | `cargo package --list --allow-dirty \| grep -E '^(tests/(keyword_list_mirrors\|property_tests)\.rs\|fuzz/)'` | only `tests/property_tests.rs`; the drift-gate test and all `fuzz/**` absent | ✓ PASS |
| Contract internally consistent on the map-keyword axis | `grep -n 'anywhere in s'` / `'root or any depth'` over `contracts/mcp-protocol-sdk-v1.yaml` | 0 hits each | ✓ PASS |
| WR-03 reproduction: array descent is unfenced | both `Value::Array` arms in `src/server/output_validation.rs` commented out; `cargo test --lib --features "full fuzzing" output_validation`, `cargo nextest -E 'binary(property_tests)'`, `cargo check --manifest-path fuzz/Cargo.toml` | 25/25, 21/21, clean — suite fully green with the array-descent branch DISABLED | ✓ PASS (confirms the review's claim; file restored and hash-verified afterward, `a97f5cb2…3192c`) |
| WR-01 tautology confirmed | direct read of `src/server/output_validation.rs:1417-1425` | `assert_eq!(examined, containers.len() * DATA_ONLY_KEYWORDS.len(), …)` — recomputes the loop bound from the loop bound | ✓ PASS (confirms the review's claim) |
| WR-05 local-gate blindness confirmed | trace of `Makefile:216-231` (`test-unit`, `test-property` use `--features "full"`) vs `Cargo.toml:204-205,243` (`fuzzing` in neither `default` nor `full`) vs `ci.yml:93` (`--all-features`) | local gate never compiles the `fuzzing`-gated module; CI does | ✓ PASS (confirms the review's claim) |
| SCHM-02/03 files untouched by this round | `git diff --stat f9fad51c..HEAD -- src/types/tools.rs src/types/caching.rs` | empty | ✓ PASS |
| wasm-clean | `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` | exit 0 | ✓ PASS |
| Ledger hygiene, this round's review not yet triaged | `grep -n "^## D-115-A[HIJKLM]" deferred-items.md` | highest entries are `AK`/`AL` (round-3's review); no `AM` exists yet | confirms the process gap named above |

### Probe Execution

SKIPPED — no `scripts/*/tests/probe-*.sh` files exist in this repository and neither this round's
plans nor its verification criteria reference probe-based verification. This phase's runnable evidence
is the cargo test/nextest/wasm-build commands above, independently re-run this session where the
mutation was safe to perform and restore.

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SCHM-01 | 115-01, 03, 08-15, 16, 17, 18, 19 | Draft 2020-12 pin, no `$schema` auto-detect, wasm-clean, SEP-2106 | ✓ SATISFIED | Fourth booking `[x]`; `dependencies` completeness gap closed and independently re-confirmed; this round's own review's residual findings (WR-03 primarily) confirmed real but not bypass-class — routed to Human Verification rather than blocking |
| SCHM-02 | 115-01, 03, 04, 09, 10, 11 | v2 non-object `structuredContent`, v1 frozen | ✓ SATISFIED | Not reopened; files confirmed untouched by this round's diff |
| SCHM-03 | 115-01, 02, 05-11 | Additive `ttlMs`/`cacheScope` on six cacheable results | ✓ SATISFIED | Not reopened; files confirmed untouched by this round's diff |

No orphaned requirements: `.planning/REQUIREMENTS.md`'s traceability table maps only SCHM-01/02/03 to
Phase 115, and all nineteen plans (including the four round-4 gap-closure plans) declare
`requirements: [SCHM-01, SCHM-02, SCHM-03]` or a subset.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/server/output_validation.rs` | 1417-1425 | `assert_eq!(examined, containers.len() * DATA_ONLY_KEYWORDS.len(), …)` is a tautology — recomputes the loop bound from the loop bound and cannot fail for any content of either list | ⚠️ Warning (independently confirmed by direct read; this round's own review WR-01) | Does not affect the fence's real work (the `violations.is_empty()` assertion, which IS substantive), but means the anti-vacuity guard would silently pass a zero-pair sweep if `DATA_ONLY_KEYWORDS` were ever emptied. Routed to Human Verification |
| `contracts/mcp-protocol-sdk-v1.yaml` | 249-261, 324-329 | `SCHEMA POSITION` is defined in terms of descending into "member value" (object members); array ELEMENTS are not named as a schema position, even though the shipped code descends into them | ⚠️ Warning (independently confirmed real; not a demonstrated bypass — see Goal Achievement) | This round's own review WR-03; routed to Human Verification, not scored as a blocker |
| `tests/property_tests.rs` | 953 | `schema_dialect_normalization_properties` module gated `feature = "fuzzing"`, which never runs under the locally-mandated `make quality-gate` | ⚠️ Warning (independently confirmed by Makefile trace; this round's own review WR-05) | The rename-invariance fences this closure repeatedly describes as its primary independent instrument run only in CI, not in the mandated local gate. Routed to Human Verification |
| `.planning/phases/.../deferred-items.md` | — | This round's own code review (1 critical, fixed; 6 warnings; 4 info) has not yet been triaged into the ledger | ⚠️ Warning (process, not code) | Breaks this phase's own established "every finding gets fixed or explicitly booked" convention for the second time in four rounds (round 3 had the same gap, closed by the owner choosing FIX for both items) |
| (round-4-touched files) | — | `TBD`/`FIXME`/`XXX` scan over `src/server/output_validation.rs`, `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, `tests/keyword_list_mirrors.rs`, `contracts/mcp-protocol-sdk-v1.yaml`, `contracts/binding.yaml` | ℹ️ Info | Zero matches — no debt-marker gate violation |

### Human Verification Required

Neither item below is a failed truth. Both are real, independently-confirmed findings from this
round's own code review that have not yet been formally triaged (fixed or explicitly deferred), which
repeats — for the second time in four rounds — the process gap this phase's own convention exists to
prevent.

### 1. Decide the disposition of WR-03 (array-position schema descent is unfenced)

**Test:** Review this report's independent reproduction (deleting both `Value::Array` arms leaves the
entire suite green: 25/25 unit, 21/21 property, clean fuzz check) alongside `115-REVIEW.md`'s WR-03,
then decide whether to (a) accept a further gap-closure plan that adds a positive
`normalization_cases()` row for the array position and extends the contract's `walk:` clause /
POSTCONDITION / three `binding.yaml` note heads with the array rule (the fix `115-REVIEW.md` itself
proposes, and the same rule already stated as rule 4 in the module rustdoc), or (b) formally book the
finding to `deferred-items.md` as `D-115-AM`, recording that the shipped code is independently
confirmed correct at this position today and that the gap is coverage/documentation only.

**Expected:** A recorded decision — either a new gap-closure plan, or a `deferred-items.md` entry with
an owner or an explicit "unowned" marker.

**Why human:** This verification's own measurement agrees with the reviewer's claim exactly, but
diverges on severity: unlike every prior BLOCKER on this requirement, the array position has no
name-collision surface (the walk descends into every element unconditionally), so the specific failure
shape that reopened SCHM-01 three times cannot occur here. Given the history of this exact requirement
and the owner's established preference to fix rather than defer, the severity call belongs to the
owner.

### 2. Triage the remaining round-4 review findings

**Test:** Review `115-REVIEW.md` WR-01, WR-02, WR-04, WR-05, WR-06, IN-01, IN-02, IN-03, IN-04 and
record a disposition for each in `deferred-items.md`, continuing the ledger at `D-115-AM`. WR-01 (the
tautological anti-vacuity assertion) and WR-05 (the rename-invariance fences never run under the local
`make quality-gate`) were independently confirmed this session and have concrete fixes already stated
in the review.

**Expected:** Each finding fixed or explicitly booked, matching the bar `D-115-AK`/`D-115-AL` set for
the round-3 review.

**Why human:** Process convention, not a code defect — the round-4 review landed after 115-19
committed, so this is the same triage step every prior round required, applied to this round's own
review.

### Gaps Summary

**No BLOCKER this round.** The `dependencies` completeness gap (round-3 CR-01) is genuinely closed and
independently re-confirmed: all three `SUBSCHEMA_MAP_KEYWORDS` copies are byte-identical, a new
featureless source-text drift gate holds them consistent, the packaging defect this round's own review
found is fixed and confirmed via `cargo package --list`, and the contract's map-keyword scope is now
internally consistent. wasm-clean and the requirement's booking are both re-confirmed fresh.

**One substantive new finding, independently measured and ruled non-blocking.** WR-03 (array-position
descent absent from the contract and untested) is real — confirmed by deleting the code path and
observing the entire suite stay green — but the shipped code is independently confirmed CORRECT at
that position today, and the position has no name-collision surface, which is what distinguishes it
from every prior BLOCKER on this requirement. It is routed to Human Verification rather than scored as
FAILED.

**A process gap repeats.** This round's own code review (landed after 115-16..19 committed) has not
yet been triaged into `deferred-items.md`, the same situation round 3 was in. Two of its findings
(WR-01, WR-05) were independently confirmed this session as real but non-blocking; the phase's own
convention — every finding fixed or explicitly booked — has not yet been applied to them.

**SCHM-02 and SCHM-03 remain genuinely achieved**, not reopened per this task's explicit instruction,
and their files are confirmed untouched by this round's diff rather than merely cited from a prior
report.

---

_Verified: 2026-08-02_
_Verifier: Claude (gsd-verifier)_
