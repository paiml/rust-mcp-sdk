---
phase: 126
reviewers: [codex, gemini]
reviewer_instances: [fable]
reviewed_at: 2026-09-03T16:25:47Z
plans_reviewed:
  - 126-01-PLAN.md
  - 126-02-PLAN.md
  - 126-03-PLAN.md
  - 126-04-PLAN.md
  - 126-05-PLAN.md
  - 126-06-PLAN.md
  - 126-07-PLAN.md
models:
  codex: "gpt-5.6-sol (reasoning=low)"
  gemini: "unknown"
  fable: "claude-fable-5-1 (reasoning=low)"
model_sources:
  codex: "banner"
  gemini: "unknown"
  fable: "pinned"
---

# Cross-AI Plan Review — Phase 126

## Codex Review

# Summary

The seven-plan decomposition is strong and mostly faithful to SC-1..SC-6 and D-01..D-16a. The wave graph is coherent, test reachability is handled unusually well, and the plans correctly reuse Phase 125’s registry, digest, and URI machinery. However, three issues should be resolved before execution: unsafe YAML frontmatter rendering can invalidate the claimed SEP-2640 conformance; D-07 and D-10 are incompatible as currently planned, so SC-6 warnings do not actually have two delivery channels; and the proposed fuzz-input splitting can itself panic on UTF-8 boundaries. Overall risk is **MEDIUM**, rising to **HIGH** if implementation begins without correcting those contracts.

# Strengths

- The module split is mechanically sound and preserves public paths. `src/server/mod.rs` already declares `pub mod skills` and re-exports the principal types, so moving `skills.rs` to `skills/mod.rs` does not require a public API move. The plan also correctly insists on `git mv` and a standalone first commit.

- The test-placement analysis is excellent. `make test-skills` really does contain four explicit selectors and zero-test guards at [Makefile](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile:1021). Keeping projection unit tests under `server::skills::projection::tests`, and integration work in the two existing skills test binaries, avoids false-green feature-gated tests.

- The plans correctly identify the only unordered workflow accessor. `WorkflowStep::template_bindings()` returns a `HashMap` at [workflow_step.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/workflow/workflow_step.rs:329), while the other main inputs are slices or `IndexMap`s. Sorting these bindings directly protects SC-2.

- SC-5 is traced to the real implementation rather than assumed. `Skill::as_prompt_text()` adds a trailing newline and appends references at [skills.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:451), so requiring exactly one trailing newline and zero references is necessary for byte equality.

- D-04a is based on the actual handler sequence. `WorkflowPromptHandler` currently pushes user intent and then assistant plan at [prompt_handler.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/workflow/prompt_handler.rs:771), while the task-aware handler independently repeats those pushes at [task_prompt_handler.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/workflow/task_prompt_handler.rs:691). Covering both branches is essential.

- The distinction between the two `ToolInfo` types is correct. Only protocol `crate::types::ToolInfo` carries `annotations` at [tools.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/tools.rs:199); the workflow conversion type does not. This supports the decision to require caller-supplied annotation data.

- Phase 125 conformance machinery is appropriately reused. The existing build pass parses frontmatter, generates diagnostics, validates identity, and constructs manifests rather than requiring a second projection-specific registry implementation.

- The plan correctly avoids `format!("{:x}", digest)` and reuses the existing SHA-256 encoder at [skills.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:1801).

- Dependency ordering across the waves is generally correct:

  - 01 establishes the module and tracer.
  - 02 and 03 independently add render breadth and wire/fuzz coverage.
  - 04 and 05 depend on the complete renderer.
  - 06 waits for both public API and prompt integration.
  - 07 waits for wire/fuzz and documentation/golden completion.

# Concerns

- **HIGH — Raw workflow descriptions cannot safely be inserted “verbatim” into YAML frontmatter.**  
  Plan 01 prescribes:

  ```text
  description: {workflow description verbatim}
  ```

  `SequentialWorkflow::new` accepts any string without description validation at [sequential.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/workflow/sequential.rs:60). A description containing a newline, `:`, `#`, YAML indicators, or document delimiters can change the mapping, introduce extra keys, or make parsing fail. The existing skills build path really parses that YAML at [skills.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs:1240), so this is not cosmetic.

  Examples that need defined behavior include:

  ```text
  "Refund orders:\nmetadata: injected"
  "---"
  "yes"
  ""
  ```

  This also conflicts with Plan 02’s arbitrary-description property/fuzz coverage: arbitrary content may produce a `Skill`, but not necessarily a conforming or registrable one. The phase premise says the projection targets the conforming Phase 125 entry shape from day one.

- **HIGH — D-07 and D-10 are not simultaneously satisfied by the proposed API.**  
  D-07 says bare `as_skill()` has no tool map and therefore cannot generate SC-6 diagnostics. Plan 04’s `gate_check(None)` accordingly returns no gate warnings. Yet D-10 requires the infallible path to emit warnings via `tracing::warn!`, and the plans repeatedly describe SC-6 as having two channels.

  The live code confirms annotations are unavailable from `SequentialWorkflow`: they exist only on protocol `ToolInfo` at [tools.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/tools.rs:219). Therefore:

  - `SkillProjection::with_tools(...).build()` can return SC-6 warnings.
  - `SequentialWorkflow::as_skill()` cannot discover or log those warnings.
  - Plan 04 Task 3 logs only slug-fallback warnings, not the SC-6 warnings from Task 2.

  The “two delivery channels” claim is thus false for the warning that SC-6 actually requires.

- **HIGH — The proposed fuzz target can panic before exercising the projection.**  
  Plan 03 suggests applying `String::from_utf8_lossy(data)` and splitting the resulting string into slices using byte-length quarters. A lossy UTF-8 string can contain multi-byte characters, including `U+FFFD`; arbitrary quarter indices need not be character boundaries. Indexing a `str` at such boundaries panics. That would generate a false fuzz failure in the harness rather than exposing a projection defect.

- **MEDIUM — `SkillProjection::build()` has an unsettled public return type.**  
  D-02 says `build() -> Result<Skill>`, while D-10 says structured warnings are returned from `build()`. Plan 04 resolves this as:

  ```rust
  Result<(Skill, Vec<ProjectionWarning>)>
  ```

  That is plausible but is a materially different public API from the locked wording, and it is awkward for future extension. It should be explicitly ratified rather than hidden inside an implementation plan. A named `ProjectionOutput { skill, warnings }` would be clearer and more extensible.

