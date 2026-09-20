# Phase 66: Macros Cleanup + Documentation Rewrite - Context

**Gathered:** 2026-04-11
**Status:** Ready for research

> **Scope note:** Phase was originally titled "Macros Documentation Rewrite". During discussion the scope expanded to also delete deprecated and stub macros, because keeping them while rewriting the docs would be self-defeating (the README would document "new API" while `pmcp-macros` source still ships dead code). The milestone-level relabel from "v2.1 rmcp Upgrades = docs polish" to "v2.1 rmcp Upgrades = docs + macros cleanup" is a deliberate, captured scope expansion — not drift.

<domain>
## Phase Boundary

Make `pmcp-macros` cleaner and its documentation accurate:

1. **Delete deprecated and stub macros** from `pmcp-macros` — `#[tool]`, `#[tool_router]` (deprecated but functional, 683 lines of real code), `#[prompt]`, `#[resource]` (zero-op stub placeholders, never implemented). Together with their tests and module declarations.
2. **Rewrite `pmcp-macros/README.md`** to document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` as the only API. No migration section in the README itself (new readers don't need it) — migration goes into a dedicated `pmcp-macros/CHANGELOG.md` / `MIGRATION.md` entry for the v0.5.0 release.
3. **Wire the README as crate-level rustdoc** via `#![doc = include_str!("../README.md")]` in `lib.rs`, so docs.rs/pmcp-macros and GitHub show the same content from a single source.
4. **Update downstream consumers** that still reference the deleted macros: 4 pmcp-course chapters and `docs/advanced/migration-from-typescript.md`.
5. **Release coordination:** bump `pmcp-macros` v0.4.1 → v0.5.0 (pre-1.0 breaking minor bump is semver-legal), update `pmcp`'s dep pin, add CHANGELOG entries for both crates. `pmcp` itself is probably v2.2.0 → v2.3.0 — its re-exports don't change, but its transitive public surface shrinks.

**Out of scope (reject if raised):**
- Adding new macro capabilities (e.g., new attribute syntax, new code generation features). That's a feature phase.
- Changing `#[mcp_tool]` / `#[mcp_server]` / etc. behavior. Those are the target API — documenting them, not modifying them.
- Fixing the `mcp_resource` re-export gap in `pmcp/src/lib.rs:147` (it re-exports `mcp_prompt`, `mcp_server`, `mcp_tool` but not `mcp_resource`). Flag as a potential bug for a separate phase — don't bundle.
- Automated migration tooling (e.g., a `cargo pmcp migrate` command). Users hand-migrate per the CHANGELOG.
- Deprecating `#[mcp_*]` names or renaming them. Locked as the primary API.

</domain>

<decisions>
## Implementation Decisions

### Macro deletion scope
- **D-01:** Delete `#[tool]` and `#[tool_router]` entirely — the full `pmcp-macros/src/tool.rs` (426 lines), `pmcp-macros/src/tool_router.rs` (257 lines), their tests (`pmcp-macros/tests/tool_tests.rs` 129 lines, `pmcp-macros/tests/tool_router_tests.rs` 71 lines), the `pmcp-macros/tests/ui/tool_missing_description.rs` UI test if any, the `mod tool` / `mod tool_router` declarations in `lib.rs`, and the `pub fn tool` / `pub fn tool_router` exports with their `#[deprecated]` attrs.
- **D-02:** Delete `#[prompt]` and `#[resource]` entirely — these are literal identity-function stubs at `pmcp-macros/src/lib.rs:319-323` and `:338-342` with the comment "Prompt macro implementation deferred to future release". They generate zero code, have zero production usage (verified: `grep -rn "#\[prompt\]\|#\[resource\]"` returns only the stale `//!` lines in lib.rs itself), and mislead users who apply them expecting them to work. Delete the `pub fn prompt` / `pub fn resource` definitions plus the `//!` lines at `lib.rs:10-11` that advertise them.
- **D-03:** Keep `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` — these are the real, functional, primary API. No changes to their implementation in this phase.
- **D-04:** Do not leave transitional "please use `#[mcp_tool]`" compile errors or hint shims. The v0.5.0 CHANGELOG carries the migration story; the source tree stays clean.

