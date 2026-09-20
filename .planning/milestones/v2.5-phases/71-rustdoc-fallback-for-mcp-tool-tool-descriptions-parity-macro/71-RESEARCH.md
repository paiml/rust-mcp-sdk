# Phase 71: Rustdoc fallback for #[mcp_tool] tool descriptions - Research

**Researched:** 2026-04-17
**Domain:** Rust proc-macro attribute parsing (pmcp-macros crate)
**Confidence:** HIGH

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PARITY-MACRO-01 | Support rustdoc as a fallback source for `#[mcp_tool]` descriptions, so well-documented tool functions do not have to repeat themselves in the macro attribute. | Current mandatory-`description` code paths located at two sites (standalone and impl-block), rmcp reference algorithm verified, existing `trybuild` snapshot pattern identified, call-site blast radius enumerated, version-bump ripple confirmed to end at pmcp root crate. |

## Overview

Phase 71 is a narrowly-scoped, mechanically straightforward change to `pmcp-macros`: when `#[mcp_tool]` is missing a `description = "..."` attribute, harvest the annotated function's `#[doc = "..."]` attributes (i.e. `///` rustdoc comments) and use the trimmed, newline-joined result as the description. The explicit attribute always wins; if neither is present, compile-fail with an error message that names both options. The change is additive and backwards-compatible — every existing call site continues to work because the attribute-wins precedence rule means sites that already specify `description = "..."` see no behavioral change even if they also have a rustdoc block.

The research confirms three non-obvious facts the planner must account for. **First**, the `#[mcp_tool]` attribute is parsed at TWO sites in pmcp-macros: `src/mcp_tool.rs:67` (standalone functions) and `src/mcp_server.rs:578` (impl-block methods, via `parse_mcp_tool_attr`). Both must implement the rustdoc-fallback for uniform behavior, and both feed the same `McpToolArgs` struct — which means the cleanest fix is synthesizing a `description = "..."` nested-meta *before* `McpToolArgs::from_list` is invoked, rather than changing `McpToolArgs::description` from `String` to `Option<String>`. **Second**, five example sites in `examples/s23_mcp_tool_macro.rs` already have `///` doc comments immediately above `#[mcp_tool(description = "...")]` — this means the backwards-compatibility test is non-trivial: a precedence bug would *silently* change tool descriptions on those sites from the attribute value to the rustdoc value. A unit test asserting byte-for-byte description equality is essential. **Third**, the "≥100 call-sites" claim in the proposal is overstated — actual live `#[mcp_tool(` invocations in the workspace total **25** across 6 files (enumerated below). The planner should use 25 as the population for the backwards-compatibility Nyquist sampling.

**Primary recommendation:** Implement the rustdoc-harvest logic as a shared helper in `pmcp-macros/src/mcp_common.rs` returning `Option<String>`, then call it at both attribute-parse sites *before* darling parses the nested metas. If no `description = "..."` is found in the attribute args AND the helper returns `Some(doc_text)`, synthesize a synthetic nested-meta `description = doc_text` and prepend it to the `Vec<NestedMeta>` passed to `McpToolArgs::from_list`. If both are missing, fall through to the existing hard-reject error with updated wording. This approach keeps `McpToolArgs::description: String` (no ripple through call sites in `mcp_server.rs` code-gen), localizes the new logic, and is symmetric across the two parse sites.

## Project Constraints (from CLAUDE.md)

- **Zero tolerance for defects** — pre-commit hook blocks commits on fmt/clippy/build/doctest failures
- **`make quality-gate`** must pass before commit (runs fmt-check, lint, build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always)
- **ALWAYS requirements for new features (MANDATORY):** FUZZ + PROPERTY + UNIT + EXAMPLE
- **Cognitive complexity ≤25** per function
- **Zero SATD comments** allowed
- **Comprehensive rustdoc with examples** required on public APIs
- **`--test-threads=1`** in CI (race-condition prevention) — pmcp-macros tests already honor this
- **Workspace publish order:** pmcp-widget-utils → pmcp → mcp-tester → mcp-preview → cargo-pmcp. `pmcp-macros` publishes transitively via pmcp's `macros` feature — it is NOT separately published through cargo-release; it releases with pmcp.
- **Semver:** new features = minor bump (pmcp-macros 0.5.0 → 0.6.0 per proposal)

## Current State

### Attribute parsing (standalone path)

**File:** `pmcp-macros/src/mcp_tool.rs`
**Parse function:** `expand_mcp_tool(args: TokenStream, input: &ItemFn)` at lines 67-326

**Attribute struct (lines 33-47):**
```rust
#[derive(Debug, FromMeta)]
pub struct McpToolArgs {
    /// Tool description (mandatory per D-05).
    pub(crate) description: String,        // ← currently String, not Option<String>
    #[darling(default)]
    pub(crate) name: Option<String>,
    #[darling(default)]
    pub(crate) annotations: Option<McpToolAnnotations>,
    #[darling(default)]
    pub(crate) ui: Option<syn::Expr>,
}
```

**Hard-reject site (lines 70-82):**
```rust
let nested_metas = if args.is_empty() {
    return Err(syn::Error::new_spanned(
        &input.sig.ident,
        "mcp_tool requires at least `description = \"...\"` attribute",
    ));
} else {
    let parser = syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    parser
        .parse2(args)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default()
};
let macro_args = McpToolArgs::from_list(&nested_metas)
    .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;
```

**Two failure modes today:**
1. `args.is_empty()` (i.e. `#[mcp_tool]` or `#[mcp_tool()]`) → hard-reject at line 72
2. `args` present but missing `description` → `McpToolArgs::from_list` returns darling error from line 85

### Attribute parsing (impl-block path — easy to miss)

**File:** `pmcp-macros/src/mcp_server.rs`
**Parse function:** `parse_mcp_tool_attr(attr: &syn::Attribute, method: &ImplItemFn)` at lines 577-604

```rust
fn parse_mcp_tool_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpToolArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_tool requires at least `description = \"...\"` attribute",
            ));
        },
        syn::Meta::NameValue(_) => { ... },
    };
    let parser = ...;
    let nested_metas = parser.parse2(tokens)...;
    McpToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}
```

**This is the second parse site the planner MUST update.** The `#[mcp_tool]` inside `#[mcp_server] impl` blocks (e.g. `examples/s23_mcp_tool_macro.rs:78-91`) flows through `parse_mcp_tool_attr`, not `expand_mcp_tool`. Updating only `mcp_tool.rs` would give inconsistent behavior: standalone `fn`s would support rustdoc fallback, impl-block methods would not.

### Current error messages (locked by trybuild snapshots)

**`pmcp-macros/tests/ui/mcp_tool_missing_description.stderr`:**
```
error: mcp_tool requires at least `description = "..."` attribute
  --> tests/ui/mcp_tool_missing_description.rs:12:10
   |
12 | async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
   |          ^^^^^^^^
```

The existing snapshot tests the `#[mcp_tool()]` (empty parens) case. After Phase 71, this same test case must fail with the NEW error wording because the function `bad_tool` also has no rustdoc. Any change to the error string will require regenerating this snapshot via `TRYBUILD=overwrite cargo test --test ui_tests` (ASSUMED: the exact test target name should be confirmed by running `cargo test -p pmcp-macros --list | grep -i ui`; the existing `tests/ui/` directory is used by trybuild per standard convention) [ASSUMED].

[VERIFIED: `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` lines 1-6]

### Current pmcp-macros version

