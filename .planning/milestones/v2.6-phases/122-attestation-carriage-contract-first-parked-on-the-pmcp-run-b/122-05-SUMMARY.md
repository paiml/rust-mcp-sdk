---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 05
subsystem: infra
tags: [pmcp-package, reference, semver, serde, wire-compat, golden-fixtures, team-package, pinning, supply-chain]

# Dependency graph
requires:
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "122-03's `SubjectVerdict` / `validate_pack_preconditions` / `assemble_manifest` restructuring — this plan's `PinnedRef` field addition had to leave that pack path and its 266-test baseline green"
  - phase: 120
    provides: "`ConfigSlot.config_key` — the `#[serde(default, skip_serializing_if)]` precedent and its two-halves compatibility rustdoc, copied structurally here"
provides:
  - "`PinnedRef.resolved_from: Option<semver::VersionReq>` — a pin records the range it was resolved FROM, additively on the wire"
  - "`TeamPackage::pinned_components() -> Result<Vec<&PinnedRef>>` — the four-surface generalization of `WorkflowManifest`'s guard"
  - "`TeamPackage::validate_all_pinned() -> Result<()>` — the thin boolean guard 122-07's Gate A calls"
  - "A private `TeamPackage::component_refs()` fixing the traversal order (entry_point, members[].agent, built_in_servers, finalizer_agents)"
  - "A measured proof that an additive `Option` field moves ZERO golden-fixture bytes while a `Some` value DOES move the manifest digest"
affects: [122-07, 122-08, 123]

actuals:
  tokens: 31000
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "An additive serialized field states BOTH compatibility halves (wire-additive, source-breaking) with a MEASURED construction-site inventory, not a hand-waved one"
    - "An `Option` whose absence is ambiguous resolves that ambiguity in its own rustdoc, naming the consumer phase and the producer obligation, rather than leaving it for a later reader to guess"
    - "A compatibility claim is paired with its complement: `None` moves nothing (existing pinned constants still match) AND `Some` moves the digest — either alone is satisfiable by a field the digest path ignores"
    - "A multi-surface traversal is proven per-surface: one single-unresolved-surface test per surface, each asserting THAT component's name in the error, so a short-circuiting traversal fails more than one test"

key-files:
  created: []
  modified:
    - crates/pmcp-package/src/reference.rs
    - crates/pmcp-package/src/package/team.rs
    - crates/pmcp-package/src/package/workflow.rs
    - crates/pmcp-package/src/oci/unpack.rs
    - crates/pmcp-package/tests/digest_stability.rs
    - crates/pmcp-team-servers/src/team/identity.rs

key-decisions:
  - "`resolved_from` is placed LAST in `PinnedRef`'s field order, so the five pre-existing serialized keys keep their emission order and the only possible wire change is an appended key"
  - "The `None` ambiguity is resolved as ACCEPTED rather than removed: no schema-version discriminator is carried, because the crate is 0.x and the package tree's standing position is to break freely instead of shipping compatibility shims"
  - "The rustdoc records a PRODUCER obligation as well as the consumer one — a range-resolving producer that writes `None` is indistinguishable from an old package and destroys the signal — because only stating the consumer half leaves the field silently forgeable by omission"
  - "`TeamPackage`'s traversal order is FIXED, not sorted: a stable chain is cheaper than a sort and the caller needs identification, not ordering. The order is stated in the rustdoc and asserted by a test, so it is a contract rather than an accident"
  - "The one-level depth limit is written into the ERROR TEXT, not only the rustdoc — a caller who never reads the docs still cannot mistake a passing team for a transitively resolved one"
  - "A FOURTH single-surface test (`finalizer_agents`) was added beyond the plan's three, because `finalizer_agents` is the last link in the chain and therefore the easiest to omit — and it is the test the falsifiability control targets"

patterns-established:
  - "TDD in Rust where the RED is a COMPILE failure: tests are written first and the compile error is captured verbatim as the RED evidence, then implementation lands in one green commit — every commit in history compiles, which CLAUDE.md's build-verification gate requires"
  - "When a background command's captured output carries a truncation marker, its reported exit code is re-measured through an explicit `MAKE_..._EXIT=$?` sentinel written to a file, because a truncated log cannot show which legs ran"

requirements-completed: [PKGX-01]

