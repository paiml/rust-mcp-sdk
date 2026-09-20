# Phase 66: Macros Cleanup + Documentation Rewrite - Research

**Researched:** 2026-04-11
**Domain:** Rust proc-macro crate documentation, `include_str!` doctest mechanics, CHANGELOG conventions, release coordination
**Confidence:** HIGH

## Summary

This phase is primarily a **deletion + documentation rewrite**, not a library selection. Research focused on the narrow Rust-ecosystem questions that genuinely need verification: (1) how `#![doc = include_str!("../README.md")]` behaves on a `proc-macro = true` crate, (2) whether `rust,no_run` code blocks inside that included README can actually compile against the parent `pmcp` crate, (3) whether `cargo-rdme`-style tooling should replace the hand-rolled `include_str!` approach, and (4) what the explicit reference target `rmcp` actually does.

The short version: **every one of the 25 locked decisions in 66-CONTEXT.md holds up against Rust ecosystem practice, with one meaningful addition — the precise mechanism that makes `rust,no_run` doctests in the included README actually compilable is the dev-dependency on `pmcp` that pmcp-macros already has, combined with writing doctests as `use pmcp::{mcp_tool, mcp_server, mcp_prompt, mcp_resource};` rather than `use pmcp_macros::...`**. This exact pattern is battle-tested in `tracing-attributes` (a widely-used proc-macro crate with identical structure) and should be copied verbatim. `cargo-rdme`/`cargo-sync-rdme` flow the *wrong* direction for this phase and should NOT be adopted.

**Primary recommendation:** Write the new README's code blocks as `rust,no_run` with imports from `pmcp` (not `pmcp_macros`), wire via `#![doc = include_str!("../README.md")]` at the top of `pmcp-macros/src/lib.rs`, add an adjacent `#[cfg(doctest)] pub struct ReadmeDoctests;` item so the README doctests actually execute under `cargo test --doc`, create `pmcp-macros/CHANGELOG.md` following the same Keep a Changelog format as the workspace root, and bump versions in the order specified in CLAUDE.md (pmcp-macros first, then pmcp).

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Macro deletion scope:**
- **D-01:** Delete `#[tool]` and `#[tool_router]` entirely — `pmcp-macros/src/tool.rs` (426 lines), `pmcp-macros/src/tool_router.rs` (257 lines), their tests (`pmcp-macros/tests/tool_tests.rs` 129 lines, `pmcp-macros/tests/tool_router_tests.rs` 71 lines), the `pmcp-macros/tests/ui/tool_missing_description.rs` UI test if any, the `mod tool` / `mod tool_router` declarations in `lib.rs`, and the `pub fn tool` / `pub fn tool_router` exports with their `#[deprecated]` attrs.
- **D-02:** Delete `#[prompt]` and `#[resource]` entirely — identity-function stubs at `pmcp-macros/src/lib.rs:319-323` and `:338-342`. Delete the `pub fn prompt` / `pub fn resource` definitions plus the `//!` lines at `lib.rs:10-11` that advertise them.
- **D-03:** Keep `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` — real, functional, primary API. No changes to their implementation in this phase.
- **D-04:** No transitional "please use `#[mcp_tool]`" compile errors or hint shims. CHANGELOG carries the migration story; source tree stays clean.

**README structure and audience:**
- **D-05:** Target audience is **new users**, not migrators. The word "migration" does not appear in the README.
- **D-06:** Single unified README covering all four `mcp_*` macros in one document, grouped by macro with consistent section structure per entry.
- **D-07:** Proportional depth — `#[mcp_tool]` is the showcase, `#[mcp_server]` gets full treatment, `#[mcp_prompt]` and `#[mcp_resource]` get focused sections.
- **D-08:** Installation uses **current pinned versions** — `pmcp = "2.3"` with `features = ["macros"]`. No `pmcp-macros` as a direct dependency in user-facing examples.
- **D-09:** Every README code block must compile. `rust,no_run` is the default — `rust,ignore` is forbidden for in-README code.

**lib.rs doc strategy:**
- **D-10:** `pmcp-macros/src/lib.rs` uses `#![doc = include_str!("../README.md")]` as the top-level module doc. Delete all existing `//!` comments at lines 1–53. Single source of truth: README.md.
- **D-11:** Per-macro `///` doc comments on `pub fn mcp_tool`, `pub fn mcp_server`, etc. stay in place and get rewritten. Update examples to reference `s23_mcp_tool_macro`, `s24_mcp_prompt_macro` and flip `rust,ignore` → `rust,no_run` where possible.
- **D-12:** Stale per-macro doc on `pub fn tool` goes away when the function itself is deleted.

**Migration surface (external to README):**
- **D-13:** Create or update `pmcp-macros/CHANGELOG.md` with a v0.5.0 entry (Breaking + Migration subsections).
- **D-14:** If no `pmcp-macros/CHANGELOG.md` exists today, create it. (Verified — it does not exist.)
- **D-15:** `pmcp`'s top-level `CHANGELOG.md` gets a v2.3.0 entry.
- **D-16:** `docs/advanced/migration-from-typescript.md` gets updated to show current `#[mcp_tool]` syntax wherever it currently shows `#[tool]`.

**Downstream consumer updates (pmcp-course):**
- **D-17:** Four pmcp-course chapters use `#[tool(...)]` today — update all to `#[mcp_tool(...)]`. Locations verified: `pmcp-course/src/part1-foundations/ch01-03-why-rust.md:118`, `part5-security/ch13-oauth.md:177,294`, `ch13-02-oauth-basics.md:243`, `ch13-03-validation.md:78`.
- **D-18:** Phase 66 is responsible for these course updates. Do NOT defer.
- **D-19:** Course chapters that currently advertise `#[mcp_tool]` as the "v2.0 Tip" stay unchanged.

**Release coordination:**
- **D-20:** `pmcp-macros` version: v0.4.1 → v0.5.0. Pre-1.0 semver allows breaking changes at minor bumps.
- **D-21:** `pmcp` version: v2.2.0 → v2.3.0. Re-exported public API is unchanged; bumping signals users to check the CHANGELOG.
- **D-22:** Both releases ship in a single PR, `pmcp-macros` publishes before `pmcp` per CLAUDE.md release order.

### Claude's Discretion

