---
phase: 82
reviewers: [codex, gemini]
reviewed_at: 2026-05-18T00:50:24Z
plans_reviewed:
  - 82-01-PLAN.md
  - 82-02-PLAN.md
  - 82-03-PLAN.md
---

# Cross-AI Plan Review — Phase 82: builder-dx-prerequisites

Two independent AI reviewers (Codex `gpt-5.5`, Gemini `2.5-pro`-class) were given the full
Phase 82 artifact set — PROJECT.md context, ROADMAP §Phase 82, REQUIREMENTS BLDR block,
CONTEXT.md (all locked decisions D-01..D-08), and all three PLAN.md files — and asked to
find blind spots the internal plan-checker may have missed.

## Codex Review

**Summary**

The plan set is mostly well-scoped and catches several important donor-vs-recipient mismatches, especially public `ServerBuilder` lacking `tool_infos` / `prompt_infos` and public `tool_authorizer()` having `tool_protections` clearing semantics. The main risks are not conceptual; they are execution hazards: a few acceptance checks contradict the tasks, some tests assert invariants that are not publicly observable, and the one non-mechanical behavior change is only grep-verified rather than behavior-tested.

**Strengths**

- Correctly keeps Phase 82 additive: no public signature changes to existing builder methods.
- Good call to lift all six `_arc` methods now instead of leaving another symmetry gap for Phase 83.
- Correctly identifies that public `tool_arc` / `prompt_arc` must not copy private `tool_infos` / `prompt_infos` writes, because public `ServerBuilder` builds those at `build()` time.
- Correctly catches the special `tool_authorizer_arc` case: public builder must mirror `tool_authorizer()` clearing `tool_protections`.
- The D-03 callout is concrete and names the exact skipped pipeline pieces.
- Wave dependencies are mostly clean: 82-02 and 82-03 both depend on 82-01 and do not modify the same files.

**Concerns**

- **HIGH — Plan 82-02 Task 1 has a self-failing acceptance check.**
  The action requires a maintainer comment explaining why the test does not import `Server::handle_request`, but acceptance says `grep -q 'handle_request' tests/in_process_handler_pattern.rs` must fail. Those cannot both be true. Either remove the literal from the comment or narrow the negative grep to actual calls/imports, e.g. `\.handle_request\(`.

- **MEDIUM — Plan 82-01 does not behaviorally test the highest-risk recipient-specific semantic.**
  `tool_authorizer_arc` is explicitly not a verbatim donor copy because public `ServerBuilder::tool_authorizer()` clears `tool_protections` in [src/server/mod.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs:2879). The plan only source-greps for `tracing::warn!` and `self.tool_protections.clear()`. Add a unit test in `src/server/mod.rs` that configures `protect_tool(...)`, calls `tool_authorizer_arc(...)`, and proves build behavior no longer rejects mixed `protect_tool` + custom authorizer.

- **MEDIUM — Plan 82-02 Task 2 asks for an invariant that public tests cannot observe.**
  The behavior says the property test verifies `capabilities.tools`, but public `Server` has no capabilities accessor; `capabilities` is private in [src/server/mod.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs:317). The plan later says to omit if inaccessible, but `must_haves` still requires it. This should be removed from must-haves or moved to a crate-internal unit test.

- **MEDIUM — Doctest acceptance checks inspect the wrong text range.**
  Plan 82-01 Task 4 uses `awk '/pub fn get_tool/,/pub fn /{print}'`, which starts at the function line and cannot see the rustdoc preceding it. It will not verify the D-03 callout. Use `rg -B` around `pub fn get_tool` / `pub fn get_prompt`, or a small script that captures contiguous `///` lines immediately above each method.

- **MEDIUM — The doctest examples need explicit async wrapping.**
  The plan says the doctests call `.await`, but does not require the standard hidden wrapper shape used elsewhere in the repo: `# async fn example() -> pmcp::Result<()> { ... # Ok(()) # }`. Without that, executable `rust` doctests will fail at top-level `.await`.