coverage:
  - id: D1
    description: "A `PinnedRef` records BOTH what was asked for and what was chosen — a `Some(range)` emits the declared range and round-trips, and a `None` emits no key at all"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/reference.rs#a_pin_with_no_resolved_range_emits_exactly_the_original_five_keys"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/reference.rs#a_pin_carrying_the_range_it_resolved_emits_it_and_round_trips"
        status: pass
    human_judgment: false
  - id: D2
    description: "Pin JSON written before the field existed still deserializes, yielding `resolved_from: None` — asserted against a hand-written five-key JSON literal, which is what an old package holds on disk"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/reference.rs#pin_json_written_before_resolved_from_existed_deserializes_to_none"
        status: pass
    human_judgment: false
  - id: D3
    description: "The additive field moves NO checked-in golden fixture byte and NO pinned digest constant — measured, with the existing tests passing UNEDITED as the proof"
    requirement: PKGX-01
    verification:
      - kind: other
        ref: "git diff --stat fbb1970d HEAD -- crates/pmcp-package/tests/golden_fixtures/ -> NO changed files"
        status: pass
      - kind: other
        ref: "git diff --numstat fbb1970d HEAD -- crates/pmcp-package/tests/digest_stability.rs -> 54 additions, 0 deletions (no constant, no existing test body changed)"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/digest_stability.rs — all four `*_fixture_digest_matches_pinned_wire_freeze_constant` and all four `*_canonical_bytes_match_checked_in_snapshot` tests pass unmodified"
        status: pass
    human_judgment: false
  - id: D4
    description: "A recorded range genuinely participates in package identity — the same fixture with `Some(range)` yields a DIFFERENT manifest digest from the `None` variant, so the field is not cosmetic metadata strippable for free"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/digest_stability.rs#recording_the_range_a_pin_resolved_changes_the_manifest_digest"
        status: pass
    human_judgment: false
  - id: D5
    description: "`TeamPackage` can be asked whether ALL FOUR of its reference surfaces are pinned, and the traversal provably reaches every one of them"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/package/team.rs#pinned_components_returns_every_pin_from_all_four_surfaces"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/package/team.rs#a_range_entry_point_fails_and_names_that_component"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/package/team.rs#a_range_built_in_server_fails_and_names_that_component"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/package/team.rs#a_range_member_agent_fails_and_names_that_component"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/package/team.rs#a_range_finalizer_agent_fails_and_names_that_component"
        status: pass
      - kind: other
        ref: "Falsifiability control (below): finalizer_agents link removed from the chain -> 2 tests fail, team wrongly passes; reverted"
        status: pass
    human_judgment: false
  - id: D6
    description: "D-09's one-level depth limit is visible to a CALLER, not only to a reader of the source — it is inside the `InvalidReference` reason string as well as both methods' rustdoc"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/package/team.rs#the_error_states_the_one_level_depth_limit"
        status: pass
    human_judgment: false
  - id: D7
    description: "The guard reuses `PackageError::InvalidReference` — no new error variant was added for this case (D-09)"
    requirement: PKGX-01
    verification:
      - kind: other
        ref: "git diff fbb1970d HEAD -- crates/pmcp-package/src/error.rs -> file absent from the diff, 0 lines changed"
        status: pass
    human_judgment: false
  - id: D8
    description: "The returned pin order is deterministic across runs and across independently constructed equal values"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/package/team.rs#the_returned_pin_order_is_deterministic_across_runs"
        status: pass
    human_judgment: false
  - id: D9
    description: "The full aggregate `make quality-gate` — the leg plans 122-02 and 122-03 both had to leave unrun on disk exhaustion"
    verification:
      - kind: other
        ref: "make quality-gate -> MAKE_QUALITY_GATE_EXIT=0 (explicit sentinel, 11262-line log, 473 passing test suites, final `pmcp-package fmt/clippy/test OK` leg marker present)"
        status: pass
    human_judgment: false

duration: 29 min
completed: 2026-08-25
status: complete
---

# Phase 122 Plan 05: The Bounded Format Addition and the Team Guard Summary