- **`pmcp-macros/Cargo.toml:3`**: `version = "0.5.0"`
- **Root `Cargo.toml:53`**: `pmcp-macros = { version = "0.5.0", path = "pmcp-macros", optional = true }` (prod dep, optional via `macros` feature)
- **Root `Cargo.toml:147`**: `pmcp-macros = { version = "0.5.0", path = "pmcp-macros" }` (dev-dep for examples)
- **No other workspace members depend on pmcp-macros** — verified via `grep -rn "pmcp-macros" crates/ cargo-pmcp/ | grep Cargo.toml` returning empty.

**Version-bump ripple:** a pmcp-macros 0.5.0 → 0.6.0 minor bump requires ONLY updating the two `version = "0.5.0"` strings in the root `Cargo.toml` to `"0.6.0"`. No ripple into `mcp-tester`, `mcp-preview`, `cargo-pmcp`, `pmcp-widget-utils`, `pmcp-code-mode`, or `pmcp-code-mode-derive`. The pmcp root crate itself does NOT need a version bump for this change — it's additive within pmcp-macros and pmcp's public API is unchanged. (The decision of whether to piggyback a pmcp patch bump anyway for the docs.rs/crates.io view is a planner/release-captain call, not a research finding — document as an open question for Plan 3.) [VERIFIED: grep of all workspace Cargo.toml files]

## Proposed Change

### Normalization algorithm (verified against rmcp-v1.5.0/crates/rmcp-macros/src/common.rs)

rmcp's `extract_doc_line` function normalizes each `#[doc = "..."]` attribute by calling `.trim()` on the string literal, dropping empty-after-trim lines, and joining the rest with `"\n"`. We adapt the same approach but build a single `String` at macro-expansion time (rmcp builds a `concat!()` token tree for runtime assembly — unnecessary for us since we already know the lines at expansion time).

**Algorithm (deterministic, pure):**

```rust
/// Harvest `#[doc = "..."]` attributes on a syn item and return them as a normalized
/// description string, or `None` if no non-empty doc attributes are present.
///
/// Normalization rules:
/// 1. For each `#[doc = "LIT"]` attribute in source order:
///    a. Skip if the attribute's meta is not a `NameValue` with a string literal
///       (ignores `#[doc(hidden)]`, `#[doc(alias = "...")]`, etc.)
///    b. Take the literal's `.value()` and call `.trim()` on it (rmcp parity)
///    c. If the trimmed result is empty, skip this line
/// 2. Join remaining trimmed lines with `"\n"`
/// 3. If the final joined string is empty, return `None`; else `Some(string)`
pub(crate) fn extract_doc_description(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            continue; // skip #[doc(hidden)], #[doc(alias = ...)], etc.
        };
        let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit_str), .. }) = &nv.value else {
            continue;
        };
        let trimmed = lit_str.value().trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        lines.push(trimmed);
    }
    if lines.is_empty() { None } else { Some(lines.join("\n")) }
}
```

### Test vectors (lift directly into unit tests in Plan 1)

| # | Input (rustdoc source) | Expected normalized output |
|---|------------------------|-----------------------------|
| 1 | `/// Add two numbers.` | `"Add two numbers."` |
| 2 | `/// Add two numbers.`<br>`/// Returns their sum.` | `"Add two numbers.\nReturns their sum."` |
| 3 | `/// Line 1.`<br>`///`<br>`/// Line 2.` | `"Line 1.\nLine 2."` (blank middle line dropped — matches rmcp) |
| 4 | `///  Indented body.` | `"Indented body."` (leading whitespace trimmed) |
| 5 | `/// Line 1.  ` (trailing spaces) | `"Line 1."` (trailing whitespace trimmed) |
| 6 | (no `///` comments) | `None` |
| 7 | `///` (single empty comment) | `None` (all trimmed lines are empty) |
| 8 | `/// Line 1.`<br>`#[doc(hidden)]`<br>`/// Line 2.` | `"Line 1.\nLine 2."` (hidden non-NameValue attribute ignored) |
| 9 | `/// Line with "quotes"` | `"Line with \"quotes\""` (embedded quotes preserved — emitted via `LitStr`) |
| 10 | `///   `<br>`/// Real content.`<br>`///   ` | `"Real content."` (both whitespace-only lines dropped) |

**Key corner cases explicitly handled:**
- **Fenced code blocks:** rustdoc `///` lines inside a `/// ```` block are ordinary doc attributes from `syn`'s perspective (syn doesn't parse markdown); they get `.trim()`'d and joined with `\n`. This preserves the visual structure of triple-backtick fences. A fenced code block with indented body (`///    let x = 1;`) loses the 4-space indent after trim — **acceptable** because the output is a plain-text tool description displayed in UIs, not a rendered rustdoc HTML page. Document this tradeoff in the README migration section.
- **`//!` inner docs:** NOT applicable — `//!` attach to the enclosing module, not to the fn. `syn::ItemFn.attrs` and `syn::ImplItemFn.attrs` contain only outer attributes (`///` and `#[doc = "..."]`).
- **`#[doc = "..."]` written as a literal attribute:** handled identically to `///` by the above algorithm (syn lowers both to the same `NameValue` meta).
- **`#[doc(hidden)]` / `#[doc(alias = "foo")]`:** skipped (they are `Meta::List` or `Meta::Path`, not `Meta::NameValue`).

[VERIFIED: rmcp algorithm, against `rmcp-v1.5.0/crates/rmcp-macros/src/common.rs::extract_doc_line`]

### Precedence semantics

**Rule:** explicit `description = "..."` attribute wins over rustdoc. When both are present, the attribute value is used and the rustdoc is ignored. This is **silent** (no compiler warning).

**Rationale:** The proposal (`69-PROPOSALS.md` Proposal 3, Scope bullet 3) specifies silent win. Emitting a warning would require every existing call site in `examples/s23_mcp_tool_macro.rs` (5 sites) and the book/course code samples to either remove the rustdoc or remove the attribute — that's a behavior-change migration masquerading as a feature addition. Silent precedence preserves backwards compatibility without forcing churn.

**Codebase conventions that agree:** rustdoc comments above `#[mcp_tool]` sites in `examples/s23_mcp_tool_macro.rs` (lines 48, 56, 63) are meta-commentary about the site ("Minimal tool -- just args and return.", "Tool with shared state via `State<T>`.", "Sync tool -- auto-detected from `fn` (not `async fn`).") — they are NOT tool descriptions. Forcing the user to choose between "remove the meta-commentary" or "remove the attribute" would be a regression in DX.

**Implementation:** In the fallback-synthesis code, check whether `nested_metas` already contains a `description = ...` entry. If yes, do nothing (existing path wins). If no, harvest rustdoc and synthesize one. Trivial `nested_metas.iter().any(|m| /* matches description */)` check.

### Integration sketch (Plan 1 scope)

**Shared helper location:** `pmcp-macros/src/mcp_common.rs` (new function)

**Integration at standalone site (`src/mcp_tool.rs:70-85`):**
```rust
// Parse existing args into nested_metas (unchanged).
let mut nested_metas: Vec<darling::ast::NestedMeta> = if args.is_empty() {
    Vec::new()
} else {
    let parser = syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    parser.parse2(args)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default()
};

// NEW: if `description` is absent from nested_metas, try to harvest from rustdoc.
if !mcp_common::has_description_meta(&nested_metas) {
    if let Some(doc_desc) = mcp_common::extract_doc_description(&input.attrs) {
        let synthetic = mcp_common::build_description_meta(&doc_desc);
        nested_metas.push(synthetic);
    }
}

// NEW: if STILL no description, fail with updated error.
if !mcp_common::has_description_meta(&nested_metas) {
    return Err(syn::Error::new_spanned(
        &input.sig.ident,
        "mcp_tool requires either a `description = \"...\"` attribute or a rustdoc comment on the function",
    ));
}

