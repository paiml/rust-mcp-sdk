---
phase: 80
reviewers: [gemini, codex]
reviewed_at: 2026-05-12
plans_reviewed: [80-01-PLAN.md, 80-02-PLAN.md, 80-03-PLAN.md]
overall_verdict: REPLAN-REQUIRED
---

# Cross-AI Plan Review — Phase 80: SEP-2640 Skills Support

Two independent reviewers (Gemini 0.38.2, Codex 0.129.0) reviewed all three plans (~2050 lines) plus CONTEXT.md and the spike-findings skill. Gemini approved as LOW risk. Codex flagged **4 HIGH-severity issues** and recommends replanning before execution.

## Gemini Review

# Plan Review: Phase 80 — SEP-2640 Skills Support

## 1. Summary
The implementation plans for Phase 80 are of exceptional quality, demonstrating a deep understanding of both the SEP-2640 experimental specification and the existing PMCP architectural patterns. The strategy of bridging the "protocol gap" first (Plan 80-01), followed by a lift-and-shift of a validated spike (Plan 80-02), and concluding with ergonomic validation via examples and integration tests (Plan 80-03) is sound. The plans specifically address the most critical risks of the Skills extension: ensuring a byte-equal dual-surface (Skill + Prompt) to prevent host-specific context drift. The attention to detail regarding builder ordering invariance and the "no-silent-overwrite" rule for URIs demonstrates professional-grade SDK design.

## 2. Strengths
*   **Dual-Surface Invariant Enforcement:** The plan asserts the byte-equality of the SKILL surface and PROMPT surface at four distinct layers (data construction, handler response, builder registration, and wire-level integration). This is a robust defense against the "silent failure" mode identified in Spike 002.
*   **Builder Ordering Invariance:** The modification of `ServerCoreBuilder::resources` (80-02 Task 2) to detect and compose with pre-existing skills-handlers is a sophisticated DX touch. It ensures that `.skills().resources()` and `.resources().skills()` yield identical, functional behavior, preventing common user ordering bugs.
*   **Strict SEP-2640 Compliance:** The plans correctly implement §9 of the SEP, ensuring that supporting reference files are addressable by URI but explicitly excluded from discovery listing (`resources/list`) and the `index.json` enumeration.
*   **Proactive Conflict Resolution:** The planner correctly identified an example filename collision (`s38`) and autonomously proposed a non-conflicting sequence (`s44`/`c10`), maintaining repository hygiene.
*   **Zero-Overhead Protocol Change:** The addition of the `extensions` field to `ServerCapabilities` (80-01) is implemented as a non-breaking, `Option`-wrapped change that results in zero byte-delta for existing servers when not in use.

## 3. Concerns
*   **MEDIUM: Nested Composition Performance:** In `ServerCoreBuilder::skills` and the modified `resources` method (80-02 Task 2), multiple calls to register resources or skills will result in a "Matryoshka doll" of nested `ComposedResources` wrappers. While functional, this increases the call stack for every resource read. 
*   **LOW: Frontmatter Parsing Robustness:** The `parse_frontmatter_description` helper (80-02 Task 1) is a lightweight string-searcher rather than a full YAML parser. While this keeps dependencies low and cognitive complexity under the cap, it may fail on valid-but-unusual YAML (e.g., descriptions containing colons or multi-line scalars).
*   **LOW: Panic in Builder Convention:** While consistent with PMCP's internal convention, the use of `.expect()` in `.skills()` (80-02 Task 2) to handle duplicate URIs is a "fail-fast" approach. If a server is dynamically registering skills at runtime based on external input (rare but possible), this could lead to unhandled crashes.