- **MEDIUM — The task-handler plan lacks a specified way to test the private flag when suppressing the assistant plan.**  
  `WorkflowPromptHandler` fields are private at [prompt_handler.rs](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/workflow/prompt_handler.rs:92). `task_prompt_handler.rs` can call a `pub(crate)` method, but cannot directly read `prepend_projected_skill`. Plan 05 says to guard the task handler’s assistant-plan push “by the same flag condition” without defining an accessor or local mechanism.

  The clean implementation is to call `projected_prepend()` once, retain `prepend.is_some()` as the suppression condition, and then push the optional message. Without this detail, the executor may add another state accessor or duplicate logic.

- **MEDIUM — Description validity and length are omitted from the strict builder contract.**  
  The Agent Skills description has its own legality constraints, but Plan 04 says `build()` has exactly one error condition: a name that normalizes to nothing. Even if YAML escaping is fixed, an empty or overlong description may still produce a nonconforming skill. Either the projection must define normalization/fallback behavior or the strict builder must reject it.

- **MEDIUM — Frontmatter escaping and rendered display text are conflated.**  
  D-13’s “workflow description verbatim” should mean the parsed frontmatter value equals the original description, not that unescaped source bytes are concatenated after `description:`. The wire test currently checks semantic equality, while the renderer instructions mandate unsafe textual equality. Those are different contracts.

- **MEDIUM — The determinism test strategy is heavier than necessary and may be probabilistic.**  
  Reconstructing a workflow 100 times may expose randomized `HashMap` iteration, but the primary proof should be deterministic: insert template bindings in multiple known orders and assert identical output, plus inspect/require sorted rendering. The golden then pins exact bytes. The proposed loop is useful supplementary coverage, not the principal proof.

- **LOW — The Plan 01 blocking checkpoint reopens a locked decision.**  
  D-02 is already explicitly locked and the recommendation is predetermined. Making execution `autonomous: false` for a second acknowledgement adds workflow delay without reducing technical uncertainty. The genuinely unresolved public API question is the builder’s return type, not `as_skill()`.

- **LOW — The `SkillProjection::default()` test requirement is incoherent.**  
  A builder borrowing a workflow cannot meaningfully implement `Default`. Rust tests also cannot directly assert that a method “does not exist” without a compile-fail harness. Remove this acceptance branch.

- **LOW — Requiring doctests on every warning accessor is excessive.**  
  Doctests for the main entry points are valuable. Separate doctests for trivial accessors such as `kind()`, `step()`, `tool()`, and `message()` increase maintenance and digest-adjacent documentation churn without materially improving behavior coverage.

- **LOW — Several plans duplicate extensive governance prose.**  
  The repeated assumption-delta, artifact inventory, test-selector warnings, and threat registers make each plan expensive to execute and increase internal inconsistency risk. For example, the same D-10 claim is repeated even though the implementation described later cannot fulfill it for SC-6.

# Suggestions

- Define a deterministic frontmatter serializer before execution. Serialize a two-field mapping with `serde_yaml`, or implement a tightly tested YAML scalar encoder. The required invariant should be:

  > Parsing the generated frontmatter yields exactly two keys, and the parsed `description` equals the original workflow description.

  Add unit/property tests for colons, newlines, quotes, `#`, `---`, YAML-looking booleans, empty text, and Unicode.

- Resolve description legality explicitly:

  - Strict builder: reject empty or over-limit descriptions.
  - Infallible `as_skill()`: either use a deterministic legal fallback or document and test another total normalization rule.
  - Preserve semantic value where legal; do not promise unsafe raw-line insertion.

- Reconcile D-07 and D-10 at the API level. Viable options include:

  1. Make `build()` return structured warnings and emit those same warnings through tracing while building.
  2. Add an explicit infallible builder terminal such as `build_lossy()` that logs warnings and returns `Skill`.
  3. Narrow D-10 so tracing applies only to slug fallback, while SC-6 has only the structured builder channel.

  Do not claim dual delivery for SC-6 unless both channels can actually observe the same annotation-informed finding.

- Replace the tuple return with a named result:

  ```rust
  pub struct ProjectionOutput {
      skill: Skill,
      warnings: Vec<ProjectionWarning>,
  }
  ```

  This makes D-10 clear and allows additive metadata later.

- Fix fuzz partitioning by splitting the original byte slice into byte chunks first, then calling `String::from_utf8_lossy` independently on each chunk; alternatively split the lossy string using `char_indices()`.

- In Plan 05, specify the task-handler implementation exactly:

  ```rust
  let prepend = self.inner.projected_prepend();
  let suppress_plan = prepend.is_some();
  messages.extend(prepend);
  messages.push(self.inner.create_user_intent(&args));
  if !suppress_plan {
      messages.push(self.inner.create_assistant_plan()?);
  }
  ```

  This avoids exposing the private flag or evaluating the projection twice.

- Strengthen SC-2 with a deterministic permutation test: construct equivalent workflows whose template bindings are inserted in opposite orders and require identical bodies. Retain repeated fresh reconstruction as supplemental coverage.

- Move the human checkpoint from the already-locked `as_skill()` decision to the genuinely costly public API decision: `build()` return shape and warning-channel semantics.

- Add a direct property test that any successful strict projection can pass `Skills::entries()` and `Skills::into_handler()`. This catches malformed frontmatter and description constraints across a broad input set, rather than only testing a friendly `refund_flow` fixture.

- Reduce repeated plan boilerplate by putting shared constraints in `126-VALIDATION.md` and referencing them. Keep each execution plan focused on its own files, behavior, and gates.

# Risk Assessment

**Overall risk: MEDIUM.**

The architecture, sequencing, test reachability, and Phase 125 reuse are strong enough that the phase is likely implementable without major redesign. The principal drift-prevention mechanisms—one renderer, sorted bindings, byte-equality tests, wire pass-through, and a golden—are well chosen.

Risk remains material because the current render contract can generate invalid YAML from ordinary workflow descriptions, the claimed dual warning channel is structurally impossible with the planned inputs, and the fuzz harness design can panic independently of production code. Correct those three issues before execution and the remaining implementation risk should fall to **LOW–MEDIUM**.

---

## Gemini Review

