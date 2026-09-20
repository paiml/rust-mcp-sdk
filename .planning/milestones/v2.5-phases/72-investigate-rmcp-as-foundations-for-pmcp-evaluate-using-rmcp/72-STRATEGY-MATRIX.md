# Phase 72 — Strategy Matrix (RMCP-EVAL-02, reviews-mode revised)

**Baseline pin:** rmcp 1.5.0 / pmcp 2.4.0 (current main).
**Depends on:** 72-INVENTORY.md (Totals table, Mapping section) for LOC columns.
**Scoring is directional; absolute numeric weights are set by 72-CONTEXT.md and the decision rubric (not here).**

**Option-set revision (per 72-REVIEWS.md replan action #7):**
- Option C has been split into **C1 (types-only borrow)** and **C2 (transports-only borrow)** because they are materially different strategies with different migration costs and breaking-change profiles.
- Option E (Fork rmcp) has been DEMOTED from a scored row to a contingency footnote. E is not a primary strategy; it is an insurance policy attached to A or B when rmcp governance fails (T2).

## Options (rows — exactly 5 scored)

- **A. Full adopt** — pmcp re-exports rmcp's model + service + transport; deletes src/types/ (except mcp_apps, ui, auth) and src/shared/{stdio,protocol}.rs; refactors handlers to consume rmcp's RequestContext<RoleServer>.
- **B. Hybrid / wrapper** — pmcp depends on rmcp as a foundation; re-exports rmcp model types with a compatibility facade; keeps pmcp transports where pmcp has a real differentiator (WebSocket, WASM, security-hardened streamable-HTTP, SIMD SSE).
- **C1. Selective borrow — types only** — pmcp takes rmcp's model types (rows 1–10 from 72-INVENTORY.md Mapping), keeps all transports first-party. Maintenance reduction comes from type-layer only; ~4,500 LOC deletable.
- **C2. Selective borrow — transports only** — pmcp takes rmcp's stdio (row 15) and optionally streamable-http client wrapper, keeps all types first-party. Narrow scope, low migration cost.
- **D. Status quo + upstream PRs** — pmcp keeps all ~21,700 LOC first-party. Any rmcp ergonomic gaps pmcp has better answers to (e.g., row 9 ElicitationSchemaBuilder) get upstreamed as PRs. Zero migration cost.

## Criteria (columns — 5)

- **Maintenance reduction** — LOC removed from pmcp's protocol layer + spec-tracking burden transferred to rmcp.
- **Migration cost** — Engineering effort in PoC slices (cite 72-POC-PROPOSAL.md slice sizes) + downstream workspace ripple (mcp-tester, cargo-pmcp, pmcp-code-mode, pmcp-tasks).
- **Breaking-change surface** — Count of pmcp public type identities that change; impact on pmcp v3.0 SemVer planning.
- **Enterprise feature preservation** — Does the option preserve named extension-points for {TypedTool, workflow engine, mcp_apps, auth, middleware, mcp-preview, cargo-pmcp}? (This binds to T9 in the decision rubric.)
- **Spec-upgrade agility** — How quickly does pmcp track an MCP spec rev (weekly/monthly/quarterly) under this option?

## The Matrix

| Option | Maintenance reduction | Migration cost | Breaking-change surface | Enterprise feature preservation | Spec-upgrade agility |
|---|---|---|---|---|---|
| A. Full adopt | HIGH — ~6,400 LOC gone from src/types/ + ~1,200 from src/shared/ (cite 72-INVENTORY.md Totals row 1); spec-tracking burden transfers to rmcp's ~6-week cadence | HIGH — every pmcp import changes; PoC Slice 1 (see 72-POC-PROPOSAL.md) measures the compile-error count on a throwaway spike; downstream ripple into mcp-tester + cargo-pmcp + pmcp-code-mode + pmcp-tasks | HIGH — pmcp 3.0 major; pmcp::types::X changes identity to `pub use rmcp::model::X`; downstream pattern-matches / trait impls break | CONDITIONAL — Apps/workflow/auth are pmcp-exclusive (72-INVENTORY rows 11, 14, 22) and stay first-party; TypedTool touches types at every boundary, risk needs Slice 2 verification. Per T9 (enterprise-feature preservation checklist), A scores 1 only if all 7 extension-points have named preservation mechanisms. | HIGH — rmcp tracks spec ~quarterly; pmcp ships same-day once facade bumps |
| B. Hybrid / wrapper | MEDIUM — ~5,000 LOC gone (types re-exported, stdio delegated); workflow + transports stay first-party per 72-INVENTORY rows 14–23 | MEDIUM — facade construction effort ~1–2 weeks; less ripple than A because facade preserves pmcp::types::X paths; PoC Slice 2 gauges ergonomics friction | MEDIUM — v3.0 still required because identity changes under the facade; some imports stable, some break (custom From impls, enum-variant pattern matches) | FULL (naturally) — pmcp owns MCP Apps, workflow, auth, middleware, all pmcp-exclusive transports; T9 checklist passes by construction because facade preserves the surface | MEDIUM — still need to wrap new rmcp surfaces in the facade on each rmcp minor |
| C1. Selective — types only | LOW-MEDIUM — ~4,500 LOC gone (72-INVENTORY rows 1–10); transports stay first-party | LOW-MEDIUM — facade on types only; narrower than B; less ripple | MEDIUM — type-identity changes still force v3.0 for downstream type-consumers; transport users unaffected | FULL — all pmcp-exclusive subsystems kept; T9 passes | MEDIUM — rmcp type bumps tracked; transports stay under pmcp control |
| C2. Selective — transports only | LOW — ~1,200 LOC gone (stdio + streamable-http client wrapper, 72-INVENTORY row 15 + partial 18) | LOW — narrow scope, minimal ripple | LOW — types unchanged; only transport-level public APIs shift (behind feature flags) | FULL — types stay first-party, all pmcp-exclusive features stay; T9 passes | LOW — types still first-party; spec revs still hit pmcp directly |
| D. Status quo + upstream PRs | ZERO — pmcp keeps all ~21,700 LOC of src/types/ + src/shared/ per 72-INVENTORY Totals | ZERO migration; small ongoing cost of upstream PRs (Phase 69 gaps pmcp wants in rmcp, e.g., ElicitationSchemaBuilder from row 9) | ZERO — no pmcp public API changes | FULL — nothing changes | LOW — pmcp tracks spec manually; rmcp's tracking doesn't help |

## Precedent Crosswalk (from 72-RESEARCH.md §"Comparable Precedents")

| Option | Closest Rust precedent | Lesson |
|---|---|---|
| A | reqwest over hyper | Full wrapper hides foundation; advanced users need escape hatch |
| B | tonic over prost, axum over hyper | Proven layering; version-pin coupling is a real risk (axum 0.7 on hyper 1.0) |
| C1 | serde_derive → serde | Types-only borrow works when the borrowed crate is SemVer-stable |
| C2 | sqlx driver borrow | Transport-only borrow; narrow scope |
| D | (any SDK that kept wire + types first-party) | Zero churn, misses ecosystem goodwill |

## Risk Annotation

Three risks cut across all five scored options and must be carried into 72-DECISION-RUBRIC.md:
1. **rmcp governance SLA is unpublished** (72-RESEARCH.md §"Rmcp Trajectory Risk") — impacts A, B, C1, C2 via **T2** (PR merge latency + issue close time) (NOT D).
2. **pmcp v2.x breaking-change window open/closed** (locked in 72-CONTEXT.md T6) — impacts A, B, C1.
3. **pmcp downstream-user tolerance for v3.0** (locked in 72-CONTEXT.md T7) — impacts A, B, C1.

---

## Contingency (not scored): E. Fork rmcp into vendor/

E is NOT a primary strategy. Per 72-REVIEWS.md replan action #7, E is demoted to contingency only.

**When E is activated:** if rmcp governance responsiveness falls below T2 threshold (median issue close time ≥ 14 days) AFTER A or B has already been selected and migration is underway, pmcp forks rmcp into `crates/vendor/rmcp/` as an escape hatch.

**Cost profile:** maintenance burden doubles (pmcp now owns its own protocol layer AND a vendored rmcp copy). This is acceptable ONLY as insurance, never as the starting position.

**Directional scoring (informational only, not used for the Plan 03 recommendation):**
- Maintenance reduction: NEGATIVE
- Migration cost: HIGH (fork-sync tooling + CI ripple)
- Breaking-change surface: LOW externally, HIGH internally
- Enterprise feature preservation: FULL
- Spec-upgrade agility: LOW (vendored fork diverges)

**Rule for Plan 03:** E MUST NOT appear as the primary `**Recommendation:**` letter. The valid recommendation set is {A, B, C1, C2, D, DEFER}.

---

## Feeds Into

- 72-POC-PROPOSAL.md slice selection: Slice 1 disambiguates A vs B vs C1 (types feasibility); Slice 2 validates B; Slice 3 validates B workflow preservation. Slice 1 is EXECUTED in Plan 02 as a throwaway spike (new in reviews-mode revision).
- 72-DECISION-RUBRIC.md thresholds: maintenance-reduction column feeds T1 (LOC delta) and T8 (historical churn); enterprise-feature-preservation column feeds T9 (extension-point checklist).
- 72-RECOMMENDATION.md: final pick from {A, B, C1, C2, D, DEFER}. E is explicitly NOT a valid pick.
