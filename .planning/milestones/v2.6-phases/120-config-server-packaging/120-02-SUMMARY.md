---
phase: 120-config-server-packaging
plan: 02
subsystem: packaging
tags: [oci, pmcp-package, cargo-pmcp, media-types, proptest, fuzz-corpus]
requires:
  - phase: 120-01
    provides: config-only tracer, 0.2.0 envelope, MT_SERVER_* constants, index_layers/read_binary_mode helpers
provides:
  - Optional byte-verbatim OpenAPI spec layer (MT_SERVER_OPENAPI_SPEC) with ANNOTATION_TITLE filename, packed and restored under its original name
  - detect_legacy_shape — raw-JSON refusal of the pre-0.2.0 envelope shape, naming both version numbers
  - Hardened media-type layer index — duplicate-media-type rejection and the exactly-one-of binary-arm invariant as typed PackageError::Layout
  - rewrite_manifest_layers test helper + permutation proptest proving layer order is not load-bearing
  - Runnable doctests for both BinaryMode arms and for pack_server round-trip
  - cargo-pmcp detect_kind extended to the four server layer media types, with a seeded fuzz corpus reaching the config-only manifest shape
affects: [120-03, 120-04, 120-05]
actuals:
  tokens: 13900
  tasks: 3
  commits: 3
tech-stack:
  added: []
  patterns:
    - "Content-addressed manifest rewrite in tests: write a NEW manifest blob, carry the index descriptor's annotations across by hand, then REPLACE (never push) the index's single descriptor — the only way to permute layers without dying at verify-before-parse or silently reading the original manifest"
    - "Shape detection over raw JSON, not deserialize errors, when the target struct lacks deny_unknown_fields"
    - "libFuzzer reachability via a checked-in seed corpus file when the target has no seed/alphabet to extend"
key-files:
  created:
    - cargo-pmcp/fuzz/corpus/fuzz_package_kind/config_only_manifest.json
  modified:
    - crates/pmcp-package/src/oci/pack.rs
    - crates/pmcp-package/src/oci/unpack.rs
    - crates/pmcp-package/src/oci/media_types.rs
    - crates/pmcp-package/tests/config_server.rs
    - crates/pmcp-package/tests/negative.rs
    - cargo-pmcp/src/commands/package/kind.rs
    - cargo-pmcp/fuzz/corpus/fuzz_package_kind/.gitignore
key-decisions:
  - "D-14/D-16 held: the spec layer is optional with no absence marker, and one media type carries whatever bytes the author supplied (YAML or JSON) verbatim — format is evident from the filename annotation, never from a parse."
  - "D-10 implemented as SHAPE detection, not producer-version detection: detect_legacy_shape inspects the raw envelope JSON for a `binary_ref` key, because ServerEnvelope has no deny_unknown_fields and a deserialize-error-based check would silently succeed."
  - "PMAT complexity substitution: `pmat analyze complexity` parses 0 functions in crates/pmcp-package, so clippy's cognitive_complexity lint at the same threshold was used instead. This substitution is NOT equivalent and is flagged for human decision."
patterns-established:
  - "Every unpack failure mode is a typed PackageError::Layout naming the media type / layer name / version numbers / missing field — never a panic, and never blob contents in the message"
  - "A permutation proptest is only meaningful if its rewrite helper keeps the production verify chain intact; assert the helper is not a no-op (non-identity permutation must change the manifest digest)"
