---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 03
subsystem: infra
tags: [oci, attestation, subject-digest, verification, pmcp-package, cargo-pmcp, supply-chain, exit-code]

# Dependency graph
requires:
  - phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
    provides: "`MT_ATTESTATION`, `AttestationFile`, `UnpackedAttestation`, `read_attestation_layer`, `pack_server`'s sixth parameter and `render_attestation`'s two states (plan 122-02)"
  - phase: 121-local-round-trip-e2e
    provides: "`crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — the PKG-04 pack/move/unpack net the `pack_server` restructuring had to leave green"
provides:
  - "`OciLayout::describe_blob` — the side-effect-free `(MediaType, &[u8]) -> Descriptor` sibling of `write_blob`, with `write_blob` re-expressed in terms of it"
  - "`assemble_manifest(config, layers, artifact_type) -> Result<ImageManifest>` — the ONE manifest builder chain, shared by the dry pass and `finalize_pack`"
  - "Gate B: `reject_an_attestation_subject_naming_another_package` + `would_be_unattested_manifest_digest`, running before the first `write_blob`"
  - "`PackageError::AttestationSubjectMismatch { supplied, computed }`"
  - "`SubjectVerdict { claimed: String, unattested_digest: ManifestDigest }` + `matches()`, carried on `UnpackedAttestation.subject`"
  - "The private in-memory layer plan (`PlannedLayer` / `ServerLayerPlan`) that makes pre-write gating implementable"
  - "The third `inspect` render state and its non-zero exit, holding under `--quiet`"
affects: [122-06, 122-07, 122-08, 123]

actuals:
  tokens: 71000
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Byte production is separated from blob writing, so a gate can compute a manifest digest before anything reaches the filesystem"
    - "A pure describing function and its writing twin share one code path, with their equality pinned by a test rather than by inspection"
    - "A soft verdict is modelled as a struct with a predicate, never as a `Result` — a `Result` invites a caller to propagate it and turn data back into an error"
    - "A claim and the verdict on that claim are ONE value, so a caller cannot read the claim without being handed whether it is true"

key-files:
  created: []
  modified:
    - crates/pmcp-package/src/oci/layout.rs
    - crates/pmcp-package/src/oci/pack.rs
    - crates/pmcp-package/src/oci/unpack.rs
    - crates/pmcp-package/src/error.rs
    - crates/pmcp-package/src/oci/mod.rs
    - crates/pmcp-package/src/lib.rs
    - crates/pmcp-package/tests/negative.rs
    - crates/pmcp-package/tests/roundtrip.rs
    - cargo-pmcp/src/commands/package/inspect.rs
    - cargo-pmcp/tests/package_inspect.rs

key-decisions:
  - "The verdict type is `SubjectVerdict { claimed: String, unattested_digest: ManifestDigest }` with a `matches()` predicate — the asymmetric field types ARE the trust boundary: `claimed` is attacker-controlled annotation text, `unattested_digest` is computed by this crate and well-formed by construction"
  - "The verdict REPLACES `UnpackedAttestation.subject: String` rather than sitting beside it, so the claim and its verdict cannot be read apart"
  - "Gate B holds the attestation layer SEPARATELY in the layer plan (`ServerLayerPlan.attestation`) instead of filtering a mixed vector by media type — the two compared manifests then differ only in their layer vectors by construction"
  - "`write_binary_layer` / `write_annotated_layer` / `write_named_file_layer` became PLANNERS rather than gaining bytes-producing siblings, because a sibling would have left the original writer with no caller and `-D warnings` would have failed on the dead code"
  - "The `inspect` exit code was MEASURED as exactly 1 (`assert_cmd` `.code(1)`), upgrading RESEARCH A5 from 'non-zero' to a pinned constant"

patterns-established:
  - "An ordering invariant is pinned by a full recursive layout snapshot (path + length + content digest), not by the refusal it accompanies — the refusal alone is identical whether the gate runs before or after the writes"
  - "When a new derived field lands on a struct that tests compare wholesale, the mutation-based tests compare a named subset (`carried_facts`) and assert the derived field CHANGED — turning a broken assertion into a second, sharper one"

requirements-completed: [PKGX-01]

