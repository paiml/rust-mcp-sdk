---
phase: 66-macros-documentation-rewrite
plan: 02
type: execute
wave: 1
depends_on:
  - 66-01-poc-include-str-gate
files_modified:
  - pmcp-macros/src/lib.rs
  - pmcp-macros/src/tool.rs
  - pmcp-macros/src/tool_router.rs
  - pmcp-macros/tests/tool_tests.rs
  - pmcp-macros/tests/tool_router_tests.rs
  - pmcp-macros/tests/ui/tool_missing_description.rs
  - pmcp-macros/tests/ui/tool_missing_description.stderr
  - pmcp-macros/Cargo.toml
autonomous: true
requirements:
  - MACR-01
user_setup: []

must_haves:
  truths:
    - "No file at pmcp-macros/src/tool.rs"
    - "No file at pmcp-macros/src/tool_router.rs"
    - "No file at pmcp-macros/tests/tool_tests.rs"
    - "No file at pmcp-macros/tests/tool_router_tests.rs"
    - "No file at pmcp-macros/tests/ui/tool_missing_description.rs"
    - "No file at pmcp-macros/tests/ui/tool_missing_description.stderr"
    - "pmcp-macros/src/lib.rs has no `mod tool;` or `mod tool_router;` declarations"
    - "pmcp-macros/src/lib.rs has no `pub fn tool(` or `pub fn tool_router(` definitions"
    - "pmcp-macros/src/lib.rs has no `pub fn prompt(` or `pub fn resource(` stub definitions"
    - "pmcp-macros/src/lib.rs has no `#[deprecated(since = \"0.3.0\"` attributes"
    - "pmcp-macros/src/lib.rs has no `//!` module-doc comments at the top of the file"
    - "pmcp-macros/src/lib.rs first non-blank line is `#![doc = include_str!(\"../README.md\")]`"
    - "pmcp-macros/Cargo.toml has no `tool_router_dev` feature"
    - "cargo build -p pmcp-macros compiles cleanly"
    - "cargo test -p pmcp-macros passes (remaining mcp_* tests still valid)"
  artifacts:
    - path: "pmcp-macros/src/lib.rs"
      provides: "Trimmed lib.rs with only the four real `pub fn mcp_*` exports plus the include_str! wiring from Wave 0"
      not_contains: "pub fn tool"
    - path: "pmcp-macros/Cargo.toml"
      provides: "Cargo.toml without the WIP `tool_router_dev` feature"
      not_contains: "tool_router_dev"
  key_links:
    - from: "pmcp-macros/src/lib.rs"
      to: "deleted tool.rs + tool_router.rs source files"
      via: "mod tool; mod tool_router; — both removed"
      pattern: "^mod tool(_router)?;"
    - from: "pmcp-macros/src/lib.rs top of file"
      to: "pmcp-macros/README.md"
      via: "include_str! (kept from Wave 0)"
      pattern: "include_str!\\(\"\\.\\./README\\.md\"\\)"
---

<objective>
Delete ALL deprecated and stub proc-macro surface from `pmcp-macros` in a single pass:

1. **Deprecated macros (D-01):** `#[tool]` and `#[tool_router]` — `pmcp-macros/src/tool.rs`
   (426 lines), `pmcp-macros/src/tool_router.rs` (257 lines), their integration tests
   (`tool_tests.rs` 129 lines, `tool_router_tests.rs` 71 lines), their trybuild UI test pair
   (`tool_missing_description.rs` + `.stderr`, Pitfall 6), the `mod` declarations and `pub fn`
   exports in `lib.rs`, their `#[deprecated]` attributes, and the `tool_router_dev` feature
   flag in `Cargo.toml`.

2. **Stub macros (D-02):** `#[prompt]` and `#[resource]` — zero-op identity functions at
   `lib.rs` approximately lines 319-323 and 338-342. These generate no code, have no
   production usage, and mislead users. Delete the `pub fn prompt` / `pub fn resource`
   definitions with their `///` doc comments and `#[proc_macro_attribute]` markers.