// Unchanged from here.
let macro_args = McpToolArgs::from_list(&nested_metas)
    .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;
```

**Integration at impl-block site (`src/mcp_server.rs:577-604`):** symmetric — same three helper calls, same new error wording, using `method.attrs` in place of `input.attrs`.

**Helper signatures:**
```rust
pub(crate) fn extract_doc_description(attrs: &[syn::Attribute]) -> Option<String>;
pub(crate) fn has_description_meta(metas: &[darling::ast::NestedMeta]) -> bool;
pub(crate) fn build_description_meta(desc: &str) -> darling::ast::NestedMeta;
```

`build_description_meta` parses the string `description = "..."` (with proper escaping of the description content) through `syn::parse_str` to produce a `NestedMeta`. [ASSUMED: darling's `NestedMeta` round-trips through `syn::parse_str::<syn::Meta>` — standard pattern, but the planner should verify via a quick `cargo build -p pmcp-macros` during Plan 1 Wave 0 before committing to this approach. An alternative is to set `McpToolArgs::description: Option<String>` and resolve later in `expand_mcp_tool` — slightly more intrusive but avoids the string-round-trip. Either works; the helper-based approach is cleaner in the public API of `mcp_common`.]

### Updated error message (locks the trybuild snapshot)

**New text:**
```
mcp_tool requires either a `description = "..."` attribute or a rustdoc comment on the function
```

**Rationale:** Names both options explicitly (matching proposal Success Criterion 3), fits on one line (trybuild diff readability), preserves the `mcp_tool requires` prefix so grep-based error-lookup habits still work. Both parse sites (`mcp_tool.rs` and `mcp_server.rs`) MUST use this exact string — verified identical by a unit test in Plan 1 if desired.

## Integration Points

### Existing trybuild UI test pattern

**Directory:** `pmcp-macros/tests/ui/`
**Naming convention:** `<macro>_<failure_mode>.rs` + `<macro>_<failure_mode>.stderr` pair (e.g. `mcp_tool_missing_description.rs`/`.stderr`, `mcp_tool_multiple_args.rs`/`.stderr`).

**Existing shape (`mcp_tool_missing_description.rs`):**
```rust
use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Args { x: i32 }

// Missing description -- should fail at compile time (D-05)
#[mcp_tool()]
async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

fn main() {}
```

**Plan 2 must add:** `tests/ui/mcp_tool_missing_description_and_rustdoc.rs` (fn has neither rustdoc nor attribute — still fails but with the NEW error string). The existing `mcp_tool_missing_description.rs` will need its `.stderr` REGENERATED because the error wording changes — do this once via `TRYBUILD=overwrite cargo test` [ASSUMED: standard trybuild workflow] and hand-review the diff before committing.

**Note on test ergonomics:** the existing `mcp_tool_missing_description.rs` has `// Missing description -- should fail at compile time (D-05)` as a `//` line comment (NOT `///` rustdoc), so after Phase 71 it ALSO tests the "no rustdoc, no attribute" case implicitly — no behavior change to the test, only a new error message. Keep this file and its `.stderr` as-is except for the updated stderr text.

### README migration section

**Style target:** `pmcp-macros/README.md` — Phase 66 rewrite (355 lines). The `## `#[mcp_tool]`` section (lines 36-151) is the closest structural parallel. Plan 2 should add a new subsection — recommended placement: between the existing "Attributes" sub-section (line 46-52) and the existing "Example" sub-section (line 54), OR as a new "Rustdoc-derived descriptions (pmcp-macros 0.6.0+)" sub-section after the main Example.

**Required contents:**
1. One-paragraph explanation of the new behavior ("when `description = \"...\"` is omitted, the function's rustdoc is used instead")
2. A `rust,no_run` doctest showing the rustdoc-only form (satisfies the EXAMPLE requirement — see ALWAYS Requirements Map)
3. A precedence-rule sentence ("when both are present, the attribute wins")
4. The normalization rule summary ("leading/trailing whitespace trimmed, blank lines dropped, remaining lines joined with `\n`")
5. A version note ("Requires pmcp-macros 0.6.0 / pmcp ≥ TBD — planner to confirm in Plan 3")

**Also update the "Attributes" table (line 47-48):** change `description = "..."` — **required**. Human-readable description exposed via the MCP `tools/list` response.` → `description = "..."` — optional (falls back to function rustdoc if omitted). Human-readable description exposed via the MCP `tools/list` response.` And mark the old wording in a changelog note.

### Version-bump ripple (Plan 3 scope)

**Confirmed ripple targets (all in the repo root `Cargo.toml`):**
1. Line 53: `pmcp-macros = { version = "0.5.0", path = "pmcp-macros", optional = true }` → `"0.6.0"`
2. Line 147: `pmcp-macros = { version = "0.5.0", path = "pmcp-macros" }` → `"0.6.0"`
3. `pmcp-macros/Cargo.toml` line 3: `version = "0.5.0"` → `"0.6.0"`

**NO ripple into:** `crates/mcp-tester/Cargo.toml`, `crates/mcp-preview/Cargo.toml`, `cargo-pmcp/Cargo.toml`, `crates/pmcp-widget-utils/Cargo.toml`, `crates/pmcp-code-mode/Cargo.toml`, `crates/pmcp-code-mode-derive/Cargo.toml` — none of these list `pmcp-macros` as a direct dependency. [VERIFIED: `grep -rn "pmcp-macros" crates/ cargo-pmcp/ | grep Cargo.toml` returned zero matches]

**Open for planner resolution in Plan 3:** whether pmcp itself needs a concurrent patch bump. Recommendation: YES — bump pmcp from whatever the current version is (STATE.md indicates 2.3.x) to the next patch. Reasoning: (a) the pmcp crate re-exports `pmcp_macros::mcp_tool` so users upgrading pmcp pick up the new behavior by transitive version bump; (b) publishing pmcp-macros 0.6.0 WITHOUT a pmcp bump means users on `pmcp = "2.3"` pick up the new behavior without a pmcp version signal, which violates semver signaling discipline. CLAUDE.md's "Release Steps" section (line 5 of its Version Bump Rules) explicitly says: "Downstream crates that pin a bumped dependency must also be bumped". Confirm the current pmcp version by reading root `Cargo.toml` at plan time; STATE.md line 62 suggests v2.3.0 was reached during v2.x breaking-change window.

### Doctest in README — CI visibility

The `rust,no_run` doctest added in Plan 2 is compiled by `cargo test --doc -p pmcp-macros` (and by `cargo test --doc` at the workspace level). This is run by `make quality-gate` via `make test-all` → `make test-doc`. Confirmed by reading `Makefile:218`:

```
$(CARGO) test --doc --features "full"
```

No new CI wiring needed.

## Call-Site Blast Radius

### Population enumeration

Grep result for `^#\[mcp_tool\(` (accounting for leading whitespace on impl-block methods):

