---
phase: 66-macros-documentation-rewrite
plan: 01
type: execute
wave: 0
depends_on: []
files_modified:
  - pmcp-macros/README.md
  - pmcp-macros/src/lib.rs
autonomous: true
requirements:
  - MACR-03
user_setup: []

must_haves:
  truths:
    - "cargo test --doc -p pmcp-macros compiles a rust,no_run block using use pmcp::mcp_tool;"
    - "cargo test --doc -p pmcp-macros compiles a rust,no_run block using use pmcp_macros::mcp_resource;"
    - "pmcp-macros/src/lib.rs has #![doc = include_str!(\"../README.md\")] at line 1"
    - "Wave 0 POC passes before any Wave 1+ work starts"
  artifacts:
    - path: "pmcp-macros/README.md"
      provides: "Temporary 2-block POC README (overwritten in Wave 2)"
      contains: "use pmcp::mcp_tool"
    - path: "pmcp-macros/src/lib.rs"
      provides: "include_str! wiring at top of file"
      contains: "#![doc = include_str!(\"../README.md\")]"
  key_links:
    - from: "pmcp-macros/src/lib.rs"
      to: "pmcp-macros/README.md"
      via: "include_str! macro"
      pattern: "include_str!\\(\"\\.\\./README\\.md\"\\)"
    - from: "README doctest rust,no_run block"
      to: "pmcp crate re-export"
      via: "use pmcp::mcp_tool"
      pattern: "use pmcp::"
---

<objective>
Prove the `include_str!` + same-crate proc-macro doctest mechanic works on `pmcp-macros` BEFORE
committing to a 200+ line README rewrite. This de-risks research assumptions A2 and A7.

Purpose: Research assumption A2 (that `use pmcp_macros::mcp_resource;` inside a `rust,no_run`
block in README.md-included-via-doc-attribute actually compiles) is the single biggest risk in
this phase. Assumption A7 (that `#[cfg(doctest)] pub struct ReadmeDoctests;` gates correctly)
is secondary but cheap to validate in the same task. If either fails, falling back to Pitfall
4 option 3 (prose-only `mcp_resource` section) is a 10-minute adjustment — BUT discovering the
failure AFTER writing the full README is a 60+ minute sunk cost.

Output: A temporary 2-block POC `pmcp-macros/README.md` wired through
`#![doc = include_str!("../README.md")]` in `lib.rs`, verified to compile under
`cargo test -p pmcp-macros --doc`. This README is intentionally throwaway — it gets fully
rewritten in Plan 04 (Wave 2). The `lib.rs` wiring line stays.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md
@.planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md
@pmcp-macros/Cargo.toml
@pmcp-macros/src/lib.rs
@CLAUDE.md

<interfaces>
<!-- Key facts the executor needs without exploring the codebase. -->

Current state of pmcp-macros/src/lib.rs:
- Line 1 starts with `//! Procedural macros for PMCP SDK` (module doc comments)
- Lines 1-53 are all `//!` comments (module docs, to be deleted in Wave 1b)
- Line 55 begins `use proc_macro::TokenStream;`
- `pub fn mcp_tool`, `pub fn mcp_server`, `pub fn mcp_prompt`, `pub fn mcp_resource` are all present later in the file
- These four macros are exported and functional — do NOT modify them

Current state of pmcp-macros/Cargo.toml:
- Line 3: `version = "0.4.1"`
- Line 4: `edition = "2021"` (pre-2024 — include_str! paths resolve relative to lib.rs)
- Line 14: `rust-version = "1.82.0"`
- Line 17: `[lib] proc-macro = true`
- Line 27: `pmcp = { version = ">=1.20.0", path = "..", features = ["full"] }` as dev-dependency
  → THIS is the mechanism that makes `use pmcp::mcp_tool;` compilable from doctests

Current state of pmcp/src/lib.rs line 147:
- `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};`
- Note: `mcp_resource` is NOT in this re-export (the "re-export gap" from Pitfall 4)
- D-03 locks: do NOT fix this gap in Phase 66

