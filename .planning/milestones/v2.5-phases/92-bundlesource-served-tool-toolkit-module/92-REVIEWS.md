---
phase: 92
reviewers: [codex, gemini]
reviewed_at: 2026-06-10T19:21:37Z
plans_reviewed: [92-01-PLAN.md, 92-02-PLAN.md, 92-03-PLAN.md, 92-04-PLAN.md, 92-05-PLAN.md]
---

# Cross-AI Plan Review — Phase 92

## Codex Review

## Summary

The plans are strong on intent, sequencing, and verification discipline, but I would not execute them unchanged. The biggest issues are dependency/order mistakes: `workbook` features are not added until Plan 05 even though Plans 03-04 verify with `--features workbook`; Plan 03 declares modules before files exist; and the provenance field repeatedly conflates source `workbook_hash` with the required `BUNDLE.lock.combined` hash. Fix those before execution. Overall risk is **MEDIUM-HIGH** because the architecture is sound, but several plan-level details can cause failed waves or a subtly wrong public contract.

## Strengths

- Clear fail-closed design: raw-byte `BundleSource` plus shared `BundleLoader` is the right WBSV-08 shape.
- Good separation between runtime bundle loading, toolkit served surface, render resource, and final builder/example wiring.
- Strong test posture: byte-stable fixture, tamper-at-test-time, proptest coverage, integration tests, and purity gate.
- Good explicit scrub discipline against lifting customer-specific lighthouse concepts.
- The `workbook` / `workbook-embedded` split in Plan 05 is a useful refinement; it keeps plain local-dir use lean.

## Concerns

### HIGH

- **Feature ordering is broken.** Plans 03 and 04 run `cargo test ... --features workbook`, but `pmcp-server-toolkit` does not gain `workbook` / runtime dependency wiring until Plan 05. Move the Cargo feature + `lib.rs` gated module declaration earlier, before Plan 03.

- **Plan 03 Task 1 will not compile as written.** It creates `workbook/mod.rs` with `pub mod input; pub mod handler;`, but `input.rs` and `handler.rs` are not created until later tasks. Either create stubs in Task 1 or declare modules only when the files exist.

- **Provenance naming is wrong and risky.** Several plans define `ProvStamp { bundle_id, version, workbook_hash }` while saying `workbook_hash` means the combined `BUNDLE.lock` hash. That conflicts with runtime `BundleLock.workbook_hash`, which is the source workbook hash. The stamp should use an unambiguous field like `combined_hash` or `bundle_hash`.

- **D-13 scrub scope is too narrow.** Current runtime files still contain customer-ish test strings and comments such as `ufh-quote`, `Plot-3`, and `first_fix` outside `artifact_model.rs`. If “zero customer identifiers survive in SDK code/comments/tests/docs” is literal, the grep gate must cover all touched runtime/toolkit fixture/doc paths, not just `artifact_model.rs` and `workbook/*.rs`.

- **Fixture versioning is inconsistent.** The fixture path is `tax-calc@1.0.0`, but the changelog requirement says a real `v1.0.0 → v1.1.0` pair. If `diff_version` means “previous to current,” the bundle version should likely be `1.1.0`, or the changelog contract must clearly define why a `1.0.0` bundle contains a future diff.

### MEDIUM

- **Runtime `base64` dependency appears unnecessary.** Plan 01 adds `base64` to `pmcp-workbook-runtime`, but the URI codec and resource bytes live in `pmcp-server-toolkit`. Keep `base64` optional in the toolkit feature unless runtime code directly uses it.

- **The include_dir rejection fallback conflicts with WBSV-09.** Plan 01 says LocalDirSource alone “minimally satisfies WBSV-09,” but WBSV-09 explicitly requires local-dir and embedded implementations. If `include_dir` is rejected, the plan needs an alternate embedded implementation, not silent scope reduction.

- **Deterministic fixture generation is under-specified.** Runtime IR/executor types use `HashMap` in places. `serde_json::to_string_pretty` alone does not guarantee stable ordering for maps. The generator must use sorted maps, sorted serialization, or explicit canonical JSON.

- **BundleLoader extra-artifact behavior is unclear.** The loader verifies expected hashes, but the plan does not say whether unknown files in the bundle directory are rejected, ignored, or included in evidence. For a frozen bundle contract, fail-closed on unexpected members is safer.

- **Resource URI privacy risk.** `workbook://` encodes inputs, and in real workbook servers those may be sensitive. If this is the public contract, docs should explicitly warn that URIs contain inputs and may be logged by clients/proxies, or define a future opaque-handle alternative.

- **Example verification only builds the example.** The requirement says working `cargo run --example`; Plan 05 should include a smoke run that boots the HTTP server and checks the advertised tools or at least starts cleanly with a bounded timeout.

### LOW

