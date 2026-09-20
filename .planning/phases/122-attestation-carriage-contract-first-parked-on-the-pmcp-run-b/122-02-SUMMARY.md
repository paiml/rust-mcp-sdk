---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 02
subsystem: infra
tags: [oci, oci-spec, attestation, media-types, annotations, pmcp-package, cargo-pmcp, supply-chain]

# Dependency graph
requires:
  - phase: 121-local-round-trip-e2e
    provides: "`crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — the PKG-04 pack/move/unpack regression net this plan had to leave behaviourally identical, and the `test-openapi-server` make target that reaches it"
  - phase: 120-config-slots
    provides: "`validate_pack_preconditions` (extracted from `pack_server` for the cognitive-complexity ceiling) and the `ConfigFile`/`OpenApiSpecFile` optional-layer shape this plan's `AttestationFile` copies"
provides:
  - "`MT_ATTESTATION` — the KIND-NEUTRAL attestation layer media type `application/vnd.pmcp.attestation.v1`, frozen on disk"
  - "The three reverse-DNS annotation keys `run.pmcp.attestation.subject` / `.issuer` / `.payload-type`"
  - "`AttestationFile<'a>` + `write_annotated_layer` (the generalized annotated-layer writer)"
  - "`pack_server`'s sixth positional parameter `attestation: Option<AttestationFile<'_>>`"
  - "`UnpackedAttestation`, `UnpackedServer.attestation` and `read_attestation_layer`"
  - "Attestation rendering in `cargo pmcp package inspect`, for both the attested and unattested states"
  - "A tested, documented statement of D-01's two-digest consequence"
affects: [122-03, 122-06, 122-07, 122-08, 123]

actuals:
  tokens: 93600
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Cross-kind layer media types live in their own `media_types.rs` section, separate from the per-kind sections"
    - "Optional layer metadata rides in LAYER-descriptor annotations (inside the manifest digest), never index-descriptor annotations (applied after it)"
    - "Annotated-layer writing is one generalized helper taking an annotation map, not one helper per annotation vocabulary"

key-files:
  created: []
  modified:
    - crates/pmcp-package/src/oci/media_types.rs
    - crates/pmcp-package/src/oci/pack.rs
    - crates/pmcp-package/src/oci/unpack.rs
    - crates/pmcp-package/src/oci/mod.rs
    - crates/pmcp-package/src/lib.rs
    - crates/pmcp-package/tests/roundtrip.rs
    - cargo-pmcp/src/commands/package/inspect.rs
    - cargo-pmcp/tests/package_inspect.rs

key-decisions:
  - "The attestation media type is KIND-NEUTRAL (`application/vnd.pmcp.attestation.v1`), superseding the `mcp-server`-namespaced spelling written in 122-CONTEXT.md D-05 — an attestation is a claim ABOUT a package, not a part OF one kind of package, and the second carrier kind arrives in this same phase"
  - "Annotation keys use reverse-DNS (`run.pmcp.attestation.*`) because the OCI image-spec says custom keys SHOULD use reverse domain notation and `vnd.pmcp` is a media-type prefix, not a domain"
  - "`write_named_file_layer` was GENERALIZED into `write_annotated_layer` rather than copy-pasted — one mechanism serves the one-key and three-key vocabularies"
  - "D-01's two-digest consequence is pinned by a test whose rustdoc names both wrong 'fixes', so a later reader cannot apply one by accident"
  - "Fixed 4 PRE-EXISTING rustdoc warnings in `pmcp-package` so this plan's zero-warnings acceptance criterion is checkable at all — the root `doc-check` target never reaches this workspace-excluded crate"

patterns-established:
  - "Kind detection and attestation location are independent axes, asserted by a test that alters ONLY `artifactType` and observes the attestation read unchanged"
  - "Opaque-payload carriage is proven with bytes that are neither valid JSON nor valid UTF-8, so a passing test is evidence that nothing parsed them"
  - "Both terminal states of an optional feature are rendered explicitly, so 'absent' is never indistinguishable from 'this build does not know about the feature'"

requirements-completed: []