# Cross-AI Plan Review: Phase 126 — Workflow to Skill Projection (`SequentialWorkflow::as_skill()`)

**Review Target:** Implementation Plans `126-01-PLAN.md` through `126-07-PLAN.md`  
**Milestone/Phase:** PMCP SDK Extensions — Phase 126  
**Reviewed by:** Antigravity (Pair Programming & Verification Agent)  
**Overall Verdict:** **APPROVED WITH MINOR RECOMMENDATIONS (READY TO EXECUTE)**

---

## 1. Executive Summary & Quality Assessment

This plan suite is of **exceptionally high engineering quality**. It exhibits a level of rigor, foresight, and defensive testing design that is rare in multi-agent implementations:
- **Traceability:** Every requirement ([SC-1] through [SC-6]) and architectural decision ([D-01] through [D-16a]) has explicit ownership, implementation tasks, and automated verification criteria.
- **Defensive Harness Design:** The author rigorously accounted for subtle toolchain landmines—such as `make test-fuzz` swallowing errors via `|| echo` on stable toolchains, `cargo nextest -E 'test(...)'` silently selecting zero tests, and `--features "full"` excluding the `skills` module—preventing false-green test runs.
- **Wave Structure & Concurrency:** The 5-wave layout is clean. The tracer in Wave 1 validates the physical module split and base path before breadth is added. Furthermore, parallel plans within Wave 2 (Plans 02 and 03) and Wave 3 (Plans 04 and 05) modify **strictly disjoint file sets**, ensuring zero git merge conflicts during parallel execution.
- **Root-Cause Decision Repair (D-04a):** The planning process caught that `SequentialWorkflow::instructions()` was never actually emitted on production prompt wire paths, amending [D-04] into [D-04a] to suppress `create_assistant_plan` while keeping `create_user_intent`'s argument values.

A few ergonomics and builder integration gaps warrant attention before and during execution, detailed below.

---

## 2. Key Architectural Strengths

1. **Clean Module Split with Blame Preservation ([D-03]):**
   - Plan `126-01` executes `git mv src/server/skills.rs src/server/skills/mod.rs` as an isolated step before creating `src/server/skills/projection.rs`.
   - Preserves blame history on a 157 KB file while isolating renderer complexity into a dedicated module that satisfies PMAT cognitive complexity limits ($\le 25$).
2. **Single-String Anti-Drift Invariant ([D-05], [SC-5]):**
   - By ensuring that `SequentialWorkflow::as_skill().body()` produces the exact same string prepended to prompt message `[0]` and served at `skill://{name}/SKILL.md`, drift between the manual procedure and the server-accelerated path is eliminated by construction.
   - Guarded by set equality on tool names and strict anti-vacuity assertions (`references().count() == 0` and trailing newline checks).
3. **Panic-Free, Bounded Slug Normalization ([D-15], [D-15a], [SC-1]):**
   - The slugify algorithm strictly conforms to Agent Skills specs ($1\text{--}64$ chars, `[a-z0-9-]`, no `--`, no leading/trailing `-`).
   - Bounded truncation occurs *after* ASCII conversion, preventing string-slice UTF-8 boundary panics (`String::truncate` panic-free invariant).
   - Names with zero legal alphanumeric characters fall back to deterministic `workflow-{8 hex}` derived from the *original* name bytes via `sha256_digest_hex`, preventing collision cascades.
4. **Determinism Landmine Defused ([SC-2]):**
   - `WorkflowStep::template_bindings()` returns a `HashMap<String, DataSource>`. The plan mandates collecting into a `BTreeMap` by key before rendering, ensuring byte-equality across environments.
   - Property tests explicitly construct fresh workflow instances in each iteration to avoid false passes caused by fixed per-instance `RandomState` seeds.
5. **Structural Side-Effect Gate Check ([SC-6], [D-07]–[D-10]):**
   - Side effects are derived structurally from `crate::types::ToolInfo`'s `ToolAnnotations` (`destructive_hint == Some(true)` or `read_only_hint == Some(false)`) rather than fallible regexes or phrase matching on guidance prose.
   - Distinguishes between explicit side effects and missing/unannotated tools via a distinct "unverifiable" diagnostic.

---

## 3. Findings & Recommended Adjustments