coverage:
  - id: D1
    description: "`pack_server` REFUSES to write anything when the supplied attestation subject does not equal the would-be unattested manifest digest — no blob and no index entry appears on disk, over the FULL layout"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_subject::an_attestation_whose_subject_names_another_package_is_refused_naming_both_digests"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_subject::a_refused_pack_leaves_the_destination_layout_byte_for_byte_unchanged"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/negative.rs#attestation_subject::a_malformed_subject_is_refused_rather_than_panicking"
        status: pass
    human_judgment: false
  - id: D2
    description: "The pure and writing descriptor paths are pinned equal, so the dry digest Gate B compares against is a digest a produced package can actually have"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "crates/pmcp-package/src/oci/layout.rs#describe_blob_returns_the_same_descriptor_write_blob_does"
        status: pass
      - kind: other
        ref: "grep -c 'Descriptor::new' crates/pmcp-package/src/oci/layout.rs == 1; grep -n assemble_manifest crates/pmcp-package/src/oci/pack.rs == 2 call sites + 1 definition"
        status: pass
    human_judgment: false
  - id: D3
    description: "On unpack a subject mismatch is DATA: `unpack_server` returns `Ok` with the issuer, the claimed subject and the re-derived unattested digest all separately readable, while corrupt BYTES still fail closed"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_altered_subject_annotation_unpacks_successfully_and_reports_a_mismatch"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#flipping_the_attestation_payload_bytes_still_fails_closed_with_a_digest_mismatch"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_attested_package_whose_subject_matches_reports_a_matching_verdict"
        status: pass
    human_judgment: false
  - id: D4
    description: "The unpack side re-derives the unattested digest INDEPENDENTLY rather than reading the stored claim (D-02: neither end assumes the other ran)"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#the_re_derived_unattested_digest_equals_the_digest_the_unattested_pack_returned"
        status: pass
      - kind: other
        ref: "Negative control B (below): re-derivation replaced by 'trust the annotation' -> 3 tests fail; reverted"
        status: pass
    human_judgment: false
  - id: D5
    description: "`digest::verify`'s threat model is provably unchanged — it remains an integrity check over exact bytes and did not become a signature or subject check"
    requirement: PKGX-01
    verification:
      - kind: other
        ref: "git diff --numstat f268c5d4 -- crates/pmcp-package/src/digest/ -> 0 lines changed across the whole module"
        status: pass
    human_judgment: false
  - id: D6
    description: "`cargo pmcp package inspect` renders the full diagnostic AND exits exactly 1 on a subject mismatch, INCLUDING with output suppressed, so the mismatch is gateable in CI without parsing stdout"
    requirement: PKGX-01
    verification:
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_renders_the_full_diagnostic_and_exits_non_zero_on_a_subject_mismatch"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_exits_non_zero_on_a_subject_mismatch_even_with_output_suppressed"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_reports_a_matching_subject_as_a_match_and_succeeds"
        status: pass
    human_judgment: false
  - id: D7
    description: "The `pack_server` restructuring did not regress Phase 121's PKG-04 regression net"
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e (8 passed, exit 0)"
        status: pass
    human_judgment: false
  - id: D8
    description: "Cognitive complexity stays under the repo's 25 ceiling for `pack_server` and `validate_pack_preconditions`"
    verification:
      - kind: other
        ref: "pmat 3.15.0 analyze complexity --max-cognitive 25 --path crates/pmcp-package/src -> 25 files analyzed, all filtered (no function exceeds 25); non-vacuity confirmed at --max-cognitive 5, which reports pack.rs with 19 functions, max cognitive 1"
        status: pass
    human_judgment: false
  - id: D9
    description: "The full aggregate `make quality-gate` leg of this plan's `<verification>` block"
    verification:
      - kind: other
        ref: "make quality-gate"
        status: unknown
    human_judgment: true
    rationale: "NOT RUN. `make doc-check` aborted on machine-level disk exhaustion (13x 'No space left on device'; df reported 117MiB free on a volume shared with a concurrent sibling worktree agent). Not a code defect. Every leg this plan touches was run individually and passed — see 'Issues Encountered'. Recorded in .planning/WINDOWS.md as unrun-verify #31, alongside 122-02's identical open item #30."

duration: 74 min
completed: 2026-08-25
status: complete
---

