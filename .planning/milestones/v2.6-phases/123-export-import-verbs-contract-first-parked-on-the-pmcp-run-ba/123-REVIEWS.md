---
phase: 123
reviewers: [codex, gemini]
reviewed_at: 2026-08-26T17:31:24Z
plans_reviewed:
  - 123-01-PLAN.md
  - 123-02-PLAN.md
  - 123-03-PLAN.md
  - 123-04-PLAN.md
  - 123-05-PLAN.md
  - 123-06-PLAN.md
  - 123-07-PLAN.md
models:
  codex: "gpt-5.6-sol (reasoning=low)"
  gemini: "unknown"
model_sources:
  codex: "banner"
  gemini: "unknown"
---

# Cross-AI Plan Review — Phase 123

## Codex Review

## Summary

The plan set is unusually thorough and aligns well with the settled `save` / `load` / `pull` scope. The existing resolver, GraphQL, package-kind, unpacking, slot, and attestation mechanisms mostly have the shapes the plans assume. However, several cross-plan design gaps make the current plans non-executable as written: the proposed in-memory artifact representation lacks media types required to reconstruct a layout; integration tests cannot inject the proposed `pull` transport seam; the renderer is not mounted into the library despite library-test requirements; and semantic package validation occurs after destination writes. Overall risk is **HIGH until these architectural contradictions are corrected**.

## Strengths

- The verb inventory is correctly based on current source. This branch currently exposes exactly `inspect`, `capture`, `show`, `import`, and `approve`, and `import` has the shipped dry-run meaning the plans preserve ([cargo-pmcp/src/commands/package/mod.rs:31](cargo-pmcp/src/commands/package/mod.rs:31)).

- Reusing the existing environment-resolution path is sound. `get_api_base_url()` already implements `PMCP_API_URL` → `PMCP_RUN_API_URL` → configured target → default precedence ([auth.rs:113](cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:113)); configured resolution calls `resolve_active_target_name` ([auth.rs:149](cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:149)); and the cache is endpoint-keyed and TTL-checked ([auth.rs:201](cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:201)).

- The plans correctly preserve the integrity-versus-claim distinction. `unpack_server` performs digest/config/layer validation ([unpack.rs:639](crates/pmcp-package/src/oci/unpack.rs:639)), while `inspect` renders an attestation and then rejects subject mismatch outside the quiet-output gate ([inspect.rs:180](cargo-pmcp/src/commands/package/inspect.rs:180), [inspect.rs:217](cargo-pmcp/src/commands/package/inspect.rs:217)).

- The slot-reporting design uses the right source. `required_slots()` preserves duplicates and sorts stably, while explicitly distinguishing environment-variable names from dotted config paths ([required.rs:70](crates/pmcp-package/src/slot/required.rs:70), [required.rs:100](crates/pmcp-package/src/slot/required.rs:100)).

- The three-state `resolved_from` treatment is source-aligned. The type explicitly requires consumers to treat `None` as “cannot report,” never “no skew” ([reference.rs:81](crates/pmcp-package/src/reference.rs:81)).

- The plans correctly identify the Makefile reach gap. `test-cargo-pmcp` runs only `--lib` ([Makefile:283](Makefile:283)), and the integration target currently selects only four named binaries ([Makefile:393](Makefile:393)). `verb_help` is therefore genuinely ungated today.

- The framing design’s refusal to use archive-supplied paths is strong. `OciLayout::write_blob` derives the path from a digest computed over the bytes being written ([layout.rs:95](crates/pmcp-package/src/oci/layout.rs:95), [layout.rs:108](crates/pmcp-package/src/oci/layout.rs:108)).

## Concerns

- **HIGH — `VerifiedArtifact` cannot reconstruct the proposed layout as specified.** Plan 01 defines it as `index_json` plus `BTreeMap<hex, Vec<u8>>`, but `OciLayout::write_blob` requires a `MediaType` for every blob ([layout.rs:100](crates/pmcp-package/src/oci/layout.rs:100)). The plan says “media type from the manifest” without defining or validating a recursive descriptor-to-media-type map. This affects manifests, config blobs, layers, and any unreferenced blobs. The artifact representation needs to carry validated descriptors/media types or materialize exact verified blob bytes without fabricating descriptors.

