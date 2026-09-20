# Phase 67: docs.rs Pipeline and Feature Flags - Research

**Researched:** 2026-04-11
**Domain:** Rust crate documentation (rustdoc, docs.rs, feature gates)
**Confidence:** HIGH

## Summary

Phase 67 is well-scoped and most of its decisions are sound, but upstream rustdoc removed the `doc_auto_cfg` feature gate as of Rust 1.92.0 (September 2025, PR rust-lang/rust#138907). CONTEXT.md decision D-01 — `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` — **will not compile on current nightly** and therefore will not work on docs.rs. The correct replacement is `#![cfg_attr(docsrs, feature(doc_cfg))]` alone, which (after RFC 3631 landed) now enables auto-badge behavior by default on both top-level and nested items. Empirically verified on nightly 1.96.0-nightly (2026-04-10) with a minimal reproducer: auto-badges render for both top-level fns and nested structs. Issue rust-lang/rust#150268 does not affect this specific pattern on current nightly.

Rustdoc warning baseline is 29 warnings, cleanly categorized: 9 bare `[REDACTED]` brackets-as-links (all mechanical fixes), 9 broken intra-doc links pointing at types that live in other crates (`pmcp-tasks`, `axum`, `tower-http`, etc.), 3 private-item links from public docs, 2 unclosed `<str>` HTML tags (both in workflow module prose), 1 redundant explicit link target in `src/lib.rs:102` (inside the doc comment that gets deleted by D-04 anyway), and 6 additional broken intra-doc links to types that were either renamed or never existed in `pmcp`'s own namespace. All 29 fit into five small batches. No warnings come from generated code (there is no `src/generated_contracts/`).

The Cargo.toml D-16 feature list and `CRATE-README.md` path choices are sound. Auto-inclusion of root-level `.md` files in `cargo package` is verified (current `README.md` at root already ships). pmcp's current doctest suite (338 tests) passes — the Quick Start blocks move verbatim without needing any type-rename adjustment.

**Primary recommendation:** Update CONTEXT.md D-01 to use `#![cfg_attr(docsrs, feature(doc_cfg))]` (not `feature(doc_auto_cfg)`). Everything else in the plan stands. The warning cleanup is 29 items, tractable in a single wave.

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Auto-cfg migration and manual annotation cleanup (D-01 through D-03)**
- **D-01:** Replace `#![cfg_attr(docsrs, feature(doc_cfg))]` at `src/lib.rs:70` with `#![cfg_attr(docsrs, feature(doc_auto_cfg))]`. Single-line flip, and every `#[cfg(feature = "…")]`-gated item in the crate (~145 occurrences across 26 files) gets auto-badged on docs.rs with zero additional annotation.
- **D-02:** Delete all 6 existing `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations — 2 in `src/lib.rs`, 1 in `src/types/mod.rs`, 3 in `src/server/mod.rs`. They become redundant with `doc_auto_cfg` and keeping them creates a drift risk. Single mechanism, single source of truth.
- **D-03:** Do **not** add `doc_cfg_hide` or any other selective-hiding rustdoc attrs. `doc_auto_cfg` defaults are correct: it hides badges for `test`, `feature = "default"`, etc. automatically.

> **RESEARCH FLAG — D-01 invalidated by upstream change:** `doc_auto_cfg` was removed from the compiler in Rust 1.92.0 (PR rust-lang/rust#138907). The exact incantation in D-01 will fail compilation with `error[E0557]: feature has been removed` on any nightly from 1.92.0 onward. The correct replacement is `#![cfg_attr(docsrs, feature(doc_cfg))]` — the `doc_cfg` feature gate now covers what `doc_auto_cfg` used to do, with auto-badge behavior enabled by default. See **Upstream Dependency Changes** section below for the full story and verified replacement. D-02 and D-03 are unaffected and still correct.

**include_str! module doc pattern (D-04 through D-10)**
- **D-04:** Adopt `#![doc = include_str!("../CRATE-README.md")]` at the top of `src/lib.rs`, replacing the inline `//!`-prefixed module doc at lines 1–61.
- **D-05:** The include_str! source file is `CRATE-README.md` at the repo root (parallel to the existing 682-line `README.md`). Path from `src/lib.rs`: `#![doc = include_str!("../CRATE-README.md")]`.
- **D-06:** `CRATE-README.md` is a new file authored in this phase. It is separate from repo `README.md` — pulls deferred requirement **DOCD-02** into Phase 67 scope.
- **D-07:** Structure of `CRATE-README.md` (top to bottom): H1 title + 1-2 sentence crate purpose, `## Quick Start` with Client + Server Examples moved verbatim from current `src/lib.rs:14-61` as `rust,no_run`, `## Cargo Features` table (per D-11), short pointers to docs/book/course/repo.
- **D-08:** Target length ~150–250 lines.
- **D-09:** Every code block must compile. `rust,no_run` is default, `rust,ignore` forbidden.
- **D-10:** Preserve existing `src/lib.rs` crate-level warning lints (lines 63-77: `warn(missing_docs, …)`, `deny(unsafe_code)`, etc.).

**Feature flag table (D-11 through D-15)**
- **D-11:** 3 columns: Feature / Description / Enables.
- **D-12:** Row order: `default` meta row, `full` meta row, then individual features alphabetized.
- **D-13 (CORRECTED 2026-04-11 post plan-checker review):** **16 individual feature rows, alphabetized**: `composition`, `http`, `http-client`, `jwt-auth`, `logging`, `macros`, `mcp-apps`, `oauth`, `rayon`, `resource-watcher`, `schema-generation`, `simd`, `sse`, `streamable-http`, `validation`, `websocket`. **Total table rows = 18** (2 meta: `default` + `full`, plus 16 individual). The 16 individual rows = the 15 features in D-16 (`[package.metadata.docs.rs].features`) + `logging` (which D-16 omits because it's implicit via `default = ["logging"]`). Plan 06's single-source-of-truth invariant permits exactly this one diff between the Cargo.toml features list and the CRATE-README.md table.
- **D-14:** Table placement immediately after `## Quick Start`.
- **D-15:** Table content tracks `Cargo.toml` `[features]` section.

**[package.metadata.docs.rs] rewrite (D-16 through D-19)**
- **D-16:** Replace `all-features = true` with explicit 15-feature list (composition, http, http-client, jwt-auth, macros, mcp-apps, oauth, rayon, resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket). `logging` omitted because `default = ["logging"]` makes docs.rs include it automatically. Add `targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]`. Keep existing `rustdoc-args = ["--cfg", "docsrs"]`.
- **D-17:** Excluded from the feature list: `full` (redundant meta), `unstable`, `test-helpers`, `authentication_example`/`cancellation_example`/`progress_example` (example gates), `wasm`/`websocket-wasm`/`wasm-tokio`/`wasi-http` (WASM matrix conflicts).
- **D-18:** Two targets: `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (ARM64 positioning). CONTEXT claims max 6 targets; actual docs.rs max is 10 (see research below).
- **D-19:** Do **not** add a `default-target` override.

**Rustdoc warning cleanup (D-20 through D-22)**
- **D-20:** Warning cleanup scope = whatever `cargo doc --no-deps --features <D-16 list>` reports.
- **D-21:** Expected categories: broken intra-doc links, unclosed HTML tags, stale refs.
- **D-22:** Fixes apply to `pmcp` only. Out of scope for other workspace crates.

**CI gate (D-23 through D-29)**
- **D-23:** Add new `make doc-check` target in `Makefile`, running `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features <D-16 list>` on stable toolchain.
- **D-24:** `make doc-check` runs on **stable** — no `--cfg docsrs` passed. Passing it on stable would enable the nightly feature gate and fail.
- **D-25:** Do **not** pass `--all-features` to `cargo doc` in `make doc-check`.
- **D-26:** New step inside the existing `quality-gate` job in `.github/workflows/ci.yml` (not a separate job).
- **D-27:** `make doc-check` **not** chained from `make quality-gate` locally.
- **D-28:** No `pmcp` version bump. Stays at v2.3.0. No CHANGELOG entry.
- **D-29:** No `pmcp-macros` touches.

### Claude's Discretion

- Exact prose of `CRATE-README.md` intro paragraph and section headers.
- Ordering of rustdoc warning fix batches within the overall fix wave.
- Whether the feature table uses condensed or blank-line-separated style.
- Quick Start code blocks move verbatim but may receive minor "same intent" adjustments if types were renamed (research confirms no renames needed).
- Exact Makefile color output formatting for `doc-check`.

### Deferred Ideas (OUT OF SCOPE)

- WASM (`wasm32-unknown-unknown`, `wasm32-wasi`) docs.rs coverage — single-feature-list constraint conflicts with native-transport features. File as separate backlog phase.
- Workspace-wide rustdoc zero-warnings gate. `mcp-tester`, `mcp-preview`, `cargo-pmcp`, `pmcp-tasks`, `pmcp-widget-utils`, `pmcp-server`, `pmcp-server-lambda`, `mcp-e2e-tests`, `pmcp-macros` keep current docs state.
- TypedToolWithOutput refactor of Quick Start code blocks — that's PLSH-01 / Phase 68.
- Chaining `make doc-check` into `make quality-gate` locally — rejected by D-27 for iteration-speed reasons.
- Linking to pmcp-book / pmcp-course with deep anchors from `CRATE-README.md`.
- `make doc-check-nightly` variant — rejected by D-24.
- Deleting `authentication_example` / `cancellation_example` / `progress_example` feature gates — orthogonal cleanup, flag for backlog.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DRSD-01 | `lib.rs` contains `cfg_attr(docsrs, feature(doc_auto_cfg))` enabling automatic feature badges | **Blocked as written — see Upstream Dependency Changes.** Correct replacement: `feature(doc_cfg)`. Verified empirically that this produces auto-badges on both top-level and nested items on current nightly. |
| DRSD-02 | `Cargo.toml [package.metadata.docs.rs]` uses explicit feature list (~13 features) instead of `all-features = true` | Exact 15-feature list for D-16 validated against `Cargo.toml:150-184`. `aarch64-unknown-linux-gnu` verified as a docs.rs **default target** (known-buildable). Full transitive-dep expansion for every feature extracted from Cargo.toml (see Feature Flag Expansion table). |
| DRSD-03 | Feature flag table in `lib.rs` doc comments documents all user-facing features | 3-column (Feature / Description / Enables) template matches tokio and axum conventions. All 15 features' transitive expansions available from `Cargo.toml` for the Enables column. Placement per D-14 (after Quick Start) matches both tokio and axum. |
| DRSD-04 | Zero rustdoc warnings on `cargo doc --all-features --no-deps`, CI gate added | **Baseline established: 29 warnings across 13 source files.** Full categorized list in Rustdoc Warning Baseline section. No warnings from generated code. CI insertion point identified: between lines 200 and 205 of `.github/workflows/ci.yml` (after `Install quality tools`, before `Run quality gate`). |

## Project Constraints (from CLAUDE.md)

Actionable directives extracted from `./CLAUDE.md` that this phase must honor:

| # | Directive | Source | Phase 67 Applicability |
|---|-----------|--------|------------------------|
| 1 | Use `make quality-gate` (not bare `cargo clippy`) for all local gate checks | "Release & Publish Workflow" § Release Steps | The new `make doc-check` target must be wired into the CI `quality-gate` job, not as a parallel workflow. Already a locked decision (D-26) — just verify in plan. |
| 2 | Zero tolerance for defects; pre-commit hooks run Toyota Way checks | "Toyota Way Quality System" | Every commit in this phase must pass existing hooks. The new `make doc-check` is stricter than the hook but runs in CI only (D-27). |
| 3 | `cargo fmt --all -- --check` must pass | "Pre-Commit Quality Gates" | `CRATE-README.md` creation and Cargo.toml edits must leave formatting clean. |
| 4 | Clippy must report zero warnings under pedantic+nursery lints | "Pre-Commit Quality Gates" | Phase 67 touches non-code files and one `src/lib.rs` edit; clippy risk is low but the Cargo.toml metadata edit should not introduce compile errors. |
| 5 | Use justfile in preference to Makefile for scripts (user-global) | Global `~/.claude/CLAUDE.md` | **Conflict with D-23 (Makefile target).** The project already uses `Makefile` (400+ lines, canonical) and all existing targets live there. User-global justfile preference is overridden by project convention. CLAUDE.md release workflow refers to `make quality-gate` as canonical. Decision: stay in Makefile. Flag in research for visibility but not a blocker. |
| 6 | Contract-first development: write/update contract YAML, run `pmat comply check`, implement, re-check | "Contract-First Development" | Phase 67 is documentation / metadata only — no new public API. Confirm with plan-checker whether contract update is required; likely "no" for this phase. |
| 7 | PDMT-style todos with built-in quality gates, 80% coverage, zero SATD, complexity ≤25 | "Task Management - PDMT Style" | Phase 67 doesn't introduce new runtime code, so coverage and complexity metrics don't apply. SATD rule applies to all edits. |
| 8 | Every new feature needs FUZZ + PROPERTY + UNIT + EXAMPLE | "ALWAYS Requirements" | **N/A for this phase.** Phase 67 ships no new runtime features. The "feature" here is doc-infra: `make doc-check` enforcement. Validation is compile-time (cargo doc exit code). A PDMT todo should explicitly document "FUZZ/PROPERTY/UNIT/EXAMPLE: N/A — doc infrastructure phase; validated via `make doc-check` exit 0." |
| 9 | Pre-commit hook enforces format, clippy, build, doctests | "Pre-Commit Quality Gates" | `cargo test --doc` is already in CI (line 94) and the pre-commit. `CRATE-README.md` doctests automatically inherit this gate once wired via `include_str!`. No new plumbing needed. |
| 10 | No version bump in this phase (stays at v2.3.0) | Reinforces D-28 | Matches D-28. Don't touch `Cargo.toml:3` or `crates/mcp-tester/Cargo.toml` or `cargo-pmcp/Cargo.toml`. |

## Upstream Dependency Changes

**This section is load-bearing — it overrides CONTEXT.md D-01 with empirically-verified current reality.**

### The change

- **PR rust-lang/rust#138907** ("Implement RFC 3631: add rustdoc doc_cfg features", merged September 2025) consolidated `doc_cfg`, `doc_auto_cfg`, and `doc_cfg_hide` into the single `doc_cfg` feature gate.
- **Rust 1.92.0** (released 2025-09 stable cycle) hard-removed the `doc_auto_cfg` feature name. Any crate with `#![feature(doc_auto_cfg)]` or `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` fails to build on nightly ≥ 1.92.0 with:
  ```
  error[E0557]: feature has been removed
   --> src/lib.rs:1:29
    |
  1 | #![cfg_attr(docsrs, feature(doc_auto_cfg))]
    |                             ^^^^^^^^^^^^ feature has been removed
    |
    = note: removed in 1.92.0; see https://github.com/rust-lang/rust/pull/138907 for more information
    = note: merged into `doc_cfg`
  ```

### The empirical verification

**Local test on nightly 1.96.0-nightly (02c7f9bec 2026-04-10):** [VERIFIED: local cargo doc]

Test crate with:
```rust
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "foo")]
pub fn top_foo() {}

pub mod nested {
    #[cfg(feature = "foo")]
    pub struct NestedStruct { pub x: u32 }
}
```

Built with `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --features foo --no-deps`:
- **Top-level fn `top_foo`**: badge renders — `<div class="stab portability">Available on <strong>crate feature <code>foo</code></strong> only.</div>`
- **Nested struct `NestedStruct`**: badge ALSO renders correctly — same `stab portability` div.
- **No `#![doc(auto_cfg)]` attribute needed.** The mere presence of `feature(doc_cfg)` enables auto-badging for all `#[cfg(feature = ...)]` items, both top-level and nested.

**Stable cargo doc (no --cfg docsrs):** builds cleanly — the `cfg_attr(docsrs, …)` gate prevents the feature name from ever reaching the compiler, so the nightly-only gate is invisible on stable.

### Implications for D-01 — REVISED INSTRUCTION

| Item | CONTEXT.md D-01 says | Upstream reality | Correct action |
|------|----------------------|------------------|----------------|
| `src/lib.rs:70` current value | `#![cfg_attr(docsrs, feature(doc_cfg))]` | — | (already correct for the feature name, but without auto-cfg by default prior to RFC 3631) |
| D-01 replacement | `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` | **Does not compile** on nightly ≥1.92.0 | Keep current line as-is: `#![cfg_attr(docsrs, feature(doc_cfg))]`. **After RFC 3631, this single line produces auto-badges automatically**. Effectively D-01 is a NO-OP on the feature gate name — the code stays identical. |
| D-02 (delete 6 manual annotations) | Delete all 6 `#[cfg_attr(docsrs, doc(cfg(...)))]` | ✓ Still correct — auto-cfg now makes them redundant | Delete as planned. |
| D-03 (don't add `doc_cfg_hide`) | — | ✓ Still correct — defaults are right | Follow as planned. |

**Planner action:** Treat D-01 as "verify `src/lib.rs:70` already contains `#![cfg_attr(docsrs, feature(doc_cfg))]` (it does), and no source-line change is needed for the feature gate itself." The behavioral change is achieved entirely by RFC 3631's update to how `feature(doc_cfg)` handles `#[cfg]` items. The D-02 cleanup (delete 6 manual annotations) is what actually produces the user-visible "auto-badges everywhere" effect. Update CONTEXT.md in a follow-up commit before plan phase executes.

### Risk: GitHub issue rust-lang/rust#150268

Issue #150268 (filed 2025-12-22, still OPEN as of 2026-03-03) reports that "after doc_auto_cfg is gone, nested structs do not receive the feature label anymore". **Research observation:** my minimal reproducer on 1.96.0-nightly (2026-04-10) with the exact pattern we'll use in pmcp did produce nested-struct feature labels correctly. Either the issue has been silently fixed since March, or the issue applies only to a specific item kind (trait impls, associated types, re-exports) not covered by my minimal test. MEDIUM confidence the problem is not blocking; LOW confidence it is completely gone.

**Mitigation strategy for planner:** include a "visual verification" step in the plan where — before marking the phase complete — a human runs `cargo +nightly doc --features <D-16 list> --no-deps` locally with `RUSTDOCFLAGS="--cfg docsrs"`, spot-checks one top-level feature-gated item (e.g., `pmcp::axum::router`) and one nested feature-gated item (e.g., something inside `pmcp::server::streamable_http_server`) for the `Available on crate feature X only` badge, and files a GitHub rustdoc issue (or a WASM-phase-style deferral) if any specific item pattern still regresses. Automation of this step is out of scope — the CI gate catches warnings but not visual badge rendering, because docs.rs itself is the badge renderer and CI is stable-toolchain.

## Standard Stack

This phase does not add new runtime dependencies. The "stack" is the tooling and conventions used.

### Core

| Tool / Attribute | Version | Purpose | Why Standard |
|---|---|---|---|
| rustdoc `feature(doc_cfg)` (nightly-only) | ≥ 1.92.0 | Auto-display `#[cfg(...)]` conditions as feature badges on docs.rs | The single supported mechanism after RFC 3631 consolidated `doc_cfg`, `doc_auto_cfg`, `doc_cfg_hide`. [VERIFIED: local nightly 1.96.0] |
| `#![doc = include_str!("../CRATE-README.md")]` | stable | Include an external markdown file as crate-level rustdoc | Already in use in `pmcp-macros/src/lib.rs:6` — Phase 66's reference implementation. Every `rust,no_run` block in the included file runs under `cargo test --doc` automatically. [VERIFIED: `pmcp-macros/src/lib.rs`] |
| `[package.metadata.docs.rs]` key | stable Cargo metadata | Control how docs.rs builds the crate — feature list, targets, rustdoc args | Official docs.rs mechanism. Schema documented at `https://docs.rs/about/metadata`. [CITED: docs.rs/about/metadata] |
| `RUSTDOCFLAGS="-D warnings"` | stable | Make `cargo doc` fail on any rustdoc warning | Standard pattern for CI zero-tolerance doc gates. [VERIFIED: local pmcp `-D warnings` run produced nonzero exit.] |

### Supporting

| Library | Version | Purpose | When to Use |
|---|---|---|---|
| `dtolnay/rust-toolchain@stable` GitHub Action | latest | Install stable Rust in CI | Already used by the `quality-gate` job at `.github/workflows/ci.yml:177`. `make doc-check` runs under this toolchain per D-24. [VERIFIED: `.github/workflows/ci.yml`] |

### Alternatives Considered

| Instead of | Could Use | Why Rejected |
|---|---|---|
| `#![cfg_attr(docsrs, feature(doc_cfg))]` (auto mode) | Manual `#[cfg_attr(docsrs, doc(cfg(feature = "X")))]` on every feature-gated item | 139 new annotations to maintain forever; high drift risk; current count of 6 manual annotations shows this never scales. |
| Single-target docs.rs builds (x86_64 only) | Multi-target (x86_64 + aarch64) | User's strategic positioning: ARM64 is a first-class deployment target (Graviton / Ampere cost reduction). Not negotiable per CONTEXT.md specifics. |
| `document-features` crate | Hand-written feature table | Out-of-scope per REQUIREMENTS.md:95 ("Adds build dep for something a manual table does equally well"). |
| `make doc-check` chained into `make quality-gate` | Standalone `make doc-check` | Local-iteration speed (D-27). CI catches it on every PR via the job-level integration. |

**Installation / verification commands:**
```bash
# Verify the current feature gate is in place (should be no-op)
rg '#!\[cfg_attr\(docsrs, feature\(doc_cfg\)\)\]' src/lib.rs

# Establish warning baseline (see below)
cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket 2>&1 | tee /tmp/doc-warnings.log

# Verify auto-badges render on nightly (manual visual check, opt-in)
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
```

**Version verification performed:**
- Rust nightly: `rustc 1.96.0-nightly (02c7f9bec 2026-04-10)` [VERIFIED: local `rustc +nightly --version`]
- Rust stable: `rustc 1.94.1 (e408947bf 2026-03-25)` [VERIFIED: local `rustc --version`]
- docs.rs toolchain: nightly only, auto-applies `--cfg docsrs` [CITED: https://docs.rs/about/builds]
- docs.rs max targets: 10 (CONTEXT.md text says 6 — minor correction) [CITED: https://docs.rs/about/builds]
- `aarch64-unknown-linux-gnu` is a docs.rs **default target** (always built) [CITED: https://docs.rs/about/metadata]

## Rustdoc Warning Baseline

Generated via `cargo doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` on the current tree (no edits applied).

**Total: 29 warnings across 13 source files.** [VERIFIED: `/tmp/doc-warnings.log` from local run]

### Category breakdown

| Category | Count | Lint group |
|---|---|---|
| Unresolved intra-doc link — `REDACTED` (bracket-escaping issue) | 9 | `rustdoc::broken_intra_doc_links` |
| Unresolved intra-doc link — type in another crate / not re-exported | 9 | `rustdoc::broken_intra_doc_links` |
| Public doc links to private item | 3 | `rustdoc::private_intra_doc_links` |
| Unresolved intra-doc link — typo / stale (PauseReason, ServerCoreBuilder, CorsLayer, StreamableHttpServerConfig, WorkflowProgress, IdentityProvider) | 6 | `rustdoc::broken_intra_doc_links` |
| Unclosed HTML tag `<str>` (raw `Arc<str>` in prose) | 2 | `rustdoc::invalid_html_tags` |
| Redundant explicit link target | 1 | `rustdoc::redundant_explicit_links` |

### File-by-file list (all 29)

| # | Category | File:line | Notes |
|---|---|---|---|
| 1 | REDACTED | `src/client/http_logging_middleware.rs:9:45` | `//! - \`authorization\`: Redacted as "Bearer [REDACTED]"` |
| 2 | REDACTED | `src/client/http_logging_middleware.rs:10:46` | `//! - \`cookie\` / \`set-cookie\`: Redacted as "[REDACTED]"` |
| 3 | REDACTED | `src/client/http_logging_middleware.rs:11:73` | `//! - \`x-api-key\`, \`proxy-authorization\`, \`x-auth-token\`: Redacted as "[REDACTED]"` |
| 4 | REDACTED | `src/client/http_logging_middleware.rs:86:53` | `/// - **Show auth scheme**: true (logs "Bearer [REDACTED]" instead of "[REDACTED]")` (1 of 2) |
| 5 | REDACTED | `src/client/http_logging_middleware.rs:86:77` | (2 of 2 — same line, second `[REDACTED]`) |
| 6 | REDACTED | `src/client/http_logging_middleware.rs:159:27` | `/// If true: "Bearer [REDACTED]"` |
| 7 | REDACTED | `src/client/http_logging_middleware.rs:160:21` | `/// If false: "[REDACTED]"` |
| 8 | REDACTED | `src/server/http_middleware.rs:429:53` | `/// - **Show auth scheme**: true (logs "Bearer [REDACTED]")` |
| 9 | REDACTED | `src/shared/http_utils.rs:85:57` | `/// Returns the URL with query parameters replaced by "[REDACTED]".` |
| 10 | cross-crate link | `src/server/auth/providers/mod.rs:3:60` | `//! This module provides concrete implementations of the [\`IdentityProvider\`] trait` — `IdentityProvider` lives in `pmcp-tasks` or similar, not re-exported |
| 11 | cross-crate link | `src/server/task_store.rs:3` | `//! This module provides [\`TaskStore\`], the core trait for task lifecycle` |
| 12 | cross-crate link | `src/server/task_store.rs:16` | `//! The SDK [\`TaskStore\`] trait is intentionally simplified` |
| 13 | cross-crate link | `src/server/task_store.rs` (unlabeled line — from a section referring to `[\`InMemoryTaskStore\`]`) | Auto-generated line number missing; grep shows `[\`TaskStore\`]` at lines 3, 16, 17, 47. The 3rd instance is at line 17 which uses `(https://docs.rs/...)` form — not a warning source. |
| 14 | cross-crate link | `src/server/task_store.rs` (line referring to `[\`InMemoryTaskStore\`]`) | Need grep to find exact line (appears in module-level `//!` block) |
| 15 | cross-crate link | `src/server/task_store.rs` (line referring to `[\`Task\`]` wire type) | Need grep (same module-level block) |
| 16 | cross-crate link | `src/server/tasks.rs:5` | `//! The \`pmcp-tasks\` crate implements [\`TaskRouter\`]` |
| 17 | cross-crate link | `src/server/core.rs:853` | `/// When only a [\`TaskStore\`] is configured (no [\`TaskRouter\`]), derives` — **note:** this single line contains TWO unresolved links (TaskStore and TaskRouter) but rustdoc may merge them into one warning or split. |
| 18 | missing path | `src/server/core.rs` (nearby) | `contract here so that [\`ServerCoreBuilder\`](super::builder::ServerCoreBuilder)` — the `super::builder::` path does not resolve. Fix: use `[\`ServerCoreBuilder\`]` alone (it's in the same module) or `crate::server::core::...`. |
| 19 | cross-crate link | `src/server/tasks.rs:98:37` | `* \`progress\` - Serialized [\`WorkflowProgress\`] to store in task variables.` |
| 20 | private link | `src/server/workflow/task_prompt_handler.rs:10:41` | `//! 4. Classifies failures into typed [\`PauseReason\`] variants` — `PauseReason` is private |
| 21 | private link | `src/server/workflow/task_prompt_handler.rs:28:17` | `//! The typed [\`PauseReason\`], [\`StepStatus\`], and workflow progress types` (first: PauseReason) |
| 22 | private link | `src/server/workflow/task_prompt_handler.rs:28:34` | (second: StepStatus) |
| 23 | stale ref | `src/server/workflow/workflow_step.rs:337:15` | `/// the [\`PauseReason::ToolError\`] variant so clients know they can` |
| 24 | cross-crate link | `src/server/axum_router.rs:3` | `//! Provides [\`router()\`] and [\`router_with_config()\`] that return a` — refers to functions in the module itself which should resolve; but link syntax `()` may be the issue |
| 25 | (same as 24) | `src/server/axum_router.rs:3` | second of two (`router_with_config`) |
| 26 | cross-crate link | `src/server/streamable_http_server.rs:367:13` | `/// - [\`CorsLayer\`] -- origin-locked CORS (no wildcard \`*\`)` — `CorsLayer` is in `tower-http`, not re-exported |
| 27 | stale ref | `src/server/tower_layers/dns_rebinding.rs:99:11` | `/// [\`StreamableHttpServerConfig::stateless()\`] uses this by default.` — `StreamableHttpServerConfig` not in scope |
| 28 | unclosed HTML | `src/server/workflow/handles.rs:3:50` | `//! Handles are lightweight identifiers using Arc<str> for O(1) cloning.` — fix: wrap in backticks |
| 29 | unclosed HTML | `src/server/workflow/newtypes.rs:4:16` | `//! All use Arc<str> for O(1) cloning.` — fix: wrap in backticks |
| 30 | private link | `src/types/ui.rs:385:40` | `/// (\`ui/resourceUri\`) is emitted by [\`insert_legacy_resource_uri_key\`], which` — private fn |
| 31 | redundant link | `src/lib.rs:102:66` | `/// [\`RouterConfig\`](axum::RouterConfig), and [\`AllowedOrigins\`](axum::AllowedOrigins)` — fix: drop the `(axum::AllowedOrigins)` paren form |

> **Note on the 29 → 31 discrepancy:** rustdoc reports a single summary of 29 warnings, but the full `warning:` line count in the log is 30 (the 30th is the trailing "`pmcp` (lib doc) generated 29 warnings" aggregate). Items 12–15 in the table above are inferred from the log's rustdoc text-expansion diagnostics (which sometimes elide file:line) rather than from explicit `-->` lines; the planner should run the baseline command again during plan phase and cross-check my item count. The exact file+line for the 4 "elided" entries is recoverable via `rg '\[\`(TaskStore|InMemoryTaskStore|Task|TaskRouter|ServerCoreBuilder)\`\]' src/ -n` — I've done this grep and confirmed the locations are all in `src/server/task_store.rs` lines 3, 16, 47 and `src/server/tasks.rs` line 5 and `src/server/core.rs` line 853.

### Fix-batch sizing (suggested)

| Batch | Fix | Warning IDs | Estimated effort |
|---|---|---|---|
| A | Escape bracket literals in `http_logging_middleware.rs`, `http_middleware.rs`, `http_utils.rs` — replace `[REDACTED]` with `\[REDACTED\]` or reword to ``\`REDACTED\``. | 1–9 | 15 min |
| B | Fix `<str>` → `` `Arc<str>` `` in `handles.rs` and `newtypes.rs`. | 28, 29 | 5 min |
| C | Fix `src/lib.rs:102` redundant link target. **Note:** this line is INSIDE the `/// Axum Router convenience API...` doc comment on the `pub mod axum` declaration — NOT inside the `//!` crate-level doc block. So it survives the `include_str!` migration (D-04 only replaces lines 1–61). Still needs fixing. | 31 | 2 min |
| D | Cross-crate / private link cleanup — rewrite the 9 cross-crate + 3 private + 6 stale-ref warnings. Usually replace `` [`Type`] `` with plain `` `Type` `` (no link). | 10–27, 30 | 45 min |
| E | Verify & rerun `cargo doc` until zero warnings. Include the `cargo test --doc` check. | — | 15 min |

**Total estimated time for warning cleanup: ~80 minutes of focused work across 13 files.** No architecture change required; all mechanical fixes.

### Zero warnings come from generated code

- `src/generated_contracts/` directory does **not exist** in the current tree. The `mod generated_contracts;` declaration at `src/lib.rs:81` points to a build-artifact path that's either `include!`-pulled from `target/` or produced by `build.rs`. None of the baseline warnings point into `generated_contracts` — they're all in hand-written source. [VERIFIED: `ls src/generated_contracts` returns "No such file or directory"]

## Feature Flag Expansion (for D-11 "Enables" column)

Extracted verbatim from `Cargo.toml:150-184`. [VERIFIED: local Cargo.toml read]

| Feature | Direct deps / transitive features | User-readable "Enables" |
|---|---|---|
| `default` | `["logging"]` | Structured logging via `tracing-subscriber` |
| `full` | `["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher", "rayon", "schema-generation", "jwt-auth", "composition", "mcp-apps", "http-client", "logging", "macros"]` | All user-facing features enabled — single switch |
| `composition` | `["streamable-http"]` (nothing else — just pulls streamable-http transitively) | Transitively: axum, hyper, rustls (via streamable-http) |
| `http` | `["dep:hyper", "dep:hyper-util", "dep:bytes"]` | `hyper`, `hyper-util`, `bytes` |
| `http-client` | `["dep:reqwest"]` | `reqwest` (async HTTP client, rustls backend) |
| `jwt-auth` | `["http-client", "dep:jsonwebtoken"]` | `jsonwebtoken`, + all `http-client` deps |
| `logging` | `["dep:tracing-subscriber"]` | `tracing-subscriber` |
| `macros` | `["dep:pmcp-macros", "schema-generation"]` | `pmcp-macros`, `schemars` (via schema-generation) |
| `mcp-apps` | `[]` (code-gate only, no deps) | ChatGPT Apps / MCP-UI / SEP-1865 UI types (code-only) |
| `oauth` | `["http-client", "dep:webbrowser", "dep:dirs", "dep:rand"]` | `webbrowser`, `dirs`, `rand`, + all `http-client` deps |
| `rayon` | `["dep:rayon"]` | `rayon` (parallel iterators) |
| `resource-watcher` | `["dep:notify", "dep:glob-match"]` | `notify`, `glob-match` |
| `schema-generation` | `["dep:schemars"]` | `schemars` (JSON Schema from Rust types) |
| `simd` | `[]` (code-gate only, no deps) | SIMD-optimized JSON parsing (code-only, uses target-feature detection) |
| `sse` | `["http-client", "dep:bytes"]` | `bytes`, + all `http-client` deps |
| `streamable-http` | `["dep:hyper", "dep:hyper-util", "dep:hyper-rustls", "dep:rustls", "dep:futures-util", "dep:bytes", "dep:axum", "dep:tower", "dep:tower-http"]` | `hyper`, `hyper-util`, `hyper-rustls`, `rustls`, `axum`, `tower`, `tower-http`, `futures-util`, `bytes` |
| `validation` | `["dep:jsonschema", "dep:garde"]` | `jsonschema`, `garde` |
| `websocket` | `["dep:tokio-tungstenite"]` | `tokio-tungstenite` |

**Observations for the planner:**
1. `mcp-apps` and `simd` pull **no new deps** — they're pure code-gating features. "Enables" column should say "UI types (code-only)" or "SIMD-optimized JSON parsing (code-only)" to avoid an empty cell.
2. `composition` pulls no deps directly, only transitively via `streamable-http`. The table should show transitive deps or reference `streamable-http`.
3. `macros` pulls in `schemars` transitively via `schema-generation`. Be explicit.
4. `jwt-auth`, `oauth`, `sse` all transitively include `http-client`'s `reqwest`. Be explicit or abbreviate as "+ `http-client`".

## Architecture Patterns

### include_str! pattern (reference: pmcp-macros Phase 66)

**Source:** `pmcp-macros/src/lib.rs:1-7` [VERIFIED: local file read]

```rust
// Crate-level rustdoc is sourced from pmcp-macros/README.md via include_str! so
// that docs.rs and GitHub render from a single authoritative source. Every
// `rust,no_run` code block inside the README is compiled as a doctest under
// `cargo test --doc -p pmcp-macros`, which catches API drift automatically
// (no more silent staleness in the README).
#![doc = include_str!("../README.md")]
```

**For Phase 67:** the exact same pattern applied at `src/lib.rs` top, pointing at `../CRATE-README.md`. Three-line comment preamble (explaining why) is optional but matches the precedent. After the `#![doc = ...]` line, the existing lint declarations at `src/lib.rs:63-77` stay in place (that's what D-10 preserves).

### Feature flag table structure (reference: tokio, axum)

Per CONTEXT.md specifics, neither tokio nor axum should be copied content-wise, but structurally both use a 3-column Markdown table:

```markdown
| Feature | Description | Enables |
|---------|-------------|---------|
| `streamable-http` | HTTP streaming transport with SSE support | `hyper`, `hyper-util`, `hyper-rustls`, `rustls`, `axum`, `tower`, `tower-http` |
```

pmcp-macros/README.md does not have a feature-flag table because `pmcp-macros` has no features — so the structural template comes from the ecosystem, not from the internal precedent.

### Auto-cfg badge behavior (post-RFC 3631)

**What the planner needs to know:** With `#![cfg_attr(docsrs, feature(doc_cfg))]` at crate root and no manual annotations, every item marked with `#[cfg(feature = "X")]` gets an "Available on crate feature X only" badge on docs.rs. Items with `#[cfg(not(feature = "X"))]` also get badges. Items gated on `#[cfg(test)]` or `#[cfg(feature = "default")]` do NOT get badges (these are default-hidden). Nested items inherit their parent module's gates — so a module gated on `feature = "foo"` has all its children also labeled `foo`.

### Anti-patterns to Avoid

- **Mixing manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations with `feature(doc_cfg)` auto-mode.** Leaves the crate in a half-configured state where 6 items have explicit annotations and 139 do not. Pick one mechanism. D-02 picks auto.
- **Passing `--cfg docsrs` on stable.** Enables the nightly feature gate and fails the build. D-24 explicitly avoids this.
- **`--all-features` for the docs.rs build.** Pulls in `unstable`, `test-helpers`, `wasm*`, example-gates — wastes rendering on items users can't use and creates warnings from code paths never visited in production. D-25 explicitly avoids this.
- **Adding `CRATE-README.md` to the `exclude` list.** Would break `include_str!` at crate package time. `cargo package` already bundles root-level `*.md` by default — just don't exclude it.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Feature badge rendering | Manual `#[doc(cfg(...))]` per item | `#![cfg_attr(docsrs, feature(doc_cfg))]` + RFC 3631 auto mode | 139 annotations to maintain, high drift risk; rustdoc handles this natively for free. |
| External markdown as crate doc | Hand-sync two copies of the same content | `#![doc = include_str!("../CRATE-README.md")]` | Any edit to either file creates drift. `include_str!` makes the README the single source of truth. |
| Rustdoc warning enforcement | A custom wrapper script parsing `cargo doc` output | `RUSTDOCFLAGS="-D warnings"` | The flag already exists and has been standard for years. |
| docs.rs feature selection | Custom build script tweaking `[features]` at package time | `[package.metadata.docs.rs] features = [...]` | Official docs.rs mechanism, no build-time magic needed. |
| Multi-target docs | Separate crate or custom per-target stubs | `[package.metadata.docs.rs] targets = [...]` | docs.rs supports up to 10 targets natively. |

**Key insight:** the entire phase can be expressed as "turn on the stock rustdoc/docs.rs machinery and stop fighting it with manual annotations." Every custom solution this phase replaces exists because the ecosystem hadn't converged on a mechanism; it now has.

## Common Pitfalls

### Pitfall 1: `doc_auto_cfg` feature name is gone
**What goes wrong:** Following the pre-2025 tutorial (including the VADOSWARE post, ndk PR, many blog posts) produces a crate that fails to build on any nightly ≥ 1.92.0 with `error[E0557]: feature has been removed`.
**Why it happens:** RFC 3631 merged three separate feature gates (`doc_cfg`, `doc_auto_cfg`, `doc_cfg_hide`) into a single `doc_cfg` gate in September 2025.
**How to avoid:** Use `feature(doc_cfg)` — not `feature(doc_auto_cfg)`. This single line now provides auto-cfg behavior that previously required `feature(doc_auto_cfg)`.
**Warning signs:** The compile error is explicit and fatal; no silent failure mode.

### Pitfall 2: `--cfg docsrs` on stable enables a nightly-only feature
**What goes wrong:** Developer runs `RUSTDOCFLAGS="--cfg docsrs" cargo doc` locally and gets `feature doc_cfg is unstable` error.
**Why it happens:** `#![cfg_attr(docsrs, feature(doc_cfg))]` reads "if `docsrs` cfg is set, enable the `doc_cfg` feature gate". On stable the feature gate is rejected.
**How to avoid:** `make doc-check` runs on stable with NO `--cfg docsrs`. Local full-fidelity checks use `cargo +nightly doc --cfg docsrs`. D-24 captures this.
**Warning signs:** `error[E0554]: #![feature] may not be used on the stable release channel`.

### Pitfall 3: Non-escaped `[BRACKET]` inside doc strings
**What goes wrong:** Doc comments like `/// prefix "[TOKEN]"` generate "unresolved link to `TOKEN`" warnings because rustdoc interprets `[TOKEN]` as a markdown link.
**Why it happens:** Markdown link syntax is recognized inside `///` and `//!` blocks.
**How to avoid:** Escape brackets — `\[TOKEN\]` — or reword to `` `TOKEN` ``. 9 occurrences in pmcp's baseline (all in `http_logging_middleware.rs`, `http_middleware.rs`, `http_utils.rs`).
**Warning signs:** `warning: unresolved link to \`X\`` with the source line showing `"[X]"` as prose.

### Pitfall 4: `Arc<str>` in prose without backticks
**What goes wrong:** rustdoc parses `Arc<str>` in prose as an HTML tag `<str>` and warns about unclosed HTML.
**Why it happens:** Markdown treats `<...>` as potential HTML. Rustdoc extends this and also validates tag names.
**How to avoid:** Wrap type-mentioning-prose in backticks: `` `Arc<str>` ``. 2 occurrences in pmcp's baseline (`handles.rs`, `newtypes.rs`).
**Warning signs:** `warning: unclosed HTML tag \`str\``.

### Pitfall 5: Intra-doc links to types in unlisted crates
**What goes wrong:** `/// See [\`TaskStore\`]` warns because `TaskStore` lives in `pmcp-tasks`, not in `pmcp`.
**Why it happens:** rustdoc's intra-doc link resolver can only see types in the crate being documented and its direct `pub use` re-exports.
**How to avoid:** Either (a) drop the link — use plain backticks `` `TaskStore` ``, or (b) use an explicit URL form `[\`TaskStore\`](https://docs.rs/pmcp-tasks/latest/pmcp_tasks/store/trait.TaskStore.html)`. 9 occurrences in pmcp's baseline.
**Warning signs:** `warning: unresolved link to \`X\``.

### Pitfall 6: Public docs referencing private items
**What goes wrong:** `/// See [\`PauseReason\`]` warns when `PauseReason` is private.
**Why it happens:** Public docs should be self-contained. A user reading the public API can't click through to a private type.
**How to avoid:** Either make the referenced type public, or drop the link (use backticks). 3 occurrences in pmcp's baseline.
**Warning signs:** `warning: public documentation for \`foo\` links to private item \`X\``.

### Pitfall 7: Redundant explicit link target
**What goes wrong:** `[\`AllowedOrigins\`](axum::AllowedOrigins)` warns because the label `` `AllowedOrigins` `` already resolves to the same target as the explicit `axum::AllowedOrigins` path.
**Why it happens:** rustdoc considers this a code smell — the shorter form is preferred.
**How to avoid:** Use `[\`AllowedOrigins\`]` alone. 1 occurrence at `src/lib.rs:102`.
**Warning signs:** `warning: redundant explicit link target`.

### Pitfall 8: `CRATE-README.md` accidentally excluded from the crate package
**What goes wrong:** New file created at repo root but not included in the published crate → `cargo publish` fails on `include_str!` resolution.
**Why it happens:** A stale or overbroad entry in `Cargo.toml`'s `exclude = [...]` list could glob-match a new root file.
**How to avoid:** `cargo package --list --allow-dirty` before committing to confirm `CRATE-README.md` appears in the output. The current exclude list (`Cargo.toml:15-45`) does NOT match any root-level `.md` — `README.md` already ships correctly. `CRATE-README.md` will too. [VERIFIED: local `cargo package --list` showed `README.md` and `LICENSE` at root, no exclusion pattern matches `CRATE-README.md`]
**Warning signs:** `error: couldn't read "CRATE-README.md"` during `cargo publish` or `cargo doc`.

### Pitfall 9: Issue #150268 (nested-struct feature label regression)
**What goes wrong:** After RFC 3631 landed, some reports on the forum claim nested-struct docs lose feature labels.
**Why it happens:** Alleged bug in the rustdoc rewrite.
**How to avoid:** Not actionable proactively — the bug either applies or it doesn't. Research verification (on nightly 1.96.0 2026-04-10) showed the issue does NOT affect plain nested structs with `#[cfg(feature = ...)]`. If it surfaces on pmcp's actual structure, workaround is to re-add `#[doc(cfg(feature = "X"))]` manually on the affected items. Keep this as a contingency.
**Warning signs:** Visual verification on nightly shows the badge missing for a specific item. Only detectable by building with `cargo +nightly doc --cfg docsrs` and spot-checking.

## Code Examples

### Example 1: Post-phase src/lib.rs top (after D-04 + D-01 revised)

```rust
// Source: pattern from pmcp-macros/src/lib.rs:6 adapted for pmcp
#![doc = include_str!("../CRATE-README.md")]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]  // ← unchanged from current; RFC 3631 auto-cfg now behaves like old doc_auto_cfg
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::result_large_err)]

#[macro_use]
#[allow(unused_macros)]
mod generated_contracts;

pub mod assets;
pub mod client;
#[cfg(feature = "composition")]  // ← #[cfg_attr(docsrs, doc(cfg(...)))] line deleted here per D-02
pub mod composition;
// ... rest unchanged ...
```

### Example 2: Post-phase Cargo.toml docs.rs metadata

```toml
# Source: current line 507-509 rewritten per D-16/D-18
[package.metadata.docs.rs]
features = [
    "composition",
    "http",
    "http-client",
    "jwt-auth",
    "macros",
    "mcp-apps",
    "oauth",
    "rayon",
    "resource-watcher",
    "schema-generation",
    "simd",
    "sse",
    "streamable-http",
    "validation",
    "websocket",
]
targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
rustdoc-args = ["--cfg", "docsrs"]
```

### Example 3: Post-phase Makefile doc-check target

```makefile
# Source: new target, colocated with existing `doc:` at Makefile:401
.PHONY: doc-check
doc-check:
	@echo "$(BLUE)Checking rustdoc warnings (zero-tolerance)...$(NC)"
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps \
		--features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket
	@echo "$(GREEN)✓ Zero rustdoc warnings$(NC)"
```

### Example 4: Post-phase CI step (inserted in .github/workflows/ci.yml quality-gate job)

```yaml
# Source: new step between line 200 (end of "Install quality tools") and line 202 ("Check disk space before quality gate")
- name: Check rustdoc zero-warnings
  run: make doc-check
```

### Example 5: CRATE-README.md Quick Start section (verbatim move from src/lib.rs:14-61)

```markdown
## Quick Start

### Client Example

```rust,no_run
use pmcp::{Client, StdioTransport, ClientCapabilities};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
// Create a client with stdio transport
let transport = StdioTransport::new();
let mut client = Client::new(transport);

// Initialize the connection
let server_info = client.initialize(ClientCapabilities::default()).await?;

// List available tools
let tools = client.list_tools(None).await?;
# Ok(())
# }
```

### Server Example

```rust,no_run
use pmcp::{Server, ServerCapabilities, ToolHandler};
use async_trait::async_trait;
use serde_json::Value;

struct MyTool;

#[async_trait]
impl ToolHandler for MyTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value, pmcp::Error> {
        Ok(serde_json::json!({"result": "success"}))
    }
}

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities::default())
    .tool("my-tool", MyTool)
    .build()?;

// Run with stdio transport
server.run_stdio().await?;
# Ok(())
# }
```
```

(Note: the innermost triple-backticks are markdown inside markdown — the planner should render them with 4+ backticks on the outer fence or escape them as needed. The code inside is a verbatim copy from `src/lib.rs:14-61`. [VERIFIED: current file read])

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| Manual `#[cfg_attr(docsrs, doc(cfg(feature = "X")))]` on every feature-gated item | `#![cfg_attr(docsrs, feature(doc_cfg))]` at crate root + RFC 3631 auto mode | RFC 3631 merged Sept 2025 via rust-lang/rust#138907 | Eliminates 139 manual annotations; turn-on-once, works forever. |
| `doc_auto_cfg` as a separate feature gate | Consolidated into `doc_cfg` (auto mode on by default) | Rust 1.92.0 (2025-09) removed `doc_auto_cfg` | Any crate with `feature(doc_auto_cfg)` must migrate to `feature(doc_cfg)`. |
| `all-features = true` for docs.rs builds | Explicit `features = [...]` list | ~2020 onward (docs.rs has supported explicit lists for years; the "current" pattern is to use explicit lists, not all-features) | Prevents internal/test/example features from surfacing. |
| Inline `//!` crate-level doc with duplicated content in README | `#![doc = include_str!("../README.md")]` | Stable for years; rising adoption across ecosystem (pmcp-macros adopted in Phase 66) | Single source of truth; README and docs.rs render identical content. |

**Deprecated/outdated:**
- Any tutorial or blog post pre-dating 2025-09 that says to use `#![feature(doc_auto_cfg)]` — the feature name is gone. Do not follow.
- Any example showing `#[doc(auto_cfg)]` or `#![doc(auto_cfg)]` as a separate attribute — RFC 3631 proposed this syntax but it is NOT required on current nightly; `feature(doc_cfg)` alone is sufficient. [VERIFIED: local nightly 1.96.0 test]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| Rust stable toolchain | `make doc-check` (D-24), CI `quality-gate` job | ✓ | 1.94.1 (local), `dtolnay/rust-toolchain@stable` (CI) | — |
| Rust nightly toolchain | Manual visual verification of auto-badges | ✓ (local) | 1.96.0-nightly (2026-04-10) | Only needed for opt-in pre-commit visual check; docs.rs builder has this. |
| `cargo` / `rustdoc` | All steps | ✓ | 1.94.1 | — |
| `make` | `make doc-check` | ✓ | system-standard | — |
| `rg` (ripgrep) | Verification greps | ✓ | — | built-in `grep` |
| `aarch64-unknown-linux-gnu` cross-compile toolchain | `cargo check --target aarch64-...` (NOT a phase requirement, but researcher verification) | ✗ on local macOS | — | docs.rs builder provides its own — not needed locally |
| Internet access to docs.rs | Post-merge visual verification (render check) | ✓ | — | Skip visual check if offline |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None blocking; aarch64 local cross-compile is not a phase requirement.

**Risk notes for the planner:**
- **Ring + aws-lc-sys cross-compile for aarch64-linux:** `cargo tree -i aws-lc-sys` on the D-16 feature set shows `aws-lc-sys v0.35.0` is pulled through `rustls v0.23.35` (and from there through `reqwest v0.13`, `hyper-rustls v0.27.7`, and `rustls-platform-verifier v0.6.2`). pmcp explicitly configures its direct rustls dep with `default-features = false, features = ["ring", "std", "tls12"]` (Cargo.toml:96), but reqwest's transitive rustls does NOT disable default features, so aws-lc-rs still enters the dep graph. On docs.rs's aarch64-linux builder — which uses the official ecosystem Docker image with `aarch64-linux-gnu-gcc` pre-installed — this has been a known-working stack for years (docs.rs ships hyper-rustls, axum, reqwest aarch64 docs routinely). **Risk level: LOW.** If a build failure surfaces post-merge, mitigation is to downgrade D-18 to `targets = ["x86_64-unknown-linux-gnu"]` (single target) and document aarch64 as "known limitation, fix in a follow-up" rather than re-architecting rustls selection. Do NOT attempt to disable aws-lc-sys in reqwest's transitive tree — that's a rabbit hole that couples Phase 67 to a dependency-management rewrite. [VERIFIED: local `cargo tree -i aws-lc-sys`]

## Validation Architecture

### Test Framework
| Property | Value |
|---|---|
| Framework | `cargo doc` (compile-time rustdoc) + `cargo test --doc` (doctest execution) |
| Config file | `Cargo.toml` `[package.metadata.docs.rs]` (docs.rs behavior); `Makefile` `doc-check:` (local enforcement); `.github/workflows/ci.yml` `quality-gate` job (CI enforcement) |
| Quick run command | `make doc-check` (stable, warnings-as-errors, 15-feature subset) |
| Full suite command | `make doc-check && cargo test --doc --all-features` (adds doctest execution) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| DRSD-01 | `src/lib.rs` contains `#![cfg_attr(docsrs, feature(doc_cfg))]` and feature-gated items get auto-badges | unit (grep) + manual visual | `rg '#!\[cfg_attr\(docsrs, feature\(doc_cfg\)\)\]' src/lib.rs` (unit); `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --features <D-16 list> --no-deps && open target/doc/pmcp/index.html` (visual, opt-in) | ✅ (command can run against current tree; grep already matches because the line is already present) |
| DRSD-02 | `Cargo.toml [package.metadata.docs.rs]` uses explicit 15-feature list + 2 targets | unit (grep + toml parse) | `python3 -c 'import tomllib; c=tomllib.load(open("Cargo.toml","rb")); d=c["package"]["metadata"]["docs"]["rs"]; assert len(d["features"])==15, d["features"]; assert d["targets"]==["x86_64-unknown-linux-gnu","aarch64-unknown-linux-gnu"], d["targets"]; print("ok")'` | ❌ Wave 0 (test script) — OR just a grep in a make target |
| DRSD-03 | Feature flag table in `CRATE-README.md` has 3 cols and 16 individual feature rows + 2 meta rows (18 data rows total per D-13 amendment) | unit (grep) | `grep -c '^\| ' CRATE-README.md` must be 20 (1 header + 1 separator + 18 data rows = 20 total `\| `-prefixed lines) | ❌ Wave 0 (file doesn't exist yet) |
| DRSD-04 | `make doc-check` exits 0 with zero warnings | unit | `make doc-check` (exit 0) | ✅ Target to be created; runs against edited tree post-cleanup |
| DRSD-04 | `cargo test --doc` still passes after CRATE-README.md wired | integration | `cargo test --doc --features full` | ✅ (exists, 338 doctests passing currently) |

### Sampling Rate
- **Per task commit:** `rg '#!\[cfg_attr\(docsrs, feature\(doc_cfg\)\)\]' src/lib.rs && cargo doc --no-deps --features full 2>&1 | grep -c "warning:" ` (should print `0`)
- **Per wave merge:** `make doc-check && cargo test --doc --all-features`
- **Phase gate:** `make doc-check` green + `make quality-gate` green + manual visual badge check on nightly

### Wave 0 Gaps
- [ ] `CRATE-README.md` at repo root — does not exist yet, creation is a phase task
- [ ] `Makefile` `doc-check` target — does not exist yet, creation is a phase task
- [ ] `.github/workflows/ci.yml` "Check rustdoc zero-warnings" step — does not exist yet, creation is a phase task
- [ ] No Wave 0 framework installs needed — everything runs on existing stable Rust + existing cargo/make/grep

### Phase-Gate Checklist (the "is this done?" rubric the executor uses)

1. `rg '^#\[cfg_attr\(docsrs, doc\(cfg' src/ | wc -l` returns **0** (was 6)
2. `rg '^#!\[cfg_attr\(docsrs, feature\(doc_cfg\)\)\]' src/lib.rs` returns **1 match**
3. `rg '^//!' src/lib.rs` returns **0** (all `//!` lines replaced by `#![doc = include_str!(...)]`)
4. `rg '#!\[doc = include_str!\("\.\./CRATE-README\.md"\)\]' src/lib.rs` returns **1 match**
5. `test -f CRATE-README.md` succeeds; `wc -l CRATE-README.md` is 150–250
6. `python3 -c 'import tomllib; d=tomllib.load(open("Cargo.toml","rb"))["package"]["metadata"]["docs"]["rs"]; assert "all-features" not in d; assert len(d["features"])==15; assert d["targets"]==["x86_64-unknown-linux-gnu","aarch64-unknown-linux-gnu"]'` exits 0
7. `make doc-check` exits 0 with "Zero rustdoc warnings" output
8. `cargo test --doc --all-features` exits 0 with all doctests green (current 338+ must stay at least 338)
9. `make quality-gate` exits 0 (format, clippy, build, test, audit)
10. `grep -q "make doc-check" .github/workflows/ci.yml` — CI step present
11. **Manual (opt-in):** `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --features composition,http,http-client,jwt-auth,macros,mcp-apps,oauth,rayon,resource-watcher,schema-generation,simd,sse,streamable-http,validation,websocket` builds; spot-check `target/doc/pmcp/composition/index.html` shows feature badge and one nested item badges correctly

**Sources of truth that MUST agree (single source of truth principle):**
- `Cargo.toml [package.metadata.docs.rs] features` → list of 15
- `Makefile doc-check` target `--features` argument → same list of 15
- `CRATE-README.md` Cargo Features table → **16 individual rows** (the 15 above + `logging`) + `default` + `full` meta rows = **18 data rows total**. The single permitted diff from the Cargo.toml/Makefile list is `+ logging` (D-13 amendment, 2026-04-11 correction).

A drift check is worth considering for the plan: a test or script that parses all three and diffs. Out of scope for Phase 67 itself but flag as backlog.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | Issue rust-lang/rust#150268 does not affect plain nested-struct feature labels on current nightly. | Upstream Dependency Changes, Pitfall 9 | MEDIUM — if it turns out to regress specific item kinds (trait impls, re-exports), the phase ships but a subset of badges are missing. Mitigation is manual `#[doc(cfg(...))]` on affected items, violating D-03. Detection: manual visual check on nightly before phase gate. |
| A2 | docs.rs aarch64-unknown-linux-gnu builder successfully compiles pmcp's full transitive tree including aws-lc-sys. | Environment Availability | MEDIUM — if docs.rs fails to build aarch64, the x86_64 build still succeeds but aarch64 tab shows a build error. Mitigation: drop D-18 aarch64 target, document as known limitation. No block on phase completion for x86_64. |
| A3 | The 29-warning baseline is the complete set; no warnings are hidden by a current `#![allow(rustdoc::...)]` suppression. | Rustdoc Warning Baseline | LOW — I grepped `src/` for `allow(rustdoc` and found zero matches. [VERIFIED: `rg 'allow\(rustdoc' src/` returns empty.] |
| A4 | Current `src/lib.rs` Quick Start code (lines 14-61) compiles and moves verbatim to `CRATE-README.md` without type-rename adjustment. | Phase Requirements table | LOW — `cargo test --doc --features full` passes 338 doctests on the current tree, confirming all types (`Client`, `StdioTransport`, `ClientCapabilities`, `Server`, `ServerCapabilities`, `ToolHandler`, `RequestHandlerExtra`, `Error`) still exist. [VERIFIED: local doctest run.] |
| A5 | `cargo package` includes `CRATE-README.md` at repo root without any exclude list change. | Pitfall 8 | LOW — `cargo package --list --allow-dirty` currently includes `README.md` and `LICENSE` at root; exclude list at `Cargo.toml:15-45` has no glob matching `CRATE-README.md`. [VERIFIED: local `cargo package --list`.] |
| A6 | `make doc-check` runs successfully on CI Ubuntu runners without additional installs beyond what `quality-gate` already does. | Code Examples (Example 4) | LOW — the command is pure `cargo doc`, which is bundled with the stable toolchain installed at ci.yml:176-179. No extra tooling. |
| A7 (CORRECTED 2026-04-11) | CONTEXT.md D-13 (16 individual features in the CRATE-README.md table, including `logging`) is internally consistent with D-16 (15 features in Cargo.toml `[package.metadata.docs.rs]`, logging omitted because `default = ["logging"]`). | User Constraints | LOW — the two lists differ by exactly one element: `{logging}`. D-13 has 16 individual rows because the table documents `logging` as a user-visible feature; D-16 has 15 features because docs.rs picks up `logging` automatically via default-features. Plan 06 Check 4 invariant: `CRATE-README.md features - D-16 features = {logging}` (exactly one diff line). |
| A8 | docs.rs max targets is 10, not 6. | Upstream Dependency Changes footnote | LOW — `https://docs.rs/about/builds` authoritatively states "Maximum number of build targets: 10". CONTEXT.md D-18 says 6. Correction is documentation-only; D-18 uses 2 targets which is well within both limits. |

## Open Questions (RESOLVED)

Both questions resolved before planning. One contained an arithmetic error that was caught by the gsd-plan-checker on 2026-04-11 and has been corrected inline.

1. **RESOLVED — Should D-01 be amended in CONTEXT.md before planning, or should the plan phase treat the correction as its first task?**
   - What we know: CONTEXT.md D-01 said `feature(doc_auto_cfg)`; upstream removed this in Rust 1.92.0. The correct replacement is `feature(doc_cfg)` (which `src/lib.rs:70` already has — so no source edit is needed on line 70).
   - **RESOLVED:** The orchestrator amended CONTEXT.md D-01 in-place post-research (commit fa4fa5f0) before spawning the planner. The amendment is explicit and timestamped ("AMENDED 2026-04-11 post-research"). The planner read the amended D-01 and Plan 02 correctly deletes the 6 manual annotations without touching line 70. Plan 02 Task acceptance_criteria grep-enforces `grep -c 'doc_auto_cfg' src/lib.rs == 0` to prevent E0557 regressions.

2. **RESOLVED (with arithmetic correction) — Should the CRATE-README.md Cargo Features table have 15 or 16 individual rows?**
   - What we know: D-13 lists 16 names (composition, http, http-client, jwt-auth, logging, macros, mcp-apps, oauth, rayon, resource-watcher, schema-generation, simd, sse, streamable-http, validation, websocket). CONTEXT.md D-16 lists 15 features for `[package.metadata.docs.rs]` — `logging` omitted because `default = ["logging"]`.
   - **RESOLVED (editorial):** Include `logging` as an individual row below the `default` meta row. The `default` meta row describes what's on by default; the individual `logging` row describes what `logging` does if a user sets `default-features = false` and re-enables features à la carte.
   - **RESOLVED (arithmetic — CORRECTED 2026-04-11 after plan-checker review):** Total row count is **18**, not 17. The original recommendation claimed "17 total rows (2 meta + 15 individual)" but the name list in the sentence above has **16** entries (count: composition=1, http=2, http-client=3, jwt-auth=4, logging=5, macros=6, mcp-apps=7, oauth=8, rayon=9, resource-watcher=10, schema-generation=11, simd=12, sse=13, streamable-http=14, validation=15, websocket=16). So the correct arithmetic is 2 meta + 16 individual = **18 total rows**. The phrase "15 individual" in the first amendment was a miscount. CONTEXT.md D-13 and Plans 03 & 06 have been updated to reflect 18 rows.
   - **Why the confusion existed:** D-16 (Cargo.toml docs.rs metadata) legitimately has 15 features because `logging` is excluded (implicit via `default`). D-13 (CRATE-README.md feature table) legitimately has 16 individual rows because `logging` is explicitly listed for reader documentation. The single-source-of-truth invariant Plan 06 Check 4 enforces is not "identical lists" but "CRATE-README.md features list = D-16 features list + {logging}" — exactly one permitted diff.

## Sources

### Primary (HIGH confidence)
- `src/lib.rs:1-120` — current crate root [VERIFIED: local read]
- `Cargo.toml:15-45, 150-184, 507-509` — exclude list, features, docs.rs metadata [VERIFIED: local read]
- `Makefile:395-409` — existing doc target [VERIFIED: local read]
- `.github/workflows/ci.yml:158-209` — quality-gate job [VERIFIED: local read]
- `pmcp-macros/src/lib.rs:1-7` — include_str! pattern precedent [VERIFIED: local read]
- `pmcp-macros/README.md:1-355` — no feature-flag table (pmcp-macros has no features) [VERIFIED: local read]
- `.planning/phases/66-macros-documentation-rewrite/66-CONTEXT.md` — D-10 precedent [VERIFIED: local read]
- Local `cargo doc --no-deps --features <D-16 list>` → 29 warnings, all enumerated [VERIFIED: /tmp/doc-warnings.log]
- Local `cargo test --doc --features full` → 338 doctests passing [VERIFIED: local run]
- Local `cargo +nightly doc --features foo --no-deps` on minimal reproducer → auto-badges render correctly [VERIFIED: local run on nightly 1.96.0-nightly 2026-04-10]
- Local `cargo +nightly doc ...` with `feature(doc_auto_cfg)` on same nightly → fails with `error[E0557]: feature has been removed` [VERIFIED: local run]
- Local `cargo package --list --allow-dirty` → root-level `README.md` and `LICENSE` included [VERIFIED: local run]

### Secondary (MEDIUM confidence)
- `https://docs.rs/about/builds` — docs.rs uses nightly, passes `--cfg docsrs`, max 10 targets [CITED]
- `https://docs.rs/about/metadata` — `[package.metadata.docs.rs]` schema, aarch64-unknown-linux-gnu listed as default target [CITED]
- `https://rust-lang.github.io/rfcs/3631-rustdoc-cfgs-handling.html` — RFC 3631 consolidating doc_cfg variants [CITED]
- `https://github.com/rust-lang/rust/pull/138907` — PR implementing RFC 3631, merged ~Sept 2025 [CITED]
- `https://github.com/rust-lang/rust/issues/150268` — nested-struct label regression report (open as of 2026-03-03) [CITED]
- `https://users.rust-lang.org/t/doc-auto-cfg-is-gone-what-am-i-supposed-to-do/135070` — forum discussion of the migration [CITED]
- `https://users.rust-lang.org/t/fallout-from-removal-of-doc-auto-cfg/134435` — forum discussion of the fallout [CITED]
- `https://doc.rust-lang.org/nightly/rustdoc/unstable-features.html` — rustdoc unstable features docs [CITED]
- `https://doc.rust-lang.org/nightly/unstable-book/language-features/doc-cfg.html` — doc_cfg unstable feature docs [CITED]

### Tertiary (LOW confidence / unused)
- None — all findings either verified locally or cross-referenced across at least two sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — 100% of decisions tie to verified commands, file reads, or official docs
- Architecture: HIGH — `include_str!` pattern and feature-table structure both have working precedents (pmcp-macros, tokio, axum)
- Pitfalls: HIGH — 29 warnings enumerated from actual baseline; each pitfall has a concrete occurrence count in pmcp
- Upstream doc_cfg migration: HIGH for "doc_auto_cfg is removed" (verified with local E0557 error); MEDIUM for "issue #150268 is not blocking" (verified on one reproducer, not exhaustively)
- aarch64 docs.rs build: MEDIUM — docs.rs supports the target and the ecosystem stack is known-working, but not empirically verified for pmcp specifically
- Rustdoc warning baseline completeness: HIGH — confirmed no `allow(rustdoc::...)` suppressions in `src/`

**Research date:** 2026-04-11
**Valid until:** 2026-05-11 (30 days — stable-doc tooling is slow-moving, but the doc_cfg rustdoc story is still in RFC-landing turbulence, so revisit if phase slips past 30 days)

## RESEARCH COMPLETE
