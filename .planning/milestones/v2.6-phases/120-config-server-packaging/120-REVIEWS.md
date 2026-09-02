---
phase: 120
reviewers: [codex, gemini]
reviewed_at: 2026-08-23T04:49:25Z
plans_reviewed: [120-01-PLAN.md, 120-02-PLAN.md, 120-03-PLAN.md, 120-04-PLAN.md, 120-05-PLAN.md]
models:
  codex: "gpt-5.6-sol (reasoning=low)"
  gemini: "unknown"
model_sources:
  codex: "banner"
  gemini: "unknown"
---

# Cross-AI Plan Review — Phase 120

## Codex Review

# Cross-AI Plan Review — Phase 120

## Executive assessment

The phase is thoughtfully decomposed and closely aligned with PKG-01–03, especially around media-type lookup, binary-mode typing, strict slot classification, and real-fixture validation. However, three source-verified issues should be corrected before execution:

1. Plan 120-03 adds a required public `ConfigSlot` field but only budgets edits inside `pmcp-package`; at least 44 struct literals exist across the repository, so the root quality gate will fail.
2. Plan 120-01 updates four dependency pins but misses the generated-agent template and its test, which still hard-code `pmcp-package = "0.1"`.
3. Plan 120-02’s manifest-permutation test is underspecified for a content-addressed OCI layout: changing manifest bytes requires writing a new manifest blob and replacing the digest-bearing index descriptor.

With those fixed, the overall design is strong. Current overall risk: **MEDIUM-HIGH**.

---

# Plan 120-01 — Config-only tracer and 0.2.0 break

## Summary

This is a strong tracer plan that attacks the correct architectural seam: `pack_server` currently requires bootstrap bytes and `unpack_server` assumes six positional layers. The proposed enum-based binary modes and media-type-keyed unpacking directly solve PKG-01 and PKG-02. The main weakness is incomplete downstream version-plumbing coverage.

## Strengths

- The plan correctly replaces the required bootstrap argument at [pack.rs:51](crates/pmcp-package/src/oci/pack.rs:51), where the current API accepts `bootstrap: &[u8]`, with a typed binary mode.

- Removing `ServerPackage.binary_ref` is consistent with the existing separation between raw binary layers and typed metadata. Today the same identity is carried through both [ServerEnvelope.binary_ref](crates/pmcp-package/src/oci/pack.rs:42) and [ServerPackage.binary_ref](crates/pmcp-package/src/package/server.rs:373), so D-08 removes real duplication.

- The plan correctly targets the positional assumptions at [unpack.rs:78](crates/pmcp-package/src/oci/unpack.rs:78). Optional bootstrap/config/spec layers cannot safely coexist with `.first()`/`.get(n)` access.

- Digest verification remains on the established chokepoint at [unpack.rs:33](crates/pmcp-package/src/oci/unpack.rs:33), preserving verify-before-deserialize.

- `UnpackedServer` is a reasonable amendment to D-06. A two-tuple cannot return the restored config/spec without introducing another API path.

- Moving all four actual path-dependency requirements is necessary. They are present at:

  - [cargo-pmcp/Cargo.toml:86](cargo-pmcp/Cargo.toml:86)
  - [pmcp-agent/Cargo.toml:18](crates/pmcp-agent/Cargo.toml:18)
  - [pmcp-team-servers/Cargo.toml:24](crates/pmcp-team-servers/Cargo.toml:24)
  - [pmcp-cfn-renderer/Cargo.toml:10](crates/pmcp-cfn-renderer/Cargo.toml:10)

## Concerns

- **HIGH — Generated project templates remain pinned to 0.1.**  
  The plan’s “four root-workspace version requirements” is not the complete in-repo version-plumbing surface. The agent scaffold emits `pmcp-package = "0.1"` at [templates/agent.rs:73](cargo-pmcp/src/templates/agent.rs:73), and its test explicitly expects that string at [templates/agent.rs:341](cargo-pmcp/src/templates/agent.rs:341). After publishing 0.2.0, newly scaffolded projects will still request the incompatible 0.1 line. A root test may also fail if the template is changed later without updating its assertion.

- **MEDIUM — The blocking decision checkpoint duplicates locked context.**  
  D-08 through D-10 already lock the 0.2.0 break. Stopping execution to reconfirm it adds process friction unless the intent is specifically to override the earlier decision about Phase 124 ownership.

- **MEDIUM — Introducing `ConfigSlotViolation` before any code can produce it expands the public error surface prematurely.**  
  This is harmless mechanically, but it couples the tracer to Plan 120-05. If Plan 05 changes validation design, the public 0.2.0 error API will already be minted.

- **LOW — “Referenced digest is non-empty” is redundant for a valid `ManifestDigest`.**  
  If `ManifestDigest` parsing enforces the canonical digest shape, an “empty digest” cannot exist in the API enum. The meaningful null/missing check applies only when decoding the tolerant `BinaryRef` wire payload.

## Suggestions

- Add `cargo-pmcp/src/templates/agent.rs` to `files_modified` and change both the emitted dependency and its test from `"0.1"` to `"0.2"`.

- Add a repository-wide verification such as:

  ```bash
  rg -n 'pmcp-package\s*=\s*["{].*0\.1' --glob '*.toml' --glob '*.rs'
  ```

  Classify any intentional historical fixtures explicitly.

- Remove or reframe the checkpoint as a non-blocking execution note because the wire break is already a locked user decision.

- Consider delaying `ConfigSlotViolation` until Plan 120-05 unless keeping all public error changes in the 0.2.0 tracer is an explicit wire-freeze policy.

## Risk assessment

**MEDIUM.** The core refactor is well targeted, but the missed template pin would ship stale scaffolding and contradict the “all in-repo consumers moved” claim.

---

# Plan 120-02 — Spec layer and unpack hardening

## Summary

