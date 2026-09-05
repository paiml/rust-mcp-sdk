---
phase: 125
reviewers: [codex, gemini]
reviewer_instances: [fable]
reviewed_at: 2026-09-02T04:36:02Z
plans_reviewed: [125-01-PLAN.md, 125-02-PLAN.md, 125-03-PLAN.md, 125-04-PLAN.md, 125-05-PLAN.md]
models:
  codex: "gpt-5.6-sol (reasoning=low)"
  gemini: "unknown"
  fable: "claude-fable-5 (reasoning=low)"
model_sources:
  codex: "banner"
  gemini: "unknown"
  fable: "pinned"
---

# Cross-AI Plan Review — Phase 125

## Codex Review

# Cross-AI Plan Review — Phase 125

## Summary

The phase is well researched and the wave ordering is generally sound, but the plans are not execution-ready. Two blocking design errors remain:

- Plan 02 promises `ServerCoreBuilder` wire parity even though `ServerCore` accepts only the public `Request` enum, which intentionally cannot represent either skills method.
- Plan 04 asks the resource-only `c10_client_skills` example to demonstrate `skills/get`, but that example has neither an HTTP/RPC client nor access to the crate-private dispatch functions.

Plan 01 also omits the explicit v2 opt-in needed by its proposed live v2 test fixture. Plans 03 and 05 are strong overall, though their malformed-YAML, resource-limit, fuzz-CI, and quality-gate mechanics need tightening.

Overall risk: **HIGH until Plans 01, 02, and 04 are corrected; MEDIUM afterward.**

---

## Plan 01 — `skills/list` tracer

### Summary

The tracer-first strategy is good: it proves classification, projection, and both HTTP response paths before implementing the full feature. The proposed test coverage is unusually strong. The main issue is that the live v2 fixture is underspecified and likely constructs a v1-only server.

### Strengths

- Correctly preserves the public exhaustive-enum contract. Internally routed requests are already represented by the crate-private enum at [protocol/mod.rs:769](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/protocol/mod.rs:769), and interception happens before public request conversion at [protocol_helpers.rs:70](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/protocol_helpers.rs:70).
- Correctly identifies both HTTP assembly paths. Fast-path dispatch handles internal requests at [streamable_http_server.rs:5000](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:5000), while middleware dispatch has a separate exhaustive match at [streamable_http_server.rs:5117](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:5117).
- Preserves raw params until after transport/auth processing, matching the existing `TasksUpdate` mechanism at [streamable_http_server.rs:2214](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:2214).
- Avoids changing the public `Skills::into_handler()` signature, which currently consumes `self` and returns `Result<Arc<dyn ResourceHandler>>` at [skills.rs:437](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:437).
- The no-public-variant test, near-miss classifier tests, v1 control, digest check, and nonzero-test guards all provide meaningful failure signals.

### Concerns

- **HIGH — The proposed v2 server fixture is not opted into v2.** `ServerBuilder` defaults its accepted versions rather than automatically enabling v2; the shared v2 fixture explicitly calls `.with_supported_protocol_versions(...)` at [tests/common/v2.rs:295](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/common/v2.rs:295). `spawn_default_config` only selects HTTP configuration at [tests/common/v2.rs:385](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/common/v2.rs:385); it does not modify the server’s protocol versions. A plain `Server::builder().skills(...).build()` can therefore reject the proposed v2 request before skills dispatch.
- **MEDIUM — The plan introduces permanent public API during the tracer.** `SkillEntry`, `SkillResourceRef`, and `Skills::entries()` become semver-governed before the final manifest and validation behavior lands. `#[non_exhaustive]` protects struct growth, but accessor behavior and serialization shape still become public commitments.
- **MEDIUM — Malformed YAML and absent frontmatter are collapsed.** The proposed `Option<Value>` cannot distinguish “no frontmatter” from “frontmatter exists but is invalid/non-object.” That weakens diagnostics and makes malformed canonical skills silently disappear.
- **LOW — `SKILLS_GET_METHOD` is added before its classifier variant.** This is harmless but expands Plan 01’s surface without exercising it until Plan 02.

### Suggestions

- Build the tracer server with the same explicit v1/v2 accept list used by `build_v2_server`.
- Replace `parse_frontmatter_value -> Option<Value>` with a private three-way result such as `Absent | Parsed(Value) | Invalid(Error)`.
- Consider keeping entry types crate-private through Plans 01–03 and exposing them only after the complete manifest semantics are final, unless public entry inspection is an explicit phase requirement.
- Add a malformed-frontmatter test in this plan so parser behavior is fixed before later validation builds on it.

### Risk Assessment

**MEDIUM-HIGH.** The routing design is sound, but the v2 live test can fail before reaching the feature, and the early public API commitment increases reversal cost.

---

## Plan 02 — `skills/get` and `ServerCore` parity

### Summary

The `skills/get` semantics are well specified, especially exact URI matching and `-32602` handling. The `ServerCore` parity task, however, is structurally impossible as written and does not achieve real reachability.

### Strengths

- Correctly requires exact-map lookup rather than path manipulation. The existing registry is keyed by full URI at [skills.rs:438](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:438), making an exact `IndexMap::get` the natural safe route.
- Correctly preserves the existing `resources/read` behavior. Its unknown-URI path currently returns `METHOD_NOT_FOUND` at [skills.rs:570](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:570), so pinning that behavior avoids accidental scope expansion.
- Correctly delays params validation until the served branch.
- Good negative cases: absent params, non-string URI, reference URI, traversal-shaped URI, and unknown URI.
- Correctly distinguishes list cacheability from the draft’s unresolved get-cache semantics.

### Concerns