Why this POC: research assumption A2 says a `rust,no_run` block inside README.md (included
via `#![doc = include_str!(...)]` on `lib.rs`) can use `use pmcp_macros::mcp_resource;` even
though `pmcp-macros` is a proc-macro crate. Same-crate proc-macro imports are usually blocked
by cyclic-dependency (rust-lang/rust#58700), BUT because the doctest compiles as an external
crate using `pmcp-macros` as a path dep via the existing `pmcp` dev-dep, the import path
through `pmcp::mcp_tool` works. Whether the direct `pmcp_macros::mcp_resource` path also works
is subtle enough to warrant 2 minutes of empirical validation.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: POC — write 2-block throwaway README, wire include_str!, verify cargo test --doc</name>
  <files>pmcp-macros/README.md, pmcp-macros/src/lib.rs</files>

  <read_first>
    - pmcp-macros/src/lib.rs (current state — lines 1-53 are `//!` module docs that will be
      temporarily REPLACED by the include_str! attribute; subsequent Wave 1b plan fully deletes
      the leftover `//!` lines)
    - pmcp-macros/Cargo.toml (confirm line 27 dev-dep on `pmcp` is unchanged — this powers the doctest)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-10 locks `include_str!`,
      D-09 locks `rust,no_run` not `rust,ignore`)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md sections:
      "Standard Stack" (explains the `pmcp` dev-dep mechanism),
      "Pattern 2: rust,no_run code blocks that compile via pmcp dev-dependency",
      "Pitfall 4: mcp_resource re-export gap creates an asymmetry",
      "Assumptions Log A2 and A7"
    - pmcp/src/lib.rs line 147 (verify the re-export line still reads
      `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};` — absence of `mcp_resource`
      is the exact asymmetry being tested)
  </read_first>

  <action>
    Step 1 — Overwrite `pmcp-macros/README.md` with this exact throwaway content (it WILL be
    overwritten by Plan 04 — do not polish it):

    ````markdown
    # pmcp-macros (POC — Wave 0 validation)

    This file is a temporary proof-of-concept. It gets overwritten in Wave 2 (Plan 04).

    ## Block A — imports via `pmcp` re-export (mcp_tool)

    ```rust,no_run
    use pmcp::mcp_tool;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, JsonSchema)]
    struct AddArgs {
        /// First addend
        a: f64,
        /// Second addend
        b: f64,
    }

    #[derive(Debug, Serialize, JsonSchema)]
    struct AddResult {
        sum: f64,
    }

    #[mcp_tool(description = "Add two numbers")]
    async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult { sum: args.a + args.b })
    }
    ```

    ## Block B — direct import of `mcp_resource` via `pmcp_macros` (re-export gap workaround)

    ```rust,no_run
    use pmcp_macros::mcp_resource;

    #[mcp_resource(uri = "poc://example", description = "POC resource")]
    async fn example_resource() -> pmcp::Result<String> {
        Ok("ok".to_string())
    }
    ```
    ````

    Notes for the executor:
    - Block A imports from `pmcp::mcp_tool` (via re-export at `pmcp/src/lib.rs:147`).
    - Block B imports from `pmcp_macros::mcp_resource` directly (since `mcp_resource` is NOT in
      that re-export line).
    - Both blocks use `rust,no_run` per D-09. Never `rust,ignore`, never bare `rust`.
    - The `mcp_resource` attribute arguments (`uri`, `description`) must match the current
      `pmcp-macros/src/mcp_resource.rs` signature — if the POC fails because the attribute args
      don't match, read that source file and adjust. The goal is to prove the IMPORT path works;
      the attribute signature is secondary.

    Step 2 — Edit `pmcp-macros/src/lib.rs` to add the include_str! wiring. Read the file first;
    the current line 1 is `//! Procedural macros for PMCP SDK`. Insert ABOVE line 1 (at the
    very top of the file) these three lines:

    ```rust
    #![doc = include_str!("../README.md")]

    #[cfg(doctest)]
    pub struct ReadmeDoctests;

    ```

    Do NOT delete the existing `//!` lines 1-53 in this task — they'll be cleaned up in Wave 1b
    (Plan 02). Having both the include_str! AND the `//!` lines simultaneously is fine for the
    POC (rustdoc concatenates them); the goal of Wave 0 is just to validate the mechanism.

    Step 3 — Run the verification commands from the `<verify>` block below. If Block B fails
    to compile (assumption A2 broken), STOP and flag the planner — the fallback is to document
    `#[mcp_resource]` with a prose-only section in Plan 04 per Pitfall 4 option 3. Do NOT try
    to fix the re-export gap (D-03 forbids it). Do NOT proceed to Wave 1 if the POC fails.

    Step 4 — Commit the two modified files with message
    `docs(66): Wave 0 POC — validate include_str! + rust,no_run doctest mechanics`.
    Do NOT amend. Do NOT skip hooks.
  </action>

  <acceptance_criteria>
    - File `pmcp-macros/README.md` exists and contains the exact string `## Block A` and `## Block B`
      (verify: `grep -q '## Block A' pmcp-macros/README.md && grep -q '## Block B' pmcp-macros/README.md`)
    - File `pmcp-macros/README.md` contains at least 2 `rust,no_run` fenced blocks
      (verify: `[ $(grep -c '^```rust,no_run' pmcp-macros/README.md) -ge 2 ]`)
    - File `pmcp-macros/README.md` contains zero `rust,ignore` blocks
      (verify: `! grep -q 'rust,ignore' pmcp-macros/README.md`)
    - File `pmcp-macros/README.md` contains the string `use pmcp::mcp_tool`
      (verify: `grep -q 'use pmcp::mcp_tool' pmcp-macros/README.md`)
    - File `pmcp-macros/README.md` contains the string `use pmcp_macros::mcp_resource`
      (verify: `grep -q 'use pmcp_macros::mcp_resource' pmcp-macros/README.md`)
    - File `pmcp-macros/src/lib.rs` contains the include_str! attribute as the first non-blank line
      (verify: `head -3 pmcp-macros/src/lib.rs | grep -q '#!\[doc = include_str!("../README.md")\]'`)
    - File `pmcp-macros/src/lib.rs` contains the `ReadmeDoctests` hidden struct gated by `#[cfg(doctest)]`
      (verify: `grep -q '#\[cfg(doctest)\]' pmcp-macros/src/lib.rs && grep -q 'pub struct ReadmeDoctests' pmcp-macros/src/lib.rs`)
    - `cargo test -p pmcp-macros --doc` exits 0 (all doctests compile)
    - `cargo build -p pmcp-macros` exits 0 (no regression to the crate build)
    - Git log shows a new commit with the Wave 0 POC message (verify:
      `git log -1 --pretty=%s | grep -q 'Wave 0 POC'`)
  </acceptance_criteria>

  <verify>
    <automated>cargo test -p pmcp-macros --doc 2>&1 | tail -20 && cargo build -p pmcp-macros 2>&1 | tail -5 && grep -c '^```rust,no_run' pmcp-macros/README.md && head -3 pmcp-macros/src/lib.rs | grep -F '#![doc = include_str!("../README.md")]'</automated>
  </verify>

  <done>
    Both `rust,no_run` blocks in the POC README compile under `cargo test -p pmcp-macros --doc`.
    The `include_str!` wiring is in place at the top of `pmcp-macros/src/lib.rs`. Assumption A2
    is validated (or if it fails, the planner is notified so Plan 04 can fall back to Pitfall 4
    option 3). Wave 1 is unblocked. Research assumption A7 is implicitly validated because the
    crate builds without the hidden struct leaking into public API.

    If Block B fails to compile: the executor MUST stop, flip `66-VALIDATION.md`'s
    `wave_0_complete` to `false` and add a note, and return control to the planner. Do NOT
    proceed to Wave 1 with a broken POC.

    If both blocks compile: the executor sets `wave_0_complete: true` in `66-VALIDATION.md`
    frontmatter and proceeds to Wave 1.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| None | This is a documentation-only compile-time change on a proc-macro crate. No trust boundary crossed at runtime. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-66-01 | Tampering | POC README.md | accept | The POC README is intentionally throwaway; Plan 04 overwrites it. No user ever sees it. |
| T-66-02 | Denial of Service | `cargo test --doc` build time | accept | POC adds 2 small doctests (~1s additional compile time). Negligible. |

Security verdict: N/A — Wave 0 is a ~5 minute mechanism validation with no runtime, no
input, no auth, no network surface.
</threat_model>

<verification>
- `cargo test -p pmcp-macros --doc` exits 0
- `cargo build -p pmcp-macros` exits 0
- `pmcp-macros/README.md` contains both POC blocks
- `pmcp-macros/src/lib.rs` has the `include_str!` attribute at the top
- Git commit created with "Wave 0 POC" in the subject
- `66-VALIDATION.md` `wave_0_complete` flipped to `true`
</verification>

<success_criteria>
- Wave 0 gate passes: the `include_str!` mechanic is empirically proven on this crate
- Wave 1 unblocked (executor proceeds to Plan 02, 03, and 04)
- Research assumption A2 validated (or fallback path documented if it fails)
</success_criteria>

<output>
After completion, create `.planning/phases/66-macros-documentation-rewrite/66-01-poc-include-str-gate-SUMMARY.md`
documenting: (a) whether Block B compiled, (b) the actual `cargo test --doc` output, (c) any
adjustments made to the POC README to match the real `mcp_resource` attribute signature,
(d) the commit SHA, and (e) the `wave_0_complete` flag state.
</output>