The plan covers the correct follow-on work: optional spec carriage, explicit legacy refusal, exactly-one binary enforcement, duplicate-media-type rejection, and CLI kind coverage. Its main technical gap is that the proposed layer-permutation property test does not fully describe how to mutate a content-addressed OCI manifest while keeping `index.json` valid.

## Strengths

- The optional spec behavior matches the runtime’s existing optional `--spec` surface and preserves curated-only servers.

- The plan correctly requires the raw spec bytes to use `write_blob`, not canonical JSON serialization. This is consistent with [OciLayout::write_blob](crates/pmcp-package/src/oci/layout.rs:76), which already content-addresses arbitrary bytes.

- Raw-envelope legacy detection is necessary. `ServerEnvelope` currently derives plain `Deserialize` without `deny_unknown_fields` at [pack.rs:41](crates/pmcp-package/src/oci/pack.rs:41), so merely removing `binary_ref` would silently accept and discard the old key.

- Duplicate media types must be rejected. Replacing positional access with a map otherwise creates a last-wins/first-wins shadowing risk.

- The plan correctly recognizes that `detect_kind` should understand the new media types. Its current server arm recognizes only the artifact type and envelope at [kind.rs:49](cargo-pmcp/src/commands/package/kind.rs:49).

- The plan includes both property and fuzz coverage, satisfying the repository’s feature-testing requirements.

## Concerns

- **HIGH — The permutation property test is incomplete for content-addressed manifests.**  
  The plan says to “rewrite the manifest blob with the permuted order.” That cannot be done in place while preserving integrity: `write_blob` derives the blob path and descriptor digest from its bytes at [layout.rs:76](crates/pmcp-package/src/oci/layout.rs:76), while unpack verifies the index descriptor’s declared digest before parsing at [unpack.rs:42](crates/pmcp-package/src/oci/unpack.rs:42). The test must:

  1. Serialize/canonicalize the permuted manifest.
  2. Write it as a new manifest blob.
  3. Preserve/reapply the old index descriptor annotations.
  4. Replace the index’s manifest descriptor with the new descriptor.
  5. Write the updated index.

  Without this, the test will either read the original manifest or fail on digest mismatch before exercising media-type lookup.

- **MEDIUM — The stated `artifact_type_from_manifest_json` fallback mechanism is inaccurate.**  
  That function returns `config.mediaType` before examining layers at [kind.rs:69](cargo-pmcp/src/commands/package/kind.rs:69). Since PMCP packages use the empty OCI config media type, it does not reach `layers[0]` in that shape. Actual inspection is nevertheless safe because `inspect` separately appends every layer media type at [inspect.rs:83](cargo-pmcp/src/commands/package/inspect.rs:83). The plan should cite the real mechanism rather than claim the third-choice fallback identifies config-only packages.

- **MEDIUM — The fuzz-target extension may add little coverage.**  
  `artifact_type_from_manifest_json` already receives arbitrary bytes, and the source test already has a never-panic property at [kind.rs:173](cargo-pmcp/src/commands/package/kind.rs:173). Adding constants to a “seed/alphabet” is only useful if the actual fuzz target synthesizes structured manifests; the plan should confirm that source structure before prescribing the edit.

- **LOW — “0.1.x envelope” detection is heuristic rather than an actual version marker.**  
  Any future/custom envelope containing a `binary_ref` extension key will be labeled 0.1.x. This is acceptable under the locked break but should be documented as shape detection, not authoritative producer-version detection.

## Suggestions

- Spell out a `rewrite_manifest_layers` test helper that writes a new manifest descriptor and atomically replaces the sole index descriptor.

- Correct the CLI rationale:

  - `artifactType` is authoritative.
  - `inspect` independently collects all manifest layer types.
  - Expanding `detect_kind` protects malformed/legacy manifests that lack `artifactType`.

- Add a direct CLI unit test with no `artifactType`, empty-config media type, and a new server layer, exercising the same candidate aggregation used by `inspect`, not merely `detect_kind` in isolation.

- Confirm whether the fuzz target constructs JSON or only forwards arbitrary bytes; remove the proposed seed edit if it does not materially increase reachability.

## Risk assessment

**MEDIUM.** The production design is sound, but the most important D-11 property test may be incorrectly implemented unless the OCI index/digest rewrite is made explicit.

---

# Plan 120-03 — Slot vocabulary and `required_slots`

## Summary

The new slot variants and separate `required_slots` function are well designed. Replacing the wildcard in `tested_value()` is especially valuable. The major problem is source compatibility: adding a required `config_key` field to `ConfigSlot` breaks every Rust struct literal, not just literals within `pmcp-package`.

## Strengths

- The plan correctly identifies the silent-classification hazard at [slot/types.rs:93](crates/pmcp-package/src/slot/types.rs:93): the current `_ => None` at line 97 would silently classify new variants as identity-bearing.

- `Endpoint` and `AuthMode` fit the established behavior-relevant shape used by `LlmProvider` and `BudgetOverride` at [slot/types.rs:57](crates/pmcp-package/src/slot/types.rs:57).

- Keeping `detect_deviation` unchanged is correct. Its semantic role is comparison, not required-input enumeration.

- `config_key` solves a genuine missing mapping: `Secret.name` identifies an environment binding, while placeholder validation needs a dotted config path.

- `#[serde(default, skip_serializing_if = "Option::is_none")]` appropriately preserves existing serialized fixtures when the field is absent.

- Deterministic `required_slots` ordering based on `SlotType::key()` aligns its identity semantics with `aggregate`.

## Concerns

- **HIGH — Adding `config_key` breaks at least 44 Rust construction sites, but the plan only edits the standalone crate.**  
  `ConfigSlot` currently has one required field at [slot/types.rs:105](crates/pmcp-package/src/slot/types.rs:105). Adding another public field requires every literal to specify it. Repository search finds literals in:

  - `pmcp-agent`
  - `pmcp-team-servers`
  - `cargo-pmcp`
  - examples and integration tests
  - `pmcp-package` itself

  For example, [negative.rs:180](crates/pmcp-package/tests/negative.rs:180) and numerous downstream literals will fail compilation. The plan says to update every “in-crate” literal only, while its final phase gate is `make quality-gate`, which builds the root workspace.