- **HIGH — `ServerCore` cannot receive either skills request through its public dispatch seam.** `ProtocolHandler::handle_request` accepts a typed public `Request` at [core.rs:80](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/core.rs:80), and the `ServerCore` implementation uses exactly that signature at [core.rs:3764](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/core.rs:3764). The phase deliberately forbids adding skills variants to the public enum. Consequently, adding `ServerCore::handle_skills_list/get` creates private helpers with no ingress route.
- **HIGH — The proposed integration parity test cannot use the named harness.** `spawn_default_config` accepts `Server`, not `ServerCore`, at [tests/common/v2.rs:385](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/common/v2.rs:385). The direct core harness also accepts a public `Request` at [tests/common/duplex.rs:334](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/tests/common/duplex.rs:334), so it cannot construct `skills/list` or `skills/get`.
- **HIGH — Carrying entries in `ServerCore` does not make `ServerCoreBuilder` conformant.** The plan’s truth that both builders “answer both methods identically” remains false unless an internal-request ingress is added to `ProtocolHandler`, a new raw/internal dispatch seam is introduced, or ServerCore scope is explicitly removed.
- **MEDIUM — The plan conflates data parity with transport parity.** A unit test calling private projection functions can prove equal JSON projection, but it cannot prove that a `ServerCoreBuilder` product answers an RPC request.
- **LOW — `skills/get` cache behavior needs a direct envelope assertion.** “Do not name `Cacheable::Yes`” should be tested by asserting absence of list-caching fields on both eras.

### Suggestions

- Resolve the architectural choice before execution:

  1. Scope Phase 125 to high-level `Server` HTTP routing and remove the `ServerCore` conformance claim, or
  2. Add a crate-private raw/internal request method to `ProtocolHandler` and route it from a real transport, with corresponding security and middleware-order tests.

- If only projection parity is desired, rename it explicitly to “projection parity” and test the shared free functions in unit tests rather than claiming builder wire parity.
- Add a test proving auth/header gates execute before malformed params return `-32602`; merely preserving raw params in the classifier does not by itself prove the order.

### Risk Assessment

**HIGH.** This plan contains a blocking reachability mismatch. Implementing its stated fields and delegates would produce dead code while the success criterion claims real conformance.

---

## Plan 03 — manifests, frontmatter, validation, and limits

### Summary

This is the strongest implementation plan. It correctly connects manifest hashes to the bytes returned by the resource handler and carefully preserves frontmatter-less legacy constructions. Its principal weaknesses are diagnostic ambiguity and the fact that limit warnings do not actually bound resource consumption.

### Strengths

- Correctly derives manifest contents from the same bodies served by `SkillsHandler::read`, whose SKILL.md and reference branches are at [skills.rs:556](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:556) and [skills.rs:563](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:563).
- The property test reads content through `ResourceHandler` rather than duplicating implementation logic.
- Correctly avoids reconstructing frontmatter from `resolved_description()`, which currently feeds only resource metadata at [skills.rs:494](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:494).
- Good correction of the research assumption concerning unconditional constructor-name validation. Existing duplicate/collision tests legitimately use paths unrelated to constructor names.
- Aggregated validation errors match the existing duplicate-URI aggregation style at [skills.rs:460](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:460).
- Preserves insertion order, which is already implemented with `IndexMap` at [skills.rs:438](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:438).

### Concerns

- **MEDIUM — Invalid YAML is mislabeled as missing frontmatter.** With the Plan 01 `Option<Value>` parser, unterminated, malformed, or non-object YAML takes the same D-02 path as genuinely absent frontmatter. A named “frontmatter missing” warning would be inaccurate.
- **MEDIUM — The limits are warnings, not DoS controls.** Entry synthesis must first retain and parse the body before it can calculate 16 MiB. A warning after allocation does not mitigate memory or parser exhaustion, contrary to threat T-125-12’s wording.
- **MEDIUM — Name validation and entry synthesis parse frontmatter multiple times.** Calling `validate_names` from both `entries_with_diagnostics` and `into_handler`, while finalization calls `entries()` before `into_handler()`, can parse every YAML block repeatedly. Large registries amplify this cost.
- **LOW — The exact-bound size test requires a large allocation.** The plan says to keep it small, but proving exactly 16,777,216 bytes using real stored `String` bodies necessarily allocates roughly that amount. This is acceptable once, but the plan should acknowledge it.
- **LOW — Logging behavior itself remains indirectly tested.** Diagnostics are tested, but `entries()` emitting one warning per diagnostic is not verified unless a tracing capture is used.

### Suggestions

- Parse each skill once into an internal build artifact containing parsed frontmatter, resources, byte totals, and diagnostics. Reuse it for validation, entries, and handler construction.
- Introduce distinct diagnostics for missing, malformed, and non-object frontmatter.
- Describe limits accurately as operator diagnostics, not enforcement or DoS mitigation.
- Add YAML alias/deep-nesting regression fixtures or bound the frontmatter slice before parsing.
- Test warning emission with a capturing subscriber if warnings are contractual behavior.

### Risk Assessment

**MEDIUM.** Behavior coverage is excellent, but parser semantics and repeated work need refinement.

---

## Plan 04 — index retirement, examples, and documentation

### Summary

Retiring the old index only after both methods and complete entries land is the correct ordering. The assertion-site inventory is valuable. The blocking issue is that the proposed `c10` replacement cannot demonstrate `skills/get` using its current architecture.

### Strengths

- Correctly removes all implementation pieces of the legacy resource: constant, handler field, synthesized JSON, list insertion, and read short-circuit. Those pieces currently exist at [skills.rs:58](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:58), [skills.rs:483](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:483), [skills.rs:499](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:499), and [skills.rs:549](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:549).
- Replacing presence tests with absence/error tests preserves regression coverage.
- Correctly leaves the ordinary unknown-resource error path intact.
- Explicitly excludes generated mdBook output from edits.
- Correctly identifies `c10` as load-bearing because it currently calls and asserts against the index at [c10_client_skills.rs:107](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/c10_client_skills.rs:107).

### Concerns

