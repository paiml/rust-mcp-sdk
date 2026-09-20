---
phase: 67-docs-rs-pipeline-and-feature-flags
plan: 02
type: execute
wave: 1
depends_on: []
files_modified:
  - src/lib.rs
  - src/types/mod.rs
  - src/server/mod.rs
autonomous: true
requirements:
  - DRSD-01
tags:
  - rust
  - rustdoc
  - docs-rs
must_haves:
  truths:
    - "Zero manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations remain in src/"
    - "`#![cfg_attr(docsrs, feature(doc_cfg))]` at src/lib.rs:70 is unchanged (NOT flipped to doc_auto_cfg — that feature name was removed in Rust 1.92.0)"
    - "Crate still compiles on stable and (when docsrs cfg is set) on nightly"
    - "All 145 `#[cfg(feature = ...)]`-gated items are ready to receive automatic badges from rustdoc's post-RFC-3631 auto-cfg mode"
  artifacts:
    - path: "src/lib.rs"
      provides: "Crate root with unchanged feature(doc_cfg) line and 2 deleted manual annotations"
      contains: "feature(doc_cfg)"
    - path: "src/types/mod.rs"
      provides: "types module with 1 deleted manual annotation"
    - path: "src/server/mod.rs"
      provides: "server module with 3 deleted manual annotations"
  key_links:
    - from: "src/lib.rs:70"
      to: "rustdoc auto-cfg behavior (RFC 3631)"
      via: "#![cfg_attr(docsrs, feature(doc_cfg))]"
      pattern: "feature\\(doc_cfg\\)"
---

<objective>
Delete all 6 manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations across `src/lib.rs` (2), `src/types/mod.rs` (1), and `src/server/mod.rs` (3). These become redundant once RFC 3631's auto-cfg behavior takes effect — rustdoc now automatically renders feature badges on every `#[cfg(feature = "...")]`-gated item when `#![cfg_attr(docsrs, feature(doc_cfg))]` is set at crate root (which it already is at `src/lib.rs:70` — unchanged).

Purpose: Eliminate the manual-annotation drift risk. The current state is 6 annotations for 145 feature-gated items (~4% coverage) — a maintenance trap where future contributors won't know whether new items need manual annotation. Switching to the single auto-cfg mechanism is D-02 from CONTEXT.md.

CRITICAL: Do NOT touch line 70 (`#![cfg_attr(docsrs, feature(doc_cfg))]`). The original D-01 intent (flip `doc_cfg` → `doc_auto_cfg`) is INVALIDATED by upstream — Rust 1.92.0 hard-removed `feature(doc_auto_cfg)` via PR rust-lang/rust#138907. Any edit that types `feature(doc_auto_cfg)` will fail with `error[E0557]: feature has been removed`. The existing line already enables auto-cfg behavior post-RFC-3631.

Output: 6 lines deleted across 3 files. `cargo check` still passes. `feature(doc_cfg)` at `src/lib.rs:70` remains exactly as-is.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md
@.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md
@src/lib.rs
@src/types/mod.rs
@src/server/mod.rs

<interfaces>
<!-- Current line-exact locations of the 6 manual annotations to delete. -->
<!-- Verified by `rg '#\[cfg_attr\(docsrs, doc\(cfg' src/` on 2026-04-11. -->

1. `src/lib.rs:86` — `#[cfg_attr(docsrs, doc(cfg(feature = "composition")))]` (above `pub mod composition;`)
2. `src/lib.rs:105` — `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` (above `pub mod axum {` block)
3. `src/types/mod.rs:25` — `#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]` (above `pub mod mcp_apps;`)
4. `src/server/mod.rs:107` — `#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]` (above `pub mod mcp_apps;`)
5. `src/server/mod.rs:139` — `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` (above `pub mod axum_router;`)
6. `src/server/mod.rs:157` — `#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]` (above `pub mod tower_layers;`)

After deletion, each `#[cfg(feature = "...")]` line immediately preceding the affected `pub mod` declaration stays in place — the cfg gate itself is not touched, only the doc-cfg annotation line is removed.

