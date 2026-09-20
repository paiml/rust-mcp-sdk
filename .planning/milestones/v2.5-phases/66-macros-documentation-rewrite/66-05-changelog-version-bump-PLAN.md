---
phase: 66-macros-documentation-rewrite
plan: 05
type: execute
wave: 3
depends_on:
  - 66-02-delete-deprecated-macros
  - 66-03-downstream-markdown-fixup
  - 66-04-readme-and-rustdoc-rewrite
files_modified:
  - pmcp-macros/CHANGELOG.md
  - CHANGELOG.md
  - pmcp-macros/Cargo.toml
  - Cargo.toml
autonomous: true
requirements:
  - MACR-02
user_setup: []

must_haves:
  truths:
    - "pmcp-macros/CHANGELOG.md exists and contains a v0.5.0 entry with breaking-change removal and migration guidance"
    - "Root CHANGELOG.md has a v2.3.0 entry with `### pmcp 2.3.0` and `### pmcp-macros 0.5.0` sub-sections"
    - "pmcp-macros/Cargo.toml line 3 reads `version = \"0.5.0\"`"
    - "Root Cargo.toml pmcp-macros dep pin at line ~53 (optional dep) uses version 0.5.0"
    - "Root Cargo.toml pmcp-macros dep pin at line ~147 (examples dev-dep) uses version 0.5.0"
    - "Root Cargo.toml `version` line for `pmcp` is bumped to 2.3.0"
    - "Root Cargo.toml has zero remaining stale `NN_mcp_*_macro` example references (the 63_→s23_ sweep is explicit, not implicit)"
    - "make quality-gate exits 0 (final phase gate matching CI)"
    - "The CHANGELOG migration content covers #[tool]→#[mcp_tool], #[tool_router]→#[mcp_server], and the #[prompt]/#[resource] stubs"
  artifacts:
    - path: "pmcp-macros/CHANGELOG.md"
      provides: "New per-crate CHANGELOG following Keep a Changelog 1.0.0 format"
      contains: "## [0.5.0]"
    - path: "CHANGELOG.md"
      provides: "Root CHANGELOG with v2.3.0 entry"
      contains: "## [2.3.0]"
    - path: "pmcp-macros/Cargo.toml"
      provides: "pmcp-macros version bumped to 0.5.0"
      contains: "version = \"0.5.0\""
    - path: "Cargo.toml"
      provides: "pmcp version bumped to 2.3.0 and both pmcp-macros pins at 0.5.0"
      contains: "version = \"2.3.0\""
  key_links:
    - from: "Cargo.toml:53"
      to: "pmcp-macros/Cargo.toml version"
      via: "`pmcp-macros = { version = \"0.5.0\", path = \"pmcp-macros\", optional = true }`"
      pattern: "pmcp-macros = \\{ version = \"0\\.5"
    - from: "Cargo.toml:147"
      to: "pmcp-macros/Cargo.toml version"
      via: "`pmcp-macros = { version = \"0.5.0\", path = \"pmcp-macros\" }`"
      pattern: "pmcp-macros = \\{ version = \"0\\.5"
---

<objective>
Create the per-crate `pmcp-macros/CHANGELOG.md` (the MACR-02 migration content lives here,
NOT in the README per D-05), add a root `CHANGELOG.md` v2.3.0 entry following the established
multi-crate sub-heading pattern, bump `pmcp-macros` version 0.4.1 → 0.5.0 (D-20), bump `pmcp`
version 2.2.0 → 2.3.0 (D-21), update both `Cargo.toml:53` and `Cargo.toml:147` pmcp-macros
pins to 0.5.0, and run `make quality-gate` as the final CI-equivalent gate.

Purpose: MACR-02 requires a migration section guiding users from deprecated `#[tool]` /
`#[tool_router]` to `#[mcp_tool]` / `#[mcp_server]` with before/after code comparisons. Per
D-05 (locked) and D-13-D-15, this content lives in `pmcp-macros/CHANGELOG.md` (not the
README) with a companion entry in the root `CHANGELOG.md`. Version bumps are required because
the public API surface of `pmcp-macros` is shrinking (breaking change — legal at pre-1.0
minor bump per D-20), and `pmcp` downstream pins need to move with it.