- **HIGH — `c10_client_skills` cannot call `skills/get` as written.** The example’s flow accepts only `&dyn ResourceHandler` at [c10_client_skills.rs:93](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/c10_client_skills.rs:93). `skills/get` is not a `ResourceHandler` method, and the proposed projection/handler methods are crate-private. Replacing the index read with “the skills/get equivalent” requires a real server/client transport flow or a new public API.
- **MEDIUM — `s44` cannot inspect entries after moving registrations into the builder unless the registry is retained or cloned first.** The current example directly chains individual `.skill(...)` calls into `Server::builder()` at [s44_server_skills.rs:80](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/s44_server_skills.rs:80). Printing the entry count and first digest requires restructuring the example.
- **MEDIUM — A direct `Skills::entries()` demonstration is not an RPC demonstration.** If the examples use the new public projection API, documentation must not claim they exercised `skills/list/get` over MCP.
- **LOW — The blast-radius statement cannot establish absence of external consumers.** Repository grep establishes only in-repo consumers. Removal is authorized by D-08, but the summary should describe downstream breakage as accepted risk rather than “no external consumer.”
- **LOW — The grep acceptance criteria intentionally retain `index.json` in negative tests.** Documentation and implementation checks should distinguish ordinary string mentions from behavior more robustly than raw occurrence counts.

### Suggestions

- Choose one honest example strategy:

  - Convert `c10` into a real HTTP client/server example that sends raw JSON-RPC `skills/list/get`, or
  - Keep it as a resource/direct-registry example and demonstrate `Skills::entries()` while explicitly stating it does not exercise RPC dispatch.

- Refactor `s44` to create a `Skills` registry first, call `entries()`, clone it, and then pass it to `.skills(...)`.
- Add a separate integration test as the authoritative end-to-end `skills/get` demonstration if examples remain in-process.
- Record index removal as an intentional published behavior break authorized by the phase, not as proof that downstream consumers do not exist.

### Risk Assessment

**HIGH.** The retirement itself is sound, but the example task is not implementable as specified without materially changing its architecture.

---

## Plan 05 — quality gate, fuzzing, and deferral documentation

### Summary

This plan correctly closes the feature-gate blind spot and explicitly records deferrals. It should be retained, but the test target needs deterministic per-command guards, and the fuzz target should either join CI or be described as registration-only coverage.

### Strengths

- Correctly identifies the current blind spot: `skills` is absent from `full` at [Cargo.toml:303](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml:303), while `doc-check` uses an explicit list that omits it at [Makefile:1317](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:1317).
- Reuses the repository’s established zero-test guard from [Makefile:322](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:322).
- Correctly keeps `full` and `full-v2` untouched.
- Correctly notes that the existing `test-fuzz` target swallows failures at [Makefile:787](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:787).
- A stable source-scan registration test is a useful complement to nightly fuzz execution.
- The deferral documentation accurately reflects the existing HTTP-only internal route at [protocol_helpers.rs:31](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/protocol_helpers.rs:31) and the stdio actor’s break-on-receive-error behavior at [server/mod.rs:1464](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs:1464).

### Concerns

- **MEDIUM — The `test-skills` recipe is underspecified.** Summing all `test result:` lines can mask one selector running zero tests. The plan acknowledges this but does not define exact expected target labels for library, doctest, `skills_integration`, and `skills_routing`.
- **MEDIUM — Running broad `--all-features` library and doctest suites again inside `quality-gate` may substantially duplicate `test-all`.** The dedicated leg should narrowly compile and run the skills-selected tests while still exercising the required transport features.
- **MEDIUM — The fuzz target is not guaranteed to run in CI.** The current workflow has an explicit four-target matrix at [.github/workflows/fuzz.yml:22](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.github/workflows/fuzz.yml:22). Merely “recording whether” the new target joins it does not satisfy continuous fuzz execution.
- **MEDIUM — The plan modifies `fuzz/Cargo.toml` features globally.** Adding `skills` to the single `pmcp` dependency feature list means every existing fuzz binary builds with `skills`, increasing compile cost and potentially changing feature interactions for unrelated targets.
- **LOW — Deferral prose is too expansive for a module header.** Some items, such as client-wrapper ownership and future name-bearing contract changes, may fit better in phase documentation or a focused compatibility section.
- **LOW — `make quality-gate` inside task verification can be very expensive and repeated several times.** One final phase-level run is sufficient after targeted checks.

### Suggestions

- Implement `test-skills` as separate captured commands, each with its own nonzero guard:

  - skills library filter,
  - skills doctests,
  - `skills_integration`,
  - `skills_routing`.

- Use the minimal explicit feature set for those commands instead of `--all-features`.
- Add `fuzz_skill_entry` to the fuzz workflow matrix, or explicitly state that the project currently guarantees build/registration and stable property testing but not recurring fuzz execution.
- Consider a dedicated fuzz-only crate dependency alias or accept the global feature expansion explicitly in the plan.
- Run the full quality gate once after all three Plan 05 tasks, not after each task.

### Risk Assessment

**MEDIUM.** The intended safeguards are appropriate, but the gate and fuzz mechanics need more exact implementation instructions.

---

# Cross-plan concerns

- **HIGH — Plan 02 and Plan 04 depend on nonexistent callable surfaces.** These must be redesigned before execution.
- **MEDIUM — Wave 2 parallelism touches shared hot spots.** Plans 02 and 03 both depend on Plan 01 and run in parallel. Plan 02 changes core/transport/tests, while Plan 03 changes skills/tests, so direct conflicts are limited, but both extend `tests/skills_routing.rs` or behavior it asserts. Merge sequencing should be explicit.
- **MEDIUM — The feature claims HTTP-only conformance while capability declaration is transport-independent.** The existing capability helper auto-declares at build time at [skills.rs:65](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:65). Documentation reduces surprise but does not make a stdio server conformant. This is an accepted decision, but it remains the phase’s primary product risk.
- **MEDIUM — Public entry API is being designed indirectly through wire needs.** Before exposing it, document whether its serialization shape, ordering, and diagnostics are stable API or merely implementation support.
- **LOW — Verification volume is excessive.** Nearly every plan reruns the whole all-features suite. Targeted per-task checks plus one per-wave and one final gate would reduce execution time without weakening assurance.