- **HIGH — The plan characterizes `config_key` as strictly additive, but it is source-breaking.**  
  It is wire-compatible when absent, but adding a required public struct field is a Rust API break. That distinction must be explicit.

- **MEDIUM — `required_slots` sorting without dedup can return duplicate required inputs.**  
  The plan says not to dedup because `aggregate` owns conflict handling. That is defensible, but the function’s name implies an actionable required-input inventory. If callers can pass unaggregated package slots, duplicates may appear. Either require aggregate-normalized input or return a `Result` using `aggregate`.

- **MEDIUM — Test 5’s contrast statement is misleading.**  
  “Pairing the same three slots against itself names zero” is true for all equal behavior-relevant values too, not specifically because identity-bearing slots are excluded. It does not demonstrate why `required_slots` must be separate.

- **LOW — A second blocking checkpoint repeats locked D-02.**  
  The user has already selected two typed variants.

## Suggestions

- Expand `files_modified` and Task 2 to cover every repository construction site found by:

  ```bash
  rg -n 'ConfigSlot\s*\{' --glob '*.rs'
  ```

- Add a compile/build verification covering all affected consumers, not just `pmcp-package`:

  ```bash
  cargo check -p cargo-pmcp -p pmcp-agent -p pmcp-team-servers
  ```

- Explicitly label `config_key` as:

  - serde/wire additive for old serialized data;
  - Rust source-breaking for struct-literal consumers.

- Consider one of these API shapes to reduce future source breakage:

  - `ConfigSlot::new(slot).with_config_key(...)`
  - `#[non_exhaustive]` plus constructors, if downstream literal construction can be migrated
  - embedding placement metadata in a separate wrapper type

- Make `required_slots` either accept aggregated slots or document that duplicates are intentionally preserved. Add a duplicate-input test.

- Replace the weak contrast test with one showing that a `Secret` is returned by `required_slots` while no possible `detect_deviation(secret, …)` result can enumerate it.

## Risk assessment

**HIGH.** The semantic design is good, but the current file scope guarantees widespread compilation failures once `config_key` is introduced.

---

# Plan 120-04 — Toolkit config declarations and base URL expansion

## Summary

This plan correctly addresses the real runtime blockers: strict `ServerConfig` parsing, missing endpoint expansion, and brittle parity-test string surgery. It is a good example of scope expansion justified by evidence. The main issues are insufficient validation of the new declaration vocabulary and an overly coupled env-reference helper location.

## Strengths

- Adding a field rather than weakening strict parsing follows the existing `#[serde(deny_unknown_fields)]` contract at [config.rs:100](crates/pmcp-server-toolkit/src/config.rs:100).

- The plan correctly preserves the optional HTTP feature behavior. `backend` is feature-gated today at [config.rs:115](crates/pmcp-server-toolkit/src/config.rs:115), while `config_slots` should be backend-neutral.

- The auth-mode exemption is source-verified: `AuthConfig` is internally tagged at [http/auth.rs:58](crates/pmcp-server-toolkit/src/http/auth.rs:58), so `"${AUTH_MODE}"` cannot select a serde enum variant.

- Both actual consumption sites are identified at [dispatch.rs:119](crates/pmcp-openapi-server/src/dispatch.rs:119) and [dispatch.rs:125](crates/pmcp-openapi-server/src/dispatch.rs:125). Resolving once before constructing both executors prevents inconsistent behavior.

- The error-redaction requirement is appropriate because the endpoint may carry sensitive tenant or routing information.

- Updating both fixture copies and deleting literal string replacement reduces test brittleness.

## Concerns

- **MEDIUM — `ConfigSlotDecl.kind` accepts arbitrary strings with no validation.**  
  `ConfigSlotDecl` is strict about unknown fields but not invalid values. A typo such as `kind = "endpont"` parses successfully, survives `ServerConfig::validate`, and cannot map cleanly to `pmcp_package::SlotType`. This undermines the purpose of strict declarations.

- **MEDIUM — The plan places a cross-cutting env-reference parser in a feature-gated HTTP module.**  
  `parse_env_ref` currently lives in `crate::http::auth` at [http/auth.rs:501](crates/pmcp-server-toolkit/src/http/auth.rs:501), and the entire `http` module is feature-gated at [lib.rs:43](crates/pmcp-server-toolkit/src/lib.rs:43). `BackendSection` is also HTTP-gated, so the immediate code can compile, but calling this the toolkit’s universal chokepoint is architecturally misleading. The source comment itself already claims credential consolidation, not general config expansion.

- **MEDIUM — Environment mutation cleanup is not specified.**  
  The new tests set and unset `TFL_BASE_URL` under a mutex, but should restore the prior value even after assertions. Otherwise later tests in the same binary can inherit modified state.

- **MEDIUM — Mapping `ConfigSlotDecl` into `pmcp_package::ConfigSlot` remains unimplemented.**  
  The key link says this belongs to “the packing caller,” but Phase 120 has no CLI packing caller yet. Plan 120-05 manually reconstructs the three slots in tests instead of proving the declaration block is actually parsed and mapped. This leaves D-01’s “pack reads them” only partially realized.

- **LOW — The new base URL error may not belong in `ConfigValidationError`.**  
  `ConfigValidationError` represents parse-time semantic validation at [error.rs:91](crates/pmcp-server-toolkit/src/error.rs:91), whereas environment lookup happens at dispatch time. A direct `ToolkitError` variant is cleaner.

## Suggestions

- Validate `ConfigSlotDecl.kind` against `endpoint | secret | auth_mode`, preferably with a toolkit-local enum that does not depend on `pmcp-package`. This preserves layering while preventing typos.

- Move the env-reference parser to a backend-neutral private module such as `config/env_ref.rs`, then reuse it from HTTP auth, code mode, and base URL resolution. That would make the “single chokepoint” statement accurate.

