---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 07
subsystem: infra
tags: [oci, attestation, team-package, pinning, supply-chain, pmcp-package, cargo-pmcp, api-break, exit-code]

# Dependency graph
requires:
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "`MT_ATTESTATION` (kind-neutral), `AttestationFile`, `UnpackedAttestation`, `write_annotated_layer`, `read_attestation_layer` (plan 122-02)"
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "`validate_pack_preconditions`, `assemble_manifest`, `OciLayout::describe_blob`, `SubjectVerdict`, `would_be_unattested_manifest_digest` and the measured `inspect` exit code 1 (plan 122-03)"
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "`TeamPackage::pinned_components` / `validate_all_pinned` and `PinnedRef.resolved_from` (plan 122-05)"
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "`reject_attestation_annotations_that_break_canonical_json` + `PackageError::AttestationAnnotationInvalid` — reused, not duplicated (plan 122-06)"
provides:
  - "`pack_team(package, attestation, layout)` — the attestation parameter, exposed on teams ONLY"
  - "`pack_single_layer`'s internal optional attestation, threaded through the SHARED helper (D-08: one mechanism, not two)"
  - "`UnpackedTeam { package, attestation }` + `unpack_team`'s changed return type `Result<UnpackedTeam>`"
  - "`validate_single_layer_pack_preconditions` — the single-layer sibling of `validate_pack_preconditions`, holding the two kind-neutral attestation gates"
  - "Gate A: `reject_an_attestation_over_an_unresolved_team`, refusing before the first write (D-09)"
  - "`reject_layers_a_team_may_not_carry` — the team's exactly-one-config-PLUS-optional-attestation ALLOW-LIST, distinct from `unpack_single_layer`'s strict rule"
  - "`would_be_unattested_manifest_digest(layers, artifact_type)` — generalized off the hardcoded `ARTIFACT_TYPE_SERVER`"
  - "The team's three `inspect` render states and its non-zero exit, holding under `--quiet`"
affects: [122-08, 123, 124]

actuals:
  tokens: 68000
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "A carriage parameter lives on the SHARED helper and is EXPOSED by only the wrappers the decision covers, with the reasoning written where the temptation to 'finish the job' is — at the shared helper, not at the wrappers"
    - "Two layer-count rules that differ are stated at BOTH sites with the reason they differ, so a later reader cannot 'unify' them and reopen the hole the strict one closes"
    - "A layer-inventory rule that must admit a second legal layer is an ALLOW-LIST of media types, never a relaxed count — admitting one more thing must not admit anything else"
    - "A renderer and its exit-code twin take the OPTIONAL CARRIAGE rather than a kind-specific package, so two package kinds cannot drift into gating differently"
    - "A guard's VACUITY on a sibling path is documented as a fact about the type, so the apparent asymmetry is not 'fixed' with a no-op call"

key-files:
  created: []
  modified:
    - crates/pmcp-package/src/oci/pack.rs
    - crates/pmcp-package/src/oci/unpack.rs
    - crates/pmcp-package/src/oci/mod.rs
    - crates/pmcp-package/src/oci/media_types.rs
    - crates/pmcp-package/src/lib.rs
    - crates/pmcp-package/tests/roundtrip.rs
    - crates/pmcp-package/tests/negative.rs
    - cargo-pmcp/src/commands/package/inspect.rs
    - cargo-pmcp/tests/package_inspect.rs

key-decisions:
  - "`unpack_team` BREAKS its return type rather than gaining an `unpack_team_attested` sibling — the rejected alternative ships two functions where one was asked for, and a caller reaching for the old one would silently never see an attestation the package carries. Recorded in `UnpackedTeam`'s own rustdoc so the option NOT taken is on record"
  - "Gate A is a distinct named free function called from `pack_team`, not from `validate_pack_preconditions` — that helper is SERVER-TYPED (`&ServerPackage`, `&ConfigFile`) and Gate A needs a `&TeamPackage`. Hosting it in the kind-generic sibling would have required a `SingleLayerPackage` dispatch hook whose agent and workflow impls could never run"
  - "The team's layer rule is an ALLOW-LIST (`MT_TEAM_CONFIG` + optional `MT_ATTESTATION`), NOT a loosened count — `unpack_single_layer`'s strict exactly-one rule stays untouched for agents and workflows, and both sites say why they differ"
  - "`render_attestation` and `refuse_a_subject_that_does_not_name_this_package` were RE-TYPED to take `Option<&UnpackedAttestation>` instead of `&UnpackedServer`, so the server and team arms share one implementation and therefore one exit code — a second copy could drift into gating differently by kind"
  - "The team's attested-fixture helpers pack UNATTESTED FIRST to learn the subject Gate B demands, which is also what makes the Gate-A refusal tests unambiguous: the subject they supply is CORRECT, so the only thing Gate A can be reacting to is the unresolved reference"
  - "`would_be_unattested_manifest_digest` takes `artifact_type` as a parameter (122-03's deliberately-visible one-parameter gap), because the dry manifest must match the real one in a field that is INSIDE the hash"