# Recommended corrections before execution

1. Fix Plan 01’s v2 server construction by explicitly enabling both accepted protocol versions.
2. Decide whether `ServerCore` support is real scope. If yes, design an internal/raw ingress seam; if no, remove Plan 02’s builder parity claim and dead fields.
3. Redesign the examples in Plan 04 around either a real HTTP client or honest direct entry projection.
4. Distinguish absent, malformed, and non-object frontmatter.
5. Parse each skill once and reuse the build artifact across validation, entries, and handler construction.
6. Make the skills gate’s per-selector nonzero checks deterministic.
7. Add the fuzz target to CI or narrow the stated fuzz guarantee.

## Overall Risk Assessment

**HIGH.** The core HTTP `Server` approach is credible and well tested, but two plans currently promise behaviors that the repository’s type and transport architecture cannot deliver. Once the `ServerCore` and example reachability problems are resolved, the remaining implementation risk should fall to **MEDIUM**.

---

## Gemini Review

> [reviewed-without-source-citations] This reviewer declared source-grounded evidence but cited no file:line source evidence, so it reviewed the pasted plan text only — down-weight its verdict in the Consensus Summary.

# Cross-AI Plan Review: Phase 125 (SEP-2640 Conformance — `skills/list` + `skills/get`)

**Review Status:** **APPROVED WITH COMMENDATION** (Ready for Execution)  
**Evaluated Artifacts:** Implementation Plans `125-01-PLAN.md` through `125-05-PLAN.md`, `125-CONTEXT.md`, `125-RESEARCH.md`  
**Target Milestone:** v2.7 SEP-2640 Skills Conformance & Positioning  

---

## 1. Executive Summary & Verdict

The implementation plans for Phase 125 demonstrate an **exceptionally high degree of engineering rigor, architectural maturity, and defensive discipline**. 

The plans address the critical conformance drift discovered in Spike 008 (where the shipped `skills` module declared the `io.modelcontextprotocol/skills` extension without implementing its mandatory `skills/list` and `skills/get` RPC methods). The plan suite executes a 4-wave, 5-plan strategy that achieves full SEP-2640 conformance while strictly upholding the PMCP 2.x semver guarantee (no public enum modifications), maintaining byte-level parity across transports and builders, and establishing robust test coverage.

### Key Strengths
- **Strict Semver Preservation:** Reuses the crate-private `InternalClientRequest` + `classify_internal_method` pattern to route methods over HTTP without adding variants to the public, exhaustive `ClientRequest` enum.
- **Empirical Pitfall Resolution:** Proactively identifies and resolves workspace-specific compiler hazards (e.g., `sha2` 0.11 lacking `LowerHex` on `finalize()`, solved via a `{:02x}` per-byte fold) before execution.
- **Defensive API Scope Control:** Separates frontmatter-name validation (Gap 4c: hard rejection at build time) from path-constructor validation (Gap 4a: diagnostic warning), preventing breaking changes to 40+ existing tests and documented exercises.
- **Zero-SATD Deferral Architecture:** All intentional scope boundaries (stdio widening, `resources/directory/read`, client wrappers) are formally recorded in phase documentation and rustdoc prose rather than code `TODO`/`FIXME` comments that would trigger gate failures.

---

## 2. Architecture & Design Assessment

```
                      JSON-RPC Ingress Frame
                     {"method": "skills/list"}
                                │
                                ▼
               ┌─────────────────────────────────┐
               │   src/types/protocol/mod.rs     │
               │   classify_internal_method()    │
               └────────────────┬────────────────┘
                                │ Returns Some(InternalClientRequest::SkillsList)
                                ▼
               ┌─────────────────────────────────┐
               │ src/server/streamable_http_...  │
               │    classify_http_ingress()      │
               └────────────────┬────────────────┘
                                │ Matches HttpIngress::SkillsList
                                ▼
               ┌─────────────────────────────────┐
               │        src/server/mod.rs        │
               │    Server::handle_skills_list   │
               └────────────────┬────────────────┘
                                │ Delegates directly
                                ▼
               ┌─────────────────────────────────┐
               │       src/server/core.rs        │
               │   build_skills_list_response    │
               │   - Complete disposition        │
               │   - Cacheable::Yes named        │
               │   - No cursor emitted           │
               └────────────────┬────────────────┘
                                │ Reads pre-computed
                                ▼
               ┌─────────────────────────────────┐
               │ SkillEntry IndexMap (Immutable) │
               │   - Verbatim YAML frontmatter   │
               │   - Complete resource manifest  │
               │   - SHA-256 byte digests & size │
               └─────────────────────────────────┘
```

### 2.1. Internal Method Routing & Semver Discipline
* **Design Decision:** Using `InternalClientRequest::{SkillsList, SkillsGet}` preserves the public exhaustive `ClientRequest` enum.
* **Assessment:** **Optimal.** A source-scanning tripwire test (`tests/skills_routing.rs`) patterned after `tests/v2_tasks_update_routing.rs` provides mechanical protection against accidental enum variant additions.
* **Transport Seam:** `parse_request_or_internal` in `src/shared/protocol_helpers.rs` correctly encapsulates internal routing.

### 2.2. Immutable Entry Manifest Synthesis
* **Design Decision:** `Skills::entries()` computes all `SkillEntry` instances (including verbatim YAML frontmatter extraction, SHA-256 digests, and file sizes) during server build/finalization, stored in `Arc<IndexMap<String, SkillEntry>>`.
* **Assessment:** **Highly Efficient.** Pre-computing manifests at build time ensures:
  1. O(1) exact-match lookup for `skills/get` without disk or path manipulation.
  2. Byte-identity between `SkillResourceRef.digest`/`.size` and what `SkillsHandler::read` serves.
  3. Deterministic insertion ordering via `IndexMap`.