Splitting commits in this plan: one commit for the CHANGELOG work, a separate commit for the
version bumps (following CLAUDE.md's rule that `pmcp-macros` publishes before `pmcp` — a
clean two-commit history makes it obvious which change belongs to which crate's release).

Output: Two CHANGELOG files + two Cargo.toml updates + a green `make quality-gate` run,
leaving the phase ready for a PR to the release branch.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md
@.planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md
@CHANGELOG.md
@Cargo.toml
@pmcp-macros/Cargo.toml
@CLAUDE.md

<interfaces>
<!-- Exact version strings and file locations. -->

Current versions at phase start:
- `pmcp-macros/Cargo.toml:3` → `version = "0.4.1"`  (bump to 0.5.0)
- `Cargo.toml:3` → `version = "2.2.0"`  (bump to 2.3.0)
- `Cargo.toml:53` → `pmcp-macros = { version = "0.4.1", path = "pmcp-macros", optional = true }`
  (bump pin to 0.5.0)
- `Cargo.toml:147` → `pmcp-macros = { version = "0.4.1", path = "pmcp-macros" }  # For macro examples (63_mcp_tool_macro)`
  (bump pin to 0.5.0; ALSO fix the stale `63_mcp_tool_macro` comment — Phase 65 renamed it
  to `s23_mcp_tool_macro` but the inline comment still says `63_`; this is a free cleanup
  while editing the line)

Root CHANGELOG.md pattern (verified lines 1-10):
```
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.2.0] - 2026-04-06

### `pmcp` 2.2.0 — IconInfo wire format spec compliance (CR-002)
```

The 2.2.0 entry uses `### \`pmcp\` 2.2.0 — <subtitle>` as an H3 heading for each crate's
changes. For v2.3.0, use the same pattern but with `### \`pmcp\` 2.3.0 — ...` and
`### \`pmcp-macros\` 0.5.0 — ...` sub-headings.

`pmcp-macros/CHANGELOG.md` — confirmed does NOT exist yet (verified during research). This
task creates it fresh.

Research assumption A6 locks the multi-crate sub-heading pattern as the workspace convention.

Stale-example sweep (external plan review addition): The research flagged `Cargo.toml:147`'s
`63_mcp_tool_macro` comment as a known stale reference. Gemini's review asked that the same
thoroughness be applied to ANY other renamed-example references in `Cargo.toml`. Known Phase
65 renames:
- `63_mcp_tool_macro` → `s23_mcp_tool_macro`
- `64_mcp_prompt_macro` → `s24_mcp_prompt_macro`
If additional `NN_mcp_*_macro` references surface during the sweep below that are NOT on
this mapping table, flag them in the task summary rather than inventing a rename — unknown
numbers could be unrelated to Phase 65 and require human review.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create pmcp-macros/CHANGELOG.md + update root CHANGELOG.md with v2.3.0 entry</name>
  <files>pmcp-macros/CHANGELOG.md, CHANGELOG.md</files>

  <read_first>
    - CHANGELOG.md (read lines 1-60 to see the v2.2.0 multi-crate sub-heading pattern — new
      entry must match this style)
    - pmcp-macros/src/lib.rs (confirm the current post-deletion state is clean — sanity check
      before announcing deletions in the changelog)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-13, D-14, D-15, D-16)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md "Code Examples — Verified
      pmcp-macros/CHANGELOG.md Keep a Changelog format" section (full template to copy from)
  </read_first>

  <action>
    **Part A: Create `pmcp-macros/CHANGELOG.md`** (file does not exist yet)

    Write a new file at `pmcp-macros/CHANGELOG.md` with the following content. The
    `rust,ignore` fences inside CHANGELOG.md are intentional and correct — CHANGELOG.md is
    not included via `include_str!`, so its code blocks are not doctests. `rust,ignore` means
    "syntax highlight but don't compile", which is what you want for migration snippets
    showing the OLD (deleted) syntax (research clarification).

    Use today's date (format `YYYY-MM-DD`) in the entry. Replace `2026-04-11` with `$(date
    +%Y-%m-%d)` at write time.

    Template (fill the date, adjust before/after snippets to match what was actually deleted
    — cross-reference `66-RESEARCH.md` "Code Examples — Verified pmcp-macros/CHANGELOG.md
    Keep a Changelog format" section for the full version):

    ```markdown
    # Changelog

    All notable changes to this project will be documented in this file.

    The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
    and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

    ## [0.5.0] - YYYY-MM-DD

    ### Removed (breaking)

    - **`#[tool]` macro removed.** Deprecated since 0.3.0. Use `#[mcp_tool]`. `#[mcp_tool]`
      provides compile-time `description` enforcement, `State<T>` injection, async/sync
      auto-detection, and `annotations(...)` support.
    - **`#[tool_router]` macro removed.** Deprecated since 0.3.0. Use `#[mcp_server]`.
      `#[mcp_server]` collects tools on an `impl` block and exposes them via
      `ServerBuilder::mcp_server(...)`.
    - **`#[prompt]` zero-op stub removed.** This macro was a placeholder identity function
      that generated no code. Use `#[mcp_prompt]` for the functional equivalent.
    - **`#[resource]` zero-op stub removed.** Same as above — placeholder identity function.
      Use `#[mcp_resource]` for the functional equivalent.

    ### Migration from 0.4.x

    #### `#[tool]` → `#[mcp_tool]`

    Before:

    \`\`\`rust,ignore
    #[tool(description = "Add two numbers")]
    async fn add(params: AddParams) -> Result<AddResult, String> {
        Ok(AddResult { sum: params.a + params.b })
    }
    \`\`\`

    After:

    \`\`\`rust,ignore
    #[mcp_tool(description = "Add two numbers")]
    async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult { sum: args.a + args.b })
    }
    \`\`\`

    Behavioral differences:
    - `description` is enforced at compile time (no more runtime `Option<String>`).
    - Return type is `pmcp::Result<T>` instead of `Result<T, String>`.
    - Shared state via `State<T>` parameter — no `Arc::clone` boilerplate.
    - Async/sync auto-detection from the `fn` signature.
    - Tool annotations via `annotations(read_only = true, destructive = true, ...)`.
    - Registration via `.tool("add", add())` — the macro generates a zero-arg constructor
      returning a `ToolHandler`-implementing struct.

    #### `#[tool_router]` → `#[mcp_server]`

    Before:

    \`\`\`rust,ignore
    #[tool_router]
    impl Calculator {
        #[tool(description = "Add")]
        async fn add(&self, a: i32, b: i32) -> Result<i32, String> { Ok(a + b) }
    }
    \`\`\`

    After:

    \`\`\`rust,ignore
    #[mcp_server]
    impl Calculator {
        #[mcp_tool(description = "Add")]
        async fn add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
            Ok(AddResult { sum: args.a + args.b })
        }
    }

    // Register all tools in one call:
    let builder = ServerBuilder::new().mcp_server(calculator);
    \`\`\`

    #### `#[prompt]` and `#[resource]` stubs

    These macros never generated any code in 0.4.x — they were placeholder identity
    functions. Use `#[mcp_prompt]` and `#[mcp_resource]` for the functional equivalents.
    See the rewritten [README](README.md) for usage.

    ### Changed

    - Crate-level documentation is now sourced from `README.md` via
      `#![doc = include_str!("../README.md")]`. docs.rs and GitHub render the same content
      from a single source.
    - README and per-macro doc comments use `rust,no_run` code blocks (compiled under
      `cargo test --doc`) instead of `rust,ignore`, so API drift is caught automatically.

    ## [0.4.1] - 2026-04-06

    (Prior history preserved in git log; earlier versions were tracked only in the workspace
    root CHANGELOG.md.)
    ```

    **Part B: Update root `CHANGELOG.md`** — prepend a new `## [2.3.0]` entry ABOVE the
    existing `## [2.2.0]` entry.

    Follow the exact multi-crate sub-heading pattern from v2.2.0 (lines 8-41 of current
    CHANGELOG.md). Structure:

    ```markdown
    ## [2.3.0] - YYYY-MM-DD

    ### `pmcp` 2.3.0 — no behavioral change, pmcp-macros bump signal

    #### Changed
    - **Dependency pin bump:** `pmcp-macros` dev-dep pinned at `0.5.0` (was `0.4.1`). See
      the `pmcp-macros` 0.5.0 sub-entry below for the breaking-change surface. `pmcp`'s own
      re-exported public API (`pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool}`) is
      unchanged — users of the `macros` feature who only import `pmcp::mcp_tool` etc. need
      no code changes. Users who depend on `pmcp-macros` directly must migrate; see
      [pmcp-macros/CHANGELOG.md](pmcp-macros/CHANGELOG.md) for the migration guide.
    - **Version bumped to 2.3.0** to signal the transitive macro-surface change to users
      checking `cargo update --dry-run` or crates.io diff feeds.

    ### `pmcp-macros` 0.5.0 — Deprecated macros removed, README rewritten

    #### Removed (breaking)
    - `#[tool]` macro (use `#[mcp_tool]`).
    - `#[tool_router]` macro (use `#[mcp_server]`).
    - `#[prompt]` zero-op stub (use `#[mcp_prompt]`).
    - `#[resource]` zero-op stub (use `#[mcp_resource]`).

    See [pmcp-macros/CHANGELOG.md](pmcp-macros/CHANGELOG.md) for complete migration guide
    including before/after code snippets.

    #### Changed
    - Crate-level docs sourced from `pmcp-macros/README.md` via `include_str!`. docs.rs and
      GitHub render the same content.
    - README fully rewritten to document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`,
      and `#[mcp_resource]` as the primary API with `rust,no_run` doctest-verified examples.
    - Per-macro `///` documentation references the renamed `examples/s23_mcp_tool_macro.rs`
      and `examples/s24_mcp_prompt_macro.rs` files from Phase 65.
    ```

    Insert this block AFTER the first 8 lines of the existing CHANGELOG.md (the format
    declaration) and BEFORE the existing `## [2.2.0]` entry.

    Commit with subject:
    `docs(66): add CHANGELOG entries for pmcp-macros 0.5.0 + pmcp 2.3.0 (MACR-02)`
  </action>

  <acceptance_criteria>
    - `test -f pmcp-macros/CHANGELOG.md`
    - `pmcp-macros/CHANGELOG.md` line 1 is `# Changelog` (verify: `head -1 pmcp-macros/CHANGELOG.md | grep -q '^# Changelog'`)
    - `pmcp-macros/CHANGELOG.md` contains Keep a Changelog header (verify:
      `grep -q 'Keep a Changelog' pmcp-macros/CHANGELOG.md`)
    - `pmcp-macros/CHANGELOG.md` contains `## [0.5.0]` entry (verify:
      `grep -q '^## \[0\.5\.0\]' pmcp-macros/CHANGELOG.md`)
    - `pmcp-macros/CHANGELOG.md` contains `### Removed` subsection (verify:
      `grep -q '^### Removed' pmcp-macros/CHANGELOG.md`)
    - `pmcp-macros/CHANGELOG.md` mentions `#[tool]`, `#[tool_router]`, `#[prompt]`,
      `#[resource]` deletions (verify with grep for each)
    - `pmcp-macros/CHANGELOG.md` contains migration snippets mentioning `#[mcp_tool]`,
      `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]`
    - `pmcp-macros/CHANGELOG.md` contains a `### Migration from 0.4.x` heading (verify:
      `grep -q 'Migration from 0\.4' pmcp-macros/CHANGELOG.md`)
    - `CHANGELOG.md` (root) contains `## [2.3.0]` entry ABOVE the `## [2.2.0]` entry (verify:
      `grep -n '^## \[' CHANGELOG.md | head -2` — first line is 2.3.0, second is 2.2.0)
    - `CHANGELOG.md` contains `### \`pmcp\` 2.3.0` and `### \`pmcp-macros\` 0.5.0` sub-headings
      (verify two separate greps)
    - `CHANGELOG.md` v2.3.0 entry links to `pmcp-macros/CHANGELOG.md` (verify:
      `grep -q 'pmcp-macros/CHANGELOG.md' CHANGELOG.md`)
  </acceptance_criteria>

  <verify>
    <automated>test -f pmcp-macros/CHANGELOG.md && head -1 pmcp-macros/CHANGELOG.md | grep -q '^# Changelog' && grep -q 'Keep a Changelog' pmcp-macros/CHANGELOG.md && grep -q '^## \[0\.5\.0\]' pmcp-macros/CHANGELOG.md && grep -q '^### Removed' pmcp-macros/CHANGELOG.md && grep -qF '#[tool]' pmcp-macros/CHANGELOG.md && grep -qF '#[tool_router]' pmcp-macros/CHANGELOG.md && grep -qF '#[mcp_tool]' pmcp-macros/CHANGELOG.md && grep -qF '#[mcp_server]' pmcp-macros/CHANGELOG.md && grep -q 'Migration from 0\.4' pmcp-macros/CHANGELOG.md && grep -q '^## \[2\.3\.0\]' CHANGELOG.md && grep -q '\`pmcp-macros\` 0\.5\.0' CHANGELOG.md && [ "$(grep -n '^## \[' CHANGELOG.md | head -1 | grep -c '2\.3\.0')" = "1" ]</automated>
  </verify>

  <done>
    Both CHANGELOG files are in place. `pmcp-macros/CHANGELOG.md` carries the full migration
    story per MACR-02. Root `CHANGELOG.md` has the multi-crate v2.3.0 entry following the
    established v2.2.0 pattern. The CHANGELOG commit is independent of the version-bump
    commit that follows.
  </done>