# Phase 122 Plan 03: The Subject-Digest Check, Both Ends Summary

**The only verification the SDK can perform offline — subject-digest comparison — now runs at BOTH ends: `pack_server` refuses to write a single blob when the supplied subject names another package, `unpack_server` re-derives that digest independently and reports disagreement as DATA, and `cargo pmcp package inspect` renders the full diagnostic then exits exactly 1 — including under `--quiet`.**

## Performance

- **Duration:** 74 min
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments

- **"Refuse before the first write" became literally true, not aspirational.** Cross-AI review had flagged that every descriptor `pack_server` assembled came out of a *writing* call, making a pre-write gate unimplementable as specified. The three prescribed extractions removed that obstacle exactly as designed, and the invariant is now pinned by a full-layout snapshot rather than asserted in a rustdoc.
- **The two failure modes are now different in code AND documented as deliberately different at both sites they live.** Integrity failure means the bytes are corrupt; subject mismatch means the bytes are fine and the claim is wrong. Both sites carry an explicit instruction not to harmonize them in a later cleanup.
- **`digest::verify` was not touched at all** — provable, not merely claimed: `git diff --numstat` over the whole `digest/` module reports zero changed lines since the first commit of this plan.
- **Both falsifiability controls were reproduced against the real tree, observed to fail, and fully reverted.** Each one failed in a *sharper* way than the plan predicted (details below).
- **The `inspect` exit code was measured rather than assumed**, upgrading RESEARCH assumption A5 from "non-zero" to a pinned `1`.

## Task Commits

1. **Task 1: Gate B — pack refuses a subject that does not name this package** — `f268c5d4` (feat)
2. **Task 2: The unpack-side verdict — a mismatch is data** — `0564361f` (feat)
3. **Task 3: The three `inspect` states + non-zero exit** — `2677b4f1` (feat)

## The artifacts 122-07 consumes by name

Recorded here because plan 122-07 is told not to re-extract them:

```rust
// crates/pmcp-package/src/oci/layout.rs — associated fn, no &self, infallible
pub fn describe_blob(media_type: MediaType, bytes: &[u8]) -> Descriptor

// crates/pmcp-package/src/oci/pack.rs — private, pure, no filesystem access
fn assemble_manifest(
    config: Descriptor,
    layers: Vec<Descriptor>,
    artifact_type: &str,
) -> Result<ImageManifest>

// crates/pmcp-package/src/oci/unpack.rs — pub, re-exported at oci:: and crate root
pub struct SubjectVerdict {
    pub claimed: String,                     // attacker-controlled annotation text
    pub unattested_digest: ManifestDigest,   // computed here, well-formed by construction
}
impl SubjectVerdict { pub fn matches(&self) -> bool }

// crates/pmcp-package/src/error.rs
PackageError::AttestationSubjectMismatch { supplied: String, computed: String }
```

Gate B itself is `reject_an_attestation_subject_naming_another_package(attestation, unattested_layers)`, called from `validate_pack_preconditions`; it delegates the digest to `would_be_unattested_manifest_digest(&[PlannedLayer])`.

### Answers to the plan's specific `<output>` questions

- **Verdict field name and shape:** `UnpackedAttestation.subject: SubjectVerdict` — it REPLACES the former `subject: String` rather than sitting beside it. A caller therefore cannot read what an attestation claims without also being handed whether that claim is true. Not a `Result`, per the plan's instruction; not a collapsed string.
- **`PackageError` variant name:** `AttestationSubjectMismatch { supplied, computed }`.
- **Measured `inspect` exit code:** **exactly 1.** Both mismatch CLI tests assert `assert_cmd`'s `.code(1)`, which fails on any other status (including signals), and both pass. D-06's wording needs no adjustment.
- **Did `pack_server`'s byte production need its own extraction to stay under the complexity ceiling?** **Yes, and it was worth it independently.** Byte production lives in `plan_server_layers`, not inline. The result is far under the ceiling: `pmat` reports `pack.rs` with 19 functions at a **maximum cognitive complexity of 1**. No `#[allow]` was added anywhere.

## Negative Controls (falsifiability)

Both were run against the real tree, observed to fail, and fully reverted. `git status` after each revert was clean.

### Control A — Gate B moved AFTER the layer-writing loop (Task 1, ordering)