- Exact prose and tone of the rewritten README (professional, concrete, example-first)
- Whether to include a "feature flags" table in the README or rely on docs.rs's auto-generated feature badges (phase 67's domain)
- Visual ordering of the four macros in the README — start with `#[mcp_tool]` since it's the most common, but exact sequencing is editorial
- Whether to include a brief "why proc macros for MCP" intro paragraph, or jump straight to usage
- Whether to show sync and async variants separately or in the same code block
- Exact CHANGELOG.md formatting (keepachangelog.com vs project's existing style — check main CHANGELOG.md for convention)

### Deferred Ideas (OUT OF SCOPE)

- **`mcp_resource` re-export gap** at `src/lib.rs:147` — missing from `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool}`. Adding it is a behavioral change, not cleanup. File as a standalone backlog item or small follow-up phase.
- **`include_str!("../README.md")` pattern for other workspace crates** (`mcp-tester`, `mcp-preview`, `cargo-pmcp`, `pmcp` itself) — out of scope for phase 66, candidate for phase 67 or 68.
- **Automated migration tooling** (e.g., `cargo pmcp migrate` subcommand) — user explicitly did not ask for this; hand migration via CHANGELOG is fine given pre-1.0 `pmcp-macros` user count.
- **Macro feature additions** (new attributes, new codegen capabilities) — not cleanup, not documentation. Separate phase if ever needed.
- **Moving macro implementations into `pmcp` core** — rejected because proc macros must live in a `proc-macro = true` crate per Rust compiler rules.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MACR-01 | pmcp-macros README rewritten to document #[mcp_tool], #[mcp_server], #[mcp_prompt], #[mcp_resource] as primary APIs with working examples | Standard Stack section specifies `rust,no_run` + `use pmcp::...` pattern that makes "working examples" mechanically verifiable via `cargo test --doc -p pmcp-macros`; rmcp Benchmark section specifies which rmcp choices to match vs diverge from |
| MACR-02 | Migration section guiding users from deprecated #[tool]/#[tool_router] to #[mcp_tool]/#[mcp_server] | Per CONTEXT D-05, this migration content lives in `pmcp-macros/CHANGELOG.md` v0.5.0 entry, NOT in the README itself — CHANGELOG section below specifies exact Keep a Changelog format and subsection structure matching the workspace root convention |
| MACR-03 | pmcp-macros lib.rs uses include_str!("../README.md") so docs.rs shows the rewritten README | Architecture Patterns section gives the exact 3-line snippet (module doc + optional `ReadmeDoctests` struct + existing `mod` declarations), with confirmed-working reference implementation in `tracing-attributes` |

**Note on MACR-02 vs CONTEXT.md D-05:** The requirement text says "migration section" without specifying *where*. D-05 locks the decision that migration content lives in CHANGELOG.md, not README.md. This is consistent with `rmcp-macros`' approach (no migration section in README; breaking changes tracked in CHANGELOG), and the requirement is satisfied by the CHANGELOG entry. Plan-phase should NOT add a migration section to the README.

## Project Constraints (from CLAUDE.md)

These directives apply to Phase 66 execution and MUST be honored by the planner:

- **Zero defects policy**: `make quality-gate` must pass before any commit. Phase 66 does not change runtime code, so the main gates exercised will be `cargo fmt --all -- --check`, `cargo clippy` (workspace), and `cargo test --doc` (which becomes relevant once `include_str!("../README.md")` is wired).
- **Release & Publish Workflow** (CLAUDE.md § "Release & Publish Workflow"): The authoritative publish order for this phase is `pmcp-widget-utils` → **`pmcp-macros`** → **`pmcp`** → `mcp-tester` → `mcp-preview` → `cargo-pmcp`. Phase 66 only touches `pmcp-macros` and `pmcp`, so the relevant sub-order is: bump `pmcp-macros` first, then update `pmcp`'s dep pin + bump `pmcp`'s version. One PR, separate commits per crate, per the convention in CLAUDE.md's release workflow example.
- **Version bump rules**: "Only bump crates that have changed since their last publish. Downstream crates that pin a bumped dependency must also be bumped." → `pmcp-macros` bumps because its public API shrinks (D-01, D-02). `pmcp` bumps because it pins `pmcp-macros` in `Cargo.toml:53` (optional dep) and `Cargo.toml:147` (dev-dep for examples); both pins must move to `"0.5.0"`.
- **`make quality-gate` not bare `cargo clippy`**: CLAUDE.md explicitly warns against running individual cargo commands — `make quality-gate` is what CI runs with pedantic + nursery lints. Plan's verification steps MUST use `make quality-gate`, not `cargo clippy -- -D warnings`.
- **Pre-commit hook enforces quality gates**: Expect the pre-commit hook to run on every commit in this phase. Any README rewrite that breaks doctest compilation (e.g., a stray `use pmcp_macros::mcp_tool;` inside a `rust,no_run` block) will block commit.
- **Tag convention**: Tags use `v` prefix. Phase 66's release tag would be `v2.3.0` (for `pmcp`) — the release workflow publishes all bumped crates from a single tag.
- **Contract-first development** (CLAUDE.md § "Contract-First Development"): Not applicable to this phase — no new features or bug fixes in runtime code. Pure documentation + deletion.

## Standard Stack

This is a documentation + deletion phase. The "stack" is the set of Rust ecosystem tools and patterns used to wire the README-as-crate-doc pattern correctly.

### Core

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| `#![doc = include_str!("...")]` | rustc 1.54+ (built in) | Include README as crate-level rustdoc | `[CITED: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html]` — official pattern endorsed in rustdoc book, used by `tracing-attributes` and many other proc-macro crates `[VERIFIED: rmcp-macros/src/lib.rs line 1]` |
| `#[cfg(doctest)] pub struct ReadmeDoctests;` | rustc 1.54+ (built in) | Execute README doctests under `cargo test --doc` without polluting public API | `[CITED: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html]` — official rustdoc book idiom, quote: "This will include your README as documentation on the hidden struct `ReadmeDoctests`, which will then be tested alongside the rest of your doctests." |
| `rust,no_run` code block attribute | built in | Compile but don't execute doctests — catches API drift without requiring a live server | `[CITED: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html]` — quote: "The `no_run` attribute will compile your code but not run it." |
| Keep a Changelog 1.0.0 | N/A (spec) | CHANGELOG.md format | `[VERIFIED: /Users/guy/Development/mcp/sdk/rust-mcp-sdk/CHANGELOG.md:5]` — workspace root CHANGELOG.md already declares "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)". Also used by `rmcp-macros/CHANGELOG.md` `[CITED: raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/CHANGELOG.md]`. |
| `pmcp` dev-dependency (`features = ["full"]`) | path = "..", already present | Lets doctests use `use pmcp::{mcp_tool, mcp_server, ...}` to invoke proc-macros | `[VERIFIED: pmcp-macros/Cargo.toml:27]` — already declared as `pmcp = { version = ">=1.20.0", path = "..", features = ["full"] }`. No config change needed. |

### Supporting

| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| `cargo test --doc -p pmcp-macros` | cargo 1.94+ | Verify every `rust,no_run` block in README (and per-macro `///` docs) compiles | Every commit in this phase, via the pre-commit hook's `make quality-gate` |
| `cargo doc -p pmcp-macros --open` | cargo 1.94+ | Manually verify the rendered README looks correct on local docs.rs approximation | Task verification steps for the README rewrite |
| `cargo fmt --all -- --check` | cargo 1.94+ | Workspace format check | Automatic via `make quality-gate` / pre-commit hook |

### Alternatives Considered

| Instead of | Could Use | Tradeoff | Decision |
|------------|-----------|----------|----------|
| `#![doc = include_str!("../README.md")]` (README is source, lib.rs pulls it in) | `cargo-rdme` / `cargo-sync-rdme` (rustdoc comments are source, README is generated) | `cargo-rdme` flows the OPPOSITE direction — it **generates** `README.md` FROM rustdoc comments. That forces writing all README content inside `//!` and `///` comments, which breaks the phase's goal of having one hand-written, prose-rich README. `cargo-sync-rdme` also requires nightly Rust (blocker — workspace uses stable). `[CITED: github.com/orium/cargo-rdme/blob/main/README.md]` `[CITED: github.com/gifnksm/cargo-sync-rdme]` | **REJECT both.** Stick with `include_str!`. |
| `rust,no_run` | `rust,ignore` | `ignore` is invisible to `cargo test --doc` — that's exactly how the current README ended up with `pmcp = "1.1"` at workspace v2.2.0. The rustdoc book explicitly calls this out: "`ignore` is almost never what you want as it's the most generic." `[CITED: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html]` | **REJECT `ignore`.** Per D-09, `rust,ignore` is forbidden for in-README code. |
| Per-macro `///` docs as `rust,no_run` | Per-macro `///` docs as plain ` ```rust ` (run + compile) | `rust` (plain) would attempt to execute the doctest, which for tool-handler examples means running a nonexistent server. `no_run` is the right level. Note `tracing-attributes` uses plain `rust` for its `#[instrument]` docs because its examples don't need a runtime. Tool examples DO need a runtime to actually execute, so `no_run` is correct. | **Use `rust,no_run`** for all per-macro `///` doc examples AND all in-README code blocks. |
| Creating `pmcp-macros/CHANGELOG.md` as a brand-new file | Adding a `### pmcp-macros 0.5.0` subsection to the workspace root `CHANGELOG.md` only | The workspace root already has a pattern of multi-crate subsections within one CHANGELOG entry (see 2026-04-06 entry which contains `### pmcp 2.2.0`, `### pmcp-macros 0.4.1`, `### mcp-tester 0.5.0`, `### cargo-pmcp 0.6.0` as sub-headings of a single `## [2.2.0]` release). However, D-13 and D-14 explicitly require a **standalone `pmcp-macros/CHANGELOG.md`** — this matches what `rmcp-macros` does and gives docs.rs a dedicated per-crate changelog. Both patterns are valid in the Rust ecosystem; D-13/D-14 have locked the choice. | **Create `pmcp-macros/CHANGELOG.md` as a new file AND add the multi-crate subsection to the root CHANGELOG.md**, matching the established 2026-04-06 pattern (D-15 confirms the root entry). The two are not mutually exclusive. |

**Installation verification commands** (for planner to fold into Wave 0):
```bash
# Verify the dev-dependency link that makes rust,no_run doctests compilable
grep -n '^pmcp = ' pmcp-macros/Cargo.toml
# Expected: pmcp = { version = ">=1.20.0", path = "..", features = ["full"] }

# Count current rust,ignore vs rust,no_run usage (baseline for the flip)
grep -rn 'rust,ignore' pmcp-macros/src/ pmcp-macros/README.md | wc -l  # Expected: 22 lines
grep -rn 'rust,no_run' pmcp-macros/src/ pmcp-macros/README.md | wc -l  # Expected: 0 lines

# Verify no existing CHANGELOG.md to confuse the create step
test ! -f pmcp-macros/CHANGELOG.md && echo "confirmed missing"
```

**Version verification** (done during research):
- `pmcp-macros` current version: `0.4.1` `[VERIFIED: pmcp-macros/Cargo.toml:3]`
- `pmcp` current version: checked workspace root Cargo.toml, v2.2.0 → v2.3.0 target per D-21
- `pmcp-macros` pins in workspace: lines 53 and 147 of root `Cargo.toml` `[VERIFIED]`
- Rust toolchain: `cargo 1.94.1 / rustc 1.94.1` `[VERIFIED: local cargo/rustc --version]` — well above the 1.54 minimum for `#[doc = include_str!]` and the 1.82.0 minimum declared in `pmcp-macros/Cargo.toml`.

## Architecture Patterns

### Target file layout after this phase

```
pmcp-macros/
├── Cargo.toml                  # version bumped 0.4.1 → 0.5.0, [features] cleaned
├── CHANGELOG.md                # NEW — Keep a Changelog format, v0.5.0 entry
├── README.md                   # REWRITTEN from scratch (~200-300 lines, all rust,no_run)
├── src/
│   ├── lib.rs                  # TRIMMED — deleted //! lines 1-53, deleted mod tool/tool_router,
│   │                           # deleted pub fn tool/tool_router/prompt/resource,
│   │                           # added #![doc = include_str!("../README.md")] at top,
│   │                           # added #[cfg(doctest)] pub struct ReadmeDoctests; just after,
│   │                           # rewrote /// docs on pub fn mcp_tool/mcp_server/mcp_prompt/mcp_resource
│   ├── mcp_common.rs           # UNCHANGED (target API, not touched)
│   ├── mcp_prompt.rs           # UNCHANGED
│   ├── mcp_resource.rs         # UNCHANGED
│   ├── mcp_server.rs           # UNCHANGED
│   ├── mcp_tool.rs             # UNCHANGED
│   └── utils.rs                # UNCHANGED
├── tests/
│   ├── mcp_prompt_tests.rs     # UNCHANGED
│   ├── mcp_server_tests.rs     # UNCHANGED
│   ├── mcp_tool_tests.rs       # UNCHANGED
│   └── ui/
│       ├── mcp_prompt_missing_description.rs(.stderr)  # UNCHANGED
│       ├── mcp_tool_missing_description.rs(.stderr)    # UNCHANGED
│       └── mcp_tool_multiple_args.rs(.stderr)          # UNCHANGED
# DELETED:
# ├── src/tool.rs                             (426 lines)
# ├── src/tool_router.rs                      (257 lines)
# ├── tests/tool_tests.rs                     (129 lines)
# ├── tests/tool_router_tests.rs              (71 lines)
# ├── tests/ui/tool_missing_description.rs    (+ .stderr)
```

### Pattern 1: `include_str!` with executable README doctests

**What:** Include `README.md` as crate-level rustdoc AND run its code blocks as doctests under `cargo test --doc`.

**When to use:** On `pmcp-macros/src/lib.rs`, as the first two items in the file (after any workspace-required allow lints).

**Example:**
```rust
// Source: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html
// Verified against: raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/src/lib.rs line 1
// Workspace reference: github.com/tokio-rs/tracing tracing-attributes crate

#![doc = include_str!("../README.md")]

// The `ReadmeDoctests` trick makes the README's `rust,no_run` blocks run
// under `cargo test --doc` even though the crate-level doc attribute alone
// would only register them as module-level documentation. Without this,
// `cargo test --doc` would still pick them up (module-level `#[doc = ...]`
// is already a doctest target), but the hidden-struct pattern is the
// rustdoc-book-recommended idiom and makes the intent explicit.
//
// Source: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html
#[cfg(doctest)]
pub struct ReadmeDoctests;

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn, ItemImpl};

