# Phase 71: Rustdoc fallback for #[mcp_tool] tool descriptions - Pattern Map

> **⚠ REPLAN ADDENDUM (2026-04-17):** HIGH-1 (proc-macro crate API restriction) was resolved via Option A — new sibling crate `crates/pmcp-macros-support/` holds the pure normalization helper. The following sections are SUPERSEDED:
> - Any reference to `pub mod __fuzz_support` in `pmcp-macros/src/lib.rs` — no longer exists (pmcp-macros remains a pure proc-macro crate)
> - `#[cfg(any(feature = "__fuzz", fuzzing))]` gate — replaced by an unconditional `pub` on the support crate helper
> - `fuzz/Cargo.toml [dependencies.pmcp-macros] features = ["__fuzz"]` — replaced by `[dependencies.pmcp-macros-support] path = "../crates/pmcp-macros-support"`
> - New file `pmcp-macros/tests/properties.rs` — moved to `crates/pmcp-macros-support/tests/properties.rs`
>
> Authoritative spec: 71-01-PLAN.md through 71-04-PLAN.md.

**Mapped:** 2026-04-17
**Files analyzed:** 9 (5 source/test, 1 fuzz, 1 README, 2 Cargo.toml)
**Analogs found:** 9 / 9 (all intra-repo, none requires rmcp vendoring)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `pmcp-macros/src/mcp_tool.rs` (modify) | proc-macro attribute parser | compile-time transform | itself (lines 67-85) + `pmcp-macros/src/mcp_server.rs::parse_mcp_tool_attr` (symmetric site) | exact (self-pattern) |
| `pmcp-macros/src/mcp_server.rs` (modify, `parse_mcp_tool_attr` at lines 577-604) | proc-macro attribute parser | compile-time transform | itself + `mcp_tool.rs` (must stay byte-symmetric) | exact (self-pattern) |
| `pmcp-macros/src/mcp_common.rs` (extend — add 3 helpers) | shared codegen utility module | pure function | existing helpers in the same file (`classify_param`, `type_name_matches`, `extract_state_inner`) | exact (same file, add-only) |
| `pmcp-macros/tests/ui/mcp_tool_missing_description_and_rustdoc.rs` (new) | trybuild compile-fail fixture | compile-time assertion | `pmcp-macros/tests/ui/mcp_tool_missing_description.rs` (+ `.stderr`) | exact |
| `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` (regenerate) | trybuild snapshot | compile-time assertion | the existing `.stderr` file itself (only error wording changes) | exact |
| `pmcp-macros/tests/properties.rs` (new) | property-test harness | proptest loop | `tests/handler_extensions_properties.rs` (Phase 70; closest proptest discipline in repo) | role-match (cross-crate but idiomatic) |
| `fuzz/fuzz_targets/fuzz_rustdoc_normalize.rs` (new) + `fuzz/Cargo.toml` entry | libfuzzer fuzz target | request-response (bytes → result) | `fuzz/fuzz_targets/fuzz_peer_handle.rs` | exact |
| `pmcp-macros/README.md` (extend) | doc/migration section | doc render | existing `### Example` block for `#[mcp_tool]` (lines 54-90) + `### Attributes` list (lines 46-52) | exact (same file, add subsection) |
| `pmcp-macros/Cargo.toml` + root `Cargo.toml` (version bump) | manifest | build metadata | commit `74576da3` — prior 0.4.1 → 0.5.0 bump diff | exact (apply same diff shape) |

## Pattern Assignments

---

### `pmcp-macros/src/mcp_tool.rs` (modify) — attribute-parse site (standalone path)

**Analog:** itself, lines 67-85.

**Imports pattern** (lines 25-31):
```rust
use crate::mcp_common;
use darling::FromMeta;
use heck::ToUpperCamelCase;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{ItemFn, ReturnType, Type};
```
Note: `crate::mcp_common` is already imported — new helpers land there, zero new `use` lines needed.