| File | Sites | Shape |
|------|-------|-------|
| `pmcp-macros/tests/mcp_server_tests.rs` | 9 | Impl-block `#[mcp_tool(description = ...)]` on `&self` methods |
| `pmcp-macros/tests/mcp_tool_tests.rs` | 8 | Standalone `#[mcp_tool(description = ...)]` on `async fn`/`fn` |
| `examples/s23_mcp_tool_macro.rs` | 5 | 3 standalone + 2 impl-block |
| `pmcp-macros/tests/ui/mcp_tool_missing_description.rs` | 1 | Intentional failure (empty parens) |
| `pmcp-macros/tests/ui/mcp_tool_multiple_args.rs` | 1 | Intentional failure (two args) |
| `examples/s24_mcp_prompt_macro.rs` | 1 | Impl-block method (inside an `#[mcp_server]` with prompts) |
| **Total** | **25** | |

The proposal's "≥100 call-sites" figure counts every OCCURRENCE of the string `mcp_tool` across all files (including `///` rustdoc mentions, `//!` module docs, `// Find #[mcp_tool(...)] attribute` comments in the macro's own source, etc.) — which `grep` returned as 67 total matches. Live invocations are **25**. This is the number the Nyquist sampling plan uses.

### Diverse before/after sample (5 sites from examples/)

All 5 example sites preserve current behavior under the attribute-wins rule. If the precedence logic had a bug (rustdoc winning over attribute), tool descriptions would silently change as shown in the "Regression risk" column.

| # | Site | Current rustdoc (above attr) | Current `description = "..."` | After Phase 71 (attribute wins) | Regression risk if precedence bug |
|---|------|------------------------------|-------------------------------|----------------------------------|-----------------------------------|
| 1 | `examples/s23_mcp_tool_macro.rs:48-54` | `/// Minimal tool -- just args and return.` | `"Add two numbers"` | `"Add two numbers"` (unchanged) | Would wrongly change to "Minimal tool -- just args and return." — wrong for MCP client UIs |
| 2 | `examples/s23_mcp_tool_macro.rs:56-60` | `/// Tool with shared state via `State<T>`.` | `"Greet with prefix from config"` | `"Greet with prefix from config"` (unchanged) | Would wrongly change to the meta-commentary |
| 3 | `examples/s23_mcp_tool_macro.rs:63-66` | `/// Sync tool -- auto-detected from `fn` (not `async fn`).` | `"Get server version"` | `"Get server version"` (unchanged) | Would wrongly change to the meta-commentary |
| 4 | `examples/s23_mcp_tool_macro.rs:78-83` (impl-block) | (no rustdoc) | `"Multiply two numbers"` | `"Multiply two numbers"` (unchanged) | N/A (no rustdoc to accidentally win) |
| 5 | `examples/s23_mcp_tool_macro.rs:85-88` (impl-block) | (no rustdoc) | `"Health check"` | `"Health check"` (unchanged) | N/A |

**Conclusion:** 3 of the 5 example sites have rustdoc directly above `#[mcp_tool]`. The precedence-wins-silently test is therefore load-bearing: a bug would corrupt 3 production tool descriptions at once. Plan 1 MUST include a unit test asserting byte-for-byte description equality for a rustdoc-plus-attribute combo case (test vector: rustdoc "IGNORED" + attribute "WINS" → description == "WINS"). [VERIFIED: sed -n '40,95p' examples/s23_mcp_tool_macro.rs]

### Test-file sites (10 sites in pmcp-macros/tests/)

None of the 9 `#[mcp_tool(description = ...)]` sites in `mcp_server_tests.rs` or the 8 sites in `mcp_tool_tests.rs` have `///` rustdoc immediately above — all use `//` line comments as section dividers (e.g. `// === Test 1: Minimal async tool ===`). This means the test suite's existing 17 sites exercise **only** the attribute-present path. After Phase 71, Plan 1's new unit tests must add:
- One `#[mcp_tool]` with rustdoc only, no attribute → description == rustdoc text
- One `#[mcp_tool(description = "WINS")]` with rustdoc "IGNORED" → description == "WINS"
- One `#[mcp_tool]` with multi-line rustdoc → description == joined lines
- One `#[mcp_tool]` with rustdoc inside an impl block → description == rustdoc (exercises the second parse site)

### Unusual-shape audit

Scanned all 25 live invocation sites. **No cfg-gated** `#[mcp_tool]` sites found. **No sites wrapped in a user macro** found. **No sites with pre-existing `#[doc = "..."]` attribute notation** (all use the `///` syntactic sugar). **No `#[mcp_tool]` with raw-string literals** in `description = r#"..."#` form. The shape of the live population is uniform — the only blast-radius nuance is the 3 example sites from row above that have rustdoc + attribute coexistence.

## ALWAYS Requirements Map

Per CLAUDE.md mandatory ALWAYS requirements for new features (FUZZ + PROPERTY + UNIT + EXAMPLE):

### UNIT (Plan 1)

Covered by Plan 1's unit-test module for `extract_doc_description` and the integration tests for the two parse sites. Test count target: ≥10 (matching the test-vector table above) plus 4 at-the-macro-site integration tests for precedence and "neither present" failure. No coverage gap.

### PROPERTY (Plan 1)

**Target location:** `pmcp-macros/tests/properties.rs` (new file, matching `pmcp-macros/Cargo.toml:36` which already lists `proptest = "1.6"` as a dev-dep — verified). Run via `PROPTEST_CASES=1000` in CI per `Makefile:224`.

**Invariants to verify (≥4 proptest cases):**
1. **Idempotence of trim-join:** for any `Vec<String>` of simulated doc lines, `extract_doc_description(synthesize_attrs(lines))` equals `lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")` (or `None` if the filtered vec is empty).
2. **Attribute wins:** for any `(attr_desc: String, rustdoc_lines: Vec<String>)` pair where both produce non-empty descriptions, the emitted `McpToolArgs::description` always equals the attr_desc after macro expansion.
3. **Hard-fail symmetry:** a synthesized `ItemFn` with no rustdoc and no `description` attribute always produces a `syn::Error` with message containing `"mcp_tool requires either"`.
4. **No panics:** the normalization helper never panics on any `Vec<syn::Attribute>` input — handles `Meta::Path`, `Meta::List`, malformed `NameValue`, non-string-literal values without unwrap.

### FUZZ (Plan 2 or 3)

**Target location:** `fuzz/fuzz_targets/fuzz_rustdoc_normalize.rs` (new file, added to `fuzz/Cargo.toml` as a new `[[bin]]` entry — pattern identical to existing `fuzz_peer_handle.rs` at line 77-81).

**Caveat on proc-macro fuzzing:** `cargo fuzz` cannot directly fuzz a proc-macro (proc-macros run at compile time; libfuzzer runs at runtime). The correct approach is to **extract the pure normalization logic** as `pub fn` in a library module of `pmcp-macros` that does NOT require proc-macro context — specifically, extract `extract_doc_description` to take a `&[(bool /*is_doc*/, String /*literal*/)]` mock input rather than `&[syn::Attribute]`. Alternatively, construct fuzz-generated `syn::Attribute` values via `syn::parse_str` in the target and feed them to the real helper.

**Recommended approach:** second path (real helper, synthesize attrs via `syn::parse_str`). Fuzz input: `&[u8]` interpreted as a UTF-8 string containing a sequence of `///`-prefixed lines delimited by `\n`; the target parses those into synthetic doc attrs and calls `extract_doc_description`. Invariant: no panic, output either `None` or a non-empty `String` matching the "trim each, filter empty, join" spec.

**Runtime:** `timeout 30s cargo fuzz run fuzz_rustdoc_normalize` per `Makefile:233` existing discipline.

### EXAMPLE