- Add an RAII environment guard in test support that restores the old value on drop.

- Define and test an explicit conversion seam—probably in the future packing caller—from `ConfigSlotDecl` to `pmcp_package::ConfigSlot`. If Phase 123 owns it, state that Phase 120 tests only schema acceptance and package-side enforcement, not end-to-end extraction.

- Put unresolved base URL lookup failures directly in `ToolkitError`, not `ConfigValidationError`.

## Risk assessment

**MEDIUM.** The runtime behavior is well planned, but invalid slot kinds and the missing declaration-to-package conversion leave part of the declared contract unenforced.

---

# Plan 120-05 — Placeholder validation and packed-manifest golden

## Summary

This plan closes the phase’s most important integrity and information-disclosure properties. The packed-manifest golden is correctly distinguished from the existing struct-level digests, and the real-fixture drift guard is valuable. The most significant concerns are mismatch between declaration parsing and manually constructed package slots, ambiguous treatment of non-config slot variants, and an incomplete definition of “nothing written” after failed validation.

## Strengths

- Validation is deliberately placed before the first `write_blob`, which is the correct point for preventing leaked config bytes from entering the content-addressed store.

- Promoting `toml` to a runtime dependency is justified by the locked requirement that `pack` itself validate placeholder-bearing paths. The plan carefully limits parsing to path lookup and does not reserialize the file.

- Errors are designed to name only the key, never the rejected value. That directly mitigates secret disclosure.

- D-17’s structural auth-mode carve-out is explicit and tested.

- The real fixture copy is necessary because the OpenAPI server excludes its test fixtures from its published package, and the drift guard prevents the copy from becoming a stale surrogate.

- The packed-manifest digest test correctly targets `finalize_pack` output rather than `manifest_digest(&struct)`. That is the only test shape that sees layer set, order, media types, and descriptor annotations.

- The stale-digest proof tests both identity movement and `verify` behavior.

## Concerns

- **HIGH — The plan still does not prove `[[config_slots]]` is read from the TOML.**  
  Plan 120-04 parses declarations into toolkit-local `ConfigSlotDecl`, but Plan 120-05 manually builds a `ServerPackage` with matching slots. The package validator receives `package.config_slots`; it does not compare them with the actual `[[config_slots]]` entries in the supplied TOML. Therefore:

  - a TOML declaration can be changed or removed while the manually built package slot list stays unchanged;
  - a caller can supply package slots that do not match the declaration block;
  - D-01’s “pack reads them” is not achieved by the actual API path.

- **HIGH — The validator’s “all variants except AuthMode are value slots” rule is too broad.**  
  Existing slot variants include `HumanRole`, `ChannelBinding`, `LlmProvider`, and `BudgetOverride` at [slot/types.rs:38](crates/pmcp-package/src/slot/types.rs:38). Some are not naturally TOML string placeholders, and `HumanRole` does not even identify a simple value field. The phase only requires endpoint and credentials to be placeholder-validated. Applying the rule to every future `ConfigSlot` with a `config_key` risks rejecting unrelated package kinds.

- **MEDIUM — “Failed validation leaves no layout behind” is not precisely testable as written.**  
  `OciLayout::create` immediately writes `oci-layout`, an empty `index.json`, and the blob directory at [layout.rs:40](crates/pmcp-package/src/oci/layout.rs:40). The plan’s wording should say:

  - index remains present with zero manifest entries;
  - no new layer/manifest blobs are written after layout creation.

  Testing only “no index entry” would not detect leaked config bytes in `blobs/sha256`.

- **MEDIUM — Dotted TOML traversal does not define escaped/literal-dot keys.**  
  Splitting blindly on `.` cannot address a TOML key whose literal name contains a dot. This may be acceptable for Phase 120, but the path grammar should be explicit and validated when declarations are parsed.

- **MEDIUM — `env:`/`${...}` validation may drift from runtime behavior.**  
  The plan copies the grammar instead of sharing an implementation with the toolkit. That creates exactly the divergent-parser risk Plan 120-04 warns about.

- **LOW — Task 1 says “write the eight tests first” but lists nine behaviors.**

- **LOW — Skipping validation when `config_key` is `None` weakens D-04.**  
  For a value slot, `None` means pack cannot verify that a secret or endpoint is placeholder-backed. Silently skipping may be correct for legacy non-config packages, but it should be conditional on slot/package kind rather than unconditional.

## Suggestions

- Add a package-side parser that extracts the `[[config_slots]]` table from the same TOML bytes and compares it with `package.config_slots`, or change `pack_server` to derive its slots from the config bytes. At minimum verify exact agreement on:

  - `key`
  - `kind`
  - `name`
  - `tested_value`

- Narrow placeholder enforcement explicitly:

  - `Endpoint` → required placeholder
  - credential variants intended for config values, initially `Secret` → required placeholder
  - `AuthMode` → structural exemption
  - all unrelated slot variants → either unsupported with `config_key` or handled by a documented rule

- Strengthen the failed-pack test by snapshotting the blob directory before and after and asserting no new blob files, not merely an empty index.

- Define the supported `config_key` grammar and reject empty components, leading/trailing dots, and array traversal with clear errors.

- Share env-reference recognition with the toolkit through a small common helper or duplicate it only with cross-crate parity tests over a table of accepted/rejected strings.

- Decide whether a value slot with `config_key: None` is:

  - a legacy slot exempt from config validation, or
  - an invalid packable config-server slot.

  Encode that distinction explicitly.

## Risk assessment

**MEDIUM-HIGH.** The security intent and digest tests are excellent, but the manual reconstruction of slots means the real config declaration is not the source of truth, leaving the central D-01/D-04 linkage incomplete.

---

# Cross-plan dependency and completeness review

## Strengths

- The wave order is broadly correct:

  - 120-01 establishes the breaking API.
  - 120-02 finishes and hardens OCI layer behavior.
  - 120-04 independently makes the real config bootable.
  - 120-03 adds slot vocabulary.
  - 120-05 joins both branches for enforcement and goldens.