- **HIGH — the `pull` seam is inaccessible to the proposed integration tests.** `pull.rs` is in the bin-only command tree. The library currently mounts only the pure package-kind and GraphQL-contract leaves ([cargo-pmcp/src/lib.rs:141](cargo-pmcp/src/lib.rs:141), [cargo-pmcp/src/lib.rs:155](cargo-pmcp/src/lib.rs:155)). Plan 05 modifies neither `lib.rs` nor a separate lib-mounted pipeline module, yet `package_portability_contract.rs` is expected to inject a fake transport and call the pipeline. An external integration test cannot access a private bin module or construct the binary’s internal trait implementation.

- **HIGH — `render.rs` is likewise not reachable by `cargo test --lib`.** Plan 03 requires renderer tests under the library target but does not modify `lib.rs`. Adding `pub mod render` only to `commands/package/mod.rs` compiles it into the binary command tree, not the library. Either mount a dependency-light render surface in `lib.rs` or test it through a dedicated integration/public seam.

- **HIGH — verify-before-write is incomplete at the semantic package level.** `read_verified` checks tar framing and blob-name hashes, then `write_layout` writes the destination, and only afterward `unpack_*` validates manifest structure, required media types, config blobs, legacy shapes, and deserialization. Those checks are substantive ([unpack.rs:639](crates/pmcp-package/src/oci/unpack.rs:639), [unpack.rs:649](crates/pmcp-package/src/oci/unpack.rs:649)). A correctly content-addressed but malformed package can therefore leave a written destination before `unpack_*` fails. This contradicts the plans’ broad claims that integrity/verification failure leaves the destination unchanged.

- **HIGH — the shared HTTP client cannot currently be reused for the presigned GET as described.** The GraphQL `CLIENT` is function-local inside `execute_graphql_at` ([graphql.rs:470](cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:470), [graphql.rs:481](cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:481)). A new download function cannot access it. Also, the timeout example cited by the plans belongs to a separate S3 upload client that constructs its own client ([graphql.rs:154](cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:154), [graphql.rs:167](cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:167)). Plan 05 must explicitly refactor one module-level shared client or define a justified client policy.

- **MEDIUM — the golden fixture does not test writer conformance.** Plan 04 feeds the independent tar fixture only to `read_verified` and `write_layout`. That binds the reader to the rule, but it cannot catch `write_tar` drifting from normalized headers, entry order, or inventory. The assertion that an independently authored fixture catches writer drift is false unless the writer’s output is also structurally or byte-compared with the fixture.

- **MEDIUM — delaying all Makefile registration until Plan 07 leaves earlier plans falsely “green.”** The existing Makefile says a test name should be added to both lists in the same commit that creates its binary ([Makefile:337](Makefile:337)). Plans 01, 02, 04, and 06 each run `make quality-gate`, but their new integration tests remain outside that gate until Wave 5. Each creator plan can safely add its own name in the same commit; no intermediate red period is necessary.

- **MEDIUM — Plan 07’s fuzz falsifiability experiment is not credible as written.** Removing a large per-entry cap is unlikely to produce a failing input in a short arbitrary-byte campaign, and the proposed “peak in-memory retention” property has no instrumentation. The key/digest invariant is unrelated to the cap, so disabling the cap does not falsify it. Use a bounded structured corpus/dictionary or a test-only configurable limit whose violation is directly asserted.

- **MEDIUM — destination replacement is not transactional.** `OciLayout::create` creates directories and writes `oci-layout` plus an empty index before later blob/index writes ([layout.rs:41](crates/pmcp-package/src/oci/layout.rs:41)). With `--force`, a filesystem error or interruption can leave a partially replaced destination. The plans acknowledge concurrent force as unsupported but still make broader byte-for-byte unchanged claims. A staging sibling directory followed by rename is needed for transactional replacement.