</task>

<task type="auto">
  <name>Task 2: Bump pmcp-macros 0.4.1 → 0.5.0 and pmcp 2.2.0 → 2.3.0 (update all four version strings + sweep stale example refs in Cargo.toml)</name>
  <files>pmcp-macros/Cargo.toml, Cargo.toml</files>

  <read_first>
    - pmcp-macros/Cargo.toml (confirm line 3 still reads `version = "0.4.1"`)
    - Cargo.toml (confirm line 3 still reads `version = "2.2.0"`, line 53 still reads
      `pmcp-macros = { version = "0.4.1", path = "pmcp-macros", optional = true }`, and line
      147 still reads `pmcp-macros = { version = "0.4.1", path = "pmcp-macros" }  # For macro
      examples (63_mcp_tool_macro)`)
    - CLAUDE.md § "Release & Publish Workflow" (for the version bump rules: "Only bump crates
      that have changed since their last publish. Downstream crates that pin a bumped
      dependency must also be bumped.")
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-20, D-21, D-22)
  </read_first>

  <action>
    **Pre-edit discovery sweep (added per external plan review):**

    Before making any version-string edits, enumerate every stale `NN_mcp_*_macro` example
    reference left in `Cargo.toml` so the sweep is explicit rather than implicit. The
    research previously identified only `63_mcp_tool_macro` on line 147, but Gemini's review
    flagged that any OTHER renamed-example references should get the same treatment in the
    same commit. Run:

    ```
    grep -nE '[0-9]+_mcp_[a-z_]+_macro' Cargo.toml || echo "no stale refs"
    ```

    Expected baseline (based on the research): one hit at line 147 for `63_mcp_tool_macro`
    inside the `# For macro examples (...)` inline comment. That hit is already handled by
    Edit 4 below.

    Interpret any ADDITIONAL hits using the Phase 65 rename table:
    - `63_mcp_tool_macro` → `s23_mcp_tool_macro`
    - `64_mcp_prompt_macro` → `s24_mcp_prompt_macro`

    If a hit matches the table, rewrite it to the new name in the same commit. If a hit does
    NOT match the table (i.e. a different `NN_mcp_*_macro` number surfaces), do NOT invent a
    rename — document the unknown reference in the task summary instead and ask for human
    review. The acceptance criteria at the end of this task require zero remaining hits, so
    unknown references must either be resolved or the task must stop before commit.

    **Version-string edits:**

    Perform four exact version-string edits across two files. DO NOT batch into a wildcard
    sed — each edit has context that distinguishes it from other version occurrences.

    **Edit 1 — `pmcp-macros/Cargo.toml:3`:**
    - Before: `version = "0.4.1"`
    - After:  `version = "0.5.0"`

    The only `version = "..."` line at the top of `pmcp-macros/Cargo.toml` — no ambiguity.

    **Edit 2 — `Cargo.toml:3`:**
    - Before: `version = "2.2.0"`
    - After:  `version = "2.3.0"`

    The pmcp crate's own version line at the top of root Cargo.toml's `[package]` section.

    **Edit 3 — `Cargo.toml:53` (the `pmcp-macros` optional dep used by the `macros` feature):**
    - Before: `pmcp-macros = { version = "0.4.1", path = "pmcp-macros", optional = true }`
    - After:  `pmcp-macros = { version = "0.5.0", path = "pmcp-macros", optional = true }`

    **Edit 4 — `Cargo.toml:147` (the `pmcp-macros` dev-dep used for `cargo run --example`):**
    - Before: `pmcp-macros = { version = "0.4.1", path = "pmcp-macros" }  # For macro examples (63_mcp_tool_macro)`
    - After:  `pmcp-macros = { version = "0.5.0", path = "pmcp-macros" }  # For macro examples (s23_mcp_tool_macro)`

    Note the BONUS fix in edit 4: the inline comment currently says `63_mcp_tool_macro` (stale
    from Phase 65 rename). While editing the version, also update the example name to
    `s23_mcp_tool_macro` to match the renamed example file. This is a 2-second free cleanup
    and makes the comment useful again.

    **Additional stale-ref edits (only if the pre-edit sweep found more hits):** For each
    additional hit from the baseline sweep that matches the rename table, apply the same
    inline rewrite (e.g. `64_mcp_prompt_macro` → `s24_mcp_prompt_macro`). Record each applied
    rename in the task summary so the git diff is traceable. If the sweep finds zero
    additional hits, no additional edits are needed and the acceptance criteria will pass on
    edit 4 alone — the sweep command is still required as an explicit audit step.

    **Sanity check:**
    - `cargo build -p pmcp-macros` (should still compile — no API changed)
    - `cargo build -p pmcp` (should still compile — dep pin bump only, API unchanged)
    - `cargo test -p pmcp-macros --doc` (doctests still green — nothing changed beyond versions)

    Commit with subject:
    `chore(66): bump pmcp-macros 0.4.1 → 0.5.0 and pmcp 2.2.0 → 2.3.0 (D-20, D-21)`

    Note: CLAUDE.md § "Release Steps" says to bump in separate commits per crate when
    possible. However, because the pmcp pin updates (edits 3 and 4) MUST land atomically with
    the pmcp-macros version bump (edit 1) to keep the path dependency resolvable, all four
    edits land in one commit. The two commits of this plan (CHANGELOG commit in Task 1, bump
    commit here) is the "two-crate split" pattern in effect — CHANGELOG entries are the
    narrative history, the bump commit is the machine-readable version change.
  </action>

  <acceptance_criteria>
    - `grep -q 'version = "0.5.0"' pmcp-macros/Cargo.toml`
    - `! grep -q 'version = "0.4.1"' pmcp-macros/Cargo.toml`
    - `grep -E '^version = "2\.3\.0"' Cargo.toml` (the root pmcp package version)
    - `! grep -qE '^version = "2\.2\.0"' Cargo.toml`
    - Line 53 area of root Cargo.toml matches `pmcp-macros = { version = "0\.5\.0"` (verify:
      `grep -q 'pmcp-macros = { version = "0\.5\.0", path = "pmcp-macros", optional = true }' Cargo.toml`)
    - Line 147 area of root Cargo.toml matches `pmcp-macros = { version = "0\.5\.0", path = "pmcp-macros" }` (verify:
      `grep -q 'pmcp-macros = { version = "0\.5\.0", path = "pmcp-macros" }' Cargo.toml`)
    - Stale `63_mcp_tool_macro` comment is gone from Cargo.toml (verify:
      `! grep -q '63_mcp_tool_macro' Cargo.toml`)
    - Zero remaining `NN_mcp_*_macro` stale references across Cargo.toml — the explicit
      sweep audit (verify: `! grep -qE '[0-9]+_mcp_[a-z_]+_macro' Cargo.toml`). This is the
      Gemini-review-mandated sweep check: if any unexpected reference remains unsolved, this
      fails and the task must stop before commit.
    - Zero remaining references to `version = "0.4.1"` across pmcp-macros/Cargo.toml and Cargo.toml
      (verify: `! grep -r 'version = "0\.4\.1"' pmcp-macros/Cargo.toml Cargo.toml`)
    - `cargo build -p pmcp-macros` exits 0
    - `cargo build -p pmcp` exits 0 (may take longer due to the version bump invalidating cache)
    - `cargo test -p pmcp-macros --doc` exits 0
  </acceptance_criteria>

  <verify>
    <automated>grep -q 'version = "0.5.0"' pmcp-macros/Cargo.toml && ! grep -q 'version = "0.4.1"' pmcp-macros/Cargo.toml && grep -qE '^version = "2\.3\.0"' Cargo.toml && ! grep -qE '^version = "2\.2\.0"' Cargo.toml && grep -q 'pmcp-macros = { version = "0\.5\.0", path = "pmcp-macros", optional = true }' Cargo.toml && grep -q 'pmcp-macros = { version = "0\.5\.0", path = "pmcp-macros" }' Cargo.toml && ! grep -q '63_mcp_tool_macro' Cargo.toml && ! grep -qE '[0-9]+_mcp_[a-z_]+_macro' Cargo.toml && cargo build -p pmcp-macros 2>&1 | tail -3 && cargo test -p pmcp-macros --doc 2>&1 | tail -5</automated>
  </verify>

  <done>
    All four version strings across two Cargo.toml files are bumped. pmcp-macros/Cargo.toml
    version = 0.5.0, root Cargo.toml version = 2.3.0, both pmcp-macros pins at 0.5.0, stale
    63_mcp_tool_macro comment fixed, and the explicit stale-ref sweep verifies zero remaining
    `NN_mcp_*_macro` references anywhere in Cargo.toml. Crate builds cleanly.
  </done>