patterns-established:
  - "Pattern: an executor that cannot follow a handoff LITERALLY (because the named call site is wrong-typed) satisfies the CONSTRAINTS BEHIND it and records the divergence explicitly, rather than silently doing something else or forcing a type hook nobody needs"
  - "Pattern: a depth-limit test is CONSTRUCTED literally (pack the inner artifact, pin its real digest) rather than described, and carries a rustdoc paragraph stating what its passing does NOT mean"
  - "Pattern: a background-task exit code is NEVER trusted — the sentinel written INTO the log is the status, because the harness notification has now falsely reported success twice in this phase"

requirements-completed: [PKGX-01]

coverage:
  - id: D1
    description: "SC2's team half: a team package carries an attestation through the SAME mechanism a server does — round-tripping the typed package AND the attestation, with the payload byte-identical and all three annotation values verbatim"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_attested_team_round_trips_its_package_and_its_attestation"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/oci/unpack.rs#team_pack_then_unpack_round_trips_losslessly"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#team_package_round_trips_losslessly"
        status: pass
    human_judgment: false
  - id: D2
    description: "The subject verdict is RE-DERIVED on the team path, never read from the stored claim: altering only the subject annotation unpacks successfully and reports a MISMATCH, with claim and reality both readable"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_altered_team_subject_annotation_unpacks_successfully_and_reports_a_mismatch"
        status: pass
    human_judgment: false
  - id: D3
    description: "ONE media type, no kind dispatch: an attested TEAM declares `ARTIFACT_TYPE_TEAM` while its attestation layer declares the same kind-neutral `MT_ATTESTATION` a server carries; no `MT_TEAM_ATTESTATION` was introduced"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_attested_team_declares_the_team_artifact_type_and_the_kind_neutral_attestation_type"
        status: pass
      - kind: other
        ref: "grep -rn 'MT_TEAM_ATTESTATION|mcp-server.attestation' crates/ cargo-pmcp/ -> exit 1, no match"
        status: pass
    human_judgment: false
  - id: D4
    description: "The extra-layer defence still holds where it still applies: admitting a SECOND layer on the team path did not loosen the strict exactly-one-layer rule for agents, proven by a crafted agent layout carrying a grafted attestation layer"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#a_crafted_agent_layout_with_an_extra_layer_is_still_rejected"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_unattested_team_manifest_carries_exactly_one_layer"
        status: pass
    human_judgment: false
  - id: D5
    description: "SC6: an attestation over a team holding a `ComponentRef::Range` on ANY of its four surfaces is refused with `PackageError::InvalidReference`, naming that component AND its `component_type`"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::an_attested_team_with_a_range_entry_point_is_refused_naming_that_component"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::an_attested_team_with_a_range_member_agent_is_refused_naming_that_component"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::an_attested_team_with_a_range_built_in_server_is_refused_naming_that_component"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::an_attested_team_with_a_range_finalizer_agent_is_refused_naming_that_component"
        status: pass
      - kind: other
        ref: "Falsifiability control (below): validate_all_pinned() delegation replaced by Ok(()) -> all 5 refusal tests FAIL; restored -> 28 pass"
        status: pass
    human_judgment: false
  - id: D6
    description: "The guard is scoped to the CLAIM, not to the format: the SAME unresolved team still packs UNATTESTED, and a fully pinned team packs attested"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::an_unattested_team_holding_ranges_still_packs"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::an_attested_fully_pinned_team_packs"
        status: pass
    human_judgment: false
  - id: D7
    description: "The ONE-LEVEL DEPTH LIMIT is pinned as visible behaviour: an attested team whose PINNED agent itself holds a `Range` connector still packs — constructed literally, with a rustdoc stating what that green does NOT mean"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::an_attested_team_whose_pinned_agent_itself_holds_a_range_still_packs"
        status: pass
    human_judgment: false
  - id: D8
    description: "A Gate-A refusal adds neither a blob nor an index entry — the gate runs BEFORE the first write, asserted over the FULL recursive layout rather than by the refusal alone"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_resolved::a_refused_attested_team_pack_leaves_the_destination_layout_byte_for_byte_unchanged"
        status: pass
    human_judgment: false
  - id: D9
    description: "SC3 for teams: `cargo pmcp package inspect` renders all three attestation states for a TEAM exactly as for a server, and exits with the SAME measured code 1 on a mismatch — including under `--quiet`"
    requirement: PKGX-01
    verification:
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_reports_a_matching_team_subject_as_a_match_and_succeeds"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_renders_the_full_diagnostic_and_exits_non_zero_on_a_team_subject_mismatch"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_exits_non_zero_on_a_team_subject_mismatch_even_with_output_suppressed"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_reports_an_unattested_team_fixture_as_carrying_no_attestation"
        status: pass
    human_judgment: false
  - id: D10
    description: "`UnpackedTeam` is reachable by BOTH documented paths (`pmcp_package::UnpackedTeam` and `pmcp_package::oci::UnpackedTeam`), proven by a compiling doctest that also round-trips an unattested team"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/oci/unpack.rs — UnpackedTeam rustdoc doctest (names both paths, asserts they are one type)"
        status: pass
    human_judgment: false
  - id: D11
    description: "Cognitive complexity stays under the repo's 25 ceiling across `pmcp-package` after the pack-path changes"
    verification:
      - kind: other
        ref: "pmat 3.15.0 analyze complexity --max-cognitive 25 --path crates/pmcp-package/src -> 25 files analyzed, all filtered (no function exceeds 25); non-vacuity confirmed at --max-cognitive 5, which reports 6 files with max cognitive 21"
        status: pass
    human_judgment: false
  - id: D12
    description: "The full aggregate `make quality-gate`"
    verification:
      - kind: other
        ref: "make quality-gate"
        status: unknown
    human_judgment: true
    rationale: "NOT RUN TO COMPLETION. QUALITY_GATE_EXIT=2 — the run reached `make test-unit` and aborted on machine-level disk exhaustion; every error is `No space left on device` / `rustc-LLVM ERROR: IO failure on output stream`. Free space fell 11 GiB -> 0 during the run and the volume hit ABSOLUTE ZERO, after which no further command could start at all. Not a code defect. Every leg this plan touches was run individually and passed — see 'Verification Results'. Recorded in .planning/WINDOWS.md as unrun-verify #33, the same class as 122-02's #30 and 122-03's #31."

