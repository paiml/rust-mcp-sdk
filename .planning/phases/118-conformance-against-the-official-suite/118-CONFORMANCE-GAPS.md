# Phase 118 — Declared Non-Conformance (D-21)

**Status:** decided
**Decided:** 2026-08-09
**Decision:** Scope the blocking CI gate to the surfaces that genuinely pass; declare the
remaining gaps in writing rather than suppressing them.

## Why this file exists

Plan 118-04 built the dual-version conformance target and measured the official suite
(`@modelcontextprotocol/conformance` 0.2.0-alpha.11) against it for the first time. The
result contradicted the phase's planning premise.

RESEARCH assumed the v1 leg's failures under `s47_v2_stateless_mrtr` were "a missing
fixture, never a protocol defect". With every fixture the suite names supplied, the v1 leg
measures **51 passed / 15 failed**, with **11 of 30 scored scenarios still failing** — none
for fixture reasons. They are structural SDK gaps.

Phase §9 forbids `--expected-failures`, and that prohibition is kept. Nothing here is
suppressed: the gate asserts only what is true, and the untrue part is stated plainly.

## The gaps

Each was verified directly in `src/` by the orchestrator, independently of the executor
agent that first reported it.

| ID | Gap | Evidence | Scenarios blocked | Era-independent |
|----|-----|----------|-------------------|-----------------|
| G-1 | `Content::Resource` serializes **flat** (`uri` / `text` / `mimeType` inline). Spec `EmbeddedResource` nests them under `resource:` | `src/types/content.rs:78-91` vs `schema/vendored/core-2026-07-28/schema.ts:1734-1736` | 3 | yes |
| G-2 | No blob-bearing resource-contents variant; the flat shape carries only `text` | absence in `src/types/content.rs`; suite reports `Content missing uri; Content missing blob field` | 1 | yes |
| G-3 | `notification_tx` and `peer_handle` are set **only** in `Server::run()`, which `StreamableHttpServer` never calls; `PeerHandle` also has no `elicit` | `src/server/mod.rs:1149`, `src/server/mod.rs:1173`; no `.run()` in `src/server/streamable_http_server.rs` | 6 | no |
| G-4 | `completion/complete` is lumped into a catch-all arm returning `json!({})` — no handler seam | `src/server/mod.rs:1897-1901` | 1 | yes |

### G-5 — related, found separately by 118-06

`logging/setLevel` sits in the **same** catch-all arm as `Complete` / `Subscribe` / `Ping`
and answers `Ok(json!({}))` with no era branch anywhere in `src/`. The era probe measures
`method.logging_set_level` as `served`/`served` under both eras — the retirement is
missing, though the ERA-12 replacement mechanism (`ignored` → `honored`) does reproduce.
Candidate real gap against the suite's `server-stateless` scenario.

Also observed by 118-06: `negotiate_protocol_version` never returns `2026-07-28`, so
`initialize` answers `protocolVersion: "2025-11-25"` even on the v2 leg.

## What the gate does instead

- **Blocking** on the v2 leg and the era matrix — both measured green.
- **No** `--expected-failures`, no allowlist, no baseline-of-known-failures. The phase's
  anti-suppression rule stands.
- The v1 leg's structural non-conformance is stated here and in the CI job's own output,
  so the claim the repo makes is exactly the claim it can defend.

G-1 changes the wire format of the public `Content::Resource` type — a breaking change
requiring a semver decision, which is why it is not folded into this phase.

## Follow-up

These five gaps are the natural scope of a follow-up phase. G-1, G-2, G-4 and G-5 are
era-independent and will recur on any era the suite is pointed at; G-3 is specific to the
`StreamableHttpServer` path.