3. **Obsolete module-level `//!` docs (D-10 preparation):** The `//!` comments at
   `lib.rs:1-53` advertise the deleted macros and show an obsolete `#[tool_router]` +
   `#[tool]` calculator example. They're dead weight once the `include_str!` pattern is in
   place. Delete them. The include_str! attribute added in Wave 0 (Plan 01) becomes the sole
   crate-level documentation source.

All three deletions happen in ONE plan because they edit overlapping regions of `lib.rs` —
splitting them into separate plans creates a merge conflict between parallel tasks and forces
serialization anyway. One sweep, one commit, cleanest git history.

Purpose: The scope expansion from "docs rewrite" to "macros cleanup + docs rewrite" exists
because leaving 683 lines of deprecated source + stub functions in place while the README
documents the new API would be self-defeating. The CHANGELOG entry in Plan 05 carries the
migration story; the source tree gets to be clean (D-04).

Output: A trimmed `pmcp-macros/src/lib.rs` that contains:
- The `#![doc = include_str!("../README.md")]` attribute at line 1 (from Wave 0)
- The `#[cfg(doctest)] pub struct ReadmeDoctests;` item (from Wave 0)
- `use proc_macro::TokenStream;` + `use syn::...`
- Four `mod mcp_*;` declarations + `mod mcp_common;` + `#[allow(dead_code)] mod utils;`
- Four `pub fn mcp_*` exports with their `///` doc comments (still the old text — Plan 04
  rewrites those)

And gone: `//!` lines 1-53, `mod tool;`, `mod tool_router;`, `pub fn tool`, `pub fn
tool_router`, `pub fn prompt`, `pub fn resource`, all their `///` / `#[deprecated]` /
`#[proc_macro_attribute]` decorations, 6 deleted files, and the `tool_router_dev` feature.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md
@.planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md
@pmcp-macros/src/lib.rs
@pmcp-macros/Cargo.toml

<interfaces>
<!-- Exact line locations the executor must edit. Saves scavenger-hunt time. -->

`pmcp-macros/src/lib.rs` state AFTER Wave 0 Plan 01 lands (at start of this plan):
- Line 1: `#![doc = include_str!("../README.md")]`  ← KEEP
- Line 2: blank
- Line 3: `#[cfg(doctest)]`  ← KEEP
- Line 4: `pub struct ReadmeDoctests;`  ← KEEP
- Line 5: blank
- Lines 6-58 (approx — was lines 1-53 before Wave 0 shifted them by 5): `//!` module doc
  comments advertising deleted macros. DELETE ALL OF THESE.
- Line 59 (approx): `use proc_macro::TokenStream;`  ← KEEP
- Line 60 (approx): `use syn::{parse_macro_input, ItemFn, ItemImpl};`  ← KEEP
- Lines 61-67 (approx): `mod` declarations
  - `mod mcp_common;`  ← KEEP
  - `mod mcp_prompt;`  ← KEEP
  - `mod mcp_resource;`  ← KEEP
  - `mod mcp_server;`  ← KEEP
  - `mod mcp_tool;`  ← KEEP
  - `mod tool;`  ← DELETE
  - `mod tool_router;`  ← DELETE
  - `#[allow(dead_code)] mod utils;`  ← KEEP
- Lines ~68-110 (approx): `///` doc comment + `#[deprecated(since = "0.3.0", ...)]` +
  `#[proc_macro_attribute]` + `pub fn tool(args: TokenStream, input: TokenStream) ->
  TokenStream { ... }`. DELETE ALL.
- Subsequent `pub fn mcp_tool`, `pub fn mcp_server`, `pub fn mcp_prompt`, `pub fn
  mcp_resource` with their `///` docs — KEEP UNTOUCHED (Plan 04 rewrites the docs later).
- Approximately lines 270-304: `///` doc comment + `#[deprecated]` + `#[proc_macro_attribute]`
  + `pub fn tool_router(args, input) -> TokenStream { ... }`. DELETE ALL.
- Approximately lines 306-323: `///` doc comment + `#[proc_macro_attribute]` + `pub fn
  prompt(_args, input) -> TokenStream { input }` (literal identity function, the stub).
  DELETE ALL.