duration: 118 min
completed: 2026-08-25
status: complete
---

# Phase 122 Plan 07: The Team Carrier and the Resolved-Reference Precondition Summary

**A team package now carries an attestation through the SAME shared helper a server uses — one kind-neutral media type, no kind dispatch — while `pack_agent` and `pack_workflow` deliberately do not grow the parameter; and attaching an attestation to a team holding any `ComponentRef::Range` is refused before the first write, with the one-level depth limit pinned as a PASSING test rather than left as a caveat.**

## Performance

- **Duration:** 118 min (including a full stop on machine-level disk exhaustion and a resume)
- **Tasks:** 3
- **Files modified:** 9
- **Commits:** 3 task commits (+ this metadata commit)

## Accomplishments

- **SC2's team half landed through ONE mechanism, not two.** The attestation is threaded through the SHARED `pack_single_layer`, and `pack_team` is the only wrapper that exposes it. The team path reuses `plan_attestation_layer`, `write_planned_layer`, `read_attestation_layer`, `re_derive_unattested_digest`, `SubjectVerdict`, both kind-neutral gates and the same three annotation keys **verbatim** — there is no kind dispatch anywhere and no team-specific attestation constant. 122-02's kind-independence property is the standing guard that this stays true, and a new team-side test asserts the same fact from the other direction.
- **SC6 landed with its BOUNDARY as a passing test, not a caveat.** D-09's one-level depth limit is the easiest thing in this phase to quietly leave unexamined. It is now `an_attested_team_whose_pinned_agent_itself_holds_a_range_still_packs`, built literally — pack an agent whose own connector is a `Range`, pin its REAL manifest digest into the team, attest the team, assert success — with a rustdoc paragraph stating that its passing proves the limit EXISTS and does NOT prove the graph is transitively resolved.
- **The extra-layer defence was re-stated rather than relaxed.** Admitting a second legal layer on the team path is exactly where the strict exactly-one-layer rule could have been loosened into a hole. Instead the team got an ALLOW-LIST of media types (`reject_layers_a_team_may_not_carry`), `unpack_single_layer` was left untouched, both sites carry the reason they differ, and a test grafts a real attestation layer onto an AGENT manifest and asserts it is still rejected.
- **The team `inspect` contract is the server contract, by construction rather than by convention.** `render_attestation` and `refuse_a_subject_that_does_not_name_this_package` were re-typed to take `Option<&UnpackedAttestation>`, so both kinds render through one implementation and exit through one code path. The team mismatch test asserts the exact code **1** measured in 122-03 — not merely "non-zero" — and asserts the rendered content alongside it, so a binary that printed nothing could not pass.
- **122-03's deliberately-visible one-parameter gap is closed.** `would_be_unattested_manifest_digest` takes `artifact_type` instead of hardcoding `ARTIFACT_TYPE_SERVER`, with a rustdoc stating that it MUST be the same value the matching `finalize_pack` uses — otherwise the dry manifest differs from the real one in a field inside the hash and the comparison silently stops meaning anything.