**Hard-reject site — the exact code to replace** (lines 67-85):
```rust
pub fn expand_mcp_tool(args: TokenStream, input: &ItemFn) -> syn::Result<TokenStream> {
    use mcp_common::ParamSlot;

    // Parse macro attributes via darling.
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

**Replacement pattern** (from 71-RESEARCH.md lines 207-236; input attrs live on `&input.attrs`):
```rust
// Parse existing args into nested_metas (note: no early-return on empty).
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
        nested_metas.push(mcp_common::build_description_meta(&doc_desc));
    }
}

// NEW: if STILL no description, fail with updated error.
if !mcp_common::has_description_meta(&nested_metas) {
    return Err(syn::Error::new_spanned(
        &input.sig.ident,
        "mcp_tool requires either a `description = \"...\"` attribute or a rustdoc comment on the function",
    ));
}

let macro_args = McpToolArgs::from_list(&nested_metas)
    .map_err(|e| syn::Error::new_spanned(&input.sig.ident, e.to_string()))?;
```

**`McpToolArgs` struct — keep as-is** (lines 33-47):
```rust
#[derive(Debug, FromMeta)]
pub struct McpToolArgs {
    /// Tool description (mandatory per D-05).
    pub(crate) description: String,
    #[darling(default)]
    pub(crate) name: Option<String>,
    #[darling(default)]
    pub(crate) annotations: Option<McpToolAnnotations>,
    #[darling(default)]
    pub(crate) ui: Option<syn::Expr>,
}
```
`description: String` stays unchanged because the synthesis happens BEFORE `McpToolArgs::from_list`. This preserves the rest of the function's use of `macro_args.description` (line 93: `let description = &macro_args.description;`) with zero ripple.

---

### `pmcp-macros/src/mcp_server.rs` (modify) — attribute-parse site (impl-block path)

**Analog:** itself, lines 577-604; must stay byte-symmetric with `mcp_tool.rs` replacement above.

**Exact code to replace** (lines 577-604):
```rust
/// Parse `#[mcp_tool(...)]` attribute into `McpToolArgs`.
fn parse_mcp_tool_attr(attr: &syn::Attribute, method: &ImplItemFn) -> syn::Result<McpToolArgs> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::Path(_) => {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "mcp_tool requires at least `description = \"...\"` attribute",
            ));
        },
        syn::Meta::NameValue(_) => {
            return Err(syn::Error::new_spanned(
                attr,
                "mcp_tool requires parenthesized arguments: #[mcp_tool(description = \"...\")]",
            ));
        },
    };

    let parser =
        syn::punctuated::Punctuated::<darling::ast::NestedMeta, syn::Token![,]>::parse_terminated;
    let nested_metas = parser
        .parse2(tokens)
        .map(|p| p.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();

    McpToolArgs::from_list(&nested_metas)
        .map_err(|e| syn::Error::new_spanned(&method.sig.ident, e.to_string()))
}
```

**Replacement shape** — same three helper calls as `mcp_tool.rs`, with `method.attrs` replacing `input.attrs`:
1. `Meta::Path(_)` branch — do NOT early-return; instead set `tokens = TokenStream::new()`, fall through to rustdoc harvest from `&method.attrs`, and emit the NEW error string only if harvest returns `None`.
2. `Meta::NameValue(_)` branch — **keep the existing early-return** (its error is about syntax shape, not missing description — orthogonal to Phase 71).
3. After parsing `nested_metas`, run the same 3-step sequence: `has_description_meta` → `extract_doc_description(&method.attrs)` → synth or final-reject with the NEW error string.

**Invariant:** the NEW error string MUST be byte-identical across both sites. Add a unit test `tests/error_message_symmetry.rs` asserting `MCP_TOOL_MISSING_ERROR` constant equality (optional but cheap).

---

### `pmcp-macros/src/mcp_common.rs` (extend — add 3 helpers)

**Analog:** existing public helpers in the same file (lines 44-58, `classify_param`). Same module, same `pub(crate)` visibility idiom, same `//! ` module doc style.