- **MEDIUM — dependency metadata is inconsistent.** Plan 04 consumes and cross-references `portability-v1.graphql` from Plan 02 but declares only `depends_on: ["123-01"]`. Wave prose says Wave 2 follows all of Wave 1, but executable dependency metadata should encode that explicitly.

- **LOW — Plan 01’s type says `type: execute` while its objective says it “LEADS with a tracer (`type=\"tracer\"`)”.** This is planning metadata drift and may affect orchestration.

- **LOW — the mandatory human checkpoint for `tar` provenance is disproportionate to the recorded legitimacy result.** It adds blocking process overhead before any implementation despite the research already classifying the crate as established. This is defensible supply-chain caution, but it should not obscure the more important unresolved artifact-format assumptions.

## Suggestions

- Replace `VerifiedArtifact` with a fully validated in-memory model containing:

  - parsed `ImageIndex`;
  - the selected manifest descriptor and parsed manifest;
  - a digest-keyed blob map;
  - validated media types and sizes for every referenced blob;
  - the locally derived manifest/payload digest.

- Validate semantic package structure before destination writes. A practical approach is to materialize the verified artifact into a temporary layout, run `detect_kind` and `unpack_*` there, then atomically rename the completed layout into place. Subject mismatch remains report-and-exit-1 after the final layout is installed; structural/integrity failure never reaches the destination.

- Extract the reusable pull pipeline into a dependency-light, lib-mounted module. Keep the binary’s `pull.rs` limited to clap arguments, credentials, and invoking that surface. The offline integration test can then inject a transport implementation legitimately.

- Do the same for rendering: either add a dedicated `package_render` mount in `lib.rs`, or make renderer unit tests ordinary module tests compiled through an explicitly mounted pure module.

- Move the `reqwest::Client` to module scope and give it the required timeout policy. Reuse it for GraphQL and presigned downloads while ensuring the authorization header is applied only to GraphQL requests.

- Add writer-conformance tests:

  - compare `write_tar` output against the conformant golden fixture for a precisely matching source layout; or
  - independently parse writer output and assert exact entry order, normalized headers, inventory, and byte-for-byte reproducibility.

- Register each integration test in the Makefile in the same plan/commit that creates it. Preserve the named nonzero-count guard and `RUSTFLAGS=` pin already present at [Makefile:369](Makefile:369).

- Replace fuzz-cap falsifiability with a deterministic unit/property test using an injectable small cap, while retaining raw-byte fuzzing for panic/hang resistance.

- Clarify `payloadDigest` handling before implementing Plan 05. If the backend meaning remains unanswered, isolate it behind a named digest-semantics function and mark the live test parked; do not imply cross-platform interoperability is proven.

- Add explicit dependencies:

  - Plan 04 → Plans 01 and 02.
  - Plan 05 → Plans 01–04 if it consumes the golden fixtures as written.
  - Plan 06 → Plan 05.
  - Plan 07 → all prior plans.

## Risk Assessment

**Overall risk: HIGH.**

The scope and security intent are strong, and most referenced existing mechanisms are real. The in-repo half is genuinely capable of being completed offline. However, the current plan set has multiple implementation-blocking inconsistencies around layout reconstruction, test visibility, HTTP-client reuse, and validation ordering. Those are central to the security property “transport is never trusted” and to the claim that failed verification leaves destinations unchanged. Once those architectural issues are corrected, the remaining work should fall to **MEDIUM risk**, driven mainly by the unresolved platform contract semantics and the intentionally parked live leg.

---

## Gemini Review

> **[reviewed-without-source-citations]** — see the Consensus Summary. This review declares source-grounded
> findings but cites no independent `file:line` evidence, and six of the files it presents as read
> (`artifact.rs`, `render.rs`, `pull.rs`, `portability-v1.graphql`, `package_artifact_framing.rs`,
> `package_round_trip.rs`) do not exist yet — they are outputs this phase is planned to create. Its verdict
> is not counted at full consensus weight; its Finding 3 was independently verified and does stand.

# Cross-AI Plan Review: Phase 123 — Export/Import Verbs (contract-first, parked on the pmcp.run backend)