**The proposal explicitly specifies:** "one new compiling `rust,no_run` doctest in the pmcp-macros README demonstrating rustdoc-only usage". This is the EXAMPLE deliverable — a standalone `examples/XX_rustdoc_tool.rs` is NOT required and would be redundant with existing `examples/s23_mcp_tool_macro.rs`. The README doctest (Plan 2) satisfies the ALWAYS EXAMPLE requirement for the following reasons:
- `make test-doc` compiles and runs it, so it's CI-verified
- It's visible in published rustdoc on docs.rs, driving adoption
- s23 can receive ONE new standalone-fn site added to its existing flow (optional — planner discretion) to demonstrate the rustdoc-only form end-to-end with `ServerBuilder::tool(...)` registration and `tools/list` response

**Recommended minimum:** README doctest only. **Recommended stretch:** also add a single rustdoc-only tool to `examples/s23_mcp_tool_macro.rs` alongside the existing attribute-only tools, so the "≥100 call-sites" backwards-compat claim is backed by a concrete mixed-style example. Planner's call.

### Quality-gate hook summary

`make quality-gate` runs in this order (verified, `Makefile:460-479`):
1. `fmt-check` — `cargo fmt --all -- --check`
2. `lint` — clippy with `--features "full"`, pedantic + nursery lint groups
3. `build` — full workspace build
4. `test-all` → test-unit + test-doc + test-property + test-examples + test-integration
5. `audit` — `cargo audit`
6. `unused-deps` — `cargo machete` or similar
7. `check-todos` — zero SATD enforcement
8. `check-unwraps` — zero `.unwrap()` in prod code
9. `validate-always` — ALWAYS requirements gate

**No trybuild-specific gate exists.** Trybuild runs as part of `cargo test -p pmcp-macros` via the `tests/` harness, inside `test-all → test-integration`. This is already wired.

**`cargo check --workspace --examples`** (proposal Success Criterion 4) is not a Make target directly, but is equivalent to the `examples` portion of `test-examples` (`Makefile:244-253` loops `cargo build --example X` over all examples). The planner's Plan 3 acceptance test can invoke it directly:

```
cargo check --workspace --examples --features full
```

## Validation Architecture

### Test framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (unit + integration) + `proptest = "1.6"` (property) + `trybuild = "1.0"` (UI/compile-fail) + `cargo fuzz` (fuzz) |
| Config file | `pmcp-macros/Cargo.toml:26-37` (dev-deps) + `fuzz/Cargo.toml` (fuzz targets) |
| Quick run command | `cargo test -p pmcp-macros --features full` (unit + integration + UI) |
| Full suite command | `make quality-gate` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PARITY-MACRO-01 | Rustdoc-only → description == normalized rustdoc | unit | `cargo test -p pmcp-macros test_rustdoc_only_description` | ❌ Wave 0 — Plan 1 creates |
| PARITY-MACRO-01 | Attribute wins over rustdoc | unit | `cargo test -p pmcp-macros test_attribute_wins_over_rustdoc` | ❌ Wave 0 — Plan 1 creates |
| PARITY-MACRO-01 | Multi-line rustdoc normalization | unit | `cargo test -p pmcp-macros test_multiline_rustdoc_normalization` | ❌ Wave 0 — Plan 1 creates |
| PARITY-MACRO-01 | Impl-block rustdoc harvest (second parse site) | unit | `cargo test -p pmcp-macros test_impl_block_rustdoc_harvest` | ❌ Wave 0 — Plan 1 creates |
| PARITY-MACRO-01 | Neither rustdoc nor attribute → compile-fail | ui/trybuild | `cargo test -p pmcp-macros --test trybuild_ui` [ASSUMED test target name; confirm in Wave 0] | ❌ Wave 0 — Plan 2 creates `mcp_tool_missing_description_and_rustdoc.rs` |
| PARITY-MACRO-01 | Normalization property invariants (≥4 props, 1000 cases each) | property | `PROPTEST_CASES=1000 cargo test -p pmcp-macros --test properties -- property_` | ❌ Wave 0 — Plan 1 creates `pmcp-macros/tests/properties.rs` |
| PARITY-MACRO-01 | Normalization never panics on adversarial input | fuzz | `cd fuzz && cargo fuzz run fuzz_rustdoc_normalize` (30s per run) | ❌ Wave 0 — Plan 2 or 3 creates `fuzz/fuzz_targets/fuzz_rustdoc_normalize.rs` |
| PARITY-MACRO-01 | All 25 existing `#[mcp_tool]` sites compile unchanged | integration | `cargo check --workspace --examples --features full` | ✅ exists — verifies backwards compat |
| PARITY-MACRO-01 | README doctest showing rustdoc-only form compiles + runs | doctest | `cargo test --doc -p pmcp-macros --features full` | ❌ Wave 0 — Plan 2 adds the doctest |
| PARITY-MACRO-01 | Version bump (0.5.0 → 0.6.0) consistent across 3 Cargo.toml strings | unit (grep-based CI check) | `rg '"0\.5\.0"' pmcp-macros/Cargo.toml Cargo.toml \| wc -l` → 0 after bump | ✅ verifiable by grep |

### Sampling rate (Nyquist for backwards compatibility)

**Population:** 25 live `#[mcp_tool]` invocation sites (enumerated above).

**Population class:** The 25 sites are already exhaustively enumerated by a single grep command — this is a *finite, fully-observable population*, not a sampled one. The appropriate discipline is **100% inspection**, not statistical sampling. Plan 2 or 3 must run `cargo check --workspace --examples --features full` to verify all 25 sites compile unchanged AND run `cargo test -p pmcp-macros --features full` to verify all sites produce the same tool description they did before (the test suite already asserts this via `assert_eq!(meta.description.as_deref(), Some("..."))` at sites like `tests/mcp_tool_tests.rs:46`).

**Detailed before/after audit:** the 5 example sites in the Call-Site Blast Radius section are the Nyquist-sampled cases chosen because they are the ONLY population members with rustdoc coexisting with the attribute. 3 of 5 (60%) exercise the risky precedence path. Auditing these 3 catches the precedence-bug regression class with probability 1.

**Verification rule:** if `cargo check` passes AND the full `cargo test -p pmcp-macros` test matrix passes (including the existing `test_echo_tool_metadata`-style assertions that pin `description` strings byte-for-byte), then no existing site regressed. Coverage of the 25-site population is 100%.

### Sampling rate (per-commit / per-wave)

- **Per task commit:** `cargo test -p pmcp-macros --features full` (runs unit + integration + trybuild; ≈ 15–30 s)
- **Per wave merge:** `make quality-gate` (full fmt + lint + build + test-all + audit + validate-always; ≈ 3–6 min)
- **Phase gate (before `/gsd-verify-work`):** `make quality-gate` + `cd fuzz && timeout 30s cargo fuzz run fuzz_rustdoc_normalize` + `cargo test --doc -p pmcp-macros --features full` green

### Wave 0 gaps

- [ ] `pmcp-macros/tests/properties.rs` — new file, property tests for normalization invariants (REQ-PARITY-MACRO-01)
- [ ] `pmcp-macros/tests/ui/mcp_tool_missing_description_and_rustdoc.rs` + `.stderr` — new trybuild snapshot
- [ ] `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` — REGENERATE with new error wording (existing file)
- [ ] `fuzz/fuzz_targets/fuzz_rustdoc_normalize.rs` + `[[bin]]` entry in `fuzz/Cargo.toml` — new fuzz target
- [ ] `pmcp-macros/src/mcp_common.rs` — new helpers `extract_doc_description`, `has_description_meta`, `build_description_meta`
- [ ] `pmcp-macros/README.md` — new "Rustdoc-derived descriptions (pmcp-macros 0.6.0+)" subsection with `rust,no_run` doctest