- **MEDIUM — Plan 82-03 Task 3 section extraction command is broken.**
  `awk '/^## Handler-Level Testing Pattern \(In-Process\)/,/^## /'` starts and ends on the same heading line, so it will usually print only the heading and miss the section body. Use a stateful awk range that exits on the next `##` after the heading.

- **MEDIUM — Minor version bump is in success criteria but not implemented.**
  Roadmap success criterion 4 says the methods "ship in a minor `pmcp` version bump," but no plan modifies `Cargo.toml`, changelog, release notes, or explicitly defers release-versioning. Either add a small release-metadata task or mark the version bump as handled by a later release phase.

- **LOW — Trait reference ambiguity could mislead implementation.**
  Plan 82-02 references `src/server/traits.rs` for `ToolHandler`, but that file's `ToolHandler` uses `call_tool`, not the public `pmcp::ToolHandler` with `handle` from [src/server/mod.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs:224). The task imports root `pmcp::ToolHandler`, which is correct; tighten the context to avoid the wrong trait.

**Suggestions**

- In Plan 82-02, change the negative check to reject actual private dispatch usage, not prose: `rg '\.handle_request\(|use .*handle_request'`.
- Add crate-internal tests in `src/server/mod.rs` for capability flips and `tool_authorizer_arc` clearing; keep integration tests focused on public handler retrieval and invocation.
- Remove `capabilities.tools` from Plan 82-02 must-haves unless tested internally.
- Replace brittle line-number and `awk` assertions with semantic `rg` or small scripts.
- Make the book example real Rust, not commented pseudocode, and verify the streamable HTTP client API name before documenting `pmcp::Client::connect_streamable_http(...)`.

**Risk Assessment**

Overall risk: **MEDIUM**. The implementation itself is small and mostly mechanical, but the plans currently have enough verification gaps and contradictory assertions that an executor could either fail the plan checks or pass checks while missing the most important semantic regression. Fixing the acceptance checks and adding one or two focused unit tests would bring this close to low risk.

---

## Gemini Review

The implementation plans for Phase 82 are exceptionally well-structured and demonstrate a deep understanding of the codebase's internal architecture, specifically the divergence between the private `ServerCoreBuilder` and the public `ServerBuilder`.

### Summary
This plan set provides a surgical and robust implementation of the "Configuration-Only MCP Servers" prerequisites. It successfully bridges the gap between private registration logic and public API surface while maintaining a zero-risk posture for existing users through strictly additive changes. The inclusion of a multi-layered testing strategy (doctest, integration, and property) ensures that the new DX patterns are not only functional but also stable and well-documented for downstream toolkit authors.

### Strengths
*   **Deep Context Awareness:** The plans correctly identify that the public `ServerBuilder` does not maintain a `tool_infos` map (building it at `.build()` time instead) and correctly adjusts the lifted bodies to avoid compilation errors.
*   **Behavioral Parity & Correctness:** Task 3 in Plan 01 correctly identifies that `tool_authorizer_arc` must mirror the public `tool_authorizer`'s unique logic (clearing `tool_protections` and warning), which is absent in the private donor. This prevents a critical semantic mismatch.
*   **Callout Rigor:** The plans strictly enforce the D-03 literal callout requirements (`auth_provider`, `tool_authorizer`, etc.) in both the doctests and the book section, ensuring security-conscious documentation.
*   **Equivalence Anchor:** The property test in Plan 02 (Task 2) is an excellent addition that ensures the `tool(impl)` and `tool_arc(Arc)` registration paths never drift in behavior.
*   **Safe Defaults:** The `sampling_arc` lift correctly adopts the `if is_none` pattern to avoid clobbering manual capability overrides, preserving the builder's flexibility.

### Concerns

*   **Contradictory Acceptance Criteria (LOW)**
    *   **Reference:** Plan 82-01, Task 2, Acceptance Criteria item 3.
    *   **Issue:** The text says "no `name.into()` capturing into a local first because the donor pattern uses `let name = name.into();`". This is contradictory. If the donor uses `let name = name.into();`, it *is* capturing into a local.
    *   **Impact:** The agent might be confused about whether to use a local or not. Given `name` is an `impl Into<String>`, it must be converted eventually.