**`PinnedRef` now keeps the range it resolved from (Cargo's `Cargo.toml`-plus-`Cargo.lock` model) without moving a single checked-in fixture byte, and `TeamPackage` gained `WorkflowManifest`'s pinned-components guard generalized to all four of its reference surfaces, with D-09's one-level depth limit written into the error a caller actually sees.**

## Performance

- **Duration:** 29 min
- **Tasks:** 2
- **Files modified:** 6
- **Commits:** 2 task commits (+ this metadata commit)

## Accomplishments

- **The additive-field claim is MEASURED on both sides, not asserted on one.** A `None` moves nothing — proven by all four pinned wire-freeze digests and all four canonical-byte snapshots continuing to pass **unedited**, with a zero-file diff under `golden_fixtures/` and an additions-only (54/0) diff in `digest_stability.rs`. A `Some(range)` moves the manifest digest — proven by a new test. Either half alone would have been satisfiable by a field the digest path ignores entirely, which is exactly the shape an attacker could strip for free.
- **The `None` ambiguity was resolved deliberately and written down, including the half that is easy to forget.** The rustdoc states the consumer obligation (Phase 123 must read `None` as *cannot report*, never as *no skew*) **and** the producer obligation (a producer that resolved a range MUST record it, because a `None` from a range-resolving producer is indistinguishable from an old package). Only stating the consumer half leaves the signal silently defeatable by omission.
- **The construction-site inventory in the rustdoc was re-measured, not copied from the plan.** The plan's count of EIGHT is correct; two of its recorded line numbers were stale (`unpack.rs` 626/632 -> the actual 845/851, moved by 122-03's work). The grep was authoritative, as the plan instructed.
- **The team guard's traversal is proven per-surface, not in aggregate.** Four single-unresolved-surface tests, each asserting *that specific component's name* appears in the error. A traversal that stopped at `entry_point` would fail three of them; one that read only `members[0]` would fail the member test, which deliberately breaks the SECOND member.
- **The depth limit is in the error string, not just the docs.** D-09 said the limit must be *stated rather than discovered*; a rustdoc is discovered only by someone who goes looking. A caller who never opens the docs still reads "this guard is one level deep ... cannot see inside a pinned component's own references" in the failure they get.
- **The aggregate `make quality-gate` was RUN and PASSES (exit 0).** This is the leg plans 122-02 and 122-03 both had to leave unrun on machine-level disk exhaustion. ~28 GiB free and sole occupancy of the volume made it possible.

## Task Commits

1. **Task 1: `PinnedRef` records the range it resolved from** — `81483d47` (feat)
2. **Task 2: `TeamPackage` gains the pinned-components guard** — `c94fafd4` (feat)

## What plan 122-07 should call, and with what

Recorded explicitly because 122-07 owns Gate A on the team pack path and must not re-derive this:

```rust
// crates/pmcp-package/src/package/team.rs — both pub, both on `impl TeamPackage`

/// Ok(()) iff every one of the team's four reference surfaces is pinned.
/// This is the Gate A call. Takes no arguments beyond `&self`.
pub fn validate_all_pinned(&self) -> Result<()>

/// The same traversal, returning the pins themselves. Use this ONLY if Gate A
/// needs the pin bodies (e.g. to name them in a message); otherwise prefer
/// `validate_all_pinned`, which is the thin boolean guard.
pub fn pinned_components(&self) -> Result<Vec<&PinnedRef>>
```

