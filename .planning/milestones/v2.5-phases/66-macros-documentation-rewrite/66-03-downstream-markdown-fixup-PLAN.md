---
phase: 66-macros-documentation-rewrite
plan: 03
type: execute
wave: 1
depends_on:
  - 66-01-poc-include-str-gate
files_modified:
  - pmcp-course/src/part1-foundations/ch01-03-why-rust.md
  - pmcp-course/src/part5-security/ch13-oauth.md
  - pmcp-course/src/part5-security/ch13-02-oauth-basics.md
  - pmcp-course/src/part5-security/ch13-03-validation.md
  - docs/advanced/migration-from-typescript.md
autonomous: true
requirements:
  - MACR-01
user_setup: []

must_haves:
  truths:
    - "No file under pmcp-course/src/ or docs/advanced/migration-from-typescript.md references `#[tool(` or `#[tool_router]`"
    - "All downstream markdown documentation uses `#[mcp_tool(...)]` / `#[mcp_server]` syntax instead"
    - "Edits are markdown-only; no Rust compile surface touched"
  artifacts:
    - path: "pmcp-course/src/part1-foundations/ch01-03-why-rust.md"
      provides: "Updated tool example using #[mcp_tool]"
      contains: "#[mcp_tool("
    - path: "pmcp-course/src/part5-security/ch13-oauth.md"
      provides: "Two updated tool examples using #[mcp_tool]"
      contains: "#[mcp_tool("
    - path: "pmcp-course/src/part5-security/ch13-02-oauth-basics.md"
      provides: "Updated tool example"
      contains: "#[mcp_tool("
    - path: "pmcp-course/src/part5-security/ch13-03-validation.md"
      provides: "Updated tool example"
      contains: "#[mcp_tool("
    - path: "docs/advanced/migration-from-typescript.md"
      provides: "Updated #[tool]/#[tool_router] references to #[mcp_tool]/#[mcp_server]"
      contains: "#[mcp_tool"
  key_links: []
---

<objective>
Update all downstream markdown documentation that currently shows the deprecated `#[tool(...)]`
or `#[tool_router]` syntax so they instead show the current `#[mcp_tool(...)]` / `#[mcp_server]`
API.

Purpose: Per D-17 and D-18, Phase 66 is responsible for same-phase downstream doc consistency.
The pmcp-course is shipped content that real users land on via mdbook, and
`docs/advanced/migration-from-typescript.md` is a migration guide surfaced via docs.rs — both
will spread obsolete patterns if not updated. Deferring to a follow-up "course refresh" phase
breaks the sync window and is explicitly forbidden by D-18.

This plan is markdown-only with zero Rust compile surface, so it runs in Wave 1 in parallel
with Plan 02 (the code deletion). Their `files_modified` sets are disjoint, so there is no
merge conflict.

Output: Five markdown files with consistent, current macro syntax throughout. All 7 verified
occurrences of the deprecated syntax rewritten. Zero behavioral change (markdown fences are
purely documentation).
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md
@.planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md
@pmcp-course/src/part1-foundations/ch01-03-why-rust.md
@pmcp-course/src/part5-security/ch13-oauth.md
@pmcp-course/src/part5-security/ch13-02-oauth-basics.md
@pmcp-course/src/part5-security/ch13-03-validation.md
@docs/advanced/migration-from-typescript.md

<interfaces>
<!-- Exact locations verified during planning. -->

Verified `grep -n '#\[tool' ...` output against the checkout at phase start:

- `pmcp-course/src/part1-foundations/ch01-03-why-rust.md:118` — `#[tool(`
  (one occurrence — full `#[tool(name = "...", description = "...")]` style)
- `pmcp-course/src/part5-security/ch13-oauth.md:177` — `#[tool(name = "get_my_data", description = "Get data for the authenticated user")]`
- `pmcp-course/src/part5-security/ch13-oauth.md:294` — `#[tool(`
- `pmcp-course/src/part5-security/ch13-02-oauth-basics.md:243` — `#[tool(name = "execute_query", description = "Run a database query")]`
- `pmcp-course/src/part5-security/ch13-03-validation.md:78` — `#[tool(name = "query_sales", description = "Query sales data")]`
- `docs/advanced/migration-from-typescript.md:122` — `#[tool_router]`
- `docs/advanced/migration-from-typescript.md:124` — `#[tool(name = "calculate", description = "Perform calculations")]`