No framework install needed — `proptest`, `trybuild`, `cargo fuzz`, `tokio`, etc. all present in existing `pmcp-macros/Cargo.toml` and `fuzz/Cargo.toml`.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` + stable Rust | everything | ✓ | 1.82.0+ (pmcp-macros `rust-version = "1.82.0"`) | — |
| `syn` | macro expansion | ✓ | 2.0 (pmcp-macros/Cargo.toml:20) | — |
| `quote` | macro expansion | ✓ | 1.0 | — |
| `darling` | attribute parsing | ✓ | 0.23 | — |
| `proptest` | property tests | ✓ | 1.6 | — |
| `trybuild` | UI tests | ✓ | 1.0 | — |
| `cargo-fuzz` | fuzz harness | ? | (unverified locally — CLAUDE.md says fuzz targets are supported) | `proptest` expanded to 10_000 cases as surrogate if cargo-fuzz unavailable |
| `pmat` | quality-gate proxy | ? | (per CLAUDE.md — MCP server) | `make quality-gate` local-only run covers 100% of pre-commit checks |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `cargo-fuzz` — if not installed on the dev machine, Plan 2 or 3 can substitute an expanded proptest suite (10_000 cases on the same normalization invariants). The ALWAYS FUZZ requirement is satisfied either way because `make test-fuzz` (Makefile:227-238) already handles the "no fuzz directory" case with a graceful yellow warning.

## Open Questions (RESOLVED)

1. **Should the `#[mcp_prompt]` macro receive the same treatment in a sibling follow-on?**
   - What we know: `pmcp-macros/src/mcp_prompt.rs:40,52` has the exact same mandatory-description structure. The 69-RESEARCH.md MACRO-02 row is tool-specific; the prompt variant is Medium severity at best (not on the High list).
   - What's unclear: whether users are asking for the prompt variant.
   - **RESOLVED:** explicitly OUT OF SCOPE for Phase 71 (already stated in proposal). Keep the helper `extract_doc_description` general-purpose so a future phase can wire it into `mcp_prompt.rs` in 1 LOC without research rework.

2. **Does pmcp itself need a version bump alongside pmcp-macros 0.6.0?**
   - What we know: no other workspace member depends on pmcp-macros directly; pmcp is the only caller. CLAUDE.md's Version Bump Rules say "Downstream crates that pin a bumped dependency must also be bumped".
   - What's unclear: whether the current pmcp release cycle is mid-release or can absorb a same-commit minor.
   - **RESOLVED:** Plan 3 decides — propose pmcp patch bump (e.g. 2.3.0 → 2.3.1 or whatever the current version is → next patch) to surface the new behavior to end-users via a pmcp version signal. Confirm current pmcp version at plan-time by reading root `Cargo.toml`.

3. **Is a dedicated standalone `examples/NN_rustdoc_tool.rs` file needed, or does the README doctest plus a s23 addition suffice?**
   - What we know: proposal explicitly calls for "one new compiling `rust,no_run` doctest in the pmcp-macros README" — doctest only.
   - What's unclear: whether the Phase 66 example-naming convention (role-prefix `s`/`c`) requires a standalone file for v2.1-milestone consistency.
   - **RESOLVED:** README doctest is sufficient for ALWAYS EXAMPLE. If adding to s23, use a single additional standalone `#[mcp_tool]` with rustdoc-only (≈ 6 lines) rather than creating a whole new numbered example file.

4. **Are `syn::parse_str` round-trips through `darling::ast::NestedMeta` reliable for synthesizing `description = "..."` entries at expansion time?**
   - What we know: darling's `NestedMeta` wraps `syn::Meta`, which is constructible from a string via `syn::parse_str::<syn::Meta>`. This is used in many proc-macros.
   - What's unclear: exactly how to escape a rustdoc string with embedded quotes or backslashes into a Rust string literal that will round-trip correctly. The safe path is to use `syn::LitStr::new(doc_text, span)` and build the `Meta::NameValue` directly, bypassing string formatting.
   - **RESOLVED:** Plan 1 Wave 0 task: spike a 10-line test constructing a synthetic `NestedMeta` with an embedded-quote rustdoc string and round-tripping it through `McpToolArgs::from_list`. If round-trip fails, fall back to making `McpToolArgs::description: Option<String>` and resolving in `expand_mcp_tool` body.

## Recommended Plan Split

**The 3-plan split from the proposal is VALIDATED with one refinement:**

### Plan 71-01: Core macro change + unit tests + property tests
**Scope:**
- Add `extract_doc_description`, `has_description_meta`, `build_description_meta` helpers to `pmcp-macros/src/mcp_common.rs`
- Wire the 3-helper sequence into `src/mcp_tool.rs::expand_mcp_tool` (standalone path)
- Wire the identical sequence into `src/mcp_server.rs::parse_mcp_tool_attr` (impl-block path)
- Update the error message at BOTH sites to the new "either ... or ..." wording
- Add ≥10 unit tests (10 test vectors above) + ≥4 impl-block integration tests
- Add `pmcp-macros/tests/properties.rs` with ≥4 property tests (invariants section above), 1000 cases each

**Acceptance:**
- All new unit + property tests green
- All 17 existing tests in `mcp_tool_tests.rs` + `mcp_server_tests.rs` still pass (byte-for-byte descriptions unchanged)
- `cargo check --workspace --examples --features full` green

### Plan 71-02: trybuild snapshot + README migration section + README doctest + fuzz target
**Scope:**
- REGENERATE `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` with new error wording
- ADD `pmcp-macros/tests/ui/mcp_tool_missing_description_and_rustdoc.rs` + `.stderr` (a fn with truly no rustdoc and no attribute — this is what the existing file *accidentally* already is, but adding an explicit test pinpointing the rustdoc-absence path makes the intent clear)
- ADD new subsection "Rustdoc-derived descriptions (pmcp-macros 0.6.0+)" in `pmcp-macros/README.md` + `rust,no_run` doctest
- UPDATE README "Attributes" table: mark `description = "..."` as optional with fallback note
- ADD `fuzz/fuzz_targets/fuzz_rustdoc_normalize.rs` + `[[bin]]` entry in `fuzz/Cargo.toml`

**Acceptance:**
- `cargo test -p pmcp-macros --features full` green (including both trybuild snapshots)
- `cargo test --doc -p pmcp-macros --features full` green
- `timeout 30s cargo fuzz run fuzz_rustdoc_normalize` completes without panic
- `make quality-gate` green

### Plan 71-03: Version bump + CI verification + STATE/REQUIREMENTS update
**Scope:**
- Bump `pmcp-macros/Cargo.toml` 0.5.0 → 0.6.0
- Bump both `pmcp-macros = { version = "0.5.0", ... }` strings in root `Cargo.toml` to `"0.6.0"`
- Decide (and document) whether to concurrent-bump pmcp root patch (recommended: yes, see Open Question 2)
- If pmcp root bump: update CHANGELOG.md entry
- Update `.planning/REQUIREMENTS.md` traceability row for `PARITY-MACRO-01` from `TBD | Pending` to `Phase 71 | Complete` (conditional on CI success)
- Run `make quality-gate` end-to-end; confirm every Success Criterion in `69-PROPOSALS.md` Proposal 3 is met

