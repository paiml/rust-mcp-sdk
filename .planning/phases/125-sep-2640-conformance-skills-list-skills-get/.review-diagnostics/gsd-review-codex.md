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