mod mcp_common;
mod mcp_prompt;
mod mcp_resource;
mod mcp_server;
mod mcp_tool;
#[allow(dead_code)]
mod utils;

// ... per-macro pub fns with rewritten /// docs ...
```

**Note on the `#[cfg(doctest)]` item:** `rmcp-macros` does NOT include this extra item — it just uses the bare `#![doc = include_str!("../README.md")]` at line 1. Both patterns work. The `ReadmeDoctests` item is belt-and-suspenders, explicitly documented in the rustdoc book as the way to "test doctests that are included in your README file." Phase 66 should include it for clarity, but it's low-stakes — removing it has no functional impact because the module-level doc attribute already makes the README blocks into doctests.

### Pattern 2: `rust,no_run` code blocks that compile via `pmcp` dev-dependency

**What:** README code blocks invoke `#[mcp_tool]`, `#[mcp_server]`, etc. without triggering cyclic dependency errors.

**When to use:** Every code block in `pmcp-macros/README.md`.

**Example:**
```rust,no_run
// Source: mirrors the existing pattern in pmcp-macros/tests/mcp_tool_tests.rs
// and the established pattern in tokio-rs/tracing tracing-attributes crate

use pmcp::{mcp_tool, ToolHandler};   // NOTE: pmcp::, NOT pmcp_macros::
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

**The critical detail:** `use pmcp::{mcp_tool, ...}` — NOT `use pmcp_macros::{mcp_tool, ...}`. A doctest inside the `pmcp-macros` crate cannot invoke proc-macros defined in the same crate directly; that's the cyclic-dependency limitation documented at `[CITED: github.com/rust-lang/rust/issues/58700]`. The workaround is to use the parent `pmcp` crate as a **dev-dependency** and reference the re-exported macros through it. `pmcp-macros/Cargo.toml:27` already has `pmcp = { version = ">=1.20.0", path = "..", features = ["full"] }` as a dev-dependency, so this works out of the box. This is also what `tracing-attributes` does — its doctests use `use tracing::instrument;` not `use tracing_attributes::instrument;` `[VERIFIED: raw.githubusercontent.com/tokio-rs/tracing/master/tracing-attributes/src/lib.rs]`.

**Caveat for `mcp_resource`:** `src/lib.rs:147` re-exports `{mcp_prompt, mcp_server, mcp_tool}` but NOT `mcp_resource` (the re-export gap noted in CONTEXT.md Deferred Ideas). Doctests that want to demonstrate `#[mcp_resource]` will need to either (a) import it as `use pmcp_macros::mcp_resource;` directly — which DOES compile from `rust,no_run` blocks because the README is included into `lib.rs` as a doc attribute and doctests included via `#[doc]` on the root module compile with the crate itself in scope; OR (b) wait for a follow-up phase to add `mcp_resource` to the re-export list. **Recommendation: use option (a) via `use pmcp_macros::mcp_resource;`** in the `#[mcp_resource]` README section ONLY, and leave a `// TODO: after pmcp 2.3+ adds mcp_resource re-export, flip to `use pmcp::mcp_resource;`` comment. This is consistent with D-03's "do not fix the re-export gap here" and gives the reader working code.

**Empirical validation:** Whether option (a) actually works depends on a subtle rustdoc quirk. The safest bet is for the planner to create a Wave 0 task that writes a one-block README experiment (`use pmcp_macros::mcp_resource; ...`), runs `cargo test --doc -p pmcp-macros`, and verifies the block compiles. If it doesn't, fall back to option (c): document `#[mcp_resource]` with a `rust,no_run` example that imports from `pmcp_macros` via an explicit `extern crate pmcp_macros;` header, or (option d, cleanest fallback): document `#[mcp_resource]` in the README but use a `rust,no_run` example that doesn't actually invoke the macro, instead showing the expected usage with a code comment (`// Apply #[mcp_resource(uri = "...", description = "...")] to the function below:`) — this is an acceptable downgrade if option (a) fails.

### Pattern 3: Keep a Changelog entry for a breaking minor bump (pre-1.0)

**What:** A pre-1.0 crate making breaking changes at a minor version bump (0.4.1 → 0.5.0), following Keep a Changelog 1.0.0 format.

**When to use:** Creating `pmcp-macros/CHANGELOG.md` for the v0.5.0 release.

**Example:**
```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-04-11

### Removed (breaking)

- **`#[tool]` macro removed.** Deprecated since 0.3.0 in favor of `#[mcp_tool]`.
  `#[mcp_tool]` provides mandatory compile-time `description` enforcement,
  `State<T>` injection, async/sync auto-detection, and `annotations(...)`
  support — all features `#[tool]` lacked. See Migration below.
- **`#[tool_router]` macro removed.** Deprecated since 0.3.0 in favor of
  `#[mcp_server]`. `#[mcp_server]` generates a full `McpServer` impl with
  bulk tool registration via `ServerBuilder::mcp_server(...)`.
- **`#[prompt]` and `#[resource]` zero-op stubs removed.** These were
  placeholder identity functions that did not generate any code. Use
  `#[mcp_prompt]` and `#[mcp_resource]` for functional equivalents.

### Migration

**`#[tool]` → `#[mcp_tool]`:**

```rust,ignore
// Before (0.4.x):
#[tool(description = "Add two numbers")]
async fn add(params: AddParams) -> Result<AddResult, String> {
    Ok(AddResult { sum: params.a + params.b })
}

// After (0.5.0):
#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { sum: args.a + args.b })
}
```

Behavioral differences to watch for when migrating:
- `description` is now mandatory at compile time (no more runtime `None`).
- Return type is `pmcp::Result<T>` instead of `Result<T, String>`.
- For shared state, add a `State<T>` parameter: `async fn add(args: AddArgs, db: State<Database>) -> pmcp::Result<AddResult>`.
- Register via `.tool("add", add())` (the macro generates a zero-arg
  constructor function that returns the `ToolHandler` struct).

**`#[tool_router]` → `#[mcp_server]`:** ... (before/after snippet)

**`#[prompt]` / `#[resource]`:** These never generated any code in 0.4.x.
Migrate to `#[mcp_prompt]` / `#[mcp_resource]` for functional equivalents.

### Changed

- Crate-level documentation is now sourced from `README.md` via
  `#![doc = include_str!("../README.md")]`. README and docs.rs render
  the same content.
```

**Note on the `rust,ignore` inside CHANGELOG.md:** Code blocks in a CHANGELOG are NOT doctests — `cargo test --doc -p pmcp-macros` only processes doc attributes on Rust items, and `CHANGELOG.md` is not referenced by any `#[doc]` attribute. So `rust,ignore` (or any tag) is fine here — the blocks exist for humans reading the changelog, not for the compiler. Do not flip these to `rust,no_run` (there's no upside).

**Section structure verified against workspace convention:** The workspace root `CHANGELOG.md` uses `### Fixed`, `### Added`, `### Changed`, `### Internal` as H3 subsections under a `## [X.Y.Z]` H2 entry `[VERIFIED: /Users/guy/Development/mcp/sdk/rust-mcp-sdk/CHANGELOG.md:10-41]`. The workspace does not use `### Removed (breaking)` today, but Keep a Changelog 1.0.0 explicitly lists `Removed` as a standard category. The `(breaking)` qualifier is a workspace-neutral convention that makes the breaking nature obvious — acceptable. If the planner prefers strict conformance with existing workspace style, fold the breaking-removal content under a `### Fixed` or `### Changed` subsection with `**Breaking:**` bold markers — both are valid.

### Anti-Patterns to Avoid

