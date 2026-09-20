# Phase 67: docs.rs Pipeline and Feature Flags - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 67-docs-rs-pipeline-and-feature-flags
**Areas discussed:** Auto-cfg + include_str! strategy, Explicit docs.rs feature list, Feature flag table format/placement, CI gate scope/toolchain/version

---

## Area 1: Auto-cfg + include_str! strategy

### Q1.1 — doc_cfg → doc_auto_cfg migration

| Option | Description | Selected |
|--------|-------------|----------|
| Flip to `doc_auto_cfg` + delete all 6 manual annotations | Replace `#![feature(doc_cfg)]` with `#![feature(doc_auto_cfg)]` and remove all 6 `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations. Clean, single mechanism, every `#[cfg(feature)]` item gets auto-badged. Matches rmcp's approach. | ✓ |
| Adopt `doc_auto_cfg` but keep manual annotations | Add `doc_auto_cfg`, leave the 6 manual ones in place as explicit documentation. Safer but mixes two mechanisms and creates drift risk. | |
| Stay on manual `doc_cfg`, add missing annotations to all 145 items | Keep the manual approach but extend it to cover every feature-gated item. Works on stable rustdoc but requires 139 new annotations and continuous discipline. | |

**User's choice:** Flip to `doc_auto_cfg` + delete all 6 manual annotations (D-01, D-02)
**Notes:** Single mechanism wins for drift resistance. The 4% manual coverage (6/145) was already a drift tell.

### Q1.2 — Adopt include_str! for pmcp lib.rs?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, adopt `include_str!` for pmcp lib.rs | Move inline `//!` module doc to a new file and replace with `#![doc = include_str!(...)]`. Single source of truth, matches pmcp-macros. Natural fit since we're touching lib.rs heavily. | ✓ |
| No, keep the inline `//!` module doc | Leave `src/lib.rs` module doc as-is. Edit it in place to add the feature table. Minimizes churn. | |
| Defer to Phase 68 (General Documentation Polish) | Phase 68 already plans to rewrite lib.rs doctests; could bundle. | |

**User's choice:** Yes, adopt include_str! for pmcp lib.rs (D-04)
**Notes:** Fulfills Phase 66's deferred idea 2. Pattern proven to work on pmcp-macros.

### Q1.3 — What is the include_str! source file?

| Option | Description | Selected |
|--------|-------------|----------|
| Create a new crate-specific doc file | Focused crate usage content, ~150-250 lines. Clean docs.rs, no repo-README baggage. Pulls DOCD-02 into scope. | ✓ |
| Use the existing root `README.md` via `include_str!("../README.md")` | Simplest flip, zero new content. But docs.rs shows 682 lines including GitHub Quality Gate badges, CI chrome. Weak landing. | |
| Strip-and-reuse: edit README.md to be clean for both | Trim repo README to work for both audiences. Higher risk — changes affect both simultaneously. | |

**User's choice:** Create a new crate-specific doc file (D-05, D-06)
**Notes:** Pulls deferred requirement DOCD-02 into Phase 67 scope as a consequence of D-04 — captured as intentional.

### Q1.4 — Where do the existing inline doctest examples go?

| Option | Description | Selected |
|--------|-------------|----------|
| Move into the new crate doc file as `rust,no_run` blocks | Port both Client Example + Server Example verbatim. Zero behavior change for docs.rs readers. Phase 68 refreshes them later. | ✓ |
| Drop from the crate doc, link to examples/ instead | Replace with 1-2 line pointers to `examples/s23_mcp_tool_macro.rs`. Cleaner lib.rs doc but loses copy-paste Quick Start. | |
| Refresh inline to `TypedToolWithOutput` pattern now | Rewrite to current builder patterns. This is PLSH-01 (Phase 68) work — would overlap scope. | |

**User's choice:** Move into the new crate doc file as `rust,no_run` blocks (D-07, D-09)
**Notes:** Explicitly avoid coupling mechanical move to semantic rewrite. "Move verbatim, don't refactor" is captured in specifics.

### Q1.5 — Where should the new crate-specific doc file live?

**First attempt** — I recommended `docs/crate/lib.md`. **Corrected mid-area** after realizing `Cargo.toml:16` excludes `docs/` from the published crate, which would break `include_str!` on crates.io.

