# Phase 69: rmcp parity research - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-16
**Phase:** 69-rmcp-parity-research-gap-analysis-across-ergonomics-transpor
**Areas discussed:** rmcp version pinning, Evidence depth per surface, Ergonomics scope boundary, Follow-on proposal sizing

---

## Pre-discussion context

Phase was originally added as Phase 68 but collided with an existing unplanned "Phase 68: General Documentation Polish" (PLSH-01/02/03). Resolution: renumbered new phase to 69; narrowed scope (Option B) to remove overlap with Phase 68 by excluding transports, examples polish, and docs coverage from the research surface, keeping only ergonomics + follow-on phase proposals.

## rmcp version pinning

| Option | Description | Selected |
|--------|-------------|----------|
| Latest stable crates.io release | Pin to published rmcp on 2026-04-16 | ✓ |
| Latest main branch commit | Include unreleased work | |
| Both: stable + main | Compare against both | |

**User's choice:** Latest stable crates.io release (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Ignore unreleased | Only published APIs count | ✓ |
| Note it separately | Sidebar only, doesn't affect severity | |
| Full weight | Unreleased counts as published | |

**User's choice:** Ignore unreleased — only published APIs count (Recommended)

---

## Evidence depth per surface

| Option | Description | Selected |
|--------|-------------|----------|
| Source + docs reading only | ~1 day, citations-based | ✓ |
| Hands-on tiny examples per feature | ~3 days, strongest evidence | |
| Hybrid: source-read all, hands-on top 3 | Best depth/cost ratio | |

**User's choice:** Source + docs reading only (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Current main + v2.3.0 public API | Stable baseline | ✓ |
| Only v2.3.0 published | Most conservative | |
| Main including feat/sql-code-mode | Unreleased ergonomics included | |

**User's choice:** Current main branch + v2.3.0 public API (Recommended)

---

## Ergonomics scope boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Tool macros (`#[mcp_tool]` etc.) | MUST — core ergonomics | ✓ |
| Builder APIs (`ServerCoreBuilder`) | MUST — core ergonomics | ✓ |
| Typed tool/prompt wrappers | MUST — core ergonomics | ✓ |
| Handler signatures + state/extra | MUST — core ergonomics | ✓ |

**User's choice:** All four MUSTs selected.

| Option | Description | Selected |
|--------|-------------|----------|
| Client-side ergonomics | pmcp users write both server and client | ✓ |
| Error types + result wrappers | Error ergonomics often a rough edge | ✓ |
| Tower / service integration | Middleware stacking ergonomics | |
| Transport construction ergonomics | Wiring stdio vs streamable-HTTP vs SSE | |

**User's choice:** Client-side ergonomics + Error types/result wrappers. Tower and transport construction intentionally excluded — Tower was addressed in Phase 56; transport construction is deferred unless the matrix surfaces it as a High severity gap.

---

## Follow-on proposal sizing

| Option | Description | Selected |
|--------|-------------|----------|
| 3 proposals | One per high-severity gap area | |
| 2 proposals | Top-2 gaps only | |
| 4 proposals | Broader coverage | |
| Let the gap matrix decide | No fixed count, 2–5 | ✓ |

**User's choice:** Let the gap matrix decide (data-driven over predetermined count).

| Option | Description | Selected |
|--------|-------------|----------|
| Multi-day phases, 3–5 plans each | Phase 66/67 sizing | ✓ |
| Single-day quick wins only | Fast velocity but shallow | |
| Mix: quick wins + bigger bets | Barbell approach | |

**User's choice:** Multi-day phases, 3–5 plans each (Recommended)

| Option | Description | Selected |
|--------|-------------|----------|
| Affects typical user path + clean fix | High severity bar | ✓ |
| Any measurable DX gap | Lower bar, more proposals | |
| Only architecture-deep gaps | Higher bar, fewer proposals | |

**User's choice:** Affects typical user path + clean fix exists (Recommended)

---

## Claude's Discretion

- Table formatting within 69-RESEARCH.md (column widths, wrapping)
- Whether to include an executive summary atop 69-RESEARCH.md
- Surface ordering within the matrix (default: the order listed in D-05..D-10)
- Whether to cite rmcp git blame context beyond file+line (cite when history materially explains the ergonomics choice)

## Deferred Ideas

- Tower / service integration ergonomics — Phase 56 already addressed this
- Transport construction API shape — becomes a proposal only if matrix surfaces it as High severity
- Hands-on tiny-example approach — rejected for Phase 69; possible spike inside a follow-on proposal
- Testing harness / `trybuild` conventions comparison — not in scope
- Client-side subscription/notification patterns — only surface-level client ergonomics in scope
- Performance ergonomics (zero-alloc, allocation profiles) — DX only, not perf