*   **Proptest Availability (LOW)**
    *   **Reference:** Plan 82-02, Task 2.
    *   **Issue:** The plan includes a fallback for the property test if `proptest` is not a dev-dependency. While safe, checking `Cargo.toml` early would avoid speculative task logic.
    *   **Impact:** Minor turn efficiency loss if the fallback is triggered.

### Suggestions

*   **Clarify `name.into()` Local (Plan 82-01, Task 2):**
    Ensure the code matches the donor's structure exactly. Since the donor in `builder.rs:204` uses `let name = name.into();`, the recipient should do the same to satisfy clippy's "into on name" preferences.
*   **Verify `Role` Import Path (Plan 82-02, Task 1):**
    Most protocol types are re-exported in `pmcp::types`, but `Role` is occasionally nested. Confirm `pmcp::types::Role` is correct or use the specific submodule path to avoid an "unresolved import" failure in the new integration test.
*   **Book Cross-Linking (Plan 82-03, Task 3):**
    In the new book section, consider adding an explicit link to the "Long-Running Operations (Tasks)" chapter, as toolkit authors using the `_arc` pattern are often building task-heavy servers.

### Risk Assessment: LOW
The risk is low because the changes are 100% additive. No existing method signatures are modified, and the code being lifted is already battle-tested in the private builder. The high volume of automated verification (check, clippy, test-doc, integration-test) ensures that any mechanical errors during the copy-paste lift will be caught immediately before the phase closes.

---

## Consensus Summary

Both reviewers agree the design is sound (additive-only, donor/recipient divergence correctly identified, multi-layered testing). They diverge sharply on the **execution risk** of the verification harness — Codex flags concrete acceptance-criterion contradictions that could either block the executor or pass false-positive; Gemini sees the additive surface as a low-risk safety net. Treat Codex's HIGH/MEDIUM findings as the action list before execution.

### Agreed Strengths

Concurred by both reviewers:

- **Donor-vs-recipient field divergence correctly caught** — public `ServerBuilder` has no `tool_infos`/`prompt_infos` maps (synthesized at `.build()` time); plans drop those donor lines.
- **`tool_authorizer_arc` semantic-mirror correctly caught** — must mirror the public `tool_authorizer()`'s `tool_protections.clear()` + `tracing::warn!`, not the private donor.
- **D-03 callout enforced concretely** — literal tokens `auth_provider`, `tool_authorizer`, `tool_middleware`, `pmcp::Client`, `stdio` are grep-counted in doctest and book.
- **Pure additive posture** — no existing public method signature changes anywhere; existing `tool()` / `prompt()` / `resources()` / `sampling()` / `auth_provider()` / `tool_authorizer()` bodies remain byte-for-byte unchanged.
- **Wave dependencies are clean** — 82-02 and 82-03 in parallel Wave 2 with disjoint `files_modified` sets.

### Agreed Concerns

No concern was raised by both reviewers — they did not overlap. Each reviewer caught a distinct set:
- Codex caught **assertion correctness** issues (the acceptance harness will misfire or be unfalsifiable).
- Gemini caught **task-text contradictions** (acceptance text contradicts itself; executor will be confused).

The union of both lists is the actionable backlog below.

### Highest-Priority Action Items (consolidated, sorted by severity)

1. **HIGH — Plan 82-02 Task 1: fix self-failing `handle_request` grep** (Codex). The negative grep `grep -q 'handle_request' ...` must FAIL contradicts a task action requiring a comment that names `handle_request`. Narrow the assertion to actual usage: `rg '\.handle_request\(|use .*handle_request'`.

2. **MEDIUM — Plan 82-01 Task 3: behaviorally test `tool_authorizer_arc` clearing semantics** (Codex). The only non-mechanical lift currently has source-grep coverage but no behavior assertion. Add a crate-internal unit test in `src/server/mod.rs` that builds a server with `protect_tool(...)` then `tool_authorizer_arc(...)` and proves no rejection occurs.

3. **MEDIUM — Plan 82-02 Task 2: drop `capabilities.tools` from `must_haves`** (Codex). `capabilities` is private on `Server`; the property test cannot observe it from outside the crate. Either move the assertion to a crate-internal unit test or remove the must-have.