## Task Commits

1. **Task 1: Thread carriage through the shared single-layer helper, exposing it on teams only** — `674e8023` (feat)
2. **Task 2: Gate A — an attestation over an unresolved package is refused, one level deep and no deeper** — `dfb2f61b` (feat)
3. **Task 3: The team's three inspect states, matching the server contract exactly** — `abb14457` (feat)

## The four breaking changes 122-08's emitter inventory must name

Recorded explicitly because plan 122-08 reads this SUMMARY for the final list. This plan adds **four** to the phase's running total:

1. **`pack_team` gains a second positional parameter** — `pack_team(package, attestation, layout)`. Every existing call site breaks. In-repo that was 3 sites; out-of-repo it is every consumer.
2. **`unpack_team`'s return type changes** from `Result<TeamPackage>` to `Result<UnpackedTeam>`. Source-breaking for every consumer, and deliberately so — see the decision below.
3. **`pack_team` can now REFUSE input that previously packed** — an attested pack over a team holding a `ComponentRef::Range` is `PackageError::InvalidReference`. No new error variant, but a new refusal is a behaviour break.
4. **`unpack_team` can now REFUSE a layout that previously unpacked** — the team allow-list rejects any layer that is neither `MT_TEAM_CONFIG` nor `MT_ATTESTATION`. Previously such a layout failed too (the strict count rejected it), so this is the narrower of the four, but the ERROR TEXT changed and any consumer matching on it breaks.

These sit on top of 122-02's `pack_server` parameter and `UnpackedServer` field, 122-03's `PackageError` variant and `UnpackedAttestation.subject` type change, 122-05's fifth `PinnedRef` field, and 122-06's second `PackageError` variant. **This phase publishes nothing** — 122-08's blocking checkpoint owns that.

## The `unpack_team` return-type break: three call sites and two re-export lists

RESEARCH Open Question 1 was settled in the code, taking the recommended option.