**Review Target:** Implementation plans [`123-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-01-PLAN.md) through [`123-07-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-07-PLAN.md)  
**Repository:** `/Users/guy/Development/mcp/sdk/rust-mcp-sdk`  
**Phase Focus:** `PKGX-02` (AI-Package Portability Verbs: `save`, `load`, `pull`)

---

## 1. Executive Summary & Verdict

**Verdict: APPROVED — EXCEPTIONAL PLAN QUALITY**

The plan suite for Phase 123 is meticulously structured, mathematically rigorous, and shows exceptional engineering discipline. It resolves the platform verb naming collision, implements an in-memory verify-before-write pipeline that completely prevents zip-slip path traversal, and lands the remote `pull` verb behind a clean transport seam with contract validation and informative error attribution.

### High-Level Metrics
* **Total Plans:** 7 across 5 sequential waves.
* **Requirements Coverage:** 100% of [PKGX-02](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/REQUIREMENTS.md) and Success Criteria SC1–SC5.
* **Architectural Boundaries:** Strict separation of concerns — `tar` codec confined to [`cargo-pmcp`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp), framing rules and golden fixtures in [`pmcp-package`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package) with zero format-crate dependency expansion, and single-API path reuse through [`pmcp_run/auth.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs).
* **Test & Verification Hygiene:** Comprehensive use of falsifiable invariants, negative controls on every gate, `proptest` properties, `cargo fuzz` raw-byte fuzzing, and explicit count assertions against zero-match test execution bugs.

---

## 2. Requirements & Success Criteria Traceability

| Success Criterion / Requirement | Focus | Primary Plan(s) | Verification Mechanism |
| :--- | :--- | :--- | :--- |
| **SC1: Offline `save` / `load`** | `save` config + `deploy.toml` $\to$ `.tar`; `load` `.tar` $\to$ OCI layout dir $\to$ `inspect` opens & prints slots | [`123-01`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-01-PLAN.md), [`123-03`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-03-PLAN.md) | `package_save_load.rs` integration suite, deterministic binary comparison, `inspect` exit 0 |
| **SC2: Verb Collision Resolution & Pin** | `import` stays platform's; `save`/`load`/`pull` added; `--help` preamble tested; `EXPECTED_VERBS` exact-set pin | [`123-06`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-06-PLAN.md) | `verb_help.rs` set-equality assertion against parsed `Commands:` block, preamble pattern assertion |
| **SC3: Single API Path** | `pull` reuses `get_api_base_url()`, `configure` resolver, TTL config cache; no second client or base URL | [`123-05`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-05-PLAN.md) | Grep checks over `cargo-pmcp/src/` forbidding new `PMCP_*_URL`, extra token caches, or duplicate `reqwest::Client` |
| **SC4: Contract-First `getPackageArtifact`** | Vendored `portability-v1.graphql`, offline `apollo_compiler` validation, `pull` names missing platform capability | [`123-02`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-02-PLAN.md), [`123-05`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-05-PLAN.md) | `package_portability_contract.rs`, AST schema/query validation, `anyhow` cause chain inspection |
| **SC5: Tar Framing Rule & Verify-Before-Write** | Normative framing rule in `pmcp-package` docs, checked-in golden fixture corpus, 0 writes on verification error | [`123-01`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-01-PLAN.md), [`123-04`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-04-PLAN.md), [`123-05`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-05-PLAN.md) | `package_artifact_framing.rs`, hostile fixture suite, directory-absence assertions on failure |
| **PKGX-02** | Complete AI-Package portability verbs contract & execution | All plans | End-to-end suite wired into `make quality-gate` |

---

## 3. Architecture & Design Assessment

### 3.1. Verify-in-Memory & Path Traversal Elimination (D-06, D-11, D-12)
* **Zip-Slip Immunity:** The plan avoids `tar::Archive::unpack` completely. Instead, [`read_verified`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/package/artifact.rs) validates all entries into an in-memory structure [`VerifiedArtifact`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/package/artifact.rs). Materialization occurs via [`OciLayout::write_blob`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/layout.rs), where disk filenames are computed strictly from `sha256(bytes)` generated by the SDK itself. Archive path strings are never joined to the destination filesystem.
* **Framing Rule Location:** Placing the normative framing rule and checked-in `.tar` fixtures in [`crates/pmcp-package`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package) (docs + fixture files only) allows both SDK and platform to reference the same specification without expanding `pmcp-package`'s dependency graph or tripping `[bans].allow` / `PURITY_NO_CRYPTO_CRATES`.

### 3.2. Parked Remote Leg & Transport Seam (D-02, D-04, D-05)
* **No Premature Stubbing:** Rather than shipping a dummy error-stub for `pull`, the entire 6-stage pipeline (resolve $\to$ build request $\to$ transport $\to$ verify $\to$ write $\to$ report) is fully implemented. The transport stage sits behind a narrow seam, allowing full offline testing with golden fixtures.
* **Error Attribution (D-05):** When the backend is unreachable, the top-level error context names the missing platform capability `getPackageArtifact`, while maintaining the full underlying network/socket error in the `anyhow` cause chain for `-v` diagnostics.
* **Streaming Cap & Credential Safety:** The presigned `downloadUrl` is fetched via plain unauthenticated streaming GET (avoiding credential leaks to S3/CDN origins) with an enforced running byte cap, preventing decompression/allocation bomb vulnerabilities.

### 3.3. Verb Collision & Exact-Set Pin (D-01, D-03, D-07, D-08, D-09)
* **Surface Legibility:** The `--help` preamble cleanly categorizes verbs across the three directions: local file operations (`save`/`load`), remote artifact fetch (`pull`), and environment admission (`import`).
* **Deliberate Break on Merge:** [`verb_help.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/verb_help.rs) uses set equality against `EXPECTED_VERBS` (including the clap `help` pseudo-subcommand). The constant is extensively documented with its branch context (`feat/package-172-cli`), ensuring that when the 267-commit governance branch merges, the resulting test failure forces an intentional review rather than silent drift.