**Acceptance:**
- `make quality-gate` green
- `grep -rn '"0\.5\.0"' pmcp-macros/Cargo.toml Cargo.toml | grep pmcp-macros` returns zero matches
- All 5 proposal Success Criteria ticked

**Refinement vs. proposal:** the proposal puts "fuzz target" implicitly in Plan 3 as CI verification; I've moved it to Plan 2 because it's a new file that co-belongs with the trybuild-snapshot and README deliverable as "test-surface additions". Plan 3 becomes pure release-mechanics. This keeps Plans 2 and 3 balanced in size and keeps version-bump discipline isolated in its own plan (easier to revert if Plan 2 introduces a regression).

## Sources

### Primary (HIGH confidence)
- `pmcp-macros/src/mcp_tool.rs:33-85` — mandatory-description struct and hard-reject site
- `pmcp-macros/src/mcp_server.rs:577-604` — second parse site at `parse_mcp_tool_attr`
- `pmcp-macros/src/mcp_common.rs` — no existing doc-attr handling (clean slate for helper)
- `pmcp-macros/tests/ui/mcp_tool_missing_description.{rs,stderr}` — existing trybuild pattern
- `pmcp-macros/Cargo.toml:3` — version 0.5.0; `Cargo.toml:53,147` — root pins
- `examples/s23_mcp_tool_macro.rs:48-91` — 5 diverse call sites (3 with rustdoc coexistence)
- `pmcp-macros/tests/mcp_tool_tests.rs:25-180` — existing description-assertion tests
- `Makefile:460-479, 218-253, 227-238` — `quality-gate` + `test-doc` + `test-fuzz` wiring
- `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-PROPOSALS.md:118-166` — Proposal 3 scope, criteria, rationale
- `.planning/phases/69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor/69-RESEARCH.md:36` — MACRO-02 gap cell with rmcp code pointer
- `.planning/REQUIREMENTS.md:56,145` — PARITY-MACRO-01 landing and traceability

### Secondary (HIGH confidence — external)
- `rmcp-v1.5.0/crates/rmcp-macros/src/common.rs::extract_doc_line` — rmcp's verified normalization algorithm (trim + skip-empty + join-newline) fetched via raw.githubusercontent.com
- `rmcp-v1.5.0/crates/rmcp-macros/src/tool.rs` — confirms rmcp uses `fn_item.attrs.iter().try_fold(None, extract_doc_line)` to harvest description from attrs

### Tertiary (unverified in this session — flagged)
- darling's exact `NestedMeta` round-trip behavior with `syn::parse_str` — flagged as Open Question 4 for Plan 1 Wave 0 spike.
- Exact trybuild test target name inside `pmcp-macros` — flagged [ASSUMED] above; verify with `cargo test -p pmcp-macros --list` at plan time.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `TRYBUILD=overwrite cargo test` is the standard way to regenerate trybuild `.stderr` snapshots in this repo | Integration Points / trybuild | Low — if wrong, planner runs `cargo test` and manually edits `.stderr` to match output |
| A2 | darling's `NestedMeta` round-trips cleanly through `syn::parse_str::<syn::Meta>` when synthesizing `description = "..."` from a rustdoc string | Proposed Change / Integration sketch | Medium — fallback is to change `McpToolArgs::description: Option<String>` and resolve in handler body instead of synthesizing a NestedMeta (slightly more invasive diff). Spike in Plan 1 Wave 0 resolves this. |
| A3 | The existing `tests/ui/` trybuild harness compiles and fails as expected after just editing `.stderr` — no separate test-target wiring needed | Validation Architecture / Wave 0 | Low — `pmcp-macros/Cargo.toml:32` lists `trybuild = "1.0"` as dev-dep; existing `tests/ui/*.rs` files exist, so the harness is already active. |
| A4 | `cargo fuzz` is installed on the target development machine; if not, proptest-expanded surrogate covers the FUZZ requirement | ALWAYS Requirements Map / FUZZ | Low — CLAUDE.md explicitly sanctions the surrogate path via `Makefile:230-237`'s `-d fuzz` guard |
| A5 | The current pmcp version is 2.3.x (STATE.md hint) — exact patch level should be read from root Cargo.toml at plan time | Integration Points / Version-bump ripple | Low — Plan 3 reads the value directly; this research did not hard-code the number into any deliverable |

## Metadata

**Confidence breakdown:**
- Current-state mapping (two parse sites, error messages, versions): HIGH — all evidence direct from local source files
- Normalization algorithm: HIGH — rmcp reference verified via raw.githubusercontent.com fetch; test vectors derived deterministically
- Call-site blast radius: HIGH — 25 live sites enumerated by grep with exact file:line evidence
- Version-bump ripple: HIGH — negative evidence (zero matches in non-root Cargo.toml files) confirmed
- Integration sketch: MEDIUM — darling NestedMeta synthesis is standard proc-macro pattern but flagged A2 for Plan 1 Wave 0 spike
- ALWAYS map (fuzz specifically): MEDIUM — proc-macro fuzzing via `cargo fuzz` requires extracting pure logic; the approach is sound but depends on Plan 1's helper structure

**Research date:** 2026-04-17
**Valid until:** 2026-05-17 (30 days — stable domain, no fast-moving dependencies)

---

## Review Addendum (2026-04-17)

This section captures research updates surfaced by the Codex cross-AI plan review (`71-REVIEWS.md`). The original research in the sections above remains valid. These are refinements — particularly around one invalid execution assumption (A2 proc-macro visibility) and one scope-expanded audit (pmcp ripple).

### Proc-macro crate API visibility restriction (supersedes A2 partially)