**Three call sites, all updated. Located by SYMBOL, not by the line numbers the plan recorded** — 122-05 warned that its own plan's recorded `unpack.rs` line numbers had moved under 122-03's restructuring, and the same was true here (the plan's `unpack.rs:741` is now `1258`; `roundtrip.rs:200` is now `207`).

| # | Site | Plan's line | Actual line | Change |
|---|---|---|---|---|
| 1 | `crates/pmcp-package/tests/roundtrip.rs` | 200 | **207** | `unpacked.package == package` + `attestation == None` |
| 2 | `crates/pmcp-package/src/oci/unpack.rs` (`#[cfg(test)]`) | 741 | **1258** | same |
| 3 | `cargo-pmcp/src/commands/package/inspect.rs` | 113 | **155** | binds `UnpackedTeam`, passes `&unpacked` to `render_team` |

**Two re-export lists gained `UnpackedTeam`** (neither breaks — a re-exported function name is unaffected by its return type — but without them the new type is unreachable by its intended path):

- `crates/pmcp-package/src/oci/mod.rs:34` — the `pub use unpack::{...}` list
- `crates/pmcp-package/src/lib.rs:72-73` — the crate-root `pub use oci::{...}` list

A doctest on `UnpackedTeam` names **both** paths and binds one to the other, so a future edit that drops either re-export fails to compile rather than silently narrowing the surface.

**Why break rather than add `unpack_team_attested`:** a sibling would ship two functions where one was asked for, and D-08's own instruction for carriage is "one mechanism, not two". Worse, a caller that reached for the old verb would silently never see an attestation the package actually carries — a compile error is cheap insurance against exactly that. `pmcp-package` is 0.x, the package tree's standing position is to break freely, and this phase already forces a version conversation. The rejected option is written into `UnpackedTeam`'s own rustdoc so it is on record where the next reader meets the type.

## Confirmation: no `MT_TEAM_ATTESTATION` was introduced

```
$ grep -rn 'MT_TEAM_ATTESTATION\|mcp-server.attestation' crates/ cargo-pmcp/
$ echo $?
1
```

No match. The team path reuses the single kind-neutral `MT_ATTESTATION` (`application/vnd.pmcp.attestation.v1`) that 122-02 settled BEFORE the tracer froze it — which is the only reason `read_attestation_layer` and the three annotation keys could be reused with no kind dispatch at all. `media_types.rs`'s single-layer module-doc bullet was amended: it no longer claims a team is strictly one layer, it names the optional `MT_ATTESTATION` layer, and it preserves the strict claim for agents and workflows.

## Task 2's falsifiability control

Run against the real tree, observed to fail, fully reverted; `git status` clean afterwards.

**Mutation:** the `package.validate_all_pinned()` delegation in `reject_an_attestation_over_an_unresolved_team` replaced by `Ok(())` (with `let _ = package;` so the unused parameter still compiled under `-D warnings`).

**Observed:** `test result: FAILED. 23 passed; 5 failed`

| Test | Result |
|---|---|
| `an_attested_team_with_a_range_entry_point_is_refused_naming_that_component` | **FAILED** |
| `an_attested_team_with_a_range_member_agent_is_refused_naming_that_component` | **FAILED** |
| `an_attested_team_with_a_range_built_in_server_is_refused_naming_that_component` | **FAILED** |
| `an_attested_team_with_a_range_finalizer_agent_is_refused_naming_that_component` | **FAILED** |
| `a_refused_attested_team_pack_leaves_the_destination_layout_byte_for_byte_unchanged` | **FAILED** |

All five refusal tests go red on the one mutation. The three boundary tests — unattested-still-packs, fully-pinned-packs, and the depth-limit case — stayed GREEN under the mutation, which is correct and is the point: they assert what the gate must NOT do, so they cannot detect the gate's absence. Restored, all 28 pass.

**The RED phase was also real, before the control.** With the tests written and Gate A absent, `cargo test --test negative` reported `FAILED. 23 passed; 5 failed` — the same five, each panicking with `an attestation over an unresolved team must be refused: ManifestDigest("sha256:...")`, i.e. the pack SUCCEEDED where it had to fail.

## Files Created/Modified

- `crates/pmcp-package/src/oci/pack.rs` — `pack_single_layer`'s attestation parameter + the D-08 team-of-one rustdoc; `validate_single_layer_pack_preconditions`; Gate A (`reject_an_attestation_over_an_unresolved_team`); `pack_team`'s third parameter; `artifact_type` threaded through `would_be_unattested_manifest_digest` and `reject_an_attestation_subject_naming_another_package` (+245/-17)
- `crates/pmcp-package/src/oci/unpack.rs` — `UnpackedTeam` + its doctest and rejected-alternative rustdoc; the bespoke `unpack_team`; `reject_layers_a_team_may_not_carry`; `unpack_single_layer`'s "why the team rule differs" section; module-header correction (+227/-8)
- `crates/pmcp-package/tests/negative.rs` — the whole `attestation_resolved` module, 8 tests (+344/-0)
- `crates/pmcp-package/tests/roundtrip.rs` — 5 team-carriage tests + `fully_pinned_team_package`; both team call sites updated (+206/-3)
- `cargo-pmcp/tests/package_inspect.rs` — 4 team CLI tests + the fully-pinned team fixture (+180/-3)
- `cargo-pmcp/src/commands/package/inspect.rs` — `render_team` takes `&UnpackedTeam` and renders the attestation; `render_attestation` and the exit-code function re-typed to `Option<&UnpackedAttestation>`; module docs on the two carrier kinds and the two that carry none by design (+49/-11)
- `crates/pmcp-package/src/oci/media_types.rs` — the amended single-layer bullet (+12/-5)
- `crates/pmcp-package/src/oci/mod.rs`, `crates/pmcp-package/src/lib.rs` — `UnpackedTeam` re-exports (+1/-1 each)

## Decisions Made

Recorded in frontmatter `key-decisions`. The one that needs its reasoning in full is below, because it diverges from a handoff instruction.

### Gate A's placement — 122-05's handoff could not be followed LITERALLY

**122-05's instruction:** *"call `team.validate_all_pinned()?` from inside `validate_pack_preconditions`, NOT inline in `pack_team`"*, on the grounds that 122-03 created that helper specifically to keep the pack function under the cognitive-complexity ceiling.

**Why it could not be done as written:** `validate_pack_preconditions` is **server-typed**. Its signature is `(&ServerPackage, Option<ConfigFile>, Option<AttestationFile>, &[PlannedLayer])`, and its body runs the config-slot document gates that only a `ServerPackage` has. Gate A needs a `&TeamPackage` to traverse four `ComponentRef` surfaces. There is no team on that path to validate.

**What was done instead, and why it satisfies the constraint behind the instruction:**

- Gate A is a **distinct named free function**, `reject_an_attestation_over_an_unresolved_team`, following the `reject_config_keys_without_a_config` precedent the plan named. Nothing is inline.
- It is called from `pack_team` as a single `?`-propagating line. `pack_team`'s cognitive complexity is **1**; `pmat` reports the crate's maximum at **21**, under the 25 ceiling, with no new violation.
- **The pre-write invariant is preserved and asserted, not assumed:** `pack_team` performs NO writes of its own before delegating to `pack_single_layer`, so a Gate-A refusal leaves the layout byte-for-byte as found. That is pinned by `a_refused_attested_team_pack_leaves_the_destination_layout_byte_for_byte_unchanged`, a full recursive layout snapshot — the same technique 122-03 used, and the assertion that would fail if the gate were ever moved after the writing loop.
- All three of 122-05's explicit constraints are met: `validate_all_pinned()` is the call (not a re-derivation), `?` propagates `PackageError::InvalidReference` **unchanged**, and `error.rs` was **not touched** (`git diff` reports it absent from the whole-plan diff).

**The alternative considered and rejected:** a `validate_references_resolved()` hook on the `SingleLayerPackage` trait, defaulting to `Ok(())`, so Gate A could live inside the kind-generic `validate_single_layer_pack_preconditions`. Rejected because agents and workflows never pass `Some(attestation)` (D-08), so both impls would be permanently unreachable — machinery whose only purpose is to make a gate's location look uniform, at the cost of two dead code paths a later reader must reason about. The divergence is documented at **both** sites: in `validate_single_layer_pack_preconditions`'s rustdoc (why Gate A is not here) and in `pack_team`'s body comment (why it is here, and why the invariant still holds).

### 122-06's annotation gate is REUSED, not duplicated

122-06's SUMMARY flagged exactly one thing to check: *"If 122-07 introduces its own precondition function instead, the annotation gate must be called from it explicitly."* It did introduce one — `validate_single_layer_pack_preconditions` — and that function's **first** call is `reject_attestation_annotations_that_break_canonical_json(attestation)?`, the same function `validate_pack_preconditions` calls, followed by the same `reject_an_attestation_subject_naming_another_package`. No second copy of either check exists; a fix applied to one is a fix applied to both. The C0-control-character refusal 122-06 added therefore covers team annotations for free, as intended.

## Deviations from Plan

**None — plan executed exactly as written.**

Three points of interpretation, none of which changed scope or intent:

1. **Gate A's placement**, documented in full above. The plan's acceptance criterion reads "`validate_pack_preconditions` gains at most one delegating call"; it gains **zero**, which satisfies "at most one".
2. **Five rustdoc warnings were fixed as they were introduced.** The new cross-references to `pack_single_layer`, `index_layers` and `unpack_single_layer` are links from PUBLIC docs to PRIVATE items, which emits `private_intra_doc_links`. 122-02 established a zero-warning baseline for this crate and 122-03's deviation 4 applied the same remedy; all five were demoted to plain code spans, changing no prose meaning. `cargo doc --no-deps` is back to zero warnings.
3. **Two tests beyond the plan's ask.** The plan's Task 2 criterion was "at least 5 more tests than after plan 122-03" (`negative` was 18 then); the delivered count is **28**, +10. The extras are the fourth single-surface case (`finalizer_agents` — the link 122-05's own falsifiability control targets, and the easiest to drop) and the accepting `an_attested_fully_pinned_team_packs`, without which "refuse everything" would satisfy the refusal tests.

## Verification Results

| Check | Result |
|---|---|
| `make pmcp-package-gate` (fmt + clippy `-D warnings` + tests + example, workspace-excluded crate) | **exit 0** — **300** tests (baseline 286), 417-line complete log, no truncation marker, final leg banner present |
| `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test negative` | **exit 0** — **28** (was 20) |
| `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test roundtrip` | **exit 0** — **22** (was 17) |
| `cargo test -p cargo-pmcp --test package_inspect` | **exit 0** — **12** (was 8) |
| `make test-cargo-pmcp-integration` | **exit 0** — `package_inspect` **12**, `package_attestation_contract` **3**, `package_capture_contract` **3** |
| `make no-crypto-check` | **exit 0** — no dependency entered the tree |
| `cargo build -p cargo-pmcp` | **exit 0** — zero warnings in `inspect.rs` |
| `cargo fmt --all -- --check` (root workspace) | **exit 0** |
| `cargo doc --manifest-path crates/pmcp-package/Cargo.toml --no-deps` | **zero warnings** |
| `cargo run --manifest-path crates/pmcp-package/Cargo.toml --example attestation_carriage` | **exit 0** — survived this plan's API changes (run inside the gate) |
| `pmat 3.15.0 analyze complexity --max-cognitive 25` | **no violation** — 25 files analyzed, all filtered; non-vacuity confirmed at `--max-cognitive 5`, which reports 6 files with **max cognitive 21** |
| `grep -rn 'MT_TEAM_ATTESTATION\|mcp-server.attestation' crates/ cargo-pmcp/` | **exit 1**, no match |
| `git diff -- crates/pmcp-package/src/package/workflow.rs` | **no changes** — `WorkflowManifest`'s pin guard and its tests are untouched |
| `make quality-gate` (aggregate) | **QUALITY_GATE_EXIT=2 — UNRUN, disk exhaustion. See below.** |

**Baselines held or grew, none shrank:** pmcp-package **286 → 300**; `negative` 20 → 28; `roundtrip` 17 → 22; `package_inspect` 8 → 12; `package_attestation_contract` 3 (held); `package_capture_contract` 3 (held).

## Issues Encountered

### `make quality-gate` was NOT completed — machine-level disk exhaustion, not a code defect

The run reached `make test-unit` and aborted. Every error in the log is an I/O failure:

```
error: failed to create file encoder: No space left on device (os error 28)
rustc-LLVM ERROR: IO failure on output stream: No space left on device
error: couldn't create a temp dir: No space left on device (os error 28)
make[1]: *** [test-unit] Error 101
make: *** [quality-gate] Error 2
QUALITY_GATE_EXIT=2
```

`df -h /` was checked **before** anything was attributed to the change, per this project's own debugging notes. Free space was **11 GiB** at launch and fell through 8.1 → 4.5 → **0** during the run. This is the identical failure plans 122-02 and 122-03 both recorded.

**The volume then hit ABSOLUTE ZERO, which wedged the session.** No further command could run at all — the harness could not create its own Bash output file, and `Write`/`Edit` could not stage their temp files. `rm -rf <worktree>/target` was attempted and could not execute for the same reason. The plan was halted and reported to the coordinator, who freed the disk (27 GiB) and resumed this SUMMARY-only step. **All three task commits were already in git and nothing was lost.**

### The harness notification falsely reported "exit code 0" for that run — again

The background task completed with a notification reading **"exit code 0"**. That is wrong; the real status was 2. The explicit `QUALITY_GATE_EXIT=$?` sentinel appended into the log is what caught it, and it is the only reason this SUMMARY does not record a false green.

**This is the second occurrence in this phase** — 122-02's SUMMARY recorded the identical trap (a notification reporting 0 for a run whose real exit was 2, because the reported status belonged to a trailing pipeline element rather than to `make`). 122-05 independently hit the truncated-log variant. Three plans, three false greens, all caught only by an explicit sentinel. **Treat the notification as unusable in this repo and read the sentinel.**

### `rtk` truncates `git diff` — measured, not suspected

`git diff fa0a3404 HEAD` through the `rtk` proxy returned **522 lines / 26,475 chars**. The same command as `/usr/bin/git` returned **1,591 lines / 74,973 chars** — the proxy dropped two thirds of the diff without any truncation marker. The `actuals` above are measured with the absolute path. Every `make` invocation in this plan used `/usr/bin/make` for the same reason.

### Out of scope, recorded not fixed

**`cargo clippy -p cargo-pmcp --all-targets -- -D warnings` exits 101** on two pre-existing `too_many_arguments` violations (8/7) in `crates/pmcp-workbook-runtime/src/render/mod.rs:420` and `:511` — a crate this plan never touched. **Zero findings in `cargo-pmcp`'s own sources**, and `cargo build -p cargo-pmcp` succeeds. Per the scope boundary this was not fixed; it also matches this project's recorded note that `cargo-pmcp` and the toolkit crates are not clippy-gated in CI, so a bare `-D warnings` run on them is stricter than the gate and does not block.

**No stubs, no skipped tests.** The one unrun `<verify>` leg is the aggregate gate above, appended to `.planning/WINDOWS.md` as `unrun-verify` **#33**.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready, with one open item for the verifier.**

- **122-08 (the version/publish decision)** has the full breaking-change inventory it needs: the four items this plan adds are enumerated above by name, on top of 122-02's, 122-03's, 122-05's and 122-06's. This phase publishes nothing.
- **123 (dev-to-prod skew reporting)** inherits a team carrier whose attestation implies a resolved team at its own level, plus `PinnedRef.resolved_from` from 122-05. The one-level depth limit is the boundary it must not assume away — it is stated in Gate A's rustdoc, in `TeamPackage`'s error text, and in the depth-limit test's own rustdoc.
- **Platform admission policy, not SDK work:** closing the depth limit transitively means requiring every pinned component to itself be attested. This crate is forbidden a registry client (milestone Decision 2), so it cannot resolve a referenced package offline to look inside it. Recorded here so the gap is a decision rather than a discovery.

**Concerns:**

- **The aggregate `make quality-gate` is unverified** — an unrun check, not a known defect, and the third such in this phase. `.planning/WINDOWS.md` #33 is open and should be closed by one run alongside a verification pass. The two earlier ones (#30, #31) were closed by 122-05's and 122-06's successful runs; the code this plan adds is covered individually by the twelve passing legs above, including the workspace-excluded crate's own gate.
- **Disk headroom on this machine remains the binding constraint on verification, not code quality.** The next agent to attempt a full gate should check `df -h /` first and budget well above 11 GiB — that was not enough.

## Self-Check: PASSED

- All three commit hashes resolve: `674e8023`, `dfb2f61b`, `abb14457`.
- All 9 `key-files.modified` entries exist on disk (`ls` per path) and all 9 appear in `git diff --numstat fa0a3404 HEAD` (+1265/-49 lines).
- `git diff --diff-filter=D --name-only fa0a3404 HEAD` reports **no deleted files**.
- Every new public and private symbol claimed above was confirmed present by name: `UnpackedTeam` (`unpack.rs:322`), `unpack_team` (`unpack.rs:827`), `reject_layers_a_team_may_not_carry` (`unpack.rs:871`), `pack_team` (`pack.rs:1096`), `reject_an_attestation_over_an_unresolved_team` (`pack.rs:566`), `validate_single_layer_pack_preconditions` (`pack.rs:957`).
- `crates/pmcp-package/src/package/workflow.rs` and `crates/pmcp-package/src/error.rs` are both absent from the whole-plan diff.
- Every task-level `<acceptance_criteria>` re-run and passing, EXCEPT the aggregate `make quality-gate` leg, documented above as unrun rather than passed and recorded as `human_judgment: true` (D12).
- The Task-2 falsifiability control was reproduced, observed failing with its output recorded, and reverted; the tree is clean.
- The negative grep for the rejected per-kind spellings was re-run at close and still returns no match.
- Worktree left clean.

---
*Phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b*
*Completed: 2026-08-25*