| Option (corrected) | Description | Selected |
|--------|-------------|----------|
| `CRATE-README.md` at repo root | Parallel to README.md. Automatically included in published crate. Discoverable. Clear naming. | ✓ |
| `src/lib.md` colocated with lib.rs | Guaranteed in published crate. Closest to code. Minor oddness mixing .md into src/. | |
| Keep at `docs/crate/lib.md` BUT add include path override to Cargo.toml | More flexible long-term but bigger Cargo.toml churn and risk of shipping unintended large docs. | |

**User's choice:** `CRATE-README.md` at repo root (D-05)
**Notes:** Error was caught transparently and corrected within the same discussion area — the initial recommendation would have broken crates.io builds. See CONTEXT.md D-05 for rationale including the rejected alternatives.

---

## Area 2: Explicit docs.rs feature list

### Q2.1 — Feature list composition

| Option | Description | Selected |
|--------|-------------|----------|
| All individual user-facing features (no 'full') | List ~14 features explicitly: websocket, http, streamable-http, sse, http-client, validation, resource-watcher, schema-generation, jwt-auth, oauth, composition, mcp-apps, macros, rayon. Exclude 'full' (meta-feature, redundant). Clean and explicit. | ✓ |
| Just the 'full' feature | Single-entry list. Compact but 'oauth' is NOT in 'full' — missing coverage. Doesn't give readers visible list. | |
| full + oauth + simd | Covers everything with minimum entries. Minimal maintenance but opaque. | |

**User's choice:** All individual user-facing features (no 'full') (D-16)
**Notes:** The final list is 15 (not 14) because of the simd answer in Q2.2.

### Q2.2 — simd and unstable handling

| Option | Description | Selected |
|--------|-------------|----------|
| Exclude both from docs.rs | simd is perf optimization, unstable is experimental. Neither has stable public API worth badging. | |
| Include simd, exclude unstable | If simd gates real public APIs, include it. Still skip unstable. | ✓ |
| Include both | Full transparency. More noise. | |

**User's choice:** Include simd, exclude unstable (D-16, D-17)
**Notes:** Final feature count becomes 15 when simd is added to the 14 from Q2.1.

### Q2.3 — WASM features on docs.rs

**First attempt presented options assumed exclude was recommended.** User response reframed the priorities: "Part of the pragmatic part of the PMCP SDK is the ability to run on ARM64, which is cheaper than the x86_64. Also Cloudflare has the workers framework using WASM, and I don't want to miss the opportunity to compile the Rust to WASM and deploy there."

Reframed with proper options:

| Option (reframed) | Description | Selected |
|--------|-------------|----------|
| `x86_64-linux` + `aarch64-linux`, single feature list | Add aarch64-unknown-linux-gnu as a second target. ARM64 first-class. WASM excluded because wasm needs a different feature set — handle separately. | ✓ |
| `x86_64-linux` + `aarch64-linux` + `wasm32-unknown-unknown` | Three targets, but WASM target will likely fail on native-dep features. Risky for whole docs.rs build. | |
| `x86_64-linux` + `aarch64-linux` + `aarch64-apple-darwin` | All native platforms pmcp runs on. | |
| Defer multi-target to follow-up phase | Most conservative. Only x86_64-linux. | |

**User's choice:** `x86_64-linux` + `aarch64-linux`, single feature list (D-18, D-19)
**Notes:** Key user context: ARM64 = AWS Graviton cost reduction; WASM = Cloudflare Workers. Both are first-class positioning, not afterthoughts.

### Q2.4 — WASM docs.rs coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Defer to a follow-up phase, capture as deferred idea | Per-target feature lists aren't possible in `[package.metadata.docs.rs]` — all targets share one feature list. WASM needs its own design. | ✓ |
| Handle now with conditional `features = [...]` logic | Try to land WASM coverage now. Expands scope significantly, risks missing primary goal. | |

**User's choice:** Defer to a follow-up phase (deferred ideas section of CONTEXT.md)
**Notes:** Not "we don't care about WASM" — it's "WASM docs.rs needs its own problem statement, which this phase can't accommodate."

---

## Area 3: Feature flag table

### Q3.1 — Table placement in CRATE-README.md

| Option | Description | Selected |
|--------|-------------|----------|
| After Quick Start, before deeper topics | intro → Quick Start → `## Cargo Features` → pointers. Matches tokio/axum. | ✓ |
| Very top, before Quick Start | Surface features first. But pushes code blocks below fold. | |
| Dedicated '## Cargo Features' section at bottom | Reference material. Less discoverable. | |

**User's choice:** After Quick Start, before deeper topics (D-14)