## 4. Suggestions
*   **Flatten Composition:** In `ServerCoreBuilder::skills` (80-02 Task 2), consider adding logic to detect if `self.resources` is already a `ComposedResources`. If so, instead of wrapping it again, add the new skill handler to the internal `skills` slot (or a list of handlers) to keep the dispatch tree shallow.
*   **Explicit Newline Normalization:** In `Skill::as_prompt_text` (80-02 Task 1), ensure that the concatenation logic is resilient to mixed line endings (`
` vs `
`) if the SKILL.md files are authored on different operating systems, as this could defeat the byte-equality assertion in Plan 80-03.
*   **Document Discovery Rules:** In the `examples/s44_server_skills.rs` (80-03 Task 1), add a comment explaining *why* the references don't appear in the printed URI list (referencing SEP-2640 §9), as this "readable but not listable" behavior is often counter-intuitive to new MCP developers.

## 5. Risk Assessment: LOW
The overall risk is **LOW**. The feature is additive and strictly feature-gated, ensuring no impact on the stable core of the SDK. The use of validated spike code significantly reduces implementation uncertainty. The "Dual-Surface" pattern acts as a natural safeguard: even if a host fails to implement the experimental SEP-2640 correctly, the PMCP server remains functional via the legacy Prompt fallback. The most complex logic (URI routing and concatenation) is heavily covered by unit, property, and integration tests.

***

**Verdict:** The plans are **APPROVED**. Proceed with execution starting from Plan 80-01.

---

## Codex Review

**Summary**
The plans are unusually thorough on requirements traceability and test intent, but I would not execute them as-is. The largest missed issue is that the proposed composition model breaks repeated `.skill(...)` calls and likely the planned server example itself. There are also mismatches between `ServerCoreBuilder` and the public `Server::builder()` API, plus a wire-shape concern around returning `Content::Text` instead of resource contents with URI/MIME.

**Strengths**
- Strong phase decomposition: 80-01 protocol foundation, 80-02 DX/builders, 80-03 examples/tests is coherent.
- The dual-surface invariant is correctly treated as load-bearing and gets unit, builder, integration, and property-test coverage.
- `extensions` is added as an additive sibling to `experimental`, with explicit merge behavior planned.
- Reference enumeration is correctly treated as forbidden for `resources/list` and index discovery.
- The s38/c38 naming collision is surfaced in [80-03-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-03-PLAN.md:86), which is the right kind of plan-time deviation disclosure.

**Concerns**
- HIGH: Repeated `.skill(...)` calls are broken by the composition design. `.skill` delegates to `.skills`, and `.skills` wraps the prior resource handler as `other` ([80-02-PLAN.md:690](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-02-PLAN.md:690)). But `ComposedResources::read` sends every `skill://` URI only to `self.skills` ([80-02-PLAN.md:535](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-02-PLAN.md:535)). The example chains `.skill(...).skill(...).bootstrap_skill_and_prompt(...)` ([80-03-PLAN.md:306](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-03-PLAN.md:306)), so older skills may list but fail to read. Duplicate URI detection also fails across separate `.skill(...)` calls.
- HIGH: The plan wires `ServerCoreBuilder`, but the examples use `pmcp::Server::builder()` ([80-03-PLAN.md:306](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-03-PLAN.md:306)). In the current repo, `Server::builder()` returns `ServerBuilder`, not `ServerCoreBuilder` ([src/server/mod.rs:618](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs:618)). Unless both builders get skill APIs, the examples will not compile.
- HIGH: `ReadResourceResult` should probably return `Content::Resource { uri, text, mime_type }`, not `Content::Text`. The plan’s handler returns `Content::text(...)` for `resources/read` ([80-02-PLAN.md:467](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-02-PLAN.md:467)), but PMCP’s own docs say read contents are ResourceContents with `uri`, optional `mimeType`, and `text/blob` ([src/types/resources.rs:351](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/resources.rs:351)). This risks SEP wire incompatibility and drops reference MIME types.
- HIGH: The `.resources(...)` ordering fix changes semantics under `--features skills` for all repeated resource registration, not only skills. The proposed implementation composes whenever any handler already exists ([80-02-PLAN.md:743](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-02-PLAN.md:743)). Existing `.resources(A).resources(B)` currently means “B replaces A”; with skills enabled it becomes “A handles all `skill://`, B handles everything else,” even if A is unrelated to skills.
- MEDIUM: The “wire-level” dual-surface test is optional ([80-03-PLAN.md:593](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-03-PLAN.md:593)), and the client example uses `skill.as_prompt_text()` directly rather than `prompts/get` or a prompt handler ([80-03-PLAN.md:506](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-03-PLAN.md:506)). That can miss bugs in builder prompt registration.
- MEDIUM: `Skills::into_handler()` rejects duplicate SKILL.md URIs but silently overwrites duplicate reference URIs ([80-02-PLAN.md:393](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-02-PLAN.md:393)). It also allows references such as `SKILL.md`, empty paths, absolute-looking paths, or `../` segments. There is no filesystem access, but the URI model becomes confusing and can create unreachable resources.
- MEDIUM: The module gate is only `#[cfg(feature = "skills")]` in 80-01 ([80-01-PLAN.md:23](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-01-PLAN.md:23)), while the handler traits are native-only. Use `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` unless WASM support is deliberately implemented.
- LOW: HashMap storage loses skill registration order for `resources/list` and `skill://index.json` ([80-02-PLAN.md:418](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-02-PLAN.md:418)). This can make examples, snapshots, and host UX nondeterministic.
- LOW: The frontmatter parser is intentionally small but brittle: exact `description: ` only, no trimming around `---`, no BOM handling ([80-02-PLAN.md:545](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/80-sep-2640-skills-support/80-02-PLAN.md:545)). It is acceptable for v1 if documented.
- LOW: Keeping `skills` out of `default` is right for an experimental SEP. Keeping it out of `full` is defensible but surprising because the current `full` feature reads as “all optional features” ([Cargo.toml:153](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml:153)).