### Finding 1 (DX / Ergonomics): High-Level `ServerBuilder` Integration Gap for [D-04a]
* **Observed in:** [`src/server/builder.rs:1208-1250`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/builder.rs#L1208-L1250), [126-05-PLAN.md](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-05-PLAN.md)
* **The Issue:**
  In Plan `126-05`, the opt-in setter `.with_projected_skill_prepend(bool)` is added exclusively to [`WorkflowPromptHandler`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/workflow/prompt_handler.rs).
  However, in typical server implementations, developers register workflows using:
  ```rust
  server_builder.prompt_workflow(workflow)?;
  ```
  `ServerCoreBuilder::prompt_workflow` constructs `WorkflowPromptHandler::with_middleware_executor` internally and drops no hook to toggle `prepend_projected_skill`.
  Consequently, a standard user building an MCP server has no way to activate the [D-04a] prompt prepend unless they manually instantiate `WorkflowPromptHandler` with tool registries and middleware executors.
* **Recommendation:**
  Add a chainable configuration setter to [`ServerCoreBuilder`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/builder.rs):
  ```rust
  #[cfg(feature = "skills")]
  #[must_use]
  pub fn with_workflow_skill_prepend(mut self, on: bool) -> Self;
  ```
  Or allow `SequentialWorkflow` to carry an optional prompt-prepend preference flag that `prompt_workflow` reads.

---

### Finding 2 (Execution Workflow): Blocking Human Checkpoint in Wave 1
* **Observed in:** [`126-01-PLAN.md:149-179`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-01-PLAN.md#L149-L179)
* **The Issue:**
  Task 1 of Plan 01 is configured with `type="checkpoint:decision"` and `gate="blocking-human"`, asking whether to proceed with infallible `as_skill() -> Skill`.
  However, [D-02] was already locked in [`126-CONTEXT.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-CONTEXT.md) and approved in [`126-DISCUSSION-LOG.md`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-DISCUSSION-LOG.md).
  In automated or semi-autonomous execution, this gate creates unnecessary friction by pausing execution before any code is generated.
* **Recommendation:**
  Confirm the decision up-front (`proceed`) so the executing agent can advance through Task 1 to Task 2 without stalling.

---

### Finding 3 (API Ergonomics): Return Signature of `SkillProjection::build()`
* **Observed in:** [`126-04-PLAN.md:184-189`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-04-PLAN.md#L184-L189)
* **The Issue:**
  Plan 04 specifies `pub fn build(self) -> Result<(Skill, Vec<ProjectionWarning>)>`.
  Returning a tuple forces every caller who uses the builder (e.g., for strict validation or tool annotation warnings) to unpack `let (skill, warnings) = ...` or `let (skill, _) = ...`.
* **Recommendation:**
  Follow the crate's precedent in [`Skills`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/skills.rs):
  - Option A:
    ```rust
    pub fn build(self) -> Result<Skill>; // emits tracing::warn! for warnings
    pub fn build_with_warnings(self) -> Result<(Skill, Vec<ProjectionWarning>)>;
    ```
  - Option B: If the tuple return is retained, ensure doc comments and examples clearly illustrate the destructuring syntax `let (skill, _warnings) = projection.build()?;`.

---

### Finding 4 (Data Determinism): Canonical vs. Insertion Order in `DataSource::Constant`
* **Observed in:** [`126-02-PLAN.md:198-208`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-02-PLAN.md#L198-L208), [`126-RESEARCH.md` Q2](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-RESEARCH.md)
* **The Issue:**
  `serde_json` has the `preserve_order` feature active. This guarantees that re-rendering the *same* `Value` instance is deterministic. However, if two workflows are created with semantically identical JSON objects where keys were inserted in different orders (`{"a": 1, "b": 2}` vs `{"b": 2, "a": 1}`), their generated markdown bodies and digests will differ.
* **Recommendation:**
  In `render_data_source`, when handling `DataSource::Constant(Value::Object(map))`, consider sorting the map keys before serializing, or document explicitly in `projection.rs` rustdocs that JSON constant key order is significant for digest calculation.

---

### Finding 5 (CI Pipeline Integration): Preserving Unreachable Doctests in CI
* **Observed in:** [`126-06-PLAN.md:318-326`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-06-PLAN.md#L318-L326), [`126-07-PLAN.md:245-249`](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/126-workflow-to-skill-projection-sequentialworkflow-as-skill/126-07-PLAN.md#L245-L249)
* **The Issue:**
  The plan astutely observes that `SequentialWorkflow::as_skill()` doctests live under `pmcp::server::workflow::sequential`, meaning `make test-skills` selector 2 (`--doc skills`) misses it, and `make test-all` pins `--features "full"` (which excludes `skills`).
  While Plan 06 Task 3 and Plan 07 Task 2 run this doctest via manual verify commands, it remains omitted from the automated `make quality-gate` recipe.
* **Recommendation:**
  In a future phase (or as an amendment to `Makefile`), ensure `make test-skills` or `make doc-check` includes a doctest pass covering `--features "skills,full"`.

---

## 4. Plan-by-Plan Review Matrix

| Plan | Wave | Requirements | Scope & Key Responsibilities | Quality / Risk Notes |
|---|---|---|---|---|
| **126-01** | Wave 1 | [SC-1], [SC-4] | • Physical module split `skills.rs` $\to$ `skills/mod.rs`<br>• Create `src/server/skills/projection.rs`<br>• Infallible `as_skill() -> Skill`<br>• Slugification + fallback<br>• In-process SC-4 handler test | **Low Risk.** Excellent tracer-first pattern. Pre-approving Task 1 avoids interactive blocking. |
| **126-02** | Wave 2 | [SC-2], [SC-3], [SC-5] | • Full render breadth (Context, Inputs, Steps, Guidance, Resources)<br>• Sort `template_bindings` via `BTreeMap`<br>• Proptests for determinism & slug legality<br>• SC-5 tool set equality | **Low Risk.** Disjoint file scope from 126-03. `kitchen_sink_workflow` cleanly shared between unit & property tests. |
| **126-03** | Wave 2 | [SC-4] | • SC-4 wire proof over loopback `StreamableHttpServer`<br>• Fuzz target `fuzz_workflow_projection`<br>• Fuzz registration tripwire in `tests/skills_routing.rs`<br>• CI matrix entry in `.github/workflows/fuzz.yml` | **Low Risk.** Disjoint file scope from 126-02. Rejects false-green `make test-fuzz` in favor of strict tripwire + nightly command. |
| **126-04** | Wave 3 | [SC-6], [SC-1] | • `SkillProjection` builder & `build() -> Result`<br>• Public `ProjectionWarning` struct & kind enum<br>• Gate check for guidance on side-effecting steps<br>• Dual delivery channels (`tracing::warn!` + structured return) | **Low Risk.** Disjoint file scope from 126-05. Clear handling of unannotated vs. destructive tools. |
| **126-05** | Wave 3 | [SC-5], [SC-2] | • Opt-in `with_projected_skill_prepend` on `WorkflowPromptHandler`<br>• Support both branches of `TaskWorkflowPromptHandler`<br>• Implement [D-04a] (suppress plan, keep user intent)<br>• Transcript tests in `skills_integration.rs` | **Medium Risk.** High complexity in prompt message loops. Well-mitigated by placing tests in `skills_integration.rs` for selector reachability. |
| **126-06** | Wave 4 | [SC-2], [SC-3] | • Tracked golden `tests/golden/workflow_skill_projection.md`<br>• `include_str!` golden test with actionable error message<br>• CHANGELOG & documentation contracts ([D-11], [D-12], [D-13], [D-14])<br>• Self-gated doctests | **Low Risk.** Placed in Wave 4 after render is completely frozen, eliminating golden test churn. |
| **126-07** | Wave 5 | [SC-1]..[SC-6] | • Runnable example `examples/s56_workflow_skill_projection.rs`<br>• `Cargo.toml` example registration (`s56` slot)<br>• Full quality gate run + 3 manual ungateable verifications<br>• Explicit confirmation of deferred items (e.g. Phase 125 WR-03) | **Low Risk.** Assert-then-print pattern ensures the example acts as an executable verification suite. |

---

## 5. Execution Readiness & Recommendations

### Pre-Execution Checklist
1. **Approve Task 1 of Plan 126-01:**
   Proceed with the recommended infallible signature `SequentialWorkflow::as_skill(&self) -> Skill`, backed by the builder's fallible `build() -> Result<...>`.
2. **Execute Wave 1 in Isolation:**
   Ensure `make test-skills` passes immediately following Plan 01 to confirm that `git mv` preserved module resolution and all 99 existing unit tests continue to pass.
3. **Parallel Execution in Waves 2 & 3:**
   - Wave 2: Run `126-02` and `126-03` in parallel (no overlapping files).
   - Wave 3: Run `126-04` and `126-05` in parallel (no overlapping files).
4. **Consider Builder Exposure for D-04a (Finding 1):**
   In Plan 05 or a follow-up task, expose `with_workflow_skill_prepend` on `ServerCoreBuilder` to make the opt-in prompt prepend usable from the primary server builder API.

### Conclusion
The planning package is **approved and ready for execution**. It provides an ironclad, production-grade foundation for implementing Phase 126 while strictly respecting SDK conventions, quality gates, and protocol conformance.

---

## Claude Review (fable)

## Cross-AI Plan Review: Phase 126 (7 plans)

Verified against the live tree at `a2e85522`. Every cite below was read this session.

### 1. Summary

The plan set is unusually well grounded: nearly every line cite resolves (`prompt_handler.rs:767-774` pushes, `task_prompt_handler.rs:677`/`:691-692` branches, `skills.rs:451` `as_prompt_text`, `:1444` `validate_names`, `:1801` `sha256_digest_hex`, `:1189` duplicate-URI `Err`, `Makefile:954` `SKILLS_FEATURES`, `Cargo.toml:713-717` `s45` collision, `fuzz/Cargo.toml:60` skills feature, `fuzz.yml` matrix), the wave ordering has no intra-wave file conflicts, and the test-reachability analysis is correct and load-bearing. The plans carry D-01..D-16 and the three amendments faithfully, with two deliberate deviations noted below. The most serious gap is one no plan mentions: the projection emits `description: {verbatim}` into YAML frontmatter, and Phase 125's parser (`skills.rs:1743-1785`) hands any description containing `: `, `#`, a leading special character, or a newline to `serde_yaml`, which either fails (downgraded to a warning at `:1248`, so the skill serves with **no** frontmatter and `validate_names` at `:1447-1453` silently skips it) or parses to a non-verbatim value. That defeats SC-1 and SC-4 for a large class of real descriptions and no test in the plans would catch it. The second premise-level issue is that the SDK owns the renderer but not the wiring: nothing binds the `Skill` registered in `Skills` to the workflow inside `WorkflowPromptHandler`, and the opt-in flag is unreachable from `prompt_workflow` (`builder.rs:1245-1250`, `mod.rs:4451`), so the anti-drift guarantee holds per workflow *value*, not per server.

### 2. Strengths

- **Test-reachability geometry is correct and enforced.** `Makefile:1021-1032` really is a substring `--lib skills` selector with a zero-count guard; `tests/skills_integration.rs:31` really is `#![cfg(all(feature = "skills", …))]`; `tests/workflow_prompt_e2e_test.rs:6` really is `streamable-http`-gated, so the plans' refusal to put D-04a tests there is right. Every verify block names the zero-tests signal, not just exit code.
- **Insertion point for the prepend is right.** `handle` at `prompt_handler.rs:767-774` builds `messages` then pushes intent/plan; the five exits at `:821`, `:852`, `:870-883`, `:921-939`, `:954` all serialize the accumulated vector, so a push before `:771` survives every path.
- **Task-handler dual path is real and correctly diagnosed.** `task_prompt_handler.rs:676-679` delegates only when task creation fails; `:691-692` rebuilds the header itself. Plan 05 covering both in one plan is the right call (Pitfall 6).
- **No panic in `as_skill()` is achievable.** `StepName::new` (`newtypes.rs:36`) and `ToolHandle::new` are total; `SequentialWorkflow::new` imposes no limits; the slugify order (ASCII-map before `truncate`) is stated with its reason.
- **Fuzz evidence discipline.** Requiring the CI matrix row (`fuzz.yml:26-35` is an explicit hand list) plus the registration tripwire, and rejecting `make test-fuzz`, is correct given the swallowed failure.
- **D-04a is carried exactly**: keep `create_user_intent` (the only place argument values appear, `:406-427`), suppress `create_assistant_plan` (`:429-461`), flag-off byte-identical.
- **SC-5 anti-vacuity** (references == 0, trailing `\n`, len > 200) is precisely what `as_prompt_text` at `:451-467` requires.

### 3. Concerns

- **HIGH — Frontmatter is emitted as raw YAML with an unescaped description.** Plan 01 Task 3 locks `description: {workflow description verbatim}`. `parse_frontmatter_value` (`skills.rs:1743-1785`) feeds the block to `serde_yaml::from_str`. A description like `Refund an order: fast path`, `Handle #123`, `[urgent] refund`, `"quoted"`, or anything with a newline yields `Invalid`/`NotAMapping` or a different string. `build_artifact_inner` (`:1242-1261`) turns that into a *diagnostic*, sets `frontmatter = None`, and `validate_names` (`:1447-1453`) then `continue`s. Result: the skill serves with no `name` in its `skills/list` entry, SC-1's identity check is skipped rather than enforced, and the SC-4 wire assertion in plan 03 would fail for any such fixture. Empty description (`SequentialWorkflow::new("x", "")` is legal) yields `description:` → YAML null, violating agentskills' non-empty rule. No plan mentions escaping, and the proptests only fuzz the *name*.
- **HIGH (premise) — "Cannot drift" is not enforced by the SDK's wiring.** `ServerCoreBuilder::prompt_workflow` (`builder.rs:1245`) and `ServerBuilder::prompt_workflow` (`mod.rs:4451`) construct the handler from their own clone; `Skills` holds a separately constructed `Skill`. The author must call `wf.as_skill()` and `.prompt_workflow(wf)` on the same value themselves; two calls to a `build_workflow()` fn with different arguments drift silently, and the plans declined `Skills::from_workflow` (plan 04 notes) without offering a single-call registration. The D-04a flag is only reachable via direct `WorkflowPromptHandler::new`/`with_middleware_executor` + `.prompt(name, handler)` (`builder.rs:283`, `mod.rs:4370`), so any server using `prompt_workflow` cannot turn on the consumer that makes the two surfaces "share a string". The example (plan 07) will demonstrate the flag only through hand construction, which is not the path real servers use.
- **MEDIUM — Per-request re-render in `projected_prepend`.** Plan 05 Task 1 has it call `self.workflow.as_skill()` on every `prompts/get`. Besides cost, the D-15 slug-fallback `tracing::warn!` fires per request, and message [0] is re-derived from the handler's clone while the served digest is a snapshot. Render once at construction (store the `String`) so the handler carries the same frozen bytes the registry does.
- **MEDIUM — Flag-ON silently removes a validation.** `create_assistant_plan()?` (`prompt_handler.rs:432-437`) returns `Error::Internal("Tool '{}' not found in registry")`. With the flag on it is skipped, so a workflow naming an unregistered tool no longer fails at the head of `handle`; it proceeds into the step loop. Plan 05 should either keep the registry check when suppressing, or document the behaviour change and test it.
- **MEDIUM — Plan 05 Task 3 needs a `TaskRouter` whose `create_workflow_task` succeeds to reach the independent branch (`task_prompt_handler.rs:676`).** Every existing impl is private to its test module (`task_prompt_handler.rs:1335`, `builder.rs:1778`/`:2144`, `core_tests.rs:1210`, `tests/v1_tasks_golden.rs:442`). The task says "read the file to determine which input selects which branch" but never says a stub router must be written in `skills_integration.rs` returning `{"task":{"taskId":..}}`; an executor could take the delegating branch and pass vacuously, which is exactly the failure the plan warns about.
- **MEDIUM — Fuzz target splits a lossy string "by byte length quarters".** `from_utf8_lossy` output still contains multibyte chars; `&s[..len/4]` panics on a non-char-boundary and would be reported as a crash in the harness, not in `as_skill()`. Split on `char_indices`/`is_char_boundary`.
- **MEDIUM — Quoting the description (the fix for the HIGH) interacts with `Skill::new`.** `skills.rs:313` derives `resolved_description` via `parse_frontmatter_description` (`:1677-1697`), which does `strip_prefix("description: ")` + `trim()` with no YAML decoding, so a quoted/escaped value would surface with literal quotes in `prompts/list` for `bootstrap_skill_and_prompt`. Whatever encoding plan 01 chooses, either use `Skill::new(..).with_description(wf.description())` or extend that scanner; add a test asserting `resolved_description() == wf.description()`.
- **LOW — D-02's literal shape is `build() -> Result<Skill>`; plan 04 ships `Result<(Skill, Vec<ProjectionWarning>)>`.** Defensible (D-10 needs the warnings back), but it is a deviation from the locked text and should be recorded as one. Likewise D-01 says `as_skill()` "delegates to the builder", while plan 01 Task 3 has it call `render_body` through a crate-private seam and plan 04 only asserts byte-equality.
- **LOW — `PromptArgumentType` (`src/types/prompts.rs:115`) has no `Display`.** Plan 02 says "append the type" without a mapping; the only free formatter is `Debug`, which is the exact hazard the plans forbid elsewhere. Specify the literal strings.
- **LOW — D-11 exclusion test is weak as specified.** Asserting the accessor *names* are absent proves little. The stronger pin is `render(wf.retryable(true).with_task_support(true)) == render(wf)` byte-equal (`workflow_step.rs:333`, `sequential.rs:194`).
- **LOW — Plan 07 Task 3 asks the reviewer to confirm WR-03 is "restated in `deferred-items.md`"**, but no task in any plan modifies `.planning/phases/125-…/deferred-items.md` or creates a 126 one; `files_modified` for plan 07 lists only the example and `Cargo.toml`.
- **LOW — Plan 01 Task 2 is self-contradictory on the `pub use projection::{…}` line** (add it now vs. types don't exist yet). The acceptance criteria resolve it (only `pub mod projection;` required), but the action text should match.
- **LOW — SC-5 set-equality parse.** If the golden/kitchen-sink fixture ever carries a `PromptContent::ToolHandle` instruction, the Context section will also contain a backticked tool name; the plan's "parse the `Call tool` lines" must be scoped to `## Procedure` explicitly.

### 4. Suggestions

- Add to plan 01 Task 3: encode frontmatter via `serde_yaml::to_string(&{name, description})` or an explicit double-quoted YAML scalar with `\\`/`\"`/`\n` escaping, plus a proptest `prop_frontmatter_roundtrips(description in ".*")` asserting `parse_frontmatter_value(body)` is `Parsed` with `name`/`description` equal to the inputs, and a unit test for the empty-description case (reject in `build()`, substitute in `as_skill()` with a warning, or decide and pin). Set `resolved_description` explicitly.
- Add a builder-level opt-in and single-call registration: `prompt_workflow_with_skill(wf)` (or a `prepend_projected_skill` field on `ServerCoreBuilder` consulted at `builder.rs:1245`) so the flag and the registry entry come from one workflow value. If deferred, say so in CONTEXT `<deferred>` and in the module docs, because it is the premise's weak point.
- Precompute the prepend `String` in the handler constructor/setter; have `both_handlers_produce_the_same_message_zero` compare against the registry-served bytes, not just `as_skill()`.
- Plan 05: decide and test what flag-ON does for an unregistered tool name.
- Plan 05 Task 3: name the stub `TaskRouter` explicitly and assert the transcript carries `_meta` task fields (proof the independent branch ran).
- Plan 03 Task 2: replace "byte length quarters" with a char-boundary split.
- Plan 02: replace the accessor-name absence test with the byte-equality-across-settings test; specify `PromptArgumentType` literals.
- Plan 07: add a task that appends the WR-03 restatement to a 126 `deferred-items.md`, and record the D-02 tuple-return deviation in the SUMMARY.

### 5. Risk Assessment

**MEDIUM.** Execution risk is low: the cites are accurate, waves are ordered correctly, and the gate traps are pre-empted. The residual risk is correctness of the shipped artefact rather than of the process. The frontmatter encoding gap would ship a projection that quietly stops conforming for ordinary descriptions, and because Phase 125 downgrades parse failures to warnings, nothing in the planned test suite goes red. Fixing it is a small change in plan 01 but it changes the golden bytes, so it must land before plan 06 records them. The wiring gap does not break anything but means the phase's headline claim needs to be stated more narrowly until a single-call registration path exists.

---

## Consensus Summary

Three independent lanes reviewed the seven plans with repo access: **Codex** (`gpt-5.6-sol`,
10 `file:line` citations), **Gemini** (6 citations), and the **fable** reviewer instance
(`claude-fable-5-1`, 29 citations). None carried a `[reviewed-without-repo-access]` or
`[reviewed-without-source-citations]` marker, so all three count at full consensus weight.

All three agree the plan set is **well above average**: the wave graph is sound with strictly
disjoint file sets inside each parallel wave, the test-reachability analysis is correct and
load-bearing (the `make test-skills` substring selectors, the `#![cfg(...)]` gates on the two
skills test binaries, the `--features "full"` exclusion, and the swallowed `make test-fuzz`
failure are all pre-empted rather than discovered later), and Phase 125's registry/digest/URI
machinery is reused rather than re-implemented.

**But the headline verdicts diverge sharply, and the divergence is itself the finding** — see
*Divergent Views* below. Two of three lanes rate the phase MEDIUM risk with HIGH-severity
blockers that must land before execution; one rates it "approved and ready to execute."

### Agreed Strengths

- **Module split is mechanically safe (D-03).** `src/server/mod.rs` already declares
  `pub mod skills` and re-exports the principal types, so `git mv skills.rs skills/mod.rs`
  moves no public path. All three lanes independently confirmed this against the tree, and
  all three approve the standalone-first-commit + `git mv` blame-preservation discipline.
- **The determinism landmine is correctly located and defused (SC-2).**
  `WorkflowStep::template_bindings()` returning a `HashMap` (`workflow_step.rs:329`) is the
  *only* unordered accessor on the workflow surface; the other inputs are slices or
  `IndexMap`s. Sorting into a `BTreeMap` before rendering is the right fix, and the
  property tests correctly construct **fresh** workflow instances per iteration so a fixed
  per-instance `RandomState` seed cannot produce a false pass.
- **SC-5 is traced to real code, not assumed.** `Skill::as_prompt_text()`
  (`skills.rs:451`) appends references and a trailing newline, so the plans' anti-vacuity
  assertions (`references().count() == 0`, exactly one trailing `\n`, length > 200) are
  exactly what byte-equality requires — noted by Codex and fable independently.
- **D-04a rests on the actual handler sequence.** `WorkflowPromptHandler` pushes user intent
  then assistant plan (`prompt_handler.rs:767-774`), while the task-aware handler repeats
  those pushes on an independent branch (`task_prompt_handler.rs:691`). Covering **both**
  branches in one plan (05) is correct; fable additionally verified that all five exits from
  `handle` serialize the accumulated vector, so a push before `:771` survives every path.
- **Fuzz evidence discipline.** Requiring the `fuzz.yml` CI matrix row *plus* an in-tree
  registration tripwire — and explicitly refusing to trust `make test-fuzz`, which swallows
  failures via `|| echo` on stable — is called out as correct by all three.
- **Side-effect gating is structural, not textual (SC-6).** Deriving side effects from
  `ToolAnnotations` (`destructive_hint`/`read_only_hint`) rather than regexing guidance prose,
  and distinguishing *unannotated* from *destructive*, is endorsed by Codex and Gemini.

### Agreed Concerns

Ordered by (severity × number of lanes raising it).

1. **HIGH — Raw workflow descriptions are emitted unescaped into YAML frontmatter.**
   *Codex (HIGH) + fable (HIGH). Gemini missed this entirely.*
   Plan 01 Task 3 locks `description: {workflow description verbatim}`.
   `SequentialWorkflow::new` validates nothing (`sequential.rs:60`), so a description
   containing `: `, `#`, `[`, a quote, a leading `-`, `---`, or a newline reaches
   `parse_frontmatter_value` (`skills.rs:1743-1785`) and is handed to `serde_yaml`.
   fable traced the full failure path and it is worse than a parse error:
   `build_artifact_inner` (`skills.rs:1242-1261`) **downgrades the failure to a diagnostic**,
   sets `frontmatter = None`, and `validate_names` (`:1447-1453`) then `continue`s — so the
   skill serves with **no `name`** in its `skills/list` entry, SC-1's identity check is
   *skipped rather than enforced*, and **nothing in the planned test suite goes red.**
   Empty description is legal input and renders `description:` → YAML null, violating
   agentskills' non-empty rule. The proptests fuzz the *name* only.
   → This defeats SC-1 and SC-4 for a large class of ordinary descriptions. It is a small
   change in plan 01, **but it changes the golden bytes, so it must land before plan 06
   records them.**

2. **HIGH — The "cannot drift" premise is not enforced by the SDK's wiring.**
   *fable (HIGH, premise-level) + Gemini (Finding 1, DX framing). Codex did not raise it.*
   `ServerCoreBuilder::prompt_workflow` (`builder.rs:1245`) and `ServerBuilder::prompt_workflow`
   (`mod.rs:4451`) construct the handler from their own clone, while `Skills` holds a
   separately constructed `Skill`. Nothing binds the two to one workflow value. Worse, the
   D-04a opt-in lives only on `WorkflowPromptHandler`, reachable solely via direct
   `::new`/`::with_middleware_executor` + `.prompt(name, handler)` — **so a server that
   registers workflows the normal way cannot turn the feature on at all.** The anti-drift
   guarantee therefore holds per workflow *value*, not per server, and plan 07's example will
   demonstrate the flag only through hand construction, which is not the path real servers use.
   → Either add a builder-level path (`prompt_workflow_with_skill(wf)`, or a
   `prepend_projected_skill` field consulted at `builder.rs:1245`), or narrow the phase's
   headline claim and record the gap in CONTEXT `<deferred>` + module docs.

3. **HIGH/MEDIUM — The proposed fuzz target can panic before it ever reaches the projection.**
   *Codex (HIGH) + fable (MEDIUM).*
   Plan 03 Task 2 applies `String::from_utf8_lossy(data)` then slices the result "by byte
   length quarters". Lossy output still contains multi-byte characters (including `U+FFFD`);
   an arbitrary quarter index need not be a char boundary, and `&s[..len/4]` panics. That
   surfaces as a **crash in the harness reported as a projection defect** — a false positive
   that costs a debugging cycle. → Split the original byte slice into chunks *first* and call
   `from_utf8_lossy` per chunk, or split on `char_indices()`/`is_char_boundary()`.

4. **MEDIUM — `SkillProjection::build()`'s public return type deviates from locked D-02.**
   *All three lanes — the only unanimous concern.*
   D-02's locked text is `build() -> Result<Skill>`; plan 04 ships
   `Result<(Skill, Vec<ProjectionWarning>)>`. All three consider it defensible (D-10 needs the
   warnings back) but agree it is an unrecorded deviation from locked wording and an awkward
   public shape. Codex and Gemini both propose the same remedy independently: a named
   `ProjectionOutput { skill, warnings }`, or a `build()` / `build_with_warnings()` pair.
   → Ratify explicitly and record as a deviation; do not let a locked-decision change hide
   inside an implementation plan.

5. **MEDIUM — The Wave 1 blocking human checkpoint re-opens an already-locked decision.**
   *Codex (LOW) + Gemini (Finding 2).* Plan 01 Task 1 is `checkpoint:decision` /
   `gate="blocking-human"` asking whether to proceed with infallible `as_skill() -> Skill` —
   but D-02 is already locked in CONTEXT and approved in the DISCUSSION-LOG. It stalls
   autonomous execution before any code is generated while reducing no real uncertainty.
   Codex adds the sharper point: **the genuinely unresolved API question is item 4 above
   (the builder's return shape), not `as_skill()`** — move the gate there or drop it.

6. **MEDIUM — Description legality beyond escaping is unspecified.**
   *Codex + fable, converging from different directions.* Plan 04 states `build()` has exactly
   one error condition (a name normalizing to nothing). Even with escaping fixed, empty or
   over-long descriptions still produce a non-conforming skill. fable adds the interaction:
   `Skill::new` derives `resolved_description` via `parse_frontmatter_description`
   (`skills.rs:1677-1697`), which does `strip_prefix("description: ") + trim()` with **no YAML
   decoding** — so a correctly-quoted value would surface with literal quotes in `prompts/list`.
   → Whatever encoding plan 01 picks, either set the description explicitly via
   `Skill::new(..).with_description(wf.description())` or extend that scanner, and add a test
   asserting `resolved_description() == wf.description()`.

### Divergent Views

- **The headline verdict itself is the sharpest divergence, and it is not a matter of taste.**
  Gemini returned "**exceptionally high engineering quality … APPROVED AND READY TO EXECUTE**,"
  rating six of seven plans Low Risk. Codex returned **MEDIUM, "rising to HIGH if implementation
  begins without correcting those contracts."** fable returned **MEDIUM** with the frontmatter
  gap called "the most serious gap … one no plan mentions."
  The two lanes that rated it MEDIUM cited 10 and 29 `file:line` locations; the lane that
  approved it cited 6 — **and missed the finding the other two independently ranked HIGH.**
  Gemini also self-identified in its own output as "Antigravity (Pair Programming &
  Verification Agent)", and its `model` resolved to `unknown`, so its provenance is the least
  certain of the three.
  → Weight accordingly. This reproduces the pattern already recorded in project memory
  (*"Plan-checker PASS ≠ cross-plan consistency … Codex reads source, Gemini doesn't — require
  a source-reading reviewer"*). Treat Gemini's approval as **not** independent corroboration
  that the plans are execution-ready.

- **Whether SC-6 has two working delivery channels.** *Codex only, and it is a sharp,
  checkable claim.* D-07 says bare `as_skill()` has no tool map and therefore cannot generate
  SC-6 diagnostics; D-10 requires the infallible path to emit warnings via `tracing::warn!`.
  Codex argues these are not simultaneously satisfiable: annotations live only on protocol
  `ToolInfo` (`tools.rs:219`), never on `SequentialWorkflow`, so `as_skill()` cannot *observe*
  the SC-6 finding at all, and plan 04 Task 3 in fact logs only slug-fallback warnings.
  The "two delivery channels" claim is therefore false **for the specific warning SC-6
  requires**. Gemini asserts the opposite ("Dual delivery channels … Low Risk") without
  addressing the annotation-availability argument; fable is silent. → Codex's version is the
  one with a mechanism attached and should be resolved before plan 04 executes.

- **How to prove SC-2 determinism.** Codex calls the plan's 100×-reconstruction loop
  "probabilistic … supplementary coverage, not the principal proof" and wants a deterministic
  permutation test (same bindings inserted in opposite orders → identical bytes). Gemini
  praises the same loop as a strength. Both can hold — adopt the permutation test as the
  primary proof and keep the loop as supplemental.

- **Unique to Gemini (worth keeping):** `serde_json`'s `preserve_order` makes re-rendering one
  `Value` deterministic, but two semantically identical JSON constants built with different key
  insertion order render — and digest — differently. Either sort object keys in
  `render_data_source` or document that constant key order is digest-significant.

- **Unique to fable (worth keeping):** (a) flag-ON silently skips `create_assistant_plan()?`
  (`prompt_handler.rs:432-437`), which is where `Error::Internal("Tool not found in registry")`
  is raised — so enabling the prepend **removes a validation**; decide and test that.
  (b) `projected_prepend` re-rendering `as_skill()` on every `prompts/get` re-fires the D-15
  slug `tracing::warn!` per request and lets message[0] drift from the snapshot digest —
  render once at construction. (c) Plan 05 Task 3 cannot reach the independent task branch
  without a stub `TaskRouter` whose `create_workflow_task` *succeeds*; every existing impl is
  private to a test module, so as written an executor may take the delegating branch and
  **pass vacuously**. (d) `PromptArgumentType` (`types/prompts.rs:115`) has no `Display`, so
  plan 02's "append the type" has only `Debug` available — the exact hazard the plans forbid
  elsewhere; specify literal strings. (e) No task in any plan writes the WR-03 restatement
  that plan 07 Task 3 asks a reviewer to confirm.

### Recommended Pre-Execution Actions

1. Fix frontmatter encoding in plan 01 (**before** plan 06 freezes the golden) + add a
   `prop_frontmatter_roundtrips` property test and an empty-description decision.
2. Resolve the D-07/D-10 SC-6 warning-channel contradiction at the API level.
3. Fix the fuzz char-boundary split in plan 03.
4. Ratify the `build()` return shape (named struct preferred) and record the D-02 deviation.
5. Decide the builder-level wiring question (single-call registration) or narrow the claim.
6. Move or drop the Wave 1 blocking checkpoint.