**Existing helper pattern to mirror** (lines 44-58):
```rust
pub fn classify_param(param: &FnArg) -> syn::Result<ParamRole> {
    match param {
        FnArg::Receiver(_) => Ok(ParamRole::SelfRef),
        FnArg::Typed(pat_type) => {
            let ty = &*pat_type.ty;
            if type_name_matches(ty, "State") {
                let inner = extract_state_inner(ty)?;
                Ok(ParamRole::State { inner_ty: inner })
            } else if type_name_matches(ty, "RequestHandlerExtra") {
                Ok(ParamRole::Extra)
            } else {
                Ok(ParamRole::Args(ty.clone()))
            }
        },
    }
}
```

**Three new helpers to append** (visibility `pub(crate)`, rustdoc required per CLAUDE.md):

```rust
/// Harvest `#[doc = "..."]` attributes into a normalized description string.
///
/// Applies rmcp-parity normalization: trim each doc literal, drop empty
/// post-trim lines, join remaining lines with `"\n"`. Skips non-NameValue
/// doc attrs (e.g. `#[doc(hidden)]`, `#[doc(alias = "...")]`).
///
/// Returns `None` if no non-empty rustdoc is present.
pub(crate) fn extract_doc_description(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            continue;
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

/// True iff `nested_metas` already contains a `description = "..."` entry.
pub(crate) fn has_description_meta(metas: &[darling::ast::NestedMeta]) -> bool {
    metas.iter().any(|m| {
        matches!(
            m,
            darling::ast::NestedMeta::Meta(syn::Meta::NameValue(nv))
                if nv.path.is_ident("description")
        )
    })
}

/// Build a synthetic `description = "..."` nested-meta from a plain string,
/// avoiding string-formatting round-trips (safe for embedded quotes/backslashes).
pub(crate) fn build_description_meta(desc: &str) -> darling::ast::NestedMeta {
    let lit = syn::LitStr::new(desc, proc_macro2::Span::call_site());
    let meta: syn::Meta = syn::parse_quote! { description = #lit };
    darling::ast::NestedMeta::Meta(meta)
}
```

**Key design note (supersedes A2 in RESEARCH):** use `syn::parse_quote!` + `syn::LitStr::new` rather than `syn::parse_str::<syn::Meta>` + string escaping. This bypasses the darling round-trip concern flagged as Open Question 4 because `LitStr::new` handles arbitrary UTF-8 content safely.

**Complexity:** all three helpers are ≤15 cognitive complexity (CLAUDE.md gate: ≤25). No `.unwrap()`, no SATD.

---

### `pmcp-macros/tests/ui/mcp_tool_missing_description_and_rustdoc.rs` (new)

**Analog:** `pmcp-macros/tests/ui/mcp_tool_missing_description.rs` (full file follows).

**Pattern — copy verbatim, remove the `// Missing description...` comment, add a non-doc banner comment that makes the rustdoc-absence explicit:**
```rust
use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Args {
    x: i32,
}

// No rustdoc AND no description attribute -- should fail at compile time (PARITY-MACRO-01).
// This exercises the NEW error path where BOTH fallback sources are absent.
#[mcp_tool()]
async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

fn main() {}
```

**Expected `.stderr`** (generate via `TRYBUILD=overwrite cargo test -p pmcp-macros`; lock text to):
```
error: mcp_tool requires either a `description = "..."` attribute or a rustdoc comment on the function
  --> tests/ui/mcp_tool_missing_description_and_rustdoc.rs:13:10
   |
13 | async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
   |          ^^^^^^^^
```
(Line number will be determined at regenerate time; do not hand-author.)

**Register in harness** (`pmcp-macros/tests/mcp_tool_tests.rs:197-201`):
```rust
#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/mcp_tool_missing_description.rs");
    t.compile_fail("tests/ui/mcp_tool_multiple_args.rs");
    // NEW:
    t.compile_fail("tests/ui/mcp_tool_missing_description_and_rustdoc.rs");
}
```

---

### `pmcp-macros/tests/ui/mcp_tool_missing_description.stderr` (regenerate)

**Current text** (full file, 6 lines):
```
error: mcp_tool requires at least `description = "..."` attribute
  --> tests/ui/mcp_tool_missing_description.rs:12:10
   |
12 | async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
   |          ^^^^^^^^
```

**New text after regenerate** (error wording swap only; line numbers preserved because the `.rs` fixture does not change):
```
error: mcp_tool requires either a `description = "..."` attribute or a rustdoc comment on the function
  --> tests/ui/mcp_tool_missing_description.rs:12:10
   |
12 | async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
   |          ^^^^^^^^
```

**Procedure:** `TRYBUILD=overwrite cargo test -p pmcp-macros --features full compile_fail_tests` then hand-review the diff. Commit the single-line wording change.

---

### `pmcp-macros/tests/properties.rs` (new — proptest harness)

**Analog:** `tests/handler_extensions_properties.rs` (Phase 70 — closest proptest-in-test-file pattern in the repo; same 2026-04 cadence, same CLAUDE.md discipline applied).

**Top-of-file pattern to mirror** (lines 1-10):
```rust
//! Property-based tests for RequestHandlerExtra.extensions typemap.
//!
//! Covers: insert/get round-trip, key-collision returns old value, clone
//! preserves extensions, remove::<T>() round-trip, mixed-type coexistence.

use pmcp::RequestHandlerExtra;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]
```

**Phase 71 adaptation** — same shape, target `extract_doc_description` directly. `proptest = "1.6"` already in `pmcp-macros/Cargo.toml:36`, so no dep add:
```rust
//! Property-based tests for pmcp-macros rustdoc-harvest normalization (PARITY-MACRO-01).
//!
//! Covers: trim-join idempotence, empty-input → None, attribute-wins precedence,
//! no-panic on adversarial syn::Attribute inputs.

use proptest::prelude::*;
// NOTE: extract_doc_description is pub(crate) inside the proc-macro crate,
// so test access requires either (a) #[cfg(test)] pub re-export, or
// (b) duplicating the function under test. Recommended: add a
// `#[doc(hidden)] pub mod __test_support { pub use crate::mcp_common::*; }`
// gated on `cfg(test)` at the top of pmcp-macros/src/lib.rs, OR run these
// tests as an internal `#[cfg(test)] mod tests {}` block inside mcp_common.rs
// (the simpler path — matches the existing module-local test pattern).

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, .. ProptestConfig::default() })]

    // ≥4 invariants per RESEARCH lines 378-383:
    // 1. trim-join idempotence
    // 2. attribute wins over rustdoc (requires macro-expansion helper)
    // 3. neither present → error contains "mcp_tool requires either"
    // 4. no panic on any attr vector
    #[test]
    fn prop_normalize_idempotent(/* strategy: Vec<String> of simulated doc lines */) { /* ... */ }
}
```

**Proptest config note:** Phase 70 used `cases: 100`; RESEARCH.md:376 specifies `PROPTEST_CASES=1000` for Phase 71. Honor the research value — matches `Makefile:224` CI discipline.

**Visibility caveat (applies to Plan 1 Wave 0):** since `extract_doc_description` is `pub(crate)`, the simplest access pattern is a `#[cfg(test)] mod tests { ... }` **inside** `mcp_common.rs` — not a top-level `tests/properties.rs`. Planner's call; both options are acceptable. If top-level `tests/properties.rs` is kept, add `#[cfg(any(test, feature = "__test_support"))] pub use mcp_common::extract_doc_description as __test_extract_doc_description;` in `lib.rs`.