Current `src/lib.rs:63-77` (the lint block that must be preserved intact — D-10):
```rust
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Allow certain clippy lints that are too pedantic for this codebase
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::multiple_crate_versions)]
// _meta is a protocol field name mandated by the MCP spec; suppress underscore lint
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::result_large_err)]
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Delete 6 manual doc(cfg(...)) annotations across lib.rs, types/mod.rs, server/mod.rs</name>
  <files>src/lib.rs, src/types/mod.rs, src/server/mod.rs</files>
  <read_first>
    - src/lib.rs (full — confirm current line 70 says `#![cfg_attr(docsrs, feature(doc_cfg))]` and DO NOT edit it; confirm annotations at lines 86 and 105)
    - src/types/mod.rs (first 35 lines — confirm annotation at line 25)
    - src/server/mod.rs (lines 95–165 — confirm annotations at lines 107, 139, 157)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-CONTEXT.md (D-01 amendment: line 70 is NOT edited; D-02: delete the 6 annotations; D-03: do NOT add `doc_cfg_hide`; D-10: preserve lines 63–77)
    - .planning/phases/67-docs-rs-pipeline-and-feature-flags/67-RESEARCH.md (Upstream Dependency Changes section for the full E0557 rationale; Code Examples → Example 1 for the expected post-phase lib.rs top)
  </read_first>
  <action>
Delete exactly 6 lines across 3 files. Each deletion is a single-line removal — the `#[cfg(feature = "...")]` line immediately above each `pub mod` remains intact; only the `#[cfg_attr(docsrs, doc(cfg(...)))]` line is removed.

**File 1: `src/lib.rs`**

Delete line 86 (exact current text):
```rust
#[cfg_attr(docsrs, doc(cfg(feature = "composition")))]
```
After deletion, the `pub mod composition;` block reads:
```rust
#[cfg(feature = "composition")]
pub mod composition;
```

Delete line 105 (exact current text):
```rust
#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]
```
After deletion, the `pub mod axum { ... }` block reads:
```rust
#[cfg(feature = "streamable-http")]
pub mod axum {
    pub use crate::server::axum_router::{
        router, router_with_config, AllowedOrigins, RouterConfig,
    };
}
```