### README structure and audience
- **D-05:** Target audience is **new users**, not migrators. The README leads with the current API, zero apologies about a legacy path. The word "migration" does not appear in the README.
- **D-06:** Single unified README (not four sub-READMEs per macro). Covers all four `mcp_*` macros in one document, grouped by macro with consistent section structure per entry (purpose, attributes, minimal working example, feature flags, link to full example).
- **D-07:** Each of the four macros gets **proportional** depth based on its real-world usage: `#[mcp_tool]` is the showcase (most common, richest feature surface — `description`, `name`, `annotations(...)`, `ui = "..."`, State<T> injection, async auto-detection), `#[mcp_server]` gets full treatment (router-level macro, second most common), `#[mcp_prompt]` and `#[mcp_resource]` get focused sections that cover attributes and link out to canonical examples. No macro is reduced to a one-liner.
- **D-08:** Installation section uses **current pinned versions** — `pmcp = "2.3"` (or whatever lands post-bump) with `features = ["macros"]`. No `version = "1.*"` stragglers. No `pmcp-macros` as a direct dependency in user-facing examples — tell users to pull macros via `pmcp`'s `macros` feature flag (single dependency).
- **D-09:** Every README code block must compile. `rust,no_run` is the default — `rust,ignore` is forbidden for in-README code. If a code block needs external setup (e.g., a running server), split it: show the compilable tool definition in-README with `rust,no_run`, and link to `examples/s23_mcp_tool_macro.rs` for the runnable version.

### lib.rs doc strategy
- **D-10:** `pmcp-macros/src/lib.rs` uses `#![doc = include_str!("../README.md")]` as the top-level module doc. Delete all existing `//!` comments at lines 1–53 (they currently duplicate README content AND reference the soon-to-be-deleted old macros). Single source of truth: README.md.
- **D-11:** Per-macro `///` doc comments on `pub fn mcp_tool`, `pub fn mcp_server`, etc. stay **in place and get rewritten**. These are what docs.rs shows on individual macro pages and are separate from the module doc. Scope includes updating their examples to reference renamed phase 65 example files (`s23_mcp_tool_macro`, `s24_mcp_prompt_macro`) and flipping `rust,ignore` → `rust,no_run` where possible.
- **D-12:** The stale per-macro doc on `pub fn tool` (lines 68–96) goes away when the function itself is deleted — no separate cleanup needed.

### Migration surface (external to README)
- **D-13:** Create or update `pmcp-macros/CHANGELOG.md` with a v0.5.0 entry that includes:
  - **Breaking:** `#[tool]`, `#[tool_router]`, `#[prompt]`, `#[resource]` removed
  - **Migration:** `#[tool]` → `#[mcp_tool]` with concrete before/after snippet. Note the behavior differences (mandatory `description`, State<T> injection, async auto-detection, `annotations(...)` support) — these are real changes, not just rename
  - **Migration:** `#[tool_router]` → `#[mcp_server]` with before/after
  - **Migration:** `#[prompt]` / `#[resource]` — "these never did anything; use `#[mcp_prompt]` / `#[mcp_resource]` which are the functional equivalents"
- **D-14:** If no `pmcp-macros/CHANGELOG.md` exists today, create it. If one exists, prepend the v0.5.0 entry.
- **D-15:** `pmcp`'s top-level `CHANGELOG.md` gets a v2.3.0 entry noting the `pmcp-macros` version bump and the transitive impact on direct `pmcp-macros` users. No impact for `pmcp` users unless they explicitly added `pmcp-macros` as a separate dep.
- **D-16:** `docs/advanced/migration-from-typescript.md` gets updated to show current `#[mcp_tool]` syntax wherever it currently shows `#[tool]`. Cross-referenced from the macros CHANGELOG.

### Downstream consumer updates (pmcp-course)
- **D-17:** Four pmcp-course chapters use `#[tool(...)]` syntax today (`pmcp-course/src/part1-foundations/ch01-03-why-rust.md:118`, `part5-security/ch13-oauth.md:177,294`, `ch13-02-oauth-basics.md:243`, `ch13-03-validation.md:78`). Update all to `#[mcp_tool(...)]` with any necessary argument adjustments (e.g., moving function params to a struct type for the tools that use multiple params).
- **D-18:** Phase 66 is responsible for these course updates. Do NOT defer to a separate "course refresh" phase — the course is shipped content and users will land on it, so breaking the sync is not acceptable.
- **D-19:** Course chapters that currently show "v2.0 Tip: The `#[mcp_tool]` macro eliminates `Box::pin(async move { ... })` boilerplate" (6 locations found) stay unchanged — they already advertise the new API correctly.

