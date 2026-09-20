# Phase 81: Update pmcp-book and pmcp-course with v2 advanced topics (code-mode, tasks, skills) - Context

**Gathered:** 2026-05-15
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase updates the two PMCP documentation properties — `pmcp-book/`
(reference cookbook) and `pmcp-course/` (enterprise hands-on with
exercises and quizzes) — to give first-class coverage to PMCP's three
v2 advanced features: **Skills (SEP-2640)**, **Tasks**, and **Code Mode**.

**In scope:**

- `pmcp-book/`:
  - **New** chapter `ch12-8-skills.md` (Skills, SEP-2640), slotted in
    Part III: Advanced Features between `ch12-7-tasks.md` (Tasks) and
    `ch12-9-code-mode.md` (Code Mode). Adds entry to `src/SUMMARY.md`.
  - **Full rewrite** of `ch12-9-code-mode.md` (currently 146 lines,
    predates the `#[derive(CodeMode)]` macro and the split-out
    `pmcp-code-mode` / `pmcp-code-mode-derive` crates).
  - **Targeted refresh** of `ch12-7-tasks.md` (currently 587 lines) for
    v2 drift only — protocol 2025-11-25, current Tasks API, no full rewrite.
  - One `rust,no_run` doctest at the end of the Skills chapter,
    verifiable via `cargo test --doc`.

- `pmcp-course/`:
  - **New** chapter `ch23-skills.md` appended to Part VIII: Advanced
    Patterns (after `ch22-code-mode.md`). Adds entry to `src/SUMMARY.md`.
  - **Full rewrite** of `part8-advanced/ch22-code-mode.md` (currently
    223 lines).
  - **Targeted refresh** of `part8-advanced/ch21-tasks.md` + its three
    sub-chapters (`ch21-01-lifecycle.md`, `ch21-02-capability-negotiation.md`)
    and `ch21-exercises.md` — light quiz refresh, no structural rewrite.

- Course-style exercises and quizzes (`.ai.toml`) for all three topics:
  - **Skills:** full new exercise set + new quiz (course style requires it).
  - **Code Mode:** refresh existing `ch22` exercises and quiz to match
    the derive-macro rewrite.
  - **Tasks:** light quiz refresh only; keep the existing exercise
    set intact.

- All chapters reference the **same** working examples by full path
  using inline excerpts (10–40 lines) plus a "full example: …"
  cross-link. Same style in book and course — the course's distinct
  voice lives in its exercises/quizzes, not in code presentation.

**Out of scope (deferred to future phases — explicit non-goals):**

- Renumbering existing chapters. The course will accept the asymmetric
  ordering `ch21 Tasks → ch22 Code Mode → ch23 Skills`, while the book
  reads `ch12-7 Tasks → ch12-8 Skills → ch12-9 Code Mode`. Renumbering
  the course's `ch22 Code Mode` to slide Skills in between would break
  every existing cross-reference.
- Translating doctests into book-wide migration of `rust,ignore` blocks.
  Only the Skills chapter gets a doctest in this phase. Other chapters
  keep their existing ignore-blocks.
- New top-level part for SEP extensions. Skills sits next to Tasks /
  Code Mode as a co-equal advanced feature, not segregated as
  "protocol extensions".
- `#[pmcp::skill]` proc macro (deferred from Phase 80) and SEP-2640 §4
  archive distribution. Both are explicit Phase 80 non-goals and stay
  out of scope here. Chapters may mention them as forward-looking notes.
- Updating `ch12-5-mcp-apps.md` (MCP Apps). Phase 48 already refreshed
  this chapter; not in the "v2 advanced topics" trio.

</domain>

<decisions>
## Implementation Decisions

### Depth per topic

- **D-01: Skills — full chapter in both properties.** Zero existing
  coverage. Book gets `ch12-8-skills.md` (~500–700 lines); course gets
  `ch23-skills.md` with the same depth. Mirrors the structure of the
  existing Tasks chapter so the trio reads coherently.
- **D-02: Code Mode — full rewrite of both chapters.** Book `ch12-9`
  is 146 lines and predates the v2 derive macro and split-out crates;
  course `ch22` is 223 lines with the same drift. Both rewrite from
  scratch against the current `pmcp-code-mode` / `pmcp-code-mode-derive`
  API surface (CMSUP-01..06 requirements).