- `justfile-first` in Plan 05 does not match the current repo, where `just purity-check` delegates to `make purity-check`. That is fixable, but the plan should choose one primary implementation to avoid drift.
- Tool failure semantics should distinguish domain failures from infrastructure failures. “Never protocol Err” is right for domain failures, but malformed server state, resource handler failures, or internal bugs may still need protocol errors.
- `render_xlsx` per `resources/read` is intentionally stateless but can be expensive. The plan should at least document read-size/runtime limits or future rate limiting.

## Suggestions

- Move the following from Plan 05 to Plan 01 or a new early “wiring skeleton” wave: toolkit optional runtime dependency, `workbook` / `workbook-embedded` features, `#[cfg(feature = "workbook")] pub mod workbook`, and minimal re-export stubs.

- Rename provenance fields everywhere before implementation:
  - `source_workbook_hash` for the original workbook content hash, if exposed at all.
  - `combined_hash` or `bundle_lock_hash` for `BUNDLE.lock.combined`.
  - Avoid `workbook_hash` in `ProvStamp`.

- Add a contract test that asserts the response provenance hash equals `BundleLock.combined`, not `BundleLock.workbook_hash`.

- Extend scrub gates to all touched files:
  - `crates/pmcp-workbook-runtime/src/`
  - `crates/pmcp-server-toolkit/src/workbook/`
  - `crates/pmcp-server-toolkit/tests/`
  - `crates/pmcp-server-toolkit/examples/workbook_server_http.rs`
  - `docs/workbook-uri-spec.md`

- Make fixture JSON canonical by construction: use `BTreeMap`/sorted vectors in generated artifacts, or write a canonical serializer for any map-valued artifact.

- Define exact BundleLoader membership policy: required members, allowed optional members, and whether unknown members fail closed.

- Add one end-to-end handler test that calls all five tools against the tax bundle, not only builder registration.

- Add an example smoke check: start `workbook_server_http` on an ephemeral port, fetch or initialize, assert tool list includes all five, then shut it down.

## Risk Assessment

**Overall risk: MEDIUM-HIGH.** The architecture is directionally correct and well-tested on paper, but execution risk is elevated by compile-order mistakes, ambiguous provenance naming, deterministic fixture hazards, and public-contract details around URI contents. Once the feature wiring is moved earlier, provenance is renamed, and fixture canonicalization is made explicit, the risk drops to **MEDIUM**.

---

## Gemini Review

# Plan Review: Phase 92 - BundleSource + Served-Tool Toolkit Module

## Summary
Phase 92 is an exceptionally well-structured module-for-module extraction of the workbook served layer from the lighthouse project into the PMCP SDK. The plan correctly prioritizes freezing the consumer-side bundle contract (Runtime and Toolkit) before the compiler is re-cut in Phase 93. It solves the "chicken-and-egg" dependency of having no compiler yet by generating a synthetic, byte-stable golden fixture (Plan 02) that acts as the test oracle. The security posture is robust, emphasizing fail-closed integrity checks at boot and strict re-validation of untrusted payloads on resource reads.

## Strengths
*   **Dumb Byte-Accessor Design:** The decision to make `BundleSource` a raw-byte provider and centralize parsing/integrity in a shared `BundleLoader` (Plan 01) is a critical security win, ensuring no implementation can bypass the WBSV-08 boot gate.
*   **Byte-Stability Check:** The CI mechanism for byte-identical regeneration of the tax-calc golden (Plan 02) is a sophisticated way to ensure the producer (Phase 93) and consumer (Phase 92) remain in sync without manual toil.
*   **Purity Boundary Defense:** The plan recognizes that `pmcp-server-toolkit` is not unconditionally reader-free and correctly adds *feature-specific* purity assertions to the `Makefile` (Plan 05), preventing reader leakage via feature unification.
*   **Consistent SDK Idioms:** The registration API (`WorkbookBuilderExt`) and the use of `structuredContent` for domain errors (`to_iserror_result`) perfectly mirror the established v2.2 SQL and OpenAPI toolkit patterns.
*   **Scrub Discipline:** The enumerated scrub deltas (S-1 through S-4) are surgical and grep-verifiable, mitigating the risk of leaking lighthouse customer data or logic into the SDK.

## Concerns
*   **MAX\_ENCODED\_URI\_LEN Sensitivity (LOW):** While the size guard is present (Plan 04), if the tax-calc fixture inputs are very large, the encoded URI could hit limits in certain transport or client environments. However, the use of `proptest` for codec totality largely mitigates the risk of overflow-related panics.
*   **Include\_dir Legitimacy (LOW):** This is marked as `[ASSUMED]`, but the plan correctly gates it behind a blocking human-verify checkpoint (Plan 01, Task 1) and a mandatory `cargo audit`.
*   **Feature Split Complexity (LOW):** Splitting the feature into `workbook` and `workbook-embedded` (Plan 05) adds a small amount of maintenance overhead but is necessary to keep the leaf dependency tree lean for users who don't need binary embedding.