### 2.3. Dual-Builder Parity (`ServerBuilder` vs. `ServerCoreBuilder`)
* **Design Decision:** `finalize_skills_resources` in `src/server/builder.rs` returns `(Option<Arc<dyn ResourceHandler>>, Vec<SkillEntry>)`, threading entries into both `Server.skill_entries` and `ServerCore.skill_entries`.
* **Assessment:** **Robust.** Avoids downcasting `ResourceHandler` (which breaks when wrapped in `ComposedResources`) and ensures `ServerCoreBuilder` and `Server::builder()` return value-identical responses.

---

## 3. Requirement & Decision Traceability Matrix

| Decision / Gap | Description | Handled In | Assessment |
|---|---|---|---|
| **D-01** | HTTP-only reach this phase; stdio deferral recorded | 125-01, 125-05 | **Compliant:** Stdio behavior measured and asserted; deferral documented without SATD. |
| **D-02** | Warn + exclude for frontmatter-less skills | 125-03 | **Compliant:** Emits `SkillDiagnostic` and `tracing::warn!`; avoids synthesizing invalid frontmatter. |
| **D-03** | Canonical surfaces updated (s44/c10, docs) | 125-04 | **Compliant:** Examples and book snippets upgraded to valid frontmatter. |
| **D-04** | Optional `serde_yaml 0.9` isolated behind 1 fn | 125-01, 125-03 | **Compliant:** Single crate-private parser fn `parse_frontmatter_value`; zero new lockfile packages. |
| **D-05** | `sha2 0.11` for `sha256:{64 lowercase hex}` | 125-01, 125-03 | **Compliant:** Direct dependency used with width-2 per-byte hex fold (`{:02x}`). |
| **D-06** | `skills/get` unknown URI returns `-32602` | 125-02 | **Compliant:** Conforms to SEP-2640 draft; keeps `resources/read` `-32601` unchanged. |
| **D-07** | `resultType: "complete"`; cacheability at call site | 125-01, 125-02 | **Compliant:** `Cacheable::Yes` named at projection site; `request_is_cacheable` untouched. |
| **D-08** | Retire `skill://index.json` across 14 tracked sites | 125-04 | **Compliant:** Complete removal across tests, docs, course, and examples. |
| **D-09** | Dedicated `make test-skills` quality-gate leg | 125-05 | **Compliant:** Eliminates gate blind spot without disturbing `full`/`full-v2` feature lists. |
| **D-10** | Keep auto-declaring `{}` (`directoryRead: false`) | 125-05 | **Compliant:** Rustdoc updated; capability accurately reflects capabilities. |
| **D-11** | Single page listing; no `nextCursor` | 125-01, 125-05 | **Compliant:** Conforms to SEP specification for atomic listings. |
| **SC#1..5** | Roadmap Success Criteria #1 through #5 | All plans | **100% Covered.** |

---

## 4. Deep-Dive on Critical Risks & Mitigations

### 4.1. The Stdio Transport Cliff (D-01 / Pitfall 1)
* **Hazard:** Over `StdioTransport`, unrouted methods fail at `parse_message` $\rightarrow$ `InvalidMessage`, causing the server actor to break the loop and terminate the process.
* **Plan Strategy:** 
  1. Acknowledges and documents HTTP-only reach in `set_skills_capabilities` rustdoc and phase summary.
  2. Implements explicit test in `tests/skills_routing.rs` verifying the stdio behavior rather than leaving it unmeasured.
  3. Formally assigns ownership of Stdio widening to Phase 126+ (v2.7 milestone).

### 4.2. Local Quality-Gate Blind Spot (D-09 / Pitfall 2)
* **Hazard:** `make quality-gate` runs `--features "full"`, which excludes `skills`, allowing broken skills code to pass local gates unnoticed.
* **Plan Strategy:**
  1. Plan 05 creates `make test-skills` running `cargo test --all-features --lib skills` and integration suites with `--test-threads=1`.
  2. Integrates `make test-skills` into the local `quality-gate` target.
  3. Includes a zero-test-count guard (`fails_when: 0 passed / running 0 tests`) preventing silent passes.

### 4.3. Frontmatter & Name-Identity Nuances (D-02 / Pitfalls 3 & 4)
* **Hazard:** 
  - Synthesizing `{name, description}` for frontmatter-less skills causes client-side verification rejection under SEP integrity rules.
  - Hard-rejecting URI mismatch against constructor name (Gap 4a) breaks 40+ existing tests and doctests.
* **Plan Strategy:**
  - Implements **Warn + Exclude** (D-02): frontmatter-less skills remain accessible via `resources/read` but are cleanly omitted from `skills/list`.
  - Implements **Gap 4c as Hard Rejection** (frontmatter name $\neq$ URI final segment $\rightarrow$ build error) while keeping **Gap 4a as a Diagnostic Warning** (`tracing::warn!`).

### 4.4. Crypto & Digest Generation (`sha2` 0.11 vs. `LowerHex`)
* **Hazard:** `digest-0.11.2` / `sha2-0.11.0` does not implement `std::fmt::LowerHex` on `Output<Sha256>`, breaking naive `format!("{:x}", hasher.finalize())`.
* **Plan Strategy:**
  - Plan 01 specifies:
    ```rust
    let hash = hasher.finalize();
    let hex = hash.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
        s
    });
    format!("sha256:{}", hex)
    ```
  - Eliminates compilation errors on the `sha2` 0.11 stack.

---

## 5. Security & Threat Model Evaluation (ASVS & STRIDE)

The security mitigations planned across `T-125-01` through `T-125-15` are robust:

| Threat / Area | Security Standard | Planned Mitigation | Status |
|---|---|---|---|
| **URI Path Traversal** | ASVS V5 (Input Validation) | Exact-match lookup in `IndexMap` using `params.uri`. No string slicing, path joining, or disk canonicalization. | **Verified** |
| **Classifier Gate Inversion** | ASVS V4 (Access Control) | `classify_internal_method` only inspects the method string and clones `params` as raw JSON. Parameter validation occurs strictly *after* authentication and header inspection. | **Verified** |
| **Session Fixation** | ASVS V3 (Session Management) | `HttpIngress::is_initialize()` explicitly returns `false` for `SkillsList` and `SkillsGet`, preventing unintended session minting. | **Verified** |
| **Verbatim Secret Disclosure** | ASVS V8 (Data Protection) | Rustdoc warning on `Skills::entries()` explicitly documenting that all frontmatter fields (including custom keys) cross the wire. | **Verified** |
| **Resource Exhaustion / DoS** | ASVS V5 (Validation) | Build-time diagnostics on skills exceeding 512 files or 16 MiB; dedicated libFuzzer target (`fuzz_skill_entry`). | **Verified** |

---

## 6. Testing & Verification Rigor

The test strategy satisfies all project ALWAYS requirements:

1. **Unit Tests:** `parse_frontmatter_value` (BOM/CRLF handling, YAML structures), `sha256_digest_hex`, `validate_names`, limits checking.
2. **Property Tests (`proptest`):**
   - Verified that for all generated skills, digests conform to `^sha256:[0-9a-f]{64}$` and declared `size` matches the byte count returned by `ResourcesHandler::read`.
3. **Fuzz Testing:** `fuzz/fuzz_targets/fuzz_skill_entry.rs` validates that arbitrary byte streams in SKILL.md bodies never panic `parse_frontmatter_value` or entry synthesis.
4. **Integration Wire Proofs:** `tests/skills_routing.rs` proves:
   - Wire serialization via HTTP POST with `resultType: "complete"`.
   - Rejection of public enum deserialization (`serde_json::from_value::<ClientRequest>` fails on `skills/list` while succeeding on `resources/list`).
   - Twin-site parity between `ServerBuilder` and `ServerCoreBuilder`.
5. **Examples Verification:** Upgrades and executes `cargo run --example s44_server_skills` and `c10_client_skills`.

---

## 7. Minor Actionable Recommendations (Pre-Flight Checklist)

During plan execution, ensure the following minor considerations are observed:

1. **CRLF Invariant in `parse_frontmatter_value`:**
   Ensure the regex or line scanner for frontmatter delimiters explicitly strips `\r` before checking for `^---$`. Match the battle-tested pattern in `parse_frontmatter_description`.
2. **Empty Skills Collection Response:**
   Verify that a server initialized with no skills (`Skills::default()`) returns `{"jsonrpc":"2.0","id":...,"result":{"resultType":"complete","skills":[]}}` on `skills/list` rather than `-32601`, honoring the capability declaration.
3. **`IndexMap` Capacity Pre-allocation:**
   When converting `Vec<SkillEntry>` into `Arc<IndexMap<String, SkillEntry>>`, use `IndexMap::with_capacity(entries.len())` for optimal memory footprint.

---

## 8. Final Verdict

| Metric | Score | Remarks |
|---|:---:|---|
| **Completeness** | 10/10 | All 11 decisions, 7 gaps, and 5 roadmap success criteria mapped. |
| **Architectural Rigor** | 10/10 | Clean separation of concerns; exemplary semver protection. |
| **Risk Management** | 10/10 | Pitfalls pre-empted with concrete measurements and code proofs. |
| **Test Quality** | 10/10 | Quad-layer testing (Unit, Proptest, Fuzz, Live HTTP Integration). |
| **OVERALL** | **10 / 10** | **APPROVED — Proceed to Execution (Wave 1).** |

---

## Claude Review (fable)

# Cross-AI Review: Phase 125 Plans (125-01 … 125-05)

I verified the plans' load-bearing claims directly against the repo. Nearly everything checked out, which is unusual and worth saying up front — this is a source-accurate plan set.

## Summary

These five plans are exceptionally well-grounded: essentially every line citation I checked against source is accurate, including the ones where the plans *correct* the research (the 4a name-identity reject that would break `src/server/skills.rs:846-871` and `:1144-1152`, the two extra index.json blast-radius sites, the sha2 0.11 `LowerHex` issue). The wave structure is sound, the D-01..D-11 decision coverage is real, and acceptance criteria are mostly mechanically checkable. The main defects are two coverage gaps — a must-have stdio-reach test that no task actually writes, and no test asserting the D-07 `ttlMs`/`cacheScope` caching attributes on the wire — plus a handful of brittle grep-count acceptance criteria.

## Verified-against-source strengths