### 3.4. Reporting & Slot Enumeration (D-10, D-13, D-14, D-15, D-16)
* **Slot vs Deviation:** The plans adhere strictly to the correction from Phase 121: [`required_slots`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/slot/required.rs) is the enumerator, and `detect_deviation` is not used for slot discovery.
* **3-State Pin Reporting:** Following `reference.rs:93-97`, component pins report three distinct states: declared range + resolved version/digest, directly pinned (no declared range), and unresolvable/absent (`resolved_from: None` $\to$ *CANNOT REPORT*, never *NO SKEW*).
* **Attestation Mismatch vs Corruption (D-15):** Byte corruption fails closed without writing. Attestation subject mismatch writes the layout, displays the diagnostic (issuer, claimed subject, re-derived digest), and exits with code 1 outside the quiet gate.

---

## 4. Plan-by-Plan Review

### Wave 1
* **[`123-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-01-PLAN.md) — Artifact Spine Tracer (`save`/`load`):**
  * *Strengths:* Begins with Task 1 as a `blocking-human` checkpoint to verify the maintainership provenance of `tar` before adding it to `Cargo.toml`. Establishes the core tracer with normalized, reproducible `.tar` emission (mtime 0, uid/gid 0, mode 0o644).
  * *Negative Controls:* Negative controls test duplicate entry rejection, path traversal rejection, lying header size rejection, and destination preservation on failure.
* **[`123-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-02-PLAN.md) — Contract-First `getPackageArtifact`:**
  * *Strengths:* Vendors [`portability-v1.graphql`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/contracts/pmcp-run/portability-v1.graphql) with an honest `SDK-PROPOSED` header; keeps pure codec functions in [`graphql_contract.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs); executes offline `apollo_compiler` validation; implements self-announcing skipped live leg.
  * *Hygiene:* Strips comments via `sdl_body()` before checking banned words (`enum`) to avoid false positive / self-invalidating checks.

### Wave 2
* **[`123-03-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-03-PLAN.md) — `load` Report & `save` Scope Guards:**
  * *Strengths:* Creates unified [`render.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/commands/package/render.rs) shared by `load` and `pull`. Handles deterministic rendering sorting, environment variable vs config path distinction, and subject mismatch exit 1 outside `should_output()` gate.
  * *Error Divergence:* Clearly documents why `save` treats unparseable `.pmcp/deploy.toml` as a hard error rather than a graceful fallback.
* **[`123-04-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-04-PLAN.md) — Tar Framing Rule & Golden Fixture Corpus:**
  * *Strengths:* Documents normative prose in [`crates/pmcp-package/src/oci/mod.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-package/src/oci/mod.rs). Authors 1 conformant `.tar` + 8 hostile `.tar` files independently of `write_tar` with recorded provenance in `README.md`.
  * *Binding Test:* [`package_artifact_framing.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/package_artifact_framing.rs) tests reader behavior against checked-in fixture bytes.