---

### `fuzz/fuzz_targets/fuzz_rustdoc_normalize.rs` (new)

**Analog:** `fuzz/fuzz_targets/fuzz_peer_handle.rs` (full file):

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::roots::ListRootsResult;
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
use serde_json::{from_slice, from_value, Value};

// Fuzz the serde boundary that `DispatchPeerHandle::sample` and
// `DispatchPeerHandle::list_roots` rely on when deserializing client
// responses via `serde_json::from_value`. The dispatcher returns an
// arbitrary `Value`; the peer impl must never panic on adversarial JSON —
// valid inputs round-trip, invalid inputs produce `Err`.
//
// Target surfaces:
// - `CreateMessageParams`  — outbound request (client may echo shape back)
// - `CreateMessageResult`  — sampling/createMessage response
// - `ListRootsResult`      — roots/list response
fuzz_target!(|data: &[u8]| {
    let Ok(json) = from_slice::<Value>(data) else {
        return;
    };
    let _ = from_value::<CreateMessageParams>(json.clone());
    let _ = from_value::<CreateMessageResult>(json.clone());
    let _ = from_value::<ListRootsResult>(json);
});
```

**Phase 71 adaptation** (RESEARCH.md:386-392 strategy: real helper, synthesize syn attrs via parse_str):
```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the rustdoc-harvest normalizer. The pure normalization function
// (trim each line → drop empties → join with "\n") must never panic on
// any `Vec<syn::Attribute>` input, including adversarial UTF-8 sequences
// and mixed doc/non-doc attrs. Valid inputs produce Some(non_empty_string)
// or None; invalid inputs produce None (no panic).
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    // Synthesize a sequence of #[doc = "..."] attrs from newline-delimited input.
    let mut attrs: Vec<syn::Attribute> = Vec::new();
    for line in s.split('\n') {
        // Escape embedded quotes and backslashes, then parse as a full attr.
        let escaped = line.replace('\\', "\\\\").replace('"', "\\\"");
        let src = format!("#[doc = \"{}\"]", escaped);
        if let Ok(attr) = syn::parse_str::<syn::Attribute>(&src) {
            attrs.push(attr);
        }
    }
    // Invariant: no panic on any input; output is either None or non-empty String.
    let out = pmcp_macros::__fuzz_support::extract_doc_description(&attrs);
    if let Some(s) = out.as_ref() {
        assert!(!s.is_empty());
    }
});
```

**Helper re-export required in `pmcp-macros/src/lib.rs`** (gated to keep the proc-macro surface clean):
```rust
#[doc(hidden)]
#[cfg(any(feature = "__fuzz", fuzzing))]
pub mod __fuzz_support {
    pub use crate::mcp_common::extract_doc_description;
}
```
Add `__fuzz = []` to `pmcp-macros/Cargo.toml` `[features]`. Fuzz target enables it via `fuzz/Cargo.toml`.

**`fuzz/Cargo.toml` entry pattern** (append after line 81, mirroring `fuzz_peer_handle` block):
```toml
[[bin]]
name = "fuzz_rustdoc_normalize"
path = "fuzz_targets/fuzz_rustdoc_normalize.rs"
test = false
doc = false
bench = false
```

And add the pmcp-macros path dep to `fuzz/Cargo.toml` `[dependencies]`:
```toml
[dependencies.pmcp-macros]
path = "../pmcp-macros"
features = ["__fuzz"]
```

---

### `pmcp-macros/README.md` (extend)

**Analog:** the existing `## #[mcp_tool]` section (lines 36-151), specifically the `### Attributes` list (46-52) and `### Example` block (54-90).