**Exact wiring for Gate A:** call `team.validate_all_pinned()?` from inside `validate_pack_preconditions` (122-03's pre-write gate helper), **not** inline in `pack_team` — 122-03 created `validate_pack_preconditions` specifically to keep the pack function under the cognitive-complexity ceiling, and its SUMMARY says to add gates there. The `?` propagates `PackageError::InvalidReference` unchanged; no new variant is needed and `error.rs` was deliberately left untouched by this plan.

**Three things 122-07 must know:**

1. **Gate A must run only when an attestation is supplied.** D-09 is *attestation implies resolved*, not *teams must always be pinned*. An unattested team holding ranges is legal and must keep packing. Guarding unconditionally would break every existing unattested team pack.
2. **The guard is ONE LEVEL DEEP and 122-07 owns the test that pins that.** The plan's own threat register (T-122-07) requires a test constructing *attested team -> pinned agent -> agent holds a `Range`* and asserting **the team still packs**. Nothing in this plan asserts that, because nothing here can: the limit is stated in the error text and the rustdoc, but its *visible behaviour* is a pack-path property.
3. **`would_be_unattested_manifest_digest` still hardcodes `ARTIFACT_TYPE_SERVER`** (122-03's deliberately-visible one-parameter gap). Unchanged by this plan; still 122-07's to thread.

## Files Created/Modified

- `crates/pmcp-package/src/reference.rs` — `PinnedRef.resolved_from` + its two-halves/Cargo/ambiguity rustdoc; module-docs paragraph on why the struct's first `Option` does not weaken the digest structural guarantee; 3 new tests; 4 literal updates
- `crates/pmcp-package/src/package/team.rs` — `component_refs`, `pinned_components`, `validate_all_pinned`; a fully-pinned test helper; 8 new tests
- `crates/pmcp-package/tests/digest_stability.rs` — the digest-participation test (additions only)
- `crates/pmcp-package/src/oci/unpack.rs` — 2 test-fixture literals
- `crates/pmcp-package/src/package/workflow.rs` — 1 test-helper literal
- `crates/pmcp-team-servers/src/team/identity.rs` — 1 test-helper literal (the only site outside `pmcp-package`)

## Measured `PinnedRef` construction-site inventory

Freshly run, per the acceptance criterion — the grep is authoritative over the plan's snapshot:

```
$ grep -rn 'PinnedRef {' --include="*.rs" crates cargo-pmcp
crates/pmcp-team-servers/src/team/identity.rs:94
crates/pmcp-package/src/oci/unpack.rs:845
crates/pmcp-package/src/oci/unpack.rs:851
crates/pmcp-package/src/reference.rs:47      <- the `pub struct PinnedRef {` DEFINITION, not a site
crates/pmcp-package/src/reference.rs:131
crates/pmcp-package/src/reference.rs:173
crates/pmcp-package/src/reference.rs:194
crates/pmcp-package/src/reference.rs:221
crates/pmcp-package/src/package/workflow.rs:131
```

Nine hits, **EIGHT construction sites**. Per-file: `reference.rs` 4, `unpack.rs` 2, `workflow.rs` 1, `identity.rs` 1 — matching the plan's corrected count exactly. Two line numbers differed from the plan's record (`unpack.rs` 626/632 are now 845/851, moved by 122-03); the rustdoc follows the grep and cites files rather than line numbers, so it cannot rot the same way. **All eight sites are inside `#[cfg(test)]` code** — there is no non-test `PinnedRef` struct literal in this repository, which bounds the source-breaking half's blast radius to test code in-repo (it remains fully breaking for out-of-repo consumers, and the rustdoc says so).

## Falsifiability control (Task 2)

Run against the real tree, observed to fail, fully reverted; `git status` clean afterwards.

**Mutation:** `.chain(self.finalizer_agents.iter())` removed from `TeamPackage::component_refs`.

**Observed:** `test result: FAILED. 182 passed; 2 failed`

| Test | Result | What it showed |
|---|---|---|
| `a_range_finalizer_agent_fails_and_names_that_component` | **FAILED** | `expect_err` panicked — the guard returned **`Ok`** for a team holding a `Range` finalizer. The panic message printed the 4 pins it wrongly accepted. This is the exact bug the control targets: an attestation would have been attached to an unresolved team. |
| `pinned_components_returns_every_pin_from_all_four_surfaces` | **FAILED** | `left: 4, right: 5` — the count assertion caught the missing surface independently. |

Two tests fail on the one mutation, so the property is over-determined rather than resting on a single assertion.

## Decisions Made

Recorded in frontmatter `key-decisions`. The two worth restating:

- **The traversal order is a contract, not an accident.** It is fixed (not sorted), stated in the rustdoc, and asserted by name in `pinned_components_returns_every_pin_from_all_four_surfaces` — so a future refactor that reorders the chain fails a test rather than silently changing what a caller sees.
- **A fourth single-surface test was added beyond the plan's three.** The plan's behaviour list named `entry_point`, `built_in_servers` and `members[].agent`; `finalizer_agents` is the last link in the chain and the easiest to drop, and the plan's own falsifiability control targets exactly that link. Without a corresponding test, the control would only have been caught by the count assertion — a weaker, less diagnostic signal.

## Deviations from Plan

**None — plan executed exactly as written.**

Two points of interpretation worth surfacing, neither of which changed scope or intent:

1. **TDD commit granularity.** Both tasks are `tdd="true"`, and in Rust the RED for a new struct field and for new methods is a **compile** failure — a `test(...)` commit would have put a non-compiling tree in history, which CLAUDE.md's build-verification gate forbids on every commit. Tests were therefore written first and the RED observed and captured verbatim (Task 1: 4 errors, `struct PinnedRef has no field named resolved_from`; Task 2: 13 errors, `no method named pinned_components found for struct TeamPackage`), then implementation landed in one green `feat` commit per task. The discipline is intact; only the commit split differs, in CLAUDE.md's favour.
2. **One test beyond the plan's six** (the `finalizer_agents` single-surface case), for the reason given above. The plan's criterion was "at least 6 more tests than before"; the delivered count is 8.

## Issues Encountered

**A background `make quality-gate` reported exit 0 on a TRUNCATED log — re-measured rather than trusted.**

The first aggregate run returned exit code 0, but its captured output was 1997 lines ending in a literal `... (9604 lines truncated)` marker, with the final leg's `pmcp-package fmt/clippy/test OK` line absent. A truncated log cannot show which legs ran, and this project's own notes record `rtk`-proxied output corrupting exactly this kind of check — so the green was not accepted.

The gate was re-run through a small script writing to a controlled path with an explicit `MAKE_QUALITY_GATE_EXIT=$?` sentinel appended to the log:

```
MAKE_QUALITY_GATE_EXIT=0
```

11262-line complete log, **473 passing test suites**, and the final-leg marker present. The 8 new `package::team::tests::*` cases were confirmed running *inside* the gate (log lines 7328-7340), so the gate is green **on this plan's code**, not merely green in general.

**No stubs, no skipped tests, no unrun `<verify>`.** Nothing was appended to `.planning/WINDOWS.md`.

**Ledger note (not an action taken):** `WINDOWS.md` entries **#30** and **#31** — the unrun aggregate gate for plans 122-02 and 122-03 — were already marked `fixed` on this plan's base commit (`fbb1970d`, "close unrun-verify #30/#31"). This plan's green aggregate run, on a tree containing both those plans' commits, independently confirms that closure was warranted. The ledger was not edited here.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready.**

- **122-07** has both artifacts it was waiting on, with the exact call site and its three constraints written out above. It should call `team.validate_all_pinned()?` from `validate_pack_preconditions`, gate it on an attestation being supplied, and own the transitive-depth-limit behaviour test (T-122-07).
- **122-08 owns the version consequence, and this plan adds to it.** On top of 122-02's `pack_server` parameter, 122-03's new `PackageError` variant and its `UnpackedAttestation.subject` type change, this plan adds a **fifth public field to `PinnedRef`**. That is source-breaking for any out-of-repo struct literal — additive on the wire, breaking in Rust — and the rustdoc states both halves. In-repo the blast radius is test code only (all eight sites are `#[cfg(test)]`).
- **123** inherits `resolved_from` as the input to dev-to-prod skew reporting, with its `None`-means-cannot-report obligation stated in the field's own rustdoc rather than in a planning document it might not read.

**Concerns:** none. Every leg of this plan's `<verification>` block ran and passed, including the aggregate gate.

## Self-Check: PASSED

- Both commit hashes resolve: `81483d47`, `c94fafd4`.
- All 6 `key-files.modified` entries appear in `git diff --numstat fbb1970d HEAD` (2/307/1/189/54/1 lines added, 2 removed).
- `git diff --diff-filter=D --name-only` across both commits reports **no deleted files**.
- Every task-level `<acceptance_criteria>` re-run and passing; none deferred or skipped.
- Baselines held or grew, none shrank: pmcp-package **266 -> 278** tests; `digest_stability` 20 -> 21; `roundtrip` 17 (unchanged, untouched); `pmcp-team-servers --lib` 132 (unchanged, one literal edit).
- `crates/pmcp-package/tests/golden_fixtures/` — zero changed files, verified against the base commit, not just the working tree.
- `crates/pmcp-package/src/error.rs` — absent from the whole-plan diff.
- The falsifiability control was reproduced, observed failing with its output recorded, and reverted; the tree is clean.
- Worktree left clean: `.pmat/` regenerated caches restored via `git checkout -- .pmat/`.

---
*Phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b*
*Completed: 2026-08-25*