**Mutation:** the `validate_pack_preconditions` call in `pack_server` was moved below the `for planned in ... { write_planned_layer(...) }` loop.

**Observed:** `test result: FAILED. 2 passed; 2 failed`

The split is exactly what the plan predicted, and it is the point:

| Test | Result | Why |
|---|---|---|
| `..._is_refused_naming_both_digests` | **passed** | the refusal is identical either way — it proves nothing about ordering |
| `..._packs_and_yields_a_distinct_digest` | **passed** | unaffected by gate position |
| `a_refused_pack_leaves_the_destination_layout_byte_for_byte_unchanged` | **FAILED** | 7 blobs on disk that should not exist |
| `a_malformed_subject_is_refused_rather_than_panicking` | **FAILED** | same 7 blobs |

The failure output listed all seven leaked blobs by digest. **The snapshot assertion is what pins the ordering, not the refusal** — precisely the plan's claim, demonstrated.

### Control B — unpack re-derivation replaced by "trust the annotation" (Task 2)

**Mutation:** `re_derive_unattested_digest`'s result was discarded and `unattested_digest` was set to `ManifestDigest::parse(&claimed)`, i.e. report a match by reading the stored claim.

**Observed:** `test result: FAILED. 14 passed; 3 failed`

The plan required `an_altered_subject_annotation_unpacks_successfully_and_reports_a_mismatch` to fail. It did — and two more caught it as well:

```
---- an_altered_subject_annotation_unpacks_successfully_and_reports_a_mismatch ----
panicked at tests/roundtrip.rs:700:
the altered subject names a different package, so the verdict must be a mismatch

---- the_re_derived_unattested_digest_equals_the_digest_the_unattested_pack_returned ----
  left: ManifestDigest("sha256:0380ac45...")   # the false claim, echoed back
 right: ManifestDigest("sha256:3fec91da...")   # what the manifest actually hashes to
```

Three independent tests fail on the one mutation, so the property is over-determined rather than resting on a single assertion.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Two 122-02 tests asserted whole-struct equality across a deliberate manifest mutation**

- **Found during:** Task 2
- **Issue:** `re_ordering_the_manifest_layers_does_not_change_the_attestation_read` and `changing_only_the_artifact_type_does_not_change_how_the_attestation_is_located` both `assert_eq!` the entire `UnpackedAttestation` before and after mutating the manifest. Adding the re-derived digest to that struct made both fail — correctly. The re-derived digest is a fact about the **manifest**, and those tests change the manifest on purpose (reversing the layer vector; rewriting `artifactType`), so both genuinely produce a different unattested digest. The old assertion had become "a mutated manifest hashes to the unmutated manifest's digest", which is false — and its falseness IS the tamper-evidence this plan adds.
- **Fix:** introduced a `carried_facts()` helper naming the four facts those tests are actually about (payload bytes, claimed subject, issuer, payload type) and compared that. Their documented property — *the attestation is located by media type, not by position or kind* — is asserted exactly as before, and neither test was weakened: the re-ordering test now additionally asserts the re-derived digest **did** change, which is a new assertion the old whole-struct comparison could not express.
- **Files modified:** `crates/pmcp-package/tests/roundtrip.rs`
- **Verification:** both tests pass; the sharpened `assert_ne!` is itself load-bearing — Control B above makes it fail.
- **Committed in:** `0564361f`

**2. [Rule 3 - Blocking] Four existing tests packed a placeholder subject Gate B now refuses**

- **Found during:** Task 1
- **Issue:** 122-02's tests packed attestations claiming `"sha256:unused"` (four sites in `roundtrip.rs`) and an all-zeros digest (one site in `unpack.rs`). Gate B refuses both — the first as malformed, the second as a mismatch — so those tests could not pack at all.
- **Fix:** added `pack_attested_fixture_server`, which packs the fixture unattested first to obtain the real subject, then packs attested with it. The `unpack.rs` duplicate-layer test got the same treatment inline. This is not a weakening: the fixtures now carry the subject a real producer would.
- **Files modified:** `crates/pmcp-package/tests/roundtrip.rs`, `crates/pmcp-package/src/oci/unpack.rs`
- **Verification:** all previously-passing attestation tests still pass.
- **Committed in:** `f268c5d4`