- Sequencing 120-03 after 120-02 avoids concurrent edits and crate-wide quality-gate interference in the workspace-excluded package crate.

- The plans consistently preserve `detect_deviation` semantics and defer behavioral tool-list parity to Phase 121.

## Cross-plan concerns

- **HIGH — Plan 120-03’s source break is not represented in its dependency/file graph.**  
  Downstream source updates must occur in the same plan or in a dependent plan before any root build.

- **HIGH — The phase never establishes one authoritative slot-declaration flow.**  
  Toolkit TOML declarations and package `ConfigSlot`s are parallel representations with no enforced mapping.

- **MEDIUM — Version plumbing is split inconsistently between Phase 120 and Phase 124.**  
  Moving dependency requirements is necessary now for resolution, but templates and all tripwires must move with them. Phase 124 should retain only release/publish ledger work.

- **MEDIUM — Several tests rely on fragile grep counts.**  
  Grep checks are useful secondary tripwires, but semantic Rust tests should be the acceptance authority. Counts such as “at least eight occurrences” can pass because of comments or imports.

- **MEDIUM — The plans are unusually large for atomic execution.**  
  Each contains extensive docs, property tests, fuzz work, downstream updates, and API changes. The structure is sound, but token/task estimates understate the debugging cost of a 0.2 wire break plus root-workspace source migration.

## Recommended corrections before execution

1. Amend 120-01 to update the generated-agent `pmcp-package` version and its test.
2. Amend 120-02 with an explicit content-addressed manifest permutation helper.
3. Amend 120-03 to migrate every repository `ConfigSlot` literal or introduce a constructor-based compatibility approach.
4. Amend 120-04/05 to define an enforceable conversion from TOML `ConfigSlotDecl` to package `ConfigSlot`.
5. Narrow placeholder validation to slot kinds with defined config-value semantics.
6. Make the final phase verification include:

   ```bash
   make pmcp-package-gate
   cargo test -p pmcp-server-toolkit --features http
   cargo test -p pmcp-openapi-server
   cargo test -p cargo-pmcp
   make quality-gate
   ```

# Overall risk assessment

**MEDIUM-HIGH before amendments; MEDIUM after amendments.**

The architecture is coherent and should achieve PKG-01 and PKG-02. PKG-03 is nearly covered, but currently has a source-of-truth gap: the real TOML declaration and the package slot list can diverge. The two concrete compilation/version-plumbing omissions are straightforward to fix, while the declaration-to-package mapping requires a deliberate design adjustment before implementation begins.

---

## Gemini Review

> [reviewed-without-source-citations] This reviewer declared source-grounded evidence but cited no file:line source evidence, so it reviewed the pasted plan text only — down-weight its verdict in the Consensus Summary.

# Cross-AI Plan Review: Phase 120 (Config-Server Packaging)

**Review Target:** Phase 120 Implementation Plans (`120-01-PLAN.md` through `120-05-PLAN.md`)  
**Phase Scope:** Config-Server Packaging (PKG-01, PKG-02, PKG-03) — Milestone v2.6 (AI-Package Portability)  
**Overall Verdict:** **APPROVED (EXCELLENT QUALITY / PRODUCTION-READY)**

---

## Executive Summary

The implementation plan suite for **Phase 120: Config-Server Packaging** is exceptionally thorough, rigorous, and technically sound. It translates the design decisions from `120-CONTEXT.md` and the empirical findings from `120-RESEARCH.md` into five well-sequenced, test-driven plans across 4 waves.

### Key Strengths of the Plan Suite
1. **Surgical Precision on Wire Freeze (0.2.0 Break):** Recognizes that removing `binary_ref` from `ServerPackage` is a breaking wire change and coordinates the 0.2.0 bump across all four root workspace crates (`cargo-pmcp`, `pmcp-agent`, `pmcp-team-servers`, `pmcp-cfn-renderer`) simultaneously in Plan 01.
2. **Type-Level Enforcement of Invariants (PKG-02):** Replaces optional pointers with `BinaryMode::Embedded(&[u8]) | BinaryMode::Referenced { digest, media_type }` and `UnpackedBinary::Referenced { digest, media_type }`, ensuring callers cannot mistakenly treat a referenced binary as containing local bytes.
3. **Pragmatic Seam Resolution (D-17 & D-18):** Correctly identifies that `ServerConfig`'s `deny_unknown_fields` and `AuthConfig`'s `#[serde(tag = "type")]` would make the proving fixture unparseable, and properly expands Phase 120 to include additive changes to `pmcp-server-toolkit` while exempting structural auth-mode keys from value placeholder checks.
4. **Defense in Depth & Security Posture:** Eliminates catch-all wildcards in `SlotType::tested_value()`, avoids echoing offending literal secrets in error messages, ensures layout validation runs before blob writes, and verifies all layer blobs before deserialization.
5. **Rigorous Validation & Property Testing:** Incorporates permutation property tests (proving layer order is non-load-bearing), drift-guard tests between crate fixtures, negative tamper tests, and packed manifest digest golden fixtures.

---

## 1. Requirements & Success Criteria Traceability