- **Using `cargo-rdme` or `cargo-sync-rdme`**: These tools flow the OPPOSITE direction — they generate README.md from `//!` comments. `cargo-sync-rdme` additionally requires nightly Rust (workspace is stable). `[CITED: github.com/orium/cargo-rdme]` `[CITED: github.com/gifnksm/cargo-sync-rdme]`
- **Using `rust,ignore` in README code blocks**: Invisible to `cargo test --doc`, the exact mechanism that produced the current README's `pmcp = "1.1"` drift. Explicit D-09 violation.
- **Calling proc-macros directly from `pmcp_macros`**: Writing `use pmcp_macros::mcp_tool;` inside a doctest in the `pmcp-macros` crate itself will fail with a cyclic-dependency error `[CITED: github.com/rust-lang/rust/issues/58700]`. Use `use pmcp::mcp_tool;` instead.
- **Adding a "Migration" section to the README**: D-05 explicitly forbids this. Migration belongs in CHANGELOG.md.
- **Advertising deleted macros in `//!` comments**: Delete lines 1-53 of `pmcp-macros/src/lib.rs` wholesale per D-10. Do NOT try to preserve any of the old `//!` prose — it's either obsolete (references deleted macros) or duplicates what the new README covers.
- **Leaving the `//!` reference to `#[tool_router]` in the old Calculator example at `lib.rs:17-53`**: This is part of the "lines 1-53 delete" but worth calling out specifically — the example uses the deleted macros and would be a silent booby-trap if left behind.
- **Amending a previous commit for any reason**: CLAUDE.md is emphatic — "ALWAYS create NEW commits rather than amending". Phase 66 should split into multiple commits (delete macros, rewrite README, bump versions) and absolutely never `git commit --amend`.
- **Running bare `cargo clippy`/`cargo test` instead of `make quality-gate`**: CLAUDE.md explicitly warns this is weaker than CI because CI uses pedantic + nursery lints.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Sync README content with rustdoc module docs | A custom script that parses README.md and regenerates `//!` comments | `#![doc = include_str!("../README.md")]` (Rust built-in, 1.54+) | `[CITED: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html]` — this is the official solution, zero dependencies, already used by `rmcp-macros` and the broader Rust ecosystem. |
| Catch README code drift | A pre-commit hook that `diff`s README snippets against `examples/s23_mcp_tool_macro.rs` | `cargo test --doc -p pmcp-macros` (runs automatically under `make quality-gate`) once `include_str!` is wired | Doctests execute on every quality gate run. Hand-rolled snippet diffing is fragile and duplicates what `cargo test --doc` already does for free. |
| CHANGELOG markup format | Invent a project-specific changelog format | [Keep a Changelog 1.0.0](https://keepachangelog.com/en/1.0.0/) | Already declared as the workspace format `[VERIFIED: /Users/guy/Development/mcp/sdk/rust-mcp-sdk/CHANGELOG.md:5]`, and matches `rmcp-macros/CHANGELOG.md` `[CITED: raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/CHANGELOG.md]`. |
| Detect deleted-macro references in documentation | A custom grep rule + CI gate | Rely on `cargo test --doc` to fail when a README example `use pmcp::tool;` fails to resolve | Once the macro is deleted, any doctest that imports it stops compiling — the type system is the validator. No custom tooling needed. |
| Generate breaking-change migration guide | A script that parses the old public surface and emits before/after markdown | Hand-written CHANGELOG.md v0.5.0 entry (this is a ~4 macro deletion, trivial to write) | Over-engineering for a one-time cleanup of a pre-1.0 crate with small user count. D-23 equivalent (user explicitly did not ask for automated migration tooling). |

**Key insight:** Every "tool I might reach for" in this phase is already built into Rust itself. The entire phase consists of **deleting files, rewriting prose, and flipping one attribute on the top of `lib.rs`**. Any proposed custom tooling is scope creep.

## Common Pitfalls

### Pitfall 1: `edition = "2021"` means doctest paths are relative to `lib.rs`, not `README.md`

**What goes wrong:** If any README code block uses `include_str!(...)` or `include_bytes!(...)` with a relative path, the path resolves relative to `lib.rs` (where the `#![doc = include_str!(...)]` attribute is applied), NOT relative to the README file itself. A code block that reads `include_str!("./example.rs")` expecting `pmcp-macros/README.md`'s sibling `example.rs` will actually look for `pmcp-macros/src/example.rs` and fail silently during doctest compilation.

**Why it happens:** Prior to Rust 2024 edition, `#[doc=include_str!("...")]` doctests did not carry span information from the included file, and included paths were specified relative to the source file that invoked `include_str!` `[CITED: doc.rust-lang.org/edition-guide/rust-2024/rustdoc-nested-includes.html]`. `pmcp-macros/Cargo.toml:4` declares `edition = "2021"` `[VERIFIED]`, so this phase is subject to the pre-2024 behavior.

**How to avoid:** Don't write `include_str!` or `include_bytes!` or any relative-path file-reading macro inside README.md code blocks. This is rare in practice (most tool examples don't include files), but it's the one concrete footgun of the `include_str!` doc pattern. If a README example genuinely needs to reference an external file, inline the content instead.

**Warning signs:** `cargo test --doc -p pmcp-macros` fails with `no such file or directory` errors pointing to `src/something`. If upgrading to `edition = "2024"` is desired, that's a separate migration — out of scope for Phase 66.

### Pitfall 2: `rust,ignore` is invisible to `cargo test --doc` — this is exactly how the current README drifted

**What goes wrong:** Historical context — the current 252-line README references `pmcp = { version = "1.1", features = ["macros"] }` at workspace v2.2.0 `[VERIFIED: pmcp-macros/README.md:19]`. This happened because the current README is not wired via `include_str!` and all code blocks are plain ` ```rust ` without any form of validation.

**Why it happens:** Without `cargo test --doc` coverage, nothing catches stale version numbers, deleted macro names, or renamed APIs in the README. The drift accumulates invisibly until a user reports it.

**How to avoid:** (a) `#![doc = include_str!("../README.md")]` wires the README into `cargo test --doc`, (b) use `rust,no_run` on every code block (not `rust,ignore`), (c) use `pmcp = "2.3"` (not `pmcp-macros` as a direct dep) in the installation example, so that bumping `pmcp` is the only version-pinning task.

**Warning signs:** Any code block in the new README tagged `rust,ignore`. Any `pmcp-macros = "..."` line in the installation section. Either is an immediate red flag.

### Pitfall 3: Relative links in README break on docs.rs

**What goes wrong:** A README link like `[LICENSE](LICENSE)` or `[the tool macro](src/mcp_tool.rs)` will work on GitHub (which serves relative links from the repo root) but break on docs.rs (which doesn't have access to repo files outside the crate's published set).

**Why it happens:** "rustdoc does not support relative file links in Markdown" `[CITED: linebender.org/blog/doc-include/]`.

**How to avoid:** Avoid relative file links in the README. Use (a) absolute URLs to GitHub for file references (`https://github.com/paiml/pmcp/blob/main/pmcp-macros/src/mcp_tool.rs`), (b) docs.rs links for API items (`https://docs.rs/pmcp/latest/pmcp/attr.mcp_tool.html`), or (c) intra-doc links wrapped in a workaround: "place dual link definitions in the documentation comment: an intra-doc link definition *before* the include statement, and an external docs.rs link in the README" `[CITED: linebender.org/blog/doc-include/]`. For a ~200-line README focused on macro usage, just use absolute URLs — simpler.

**Warning signs:** Any `[text](relative/path)` in the new README. Any GitHub-only markdown features (e.g., GitHub admonitions like `> [!NOTE]`) that won't render on docs.rs.

### Pitfall 4: The `mcp_resource` re-export gap creates an asymmetry in README code blocks

**What goes wrong:** The README's `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]` examples can all use `use pmcp::{mcp_tool, mcp_server, mcp_prompt};`. The `#[mcp_resource]` example cannot — `mcp_resource` is NOT in the `pub use` line at `src/lib.rs:147`. A reader will notice the inconsistency.

**Why it happens:** The re-export gap is real, out-of-scope for this phase (D-03 and Deferred Ideas both say "don't fix it here"), and visible in the README as an asymmetric import.

**How to avoid:** Three options, in order of preference:
1. **Use `use pmcp_macros::mcp_resource;` in the README's `#[mcp_resource]` section ONLY**, with an inline explanatory comment: `// Note: also available as pmcp::mcp_resource in a future release`. Asymmetric but honest.
2. **Use `use pmcp::mcp_resource;` anyway and let `cargo test --doc` fail**, then add a blocker note to CONTEXT.md for the planner forcing the scope to include fixing the re-export gap. Scope creep, violates D-03.
3. **Document `#[mcp_resource]` with a prose-first section that shows the attribute usage WITHOUT a complete compiling example**, linking out to an example file instead. Least satisfying for readers.

**Recommendation:** **Option 1.** The asymmetric import is a minor visual wart for a few weeks until the re-export gap is closed in a follow-up phase, and it gives the reader working code today.

**Warning signs:** The `#[mcp_resource]` README example fails to compile under `cargo test --doc`. If it does, flip to option 1 immediately.

### Pitfall 5: `cargo test --doc` doesn't test README code blocks on proc-macro crates unless they're explicitly included

**What goes wrong:** Some developers assume that just adding `#![doc = include_str!("../README.md")]` to a `proc-macro = true` crate's `lib.rs` causes doctests in the included README to run. This is true — module-level doc attributes compile to doctests — but there's a subtle rustdoc diagnostic quirk: error spans in included files point to the wrong line (the `include_str!` site, not the README line) `[CITED: github.com/rust-lang/rust/issues/81070]`.

**Why it happens:** The diagnostic span-mapping for `include_str!`-included doctests was improved in Rust 2024 edition, but `pmcp-macros` is `edition = "2021"`. Pre-2024, error messages say "ducks.rs:14:9" instead of the actual README.md line where the problem is.

**How to avoid:** Accept the minor diagnostic inconvenience. When `cargo test --doc -p pmcp-macros` fails, the error will point to `pmcp-macros/src/lib.rs:1` (the `include_str!` site). The planner should note this in the verification steps so a confused executor doesn't chase phantom errors in `lib.rs`.