</task>

<task type="auto">
  <name>Task 3: Run `make quality-gate` as the final CI-equivalent gate</name>
  <files></files>

  <read_first>
    - CLAUDE.md § "Why `make quality-gate` (not individual cargo commands)" (reminder that
      bare `cargo clippy -- -D warnings` is WEAKER than CI — must use the make target)
    - Makefile or justfile (find the `quality-gate` target definition to understand what it
      runs, in case of failure diagnosis)
  </read_first>

  <action>
    Run `make quality-gate` from the workspace root. This is the exact gate CI runs and
    includes:
    - `cargo fmt --all -- --check`
    - `cargo clippy` workspace-wide with pedantic + nursery lints (`-W clippy::pedantic
      -W clippy::nursery`)
    - `cargo build` with `--features "full"`
    - `cargo test` (may include doc tests depending on the target definition)
    - `cargo audit` (security audit)
    - Plus any additional Toyota Way quality checks per CLAUDE.md

    Expected to be green given:
    - Plans 02-04 preserved real code paths — only deletions and documentation changes
    - All changes compile cleanly at the plan level
    - The Wave 0 POC validated the doctest mechanism

    If it fails:
    - `cargo fmt` fail → run `cargo fmt --all` and stage the result
    - `cargo clippy` fail → read the error, fix the flagged code (NOT by adding `#[allow(...)]`
      unless CLAUDE.md permits it), re-run
    - `cargo audit` fail → investigate; unrelated to this phase, may require a dep update in
      a separate commit
    - `cargo test` fail → check if a test references a deleted macro that Plan 02 missed,
      or if an edit to lib.rs accidentally broke a real `pub fn mcp_*` export

    DO NOT commit anything in this task — this task's purpose is the gate. If `make
    quality-gate` passes, no new commit is needed. If it fails and requires a fix, commit
    the fix as a NEW commit with subject:
    `fix(66): address quality-gate feedback — <specific issue>`
    (NEVER use `git commit --amend` on prior phase 66 commits — CLAUDE.md is emphatic on this).

    After `make quality-gate` is green, update `.planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md`:
    - Set `nyquist_compliant: true` in frontmatter
    - Set `wave_0_complete: true` (should already be true from Plan 01, but double-check)
    - Set `status: approved` in frontmatter
    - Under the "Validation Sign-Off" section, check all the boxes and mark `**Approval:** approved`
  </action>

  <acceptance_criteria>
    - `make quality-gate` exits 0
    - `.planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md` frontmatter has
      `nyquist_compliant: true` (verify: `grep -q 'nyquist_compliant: true' .planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md`)
    - `.planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md` frontmatter has
      `status: approved` (verify: `grep -q 'status: approved' .planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md`)
    - `cargo test -p pmcp-macros --doc` still passes (this is the fast sanity check that
      README doctests compile — redundant with `make quality-gate` but cheap and fast)
    - `git status` shows a clean working tree (no uncommitted changes unless a fix was needed)
  </acceptance_criteria>

  <verify>
    <automated>make quality-gate 2>&1 | tail -30 && grep -q 'nyquist_compliant: true' .planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md && grep -q 'status: approved' .planning/phases/66-macros-documentation-rewrite/66-VALIDATION.md && cargo test -p pmcp-macros --doc 2>&1 | tail -5</automated>
  </verify>

  <done>
    `make quality-gate` is green. Phase 66 is complete: deprecated macros deleted, stubs
    deleted, README rewritten, CHANGELOGs written, versions bumped, full workspace CI gate
    passes. Ready for PR to the release branch.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Supply chain | `pmcp-macros` v0.5.0 + `pmcp` v2.3.0 will be published to crates.io via CI on tag push (handled by `.github/workflows/release.yml` per CLAUDE.md). Downstream users automatically pick up the new versions via `cargo update`. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-66-10 | Tampering | Downstream users upgrading to 0.5.0 without reading CHANGELOG | mitigate | Pre-1.0 semver signals breaking change at minor bump. CHANGELOG migration section provides concrete before/after code. Users who `cargo update` without reading CHANGELOG will get a clear compile error (their `#[tool]` / `#[tool_router]` call becomes a "not found" error), which is the fail-fast signal. No silent breakage. |