### Wave 3
* **[`123-05-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-05-PLAN.md) — `pull` Six-Stage Pipeline:**
  * *Strengths:* Splits `pull` into 6 distinct functions to strictly comply with PMAT cognitive complexity $\le 25$. Reuses [`auth::get_credentials`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs) and [`graphql.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs) without duplicating network logic.
  * *Verification:* Offline tests feed golden fixtures through the seam double, asserting zero pre-network calls when destination exists, and confirming that `pull` and `load` produce byte-identical OCI layouts.

### Wave 4
* **[`123-06-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-06-PLAN.md) — Verb Pin & Outward Platform Note:**
  * *Strengths:* Updates `main.rs` `Package` doc comment with the 3-direction preamble. Pins the exact 9-element subcommand set in [`verb_help.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/tests/verb_help.rs).
  * *Governance:* Task 3 introduces a `decision` checkpoint before drafting [`package-portability-verb-set-sdk-note.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/docs/platform-requests/package-portability-verb-set-sdk-note.md) and updating [`package-portability-pmcp-run-handoff.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/docs/design/package-portability-pmcp-run-handoff.md) §7 to mark the superseded ordering.

### Wave 5
* **[`123-07-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-07-PLAN.md) — ALWAYS Requirements & Makefile Gate Wiring:**
  * *Strengths:* Fulfills CLAUDE.md ALWAYS requirements via [`fuzz_package_artifact.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/fuzz/fuzz_targets/fuzz_package_artifact.rs) (with proved falsifiability) and [`package_round_trip.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/examples/package_round_trip.rs).
  * *Gate Wiring:* Appends `verb_help`, `package_save_load`, `package_portability_contract`, and `package_artifact_framing` to `REQUIRED_TEST_BINARIES` and `--test` list in [`Makefile`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile) in one atomic commit, preserving the `RUSTFLAGS=` pin.

---

## 5. Key Findings & Recommendations

### Finding 1: Integration Test Gate Timing Between Waves 1–4 and Wave 5
* **Context:** In `Makefile`, `REQUIRED_TEST_BINARIES` is append-only. Adding test binary names before they exist turns `make quality-gate` red. Thus, Plan 07 appropriately defers the Makefile edits until all 4 binaries exist.
* **Observation:** During Waves 1 through 4, running `make test-cargo-pmcp-integration` will pass, but it will *not* execute the newly created integration binaries (`package_save_load`, `package_portability_contract`, `package_artifact_framing`, or `verb_help`).
* **Recommendation:** Ensure all individual task and wave verification steps continue to invoke `cargo test -p cargo-pmcp --test <binary_name> -- --test-threads=1` explicitly (with nonzero count checks) until Plan 07 wires the master gate.

### Finding 2: `SaveArgs` OpenAPI Spec Resolution Semantics
* **Context:** In Plan 01 Task 2, `SaveArgs` defines `--spec <PATH>` as optional. It is noted that `london-tube.toml` does not declare a spec path in its `[backend]` table.
* **Observation:** For servers that require an OpenAPI spec (like `pmcp-openapi-server` Shape A), omitting `--spec` on the CLI results in `spec: None` being passed to `pack_server`.
* **Recommendation:** In the CLI help text for `save` and in the runnable example ([`package_round_trip.rs`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/examples/package_round_trip.rs)), clearly document that for OpenAPI-backed Shape A servers, `--spec` must be explicitly provided if not discoverable, whereas pure configuration servers without specs can omit it.

### Finding 3: Synchronization of `123-VALIDATION.md`
* **Context:** [`123-VALIDATION.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-VALIDATION.md) is currently in a draft template state with unfilled placeholders (`{quick command}`, `{full command}`, etc.).
* **Recommendation:** Populate `123-VALIDATION.md` with the commands defined in `123-RESEARCH.md` and `123-07-PLAN.md` (e.g. `RUSTFLAGS= cargo test -p cargo-pmcp --test verb_help --test package_portability_contract -- --test-threads=1` and `RUSTFLAGS="" make quality-gate`), ensuring full alignment with the project's Nyquist validation workflow.

