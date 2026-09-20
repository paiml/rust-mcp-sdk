# Phase 69: rmcp parity research — ergonomics gap analysis - Context

**Gathered:** 2026-04-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Produce an evidence-backed gap matrix comparing pmcp vs rmcp on *ergonomics* surfaces (tool macros, builder APIs, typed tool/prompt wrappers, handler signatures and state/extra injection, plus client-side ergonomics and error types) and derive follow-on phase proposals from it.

**Out of scope for this phase** (handled elsewhere):
- Transport matrix table, lib.rs doctests, CI example-count gate, cargo semver-checks — all covered by Phase 68 (General Documentation Polish)
- Tower / service integration ergonomics — already addressed in Phase 56
- Transport construction API shape — deferred; can become a follow-on proposal if the matrix surfaces it as a gap
- Implementing any of the follow-on proposals — Phase 69 delivers documents only

Deliverable is two documents that seed new phase proposals; no SDK code changes are produced by Phase 69 itself.

</domain>

<decisions>
## Implementation Decisions

### rmcp reference baseline
- **D-01:** Pin comparison to the latest stable rmcp release on crates.io as of the research start date (2026-04-16). Record the exact version string in 69-RESEARCH.md so findings stay reproducible.
- **D-02:** Ignore unreleased rmcp work (main-branch or unreleased crates) when scoring gap severity. Users can only depend on published APIs. Unreleased direction-of-travel may be footnoted for context but does not affect severity scoring.

### Evidence depth
- **D-03:** Source + docs reading only. Read rmcp source, rustdoc, README, and published examples. Cite file paths and line numbers as evidence for every claim. Target ~1 day of research. No hands-on "build tiny example in both SDKs" — the depth-to-cost ratio does not justify tripling research time.
- **D-04:** pmcp-side baseline is v2.3.0 public API (published) plus current main-branch commits that have already landed on main. The `feat/sql-code-mode` branch is included because it is merged-ready and represents current direction, but findings must note which APIs are v2.3.0 vs post-v2.3.0.

### Ergonomics scope (surfaces to compare)
- **D-05 (MUST):** Tool macros — compare `#[mcp_tool]` (and related `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` macros from pmcp-macros 0.5.0) against rmcp's tool/router macros. Evaluate attribute richness, async support, error handling shape, state injection, schema derivation.
- **D-06 (MUST):** Builder APIs — compare `ServerCoreBuilder` against rmcp's server construction. Evaluate fluent builder shape, discoverability, type-state guardrails, registration patterns for tools/prompts/resources.
- **D-07 (MUST):** Typed tool/prompt wrappers — compare `TypedTool`, `TypedToolWithOutput`, `TypedPrompt`, `TypedSyncTool`, `WasmTypedTool` against rmcp's analogs. Evaluate schema derivation, input/output typing, output schema top-level support.
- **D-08 (MUST):** Handler signatures + state/extra injection — compare async handler fn shape, `RequestHandlerExtra`, cancellation, and state injection patterns (`Arc<State>` composition) against rmcp's equivalents.
- **D-09 (INCLUDE):** Client-side ergonomics — compare how building an MCP client feels: request/response construction, notification handling, progress tracking. Included because pmcp users frequently write both server and client.
- **D-10 (INCLUDE):** Error types + result wrappers — compare error ergonomics, `thiserror` usage, error conversion, `?` operator friction.
- **D-11 (OUT):** Tower / service integration — not compared in Phase 69. pmcp shipped dedicated Tower middleware in Phase 56; if rmcp has relevant patterns, capture as a deferred idea rather than expanding this phase.
- **D-12 (OUT):** Transport construction API shape — not compared here. If the gap matrix surfaces it as a meaningful ergonomics issue, it becomes a follow-on proposal in 69-PROPOSALS.md.

### Gap matrix output shape
- **D-13:** `69-RESEARCH.md` is the gap matrix. Structure per ergonomics surface: table with columns (Surface | rmcp approach + evidence citation | pmcp approach + evidence citation | Gap description | Severity). Followed by a short per-row paragraph only where the table cannot carry the nuance.
- **D-14:** Severity scale — three levels: **High** (affects the typical first-hour user path and has a clean articulable fix), **Medium** (affects less common paths or has no single clean fix), **Low** (cosmetic or theoretical). Severity is assigned during research, not after.