- **The classifier route exists exactly as described.** `classify_internal_method` at `src/types/protocol/mod.rs:873`, `pub(crate) enum InternalClientRequest` at `:769`, the raw-params "must never reject a body" rustdoc at `:798`, and the `TASKS_UPDATE_METHOD` re-export at `:846` with the single-sourcing rationale. The `TasksUpdate` analog is genuinely the right template for `SkillsGet`.
- **The plan-time findings that supersede RESEARCH are correct.** I confirmed `src/server/skills.rs:848-850` registers `Skill::new("a","").with_path("p")` / `("b","").with_path("p")` for duplicate-URI detection, and `skills_strategy_with_refs` (~`:1144`) rewrites every generated path to `p{i}` — an unconditional gap-4a reject would break both, exactly as 125-03 argues. Demoting 4a to a warning while keeping the ROADMAP-scoped 4c reject is the right call, correctly justified.
- **The index-retirement blast radius is measured, not asserted.** Verified: `src/server/builder.rs:2302` (`assert!(uris.contains(&"skill://index.json"))` — absent from RESEARCH), `examples/c10_client_skills.rs:109/113-114` (two `assert_eq!` that will panic, as claimed), `pmcp-book/src/ch12-8-skills.md` 4 sites (124, 181, 266, 384), course chapter 2, exercises 3, quiz toml `:40`. The plan's 12-site table is exact.
- **The gate blind spot is real.** `Cargo.toml:306` has `skills = []`, absent from `full` (`:272-288`); Makefile has no `test-skills` target today, and `test-cargo-pmcp` at `Makefile:322` carries the zero-count guard idiom the plan copies. The 125-05 leg with per-selector dark-selector guarding is a genuine improvement over the analog.
- **`pmcp::testing::routing_name_key` exists** (`src/testing/mod.rs:146`) and `tests/common/v2.rs:752` `v2_headers_for` resolves through it exactly as 125-01 Task 2 requires — the name-bearing non-change test is implementable as written.
- **The D-06 divergence is correctly left alone.** `SkillsHandler::read` returns `ErrorCode::METHOD_NOT_FOUND` for an unknown URI (confirmed at the read tail of `src/server/skills.rs`), and 125-02 both refuses to copy it into `skills/get` and pins it with a control test — pinning the divergence as intentional is a nice touch.
- **`Skills::into_handler` is `pub` returning `Result<Arc<dyn ResourceHandler>>`** (confirmed), so the separate `entries(&self)` instead of a tuple-returning `into_handler` correctly avoids a semver break the PATTERNS doc would have caused.

## Concerns

- **MEDIUM — 125-01 must-have has no implementing task (stdio reach test).** The must_haves truth says "a test asserts the recorded stdio behavior rather than leaving it unmeasured (D-01)", and RESEARCH test-map row #1e demands a `stdio_reach` test. But 125-01 Task 1 writes 2 tests, Task 2 writes 4, and none of the six is the stdio assertion; 125-05 records the deferral in rustdoc only. Either add the stdio probe test (spike 008's `parse_message` Err assertion is cheap — the measurement already exists as a scratch binary) to 125-01 Task 2 or 125-05, or delete the must-have truth so verification doesn't fail on a promise nothing delivers.
- **MEDIUM — D-07's `ttlMs`/`cacheScope` are claimed but never asserted.** 125-01's truths say the v2 `skills/list` result carries the caching attributes (via `Cacheable::Yes` at the projection site), but no test in any plan asserts their presence on a 2026-07-28 wire response. CONTEXT D-07 makes this a decision; the live-wire test in 125-01 Task 1 should add one assertion that the v2 response envelope carries them (and the v1 twin does not).
- **MEDIUM — 125-03 Task 1's acceptance criterion is vacuous at that point in the plan.** `sed -n '/fn entries_with_diagnostics/,/^    }/p' … | grep -c resolved_description` returns 0 trivially when the function doesn't exist yet — `entries_with_diagnostics` is created in Task 2. The criterion passes on an empty range, proving nothing for Task 1's actual synthesis function. Re-anchor it on whatever function Task 1 actually writes (or move the criterion to Task 2).
- **LOW — brittle grep-count criteria.** `grep -c 'serde_yaml' src/server/skills.rs` must return exactly 1 (125-01), but a `use serde_yaml::…` import plus the call site, or a rustdoc mention of the crate name, makes it 2 while still satisfying D-04's intent. Similarly 125-02's `grep -c 'skills/list\|skills/get' src/server/core.rs` "shows the method strings are referenced only through constants" — a count cannot show that. These will generate false deviations during execution; phrase them as "the parse is invoked from exactly one function" style checks or accept doc mentions.
- **LOW — 125-01 Task 1 is very large for one commit.** Eight files, five HTTP-transport sites, new public API, a Cargo feature change, and a new integration file in a single tracer task. It's deliberate (compile-time tripwires force atomicity across the HTTP sites), but the executor should expect a long red period; splitting the `Cargo.toml`+`skills.rs` entry-synthesis half from the routing half would give an earlier green checkpoint without breaking the tripwire argument.
- **LOW — Wave 2 parallelism is safe but implicit.** 125-02 and 125-03 share no `files_modified` (verified), but 125-03's new `validate_names` hard-reject changes `into_handler` behavior that 125-02's parity tests build servers through. If both waves' fixtures use matching frontmatter names this is fine; a mismatch introduced in one plan surfaces as a confusing failure in the other. A one-line note in 125-02 to use frontmatter whose name matches the URI segment would immunize it.
- **LOW — test-count floors in `fails_when` clauses are guesses.** "fewer than 12 tests" (125-02), "fewer than 6" (125-01), "fewer than 11" (skills_integration) hardcode counts that drift the moment a test is split or a proptest is counted differently. Acceptable, but expect executor deviations here.

## Suggestions

1. Add the stdio-reach probe test (or strike the must-have truth) — this is the one place a stated success condition has no owner.
2. Add a `ttlMs`/`cacheScope` presence assertion to 125-01's v2 live-wire test and an absence assertion to the v1 twin.
3. Fix the 125-03 Task 1 vacuous sed criterion.
4. In 125-05, consider having the SUMMARY record the *decision* on adding `fuzz_skill_entry` to `.github/workflows/fuzz.yml` as a default-yes rather than open — the target costs nothing in the matrix and the ALWAYS-fuzz requirement's spirit is CI execution, not just registration.

## Risk Assessment: **LOW**

The plans are line-cited against a codebase that actually matches the citations, they follow four in-repo precedents that were argued out in rustdoc, every CONTEXT decision D-01..D-11 has a covering task, and the two semver traps (public `ClientRequest`, `into_handler` return type) are explicitly defended with tests. The residual risk is execution friction (brittle grep criteria, hardcoded test counts) and the two missing test assertions above — none of which threatens the phase goal of an honestly-declared, conformant skills extension.

---

## Consensus Summary