requirements-completed: [PKG-01, PKG-02]
coverage:
  - id: D-14
    description: "The OpenAPI spec layer is optional — present only when the caller supplies one, absent with no marker otherwise"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/tests/config_server.rs::a_package_packed_without_a_spec_carries_no_spec_layer_at_all"
        status: pass
    human_judgment: false
  - id: D-15
    description: "The spec's original file name is inside the digested manifest (ANNOTATION_TITLE), so renaming it moves the digest"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/tests/config_server.rs::renaming_only_the_spec_file_changes_the_manifest_digest (assert_ne! on two ManifestDigest values)"
        status: pass
    human_judgment: false
  - id: D-16
    description: "One media type, bytes verbatim — a YAML and a JSON spec round-trip identically and are never parsed"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/tests/config_server.rs::a_packed_spec_restores_its_bytes_verbatim_under_its_original_name, ::a_json_spec_round_trips_under_exactly_the_same_media_type_as_a_yaml_one"
        status: pass
      - kind: static
        ref: "grep -A3 'MT_SERVER_OPENAPI_SPEC' crates/pmcp-package/src/oci/pack.rs | grep -c canonicalize => 0"
        status: pass
    human_judgment: false
  - id: D-10
    description: "A pre-0.2.0 envelope shape is refused by name, never mis-deserialized into a 0.2.0 struct"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/tests/negative.rs::an_envelope_carrying_the_legacy_binary_ref_shape_is_refused_by_name, ::well_formed_0_2_0_packages_of_either_binary_mode_still_unpack"
        status: pass
      - kind: static
        ref: "unpack.rs:347 detect_legacy_shape(&envelope_bytes)? precedes unpack.rs:348 serde_json::from_slice::<ServerEnvelope>"
        status: pass
    human_judgment: false
  - id: T-120-07
    description: "Duplicate media type in the layer index is a typed Layout error naming the type — never last-wins"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/tests/negative.rs::two_layers_sharing_one_media_type_are_rejected_naming_that_type"
        status: pass
    human_judgment: false
  - id: BIN-ARM
    description: "Exactly one of bootstrap / binary-ref; both or neither is a typed Layout error, and a null wire digest is rejected naming the missing field"
    verification:
      - kind: unit
        ref: "crates/pmcp-package/tests/negative.rs::a_manifest_carrying_both_binary_arms_is_rejected, ::a_manifest_carrying_neither_binary_arm_is_rejected, ::a_binary_ref_whose_wire_digest_is_null_is_rejected_naming_the_missing_digest"
        status: pass
    human_judgment: false
  - id: D-11
    description: "Layer order is not load-bearing — every read is keyed by media type"
    verification:
      - kind: property
        ref: "crates/pmcp-package/tests/config_server.rs::any_layer_permutation_unpacks_to_an_equal_server (proptest over rewrite_manifest_layers)"
        status: pass
      - kind: unit
        ref: "crates/pmcp-package/tests/config_server.rs::the_permutation_helper_actually_rewrites_the_content_addressed_manifest (non-identity permutation changes the digest — the helper is not a no-op)"
        status: pass
    human_judgment: false
  - id: PKG-KIND
    description: "A config-only, artifactType-less manifest resolves to PackageKind::Server through inspect's real candidate-aggregation path"
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/package/kind.rs::an_artifact_type_less_config_only_manifest_resolves_to_server_via_layer_candidates, ::every_server_layer_media_type_resolves_to_server_on_its_own, ::artifact_type_from_manifest_json_reads_artifact_type_first — `cargo test -p cargo-pmcp --lib package_kind` => 9 passed"
        status: pass
      - kind: fuzz
        ref: "cargo +nightly fuzz run --fuzz-dir cargo-pmcp/fuzz fuzz_package_kind -- -runs=20000 => exit 0, config_only_manifest.json seed ingested"
        status: pass
    human_judgment: false
  - id: EXAMPLE
    description: "CLAUDE.md ALWAYS example-demonstration leg: runnable doctests for both BinaryMode arms and a pack/unpack round-trip"
    verification:
      - kind: doctest
        ref: "make pmcp-package-gate => 2 doctests pass; grep -cE '```(ignore|no_run)' crates/pmcp-package/src/oci/pack.rs => 0"
        status: pass
    human_judgment: false
  - id: GATE
    description: "Crate-wide gate: fmt + clippy -D warnings --all-targets + full test suite"
    verification:
      - kind: gate
        ref: "make pmcp-package-gate => exit 0 (153 tests + 2 doctests). Cited from the pre-ENOSPC run; re-proven by the orchestrator's wave-end post-merge gate."
        status: pass
    human_judgment: false
  - id: COMPLEXITY
    description: "Cognitive complexity <= 25 for unpack_server, index_layers, read_binary_mode, read_named_file_layer, detect_legacy_shape"
    verification:
      - kind: static
        ref: "clippy cognitive_complexity at threshold 25 => 0 hits (proven live: 6 hits at threshold 1). SUBSTITUTED for `pmat analyze complexity`, which parses 0 functions in this crate — see Issues Encountered."
        status: pass
    human_judgment: true
duration: ~29min (execution) + close-out
completed: 2026-08-23
status: complete
---

# Phase 120 Plan 02: OpenAPI Spec Layer, 0.1.x Refusal, and Layer-Index Hardening Summary

**The config-only server package now carries an optional, byte-verbatim OpenAPI spec layer under its original file name; a pre-0.2.0 envelope is refused by name instead of silently mis-read; and the media-type layer index rejects duplicates, enforces exactly one binary arm, and is proven order-independent by a proptest that performs a real content-addressed manifest rewrite.**

## Performance