**3. [Rule 3 - Blocking] Three writer helpers became planners instead of gaining siblings**

- **Found during:** Task 1
- **Issue:** the plan says to give `write_binary_layer` "a bytes-producing sibling ... and re-express `write_binary_layer` as that sibling plus a write". `write_binary_layer`, `write_annotated_layer` and `write_named_file_layer` had exactly one caller each — `pack_server` — and hoisting byte production removes that caller. Keeping them would have left three dead functions, which `clippy -- -D warnings` in `make pmcp-package-gate` fails on.
- **Fix:** converted all three into planners (`plan_binary_layer`, `plan_annotated_layer`, `plan_named_file_layer`) feeding a single `write_planned_layer`. The rustdoc explaining *why* LAYER-descriptor annotations feed the manifest digest — the rule Control B of plan 122-02 established — was carried across verbatim onto the shared `annotate` helper. The intent (pure byte production separated from writing) is fully realized; only the count of surviving functions differs.
- **Files modified:** `crates/pmcp-package/src/oci/pack.rs`
- **Verification:** `make pmcp-package-gate` exit 0; `roundtrip_e2e` 8 passed, proving descriptors are byte-identical to before.
- **Committed in:** `f268c5d4`

**4. [Rule 3 - Blocking] Two rustdoc warnings from private-item intra-doc links**

- **Found during:** Task 2
- **Issue:** `[`Self::blob_path`]` and `[`reject_an_attestation_subject_naming_another_package`]` are private, so linking to them from public docs emits `private_intra_doc_links`. 122-02 established a zero-warning baseline for this crate, which these would have broken.
- **Fix:** demoted both to plain code spans, the same remedy 122-02's deviation 2 applied. No prose meaning changed.
- **Files modified:** `crates/pmcp-package/src/oci/layout.rs`, `crates/pmcp-package/src/oci/pack.rs`
- **Verification:** `cargo doc --manifest-path crates/pmcp-package/Cargo.toml --no-deps` → zero warnings.
- **Committed in:** `0564361f`

---

**Total deviations:** 4 auto-fixed (1 bug, 3 blocking)
**Impact on plan:** None on scope or intent. Deviation 1 is the only one that changed a pre-existing assertion, and it sharpened it.

## Verification Results