| Requirement / Success Criterion | Target Plans | Verification & Coverage Mechanism | Assessment |
|---|---|---|---|
| **PKG-01 (No bespoke binary packaging)** | `120-01`, `120-02`, `120-05` | Manifest carries `MT_SERVER_CONFIG` and `MT_SERVER_OPENAPI_SPEC` layers without `MT_SERVER_BOOTSTRAP`. `unpack_server` restores both files byte-identically under their original names via `ANNOTATION_TITLE`. | **COMPLETE** |
| **PKG-02 (Dual-mode binary representation)** | `120-01`, `120-02` | `BinaryMode` and `UnpackedBinary` enums enforce embedded vs referenced at the type level. Unpacking referenced returns digest without searching disk (D-07). Legacy 0.1.x envelope is rejected explicitly (D-10). | **COMPLETE** |
| **PKG-03 (Baked vs Slot split enforcement)** | `120-03`, `120-04`, `120-05` | 1-byte mutation of OpenAPI spec moves manifest digest and fails `verify`. Endpoint, credentials, and auth mode surface as `ConfigSlot`s that `classify` and `aggregate` handle, with no spec-derived slot. | **COMPLETE** |
| **Success Criterion 1 (London Tube config+spec roundtrip)** | `120-01`, `120-02`, `120-04`, `120-05` | `london-tube.toml` + `london-tube-api.yaml` pack and unpack byte-identically. | **COMPLETE** |
| **Success Criterion 2 (Dual-mode roundtrip without ambiguity)** | `120-01`, `120-02` | Exhaustive `match` on `UnpackedBinary`; zero byte access on `Referenced`. | **COMPLETE** |
| **Success Criterion 3 (Enforced baked/slot split + verify rejection)** | `120-03`, `120-04`, `120-05` | `one_byte_change_in_the_spec_moves_the_manifest_digest_and_verify_rejects_the_stale_one` test + slot classification assertions. | **COMPLETE** |
| **Success Criterion 4 (Packed manifest golden fixture)** | `120-05` | `EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST` pins `finalize_pack` digest in `tests/digest_stability.rs`. | **COMPLETE** |

---

## 2. Architecture & Technical Soundness Assessment

```
                                  +---------------------------------------+
                                  |         Plan 120-01 (Wave 1)          |
                                  | - BinaryMode / UnpackedBinary enum    |
                                  | - Media-type indexed unpack (D-11)    |
                                  | - Wire break: pmcp-package 0.2.0      |
                                  | - Downstream 4-crate pin alignment    |
                                  +-------------------+-------------------+
                                                      |
                         +----------------------------+----------------------------+
                         |                                                         |
                         v                                                         v
      +-------------------------------------+                   +-------------------------------------+
      |        Plan 120-02 (Wave 2)         |                   |        Plan 120-04 (Wave 2)         |
      | - Optional OpenAPI spec layer       |                   | - ServerConfig [[config_slots]]     |
      | - 0.1.x raw envelope refusal (D-10) |                   | - base_url ${VAR} expansion         |
      | - Layer permutation proptest        |                   | - london-tube.toml slots & env var  |
      | - detect_kind Server arm fallback   |                   | - De-brittle parity_replay.rs       |
      +------------------+------------------+                   +------------------+------------------+
                         |                                                         |
                         v                                                         |
      +-------------------------------------+                                      |
      |        Plan 120-03 (Wave 3)         |                                      |
      | - SlotType::Endpoint & AuthMode     |                                      |
      | - Kill tested_value() wildcard      |                                      |
      | - ConfigSlot.config_key field       |                                      |
      | - required_slots() standalone fn    |                                      |
      +------------------+------------------+                                      |
                         |                                                         |
                         +----------------------------+----------------------------+
                                                      |
                                                      v
                                  +---------------------------------------+
                                  |         Plan 120-05 (Wave 4)          |
                                  | - Pack-time placeholder validation    |
                                  | - D-17 AuthMode structural carve-out  |
                                  | - Vendor london-tube fixtures + drift |
                                  | - Baked-spec proof & golden digest    |
                                  +---------------------------------------+
```

### 1. Dual-Mode Binary Representation (`BinaryMode` & `UnpackedBinary`)
- **Type Safety:** Defining `BinaryMode::Referenced { digest: ManifestDigest, media_type: String }` with a **non-optional** `ManifestDigest` prevents packaging unpinned runtime binaries.
- **Envelope Cleanliness:** Dropping `binary_ref` from `ServerPackage` and `ServerEnvelope` ensures binary identity is governed strictly by layer presence (`MT_SERVER_BOOTSTRAP` vs `MT_SERVER_BINARY_REF`), honoring the OCI design principle that binary payloads are layers, not struct fields.

### 2. OCI Layer & Filename Annotations
- **Media-Type Indexing (`index_layers`):** Replaces brittle positional index lookups with `BTreeMap<String, &Descriptor>`. Adding a duplicate-layer check prevents malicious or accidental layer-shadowing vulnerabilities (STRIDE Tampering T-120-01).
- **Standards Compliance:** Using `oci_spec::image::ANNOTATION_TITLE` (`"org.opencontainers.image.title"`) on layer descriptors allows unpacking to restore `london-tube.toml` and `london-tube-api.yaml` byte-identically with zero custom annotation namespaces.
- **Digest Feeding:** Correctly recognizes that layer-descriptor annotations are part of the canonical `ImageManifest` and therefore alter the manifest digest, whereas index-descriptor annotations do not.

### 3. Slot Typing & Derivation
- **Variant Classification:** `SlotType::Endpoint` and `SlotType::AuthMode` carry `tested_value: String`. Because `classify()` checks `slot.tested_value().is_some()`, both automatically classify as `BehaviorRelevant` without any manual match branching.
- **Exhaustive Pattern Matching:** Replacing the wildcard `_ => None` in `tested_value()` with explicit variant arms is a crucial fix that prevents future slot types from silently falling back to `IdentityBearing`.
- **Preserving Invariants:** Retaining `detect_deviation` for pairwise behavioral comparison while providing `required_slots` for target-environment enumeration prevents breaking the documented contract that `detect_deviation` never flags identity-bearing slots.
- **Digest Stability:** Using `#[serde(default, skip_serializing_if = "Option::is_none")]` for `ConfigSlot.config_key` guarantees that existing golden fixtures (`server_team_fs_v1.json`, etc.) remain byte-identical.