- 3 tasks, 3 commits, ~29 minutes of execution (commit span 23:29:04 → 23:43:25 on 2026-08-22), plus a separate close-out session after an ENOSPC kill.
- Realized diff: 8 files changed, 1092 insertions, 38 deletions (~55.4 KB ⇒ ~13.9k tokens on the chars/4 scale). The plan's `estimate.tokens` was 48000 on the same nominal scale — the realized diff came in well under, largely because 120-01 had already landed every helper this plan reuses (`index_layers`, `read_named_file_layer`, `read_binary_mode`, the `MT_SERVER_*` constants).

## Accomplishments

**Task 1 — optional OpenAPI spec layer (D-14, D-15, D-16).** `pack_server`'s 120-01 placeholder guard (`"spec layer not yet supported"`) is gone. A `Some(OpenApiSpecFile { file_name, bytes })` is written with `write_blob(vendor_media_type(MT_SERVER_OPENAPI_SPEC), bytes)` — raw bytes, never through `canonicalize`, never parsed — with `ANNOTATION_TITLE` carrying the original file name, pushed last so push order stays deterministic. `None` pushes nothing: no absence marker. `unpack_server` reads it through the existing `read_named_file_layer` against the spec media-type key. Four tests cover the YAML round-trip, the spec-less path, the JSON-under-the-same-media-type case, and the digest-moves-on-rename property.

**Task 2 — 0.1.x refusal and index invariants (D-10, T-120-07..09).** `detect_legacy_shape` (`unpack.rs:284`) parses the envelope blob to `serde_json::Value` and refuses any object holding `LEGACY_ENVELOPE_KEY` (`"binary_ref"`), with an error naming the 0.1.x *shape*, the 0.2.0 format change, and the fact that 0.2.0 does not read 0.1.x packages. It is called at `unpack.rs:347`, one line before the `ServerEnvelope` deserialize at `:348`. Six negative tests pin the refusal, the duplicate-media-type rejection, the both-arms and neither-arm rejections, the null-wire-digest rejection, and — critically — that well-formed 0.2.0 packages of *either* binary mode still unpack (the refusal is narrow). The non-test half of `unpack.rs` contains zero `.unwrap()`/`.expect(` calls.