**Suggestions**
- Replace composition-by-wrapping for skills with explicit builder state: store pending `Skills` in the builder, merge `.skill(...)` calls into one registry, detect duplicates globally, and compose exactly once at `build()` or first `.skills(...)`.
- Add skill APIs to both `ServerCoreBuilder` and `ServerBuilder`, or change all examples/tests to use only the lower-level builder. Prefer both, since public docs use `Server::builder()`.
- Make `ComposedResources::read` either fallback on `METHOD_NOT_FOUND` for `skill://` misses or avoid nested skills handlers entirely. The cleaner fix is a single merged `SkillsHandler`.
- Return `Content::resource_with_text(uri, body, mime_type)` from `resources/read` and update tests to assert URI and MIME.
- Make `DUAL_SURFACE_VIA_FULL_BUILDER` non-optional. The current `Server` has `get_prompt`, so the public-builder path is testable.
- Add `try_skills(...) -> Result<Self>` and let `.skills(...)` remain a panic-on-invalid convenience if you want fluent parity. This avoids forcing production users into panic behavior.
- Validate `SkillReference::relative_path`: non-empty, relative, no `..`, no leading slash, no scheme, no `SKILL.md` collision; reject duplicate reference URIs.
- Use `Vec<Skill>` or `IndexMap`-like ordering without adding deps: keep the original `Vec` for list/index order and a `HashMap` only for lookup.
- Add one test for CRLF and BOM content so the byte-equality contract is explicit rather than implied by shared helper logic.

**Risk Assessment**
Overall risk: HIGH until the builder/resource composition model is fixed. The protocol field addition itself is low risk, and the test strategy is strong, but the planned `.skill(...).skill(...)` flow appears structurally broken, and the public examples target a different builder than the one being modified. After addressing those two points and the `Content::Resource` wire-shape issue, I would downgrade this to MEDIUM/LOW for an experimental feature.

---

## Consensus Summary

### Agreed Strengths (both reviewers)