**Attributes list — modify line 47** (current):
```markdown
- `description = "..."` — **required**. Human-readable description exposed via the
  MCP `tools/list` response.
```
Replace with:
```markdown
- `description = "..."` — optional as of pmcp-macros 0.6.0. Human-readable
  description exposed via the MCP `tools/list` response. If omitted, the function's
  rustdoc comment is used instead (see "Rustdoc-derived descriptions" below).
  If both are present, the attribute wins.
```

**New subsection — insert AFTER the existing `### Example` block (after line 90), BEFORE `### Shared state with State<T>` (line 96).** Title: `### Rustdoc-derived descriptions (pmcp-macros 0.6.0+)`.

**Structural template to mirror** (from existing `### Example` block, lines 54-94):
````markdown
### Example

```rust,no_run
use pmcp::{mcp_tool, ServerBuilder, ServerCapabilities};
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

The macro expands the annotated function into a zero-arg constructor (`add()`) that
returns a generated `AddTool` struct implementing `ToolHandler`. Register it with
`ServerBuilder::tool(name, handler)`.
````

**New subsection to add** (required contents per RESEARCH.md:292-298: one-paragraph explanation, rust,no_run doctest, precedence sentence, normalization rule, version note):

````markdown
### Rustdoc-derived descriptions (pmcp-macros 0.6.0+)

When you omit the `description = "..."` attribute, pmcp-macros harvests the
function's rustdoc comment and uses it as the tool description. This eliminates
the duplication of writing the same prose in both a `///` block and the macro
attribute.