---

## 6. Conclusion

The Phase 123 implementation plan set demonstrates exemplary architecture, rigorous threat modeling, and comprehensive test coverage. All phase boundaries, user decisions, and quality rules from `CLAUDE.md` are respected.

**Execution is fully approved to proceed starting with Wave 1 ([`123-01-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-01-PLAN.md) and [`123-02-PLAN.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba/123-02-PLAN.md)).**

---

## Consensus Summary

**The two reviews disagree at the top line, and the disagreement is not a tie.** Codex returned
**HIGH risk — non-executable as written**; Gemini returned **"APPROVED — EXCEPTIONAL PLAN QUALITY"**.
They are not weighted equally here, because only one of them read the repository.

Codex cited specific `file:line` evidence throughout, and its four load-bearing HIGH findings were
**independently re-verified against source during this review** — all four hold:

| Codex finding | Independent verification |
| :--- | :--- |
| `VerifiedArtifact` (index + digest→bytes map) cannot reconstruct a layout — `write_blob` needs a `MediaType` per blob | **Confirmed.** `crates/pmcp-package/src/oci/layout.rs:108` — `pub fn write_blob(&self, media_type: MediaType, bytes: &[u8])` |
| `render.rs` / the `pull` pipeline are not lib-mounted, yet their tests need library access | **Confirmed.** `cargo-pmcp/src/lib.rs` mounts only `package_kind` (`:153`) and `pmcp_run_graphql` (`:175`). Plans 03 and 05 do not list `lib.rs` in `files_modified`. (Plan 01 *does* mount `package_artifact`, so the artifact seam alone is covered.) |
| The GraphQL client cannot be reused for the presigned GET | **Confirmed.** `CLIENT` is a `OnceLock` declared *inside* `execute_graphql_at` at `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs:481`, built with `Client::new` (no timeout). The timeout builder at `:167` belongs to a separate S3 upload client. |
| Semantic validation runs *after* the destination is written, narrowing the "unchanged on failure" claim | **Confirmed.** `123-01-PLAN.md:306` orders `write_layout` → `detect_kind` → `unpack_*`, while `:376` claims "after ANY of the above refusals … byte-for-byte unchanged." Framing/integrity gates do precede writes; semantic `unpack_*` failures do not. |

Gemini's architectural assessment cites `artifact.rs`, `render.rs`, `pull.rs`,
`portability-v1.graphql`, `package_artifact_framing.rs` and `package_round_trip.rs` as though it had
read them. **None of those files exist** — they are outputs this phase is planned to create. Its one
`file:line` citation (`reference.rs:93-97`) is lifted from the plan text. Its approval is therefore a
review of the plans' own claims restated, which is exactly the failure mode the source-grounding
instruction exists to prevent. This is a **repeat of the pattern already recorded for Phases 116,
118 and 119** in this project: checker-approved plans carrying HIGH defects that only a
source-reading reviewer found.

### Agreed Strengths

Both reviewers independently identified these, and they survive scrutiny:

- **The zip-slip elimination is structural, not filtered.** Destinations derive from a digest computed
  over bytes the SDK holds, so an archive-supplied path is unrepresentable rather than sanitized.