coverage:
  - id: D1
    description: "Attestation bytes supplied to `pack_server` survive a pack/unpack round trip byte-identically and are rendered by the real `cargo pmcp package inspect` binary, offline, from a fixture whose payload is not parseable JSON"
    requirement: PKGX-01
    verification:
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_renders_an_attested_server_fixture_offline"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#attestation_bytes_that_are_neither_json_nor_utf8_round_trip_byte_identically"
        status: pass
    human_judgment: false
  - id: D2
    description: "D-01's two-digest consequence is a tested fact: the same package packed with and without an attestation yields DISTINCT manifest digests, and the attestation's subject annotation equals the UNATTESTED digest"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#packing_with_and_without_an_attestation_yields_two_distinct_digests"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#the_attestation_subject_annotation_names_the_unattested_digest"
        status: pass
    human_judgment: false
  - id: D3
    description: "Absence is the layer's absence (D-14): an unattested package round-trips to `attestation: None`, carries no attestation layer, and renders as explicitly unattested with no subject-digest line"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_unattested_package_round_trips_with_no_attestation"
        status: pass
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#an_unattested_manifest_carries_no_attestation_layer_at_all"
        status: pass
      - kind: e2e
        ref: "cargo-pmcp/tests/package_inspect.rs#inspect_reports_an_unattested_server_fixture_as_carrying_no_attestation"
        status: pass
    human_judgment: false
  - id: D4
    description: "Layer lookup is media-type-keyed, not positional, and a duplicate attestation layer is rejected rather than last-wins"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#re_ordering_the_manifest_layers_does_not_change_the_attestation_read"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/src/oci/unpack.rs#a_duplicated_attestation_layer_is_rejected_rather_than_last_wins"
        status: pass
    human_judgment: false
  - id: D5
    description: "The kind-neutral media type cannot become a kind signal by accident: altering ONLY `artifactType` (server -> team) does not change how the attestation is located or read — the guard that lets 122-07 reuse `MT_ATTESTATION` unchanged"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "crates/pmcp-package/tests/roundtrip.rs#changing_only_the_artifact_type_does_not_change_how_the_attestation_is_located"
        status: pass
    human_judgment: false
  - id: D6
    description: "Every `pack_server` call site in the repo updated (36), with `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` left behaviourally identical (Phase 121's PKG-04 regression net)"
    verification:
      - kind: integration
        ref: "cargo test -p pmcp-openapi-server --test roundtrip_e2e (8 passed, exit 0; diff is one added `None` line)"
        status: pass
    human_judgment: false
  - id: D7
    description: "The full `make quality-gate` leg of this plan's `<verification>` block"
    verification:
      - kind: other
        ref: "make quality-gate"
        status: unknown
    human_judgment: true
    rationale: "Could not be completed — the run aborted on machine-level disk exhaustion (`No space left on device`), not on any code defect. Every per-crate and per-target gate this plan touches was run individually and passed; the aggregate gate must be re-run on a machine with sufficient free space before this phase ships. See 'Issues Encountered'."

duration: 86 min
completed: 2026-08-25
status: complete
---

# Phase 122 Plan 02: Attestation Carriage Tracer Summary

**A pmcp.run attestation now rides inside a `.pmcp` package as a kind-neutral opaque OCI layer whose subject/issuer/payload-type live in LAYER-descriptor annotations (and therefore inside the manifest digest), survives pack→unpack byte-identically, and renders through the real `cargo pmcp package inspect` binary offline — with D-01's two-digest consequence pinned by a test rather than left as a side effect.**

## Performance

- **Duration:** 86 min
- **Started:** 2026-08-25T18:39:00Z
- **Completed:** 2026-08-25T20:05:00Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments

- **The whole carriage path landed as one tracer**, end to end: `pack_server` → opaque annotated layer → `unpack_server` → `cargo pmcp package inspect`. The phase's architectural risk was resolved after one task rather than after four plans of layered work.
- **The media-type noun was corrected before it reached disk.** Cross-AI review found this plan defined a `mcp-server`-namespaced attestation media type while 122-07 reuses the same constant for teams. The constant shipped KIND-NEUTRAL, with the four reasons written at the constant so the next reader meets them there.
- **Six carriage properties are asserted, not assumed** — two-digest, absence, opacity, duplicate rejection, position independence, kind independence — and two of them carry a recorded negative control proving the assertion is load-bearing.
- **Both terminal states are documented and tested.** `attestation: None` is documented at `unpack_server` as "not a decoding default", and `inspect` says so on its own line rather than rendering nothing.
- **`inspect`'s module docs now state plainly what is NOT verified**: the SDK holds no keys, checks no signature, and identity verification is a remote call this phase deliberately does not make (D-11).