The prior research Assumption A2 flagged the darling `NestedMeta` round-trip as the main risk. It missed a more fundamental restriction that Codex surfaced: **proc-macro crates cannot expose arbitrary public API beyond their `#[proc_macro]` items** (Rust Reference, https://doc.rust-lang.org/reference/procedural-macros.html).

This means the prior plan's approach — adding `#[doc(hidden)] pub mod __fuzz_support { pub use crate::mcp_common::extract_doc_description; }` inside `pmcp-macros/src/lib.rs`, gated by a `__fuzz` cargo feature — **will not compile**. Integration tests in `pmcp-macros/tests/*.rs` and external fuzz targets cannot import library items from a proc-macro crate, regardless of `#[cfg]` gates or feature flags.

**Resolution (HIGH-1 Option A):** A new non-proc-macro sibling crate `crates/pmcp-macros-support/` holds the pure `extract_doc_description` + `reference_normalize` helpers. `pmcp-macros` depends on it as a regular path dep. Property tests + fuzz targets consume the support crate directly. This follows the rmcp pattern.

**Alternatives rejected:**
- **Option B (intra-crate `#[cfg(test)]`):** Keeps helpers inside `pmcp-macros` as test-only, but sacrifices ALWAYS FUZZ since proc-macro crates cannot have `src/bin/` and cannot expose external fuzz entry points.
- **Option C (duplicate pure logic):** Keep helpers inside `pmcp-macros` for the macro expansion path AND duplicate them inside the support crate for tests/fuzz. Rejected because duplication increases drift risk and the round-trip equivalence test is harder to write than simply consuming one copy.

**Impact on task structure:** Plan 01 now owns the support crate scaffold + real implementation + unit tests + property tests; `pmcp-macros` edits move to Plan 02.

### Workspace `pmcp`-dep ripple audit (supersedes part of §"Integration Points / Version-bump ripple")

The prior research audited only `pmcp-macros =` pins and found zero non-root matches, which is correct. It did NOT audit `pmcp =` pins across downstream workspace crates. Codex correctly flagged this as a release risk.

**Explicit ripple audit (2026-04-17 tree state):**

```
$ grep -rn '^pmcp = \|pmcp = {' Cargo.toml cargo-pmcp/Cargo.toml crates/*/Cargo.toml fuzz/Cargo.toml
Cargo.toml:53: pmcp-macros = { version = "0.5.0", path = "pmcp-macros", optional = true }
Cargo.toml:74: pmcp-widget-utils = { path = "crates/pmcp-widget-utils", version = "0.1.0" }
Cargo.toml:147: pmcp-macros = { version = "0.5.0", path = "pmcp-macros" }
cargo-pmcp/Cargo.toml:38: pmcp = { version = "2.2.0", path = "..", features = ["streamable-http", "oauth"] }
crates/mcp-tester/Cargo.toml:21: pmcp = { version = "2.2.0", path = "../../", features = ["streamable-http", "oauth"] }
```

**Analysis:**
- `cargo-pmcp/Cargo.toml:38` — `pmcp = "2.2.0"` — caret-by-default (accepts all 2.x) — **accepts 2.4.0 without edit**.
- `crates/mcp-tester/Cargo.toml:21` — `pmcp = "2.2.0"` — same — **accepts 2.4.0 without edit**.
- `crates/mcp-preview/Cargo.toml` — no `pmcp` dep (depends only on `pmcp-widget-utils`).

No non-caret pmcp pins (`= 2.3.0`, `~2.3.0`) exist. The 2.3.0 → 2.4.0 bump therefore requires NO concurrent downstream bumps. This is different from the prior assumption that downstream bumps might be required.

**Impact on Plan 04:** Task 1 Step 1 runs the grep explicitly as evidence and records the output in the SUMMARY. If future tree state introduces a non-caret pin, the audit would catch it and force a concurrent bump.

### Shared resolver refactor (MEDIUM-1 rationale)

The prior plan called three helpers (`has_description_meta`, `extract_doc_description`, `build_description_meta`) at each parse site — a near-identical 5-line call sequence in both `mcp_tool.rs` and `mcp_server.rs::parse_mcp_tool_attr`. Codex observed this invites drift: a future refactor could change one site's logic without updating the other.

**Resolution:** Extract ONE shared function `resolve_tool_args(args_tokens, item_attrs, error_span_ident) -> syn::Result<Vec<NestedMeta>>` in `pmcp-macros/src/mcp_common.rs`. Both parse sites call it as a one-liner. The resolver encapsulates:
1. parsing `args_tokens` into `Vec<NestedMeta>`,
2. checking for `description = ...` presence,
3. synthesizing from rustdoc if absent,
4. emitting the canonical error if neither source supplied a description.

This reduces the two parse sites to functions of shape: "get tokens → call resolver → feed to `McpToolArgs::from_list`". Drift is structurally impossible because only one function defines the fallback semantics.

### Unsupported rustdoc forms (MEDIUM-3)

The prior research mentioned `#[doc = include_str!(...)]` only in passing. Codex asked for explicit documentation. The support boundary is:

| Form | Support | Behavior |
|------|---------|----------|
| `/// single-line` | ✅ supported | Harvested and trimmed |
| `/// multi-line\n/// second` | ✅ supported | Each line trimmed, empty lines dropped, joined with `\n` |
| `#[doc = "string literal"]` | ✅ supported | Same as `///` |
| `#[doc(hidden)]` | ✅ skipped | Meta::List shape, ignored by the `Meta::NameValue` guard |
| `#[doc(alias = "...")]` | ✅ skipped | Meta::List shape |
| `#[doc = include_str!("...")]` | ❌ not supported | Meta::NameValue with Expr::Macro (not Expr::Lit), silently skipped |
| `#[cfg_attr(cond, doc = "...")]` | ❌ not supported | Outer path is `cfg_attr`, not `doc` — silently skipped |
| Indented code fences inside `///` | ⚠️ lossy | Each line is trimmed, so indentation inside ` ``` ` blocks is lost. Acceptable because MCP clients render tool descriptions as plain text, not rustdoc HTML |
| `description = ""` (explicit empty) | ⚠️ present-and-winning | The empty string counts as a valid description meta; rustdoc fallback is NOT triggered; the tool's description is the empty string. Behavior is consistent with pre-Phase-71 (empty string was already accepted by darling) |

**Where documented:**
- Unit tests in `crates/pmcp-macros-support/src/lib.rs` cover `include_str!` and `cfg_attr` skip behavior.
- Unit tests in `pmcp-macros/src/mcp_common.rs::rustdoc_fallback_tests` cover `description = ""` semantics (two test cases: with and without rustdoc).
- README `pmcp-macros/README.md` "Limitations" subsection enumerates all four forms with workarounds.

**Rationale for `description = ""` choice:** Two options were considered:
- **(chosen)** Treat as present-and-winning. Consistent with pre-Phase-71 behavior; no new validation rule introduced in Phase 71; Codex's "either is defensible" note applies.
- **Rejected:** Fail-fast on explicit empty. Would require a new trybuild snapshot and would be a behavior change beyond Phase 71's scope (which is strictly additive).

### Semver posture decision (MEDIUM-4)

The prior plan bumped `pmcp` 2.3.0 → 2.3.1 (patch). Codex observed this may under-signal that `#[mcp_tool]` now accepts a newly-valid source form (rustdoc-only functions). Consumers whose code previously failed to compile will now compile — that is additive feature surface.

**Decision: bump pmcp 2.3.0 → 2.4.0 (minor).**

Rationale: Rust semver convention for additive features (new accepted syntax, new public APIs) is a minor bump. The downstream caret pins in the workspace accept 2.4.0 without any resolution change, so this is zero-cost to the workspace — it is purely a signal to external consumers.

### LOW-3: Fuzz target scope decision

With the support-crate approach (Plan 01), the fuzz target can consume `pmcp-macros-support` directly as a regular path dep — no feature gate, no `__fuzz_support` re-export. This makes it cheap to expand the fuzz target to mixed attribute shapes.

**Decision: Expand fuzz target.** `fuzz/fuzz_targets/rustdoc_normalize.rs` uses a selector (first byte of each newline-split chunk, modulo 4) to generate one of four attribute shapes: plain `#[doc = "..."]`, `#[doc(hidden)]`, `#[doc(alias = "foo")]`, or `#[allow(dead_code)]`. This exercises the `Meta::Path` and `Meta::List` skip paths in addition to the `Meta::NameValue` harvest path, which the prior narrow fuzz target did not cover.

### Revision summary

- **Plans:** 3 → 4 (added Plan 04 for release mechanics; Plan 01 became the support-crate scaffold; old Plan 01 logic moved to Plan 02)
- **Waves:** 3 → 4
- **Tasks:** 9 → 12
- **New workspace member:** `crates/pmcp-macros-support/`
- **Retired artifacts:** `__fuzz` cargo feature on `pmcp-macros`; `__fuzz_support` re-export in `pmcp-macros/src/lib.rs`
- **New artifacts:** non-empty-args trybuild fixture (MEDIUM-2); README Limitations subsection (MEDIUM-3); shared `resolve_tool_args` resolver (MEDIUM-1); explicit workspace ripple audit (HIGH-2)
- **Semver posture:** patch → minor (MEDIUM-4)

**Revision date:** 2026-04-17 (revision 2)