- Approximately lines 325-342: `///` doc comment + `#[proc_macro_attribute]` + `pub fn
  resource(_args, input) -> TokenStream { input }` (literal identity function, the stub).
  DELETE ALL.

Line numbers are approximate — read the file at task execution time, use grep for the anchor
strings listed above, and delete by anchor, not by line number.

`pmcp-macros/Cargo.toml` current state:
- Line 38-41:
  ```
  [features]
  default = []
  debug = []  # Enable debug output in macros
  tool_router_dev = []  # Enable WIP tool_router tests
  ```
  DELETE the `tool_router_dev` line (41). KEEP `default` and `debug`.

Files to delete (full file removal):
- `pmcp-macros/src/tool.rs` (426 lines)
- `pmcp-macros/src/tool_router.rs` (257 lines)
- `pmcp-macros/tests/tool_tests.rs` (129 lines)
- `pmcp-macros/tests/tool_router_tests.rs` (71 lines)
- `pmcp-macros/tests/ui/tool_missing_description.rs` (verified present)
- `pmcp-macros/tests/ui/tool_missing_description.stderr` (verified present)

KEEP (real macros — do not touch):
- `pmcp-macros/src/mcp_common.rs`, `mcp_prompt.rs`, `mcp_resource.rs`, `mcp_server.rs`,
  `mcp_tool.rs`, `utils.rs`
- `pmcp-macros/tests/mcp_prompt_tests.rs`, `mcp_server_tests.rs`, `mcp_tool_tests.rs`
- `pmcp-macros/tests/ui/mcp_prompt_missing_description.rs[.stderr]`
- `pmcp-macros/tests/ui/mcp_tool_missing_description.rs[.stderr]`
- `pmcp-macros/tests/ui/mcp_tool_multiple_args.rs[.stderr]`
</interfaces>
</context>

<notes>
**EXECUTOR SAFETY — MANDATORY READ BEFORE STARTING TASK 2**

This plan intentionally leaves the workspace in a **broken-build state** between Task 1 and
Task 2. This is unavoidable when deleting proc-macro source files that are referenced from
`lib.rs`: the files are removed by `git rm` in Task 1, and `lib.rs` still carries `mod tool;`
and `mod tool_router;` declarations pointing at the now-missing files until Task 2 edits them
away. A `cargo` invocation during that window will fail with "file not found for module
`tool`" or similar — which is expected, not a regression.

**Rules while executing this plan:**

1. **Between Task 1 and Task 2, DO NOT run any of the following:**
   - `cargo check --workspace`
   - `cargo check -p pmcp-macros`
   - `cargo build --workspace`
   - `cargo build -p pmcp-macros`
   - `cargo test`
   - `cargo clippy`
   - `make quality-gate`
   - `make lint`
   - Any other command that invokes the Rust compiler on pmcp-macros or the workspace.

2. **Task 1's verification is file-existence only.** The `<verify>` block intentionally uses
   `! test -f <path>` checks (plus `git status` staged-deletions check) to confirm the
   deletions landed. Do NOT add `cargo check` or `cargo build` to Task 1's verify step — the
   crate is not whole yet and cannot compile.

3. **Task 1's commit will almost certainly trip the repo's pre-commit hook.** The PMCP
   pre-commit hook runs `make quality-gate` / `cargo check` per CLAUDE.md. Expected handling:
   - First, try `git commit` normally and observe the failure.
   - If the failure is a compile error in pmcp-macros (expected — lib.rs still references
     deleted files), use the documented PMCP "Emergency Override" pattern from CLAUDE.md:
     `git commit --no-verify -m "refactor(66): delete 6 deprecated macro files (WIP — lib.rs cleanup follows in next commit)"`
     This is justified here because Task 2 of this same plan immediately restores quality
     before any other work can run, and CLAUDE.md explicitly lists "critical hotfixes"
     and staged multi-commit refactors as valid override cases provided the next commit
     restores quality standards.
   - Do NOT squash Task 1 and Task 2 into a single commit; they are deliberately separate so
     the raw deletion is easy to review and revert if the lib.rs edit in Task 2 goes wrong.