| T-66-11 | Denial of Service | CI publish pipeline fails mid-release | accept | CLAUDE.md release workflow has retry logic via the 30-second wait between crate publishes and graceful handling of already-published versions. Not unique to this phase. |
| T-66-12 | Elevation of Privilege | `make quality-gate` bypassed | mitigate | Pre-commit hook enforces `make quality-gate` per CLAUDE.md. CLAUDE.md explicitly forbids `--no-verify` without justification. This plan's Task 3 runs the gate explicitly. |
| T-66-13 | Information Disclosure | CHANGELOG leaks sensitive details | accept | N/A — CHANGELOG documents public API changes only; no secrets, credentials, or internal paths. Verified content above contains only macro names and semver strings. |

Release-coordination phase; security surface is the supply chain, and the existing CI
release workflow owns that gate.
</threat_model>

<verification>
- Both CHANGELOG files exist with correct content per acceptance criteria
- All four version strings bumped
- Cargo.toml has zero stale `NN_mcp_*_macro` example references (explicit sweep audit)
- `make quality-gate` exits 0
- `cargo test -p pmcp-macros --doc` exits 0
- `66-VALIDATION.md` marked `nyquist_compliant: true` and `status: approved`
- Git history has three new commits: Task 1 (CHANGELOG), Task 2 (version bumps), optionally
  Task 3 fixup commit if quality gate surfaced issues