### Release coordination
- **D-20:** `pmcp-macros` version: v0.4.1 → v0.5.0. Pre-1.0 semver allows breaking changes at minor bumps, so this is correct per the Rust API guidelines.
- **D-21:** `pmcp` version: v2.2.0 → v2.3.0. `pmcp`'s own re-exported public API is unchanged (it only re-exports the `mcp_*` macros), but bumping to v2.3 communicates that the macro ecosystem underneath changed and signals users to check the CHANGELOG. A patch bump would technically work but under-communicates.
- **D-22:** Both releases ship in a single PR, tagged in the sequence the existing release workflow handles (`pmcp-macros` publishes before `pmcp` per CLAUDE.md release order). One PR, one tag per crate.

### Claude's Discretion
- Exact prose and tone of the rewritten README (professional, concrete, example-first)
- Whether to include a "feature flags" table in the README or rely on docs.rs's auto-generated feature badges (phase 67's domain)
- Visual ordering of the four macros in the README — start with `#[mcp_tool]` since it's the most common, but exact sequencing is editorial
- Whether to include a brief "why proc macros for MCP" intro paragraph, or jump straight to usage
- Whether to show sync and async variants separately or in the same code block
- Exact CHANGELOG.md formatting (keepachangelog.com vs project's existing style — check main CHANGELOG.md for convention)

</decisions>

<specifics>
## Specific Ideas

- "The v2 should be cleaner" — user's direct framing. No apologies about old API in the new docs; don't carry legacy baggage into the forward narrative.
- **Memory reinforcement:** Saved feedback from prior session — *"During breaking-change window, consolidate aggressively — don't defer as 'not worth the churn'"*. This phase is exactly that philosophy applied.
- Reference to `rmcp` (official Rust MCP SDK) is the framing for the whole v2.1 milestone — their docs.rs page is clean and current. The target bar for PMCP's `pmcp-macros` README is "at least as clean as rmcp's crate docs."
- Prefer `rust,no_run` over `rust,ignore` — the whole point of doctests is to catch drift. `rust,ignore` is how we ended up with a README referencing `pmcp = "1.1"` at v2.2.0.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner) MUST read these before proceeding.**

### Current source to replace or delete
- `pmcp-macros/src/lib.rs` — the target of most edits: module docs (lines 1–53, delete), per-macro `///` docs (update), deprecated `#[tool]`/`#[tool_router]`/`#[prompt]`/`#[resource]` exports (delete)
- `pmcp-macros/src/tool.rs` — delete entirely (426 lines)
- `pmcp-macros/src/tool_router.rs` — delete entirely (257 lines)
- `pmcp-macros/tests/tool_tests.rs` — delete entirely (129 lines)
- `pmcp-macros/tests/tool_router_tests.rs` — delete entirely (71 lines)
- `pmcp-macros/tests/ui/` — check for `tool_missing_description.rs` and delete if present
- `pmcp-macros/README.md` — rewrite completely (252 lines today, mostly obsolete)

### Current functional macros (the target API, do NOT modify)
- `pmcp-macros/src/mcp_tool.rs` — functional `#[mcp_tool]` implementation
- `pmcp-macros/src/mcp_server.rs` — functional `#[mcp_server]` implementation
- `pmcp-macros/src/mcp_prompt.rs` — functional `#[mcp_prompt]` implementation
- `pmcp-macros/src/mcp_resource.rs` — functional `#[mcp_resource]` implementation
- `pmcp-macros/src/mcp_common.rs` — shared helpers used by all four

### Working examples (authoritative code references for the README)
- `examples/s23_mcp_tool_macro.rs` — `#[mcp_tool]` showcase (renamed in Phase 65)
- `examples/s24_mcp_prompt_macro.rs` — `#[mcp_prompt]` showcase (renamed in Phase 65)
- Both require `features = ["full"]` per current Cargo.toml

### Existing design docs (source material, may need updates as part of this phase)
- `docs/design/mcp-macros-guide.md` (343 lines) — current macro patterns reference. May already be accurate; verify and update if it references deleted macros
- `docs/design/mcp-tool-macro-design.md` (360 lines) — design rationale for `#[mcp_tool]`. Unlikely to need changes, but cross-check

### Downstream consumers that must be updated in this phase
- `pmcp-course/src/part1-foundations/ch01-03-why-rust.md:118` — `#[tool(...)]` → `#[mcp_tool(...)]`
- `pmcp-course/src/part5-security/ch13-oauth.md:177,294` — two `#[tool(...)]` occurrences
- `pmcp-course/src/part5-security/ch13-02-oauth-basics.md:243` — `#[tool(...)]`
- `pmcp-course/src/part5-security/ch13-03-validation.md:78` — `#[tool(...)]`
- `docs/advanced/migration-from-typescript.md` — update stale `#[tool]` references