### Q3.2 — Table depth/columns

| Option | Description | Selected |
|--------|-------------|----------|
| 3 columns: Feature / Description / Enables | Dependency disclosure via 'Enables' column. Matches tokio. | ✓ |
| 2 columns: Feature / Description | Simpler, less to maintain. No dep disclosure. | |
| 4 columns: Feature / Description / Enables / Example usage | Most thorough. Extra Cargo.toml snippet column. Table too wide for narrow docs.rs pages. | |

**User's choice:** 3 columns (D-11)

### Q3.3 — Meta-features (default, full) in table

| Option | Description | Selected |
|--------|-------------|----------|
| Document both with a note row at top | First row: default = ["logging"]. Second row: full = everything. Then individual features alphabetized. | ✓ |
| Document 'full' only, default implicit | Skip the default row. Shorter. | |
| Individual features only, meta-features in prose above | Paragraph above the table explains meta. Table stays pure. | |

**User's choice:** Document both with a note row at top (D-12, D-13)

---

## Area 4: CI gate

### Q4.1 — make doc-check target and CI integration

| Option | Description | Selected |
|--------|-------------|----------|
| New 'doc-check' target + new CI step, stable toolchain | Add step to existing quality-gate job. No separate CI job/runner. | ✓ |
| New 'doc-check' target + integrate into 'make quality-gate' locally | Chain doc-check from quality-gate. Strongest enforcement but slows every pre-commit by ~30s. | |
| New 'doc-check' target + dedicated CI job, nightly toolchain | Separate CI job, +nightly --cfg docsrs. Highest fidelity but extra CI minutes. | |

**User's choice:** New 'doc-check' target + new CI step, stable toolchain (D-23, D-24, D-26, D-27)
**Notes:** Explicit trade-off captured in D-27: do not chain doc-check into local quality-gate — protects developer iteration speed. CI is the enforcement point.

### Q4.2 — Feature set for doc-check

| Option | Description | Selected |
|--------|-------------|----------|
| Mirror the explicit docs.rs feature list from Cargo.toml | Same features = [...] list as D-16. Single source of truth — if clean locally, clean on docs.rs. | ✓ |
| Use `--all-features` | Simpler command. But checks MORE than docs.rs ships (test-helpers, unstable, example gates). False-positive risk. | |
| Use default features only | Fastest. Misses gated code paths — defeats the phase. | |

**User's choice:** Mirror the explicit docs.rs feature list (D-23, D-25)
**Notes:** D-23's Makefile snippet and D-16's metadata list must stay in sync. Drift between them is a regression.

### Q4.3 — Version bump for docs infrastructure

| Option | Description | Selected |
|--------|-------------|----------|
| No bump — v2.3.0 stays, docs.rs re-renders on next release | Infrastructure-only change, no public API impact. docs.rs re-renders automatically on next unrelated release. No version churn. | ✓ |
| Patch bump: 2.3.0 → 2.3.1 | Ship as visible release. Thin release for doc-only changes. | |
| Defer decision to end of v2.1 milestone | Bump once after Phase 68 as v2.4.0 with combined docs story. | |

**User's choice:** No bump — v2.3.0 stays (D-28)
**Notes:** Consistent with the project's pattern of versioning on API/behavior change, not tooling change.

---

## Claude's Discretion

Areas where user left flexibility (captured in CONTEXT.md D-29 and beyond):
- Exact prose wording of `CRATE-README.md`
- Ordering of rustdoc warning fixes within atomic batches
- Table rendering style (blank-line-separated vs condensed) as long as GFM renders it
- Whether Quick Start imports/types get minor "same-intent" adjustments for renamed types (but NO TypedToolWithOutput refactor)
- Makefile echo/color formatting for doc-check matching existing targets

## Scope Creep Redirected

No scope creep attempts during this discussion. The user stayed within phase boundary. The WASM ARM64 framing was within scope as a target decision for docs.rs (Area 2), not as a new capability request.

## Deferred Ideas Captured

See CONTEXT.md `<deferred>` section for the full list. Highlights:
- WASM docs.rs multi-target coverage (its own phase)
- Workspace-wide rustdoc gate (future phase)
- TypedToolWithOutput refactor of Quick Start code blocks (Phase 68, PLSH-01)
- `*_example` feature-gate cleanup (backlog)

---

*Phase: 67-docs-rs-pipeline-and-feature-flags*
*Discussion date: 2026-04-11*