- **Phase decomposition is sound.** 80-01 (protocol foundation) → 80-02 (DX/builders) → 80-03 (examples/tests) is the right shape and the wave dependency chain is coherent.
- **Dual-surface invariant is correctly treated as load-bearing** and validated at multiple test layers.
- **`extensions` field addition** is non-breaking, `Option`-wrapped, and additive — zero byte-delta for existing servers.
- **Reference enumeration correctly excluded** from `resources/list` and `skill://index.json` per SEP-2640 §9.
- **s38/c38 naming collision proactively surfaced** in the plan rather than left as a build-time surprise.

### Agreed Concerns (raised by both reviewers)

| # | Severity | Concern | Reviewer Citations |
|---|---|---|---|
| C1 | **HIGH (Codex) / MEDIUM (Gemini)** | **Composition model breaks repeated `.skill()` calls and creates "Matryoshka" nested handlers.** `.skill` delegates to `.skills` which wraps the prior handler as `other`; the second `.skill()` call wraps the first SkillsHandler inside `other`, but `ComposedResources::read` always routes `skill://` URIs to `self.skills` only — so the older skills become unreachable. The s44 example chains three skill calls (`.skill(hello).skill(refunds).bootstrap_skill_and_prompt(code_mode, ...)`) and would silently fail. Duplicate-URI detection also fails across separate `.skill()` calls because each call creates a fresh `Skills` registry. | Codex: 80-02:535, 80-02:690, 80-03:306 / Gemini: 80-02 Task 2 |
| C2 | LOW (Codex) / LOW (Gemini) | **Frontmatter parser brittle on edge cases** — exact `description: ` prefix only, no BOM handling, no multi-line scalars, no escaped colons. Acceptable for v1 if documented. | Codex: 80-02:545 / Gemini: 80-02 Task 1 |

### Codex-Only HIGH Concerns (not surfaced by Gemini or the internal plan-checker)

| # | Concern | Citation |
|---|---|---|
| C3 | **Builder type mismatch.** Plans wire `ServerCoreBuilder`, but examples call `pmcp::Server::builder()` which returns `ServerBuilder` (different type) per `src/server/mod.rs:618`. Examples will not compile unless both builders get the new skill APIs, or examples switch to the lower-level builder. | 80-03:306, src/server/mod.rs:618 |
| C4 | **Wire-shape regression.** The `SkillsHandler::read` returns `Content::Text` but PMCP's own `ReadResourceResult` contract (`src/types/resources.rs:351`) says contents are `ResourceContents` with `uri`, optional `mimeType`, and `text`/`blob`. Returning bare `Content::Text` drops the per-resource `mimeType` (which matters for the GraphQL reference) and may be SEP-wire-incompatible. The spike binary may have accidentally validated against a non-strict deserializer. | 80-02:467, src/types/resources.rs:351 |
| C5 | **`.resources(...)` semantics change affects unrelated callers.** The proposed ordering fix composes whenever any prior handler exists `#[cfg(feature = "skills")]`. But existing `.resources(A).resources(B)` callers (with no skills involved) get behavior changed from "B replaces A" to "A handles skill://, B handles everything else" — even if A is unrelated to skills. This is a hidden behavior change for any user who enables the `skills` feature, not just users who actually register skills. | 80-02:743 |

### Codex-Only MEDIUM Concerns

| # | Concern | Citation |
|---|---|---|
| C6 | Wire-level dual-surface test (DUAL_SURFACE_VIA_FULL_BUILDER) is marked optional. The c10 client example uses `skill.as_prompt_text()` directly instead of going through `prompts/get` — can miss bugs in the builder's prompt registration path. `Server` already has `get_prompt` so the public-builder path IS testable. | 80-03:506, 80-03:593 |
| C7 | `Skills::into_handler()` rejects duplicate SKILL.md URIs but **silently overwrites duplicate reference URIs**. Also allows references like `SKILL.md`, empty paths, absolute-looking paths, `../` segments. No filesystem access risk but creates unreachable/confusing URIs. | 80-02:393 |
| C8 | Module gate is only `#[cfg(feature = "skills")]` in 80-01 but `ResourceHandler` is non-wasm-only. Should be `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]` unless WASM is deliberately supported. | 80-01:23 |