**DO NOT TOUCH `src/lib.rs:70`** — that line currently says:
```rust
#![cfg_attr(docsrs, feature(doc_cfg))]
```
and it must stay exactly like that. Per RESEARCH.md Upstream Dependency Changes: Rust 1.92.0 (Sept 2025, PR rust-lang/rust#138907) removed the `doc_auto_cfg` feature name. If you change this line to `feature(doc_auto_cfg)`, the nightly build will fail with `error[E0557]: feature has been removed`. Per CONTEXT.md D-01 amendment: the existing line produces RFC-3631 auto-cfg behavior without any source edit.

**DO NOT TOUCH `src/lib.rs:63-77`** — the lint block (`#![warn(...)]`, `#![deny(unsafe_code)]`, clippy allows). Per D-10 these are independent of the module doc and must remain intact.

**File 2: `src/types/mod.rs`**

Delete line 25 (exact current text):
```rust
#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]
```
After deletion, the `pub mod mcp_apps;` block reads:
```rust
/// MCP Apps Extension types for interactive UI support (ChatGPT Apps, MCP-UI)
#[cfg(feature = "mcp-apps")]
pub mod mcp_apps;
```

**File 3: `src/server/mod.rs`**

Delete line 107:
```rust
#[cfg_attr(docsrs, doc(cfg(feature = "mcp-apps")))]
```
Surrounding context becomes:
```rust
/// MCP Apps Extension - Interactive UI support for multiple MCP hosts.
///
/// Provides adapters for `ChatGPT` Apps, MCP Apps (SEP-1865), and MCP-UI.
#[cfg(all(not(target_arch = "wasm32"), feature = "mcp-apps"))]
pub mod mcp_apps;
```

Delete line 139:
```rust
#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]
```
Surrounding context becomes:
```rust
/// Axum Router convenience function for secure MCP server hosting.
#[cfg(feature = "streamable-http")]
pub mod axum_router;
```

Delete line 157:
```rust
#[cfg_attr(docsrs, doc(cfg(feature = "streamable-http")))]
```
Surrounding context becomes:
```rust
/// Tower middleware layers for MCP HTTP security (DNS rebinding, security headers).
#[cfg(feature = "streamable-http")]
pub mod tower_layers;
```

**Do NOT add `doc_cfg_hide`** (D-03) — the default auto-cfg behavior already hides badges for `test`, `feature = "default"`, etc. correctly.

**Do NOT touch pmcp-macros** (D-29) — Phase 66 just shipped pmcp-macros 0.5.0 with its own clean docs story.

After all deletions, run:
```
cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
```
to confirm the crate still compiles.

**Note on line numbers:** The numbers above (86, 105, 25, 107, 139, 157) match the file state at plan creation time. If an edit to another file has shifted them (unlikely in Wave 1 parallel execution since plans 01 and 03 touch different files), use grep to locate each annotation — the content itself is unique and unambiguous.
  </action>
  <verify>
    <automated>[ "$(rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ -c | awk -F: '{s+=$2} END {print s+0}')" = "0" ] && grep -q '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs && ! grep -q 'doc_auto_cfg' src/lib.rs && cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket</automated>
  </verify>
  <acceptance_criteria>
    - `rg '#\[cfg_attr\(docsrs, doc\(cfg' src/ -c` returns nothing (count = 0)
    - `grep -c '#\[cfg_attr(docsrs, doc(cfg' src/lib.rs` returns `0`
    - `grep -c '#\[cfg_attr(docsrs, doc(cfg' src/types/mod.rs` returns `0`
    - `grep -c '#\[cfg_attr(docsrs, doc(cfg' src/server/mod.rs` returns `0`
    - `grep -c '^#!\[cfg_attr(docsrs, feature(doc_cfg))\]$' src/lib.rs` returns exactly `1` (line unchanged)
    - `grep -c 'doc_auto_cfg' src/lib.rs` returns `0` (feature name REMOVED in 1.92.0 — never write it)
    - `grep -c '#!\[deny(unsafe_code)\]' src/lib.rs` returns `1` (D-10 lint block preserved)
    - `grep -c '#!\[warn(' src/lib.rs` returns `1` (D-10)
    - `grep -c 'pub mod composition;' src/lib.rs` returns `1` (module declaration intact)
    - `grep -c 'pub mod mcp_apps;' src/types/mod.rs` returns `1`
    - `grep -c 'pub mod axum_router;' src/server/mod.rs` returns `1`
    - `grep -c 'pub mod tower_layers;' src/server/mod.rs` returns `1`
    - `cargo check --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` exits 0
    - No edits to `pmcp-macros/**`
  </acceptance_criteria>
  <done>
Zero manual `doc(cfg(...))` annotations remain in `src/`. `feature(doc_cfg)` at `src/lib.rs:70` unchanged. Crate compiles on stable. Lint block at `src/lib.rs:63-77` intact.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

None introduced — this plan deletes source annotations and does not touch runtime code, authentication, or data handling.

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-67-02-01 | Tampering | `src/lib.rs:70` (`feature(doc_cfg)` line) | mitigate | Task action and acceptance_criteria explicitly forbid editing line 70 or typing `doc_auto_cfg` anywhere. Verified by `grep -c 'doc_auto_cfg' src/lib.rs` returning 0. |
| T-67-02-02 | Denial of service | Build integrity under stable toolchain | mitigate | Post-deletion `cargo check --features <D-16 list>` command verifies the crate still builds. If it doesn't, the plan fails loudly (not silently). |

No runtime attack surface. Plan only deletes source-level doc annotations.
</threat_model>

<verification>
Single-task plan with strict pre/post grep invariants. No dependency on Plans 01 or 03 (this plan touches src/, they touch Cargo.toml and CRATE-README.md+src/lib.rs module doc respectively — Plan 03 edits the top of src/lib.rs, so check that Plan 03's include_str! flip lands before AND after this deletion without conflict). The file overlap on `src/lib.rs` with Plan 03 means that if both run in Wave 1 in parallel, one must follow the other sequentially via git. The wave scheduler enforces this because files_modified overlap → sequential waves. See Plan 03's dependency resolution.

**Note to wave scheduler:** This plan modifies `src/lib.rs`; Plan 03 also modifies `src/lib.rs`. Both cannot run in the same wave — Plan 03 is bumped to Wave 2 below. (Corrected at plan-build time: see Plan 03 wave field = 2.)
</verification>

<success_criteria>
- 6 manual `doc(cfg(...))` annotations removed
- `#![cfg_attr(docsrs, feature(doc_cfg))]` at `src/lib.rs:70` unchanged (grep confirms)
- No `doc_auto_cfg` string anywhere in `src/`
- `cargo check --features <D-16 list>` exits 0
- Lint block at `src/lib.rs:63-77` preserved intact (D-10)
- No edits to `pmcp-macros/` (D-29)
</success_criteria>

<output>
After completion, create `.planning/phases/67-docs-rs-pipeline-and-feature-flags/67-02-SUMMARY.md` with:
- The 6 deleted lines (file:line_before → file:line_after pairs)
- Confirmation that `cargo check --features <D-16 list>` compiles
- Confirmation that `src/lib.rs:70` still contains `#![cfg_attr(docsrs, feature(doc_cfg))]` verbatim
</output>