Total: 7 occurrences across 5 files. Confirm via grep at task execution time.

Important caveats for the `#[mcp_tool]` replacement:

1. `#[mcp_tool]` requires `description` as a mandatory compile-time argument. All seven
   occurrences already include `description = "..."`, so the attribute-argument surface is
   directly compatible.

2. `#[mcp_tool]` does NOT accept multiple positional parameters on the function — it expects
   a single argument struct (`fn foo(args: FooArgs)`). If any of the existing examples use
   the multi-parameter style (e.g., `async fn add(a: i32, b: i32) -> ...`), the prose
   surrounding the code block may also need adjustment. HOWEVER — these are markdown examples,
   not compile-tested, so strict `#[mcp_tool]` shape-compliance is NOT required. The rule:
   **stay compatible with the surrounding prose**. If the prose says "this tool takes two
   arguments a and b", don't silently restructure the code to use an `AddArgs` struct without
   updating the prose.

3. The `ch01-03-why-rust.md:118` occurrence is inside a chapter about Rust ergonomics and is
   likely a comparison example, not a copy-paste-runnable snippet. Same rules apply: match
   the prose, don't restructure unnecessarily.

4. The `docs/advanced/migration-from-typescript.md` occurrences at lines 122 and 124 are a
   `#[tool_router] impl Calculator { #[tool(...)] async fn calculate(...) }` block. Rewrite
   the outer `#[tool_router]` to `#[mcp_server]` and the inner `#[tool(...)]` to
   `#[mcp_tool(...)]`. If the function body uses multiple positional params, consider
   leaving them as-is in the markdown (it's a migration guide, not a compile-tested example)
   and add a one-line comment above noting that `#[mcp_server]` + `#[mcp_tool]` expects a
   single-arg-struct pattern — link to `pmcp-macros/README.md` (which will be rewritten in
   Plan 04) for the full signature. Or: rewrite the example to use an `AddArgs` struct. Both
   are acceptable. Pick the option that requires fewer prose rewrites.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Update 4 pmcp-course chapters — #[tool(...)] → #[mcp_tool(...)]</name>
  <files>
    pmcp-course/src/part1-foundations/ch01-03-why-rust.md,
    pmcp-course/src/part5-security/ch13-oauth.md,
    pmcp-course/src/part5-security/ch13-02-oauth-basics.md,
    pmcp-course/src/part5-security/ch13-03-validation.md
  </files>

  <read_first>
    - pmcp-course/src/part1-foundations/ch01-03-why-rust.md (read the full section around
      line 118 — need to see the surrounding prose AND the full code block to make a coherent
      edit. The `#[tool(` line is probably inside a ` ```rust ` fence.)
    - pmcp-course/src/part5-security/ch13-oauth.md (read around both lines 177 and 294 — two
      separate code blocks)
    - pmcp-course/src/part5-security/ch13-02-oauth-basics.md (read around line 243)
    - pmcp-course/src/part5-security/ch13-03-validation.md (read around line 78)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-17, D-18, D-19)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md Pattern 2 section (for
      the canonical `#[mcp_tool]` invocation shape if the executor needs a reference)
  </read_first>

  <action>
    For each of the 5 verified `#[tool(...)]` occurrences (one in ch01-03, two in ch13-oauth,
    one in ch13-02-oauth-basics, one in ch13-03-validation):

    1. Read the full code block containing the occurrence.
    2. Replace `#[tool(` → `#[mcp_tool(` preserving all existing attribute arguments
       (`name = "..."`, `description = "..."`, any others).
    3. If the function signature uses multiple positional parameters AND the surrounding
       prose is NOT committed to the multi-param shape, restructure to single-arg-struct
       style (introduce a `FooArgs` struct before the function) AND update the return type
       to `pmcp::Result<FooResult>` if the original was `Result<FooResult, String>`.
    4. If the surrounding prose IS committed to multi-param shape (e.g., the chapter is
       teaching parameter basics and the code is illustrative), leave the function signature
       untouched — rename ONLY the attribute. The example may not strictly compile under the
       current `#[mcp_tool]` implementation, but it's teaching a concept, not a runnable
       snippet. Leave an HTML comment above the block: `<!-- Illustrative only; compile-ready
       form uses an args struct — see pmcp-macros README -->`.
    5. Do NOT add imports (`use pmcp::...`) unless the chapter already has them. These are
       chapter snippets, not standalone files.
    6. Do NOT touch any occurrence of `#[mcp_tool]` that is already present (D-19 confirms
       there are 6 "v2.0 Tip" locations already advertising the new API — those stay).

    Enumeration with find/replace strings per file:

    - `pmcp-course/src/part1-foundations/ch01-03-why-rust.md:118`
      - Before: `#[tool(` → After: `#[mcp_tool(`
      - Preserve all existing args. Check the full `#[tool(...)]` span — it may span
        multiple lines if arguments were wrapped.

    - `pmcp-course/src/part5-security/ch13-oauth.md:177`
      - Before: `#[tool(name = "get_my_data", description = "Get data for the authenticated user")]`
      - After:  `#[mcp_tool(name = "get_my_data", description = "Get data for the authenticated user")]`

    - `pmcp-course/src/part5-security/ch13-oauth.md:294`
      - Before: `#[tool(` → After: `#[mcp_tool(` (preserve span)

    - `pmcp-course/src/part5-security/ch13-02-oauth-basics.md:243`
      - Before: `#[tool(name = "execute_query", description = "Run a database query")]`
      - After:  `#[mcp_tool(name = "execute_query", description = "Run a database query")]`

    - `pmcp-course/src/part5-security/ch13-03-validation.md:78`
      - Before: `#[tool(name = "query_sales", description = "Query sales data")]`
      - After:  `#[mcp_tool(name = "query_sales", description = "Query sales data")]`

    Commit with subject:
    `docs(66): update pmcp-course chapters to #[mcp_tool] (D-17)`
  </action>

  <acceptance_criteria>
    - `! grep -rn '#\[tool(' pmcp-course/src/part1-foundations/` (zero matches in part1)
    - `! grep -rn '#\[tool(' pmcp-course/src/part5-security/` (zero matches in part5)
    - `grep -c '#\[mcp_tool(' pmcp-course/src/part1-foundations/ch01-03-why-rust.md` ≥ 1
    - `grep -c '#\[mcp_tool(' pmcp-course/src/part5-security/ch13-oauth.md` ≥ 2
    - `grep -c '#\[mcp_tool(' pmcp-course/src/part5-security/ch13-02-oauth-basics.md` ≥ 1
    - `grep -c '#\[mcp_tool(' pmcp-course/src/part5-security/ch13-03-validation.md` ≥ 1
    - The existing 6 "v2.0 Tip: `#[mcp_tool]` ..." locations are preserved (spot-check: `grep -rn 'v2.0 Tip.*mcp_tool' pmcp-course/src/` returns non-zero count same as before)
  </acceptance_criteria>

  <verify>
    <automated>! grep -rn '#\[tool(' pmcp-course/src/part1-foundations/ pmcp-course/src/part5-security/ && grep -c '#\[mcp_tool(' pmcp-course/src/part1-foundations/ch01-03-why-rust.md && grep -c '#\[mcp_tool(' pmcp-course/src/part5-security/ch13-oauth.md && grep -c '#\[mcp_tool(' pmcp-course/src/part5-security/ch13-02-oauth-basics.md && grep -c '#\[mcp_tool(' pmcp-course/src/part5-security/ch13-03-validation.md</automated>
  </verify>

  <done>
    All 5 verified `#[tool(...)]` occurrences in pmcp-course markdown are rewritten to
    `#[mcp_tool(...)]` with attribute arguments preserved. Surrounding prose remains coherent.
    No other chapters were modified.
  </done>
</task>

<task type="auto">
  <name>Task 2: Update docs/advanced/migration-from-typescript.md — #[tool_router] → #[mcp_server], #[tool(...)] → #[mcp_tool(...)]</name>
  <files>docs/advanced/migration-from-typescript.md</files>

  <read_first>
    - docs/advanced/migration-from-typescript.md (read the full section around lines 118-130
      — need to see the `#[tool_router] impl Calculator { ... }` block in full before editing)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-16)
  </read_first>

  <action>
    The file has two related occurrences on lines 122 and 124 forming a single `impl` block:

    Before (approximate — verify by reading):
    ```rust
    #[tool_router]
    impl Calculator {
        #[tool(name = "calculate", description = "Perform calculations")]
        async fn calculate(&self, ... ) -> ... { ... }
    }
    ```

    After:
    ```rust
    #[mcp_server]
    impl Calculator {
        #[mcp_tool(name = "calculate", description = "Perform calculations")]
        async fn calculate(&self, ... ) -> ... { ... }
    }
    ```

    Do the replacements verbatim:
    - `#[tool_router]` → `#[mcp_server]` (single occurrence on line 122 approximate)
    - `#[tool(name = "calculate", description = "Perform calculations")]` →
      `#[mcp_tool(name = "calculate", description = "Perform calculations")]` (line 124 approximate)

    The function body and signature can stay as-is — this is a migration guide, not a
    compile-tested example. Don't restructure `&self, a, b` into an args struct unless the
    surrounding prose forces it. If the function body uses multiple positional params,
    optionally add an HTML comment above the code block:
    `<!-- Note: real #[mcp_tool] requires a single args struct — see pmcp-macros/README.md -->`

    Grep for any OTHER `#[tool` in the file — the verified count was 2, but re-verify in case
    there are additional occurrences. If any exist, update them to `#[mcp_tool(...)]` too.

    Commit with subject:
    `docs(66): update migration-from-typescript.md to #[mcp_tool]/#[mcp_server] (D-16)`
  </action>

  <acceptance_criteria>
    - `! grep '#\[tool_router\]' docs/advanced/migration-from-typescript.md`
    - `! grep '#\[tool(' docs/advanced/migration-from-typescript.md`
    - `grep -q '#\[mcp_server\]' docs/advanced/migration-from-typescript.md`
    - `grep -q '#\[mcp_tool(' docs/advanced/migration-from-typescript.md`
  </acceptance_criteria>

  <verify>
    <automated>! grep '#\[tool_router\]' docs/advanced/migration-from-typescript.md && ! grep '#\[tool(' docs/advanced/migration-from-typescript.md && grep -q '#\[mcp_server\]' docs/advanced/migration-from-typescript.md && grep -q '#\[mcp_tool(' docs/advanced/migration-from-typescript.md</automated>
  </verify>

  <done>
    The two `#[tool_router]` and `#[tool(...)]` occurrences in migration-from-typescript.md
    are rewritten to the current API. No other references to the deprecated syntax remain in
    the file.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| None | Markdown-only documentation updates. No runtime, no compile surface, no input parsing. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-66-06 | Tampering (of user understanding) | pmcp-course and migration-from-typescript.md | mitigate | This IS the mitigation for the broader "stale documentation spreads incorrect patterns" threat identified in research. The plan itself closes the gap. |

N/A — docs-only phase with no security surface.
</threat_model>

<verification>
- `grep -rn '#\[tool(' pmcp-course/src/ docs/advanced/migration-from-typescript.md` returns empty
- `grep -rn '#\[tool_router\]' pmcp-course/src/ docs/advanced/migration-from-typescript.md` returns empty
- Every chapter that had a deprecated reference now has an `#[mcp_tool(...)]` or `#[mcp_server]` reference
- The 6 existing "v2.0 Tip" `#[mcp_tool]` references in pmcp-course are untouched
</verification>

<success_criteria>
- Zero `#[tool(...)]` / `#[tool_router]` references remain in the updated markdown files
- Seven occurrences (5 in pmcp-course + 2 in migration-from-typescript) rewritten to current syntax
- Surrounding prose remains coherent (no silent restructuring of teaching examples)
</success_criteria>

<output>
After completion, create `.planning/phases/66-macros-documentation-rewrite/66-03-downstream-markdown-fixup-SUMMARY.md`
documenting: (a) commit SHA, (b) the 7 exact before/after diffs, (c) whether any of the
pmcp-course examples needed restructuring beyond the simple attribute rename, (d) any
additional `#[tool` / `#[tool_router]` occurrences found beyond the 7 verified — these
should be flagged as research gaps if discovered.
</output>