**Warning signs:** `cargo test --doc` error messages reference `pmcp-macros/src/lib.rs:1` even though the actual broken code is in `README.md`. Grep `README.md` for the failing symbol to find the real location.

### Pitfall 6: The `tests/ui/tool_missing_description.rs` file must be deleted alongside the macro

**What goes wrong:** `trybuild`-style UI tests compile-check negative cases. The file `pmcp-macros/tests/ui/tool_missing_description.rs` (+ `.stderr`) tests that `#[tool]` (without description) emits a specific compile error. When `#[tool]` is deleted, this file becomes dead — the test itself still "passes" (because the file fails to compile for a different reason: `tool` no longer exists), but the `.stderr` file asserts a specific error message that will no longer match, causing the UI test to fail loudly.

**Why it happens:** `trybuild` compares expected stderr output against actual. A different error (missing macro vs missing argument) produces a stderr mismatch, which trybuild flags.

**How to avoid:** Delete BOTH `pmcp-macros/tests/ui/tool_missing_description.rs` AND `pmcp-macros/tests/ui/tool_missing_description.stderr` as part of the D-01 deletion set. Verified these files exist: `[VERIFIED: ls pmcp-macros/tests/ui]` — both files present.

**Warning signs:** `cargo test` fails in `pmcp-macros` with a trybuild diff error mentioning `tool_missing_description`.

### Pitfall 7: Stale `cargo run --example 63_mcp_tool_macro` headers in renamed files

**What goes wrong:** `examples/s23_mcp_tool_macro.rs:14` still says `cargo run --example 63_mcp_tool_macro --features full` even though the file was renamed `[VERIFIED: examples/s23_mcp_tool_macro.rs:14]`. If the new README points readers at this example with "run `cargo run --example s23_mcp_tool_macro`", and then the reader opens the file and sees a different name, they'll get confused.

**Why it happens:** Phase 65 renamed examples with `s*` prefixes but didn't update the `//!` doc comments inside each example file. This is leftover drift from Phase 65, not caused by Phase 66.

**How to avoid:** Phase 66 should fix the stale `cargo run --example 63_...` line inside `examples/s23_mcp_tool_macro.rs:14` and `examples/s24_mcp_prompt_macro.rs` (check similar line number). This is a trivial 2-line fix that directly supports D-11 (per-macro `///` docs reference the renamed example files). Flag this to the planner as a small additional task: "Fix stale example-name references in `s23_mcp_tool_macro.rs` and `s24_mcp_prompt_macro.rs` doc headers." It's not in CONTEXT.md but it's obviously within the spirit of phase 66.

**Warning signs:** Reader reports "cargo run --example 63_mcp_tool_macro" fails with "no such example". Grep for `63_mcp_tool_macro` / `64_mcp_prompt_macro` across `examples/`.

## Runtime State Inventory

> This phase involves deletion and renaming. The Runtime State Inventory is required.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — this is a compile-time-only change. No databases, caches, persistent stores, or user data reference `#[tool]`/`#[tool_router]`/`#[prompt]`/`#[resource]`. Macro invocations are pure Rust source and exist only in the compile graph. | None |
| Live service config | None — `pmcp-macros` has no runtime service component. It's a `proc-macro = true` crate used only by `rustc` at build time. | None |
| OS-registered state | None — no launchd/systemd/Task Scheduler/pm2 integration. | None |
| Secrets/env vars | None — no secrets or env vars reference deleted macro names. | None |
| Build artifacts / installed packages | **YES — must consider:** (a) `target/` directory will contain cached `.rlib` builds of `pmcp-macros` v0.4.1 until `cargo clean` or until the version bump invalidates them (happens automatically), (b) docs.rs has v0.4.1 published and will continue to serve it at `docs.rs/pmcp-macros/0.4.1/` — this is the intended behavior (immutable version history), (c) `crates.io` has v0.4.1 published, same immutability. **No action required** — these are all expected artifacts of a published crate version. Do NOT try to delete or hide v0.4.1 from docs.rs/crates.io. | None — the v0.4.1 historical record stays intact; users on `pmcp-macros = "0.4"` continue to work until they opt in to 0.5.0 |

**The canonical question — "After every file in the repo is updated, what runtime systems still have the old string cached, stored, or registered?"** — has **no hits for this phase**. `pmcp-macros` is purely a compile-time artifact, and all references to `#[tool]`/`#[tool_router]`/`#[prompt]`/`#[resource]` live in Rust source files + documentation markdown files, all of which are captured by the file-level edits specified in CONTEXT.md D-01, D-02, D-16, D-17.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | Build, test, quality gate | ✓ | 1.94.1 `[VERIFIED: local]` | — |
| `rustc` | Compilation | ✓ | 1.94.1 `[VERIFIED: local]` — exceeds the 1.54 minimum for `#[doc = include_str!]` and the 1.82.0 MSRV declared in `pmcp-macros/Cargo.toml:14` | — |
| `make` | Quality gate orchestration | ✓ (assumed — workspace-standard, documented in CLAUDE.md) | — | — |
| `git` | Commit/branch/push | ✓ (assumed — required for any gsd phase) | — | — |
| `gh` (GitHub CLI) | PR creation in Step 5 of the CLAUDE.md release workflow | ✓ (assumed) | — | Fallback: manual PR via web UI; release workflow still triggers on tag push |
| Internet access to `crates.io` | Release publish | N/A for this phase — phase produces a PR, not a release; CI handles the actual publish on tag push | — | — |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None.

**Key observation:** This phase has zero external runtime dependencies. Every tool needed is already part of the workspace's documented quality gate. The research-worthy question isn't "is X available" — it's "does the Rust language feature `#[doc = include_str!]` do what we think it does when applied to a `proc-macro = true` crate", and the answer has been verified above against (a) the official rustdoc book, (b) the `rmcp-macros` reference implementation, and (c) the `tracing-attributes` battle-tested equivalent.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`, `cargo test --doc`) + `trybuild` for UI tests |
| Config file | None — standard cargo workspace layout |
| Quick run command | `cargo test --doc -p pmcp-macros` (validates README + per-macro `///` doc examples) |
| Full suite command | `make quality-gate` (runs fmt, clippy, build, test, audit per CLAUDE.md) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MACR-01 | pmcp-macros README documents all four `mcp_*` macros with compilable examples | doctest | `cargo test --doc -p pmcp-macros` | Existing README.md will be rewritten; after rewrite, `cargo test --doc` must pass |
| MACR-01 | No `pmcp = "1.*"` version stragglers in README | grep check | `grep -E 'pmcp *= *"1\.' pmcp-macros/README.md` (exit 1 = fail if found) | Wave 0 verification step |
| MACR-02 | CHANGELOG migration entry exists | file check + content check | `test -f pmcp-macros/CHANGELOG.md && grep -l 'mcp_tool' pmcp-macros/CHANGELOG.md` | Wave 0 — `pmcp-macros/CHANGELOG.md` does not yet exist |
| MACR-03 | lib.rs uses include_str!("../README.md") | grep check | `grep -F 'include_str!("../README.md")' pmcp-macros/src/lib.rs` (exit 0 = pass) | Wave 0 verification step |
| D-01 | `#[tool]` / `#[tool_router]` fully removed | compile check | `! grep -rn 'pub fn tool\|pub fn tool_router' pmcp-macros/src/` (exit 0 if both absent) + `cargo build -p pmcp-macros` | Verifies deletion completeness |
| D-02 | `#[prompt]` / `#[resource]` stubs fully removed | compile check | `! grep -rn 'pub fn prompt\|pub fn resource' pmcp-macros/src/lib.rs` + `cargo build -p pmcp-macros` | Verifies deletion completeness |
| D-17 | pmcp-course chapters no longer reference `#[tool(...)]` syntax | grep check | `! grep -rn '#\[tool(' pmcp-course/src/` | Wave N verification step |
| D-20/D-21 | Version pins updated | grep check | `grep -c '0\.5\.0' pmcp-macros/Cargo.toml Cargo.toml` (expect ≥3: pmcp-macros/Cargo.toml line 3, Cargo.toml line 53, Cargo.toml line 147) | Wave N verification step |
| quality gate | Entire workspace builds cleanly with new macro surface | full suite | `make quality-gate` | All existing workspace infrastructure |

### Sampling Rate

- **Per task commit:** `cargo test --doc -p pmcp-macros && cargo build -p pmcp-macros` (fast — under 30 seconds)
- **Per wave merge:** `make quality-gate` (matches CI, enforces pedantic + nursery clippy)
- **Phase gate:** `make quality-gate` green before `/gsd-verify-work`, then full workspace `cargo test` before tagging the release

### Wave 0 Gaps

No gaps — the existing test infrastructure (integration tests in `pmcp-macros/tests/*`, UI tests in `pmcp-macros/tests/ui/*`, workspace-level `make quality-gate`) already covers everything this phase touches. The ONE new piece of validation is `cargo test --doc -p pmcp-macros` running against the README — and that happens automatically the moment `#![doc = include_str!("../README.md")]` lands. No new test files, no new framework, no new fixtures.

**One optional Wave 0 task worth considering:** A proof-of-concept task that writes a minimal 3-line README, wires it via `include_str!`, and runs `cargo test --doc -p pmcp-macros` to verify the mechanism works on the `pmcp-macros` crate specifically (validating the `use pmcp::mcp_tool;` import path). This de-risks the entire phase before the executor commits to rewriting 200+ lines of README prose. Whether to include this as an explicit task is the planner's call — I recommend YES.

## Security Domain

This phase's security surface is essentially zero:

- **No runtime code changes.** All deletions are compile-time proc-macro code. The remaining `#[mcp_*]` macros are unmodified (D-03).
- **No new dependencies.** Everything used is already in the workspace.
- **No secrets handled.** No credential flows, no auth, no crypto.
- **No input validation.** Documentation rewrites don't process user input at runtime.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | N/A — no auth surface |
| V3 Session Management | no | N/A — no session surface |
| V4 Access Control | no | N/A — no access control surface |
| V5 Input Validation | no | N/A — no runtime input handling |
| V6 Cryptography | no | N/A — no crypto |
| V14 Configuration | marginal | **Version pinning hygiene:** The D-08 requirement that README examples use `pmcp = "2.3"` (not a wildcard or `1.*`) is a supply-chain hygiene win — it reduces the risk of users accidentally picking up an incompatible breaking version. The `cargo test --doc` enforcement via `rust,no_run` makes it mechanically impossible to merge a README with a stale version (because the example would fail to compile against the workspace `pmcp`). |

### Known Threat Patterns for the stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| README documents a macro attribute that doesn't exist / has a typo, causing users to copy broken code | Tampering (of user's understanding) | `cargo test --doc -p pmcp-macros` catches the moment the doc reference breaks (pattern 2 above) |
| Deleted macros leave orphan references in downstream docs (e.g., pmcp-course), spreading incorrect patterns | Tampering (of user's understanding) | D-17 enforces same-phase update; Wave N verification step: `! grep -rn '#\[tool(' pmcp-course/src/` |
| Published v0.5.0 accidentally re-introduces a deleted macro at the `pub` level | — | `cargo test` against the integration test suite in `pmcp-macros/tests/` will fail if any test file references a deleted macro |
| CHANGELOG.md accidentally omits the breaking-change notice, users upgrade blind | — | Manual verification — but the phase gate requires `pmcp-macros/CHANGELOG.md` to exist AND contain the `### Removed (breaking)` subsection, verifiable by grep |

**Security verdict:** This phase is security-neutral. Include the Security Domain section for compliance with the phase-researcher template, but the planner's verification steps don't need dedicated security tasks.

## Code Examples

Verified patterns from primary sources, ready for the planner to reference directly:

### Idiomatic `include_str!` wiring for a `proc-macro = true` crate

```rust
// Source: raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/src/lib.rs
// Also idiomatic per: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html

#![doc = include_str!("../README.md")]

// Optional but recommended: explicit hidden struct for ReadmeDoctests so
// the crate-level doc attribute's executable code blocks are mechanically
// under test. Bare `#![doc = include_str!(...)]` also works.
#[cfg(doctest)]
pub struct ReadmeDoctests;

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn, ItemImpl};

mod mcp_common;
mod mcp_prompt;
mod mcp_resource;
mod mcp_server;
mod mcp_tool;
#[allow(dead_code)]
mod utils;

// ... pub fn mcp_tool / mcp_server / mcp_prompt / mcp_resource ...
```

### Idiomatic README code block (for pmcp-macros specifically)

````markdown
```rust,no_run
use pmcp::{mcp_tool, ToolHandler, ServerBuilder, ServerCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    /// First number
    a: f64,
    /// Second number
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    /// The sum of `a` and `b`
    sum: f64,
}

#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { sum: args.a + args.b })
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let _server = ServerBuilder::new()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("add", add())
        .build()?;
    Ok(())
}
```
````

The triple-backtick-fenced block inside a markdown file reads as `rust,no_run`. This compiles but does not execute, matching the `examples/s23_mcp_tool_macro.rs` pattern and verifying that (a) `#[mcp_tool]` is still exported, (b) `pmcp::mcp_tool` is still re-exported from the main crate, (c) `ServerBuilder::new().name(...).tool("add", add())` is still the correct API surface. Any drift on any of these surfaces will break the doctest at quality-gate time.

### Verified `pmcp-macros/CHANGELOG.md` Keep a Changelog format

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-04-11

### Removed (breaking)

- **`#[tool]` macro removed.** Deprecated since 0.3.0. Replaced by `#[mcp_tool]`.
- **`#[tool_router]` macro removed.** Deprecated since 0.3.0. Replaced by `#[mcp_server]`.
- **`#[prompt]` zero-op stub removed.** Placeholder identity function that
  generated no code. Use `#[mcp_prompt]`.
- **`#[resource]` zero-op stub removed.** Placeholder identity function that
  generated no code. Use `#[mcp_resource]`.

### Migration from 0.4.x

#### `#[tool]` → `#[mcp_tool]`

Before:
\`\`\`rust,ignore
#[tool(description = "Add two numbers")]
async fn add(params: AddParams) -> Result<AddResult, String> { /* ... */ }
\`\`\`

After:
\`\`\`rust,ignore
#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> { /* ... */ }
\`\`\`

Behavioral differences:
- `description` is enforced at compile time (no more runtime `Option<String>`).
- Return type is `pmcp::Result<T>` instead of `Result<T, String>`.
- Shared state via `State<T>` parameter — zero `Arc::clone` boilerplate.
- Async/sync auto-detection from the `fn` signature.
- MCP annotations via `annotations(read_only = true, destructive = true, ...)`.
- Registration via `.tool("add", add())` — the macro generates a zero-arg
  constructor returning a `ToolHandler`-implementing struct.

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

// Register all tools at once:
let builder = ServerBuilder::new().mcp_server(calculator);
\`\`\`

#### `#[prompt]` and `#[resource]` zero-op stubs

These never generated any code in 0.4.x. Use `#[mcp_prompt]` /
`#[mcp_resource]` for functional equivalents — see README for usage.

### Changed

- Crate-level documentation now sourced from `README.md` via
  `#![doc = include_str!("../README.md")]`. docs.rs and GitHub render the
  same content from a single source.
- README and per-macro doc comments use `rust,no_run` code blocks (compiled
  under `cargo test --doc`) instead of `rust,ignore` — catches API drift
  automatically.
```

The `\`\`\`rust,ignore` usage INSIDE the CHANGELOG is intentional and correct — CHANGELOG.md is NOT included via `include_str!`, so its code blocks are not doctests. `rust,ignore` means human-readable syntax highlighting with no compile check, which is what you want for a migration-guide snippet that intentionally shows the OLD (deleted) syntax.

## rmcp Benchmark

Concrete observations about how `rmcp-macros` (the official Model Context Protocol Rust SDK) structures its macros crate docs — the explicit "target bar" for this phase:

`[VERIFIED: raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/src/lib.rs]` — confirmed fetched
`[VERIFIED: raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/CHANGELOG.md]` — confirmed fetched
`[CITED: github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp-macros/README.md]` — fetched via WebFetch

### What rmcp-macros does that pmcp-macros SHOULD match

| Pattern | rmcp-macros | Apply to pmcp-macros? |
|---------|-------------|------------------------|
| Uses `#![doc = include_str!("../README.md")]` at line 1 of `lib.rs` | Yes — literally line 1 of `crates/rmcp-macros/src/lib.rs` | **YES** — this is exactly D-10. Directly confirmed as valid by the reference implementation. |
| Keeps per-crate `CHANGELOG.md` in the crate directory | Yes — `crates/rmcp-macros/CHANGELOG.md` exists and is actively maintained | **YES** — this is exactly D-14 (create pmcp-macros/CHANGELOG.md). Confirmed as idiomatic. |
| Follows Keep a Changelog 1.0.0 format with explicit header | Yes — identical header text to workspace root CHANGELOG.md | **YES** — matches D-13/D-14 and workspace convention. |
| Marks breaking changes inline in changelog entries (e.g., `[breaking]`) rather than a separate "Breaking" section | Yes | **Acceptable alternative to D-13's "Removed (breaking)" subsection** — either pattern is valid; `### Removed` + `(breaking)` qualifier is clearer for a major deletion wave like this phase. |
| README is short (~50 lines in rmcp-macros) and links out to the main project README for getting-started content | Yes — 4 top-level sections: rmcp, Feature Flags, Transports, License | **NO, diverge** — pmcp-macros README is a primary entry point for docs.rs/pmcp-macros. The target audience per D-05 is new users who land on docs.rs/pmcp-macros directly. A short "see main README" README would frustrate that audience. Write a full ~200-300 line README with proper examples. This is a deliberate divergence from rmcp's approach, justified by audience. |
| Per-macro `///` doc comments use plain ` ```rust ` or `rust,ignore` | Uses `rust,ignore` exclusively `[VERIFIED: raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/src/lib.rs]` | **NO, diverge** — D-09 forbids `rust,ignore` and explicitly requires `rust,no_run`. This is a pmcp-specific quality upgrade. The rmcp-macros project uses `rust,ignore` because their examples rely on runtime-only patterns that don't compile in isolation. PMCP's `use pmcp::{mcp_tool, ...}` re-export path means `rust,no_run` IS achievable — better to have compiled (even if not executed) doctests than ignored ones. |
| Defines seven macros in one crate (`tool`, `tool_router`, `tool_handler`, `prompt`, `prompt_router`, `prompt_handler`, `task_handler`) | Yes | **NO, different scope** — pmcp-macros has four macros (`mcp_tool`, `mcp_server`, `mcp_prompt`, `mcp_resource`). Different architectural choice, neither better. |
| Uses standard Keep a Changelog subsection headers (`### Added`, `### Other`, etc.) | Yes, plus `[breaking]` inline markers | **PARTIALLY** — workspace root CHANGELOG.md uses `### Fixed`, `### Added`, `### Changed`, `### Internal`. D-13's "Removed (breaking)" is slightly different. Planner's discretion. |