### 4. Server Toolkit & Proving Case (`pmcp-server-toolkit` & `pmcp-openapi-server`)
- **Preserving Strictness:** Adding `#[serde(default)] pub config_slots: Vec<ConfigSlotDecl>` to `ServerConfig` while retaining `deny_unknown_fields` upholds the toolkit doctrine ("Always ADD the missing field; do NOT loosen deny_unknown_fields").
- **Error-on-Unset Semantics:** `BackendSection::resolved_base_url` uses error-on-unset semantics (unlike credential resolution which allows empty-on-unset for optional credentials). This prevents the server from booting and making requests to a broken, empty base URL.
- **D-17 Carve-out:** Correctly recognizes that `AuthConfig` is internally tagged (`#[serde(tag = "type")]`), which precludes `${AUTH_MODE}` from deserializing. Scoping the pack-time placeholder check to value slots (`Endpoint`, `Secret`) while exempting `AuthMode` solves this elegantly without requiring an invasive refactor of `AuthConfig`.

---

## 3. Plan-by-Plan Review & Observations

### Plan 120-01: Tracer (Config-Only Pack/Unpack + 0.2.0 Break)
- **Strengths:**
  - Includes a blocking decision checkpoint for the 0.2.0 wire break.
  - Changes `pack_server` and `unpack_server` in Task 1, then bumps `pmcp-package` to 0.2.0 and aligns all four downstream workspace dependencies (`cargo-pmcp`, `pmcp-agent`, `pmcp-team-servers`, `pmcp-cfn-renderer`) in Task 2.
  - Updates the `cargo-pmcp/tests/pmcp_package_pin.rs` tripwire.
- **Assessment:** Clear, focused, and leaves the codebase in a compilable state.

### Plan 120-02: Optional OpenAPI Spec Layer & Index Hardening
- **Strengths:**
  - `OpenApiSpecFile` handled as `Option<OpenApiSpecFile>` matching `pmcp-openapi-server`'s `--spec` CLI behavior.
  - `detect_legacy_shape` inspects raw envelope JSON for `"binary_ref"` before serde deserialization (necessary because `ServerEnvelope` lacks `deny_unknown_fields`).
  - Proptest asserting layer-permutation invariance validates that layer ordering is non-load-bearing.
  - Extends `detect_kind` fallback in `cargo-pmcp` for config-only packages.
- **Assessment:** Closes all potential layer-lookup edge cases and satisfies CLAUDE.md proptest/fuzz mandates.

### Plan 120-03: Slot Vocabulary & Enumeration
- **Strengths:**
  - Implements `SlotType::Endpoint` and `SlotType::AuthMode`.
  - Removes the `_ => None` wildcard in `tested_value()`.
  - Adds `ConfigSlot.config_key` with `skip_serializing_if` to protect existing digest goldens.
  - Adds `required_slots` in a dedicated module (`src/slot/required.rs`) without touching `detect_deviation.rs`.
- **Assessment:** Clean separation of concerns with full backwards compatibility for unchanged fixtures.

### Plan 120-04: Toolkit Config Slots & Base URL Expansion
- **Strengths:**
  - Adds `ConfigSlotDecl` to `ServerConfig` with strict field validation.
  - Reuses the existing `parse_env_ref` from `http/auth.rs` rather than duplicating `${VAR}` / `env:VAR` parsing.
  - Updates both `tests/fixtures/london-tube.toml` and `examples/london-tube.toml`.
  - Simplifies `parity_replay.rs` by replacing fragile string-replacement surgery (`temp_config_pointing_at`) with `std::env::set_var("TFL_BASE_URL", backend.uri())`.
- **Assessment:** High ROI — simplifies existing test harness while making the server natively slot-aware.

### Plan 120-05: Pack-Time Placeholder Validation & Manifest Golden
- **Strengths:**
  - Promotes `toml` from dev-dependencies to regular dependencies in `pmcp-package` (PR-04) to perform structural dotted-path lookup at pack time.
  - Implements D-17 structural carve-out for `AuthMode`.
  - Vendors `london-tube` fixtures under `tests/golden_fixtures/config_server_london_tube_v1/` with a drift-guard test asserting identity against `pmcp-openapi-server` fixtures.
  - Proves the baked-versus-slot split (1-byte spec mutation fails `verify`).
  - Pins `EXPECTED_CONFIG_SERVER_PACKED_MANIFEST_DIGEST` in `digest_stability.rs`.
- **Assessment:** Directly proves all three requirements (PKG-01, PKG-02, PKG-03) and all four Success Criteria.

---

## 4. Nuances, Edge Cases & Recommendations

While the plans are ready for execution, the implementing agent should keep the following operational nuances in mind:

### 1. PMAT Cognitive Complexity Limits
- **Observation:** `unpack_server` in `pmcp-package` has multiple failure branches and layer checks.
- **Guidance:** Ensure the plan's recommendation to extract `index_layers`, `detect_legacy_shape`, `read_binary_mode`, and `read_named_file_layer` as standalone helper functions is followed strictly from the start so `pmat analyze complexity --max-cognitive 25` passes without mid-task refactoring.

### 2. Error Message Redaction (STRIDE T-120-17 & T-120-21)
- **Observation:** When `validate_config_slot_placeholders` finds a resolved literal or when `resolved_base_url` fails to resolve, error strings must never format or echo the literal value.
- **Guidance:** Keep error messages strictly parameterized by key name (e.g., `format!("slot-declared value key '{key}' must hold an environment reference")`).

### 3. Drift Guard Execution in Isolated Package Builds
- **Observation:** In `120-05-PLAN.md Task 2`, the drift-guard test checks whether the sibling directory `../pmcp-openapi-server/tests/fixtures/` exists before comparing files.
- **Guidance:** The condition `if sibling_dir.exists()` is correctly specified so that running tests inside a standalone published crate tarball does not fail, while running within the monorepo strictly fails if either fixture file is modified without updating the copy.

---

## 5. Review Conclusion

The Phase 120 plan suite is **exceptionally well-designed, comprehensive, and ready for execution**.

- **Completeness:** 100% of requirements (PKG-01, PKG-02, PKG-03) and Success Criteria 1–4 are directly covered and verified.
- **Safety:** Wire break (0.2.0) is controlled, dependencies are verified, and security invariants ("secrets never travel") are enforced.
- **Feasibility:** Execution order across the 4 waves is sound and DAG-valid.