### Release workflow
- `CLAUDE.md` § "Release & Publish Workflow" — authoritative for version bump procedure, quality gate requirements, and the `pmcp-macros` before `pmcp` publish order
- `Cargo.toml:53` — `pmcp-macros` optional dep (feature: `macros`), must update version pin
- `Cargo.toml:147` — `pmcp-macros` direct dep for macro examples, must update version pin

### Requirements traceability
- `.planning/REQUIREMENTS.md:22-24` — MACR-01, MACR-02, MACR-03 definitions
- `.planning/ROADMAP.md:712-721` — Phase 66 success criteria (note: success criteria #2 says "migration section guides users from deprecated `#[tool]`/`#[tool_router]` to `#[mcp_tool]`/`#[mcp_server]` with before/after code comparisons" — this is now satisfied by the CHANGELOG migration, not an in-README section)

### Related prior phase context
- `.planning/phases/58-mcp-tool-proc-macro/58-CONTEXT.md` and `58-RESEARCH.md` — original context for why `#[mcp_tool]` was introduced. Background only, not required reading

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`pmcp-macros/src/mcp_tool.rs`, `mcp_server.rs`, `mcp_prompt.rs`, `mcp_resource.rs`**: Functional implementations. Do NOT modify. Their `pub fn`s in `lib.rs` have `///` docs that need rewriting but the underlying codegen is the target API.
- **`examples/s23_mcp_tool_macro.rs` / `s24_mcp_prompt_macro.rs`**: Already exist as working demos — the README's "complete example" pointers should target these. Saves duplicating 100+ line examples in the README.
- **Existing `docs/design/mcp-macros-guide.md`**: 343 lines of content that can be cannibalized for the README rewrite if it's up-to-date. Check first.

### Established Patterns
- **Pre-commit hook + quality gate**: `make quality-gate` runs `cargo test --doc` which executes `rust` / `rust,no_run` blocks. `rust,ignore` is invisible to it — that's how drift happened. Pattern: compile every example in every README.
- **Release order**: `pmcp-macros` publishes first, then `pmcp` (from CLAUDE.md § Release Workflow). Plan the commits accordingly — bump `pmcp-macros` version in one commit, update `pmcp`'s dep pin + bump `pmcp` version in the next.
- **Version pin convention**: `Cargo.toml` workspace uses `version = "X.Y.Z"` with path + version for workspace members (lines 53 and 147 for `pmcp-macros`). Both pins need to move to `"0.5.0"`.
- **Inline module docs via `include_str!`**: Not currently used in this workspace. This phase introduces the pattern. Other crates in the workspace (`mcp-tester`, `mcp-preview`, `cargo-pmcp`) may benefit from the same pattern later — flag as potential future phase but out of scope here.

### Integration Points
- **`pmcp/src/lib.rs:147`** (`pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};`) — pmcp's re-export. This line is the authoritative "what does `use pmcp::*` give you" declaration. **Note a potential gap: `mcp_resource` is NOT re-exported.** Flag as a potential bug for a separate phase — do NOT expand phase 66 scope to fix this. It's a two-line change but it's a behavioral addition, not cleanup.
- **`pmcp-course`**: Separate content product under `pmcp-course/src/`. Edits to course chapters are markdown-only; no build required. Just content sync.

</code_context>

<deferred>
## Deferred Ideas

- **`mcp_resource` re-export gap** at `pmcp/src/lib.rs:147` — missing from `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool}`. Adding it is a behavioral change, not cleanup. File as a standalone backlog item or small follow-up phase.
- **`include_str!("../README.md")` pattern for other workspace crates** — `mcp-tester`, `mcp-preview`, `cargo-pmcp`, `pmcp` itself may benefit from the same pattern. Out of scope for phase 66 (which is macros-specific), but noted as a candidate for the broader v2.1 docs polish in phase 67 or 68.
- **Automated migration tooling** (e.g., `cargo pmcp migrate` subcommand that rewrites `#[tool]` → `#[mcp_tool]` in user codebases) — user explicitly did not ask for this; hand migration via CHANGELOG is fine given pre-1.0 `pmcp-macros` user count.
- **Macro feature additions** (new attributes, new codegen capabilities) — not cleanup, not documentation. Separate phase if ever needed.
- **Moving macro implementations into `pmcp` core** (eliminating `pmcp-macros` as a separate crate) — considered during context gathering, rejected because proc macros must live in a `proc-macro = true` crate per Rust compiler rules. The separation is structural, not optional.

</deferred>

---
*Phase: 66-macros-documentation-rewrite*
*Context gathered: 2026-04-11*
*Scope expanded from "docs rewrite" to "macros cleanup + docs rewrite" during discussion*