**Task 3 — permutation proptest, doctests, detect_kind coverage (D-11).** `rewrite_manifest_layers` performs the five-step content-addressed rewrite the Codex review proved necessary: read index → clone the descriptor's annotations → permute `set_layers` → re-serialize with the same `canonicalize` `finalize_pack` uses → `write_manifest` to a NEW blob → reapply annotations → `set_manifests(vec![new_descriptor])` (REPLACE, never push) → `write_index`. A companion test asserts a non-identity permutation changes the digest, so the helper cannot silently become a no-op that makes the proptest pass vacuously. Two runnable doctests satisfy the ALWAYS example leg (both `BinaryMode` arms; a `pack_server` → `unpack_server` round-trip in a `tempdir`). `cargo-pmcp`'s `detect_kind` gained the four server media types in both the `Server` arm and the `KNOWN` proptest table, with the CORRECTED rationale in its doc comment (the resolving path is `inspect.rs`'s candidate aggregation, not the unreachable `layers[0]` tier) and a direct test pinning that `artifact_type_from_manifest_json` returns the CONFIG media type for a PMCP-shaped manifest.

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Wire the optional OpenAPI spec layer (D-14, D-15, D-16) | `df3f39b7` | `oci/pack.rs`, `oci/unpack.rs`, `oci/media_types.rs`, `tests/config_server.rs` |
| 2 | 0.1.x refusal, duplicate-layer rejection, exactly-one-of binary invariant | `a4aa2797` | `oci/unpack.rs`, `tests/negative.rs` |
| 3 | Permutation proptest, both-arm doctests, detect_kind coverage | `1fc8c751` | `tests/config_server.rs`, `cargo-pmcp/src/commands/package/kind.rs`, fuzz corpus + `.gitignore` |

## Files Created/Modified

Created:
- `cargo-pmcp/fuzz/corpus/fuzz_package_kind/config_only_manifest.json` — minimal config-only manifest seed: no top-level `artifactType`, empty-OCI-config descriptor, layers carrying `MT_SERVER_CONFIG` / `MT_SERVER_OPENAPI_SPEC` / `MT_SERVER_BINARY_REF` / `MT_SERVER_ENVELOPE`.

Modified:
- `crates/pmcp-package/src/oci/pack.rs` (+195/-…) — spec-layer arm, rustdoc stating optionality, two doctests.
- `crates/pmcp-package/src/oci/unpack.rs` — `LEGACY_ENVELOPE_KEY` (`:261`), `detect_legacy_shape` (`:284`), spec read, call site at `:347`.
- `crates/pmcp-package/src/oci/media_types.rs` — layer inventory doc lists the spec layer as optional.
- `crates/pmcp-package/tests/config_server.rs` — 4 spec tests, `rewrite_manifest_layers`, the permutation proptest, the reversed-order test, and the helper self-check.
- `crates/pmcp-package/tests/negative.rs` — 6 new negative cases plus the shared hand-built-layout helpers.
- `cargo-pmcp/src/commands/package/kind.rs` — `detect_kind` Server arm, `KNOWN` table, inspect-order candidate-aggregation tests.
- `cargo-pmcp/fuzz/corpus/fuzz_package_kind/.gitignore` — annotated allow-list entry for the new seed.

The fuzz target's SOURCE is deliberately unchanged (`git diff 39e550aa..HEAD -- cargo-pmcp/fuzz/fuzz_targets/fuzz_package_kind.rs` is empty), exactly as the plan specified after the Codex review found it has no seed table to extend.

## Decisions Made

1. **Shape, not provenance.** `detect_legacy_shape`'s message and rustdoc say the envelope "carries the 0.1.x layer shape", never "was written by 0.1.x" — the check is a key-presence test, so any third-party envelope carrying a `binary_ref` extension key gets the same refusal. Under D-10's blanket refusal that is correct behaviour, but the message must not claim knowledge it does not have.
2. **Raw JSON over serde.** The legacy check inspects `serde_json::Value` rather than relying on a deserialize error, because `ServerEnvelope` has no `deny_unknown_fields` (see Issues #5 below — this was confirmed empirically during the RED run, not assumed).
3. **Clippy substituted for PMAT on the complexity criterion** (flagged for human decision — see Issues #3).
4. **Seed corpus over target edits** for the fuzz leg, since the target has no seed/alphabet to extend.

## Deviations from Plan

**1. [Rule 1 - Bug] The plan's `cargo-pmcp` verify command measures nothing.**
- **Found during:** Task 3.
- **Issue:** `cargo test -p cargo-pmcp --lib commands::package::kind` reports `0 passed; 468 filtered out` and exits 0 — a false green. The `package::kind` module is bin-only and is mounted into the lib under the name `package_kind` via `#[path]` at `cargo-pmcp/src/lib.rs:153`, so the module-path filter matches nothing.
- **Fix:** used the corrected filter `cargo test -p cargo-pmcp --lib package_kind` → **9 passed**.
- **Propagate:** sibling plans in this phase carry the same command verbatim; they should be corrected before execution or they will "verify" nothing. This is another instance of the recorded verify-command false-green family.

**2. [Rule 3 - Blocking] The prescribed fuzz seed would never have been committed.**
- **Found during:** Task 3.
- **Issue:** `cargo-pmcp/fuzz/corpus/fuzz_package_kind/.gitignore` ignores `*` and only un-ignores `.gitignore` and `seed_*.toml`. The plan-prescribed path `config_only_manifest.json` matched the `*` rule, so `git add` would have silently done nothing and the acceptance criterion "the file exists" would still have passed in the working tree while the file never reached the branch.
- **Fix:** added an annotated `!config_only_manifest.json` allow-list entry explaining why it is listed by name rather than renamed under the `seed_*` glob (the plan and its acceptance criteria reference this exact path).
- **Commit:** `1fc8c751`.

**3. [Rule 3 - Blocking] `pmat analyze complexity` parses 0 functions in this crate.**
- **Found during:** Task 2 acceptance.
- **Issue:** `pmat` 3.15.0 reports an empty violations list for `crates/pmcp-package` — but from a zero-function parse, most likely because the crate is workspace-excluded with its own `[workspace]` table. An empty violations list from a zero-function parse is not a pass; it is a non-measurement.
- **Fix:** substituted clippy's `cognitive_complexity` lint at the same threshold, and proved it live rather than trusting a green: 6 hits at threshold 1, 0 hits at threshold 25.
- **Flag for human decision:** cognitive complexity as clippy computes it and as PMAT computes it are *related, not identical* metrics. This substitution should be confirmed (or the PMAT parse fixed) rather than inherited silently by later plans.

**4. [Rule 1 - Bug] Obsolete unit test removed.**
- **Found during:** Task 1.
- **Issue:** `pack_server_refuses_a_spec_rather_than_silently_discarding_it` asserted the 120-01 placeholder behaviour that this plan's whole purpose is to remove; it failed as soon as the spec arm was wired.
- **Fix:** the test was updated/removed with the placeholder guard it was pinning. Verified gone: `grep -r 'pack_server_refuses_a_spec_rather_than_silently_discarding_it' crates/pmcp-package/` returns nothing.

## Issues Encountered

**ENOSPC killed the previous execution session at the SUMMARY step.** All three production commits had landed and all verification had already run green; the disk filled during the final `make quality-gate` (exit 134). `target/` was subsequently deleted to recover the volume, so this close-out session ran **no compiles at all** — every compile-time result below is cited from the pre-ENOSPC run and re-proven by the orchestrator's wave-end post-merge gate.

Verification that ran GREEN before the disk filled:
- `make pmcp-package-gate` → exit 0 (fmt + clippy `-D warnings --all-targets` + 153 tests + 2 doctests)
- `cargo test -p cargo-pmcp --lib package_kind` → 9 passed
- `cargo +nightly fuzz run --fuzz-dir cargo-pmcp/fuzz fuzz_package_kind -- -runs=20000` → exit 0, seed ingested
- Every task's `<acceptance_criteria>` grep/CLI check → pass (re-run cheaply in this close-out session; see Self-Check)

**Still owed: `make quality-gate` never completed.** It aborted at ENOSPC (exit 134), so the full workspace gate — not just the `pmcp-package` crate gate — has NOT been proven for this branch. It must pass before this work is pushed.

**RED-run finding (D-10 confirmation, worth recording).** During the failing-test run for Task 2, a 0.1.x-shaped envelope `{"name":"x","version":"1.0.0","binary_ref":{...}}` deserialized **cleanly** into a valid-looking 0.2.0 `ServerPackage` with `binary_ref` silently dropped. `ServerEnvelope` has no `deny_unknown_fields`, so a deserialize-error-based check would have been a no-op that looked like a passing guard. This is the empirical justification for `detect_legacy_shape` inspecting raw JSON, and it is exactly the failure mode T-120-08 predicted.

## User Setup Required

None for this plan. One outstanding human decision: confirm or reject the clippy-for-PMAT complexity-metric substitution (Deviation #3).

## Next Phase Readiness

- The layer set PKG-01 requires is complete: envelope, config, optional spec, and exactly one binary arm — all media-type keyed, all digest-verified, all order-independent.
- Plans 120-03/04/05 build on `unpack_server`'s hardened index and on `detect_kind`'s extended Server arm; both are now proven surfaces.
- **Blocking before push:** a full `make quality-gate` run (see Issues). The wave-end post-merge gate is the designated re-prover for the compile-time claims cited here.
- **Carry forward:** the corrected `cargo test -p cargo-pmcp --lib package_kind` filter (Deviation #1) into the sibling plans that still carry the false-green command.

## Self-Check: PASSED

Verified in this close-out session using cheap checks only (no compiles — `target/` was deleted to recover the full disk):

- **Commits exist on `worktree-agent-a09e2f645e44e35a0`:** `df3f39b7`, `a4aa2797`, `1fc8c751`, all descendants of base `39e550aa`. FOUND.
- **Files exist:** `crates/pmcp-package/src/oci/pack.rs`, `.../unpack.rs`, `.../media_types.rs`, `.../tests/config_server.rs`, `.../tests/negative.rs`, `cargo-pmcp/src/commands/package/kind.rs`, `cargo-pmcp/fuzz/corpus/fuzz_package_kind/config_only_manifest.json`. FOUND.
- **Static acceptance criteria re-run and passing:** placeholder guard gone (0); `MT_SERVER_OPENAPI_SPEC` in `pack.rs` (5); no `canonicalize` near the spec write (0); `assert_ne!` digest assertions (2); `fn detect_legacy_shape` (1); `LEGACY_ENVELOPE_KEY` (4); zero `.unwrap()`/`.expect(` in the non-test half of `unpack.rs`; `detect_legacy_shape` call at `:347` precedes the `ServerEnvelope` deserialize at `:348`; `fn rewrite_manifest_layers` (1); `write_manifest`/`write_index`/`set_annotations` (5 ≥ 3); `set_manifests(vec![` (1); `manifests.push(` (0); `proptest!` (1); no `ignore`/`no_run` doctests in `pack.rs` (0); server media types in `kind.rs` (17 ≥ 8); fuzz target source unchanged (empty diff); seed corpus parses as JSON, carries 3 vendor server media types, and carries no `"artifactType"` (0).
- **Compile-time checks are CITED, not re-run:** `make pmcp-package-gate`, `cargo test -p cargo-pmcp --lib package_kind`, the doctests, and the bounded fuzz run all passed in the pre-ENOSPC execution session and are recorded above with that provenance. The orchestrator's wave-end post-merge gate is the re-prover. `make quality-gate` was never completed and remains owed.

---
*Phase: 120-config-server-packaging*
*Completed: 2026-08-23*