| Check | Result |
|---|---|
| `make pmcp-package-gate` (fmt + clippy `-D warnings` + tests, workspace-excluded crate) | **exit 0** — 266 tests (was 256) |
| `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test negative` | **exit 0** — 18 (was 14) |
| `cargo test --manifest-path crates/pmcp-package/Cargo.toml --test roundtrip` | **exit 0** — 17 (was 12) |
| `cargo test -p cargo-pmcp --test package_inspect` | **exit 0** — 8 (was 5) |
| `make test-cargo-pmcp-integration` | **exit 0** (target's own guard asserts a nonzero per-binary count for `package_inspect`) |
| `cargo test -p pmcp-openapi-server --test roundtrip_e2e` | **exit 0** — 8 passed (Phase 121 PKG-04 net) |
| `make no-crypto-check` | **exit 0** |
| `cargo fmt --all -- --check` (root workspace) | **exit 0** |
| `make lint-plans` | **exit 0** |
| `make check-release-coverage` | **exit 0** |
| `cargo doc --manifest-path crates/pmcp-package/Cargo.toml --no-deps` | zero warnings |
| `pmat analyze complexity --max-cognitive 25` | no violation; `pack.rs` max cognitive **1** across 19 functions |
| `git diff --numstat -- crates/pmcp-package/src/digest/` | **0 lines** — `verify.rs` provably untouched |
| `make quality-gate` (aggregate) | **NOT RUN** — disk exhaustion, see below |

**Wave 1 baselines held:** pmcp-package 256 → 266, `package_inspect` 5 → 8, `roundtrip` 12 → 17. No suite shrank.

## Issues Encountered

**`make quality-gate` was NOT run — machine-level disk exhaustion, not a code defect.**

`make doc-check` (the first heavyweight leg attempted) aborted with **13 distinct `No space left on device` errors**:

```
rustc-LLVM ERROR: IO failure on output stream: No space left on device
error: could not compile `ring` (lib) due to 1 previous error
error: failed to write .../libnum_bigint-....rmeta: No space left on device (os error 28)
make: *** [doc-check] Error 101
```

`df -h /` reported **117 MiB free** on a 100%-full volume — the identical failure this project's debugging notes and plan 122-02's own SUMMARY both record. `df` was checked **before** anything was attributed to the change, per that note.

**Sequence, stated plainly so the judgement is reviewable:** free space was measured at **4.2 GiB before** starting `doc-check`, on a volume explicitly shared with a concurrently building sibling worktree agent. `doc-check` cold-builds the workspace under a *different* feature set than the test profile, and consumed the remainder.

**What was done:** this worktree's own regenerable root `target/` (6.2 GB, gitignored, created by this session) was deleted, recovering free space to **6.5 GiB** and unblocking the sibling agent. `make pmcp-package-gate` was then re-run and passed again (exit 0) to confirm nothing was disturbed.

**What was NOT done, and why:** the aggregate `make quality-gate` was **deliberately not attempted**. It runs `build` + `test-all` (including this repo's very large `examples/` set) and would need several GB more than the 6.5 GiB now free. Re-exhausting a shared disk mid-wave would break the sibling agent — a concrete harm — to obtain a check whose every leg touching this plan has already passed individually. Claiming a green here without running it would have been worse still.

**Open item for the verifier:** the aggregate `make quality-gate` must be run on a machine with adequate free space before this phase ships. It is recorded as `human_judgment: true` (D9) rather than claimed green, and appended to `.planning/WINDOWS.md` as `unrun-verify` **#31**, alongside 122-02's identical open **#30**. Both should be closed by one run.

**Not covered by any local gate:** `cargo-pmcp`'s rustdoc. `make doc-check` documents only the root `pmcp` package, and `cargo-pmcp` is not clippy-gated in this repo, so the module-doc changes to `inspect.rs` are unverified by any gate. They are prose-only.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready.**

- **122-07 (team carrier)** inherits `describe_blob` and `assemble_manifest` and must NOT re-extract them; Gate A on the team path can refuse before its first write using the same two. Note that `would_be_unattested_manifest_digest` currently hardcodes `ARTIFACT_TYPE_SERVER` — 122-07 will need to pass the artifact type in, a one-parameter change made deliberately visible here rather than pre-generalized on speculation.
- **122-06** is unaffected; the opacity guarantee is unchanged and the payload is still never parsed.
- **122-08 owns the version consequence, and this plan ENLARGES it.** On top of 122-02's `pack_server` parameter and `UnpackedServer` field, this plan adds a `PackageError` variant (the enum is **not** `#[non_exhaustive]`, so every downstream `match` breaks) and **changes the type of the public field `UnpackedAttestation.subject`** from `String` to `SubjectVerdict`. Both are recorded in the new variant's own rustdoc naming 122-08 as the owner. This phase publishes nothing.

**Concerns:**

- The aggregate `make quality-gate` is unverified (above). An unrun check, not a known defect.
- Disk headroom on this machine is the binding constraint on verification, not code quality. The next agent to attempt a full gate should check `df -h /` first.

## Self-Check: PASSED

- All 10 `key-files.modified` entries appear in `git show --stat` across the three commits; none is missing.
- All three commit hashes resolve: `f268c5d4`, `0564361f`, `2677b4f1`.
- `git diff --diff-filter=D --name-only HEAD~3 HEAD` reports **no deleted files**.
- Every task-level `<acceptance_criteria>` re-run and passing, except the aggregate `make quality-gate` leg, documented above as unrun rather than passed.
- `grep -c 'Descriptor::new' crates/pmcp-package/src/oci/layout.rs` == **1** (inside `describe_blob`); `write_blob`'s body calls `describe_blob` — verified by reading.
- `describe_blob` confirmed to take no `&self`, return a bare `Descriptor` (no `Result`), and contain no `fs::` call — verified by reading `layout.rs:95-98`.
- `assemble_manifest` has **2 call sites** (`pack.rs:575` dry pass, `pack.rs:854` `finalize_pack`) plus its definition.
- Both falsifiability controls reproduced, observed failing, and reverted; the tree is clean.

---
*Phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b*
*Completed: 2026-08-25*