</verification>

<success_criteria>
- MACR-02 satisfied: migration content present in `pmcp-macros/CHANGELOG.md` with concrete
  before/after snippets per D-13
- D-13, D-14, D-15, D-16, D-20, D-21, D-22 all honored
- Publish order ready: `pmcp-macros` version bump visible at `pmcp-macros/Cargo.toml:3`,
  `pmcp` version bump visible at `Cargo.toml:3`, both pins aligned
- `make quality-gate` (the CI-equivalent gate) passes
- Phase ready for PR to release branch per CLAUDE.md § "Release Steps" step 5
- Gemini review fold-in: explicit stale-example sweep of Cargo.toml verified (no leftover
  `NN_mcp_*_macro` references)
</success_criteria>

<output>
After completion, create `.planning/phases/66-macros-documentation-rewrite/66-05-changelog-version-bump-SUMMARY.md`
documenting: (a) commit SHAs for the CHANGELOG commit and the version-bump commit, (b) exact
`make quality-gate` pass/fail with timing, (c) any quality-gate issues that required a fixup
commit (and the root cause), (d) the explicit stale-example sweep outcome (how many hits,
which renames were applied, or confirmation that only the `63_mcp_tool_macro` baseline was
present), (e) final phase summary: line counts, file counts, versions, readiness for PR.

Then create `.planning/phases/66-macros-documentation-rewrite/66-SUMMARY.md` (phase-level
summary) aggregating the achievements of all five plans per the template at
`.claude/get-shit-done/templates/summary.md`.
</output>