### Follow-on phase proposals
- **D-15:** Number of proposals is not fixed upfront. Write one proposal per gap that clears the severity bar. Expected range is 2–5 proposals; anything outside that range should trigger a re-read of severity assignments.
- **D-16:** Severity bar for becoming a proposal — a gap must be **High** severity (affects typical user path AND a clean specific fix can be articulated). Medium/Low gaps are noted in RESEARCH.md for possible future work but do not get a phase proposal.
- **D-17:** Each proposal targets a multi-day phase with 3–5 plans, similar sizing to Phase 66 (macros doc rewrite) and Phase 67 (docs.rs pipeline). Not single-day quick-wins; not milestone-scale overhauls.
- **D-18:** Each proposal must include: Goal statement, scope (in/out), rough success criteria (3–5 items), suggested requirement IDs, estimated plan count, and where it should slot (late v2.1 vs v2.2). Ready to drop into ROADMAP.md via `/gsd:add-phase` (or insert) with minimal editing.
- **D-19:** `69-PROPOSALS.md` is a separate file from the research matrix so the gap matrix stays stable as a reference document while proposals evolve during review.

### Claude's Discretion
- Table formatting style within 69-RESEARCH.md (column widths, wrapping) — pick whatever reads clearly
- Whether to include a short executive summary at the top of 69-RESEARCH.md — include if the matrix runs long
- Ordering of surfaces within the matrix — default to the order listed in D-05..D-10
- Whether to cite rmcp git blame context beyond file+line — cite only when the history materially explains the ergonomics choice

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project direction
- `.planning/PROJECT.md` — Core Value for v2.1: "Close credibility and DX gaps where rmcp outshines PMCP." The phrase "DX gaps" is the anchor for this phase's ergonomics focus.
- `.planning/REQUIREMENTS.md` — v2.1 requirements. Phase 69 is expected to seed new requirement IDs (post-research) that populate late-v2.1 or v2.2 milestones.
- `.planning/ROADMAP.md` §"Phase 69" — narrowed scope statement (ergonomics-only, proposal-seed, 2 deliverable docs).

### Prior phase decisions (carry forward)
- `.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md` — establishes `#[mcp_tool]`/`#[mcp_server]`/`#[mcp_prompt]`/`#[mcp_resource]` as the preferred macro surface in pmcp-macros 0.5.0. Research must compare rmcp against these, not the deprecated `#[tool]`/`#[tool_router]`.
- `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md` — establishes `CRATE-README.md` pattern + explicit feature list in `[package.metadata.docs.rs]`. Relevant when evaluating how rmcp presents its feature surface.
- `.planning/phases/65-examples-cleanup-protocol-accuracy/65-CONTEXT.md` — pmcp example index conventions (s/c/t/m role prefixes). If the matrix references examples, use this naming.

### pmcp baseline evidence
- `CRATE-README.md` — pmcp 2.3.0 surface overview (Cargo Features table, top-level API snapshot).
- `crates/pmcp-macros/README.md` — macro reference (5 `rust,no_run` doctests for `mcp_*` macros, MACR-02 migration guide).
- `src/lib.rs` — crate root, re-exports, rustdoc.
- `src/server/builder.rs` — `ServerCoreBuilder` surface.
- `src/server/traits.rs` — handler traits.
- `src/server/cancellation.rs` — `RequestHandlerExtra` (non-wasm path).
- `src/shared/cancellation.rs` — `RequestHandlerExtra` (wasm path).
- `src/types/protocol.rs`, `src/types/capabilities.rs` — protocol types (constructor patterns landed in Phase 54.1).
- `.planning/codebase/ARCHITECTURE.md`, `.planning/codebase/STRUCTURE.md` — scout docs for codebase orientation.