## Task Commits

1. **Task 1 (tracer): End-to-end "this package carries an attestation"** — `cb35074a` (feat)
2. **Task 2: Pin the three carriage properties the tracer only demonstrated once** — `dacc5b60` (test)
3. **Task 3: The unattested state — module docs, third optional layer, unattested render** — `e0f9fe51` (docs)

## Files Created/Modified

- `crates/pmcp-package/src/oci/media_types.rs` — new cross-kind section: `MT_ATTESTATION` + three annotation-key constants; module-doc inventory extended from two to three optional vendor-content layers, plus the cross-kind bullet and D-01's two-digest note
- `crates/pmcp-package/src/oci/pack.rs` — `AttestationFile<'a>`; `write_annotated_layer` (generalization) with `write_named_file_layer` re-expressed as a thin caller; `pack_server`'s sixth parameter, its attestation arm, and its two-digest rustdoc section
- `crates/pmcp-package/src/oci/unpack.rs` — `UnpackedAttestation`, `UnpackedServer.attestation`, `read_attestation_layer` + `required_annotation`; `unpack_server` rustdoc `attestation: None` section and `# Errors` entry; the duplicate-attestation-layer unit test
- `crates/pmcp-package/src/oci/mod.rs`, `crates/pmcp-package/src/lib.rs` — re-exports for the two new public types
- `crates/pmcp-package/tests/roundtrip.rs` — 8 new tests (4 → 12) plus the `rewrite_manifest` helper
- `cargo-pmcp/src/commands/package/inspect.rs` — `render_server` now takes `&UnpackedServer`; `render_attestation` covers both states; module docs state the verification boundary
- `cargo-pmcp/tests/package_inspect.rs` — attested and unattested CLI render tests (3 → 5)
- `crates/pmcp-package/tests/{common/mod.rs,config_server.rs,negative.rs}`, `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — `pack_server` call sites updated
- `crates/pmcp-package/src/oci/config_validation.rs`, `crates/pmcp-package/src/slot/deviation.rs` — pre-existing rustdoc-warning fixes (deviation 2)

## Decisions Made

### Annotation key spellings, and why

| Constant | Value |
|---|---|
| `ANNOTATION_ATTESTATION_SUBJECT` | `run.pmcp.attestation.subject` |
| `ANNOTATION_ATTESTATION_ISSUER` | `run.pmcp.attestation.issuer` |
| `ANNOTATION_ATTESTATION_PAYLOAD_TYPE` | `run.pmcp.attestation.payload-type` |

These were Claude's Discretion under D-04. **Reverse-DNS was chosen** because the OCI image-spec annotations document says custom annotation keys SHOULD use reverse domain notation and reserves `org.opencontainers` for the spec itself; D-04's proposed `vnd.pmcp.attestation.subject` is a **media-type** prefix, not a domain, so it is the wrong shape for an annotation key. There was **no in-repo precedent to follow** — this crate's only annotation key before today is `oci_spec`'s re-exported `org.opencontainers.image.title`. The reason is recorded in the constants' own rustdoc.

`ANNOTATION_ATTESTATION_SUBJECT`'s rustdoc states that its value is the `sha256:<hex>` manifest digest of the **UNATTESTED** package, explicitly not the digest of the package carrying it.

### `pack_server` call sites updated: 36

Across 7 files: `src/oci/pack.rs` (8, its own test module), `src/oci/unpack.rs` (3, its test module), `tests/config_server.rs` (20), `tests/negative.rs` (2), `tests/common/mod.rs` (1), `tests/roundtrip.rs` (1), and `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` (1). All pass `None` — they are unattested by construction.

The edit was done with a balanced-paren script rather than by hand, so the inserted argument lands in the same positional slot at every site regardless of formatting. `roundtrip_e2e.rs`'s diff is exactly **one added line** (`git diff` confirms `1 insertion(+), 0 deletions(-)`), preserving Phase 121's PKG-04 regression net unchanged as the plan requires.

### The media-type noun

Implemented as the plan's decision record specifies: `MT_ATTESTATION = "application/vnd.pmcp.attestation.v1"`. The rejected per-kind-pair alternative is described in **prose only** — neither the rejected identifiers nor the rejected media-type fragment appear anywhere in `crates/` or `cargo-pmcp/`, so the two negative-grep acceptance criteria (this plan's and 122-07's) stay valid rather than being invalidated by the comment that explains them.

## Negative Controls (falsifiability, required by Task 2)

Both were run against the real tree, observed to fail, and then fully reverted. `git diff` against the Task-1 commit confirms `pack.rs` returned to its committed state.

### Control A — `index_layers`'s duplicate guard

**Mutation:** replaced the `if index.insert(..).is_some() { return Err(..) }` guard in `crates/pmcp-package/src/oci/unpack.rs` with a bare `index.insert(..)` (silent overwrite / last-wins).

**Observed:**

```
---- oci::unpack::tests::a_duplicated_attestation_layer_is_rejected_rather_than_last_wins stdout ----
thread '...' panicked at src/oci/unpack.rs:849:42:
called `Result::unwrap_err()` on an `Ok` value: UnpackedServer { ... attestation: Some(UnpackedAttestation { ... }) }
test result: FAILED. 0 passed; 1 failed
```

The crafted two-attestation-layer layout unpacked **cleanly**, silently keeping one of the two — exactly the shadowing the guard exists to prevent. Guard restored; test green again (`1 passed`).

### Control B — annotations moved off the LAYER descriptor

**Mutation:** the attestation arm of `pack_server` was changed to write the layer **bare** (`layout.write_blob(...)`, no annotations) and to hand the three annotations to `finalize_pack` to apply on the **INDEX** descriptor instead — i.e. after `write_manifest` has already computed the manifest digest.

**Observed:**

```
---- the_attestation_subject_annotation_names_the_unattested_digest stdout ----
thread '...' panicked at tests/roundtrip.rs:380:10:
an attested package's attestation layer must carry a subject annotation
test result: FAILED. 0 passed; 1 failed
```

With the metadata on the index descriptor there is no subject annotation on the layer at all, so the subject-equality assertion cannot even read the value it compares — the tamper-evidence chain is broken and the test detects it. This is the T-122-01 mitigation the threat register claims, demonstrated rather than asserted. Surgery reverted in full.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `invalid_from_utf8` lint would have failed the `-D warnings` gate**

- **Found during:** Task 2 (opacity test)
- **Issue:** `std::str::from_utf8(OPAQUE_ATTESTATION_BYTES)` called on a constant lets rustc see through to the literal and fire `invalid_from_utf8` — which `clippy -- -D warnings` in `make pmcp-package-gate` turns into a build failure. The assertion was correct; the lint was reacting to the compiler proving it statically.
- **Fix:** routed the check through an owned `Vec<u8>` so it is evaluated at runtime, with a comment explaining why the indirection exists so it is not "simplified" away later. The assertion is unchanged in strength.
- **Files modified:** `crates/pmcp-package/tests/roundtrip.rs`
- **Verification:** `cargo clippy --manifest-path crates/pmcp-package/Cargo.toml --all-targets -- -D warnings` → clean
- **Committed in:** `dacc5b60`

**2. [Rule 3 - Blocking] Four pre-existing rustdoc warnings made a Task 3 acceptance criterion unsatisfiable**

- **Found during:** Task 3
- **Issue:** the criterion requires `cargo doc --manifest-path crates/pmcp-package/Cargo.toml --no-deps` to emit **no** warnings. At baseline it emitted **4**, none introduced by this plan: two public-doc links to private items (`index_layers`, `detect_legacy_shape` in `unpack.rs`) and two redundant explicit link targets (`config_validation.rs:278`, `slot/deviation.rs:25`). These were invisible to the repo's gate because `make doc-check` runs root-workspace `cargo doc` and `pmcp-package` is **workspace-excluded** — so the root gate never reaches this crate at all.
- **Fix:** demoted the two private-item links to plain code spans and removed the two redundant explicit link targets. No prose meaning changed.
- **Files modified:** `crates/pmcp-package/src/oci/unpack.rs`, `crates/pmcp-package/src/oci/config_validation.rs`, `crates/pmcp-package/src/slot/deviation.rs`
- **Verification:** `cargo doc --manifest-path crates/pmcp-package/Cargo.toml --no-deps` → zero warnings
- **Committed in:** `e0f9fe51`
- **Note:** this is a deliberate step slightly outside the plan's file list. Left unfixed, the criterion would have been permanently red and this crate's rustdoc would stay outside any gate. Worth surfacing to Phase 124 (PKGR-01), which owns the workspace-excluded-crate gate blind spot.

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both were required to make this plan's own acceptance criteria checkable. No scope creep — deviation 2 touches two files outside the plan's list, in the same crate, for documentation-link fixes only.

## Issues Encountered

**`make quality-gate` could not be completed — machine-level disk exhaustion, not a code defect.**

The plan's `<verification>` block requires `make quality-gate` green. The run **exited 2**, and every error in its log is an I/O failure:

```
rustc-LLVM ERROR: IO failure on output stream: No space left on device
error: could not compile `clap_builder` (lib) due to 1 previous error
error: couldn't create a temp dir: No space left on device (os error 28)
```

`df -h /` showed **117 MiB free** on a 100%-full APFS container. This is the failure mode recorded in this project's own debugging notes ("Disk exhaustion fakes code regressions — run `df -h /` BEFORE bisecting"), so it was checked before anything was attributed to the change.

Worth flagging explicitly: the background-task notification reported **"exit code 0"** for this run. That was the exit status of the trailing `echo | tee`, not of `make` — the real status was captured as `QUALITY_GATE_EXIT=2` inside the log. A reviewer trusting the notification would have recorded a false green.

**What was done:** ~7.2 GB of this worktree's own regenerable build artifacts (`target/`, gitignored, created by this session) were deleted, recovering 6.5 GiB. The full aggregate gate was then **deliberately not re-run**: it needs more headroom than remained, and re-exhausting a shared disk mid-wave would break the sibling worktree agents running in parallel. Instead every gate leg this plan actually touches was run individually, from a cold `target/`, and all passed:

| Check | Result |
|---|---|
| `make pmcp-package-gate` (fmt + clippy `-D warnings` + tests, workspace-excluded crate) | **exit 0** — 256 tests |
| `cargo test -p cargo-pmcp --test package_inspect` | **exit 0** — 5 passed |
| `cargo test -p pmcp-openapi-server --test roundtrip_e2e` | **exit 0** — 8 passed |
| `cargo fmt --all -- --check` (root workspace) | **exit 0** |
| `cargo doc --manifest-path crates/pmcp-package/Cargo.toml --no-deps` | zero warnings |
| `grep -rn 'mcp-server.attestation' crates/ cargo-pmcp/` | no match (exit 1) |

**Open item for the verifier:** the aggregate `make quality-gate` must be re-run on a machine with sufficient free space before this phase ships. It is recorded as `human_judgment: true` in the coverage block (D7) rather than claimed green, and appended to `.planning/WINDOWS.md` as an `unrun-verify`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Ready.** The tracer froze exactly what the six downstream plans build on:

- **122-03** (subject-check verdict + non-zero exit) has `UnpackedServer.attestation` to compare against, and `render_attestation`'s doc comment already names the third state it must add. `inspect` deliberately makes no exit-code decision yet.
- **122-06** inherits the opacity guarantee over generated bytes.
- **122-07** (team carrier) can reuse `MT_ATTESTATION`, the annotation vocabulary and `read_attestation_layer` **with no kind dispatch** — and the kind-independence test is the standing guard that this stays true.
- **122-08** owns the publish decision. Note the API break this plan lands: a new positional parameter on `pack_server` and a new public field on `UnpackedServer` are breaking changes for every downstream consumer of the published `pmcp-package` crate. In-repo it is a revert plus 36 call sites; **publishing** the version that names the break is the genuinely one-way step, and that is 122-08's blocking checkpoint. This phase publishes nothing.

**Concerns:**

- The aggregate `make quality-gate` is unverified (above). Not a known defect — an unrun check.
- `crates/pmcp-package/Cargo.toml` version is still `0.2.0`. Bumping it, and the `cargo-pmcp` / `pmcp-openapi-server` pins, is Phase 124's release half — deliberately not touched here.

## Self-Check: PASSED

- All 8 `key-files.modified` entries exist on disk (`[ -f ]` per path).
- `git log --oneline --all | grep` finds all three commit hashes: `cb35074a`, `dacc5b60`, `e0f9fe51`.
- All task-level `<acceptance_criteria>` re-run and passing, except the aggregate `make quality-gate` leg, which is documented above as unrun rather than passed.
- `pack_server` confirmed to have exactly six parameters with `attestation` in position five and `layout` last.
- `git diff --diff-filter=D HEAD~3 HEAD` reports no deleted files.

---
*Phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b*
*Completed: 2026-08-25*