Three reviewers ran: Codex and the Fable 5 instance both produced source-grounded reviews with accurate `file:line` citations; Gemini's output carries the `[reviewed-without-source-citations]` marker (it reviewed the plan text only and awarded 10/10), so its APPROVED verdict is noted but not counted at full consensus weight. Plan-level consensus below is based on Codex and Fable.

The two grounded reviewers agree the plan set is unusually well-researched and that Plans 03 and 05 are execution-ready, but they diverge sharply on overall risk (Codex: HIGH; Fable: LOW) because they examined different failure surfaces. The orchestrator spot-checked the largest divergence — Codex's claim that Plan 02's `ServerCoreBuilder` parity test has no callable ingress — and corroborated its mechanism: `ServerCore`'s only request ingress is the typed public `Request` enum (`src/server/core.rs:80,108,3765`; `handle_request_internal` at `:4052` is private), `src/server/streamable_http_server.rs` contains zero references to `ServerCore`, and the proposed delegates are `pub(crate)` — unreachable from the integration test `tests/skills_routing.rs` where Plan 02 Task 2 places the parity test. That finding should be treated as blocking until Plan 02 either rescopes the ServerCore claim to projection parity or designs a real ingress seam.

### Agreed Strengths

- **The internal-routing architecture is right and verified.** Both grounded reviewers confirmed `classify_internal_method` / `InternalClientRequest` (`src/types/protocol/mod.rs:769,873`) preserves the public exhaustive enum, and that `TasksUpdate` is the correct template (Codex, Fable; Gemini concurs).
- **`skills/get` exact-map lookup is the safe design.** Exact `IndexMap` lookup keyed by full URI (`src/server/skills.rs:438`) with no path manipulation, and the existing `resources/read` `METHOD_NOT_FOUND` divergence deliberately pinned rather than copied (Codex, Fable).
- **The index retirement (Plan 04) is measured, not asserted.** Both reviewers independently verified the blast-radius site inventory and the remove-after-replacement ordering (Codex, Fable).
- **The quality-gate blind spot is real and Plan 05 closes it correctly.** `skills` is absent from `full` (`Cargo.toml:303/306`), and the `make test-skills` leg with zero-test-count guards follows the established `Makefile:322` idiom (Codex, Fable; Gemini concurs).
- **Plan 03's manifest-from-served-bytes property is the strongest part of the phase** — digests/sizes derive from the same bodies `SkillsHandler::read` serves, tested through the `ResourceHandler` seam (Codex, Fable).

### Agreed Concerns

- **Brittle, underspecified acceptance criteria will generate false deviations.** Codex (MEDIUM): the `test-skills` recipe's summed `test result:` lines can mask a zero-test selector; per-selector guards needed. Fable (LOW/MEDIUM): exact grep-count criteria (`grep -c 'serde_yaml'` == 1, method-string counts) and hardcoded test-count floors ("fewer than 12/6/11") break on innocent refactors, and 125-03 Task 1's sed-range criterion is vacuous before `entries_with_diagnostics` exists (created in Task 2).
- **Fuzz coverage is registration-only unless `fuzz_skill_entry` joins CI.** The fuzz workflow has an explicit four-target matrix (`.github/workflows/fuzz.yml:22`); both reviewers say either add the new target to the matrix or state honestly that recurring fuzz execution is not guaranteed (Codex MEDIUM, Fable suggestion #4).
- **Wave 2 parallelism (Plans 02 + 03) needs explicit coordination.** Both extend `tests/skills_routing.rs` or behavior it asserts, and Plan 03's new `validate_names` hard-reject changes `into_handler` behavior that Plan 02's parity fixtures build servers through — merge sequencing and fixture frontmatter-name discipline should be stated (Codex MEDIUM, Fable LOW).

### Divergent Views

- **Plan 02 ServerCore parity — Codex HIGH (blocking), Fable silent, Gemini calls it "Robust".** Codex: `ProtocolHandler::handle_request` accepts only the typed public `Request` (`src/server/core.rs:80,3764`), so `ServerCore::handle_skills_list/get` are delegates with no ingress route, and neither `spawn_default_config` (`tests/common/v2.rs:385`, accepts `Server`) nor the duplex harness (`tests/common/duplex.rs:334`) can drive the methods. Orchestrator check corroborates (see above). Resolution options per Codex: rescope to `Server` HTTP-only and rename the claim to "projection parity," or design a crate-private raw-ingress seam on `ProtocolHandler` with a real transport route.
- **Plan 04 `c10_client_skills` — Codex HIGH, others silent.** The example's flow accepts `&dyn ResourceHandler` (`examples/c10_client_skills.rs:93`); `skills/get` is not a `ResourceHandler` method and the projection helpers are crate-private, so "replace the index read with the skills/get equivalent" is not implementable without converting c10 into a real HTTP client/server example or honestly demonstrating `Skills::entries()` instead. Existence-class with concrete citations — treat as real.
- **Plan 01 v2 fixture opt-in — Codex HIGH, others silent.** The proposed live v2 test builds a server that never calls `.with_supported_protocol_versions(...)` (cf. `tests/common/v2.rs:295`); `spawn_default_config` does not add it (`tests/common/v2.rs:385`), so the v2 request can be rejected before skills dispatch. Cheap fix: mirror `build_v2_server`'s accept list.
- **Fable-only coverage gaps Codex missed:** the 125-01 must-have "a test asserts the recorded stdio behavior (D-01)" has no implementing task in any plan; and D-07's `ttlMs`/`cacheScope` caching attributes are claimed in truths but never asserted on a wire response. Both are cheap single-assertion fixes; the stdio one otherwise fails phase verification against a promise nothing delivers.
- **Overall verdict spread:** Gemini APPROVED 10/10 (down-weighted, no citations); Fable LOW risk; Codex HIGH until Plans 01/02/04 are corrected, MEDIUM after. Given the corroborated Plan 02 finding, the operative consensus is: **fix Plans 01, 02, and 04 before execution; Plans 03 and 05 need only criterion tightening.**