**Recommendation:** Proceed with Plan `120-01-PLAN.md` (Wave 1).

---

## Consensus Summary

Two reviewers ran, but their verdicts do not carry equal weight. **Codex reviewed against source** — every finding cites `file:line` evidence from the actual repo — and rates the phase **MEDIUM-HIGH risk before amendments**, with four HIGH-severity findings. **Gemini's output carries the `[reviewed-without-source-citations]` marker**: it declared source-grounded evidence but cited no `file:line`, so it reviewed the plan text only and its **APPROVED / production-ready** verdict is down-weighted per review policy. Gemini's review largely restates the plans' own claims back as strengths, which is exactly the failure mode that marker exists to flag. Plan-level consensus therefore rests on the Codex findings, with Gemini's structural observations noted as corroboration where the two overlap.

The load-bearing conclusion: **the phase architecture is sound and achieves PKG-01/PKG-02, but four concrete defects should be amended before execution** — three are mechanical (a missed template pin, a source-breaking struct field with under-scoped edits, an under-specified OCI permutation test) and one is a design gap (no enforced mapping from the toolkit's `[[config_slots]]` TOML declarations to the package's `ConfigSlot` list, leaving PKG-03 / D-01's "pack reads them" unproven by the actual API path).

### Agreed Strengths

Both reviewers, independently, endorse:

- **Typed dual-mode binary representation** — `BinaryMode::Embedded`/`Referenced` and `UnpackedBinary` enums enforce PKG-02 at the type level; callers cannot treat a referenced binary as local bytes (Codex verified the seam at `crates/pmcp-package/src/oci/pack.rs:51`; Gemini endorses the same shape).
- **Media-type-indexed unpacking with duplicate-media-type rejection** — replaces the brittle positional `.first()`/`.get(n)` access (Codex verified at `crates/pmcp-package/src/oci/unpack.rs:78`) and closes the layer-shadowing risk.
- **Killing the `_ => None` wildcard in `SlotType::tested_value()`** — both call out the silent-misclassification hazard for future variants (Codex verified at `crates/pmcp-package/src/slot/types.rs:93-97`).
- **`#[serde(default, skip_serializing_if)]` on `config_key`** — preserves existing golden fixtures and digest stability.
- **D-17 auth-mode structural carve-out** — both verified the rationale that `AuthConfig`'s internal `#[serde(tag = "type")]` makes `${AUTH_MODE}` unparseable (Codex at `crates/pmcp-server-toolkit/src/http/auth.rs:58`).
- **Validation before first blob write + key-only (redacted) error messages** — the secrets-never-travel posture in 120-05.
- **Wave ordering is DAG-valid** — 01 → {02 ∥ 04} → 03 → 05, with the pmcp-package crate-wide gate correctly serialized.

### Agreed Concerns

No concern was independently raised by both reviewers — Gemini raised no plan defects at all, which given its marker is a data point about the review, not the plans. The priority list is Codex's source-verified findings:

1. **HIGH (120-03): `ConfigSlot.config_key` as a required public field breaks ~44 struct literals repo-wide** (e.g. `crates/pmcp-package/tests/negative.rs:180`, plus `pmcp-agent`, `pmcp-team-servers`, `cargo-pmcp`, examples), but the plan scopes edits to `pmcp-package` only — the root `make quality-gate` will fail. The plan also mislabels the change "additive": serde-additive, yes; Rust-source-breaking, no.
2. **HIGH (120-01): the generated-agent template still pins `pmcp-package = "0.1"`** at `cargo-pmcp/src/templates/agent.rs:73` with a test asserting that literal at `:341` — after 0.2.0 publishes, fresh scaffolds request the incompatible line. This contradicts the plan's "all four in-repo consumers moved" claim.
3. **HIGH (120-02): the layer-permutation property test cannot be implemented as written** — a content-addressed OCI layout requires writing the permuted manifest as a *new* blob and replacing the index's digest-bearing descriptor (verified against `layout.rs:76` and `unpack.rs:42`); "rewrite the manifest blob in place" either reads the original or fails digest verification before reaching the code under test.
4. **HIGH (120-04/05, cross-plan): no authoritative slot-declaration flow** — 120-04 parses `[[config_slots]]` into toolkit-local `ConfigSlotDecl`, but 120-05's tests manually rebuild matching `ConfigSlot`s; nothing enforces agreement between the TOML declarations and the package slot list, so D-01's "pack reads them" is unproven and the two representations can silently diverge.

Notable MEDIUMs worth carrying into replanning: the 120-05 "failed pack leaves nothing behind" test must assert no new blobs, not just an empty index (`OciLayout::create` already writes `oci-layout`/`index.json`); `ConfigSlotDecl.kind` accepts arbitrary strings unvalidated; the placeholder validator's "everything except AuthMode is a value slot" rule sweeps in `HumanRole`/`ChannelBinding`/etc.; env-ref grammar is copied, not shared, risking runtime drift; and both blocking checkpoints re-confirm already-locked decisions (D-02, D-08–D-10).

### Divergent Views

- **Overall verdict** — Gemini: APPROVED, "production-ready, proceed with 120-01." Codex: MEDIUM-HIGH, "amend before execution." The divergence is fully explained by methodology: every one of Codex's HIGH findings lives in code Gemini never opened (template literals, repo-wide struct construction sites, OCI digest mechanics). Down-weighting Gemini per the marker, the consensus verdict is **amend first, then execute** — the amendments are well-bounded and Codex itself rates the phase MEDIUM after them.
- **Traceability** — Gemini scores PKG-01/02/03 all COMPLETE; Codex agrees on PKG-01/02 but shows PKG-03's declaration→package linkage is not exercised by the real API path. The PKG-03 COMPLETE rating should not be trusted.
- **120-04's `parse_env_ref` reuse** — Gemini lists it as a strength; Codex notes the helper lives in the feature-gated `http` module (`lib.rs:43`), making "the toolkit's universal chokepoint" architecturally misleading even if it compiles.