4. **MEDIUM — Plan 82-01 Task 4: fix `awk` range that misses preceding rustdoc** (Codex). `awk '/pub fn get_tool/,/pub fn /{print}'` starts on the function line and can't see the `///` callout above. Replace with `rg -B 30 'pub fn get_tool'` or a stateful awk that walks backward over contiguous `///` lines.

5. **MEDIUM — Plan 82-01 Task 4: require hidden async wrapper in doctests** (Codex). Without `# async fn example() -> pmcp::Result<()> { ... # Ok(()) # }`, top-level `.await` will fail to compile in `rust` doctests. Add to the action explicitly.

6. **MEDIUM — Plan 82-03 Task 3: fix broken `awk` for book section extraction** (Codex). `awk '/^## H...Pattern \(In-Process\)/,/^## /'` matches start and end on the same heading line. Use a stateful walk that flips off on the *next* `##`.

7. **MEDIUM — Roadmap success criterion 4 (minor version bump) is unowned** (Codex). No plan touches `Cargo.toml` / `CHANGELOG.md` / release notes. Either add a small release-metadata task to 82-03, or explicitly defer to a release-only phase and amend ROADMAP §Phase 82 to mark the version-bump bullet as released-elsewhere.

8. **LOW — Plan 82-02 Task 1: confirm `pmcp::types::Role` import path** (Gemini). Verify the re-export shape; some types are nested under submodules. A wrong import will surface as "unresolved import" at first `cargo check`.

9. **LOW — Plan 82-02 Task 1: tighten `ToolHandler` trait reference** (Codex). The `src/server/traits.rs` `ToolHandler` (uses `call_tool`) is the wrong trait. Plans already import the root `pmcp::ToolHandler` (uses `handle`) — just remove the misleading reference from the task `<read_first>`.

10. **LOW — Plan 82-01 Task 2: resolve `name.into()` contradiction** (Gemini). The acceptance text contains a self-contradiction about whether the lifted body should `let name = name.into();`. Mirror the donor: yes, do the local-binding.

11. **LOW — Plan 82-02 Task 2: pre-check `proptest` availability** (Gemini). The dev-dep is in fact present at root `Cargo.toml:131`; this can be hard-stated as a precondition rather than a runtime fallback to remove ambiguity.

12. **LOW — Plan 82-03 Task 3: cross-link to "Long-Running Operations (Tasks)" chapter in the new book section** (Gemini). Toolkit authors using `_arc` often also use tasks — small DX polish.

### Divergent Views

| Topic | Codex | Gemini |
|---|---|---|
| Overall risk | **MEDIUM** — verification harness has self-failing checks and unobservable invariants | **LOW** — additive surface + multi-layered tests make mechanical errors trivially catchable |
| `capabilities.tools` in must_haves | Concrete blocker — private field, can't be tested publicly | Not raised |
| `awk` extraction commands | Two cases will produce no/wrong output | Not raised |
| Doctest async wrapping | Will fail to compile without `# async fn` wrapper | Not raised — assumed correct |
| Version-bump ownership | Success criterion has no owning task | Not raised |
| `name.into()` local binding | Not raised | Acceptance text is internally contradictory |
| `Role` import path | Not raised | Worth pre-verifying |
| Book cross-linking to Tasks chapter | Not raised | Worth adding |

Codex spent its budget on the verification harness; Gemini spent its on the prose. Both lenses are useful — the union covers more ground than either alone.

---

## Recommended Next Step

The findings are concrete and actionable. Run:

```
/gsd:plan-phase 82 --reviews
```

to replan incorporating this feedback. The replan should:
- Treat HIGH and MEDIUM items as mandatory revisions to the plans (not suggestions).
- Treat LOW items as polish — apply if cheap, defer if they balloon scope.
- Specifically fix items #1, #3, #4, #5, #6 (verification harness correctness — these are the path to making the plan-checker's PASS verdict actually meaningful).
- Re-run the plan-checker after revision to confirm the harness now does what it claims.