- **D-03: Tasks — targeted refresh only.** Book `ch12-7-tasks.md` (587
  lines) and the course `ch21` chapter + 3 sub-chapters are already
  substantial and structurally sound. Researcher must audit for v2 drift
  (protocol version, latest Tasks API surface) and patch affected
  sections; full rewrite is forbidden.

### Skills chapter structure

- **D-04: Book placement = `ch12-8-skills.md`** between
  `ch12-7-tasks.md` and `ch12-9-code-mode.md`, in Part III: Advanced
  Features. Reader sees Tasks → Skills → Code Mode as a coherent v2
  advanced trio.
- **D-05: Course placement = `ch23-skills.md`** appended to Part VIII:
  Advanced Patterns, after `ch22-code-mode.md`. We accept the
  book↔course ordering asymmetry to avoid renumbering existing course
  chapters. Cross-references in both `SUMMARY.md` files updated.
- **D-06: Single chapter, three-tier walkthrough — same shape in book
  and course.** The chapter walks the three-tier example from Phase 80
  in order: (1) The Dual-Surface Invariant (skill + prompt, byte-equal
  derived from one `Skill` value), (2) Tier 1 hello-world, (3) Tier 2
  refunds (real-world), (4) Tier 3 code-mode (composition with another
  advanced feature), (5) Cross-SDK compatibility note. NO sub-chapters
  per tier — sections within one file.

### Examples integration style

- **D-07: Inline excerpts (10–40 lines) + cross-link to the full
  example.** Matches the established Tasks-chapter style. Each excerpt
  is grep-stable (clear function/struct boundaries) so a future drift
  check can verify excerpts still match the live example. Cross-link
  format: ``Full example: [`examples/s44_server_skills.rs`](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/s44_server_skills.rs)``.
- **D-08: Same reference style in book and course.** Both properties
  use inline excerpts + cross-links. The course's distinguishing voice
  is its exercises/quizzes, not its code presentation. Maintenance
  burden stays consistent.

### Course exercises & quizzes

- **D-09: All three topics get exercise/quiz updates.** Skills =
  full new exercise set + new `.ai.toml` quiz (course style requires
  it for a new chapter). Code Mode = refresh `ch22` exercises and
  quiz to match the derive-macro rewrite. Tasks = light quiz refresh
  only (existing exercise set stays untouched).

### Book doctests

- **D-10: Skills chapter ends with one `rust,no_run` doctest.** Doctest
  demonstrates `.skill(...)` and `.bootstrap_skill_and_prompt(...)`
  ServerCoreBuilder methods. Matches the doctest pattern from the
  Phase 66 pmcp-macros README rewrite. Verifiable via `cargo test --doc`.
  Other chapters in this phase do NOT add new doctests — out of scope.

### Claude's Discretion

The following details are left to the researcher/planner:

- Exact length of each chapter inside the rough budgets above (Skills
  ~500–700 lines, full Code Mode rewrite size, Tasks refresh patch size).
- Which specific Tasks-chapter sections drift relative to the current
  API and need patching (research step must enumerate).
- Whether the Code Mode chapter rewrites use a single worked example
  (`s41_code_mode_graphql.rs`) end-to-end or weave together multiple
  smaller snippets from `crates/pmcp-code-mode/`.
- Exact exercise count per chapter and quiz length — match the
  existing course conventions per `ch20-exercises.md` / `ch21-exercises.md`.
- Whether to add a single forward-looking note about the deferred
  `#[pmcp::skill]` macro and SEP-2640 §4 archive distribution, or
  surface them in a single "Future Work" section per chapter.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 80 source-of-truth artifacts (Skills — non-negotiable)
- `.planning/phases/80-sep-2640-skills-support/80-CONTEXT.md` — Locks
  the dual-surface invariant, the three-tier example structure, the
  10 non-negotiable implementation decisions Skills chapters must
  reflect.
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — Top-level
  index of requirements + spike findings.
- `.claude/skills/spike-findings-rust-mcp-sdk/references/skills-wire-protocol.md`
  — SEP-2640 wire-format details the Skills chapter narrative must
  not contradict.
- `.claude/skills/spike-findings-rust-mcp-sdk/references/skills-dx-layer.md`
  — `Skill` / `Skills` types, builder methods, the dual-surface pattern
  explained at implementation depth.
- `.planning/spikes/MANIFEST.md` — Original spike requirements list.
- `.planning/spikes/002-skill-ergonomics-pragmatic/src/main.rs` —
  Reference implementation of every Skills type and the
  dual-surface byte-equality assertion.