```rust,no_run
use pmcp::mcp_tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs { a: f64, b: f64 }

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult { sum: f64 }

/// Add two numbers and return their sum.
#[mcp_tool]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { sum: args.a + args.b })
}

# fn main() { let _ = add(); }
```

**Precedence:** when both a rustdoc comment and a `description = "..."` attribute
are present, the attribute wins. This is silent — no compiler warning — so that
rustdoc can be used freely for meta-commentary above tools that already specify
an explicit description.

**Normalization:** each rustdoc line is trimmed (leading/trailing whitespace
stripped); empty post-trim lines are dropped; remaining lines are joined with
`"\n"`. Fenced code blocks (`/// ```…```) preserve their fence markers but lose
indentation inside the block; this is acceptable because tool descriptions are
rendered as plain text in MCP client UIs, not as rustdoc HTML.

**Error when both are absent:** if a `#[mcp_tool]`-annotated function has no
rustdoc and no `description = "..."` attribute, compilation fails with:

```text
error: mcp_tool requires either a `description = "..."` attribute or a rustdoc comment on the function
```

**Requires:** pmcp-macros ≥ 0.6.0 (shipped with pmcp ≥ TBD — see CHANGELOG).
````

---

### `pmcp-macros/Cargo.toml` + root `Cargo.toml` (version bump)

**Analog:** commit `74576da3` ("chore(66): bump pmcp-macros 0.4.1 -> 0.5.0"). Exact diff shape to mirror:

**Edit 1** — `pmcp-macros/Cargo.toml:3`:
```toml
version = "0.5.0"
```
→
```toml
version = "0.6.0"
```

**Edit 2** — root `Cargo.toml:53` (optional prod dep):
```toml
pmcp-macros = { version = "0.5.0", path = "pmcp-macros", optional = true }
```
→
```toml
pmcp-macros = { version = "0.6.0", path = "pmcp-macros", optional = true }
```

**Edit 3** — root `Cargo.toml:147` (dev-dep for examples):
```toml
pmcp-macros = { version = "0.5.0", path = "pmcp-macros" }  # For macro examples (s23_mcp_tool_macro)
```
→
```toml
pmcp-macros = { version = "0.6.0", path = "pmcp-macros" }  # For macro examples (s23_mcp_tool_macro)
```

**No other workspace members need bumping** (RESEARCH.md:303-308 verified via grep):
- `crates/mcp-tester/Cargo.toml` — no pmcp-macros dep
- `crates/mcp-preview/Cargo.toml` — no pmcp-macros dep
- `cargo-pmcp/Cargo.toml` — no pmcp-macros dep
- `crates/pmcp-widget-utils/Cargo.toml` — no pmcp-macros dep
- `crates/pmcp-code-mode/Cargo.toml` — no pmcp-macros dep
- `crates/pmcp-code-mode-derive/Cargo.toml` — no pmcp-macros dep

**Open question for Plan 3** (not pattern-mapped here): whether to concurrent-bump pmcp root patch (e.g. 2.3.x → 2.3.x+1). RESEARCH.md:310 recommends YES for semver-signal discipline. If doing so, mirror the pattern from commit `74576da3` which bumped pmcp 2.2.0 → 2.3.0 alongside pmcp-macros 0.4.1 → 0.5.0 in one atomic commit — same edit (root `Cargo.toml` `[package] version = "..."` near top of file).

**Commit message template** (mirror `74576da3`):
```
chore(71): bump pmcp-macros 0.5.0 -> 0.6.0 (PARITY-MACRO-01)

- pmcp-macros/Cargo.toml: version 0.5.0 -> 0.6.0 (pre-1.0 minor bump;
  new additive capability: rustdoc fallback for #[mcp_tool] descriptions)
- Cargo.toml:53 (optional dep pin): pmcp-macros "0.5.0" -> "0.6.0"
- Cargo.toml:147 (dev-dep pin for examples): "0.5.0" -> "0.6.0"
```

---

## Shared Patterns

### `pub(crate)` visibility + rustdoc on every helper
**Source:** `pmcp-macros/src/mcp_common.rs:44` (`pub fn classify_param`) and entire module.
**Apply to:** all three new helpers in `mcp_common.rs`.
**Pattern:**
```rust
/// One-sentence summary ending in a period.
///
/// Multi-paragraph body explaining behavior, normalization rules,
/// or edge cases. Matches rustdoc-cov gate enforced by `make doc-check`.
pub(crate) fn helper_name(...) -> ReturnType { ... }
```

### Error-message symmetry between parse sites
**Source:** `pmcp-macros/src/mcp_tool.rs:74` and `pmcp-macros/src/mcp_server.rs:584` — currently both say `"mcp_tool requires at least \`description = \"...\"\` attribute"` verbatim.
**Apply to:** both sites must update to exact string `"mcp_tool requires either a \`description = \"...\"\` attribute or a rustdoc comment on the function"`. Consider hoisting to a `pub(crate) const MCP_TOOL_MISSING_DESCRIPTION_ERROR: &str` in `mcp_common.rs`.

### Proc-macro integration-test pattern
**Source:** `pmcp-macros/tests/mcp_tool_tests.rs:42-52` (test_echo_tool_metadata).
**Apply to:** Plan 1's new unit tests (rustdoc-only, attribute-wins, multiline, impl-block) must follow the same assert shape:
```rust
#[test]
fn test_rustdoc_only_description() {
    let tool = rustdoc_only_tool();
    let meta = tool.metadata().expect("metadata should exist");
    assert_eq!(meta.description.as_deref(), Some("Normalized rustdoc text"));
}
```

### trybuild-harness registration
**Source:** `pmcp-macros/tests/mcp_tool_tests.rs:196-201`.
**Apply to:** register the new `mcp_tool_missing_description_and_rustdoc.rs` fixture in the same `compile_fail_tests` function; do NOT create a separate test function or separate binary.

### No `.unwrap()` in production macro code
**Source:** CLAUDE.md `check-unwraps` gate + existing helpers in `mcp_common.rs` which use `.is_some_and(...)` and `syn::Result<_>` propagation.
**Apply to:** all three new helpers. `extract_doc_description` uses `let-else` for fallible matching (see sketch above); `build_description_meta` uses `syn::parse_quote!` which is infallible for a literal-only expansion.

## No Analog Found

None. All 9 touch points have a concrete intra-repo analog; no file requires vendoring from rmcp. The rmcp `extract_doc_line` helper is a *reference* (cited in RESEARCH.md:131,190) but the Phase 71 implementation is pmcp-native and framed in pmcp's existing `mcp_common.rs` idiom.

## Call-Site Sample for Backwards-Compat Proof (5 sites from examples/)

Per RESEARCH.md lines 340-352, these are the 5 example sites the planner must verify remain byte-identical after Phase 71 (attribute-wins precedence). All 5 live in `examples/s23_mcp_tool_macro.rs`:

| # | File:Line | Shape | Current rustdoc | Current `description` | Post-71 (attribute wins) |
|---|-----------|-------|-----------------|------------------------|---------------------------|
| 1 | `examples/s23_mcp_tool_macro.rs:48-49` | standalone `async fn` | `/// Minimal tool -- just args and return.` | `"Add two numbers"` | `"Add two numbers"` (unchanged) |
| 2 | `examples/s23_mcp_tool_macro.rs:56-57` | standalone `async fn` + `State<T>` | `` /// Tool with shared state via `State<T>`. `` | `"Greet with prefix from config"` | `"Greet with prefix from config"` (unchanged) |
| 3 | `examples/s23_mcp_tool_macro.rs:62-63` | standalone sync `fn` | `` /// Sync tool -- auto-detected from `fn` (not `async fn`). `` | `"Get server version"` | `"Get server version"` (unchanged) |
| 4 | `examples/s23_mcp_tool_macro.rs:78` (impl-block) | `async fn &self` inside `#[mcp_server] impl MathServer` | (no rustdoc) | `"Multiply two numbers"` | `"Multiply two numbers"` (unchanged) |
| 5 | `examples/s23_mcp_tool_macro.rs:85` (impl-block) | `async fn &self` inside `#[mcp_server] impl MathServer` | (no rustdoc) | `"Health check"` | `"Health check"` (unchanged) |

**Concrete lift excerpt (sites 1-3, most regression-risky):**
```rust
/// Minimal tool -- just args and return.
#[mcp_tool(description = "Add two numbers")]
async fn add(args: AddArgs) -> pmcp::Result<AddResult> {
    Ok(AddResult { result: args.a + args.b })
}

/// Tool with shared state via `State<T>`.
#[mcp_tool(description = "Greet with prefix from config")]
async fn greet(args: GreetArgs, config: State<AppConfig>) -> pmcp::Result<Value> {
    Ok(json!({ "greeting": format!("{}, {}!", config.greeting_prefix, args.name) }))
}

/// Sync tool -- auto-detected from `fn` (not `async fn`).
#[mcp_tool(description = "Get server version", annotations(read_only = true))]
fn version() -> pmcp::Result<Value> {
    Ok(json!({ "version": env!("CARGO_PKG_VERSION") }))
}
```

**Sixth site in `examples/s24_mcp_prompt_macro.rs:112`** (`#[mcp_tool(description = "Add two numbers")]` inside `impl DevServer`) — no rustdoc, attribute-only; trivially unchanged. Include in `cargo check --workspace --examples --features full` pass but no special handling needed.

**Regression-detection test pattern** (planner must add, per RESEARCH.md line 352):
```rust
// Asserts attribute-wins precedence at byte level
#[mcp_tool(description = "WINS")]
/// IGNORED (rustdoc)
async fn precedence_test(args: EchoArgs) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

#[test]
fn test_attribute_wins_over_rustdoc() {
    let tool = precedence_test();
    let meta = tool.metadata().expect("metadata should exist");
    assert_eq!(meta.description.as_deref(), Some("WINS"));
}
```

## Metadata

**Analog search scope:** `pmcp-macros/src/`, `pmcp-macros/tests/`, `fuzz/fuzz_targets/`, `tests/` (repo root), `examples/`, repo-root `Cargo.toml` and `pmcp-macros/Cargo.toml`, `pmcp-macros/README.md`, git log on `pmcp-macros/Cargo.toml`.

**Files scanned:** 13 source/test files + 2 manifests + 1 README + 1 git-log analysis.

**Pattern extraction date:** 2026-04-17

**Confidence:** HIGH — every pattern has a byte-exact excerpt from a real file in this repo. The two pattern-level residual uncertainties (darling `NestedMeta` synthesis via `parse_quote!`, and `pub(crate)` visibility route for proptest) are each addressed with a recommended primary path AND a fallback path in the helper section above.