## Suggestions
*   **Standardized Error Codes:** In `error.rs` (Plan 03), ensure the `WorkbookToolError` codes are documented in a way that matches the "Self-Repair" narrative for widgets, as these will be the primary machine-readable signals for the MCP App UI.
*   **Example Documentation:** In the streamable-HTTP example (Plan 05), consider adding a comment explaining how a user would transition from the `EmbeddedSource` to a `LocalDirSource` for production environments where workbooks are updated out-of-band.
*   **Corpus Seeds:** For the `prop_validate_input_total` test (Plan 03), ensure the initial seeds include common Excel edge cases (empty strings vs nulls) to catch coercion issues early.

## Risk Assessment: LOW
The overall risk is low because the implementation reuses heavily validated code from a lighthouse project (~730 tests). The most significant technical risk—purity erosion—is mitigated by a three-layer gate. The dependency risk is handled by the synthetic fixture strategy.

*   **Logic Risk:** Low (Lift of proven code).
*   **Security Risk:** Low (Deep defense with re-validation and integrity checks).
*   **Schedule Risk:** Low (Plans are modular and strictly sequential).

The plans are ready for execution.

---

## Consensus Summary

The two reviewers split sharply on overall risk: Codex rates the plans
MEDIUM-HIGH with five HIGH-severity execution-order/contract concerns, while
Gemini rates them LOW and "ready for execution." Gemini reviewed at the
architecture level (where both agree the design is sound); Codex went deeper
into task-level mechanics (compile order, field naming, gate scope) and found
concrete defects there. The Codex HIGH items are specific and checkable —
they should be treated as the priority feedback for `--reviews` replanning.

### Agreed Strengths
- Dumb byte-accessor `BundleSource` + shared `BundleLoader` makes the WBSV-08
  fail-closed boot gate structurally unbypassable (both reviewers call this
  out as the key security win).
- Byte-stable synthetic golden fixture + tamper-at-test-time is the right
  answer to the Phase-93 chicken-and-egg, and keeps producer/consumer in sync.
- Scrub deltas S-1..S-4 are surgical and grep-verifiable; the
  `workbook`/`workbook-embedded` feature split keeps local-dir consumers lean.
- Purity-gate extension correctly treats the toolkit as NOT unconditionally
  reader-free (feature-specific assertion, not a PURITY_CRATES append).
- Strong test posture overall: proptest fuzz, integration tests, byte
  stability, boot-tamper negatives.

### Agreed Concerns
(Only one concern appears in both reviews, at different severities)
- **include_dir is an unvetted new dependency** — both note it; the plans'
  blocking human-verify checkpoint + cargo audit is accepted as the
  mitigation (Gemini LOW; Codex raises the rejected-fallback contradiction
  with WBSV-09: if include_dir is rejected, an alternate embedded impl is
  required, not silent scope reduction).

### Divergent Views / Codex-only HIGH items (verify during replanning)
1. **Feature ordering broken:** Plans 03/04 verify with
   `--features workbook` but the toolkit only gains the feature + runtime dep
   in Plan 05 — tests cannot compile as sequenced. Fix: move Cargo feature +
   gated `pub mod workbook` skeleton to Plan 01 or a new early wiring task.
2. **Plan 03 Task 1 compile order:** `workbook/mod.rs` declares
   `pub mod input; pub mod handler;` before those files exist in later tasks.
3. **Provenance hash naming conflation:** `ProvStamp.workbook_hash` is
   described as the BUNDLE.lock combined hash, but runtime
   `BundleLock.workbook_hash` is the source-workbook hash. Rename stamp field
   to `combined_hash`/`bundle_lock_hash` + add a contract test that the stamp
   equals `BundleLock.combined`.
4. **Scrub gate scope too narrow:** customer-ish strings (`ufh-quote`,
   `Plot-3`, `first_fix`) exist in runtime files beyond `artifact_model.rs`;
   the D-13 grep gate must cover all touched runtime/toolkit/test/example/doc
   paths.
5. **Fixture version vs changelog semantics:** fixture is `tax-calc@1.0.0`
   but must carry a v1.0.0→v1.1.0 changelog pair — either the bundle should
   be `@1.1.0` or the contract must define why a 1.0.0 bundle records a diff.
6. MEDIUM (Codex): runtime `base64` dep likely belongs in the toolkit;
   HashMap ordering can break byte-stable fixture generation (use
   BTreeMap/canonical serialization); BundleLoader policy for unknown bundle
   members is unspecified (fail-closed recommended); `workbook://` URIs
   encode inputs — document the privacy/logging implication; example should
   get a boot smoke-run, not just a build.
7. Gemini-only suggestions: document error codes for widget self-repair
   narratives; example comment on EmbeddedSource→LocalDirSource transition;
   proptest seeds for Excel edge cases (empty string vs null).