### Working examples to reference inline (Skills)
- `examples/s44_server_skills.rs` — Three-tier server example
  (hello-world / refunds / code-mode) with `.bootstrap_skill_and_prompt(...)`.
- `examples/c10_client_skills.rs` — Client walking both host flows
  (SEP-2640 enumeration vs `prompts/get` fallback).
- `examples/skills/hello-world/SKILL.md` — Tier 1 skill content.
- `examples/skills/refunds/SKILL.md` + `examples/skills/refunds/references/`
  — Tier 2 skill with supporting files (demonstrates SEP-2640 §9
  visibility filtering).
- `examples/skills/code-mode/SKILL.md` — Tier 3 skill that composes
  with PMCP's Code Mode feature.
- `tests/skills_integration.rs` — Integration test demonstrating the
  byte-equal invariant at runtime; chapter narrative should mention
  this as the load-bearing test.

### Working examples to reference inline (Code Mode)
- `examples/s41_code_mode_graphql.rs` — End-to-end Code Mode example
  with `#[derive(CodeMode)]`.
- `crates/pmcp-code-mode/src/lib.rs` — Source of truth for the
  `CodeModeConfig`, `TokenSecret`, `NoopPolicyEvaluator`,
  `ValidationContext` types referenced in chapter prose.
- `crates/pmcp-code-mode-derive/` — Source of truth for the
  `#[derive(CodeMode)]` macro expansion behavior.

### Working examples to reference inline (Tasks)
- `crates/pmcp-tasks/src/lib.rs` — Source of truth for the current
  Tasks API surface (`TaskStore`, `TaskContext`, etc.).
- `crates/pmcp-tasks/tests/lifecycle_integration.rs` — Lifecycle
  end-to-end test; useful as a quoting source.
- `crates/pmcp-tasks/tests/workflow_integration.rs` — Workflow
  integration test; useful for the task-prompt bridge subsection.

### Existing chapters being modified (touch points)
- `pmcp-book/src/SUMMARY.md` — Insert `ch12-8-skills.md` between
  `ch12-7-tasks.md` and `ch12-9-code-mode.md`.
- `pmcp-book/src/ch12-7-tasks.md` — Targeted refresh.
- `pmcp-book/src/ch12-9-code-mode.md` — Full rewrite.
- `pmcp-course/src/SUMMARY.md` — Append `ch23-skills.md` after the
  `ch22-code-mode.md` block.
- `pmcp-course/src/part8-advanced/ch21-tasks.md` + `ch21-01-lifecycle.md`
  + `ch21-02-capability-negotiation.md` + `ch21-exercises.md` —
  Targeted refresh.
- `pmcp-course/src/part8-advanced/ch22-code-mode.md` — Full rewrite.

### Specifications and protocol references
- SEP-2640 PR: `https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2640`
- SEP-2640 reference implementations:
  `https://github.com/modelcontextprotocol/experimental-ext-skills`
- Phase 80 ROADMAP entry (canonical "what landed") —
  `.planning/ROADMAP.md` §"Phase 80: SEP-2640 Skills Support".

### Style references
- `.planning/phases/66-*/66-CONTEXT.md` (if present) — Pattern for
  doctest-in-README from the pmcp-macros rewrite. The Skills
  chapter doctest follows the same shape.
- `pmcp-book/src/ch12-7-tasks.md` — Reference for the
  inline-excerpt + cross-link style chosen in D-07.
- `pmcp-course/src/part8-advanced/ch20-exercises.md` and
  `ch21-exercises.md` — Reference for course exercise structure
  + `.ai.toml` quiz format.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Phase 80 example tree (`examples/s44_server_skills.rs`,
  `examples/c10_client_skills.rs`, `examples/skills/*`)** — Already
  built to be a three-tier teaching example, exactly the structure the
  Skills chapter walks. No additional fixtures required for the
  chapter to teach the full surface.
- **`tests/skills_integration.rs`** — Asserts the dual-surface
  byte-equality invariant at runtime. Citing it lets the chapter say
  "this isn't aspirational — this test enforces it in CI."
- **`crates/pmcp-tasks/tests/` integration tests** — Rich end-to-end
  coverage of the Tasks API. Easier to quote from these than to
  re-derive narrative examples from scratch during the refresh.
- **`examples/s41_code_mode_graphql.rs`** — Already a worked
  end-to-end Code Mode flow. The Code Mode rewrite can hang the
  whole chapter narrative on this example without inventing a new
  scenario.