4. **The first real `cargo` invocation of this plan is at the END of Task 2**, after
   `lib.rs` has been updated to drop the `mod tool;`, `mod tool_router;`, `pub fn tool`,
   `pub fn tool_router`, `pub fn prompt`, `pub fn resource` regions. At that point the
   crate is whole again and `cargo build -p pmcp-macros` must succeed.

5. **If Task 1 succeeds but Task 2 fails** (e.g. an edit boundary chops into a real
   `mcp_*` function), the repo is in a broken state. Recovery: `git revert HEAD` to undo
   Task 1's commit, then restart the plan from Task 1 after re-reading the `<interfaces>`
   block for line anchors.

This safety note exists because Gemini's cross-AI plan review (2026-04-11) flagged the
intermediate broken-build window as a LOW-severity risk for an executor that reflexively
runs `cargo check` after every file change. The note makes the expected behaviour explicit
so the executor does not panic when Task 1's commit fails pre-commit or when `cargo check`
would fail between tasks.
</notes>

<tasks>

<task type="auto">
  <name>Task 1: Delete 6 files (tool.rs, tool_router.rs, tests, UI test pair)</name>
  <files>
    pmcp-macros/src/tool.rs,
    pmcp-macros/src/tool_router.rs,
    pmcp-macros/tests/tool_tests.rs,
    pmcp-macros/tests/tool_router_tests.rs,
    pmcp-macros/tests/ui/tool_missing_description.rs,
    pmcp-macros/tests/ui/tool_missing_description.stderr
  </files>

  <read_first>
    - pmcp-macros/src/tool.rs (head -20 — confirm it's the deprecated 426-line impl)
    - pmcp-macros/src/tool_router.rs (head -20 — same)
    - pmcp-macros/tests/tool_tests.rs (head -20 — confirm tests target `#[tool]`, not `#[mcp_tool]`)
    - pmcp-macros/tests/tool_router_tests.rs (head -20 — same for `#[tool_router]`)
    - pmcp-macros/tests/ui/tool_missing_description.rs (confirm it's the `#[tool]` trybuild UI test — Pitfall 6)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-01)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md section "Pitfall 6"
      (explains why both `.rs` AND `.stderr` must go — trybuild stderr-diff failure otherwise)
    - The plan-level `<notes>` block above titled "EXECUTOR SAFETY — MANDATORY READ BEFORE
      STARTING TASK 2" — it documents the expected broken-build window between this task
      and Task 2 and the pre-commit hook override protocol for this task's commit.
  </read_first>

  <action>
    Delete the following six files using `git rm`:

    ```
    git rm pmcp-macros/src/tool.rs
    git rm pmcp-macros/src/tool_router.rs
    git rm pmcp-macros/tests/tool_tests.rs
    git rm pmcp-macros/tests/tool_router_tests.rs
    git rm pmcp-macros/tests/ui/tool_missing_description.rs
    git rm pmcp-macros/tests/ui/tool_missing_description.stderr
    ```

    Do NOT delete any `mcp_*` files — the list of KEEP files is in the `<interfaces>` block
    above.

    **DO NOT run any `cargo` command after these deletions.** See the plan-level `<notes>`
    section for the full executor-safety protocol. The `lib.rs` `mod tool;` and
    `mod tool_router;` declarations still reference these now-missing files, so `cargo build`,
    `cargo check`, `cargo test`, `cargo clippy`, `make quality-gate`, `make lint`, or any
    similar compiler-invoking command will fail with "file not found for module `tool`" until
    Task 2 restores lib.rs. That failure is expected, not a regression — Task 2 is where the
    build gets restored.

    **Commit handling:** Commit the deletions as a standalone commit so the history stays
    clean. Subject line:
    `refactor(66): delete 6 deprecated macro files (WIP — lib.rs cleanup follows in next commit)`

    The PMCP pre-commit hook will likely reject this commit because it runs `cargo check` /
    `make quality-gate` as part of the Toyota Way gate. When that happens, use the Emergency
    Override pattern documented in CLAUDE.md:
    `git commit --no-verify -m "refactor(66): delete 6 deprecated macro files (WIP — lib.rs cleanup follows in next commit)"`
    This is the documented justification for `--no-verify` (a staged multi-commit refactor
    whose next commit immediately restores quality). Do NOT squash Task 1 and Task 2 into one
    commit — they are deliberately separate so the raw deletion is easy to review and revert
    independently.
  </action>

  <acceptance_criteria>
    - `! test -f pmcp-macros/src/tool.rs`
    - `! test -f pmcp-macros/src/tool_router.rs`
    - `! test -f pmcp-macros/tests/tool_tests.rs`
    - `! test -f pmcp-macros/tests/tool_router_tests.rs`
    - `! test -f pmcp-macros/tests/ui/tool_missing_description.rs`
    - `! test -f pmcp-macros/tests/ui/tool_missing_description.stderr`
    - `test -f pmcp-macros/src/mcp_tool.rs` (real macro — NOT deleted)
    - `test -f pmcp-macros/src/mcp_server.rs` (real macro — NOT deleted)
    - `test -f pmcp-macros/src/mcp_prompt.rs` (real macro — NOT deleted)
    - `test -f pmcp-macros/src/mcp_resource.rs` (real macro — NOT deleted)
    - `test -f pmcp-macros/tests/mcp_tool_tests.rs` (real macro's tests — NOT deleted)
    - `test -f pmcp-macros/tests/ui/mcp_tool_missing_description.rs` (real macro's UI test — NOT deleted)
    - `git status --porcelain | grep -E '^D ' | wc -l` reports ≥ 6 (six or more staged deletions)
  </acceptance_criteria>

  <verify>
    <automated>! test -f pmcp-macros/src/tool.rs && ! test -f pmcp-macros/src/tool_router.rs && ! test -f pmcp-macros/tests/tool_tests.rs && ! test -f pmcp-macros/tests/tool_router_tests.rs && ! test -f pmcp-macros/tests/ui/tool_missing_description.rs && ! test -f pmcp-macros/tests/ui/tool_missing_description.stderr && test -f pmcp-macros/src/mcp_tool.rs && test -f pmcp-macros/src/mcp_server.rs && test -f pmcp-macros/tests/mcp_tool_tests.rs</automated>
  </verify>

  <done>
    All six deprecated source and test files are deleted via git. All `mcp_*` real-macro files
    remain. Build is deliberately broken at this checkpoint — Task 2 restores it. No `cargo`
    command has been run since the deletions.
  </done>
</task>

<task type="auto">
  <name>Task 2: Clean up lib.rs (delete //! header, mod tool/tool_router, pub fn tool/tool_router/prompt/resource) and Cargo.toml (drop tool_router_dev feature)</name>
  <files>pmcp-macros/src/lib.rs, pmcp-macros/Cargo.toml</files>

  <read_first>
    - The plan-level `<notes>` block above titled "EXECUTOR SAFETY — MANDATORY READ BEFORE
      STARTING TASK 2" — THIS task is what restores the broken build left by Task 1. Do not
      skip the notes; they explain why no `cargo` invocation has run yet and where the first
      one belongs (at the end of this task's action block, after all lib.rs regions are
      edited).
    - pmcp-macros/src/lib.rs (READ THE ENTIRE FILE — this task edits 6 distinct regions
      precisely. Do not attempt blind sed; use Read then Edit operations anchored to exact
      strings listed below.)
    - pmcp-macros/Cargo.toml (confirm the features table still has `tool_router_dev = []`)
    - .planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md (D-01, D-02, D-04, D-10,
      D-12)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md "Architecture Patterns
      — Target file layout after this phase" (shows what lib.rs should look like when done)
    - .planning/phases/66-macros-documentation-rewrite/66-RESEARCH.md "Anti-Patterns to Avoid"
      section — explicitly calls out the `//!` line 1-53 deletion as critical
  </read_first>

  <action>
    **Pre-check:** Confirm that Task 1 has run and that no `cargo` command has been invoked
    in between (see plan-level `<notes>`). The workspace is currently in the expected
    broken-build state; this task's actions are what make it whole again.

    Edit `pmcp-macros/src/lib.rs` — six surgical deletions:

    **Region 1: Top-of-file `//!` module docs (lines after Wave 0 wiring, approximately a
    53-line block).**

    After Wave 0, the file starts with:
    ```
    #![doc = include_str!("../README.md")]

    #[cfg(doctest)]
    pub struct ReadmeDoctests;

    //! Procedural macros for PMCP SDK
    //!
    //! This crate provides attribute macros to reduce boilerplate when implementing
    ...
    //! }
    //! ```
    ```

    Delete EVERY contiguous line starting with `//!` (including the lines with just `//!` and
    no trailing text). There are approximately 53 such lines. They form one block. Do NOT
    delete the `#![doc = include_str!(...)]` line, the blank line after it, the `#[cfg(doctest)]`
    attribute, or the `pub struct ReadmeDoctests;` line — those are from Wave 0 and they stay.

    After this region's deletion, the first non-`//!` line (currently
    `use proc_macro::TokenStream;`) should appear immediately after `pub struct ReadmeDoctests;`
    (separated by a single blank line).

    **Region 2: `mod tool;` and `mod tool_router;` declarations.**

    Grep for them. Delete the two lines:
    ```
    mod tool;
    mod tool_router;
    ```

    Do NOT delete `mod mcp_common;`, `mod mcp_prompt;`, `mod mcp_resource;`, `mod mcp_server;`,
    `mod mcp_tool;`, `#[allow(dead_code)] mod utils;` — these are real.

    **Region 3: `pub fn tool` block.**

    Locate the block that starts with `/// Defines a tool handler with automatic schema
    generation.` and includes:
    - A multi-line `///` doc comment
    - `#[deprecated(since = "0.3.0", note = "Use #[mcp_tool] instead — ...")]`
    - `#[proc_macro_attribute]`
    - `pub fn tool(args: TokenStream, input: TokenStream) -> TokenStream { ... }` (short body,
      probably 5-10 lines calling `tool::expand_tool(...)`).

    Delete this entire block (approximately lines 68-110 pre-deletion, shifted by the Wave 0
    +5-line insertion). Do NOT confuse this with the `pub fn mcp_tool` block that appears
    later — that one has doc starting with something like `/// Attribute macro for defining
    a tool handler with enhanced DX` or similar and has NO `#[deprecated]` attribute.

    **Region 4: `pub fn tool_router` block.**

    Locate the block starting with the `///` doc comment for `tool_router` — approximately at
    pre-deletion line 270. Includes the `///` comment, `#[deprecated(since = "0.3.0", ...)]`,
    `#[proc_macro_attribute]`, and `pub fn tool_router(args: TokenStream, input: TokenStream)
    -> TokenStream { ... }`. Delete the entire block.

    Do NOT confuse with `pub fn mcp_server` which is the functional replacement — that one has
    NO `#[deprecated]` attribute.

    **Region 5: `pub fn prompt` stub block.**

    Locate the block starting with `/// Defines a prompt template with typed arguments.`
    (approximately pre-deletion line 306). Includes:
    - Multi-line `///` doc comment with a `rust,ignore` example using `#[prompt(...)]`
    - `#[proc_macro_attribute]`
    - `pub fn prompt(_args: TokenStream, input: TokenStream) -> TokenStream {`
    - `    // Prompt macro implementation deferred to future release`
    - `    input`
    - `}`

    Delete the entire block. Do NOT confuse with `pub fn mcp_prompt` (the real one — no
    "deferred to future release" comment).

    **Region 6: `pub fn resource` stub block.**

    Locate the block starting with `/// Defines a resource handler with URI pattern matching.`
    (approximately pre-deletion line 325). Includes:
    - Multi-line `///` doc comment with a `rust,ignore` example using `#[resource(...)]`
    - `#[proc_macro_attribute]`
    - `pub fn resource(_args: TokenStream, input: TokenStream) -> TokenStream {`
    - `    // Resource macro implementation deferred to future release`
    - `    input`
    - `}`

    Delete the entire block. Do NOT confuse with `pub fn mcp_resource`.

    ---

    Edit `pmcp-macros/Cargo.toml`:

    Delete line 41 exactly:
    ```
    tool_router_dev = []  # Enable WIP tool_router tests
    ```

    The `[features]` section should read after the edit:
    ```
    [features]
    default = []
    debug = []  # Enable debug output in macros
    ```

    ---

    Verify-then-commit (this is where the plan's first `cargo` invocation lands):

    Run `cargo build -p pmcp-macros`. If it fails, the error tells you exactly what residual
    reference is left (probably an orphan `tool::` or `tool_router::` import somewhere that
    got missed, or an edit boundary that accidentally chopped a real `mcp_*` function). Fix
    until clean.

    Run `cargo test -p pmcp-macros`. All remaining `mcp_*` tests must pass. The trybuild UI
    tests for `mcp_*` stay green — they don't reference `#[tool]` or `#[tool_router]`.

    Run `cargo test -p pmcp-macros --doc`. The Wave 0 POC README doctests must still pass.

    Commit with this exact subject (create a NEW commit — never amend Wave 0 or Task 1):
    `refactor(66): delete deprecated #[tool]/#[tool_router] and stub #[prompt]/#[resource]`
  </action>

  <acceptance_criteria>
    - `! grep -n '^//! ' pmcp-macros/src/lib.rs` (zero `//!` lines at top of file; wipe check)
    - `! grep -E '^mod tool;$|^mod tool_router;$' pmcp-macros/src/lib.rs`
    - `! grep 'pub fn tool(' pmcp-macros/src/lib.rs`
    - `! grep 'pub fn tool_router(' pmcp-macros/src/lib.rs`
    - `! grep 'pub fn prompt(' pmcp-macros/src/lib.rs`
    - `! grep 'pub fn resource(' pmcp-macros/src/lib.rs`
    - `! grep '#\[deprecated' pmcp-macros/src/lib.rs`
    - `! grep 'deferred to future release' pmcp-macros/src/lib.rs`
    - `grep -q '#!\[doc = include_str!("../README.md")\]' pmcp-macros/src/lib.rs` (Wave 0 wiring preserved)
    - `grep -q 'pub struct ReadmeDoctests' pmcp-macros/src/lib.rs` (Wave 0 item preserved)
    - `grep -q 'pub fn mcp_tool(' pmcp-macros/src/lib.rs` (real macro preserved)
    - `grep -q 'pub fn mcp_server(' pmcp-macros/src/lib.rs` (real macro preserved)
    - `grep -q 'pub fn mcp_prompt(' pmcp-macros/src/lib.rs` (real macro preserved)
    - `grep -q 'pub fn mcp_resource(' pmcp-macros/src/lib.rs` (real macro preserved)
    - `grep -q '^mod mcp_tool;' pmcp-macros/src/lib.rs` (real mod decl preserved)
    - `grep -q '^mod mcp_server;' pmcp-macros/src/lib.rs`
    - `grep -q '^mod mcp_prompt;' pmcp-macros/src/lib.rs`
    - `grep -q '^mod mcp_resource;' pmcp-macros/src/lib.rs`
    - `! grep 'tool_router_dev' pmcp-macros/Cargo.toml`
    - `grep -q '^default = \[\]' pmcp-macros/Cargo.toml` (default feature preserved)
    - `grep -q '^debug = \[\]' pmcp-macros/Cargo.toml` (debug feature preserved)
    - `cargo build -p pmcp-macros` exits 0
    - `cargo test -p pmcp-macros` exits 0
    - `cargo test -p pmcp-macros --doc` exits 0 (Wave 0 POC still works)
    - `git log -1 --pretty=%s` matches `refactor(66): delete deprecated #\[tool\]/#\[tool_router\] and stub #\[prompt\]/#\[resource\]`
  </acceptance_criteria>

  <verify>
    <automated>! grep -n '^//! ' pmcp-macros/src/lib.rs && ! grep -E '^mod tool;$|^mod tool_router;$' pmcp-macros/src/lib.rs && ! grep 'pub fn tool(' pmcp-macros/src/lib.rs && ! grep 'pub fn tool_router(' pmcp-macros/src/lib.rs && ! grep 'pub fn prompt(' pmcp-macros/src/lib.rs && ! grep 'pub fn resource(' pmcp-macros/src/lib.rs && ! grep '#\[deprecated' pmcp-macros/src/lib.rs && grep -q 'pub fn mcp_tool(' pmcp-macros/src/lib.rs && grep -q 'pub fn mcp_server(' pmcp-macros/src/lib.rs && grep -q 'pub fn mcp_prompt(' pmcp-macros/src/lib.rs && grep -q 'pub fn mcp_resource(' pmcp-macros/src/lib.rs && ! grep 'tool_router_dev' pmcp-macros/Cargo.toml && cargo build -p pmcp-macros 2>&1 | tail -3 && cargo test -p pmcp-macros 2>&1 | tail -5 && cargo test -p pmcp-macros --doc 2>&1 | tail -5</automated>
  </verify>

  <done>
    `pmcp-macros/src/lib.rs` contains only the Wave 0 wiring + real `mcp_*` module declarations
    and `pub fn` exports + essential `use` statements. No `//!` top-of-file comments, no
    deprecated surfaces, no stub identity functions. `Cargo.toml` features are clean. Both
    `cargo build` and `cargo test` pass. Workspace is whole again.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| None | Compile-time deletion of dead code + stub functions. No runtime surface affected. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-66-03 | Tampering | Downstream users on `pmcp-macros = "0.4"` | accept | Semver-legal pre-1.0 breaking minor bump (D-20). Migration documented in `pmcp-macros/CHANGELOG.md` v0.5.0 (Plan 05). v0.4.x stays published on crates.io forever — users pin until they opt into v0.5.0. |
| T-66-04 | Information Disclosure | git history of deleted files | accept | Standard `git rm` — history preserved, files gone from HEAD. No secrets in deleted content (verified: all six files are macro codegen + tests, no env vars or credentials). |
| T-66-05 | Tampering | Orphan users of `#[prompt]` / `#[resource]` stubs who expect them to "work" | accept | These macros were pure identity functions generating zero code (verified in `<interfaces>` block). Any user applying them today gets zero behavior; deleting them surfaces the bug as a compile error, which is strictly more honest. |

Docs/deletion phase — security surface is zero.
</threat_model>

<verification>
- `cargo build -p pmcp-macros` passes
- `cargo test -p pmcp-macros` passes (mcp_* tests still green)
- `cargo test -p pmcp-macros --doc` passes (Wave 0 POC still compiles)
- Six files deleted per `git status`
- `pmcp-macros/src/lib.rs` is trimmed to only real-macro content + Wave 0 wiring
- `pmcp-macros/Cargo.toml` features table cleaned
</verification>

<success_criteria>
- Deprecated `#[tool]` and `#[tool_router]` macros fully removed (source + tests)
- Stub `#[prompt]` and `#[resource]` identity functions fully removed
- Top-of-file `//!` module docs deleted (single source of truth = README via `include_str!`)
- Crate still builds cleanly
- Real `#[mcp_*]` macros completely untouched
- `tool_router_dev` feature flag gone
- Ready for Plan 04 (Wave 2) to rewrite README and per-macro `///` docs
</success_criteria>

<output>
After completion, create `.planning/phases/66-macros-documentation-rewrite/66-02-delete-deprecated-macros-SUMMARY.md`
documenting: (a) SHA of the commit, (b) line counts of the 6 deleted files (exact numbers from
git rm stats), (c) `cargo build` / `cargo test` / `cargo test --doc` output confirming all
three gates pass, (d) any unexpected references that had to be cleaned up beyond the
six regions listed above, (e) the final line count of `pmcp-macros/src/lib.rs`
(should be roughly 200-220 lines after trimming, down from ~340 pre-edit).
</output>