### rmcp baseline (external — cite version in RESEARCH.md)
- https://crates.io/crates/rmcp — published version pin (record exact version on research start)
- https://docs.rs/rmcp — rustdoc for the pinned version
- https://github.com/modelcontextprotocol/rust-sdk — source repository (read the release tag matching the crates.io version, not main)

### Methodology
- `.planning/get-shit-done/references/universal-anti-patterns.md` (via .claude) — anti-patterns to avoid when writing the matrix (e.g., cherry-picked evidence, unfalsifiable severity claims).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pmcp-macros/README.md` doctest pattern (from Phase 66) — if follow-on proposals include macro doc work, the `rust,no_run` doctest convention is already established and can be reused.
- `CRATE-README.md` + `#![doc = include_str!("../CRATE-README.md")]` pattern (from Phase 67) — available for any proposal that adds new crate-level documentation.
- `make doc-check` target (from Phase 67) — zero-warning rustdoc gate. If a proposal involves new rustdoc, the gate is already in place.
- `.planning/codebase/ARCHITECTURE.md` and `STRUCTURE.md` — orientation docs for future planners consuming 69-PROPOSALS.md.

### Established Patterns
- Builder-pattern server construction (`ServerCoreBuilder`) is locked — proposals should propose refinements, not replace the builder.
- Typed wrappers (`TypedTool`, `TypedToolWithOutput`, `TypedPrompt`, `TypedSyncTool`) are the pmcp direction — proposals should frame ergonomics improvements around typed wrappers, not stringly-typed handlers.
- Protocol type construction landed uniform constructors in Phase 54.1 — proposals should not reintroduce ad-hoc construction.
- Feature flags gate optional functionality — any proposal adding surface area must specify feature-gating.
- Macros use `darling` attribute parsing and `trybuild` compile-fail snapshots (from Phase 67.2) — proposals extending macros should follow the same testing discipline.

### Integration Points
- Follow-on proposals slot into ROADMAP.md via `/gsd:add-phase` (late-v2.1) or seed a v2.2 milestone file.
- New requirement IDs from proposals land in REQUIREMENTS.md v2.1 Active section or a future v2.2 requirements block.
- If a proposal targets pmcp-macros, the publish order in CLAUDE.md (`pmcp-widget-utils → pmcp → pmcp-macros → pmcp-code-mode → pmcp-code-mode-derive → mcp-tester → mcp-preview → cargo-pmcp`) must be respected.

</code_context>

<specifics>
## Specific Ideas

- "Close credibility and DX gaps where rmcp outshines PMCP" is the v2.1 Core Value — the matrix should be phrased as *what rmcp does better*, then *how pmcp could close the gap*. Do not frame rmcp negatively.
- Evidence citations must be file paths + line numbers (or rustdoc anchor URLs for rmcp). A claim without a citation is not a finding.
- When both SDKs take the same approach on a surface, say so and move on — not every row needs to be a gap.
- When the matrix identifies that rmcp does something worse than pmcp, note it as a "Strength to preserve" not a gap — useful for calibrating future changes.
- Proposals should avoid rmcp-architecture-mimicking (e.g., copying trait-based router layout) — v2.1 Out-of-Scope already rules this out in REQUIREMENTS.md.

</specifics>

<deferred>
## Deferred Ideas

- Tower / service integration ergonomics comparison — pmcp already shipped dedicated Tower middleware in Phase 56; revisit only if 69 unearths something concrete.
- Transport construction API shape — deferred unless the matrix surfaces it as High severity; in that case it becomes a follow-on proposal inside 69-PROPOSALS.md.
- Hands-on build-tiny-example-in-both-SDKs approach — rejected for Phase 69 due to cost; may be worth a spike inside a follow-on proposal if source-reading evidence proves insufficient for a specific surface.
- Gap coverage of testing harness / fixtures / `trybuild` conventions — not in scope; could become its own proposal.
- Client-side subscription and notification ergonomics deep dive — only the surface-level client ergonomics (D-09) are in scope; subscription patterns specifically are deferred.
- Performance ergonomics (zero-alloc patterns, allocation profiles) — not in scope; DX only.

</deferred>

---

*Phase: 69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor*
*Context gathered: 2026-04-16*