### Gemini-Only Concerns

| # | Severity | Concern |
|---|---|---|
| G1 | MEDIUM | **Nested ComposedResources performance.** Multiple skill/resource registrations create a deep wrapper tree, increasing call-stack depth for every `resources/read`. (Note: this is the symptom; Codex's C1 is the underlying cause.) |
| G2 | LOW | `.skills(...)` uses `.expect()` on duplicate-URI failure rather than returning `Result`. Acceptable for static registration but a crash vector for runtime dynamic registration. |

### Codex-Only LOW Concerns

| # | Concern |
|---|---|
| C9 | HashMap storage loses skill registration order for `resources/list` and `skill://index.json` — nondeterministic output across runs. Use `Vec<Skill>` for ordering + `HashMap` for lookup. |
| C10 | `skills` feature is not in `default` (correct) but also not in `full` (defensible but surprising since `full` reads as "all optional features"). |

### Divergent Views

The biggest divergence is **overall risk assessment**: Gemini says **LOW** (approve, proceed); Codex says **HIGH** until the composition model and wire-shape issues are fixed. The divergence reflects depth of code-path tracing — Gemini approved the structural design at the abstraction level; Codex traced the actual code paths and found the routing flaw in repeated `.skill()` calls.

The plan-checker also missed all four of Codex's HIGH-severity issues, including the structural composition flaw. This is exactly what adversarial cross-AI review is for.

### Recommended Action

**REPLAN required before execution.** Specific surgical changes to fold into a revised plan via `/gsd-plan-phase 80 --reviews`:

1. **Fix the composition model (C1, G1).** Replace builder-level wrapping with a single merged `SkillsHandler` that accumulates across `.skill()` / `.skills()` calls. Detect duplicate URIs globally at `.build()` time, not per-call. One `ComposedResources` wrapper at most per server.
2. **Reconcile builder targeting (C3).** Add the skill API to both `ServerCoreBuilder` and `ServerBuilder`, OR change examples to use the lower-level builder. Codex recommends both; public docs and existing examples use `Server::builder()`.
3. **Fix the wire shape (C4).** `SkillsHandler::read` returns `Content::resource_with_text(uri, body, mime_type)` so each reference's MIME type survives the wire round-trip. Update tests to assert URI and MIME.
4. **Narrow the `.resources(...)` ordering fix (C5).** Only compose when the new handler being added is the SkillsHandler, OR when the existing handler is the SkillsHandler — not for arbitrary repeated `.resources()` calls. Add explicit test that confirms `.resources(A).resources(B)` still means "B replaces A" when neither A nor B is a SkillsHandler.
5. **Promote DUAL_SURFACE_VIA_FULL_BUILDER from optional to mandatory (C6).** Go through the real `prompts/get` round-trip in the integration test, not direct `as_prompt_text()` calls.
6. **Validate reference paths (C7).** Reject empty, absolute, `../`-containing, `SKILL.md`-colliding, and duplicate reference URIs in `Skill::with_reference` or at `into_handler` time.
7. **Add CRLF/BOM test (Gemini suggestion).** Make the byte-equality contract explicit against mixed line endings — the SKILL.md authored on Windows vs Linux scenario should not defeat the dual-surface invariant.
8. **Add `try_skills(...) -> Result<Self>` builder method (Codex suggestion).** Keep `.skills(...)` as panic-convenience; offer `.try_skills(...)` for runtime-dynamic registration.
9. **Switch `HashMap<String, Skill>` to ordered storage (C9).** `Vec<Skill>` for list order + `HashMap<String, usize>` for lookup, or use `IndexMap` (already in PMCP dep graph per CLAUDE.md memory).
10. **Tighten the wasm cfg gate (C8).** `#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]`.

After replan, re-spawn the plan-checker to verify the fixes don't introduce new issues, then re-run cross-AI review.