### What rmcp-macros does that pmcp-macros SHOULD NOT copy

| Pattern | Why rmcp-macros does it | Why pmcp-macros shouldn't |
|---------|-------------------------|---------------------------|
| README as a short gateway, deferring to main project README | rmcp is a cohesive SDK with one "front door" README where all onboarding lives; `rmcp-macros` is a sub-crate that most users never interact with directly (they use `rmcp` with the `macros` feature). | pmcp's main `README.md` is a **repo README**, not a crate README. The repo README serves GitHub visitors, not docs.rs/pmcp-macros visitors. `docs.rs/pmcp-macros` is the URL a confused developer lands on when they grep their Cargo.lock for "pmcp-macros" — they need self-contained docs, not a chain of "see the other README." D-05 locks the target audience as "new users landing on docs.rs/pmcp-macros". |
| `rust,ignore` in macro examples | rmcp macro examples often reference custom server types that can't compile in isolation | pmcp has `pmcp = { path = "..", features = ["full"] }` as a dev-dep, which makes `rust,no_run` a strict improvement. There's no reason to settle for `ignore`. |
| No README code examples at all | Four-line description of each macro in a table is enough context given the main-README forwarding | D-05 requires proportional macro depth, explicit compilable examples — that's higher bar. |

### Net "beat rmcp at its own game" opportunities for this phase

The phase can legitimately claim to **exceed** rmcp-macros' quality on two dimensions:

1. **Doctest coverage:** `rust,no_run` with a `pmcp` dev-dep means pmcp-macros examples will break CI when macro surface drifts. rmcp-macros' `rust,ignore` examples don't. This is a measurable quality win.
2. **Self-contained onboarding:** A new user lands on docs.rs/pmcp-macros and has everything they need — attributes, examples, install instructions, links to runnable examples. docs.rs/rmcp-macros redirects them elsewhere.

These are worth calling out explicitly in the PR description when this phase lands — "our macros crate docs match the official Model Context Protocol Rust SDK's structural approach AND are doctest-verified" is a legitimate ecosystem bragging right.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `doc-comment` crate for including README in rustdoc | `#![doc = include_str!("../README.md")]` (built-in) | Rust 1.54 (2021-07) | The `doc-comment` crate is now obsolete for this use case; stdlib handles it. `[CITED: blog.guillaume-gomez.fr/articles/2020-07-01+doc-comment+0.4:+proc-macro+time]` is historical reference only. |
| `rust,ignore` as the default for "might not compile" doctests | `rust,no_run` wherever feasible | rustdoc book recommendation, ongoing | "`ignore` is almost never what you want" `[CITED: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html]`. |
| Relative paths in `#[doc = include_str!(...)]` doctests resolved relative to source file | In Rust 2024 edition, paths resolve relative to the included file | Rust 2024 edition | `pmcp-macros` is `edition = "2021"` — the old behavior applies. No action needed for phase 66. Upgrading is a separate migration. `[CITED: doc.rust-lang.org/edition-guide/rust-2024/rustdoc-nested-includes.html]` |
| `cargo-readme` (hand-written markers) | `cargo-rdme` and `cargo-sync-rdme` (more automated) | 2022-2024 | **All three flow README← rustdoc, NOT the direction pmcp-macros needs.** Out of scope. `[CITED: github.com/orium/cargo-rdme]` `[CITED: github.com/gifnksm/cargo-sync-rdme]` |

**Deprecated/outdated:**
- **`doc-comment` crate**: Replaced by stdlib `#[doc = include_str!(...)]` in Rust 1.54. Do not add `doc-comment` to `pmcp-macros/Cargo.toml`.
- **Documenting proc-macros via `//!` comments duplicated with README content**: This is what `pmcp-macros` does today (`src/lib.rs:1-53`). D-10 deletes it.
- **`rust,ignore` as "safe default" for doctests with external dependencies**: The modern answer is `rust,no_run` with a dev-dependency on the parent crate. `tracing-attributes` demonstrates this is the new norm.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `use pmcp::mcp_resource;` in a doctest inside `pmcp-macros` will fail to compile (because `mcp_resource` is not re-exported from `pmcp/src/lib.rs:147`). Pitfall 4 recommends falling back to `use pmcp_macros::mcp_resource;`. | Architecture Patterns Pattern 2, Pitfall 4 | Low — if `pmcp::mcp_resource` turns out to work (e.g., via some other re-export I missed), the asymmetric-import workaround is unneeded and the README can use consistent `use pmcp::{...};` imports throughout. Wave 0 verification task catches this in <30 seconds. |
| A2 | Writing `use pmcp_macros::mcp_resource;` inside a `rust,no_run` block in `pmcp-macros/README.md` (which is `include_str!`'d into `pmcp-macros/src/lib.rs` as the crate-level doc) will successfully compile the doctest. | Pattern 2 | Medium — the exact interaction of `#[doc = include_str!]` doctests with same-crate proc-macro imports is subtle. If this fails, Pitfall 4 option 3 (prose description without a compiling example) is the fallback. Wave 0 proof-of-concept task eliminates this risk entirely. |
| A3 | `pmcp-macros` is an `edition = "2021"` crate and will remain so for phase 66 (no opt-in edition upgrade). Doctest path behavior follows pre-2024 rules. | Pitfall 1 | Low — verified via `[VERIFIED: pmcp-macros/Cargo.toml:4]`. If the planner decides to upgrade to 2024 as a side effect, Pitfall 1 inverts (path becomes relative to README), which is the newer, more intuitive behavior. Either way, the pitfall is avoided by not using `include_str!` inside README code blocks. |
| A4 | The existing pre-commit hook automatically runs `cargo test --doc` as part of `make quality-gate`, so wiring `include_str!` will exercise the README doctests on every commit. | Validation Architecture | Low — CLAUDE.md explicitly states `make quality-gate` enforces "Doctest validation: All doctests must pass". `[CITED: /Users/guy/Development/mcp/sdk/rust-mcp-sdk/CLAUDE.md § "Pre-Commit Quality Gates"]`. If the hook config has drifted, the planner's task should include a verification step to confirm `make quality-gate` actually invokes `cargo test --doc`. |
| A5 | The `docs/advanced/migration-from-typescript.md` file at lines 122-124 uses `#[tool_router]` and `#[tool(...)]` and is the full set of locations in that file needing update. | D-16, research validation | Low — `[VERIFIED: grep -n '#\[tool' docs/advanced/migration-from-typescript.md]` returned exactly 2 matches at lines 122, 124. Confirmed complete. |
| A6 | The workspace root `CHANGELOG.md`'s `## [2.2.0]` entry pattern of multi-crate sub-headings (`### pmcp 2.2.0`, `### pmcp-macros 0.4.1`, `### mcp-tester 0.5.0`, `### cargo-pmcp 0.6.0`) is the established convention for multi-crate releases, and phase 66's root CHANGELOG entry should follow the same shape. | Standard Stack — Alternatives Considered | Low — directly verified in the file `[VERIFIED: /Users/guy/Development/mcp/sdk/rust-mcp-sdk/CHANGELOG.md:10-41]`. |
| A7 | `cargo test --doc -p pmcp-macros` runs README-included doctests and executes at most the body under `#[cfg(doctest)] pub struct ReadmeDoctests;`. The hidden struct does NOT pollute the public API of `pmcp-macros` because `#[cfg(doctest)]` gates it out of non-test builds. | Pattern 1 | Low — `[CITED: doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html]` explicitly documents this pattern. |

**All assumptions are verifiable by a Wave 0 proof-of-concept task** that writes a 3-line `pmcp-macros/README.md`, adds `#![doc = include_str!("../README.md")]` to `lib.rs`, and runs `cargo test --doc -p pmcp-macros`. Recommend the planner include this as the first task.

## Open Questions

1. **Should phase 66 ALSO fix the stale `cargo run --example 63_mcp_tool_macro` headers inside `examples/s23_mcp_tool_macro.rs:14` and `examples/s24_mcp_prompt_macro.rs`?**
   - What we know: These are drift leftovers from Phase 65 (when examples were renumbered). The actual example file names are `s23_mcp_tool_macro.rs` and `s24_mcp_prompt_macro.rs`, but the `//!` doc headers still say `63_mcp_tool_macro` `[VERIFIED: examples/s23_mcp_tool_macro.rs:14]`.
   - What's unclear: Whether this is Phase 66's job or a follow-up fix. CONTEXT.md doesn't mention it. But D-11 says "updating their examples to reference renamed phase 65 example files (`s23_mcp_tool_macro`, `s24_mcp_prompt_macro`)" — so phase 66 is going to be *looking at* those example files. Fixing two stale `//!` lines while you're there is trivial and aligned with the phase's spirit.
   - Recommendation: **Include as a small additional task.** Flag to user during discuss-phase if not already addressed.

2. **Should `pmcp-macros/src/lib.rs` also add `#[cfg_attr(docsrs, feature(doc_auto_cfg))]` for automatic feature badges?**
   - What we know: That's Phase 67's territory (DRSD-01). Phase 66 is scoped to cleanup + docs rewrite, not docs.rs pipeline work.
   - What's unclear: Whether adding it now saves a round trip. One line of code, no functional impact.
   - Recommendation: **Do NOT add in phase 66.** Respect the phase boundary. Phase 67 owns `doc_auto_cfg`.

3. **Should we flip `edition = "2021"` → `edition = "2024"` on `pmcp-macros/Cargo.toml` to get improved `include_str!` path semantics?**
   - What we know: 2024 edition makes `include_str!` paths inside doctests resolve relative to the included file, not the source file `[CITED: doc.rust-lang.org/edition-guide/rust-2024/rustdoc-nested-includes.html]`.
   - What's unclear: Whether the 2024 edition upgrade has other ripple effects on a proc-macro crate (new clippy lints, new syntax requirements, etc.).
   - Recommendation: **Do NOT flip in phase 66.** Edition migrations are non-trivial and should be a dedicated phase. The 2021 behavior is well-understood and pitfall 1's avoidance is easy (don't use `include_str!` inside README code blocks).