- **Reusing the existing `pmcp_run` resolver seam** (`PMCP_API_URL` → `PMCP_RUN_API_URL` → configured
  → default, plus the TTL'd endpoint-keyed cache) rather than inventing a second API path — SC3.
- **The framing rule + independently-authored golden fixtures** live in `pmcp-package` as prose and
  bytes only, leaving Phase 122's dependency allowlist and the nine-emitter version lockstep untouched.
- **Integrity failure and attestation subject mismatch stay distinct verdicts** rather than being harmonized.
- **The verb inventory and the `--help` three-direction preamble** correctly preserve `import` as the
  platform's meaning.
- **The `verb_help` gate gap is real.** Verified independently: `verb_help` appears **zero** times in
  the Makefile; `test-cargo-pmcp` is `--lib` only and the integration selector names just four other
  binaries. Plan 07's claim that SC2's pin "would have read green forever" is accurate — and the false
  comment it fixes is live at `cargo-pmcp/tests/verb_help.rs:37-38`.

### Agreed Concerns

Only one concern was raised by both reviewers — and they **disagree on its remedy** (see Divergent Views):

- **Makefile gate registration is deferred to Plan 07 (MEDIUM).** Both note that Waves 1–4 create
  integration binaries that `make quality-gate` does not execute. Every plan in those waves runs the
  gate and passes without its own new tests ever running.

Concerns raised by Codex alone, all source-verified above and none contradicted by Gemini
(Gemini simply did not look): the four HIGH items in the table, plus —

- **MEDIUM — the golden fixture cannot catch *writer* drift.** Plan 04 feeds the fixture only to the
  reader. Binding the reader to the rule does not bind `write_tar` to it; the plan's claim that an
  independently-authored fixture catches writer drift needs a writer-side comparison to be true.
- **MEDIUM — destination replacement is not transactional** under `--force`; a staging-then-rename is
  needed for the byte-for-byte claims to hold under interruption.
- **MEDIUM — Plan 07's fuzz falsifiability experiment is not credible** — removing a large per-entry cap
  is unlikely to produce a failing input from arbitrary bytes, and the cap is unrelated to the
  key/digest invariant it is said to falsify.
- **MEDIUM — `depends_on` metadata is under-specified.** Plan 04 consumes Plan 02's SDL but declares
  only `["123-01"]`.
- **LOW — Plan 01's `type: execute` contradicts its own objective**, which says it leads with `type="tracer"`.

Raised by Gemini alone and independently verified as real:

- **`123-VALIDATION.md` is an unfilled template.** Confirmed: `:22-51` still carry
  `{quick command}`, `{full command}`, `REQ-{XX}`, `{tests/test_file.py}` placeholders.

### Divergent Views

1. **Whether deferring Makefile registration to Plan 07 is correct.** Gemini calls it "appropriately
   deferred" and asks only that intermediate waves run `cargo test --test <name>` by hand. Codex calls
   it a false-green and notes the Makefile's own instruction at `Makefile:337` says a name should be
   added to both lists in the same commit that creates its binary — each creator plan can safely
   register itself, so no intermediate red period is necessary. **Codex has the stronger case**: it
   cites the repo's existing convention, and "remember to run it by hand" is precisely the discipline
   the named-count gate exists to replace. Worth noting Plan 07's own must-have asserts the
   append-only ordering is required — that assertion should be re-examined, not assumed.

2. **Overall risk and readiness.** HIGH/non-executable versus approved-to-proceed. Given that four
   HIGH findings were re-verified against source and the approving review read none of that source,
   the plan set should be treated as **needing revision before Wave 1**, not as approved.

### Recommended Next Step

The four HIGH findings are architectural and cheap to fix at plan time, expensive after Wave 1:
carry `MediaType`/descriptor data in `VerifiedArtifact`; add lib mounts (or a lib-mounted pipeline
module) for the render and pull seams; hoist the `reqwest::Client` to module scope with a timeout
policy; and either stage-then-rename or narrow the "destination unchanged" claim to the gates that
actually precede writes.

```
/gsd-plan-phase 123 --reviews
```