### Established Patterns
- **Book chapter style (Tasks reference):** Title → "Why" framing
  → mermaid/ASCII diagram → narrative with inline excerpts → "Full
  example" cross-link → optional doctest at end. The Skills chapter
  follows this exact shape.
- **Course chapter style (`ch20`, `ch21`):** Title → Learning
  Objectives bullets → "Why X matters for enterprise MCP" framing
  → conceptual sections → hands-on patterns → exercises page +
  `.ai.toml` quiz. Skills chapter must include all of these.
- **Phase 66 pmcp-macros README pattern:** `rust,no_run` doctest
  wired via `#![doc = include_str!("../README.md")]` so the doctest
  is verified by `cargo test --doc`. The Skills book chapter doctest
  follows the same `rust,no_run` annotation and is verifiable via
  the same command.
- **SUMMARY.md edits:** Both `pmcp-book/src/SUMMARY.md` and
  `pmcp-course/src/SUMMARY.md` are bullet-style mdBook indexes. A
  new entry is a single `- [Chapter Title](filename.md)` line at
  the correct indentation.

### Integration Points
- mdBook build: both `pmcp-book/book.toml` and `pmcp-course/book.toml`
  are configured; `cargo install mdbook` (already present in
  CI) renders both. New chapters appear automatically once linked
  in `SUMMARY.md`. No build-config edits required.
- `cargo test --doc -p pmcp` for the Skills chapter doctest (if the
  doctest lives in a Rust source file referenced by `include_str!`).
  Alternative is to keep the doctest inside the markdown chapter
  and run `mdbook test` — planner's call (D-13 alternative).
- Phase 80 examples are already registered in the workspace
  (`Cargo.toml` `[[example]]` entries) and ship in CI. The chapter
  cross-links can rely on them being present at the named paths.

</code_context>

<specifics>
## Specific Ideas

- **Cross-SDK compatibility section (Skills chapter):** Phase 80's
  three-tier example was deliberately chosen so that hello-world and
  refunds match other SEP-2640 reference implementations (TS SDK,
  gemini-cli, fast-agent, goose, codex). The Skills chapter should
  call this out explicitly so reader can compare PMCP's ergonomics
  side-by-side with other SDKs (URL:
  `https://github.com/modelcontextprotocol/experimental-ext-skills`).
- **Dual-surface invariant — chapter must NOT bury the lede:** The
  byte-equality between the skill surface and the prompt surface is
  the load-bearing design property. Lead with it; don't let it become
  a footnote. Phase 80 spent significant design effort on this — the
  chapter inherits that prominence.
- **Code Mode chapter — derive-macro-first framing:** Phase 80
  ROADMAP and CMSUP-04 establish `#[derive(CodeMode)]` as the
  primary API. The rewrite must NOT lead with manual handler
  registration as the canonical path; the derive macro is the
  "happy path" and manual registration is the advanced/escape-hatch
  variant.
- **Tasks refresh — narrow audit, not narrative restructuring:**
  The existing Tasks chapters are good. The refresh is mechanical
  drift correction (protocol version strings, current type names,
  current method signatures), not a re-think of pedagogy.

</specifics>

<deferred>
## Deferred Ideas

- **`#[pmcp::skill]` proc macro chapter coverage.** Macro is itself
  deferred (Phase 80 explicit non-goal). When the macro lands in a
  future phase, this chapter gets a sub-section.
- **SEP-2640 §4 archive distribution.** Blocked by Phase 80 GAP #2
  (no `blob` field on `Content::Resource`). Mentioned in chapter as
  a forward-looking note only; not implemented or taught.
- **Renumbering the course's `ch22-code-mode.md` → `ch23`** to make
  Skills slot in as `ch22` and align with the book ordering. Pure
  cosmetic consistency win at the cost of breaking every existing
  cross-reference. Worth doing as a dedicated `course-renumbering`
  phase if the project ever does a major TOC refresh.
- **Migration of all book chapters from `rust,ignore` to
  `rust,no_run` doctests.** Phase 81 only adds one new doctest (in
  the Skills chapter). A workspace-wide migration is a follow-on
  phase that needs its own CI plumbing.
- **`mdbook test` integration in CI.** Currently not wired. Adding
  it would enforce every `rust,no_run` block in the book compiles
  on each PR. Worth a follow-on phase paired with the migration
  above.

</deferred>

---

*Phase: 81-update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod*
*Context gathered: 2026-05-15*