4. **Does the planner want a "README experiment" Wave 0 task to validate the `include_str!` + `use pmcp_macros::mcp_resource` mechanics before committing to the full rewrite?**
   - What we know: This de-risks assumptions A2 and A7 in ~2 minutes of work.
   - What's unclear: Whether the planner prefers to bundle proof-of-concept validation into the main rewrite task.
   - Recommendation: **YES, separate Wave 0 task.** Small time investment, high information value, catches assumption failures before the executor commits to rewriting 200+ lines.

5. **Should the `pmcp` v2.3.0 root-CHANGELOG.md entry follow the existing multi-crate sub-heading pattern (`### pmcp 2.3.0`, `### pmcp-macros 0.5.0`), or a simpler single-crate format?**
   - What we know: The existing workspace pattern (2026-04-06 entry) uses multi-crate sub-headings even when only some crates change.
   - What's unclear: Whether consistency with the existing pattern is worth the mild verbosity for a 2-crate release.
   - Recommendation: **Follow the existing pattern.** Workspace consistency > marginal verbosity savings. `### pmcp 2.3.0` + `### pmcp-macros 0.5.0` as H3 subsections under a single `## [2.3.0] - 2026-04-XX` H2 entry.

## Sources

### Primary (HIGH confidence)

- **The Rustdoc Book — Documentation Tests** — `https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html` — authoritative source for `rust,no_run` vs `rust,ignore` semantics, `#[cfg(doctest)] pub struct ReadmeDoctests;` pattern, `#[doc = include_str!]` mechanics. Fetched directly via WebFetch, key quotes captured in Standard Stack and Architecture Patterns sections.
- **rmcp-macros/src/lib.rs** (official Model Context Protocol Rust SDK) — `https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/src/lib.rs` — **directly verified** line 1 is `#![doc = include_str!("../README.md")]`. This is the explicit reference target for the phase. Fetched via WebFetch.
- **rmcp-macros/CHANGELOG.md** — `https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp-macros/CHANGELOG.md` — **directly verified** Keep a Changelog 1.0.0 format with `## [Unreleased]` section and `### Added` / `### Other` subsections. Fetched via WebFetch.
- **tracing-attributes/Cargo.toml** — `https://raw.githubusercontent.com/tokio-rs/tracing/master/tracing-attributes/Cargo.toml` — **directly verified** `tracing = { path = "../tracing", version = "0.2" }` as dev-dependency. Fetched via WebFetch. This is the battle-tested equivalent of `pmcp-macros`' existing `pmcp` dev-dep.
- **tracing-attributes/src/lib.rs** — `https://raw.githubusercontent.com/tokio-rs/tracing/master/tracing-attributes/src/lib.rs` — **directly verified** doctests use `use tracing::instrument;` (not `use tracing_attributes::instrument;`) and compile successfully as plain `rust` blocks. Fetched via WebFetch. This is the empirical proof that the D-09 + `use pmcp::{...}` pattern works for a `proc-macro = true` crate.
- **rust-lang/rust Issue #58700 — "Writing testable documentation examples for proc macros is not possible"** — `https://github.com/rust-lang/rust/issues/58700` — authoritative documentation of the proc-macro-doctest cyclic-dependency limitation. Fetched via WebFetch. Confirmed the workarounds available.
- **The Rust Edition Guide — Rust 2024 rustdoc nested includes** — `https://doc.rust-lang.org/edition-guide/rust-2024/rustdoc-nested-includes.html` — authoritative description of the edition 2021 vs 2024 behavior difference for `include_str!` paths inside doctests.
- **Workspace `CHANGELOG.md`** — `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CHANGELOG.md` — directly read lines 1-60. Confirms Keep a Changelog 1.0.0 is the workspace convention and shows the multi-crate sub-heading pattern for v2.2.0.
- **Workspace `CLAUDE.md`** — `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/CLAUDE.md` — directly read. Release workflow publish order (`pmcp-macros` → `pmcp`) and quality gate requirements confirmed.
- **`pmcp-macros/Cargo.toml`** — `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/pmcp-macros/Cargo.toml` — directly read. Confirmed `version = "0.4.1"`, `edition = "2021"`, `rust-version = "1.82.0"`, `[lib] proc-macro = true`, and the `pmcp = { version = ">=1.20.0", path = "..", features = ["full"] }` dev-dependency that unlocks `rust,no_run` doctests.
- **`pmcp-macros/src/lib.rs`** — `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/pmcp-macros/src/lib.rs` — directly read. Confirmed current state of all `//!` module docs (lines 1-53 to be deleted) and all `///` per-macro docs (lines 68-96 stale, rest to be rewritten).
- **Current `pmcp-macros/README.md`** — `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/pmcp-macros/README.md` — directly read. Confirmed 252 lines of obsolete content referencing `pmcp = "1.1"`, `#[tool]`, `#[tool_router]` — wholesale replacement target.
- **`src/lib.rs:147`** — `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/lib.rs` — directly read. Confirmed the re-export gap: `pub use pmcp_macros::{mcp_prompt, mcp_server, mcp_tool};` — no `mcp_resource`. This is the source of Pitfall 4.
- **Downstream consumer file locations** (`pmcp-course/src/**/*.md`, `docs/advanced/migration-from-typescript.md`) — directly grepped for `#[tool` references. All 5+2 = 7 locations verified.

### Secondary (MEDIUM confidence)

- **Linebender blog — "`#![doc = include_str!()]` with intra-doc links"** — `https://linebender.org/blog/doc-include/` — community-authored but technically accurate discussion of `include_str!` limitations (hidden lines, relative file links, intra-doc link workarounds). Cross-verified key claims against the rustdoc book.
- **cargo-rdme** — `https://github.com/orium/cargo-rdme` — discovery that this tool flows README← rustdoc (wrong direction for phase 66). WebFetch result.
- **cargo-sync-rdme** — `https://github.com/gifnksm/cargo-sync-rdme` — same finding as cargo-rdme, plus nightly-toolchain requirement which is an immediate blocker for stable-only workspace. WebFetch result.
- **rust-lang/rust Issue #81070** — `https://github.com/rust-lang/rust/issues/81070` — doctest diagnostic span issue, relevant to Pitfall 5. WebFetch result; closed as resolved but exact 2024-edition resolution details not captured in the issue thread.

### Tertiary (LOW confidence, not used for load-bearing claims)

- **"Nine Rules for Creating Procedural Macros in Rust" (Towards Data Science)** — surfaced during WebSearch, not fetched. Community blog content, unnecessary for phase 66 (rustdoc book and rmcp-macros reference are sufficient).
- **DeepWiki pages for modelcontextprotocol/rust-sdk** — surfaced during WebSearch, not fetched. DeepWiki is AI-generated summaries of repos; always prefer direct reads of the raw source files.

## Metadata

**Confidence breakdown:**

- **Standard Stack**: HIGH — every tool/pattern verified against rustdoc book or a live reference implementation (`rmcp-macros`, `tracing-attributes`). Zero assumptions.
- **Architecture patterns**: HIGH — Pattern 1 and Pattern 3 directly mirror verified reference implementations. Pattern 2 has ONE subtle piece (assumption A2 for `mcp_resource`) that should be validated in Wave 0.
- **Common Pitfalls**: HIGH — every pitfall has a cited source or a directly verified reproduction case (e.g., "current README says `pmcp = 1.1`" is a `cat` away).
- **rmcp Benchmark**: HIGH — every claim about rmcp-macros' behavior was directly fetched from raw GitHub content, not inferred or DeepWiki'd.
- **Release coordination**: HIGH — CLAUDE.md is authoritative and directly quoted.
- **Validation Architecture**: HIGH — existing test infrastructure is verified, `make quality-gate` behavior is CLAUDE.md-authoritative.
- **Security Domain**: HIGH — correctly assessed as minimal-surface, with the one marginal item (V14 version pinning hygiene) called out.

**Research date:** 2026-04-11
**Valid until:** 2026-05-11 (30 days — Rust ecosystem is stable on all the points researched; the only fast-moving thing is if rmcp-macros restructures its docs before phase 66 ships, which is unlikely in a 30-day window).

**Known conflicts with CONTEXT.md locked decisions:** None. All 25 decisions validated against Rust ecosystem practice. Where rmcp-macros differs from CONTEXT.md (e.g., uses `rust,ignore`, uses short gateway README), CONTEXT.md's choices are the correct divergence for pmcp-macros' context (see rmcp Benchmark — "Why pmcp-macros SHOULD NOT copy").

**Known additions to CONTEXT.md worth discussing with user:**

1. **Delete `pmcp-macros/tests/ui/tool_missing_description.rs` AND `.stderr`** — not explicitly listed in CONTEXT.md D-01's deletion set (which says "the `pmcp-macros/tests/ui/tool_missing_description.rs` UI test if any"). Both files verified to exist. Add to the deletion task list explicitly.
2. **Fix stale example-name headers in `examples/s23_mcp_tool_macro.rs:14` and `examples/s24_mcp_prompt_macro.rs`** — stale leftover from Phase 65, two-line fix, obviously in scope for phase 66 which is already touching these files via D-11. Open question #1.
3. **Wave 0 proof-of-concept task for `include_str!` mechanics** — de-risks assumptions A2 and A7 for ~2 minutes of work. Open question #4.
