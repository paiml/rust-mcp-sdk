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
| G-9 | `RequestHandlerExtra::client_capabilities()` is populated **only** from the v2 per-request `_meta` key. The v1 `initialize` handshake's capabilities land on the server-level `client_capabilities` lock and never reach the handler, so any v1 server-side capability gate reads permanently `false` | `src/types/protocol/context.rs:384-390` vs `src/server/mod.rs:1684` (verified) | — | no |
| G-4 | `completion/complete` is lumped into a catch-all arm returning `json!({})` — no handler seam | `src/server/mod.rs:1897-1901` | 1 | yes |
| G-6 | `_meta` validation answers `-32020` (not `-32602` + HTTP 400), and never rejects a missing `clientCapabilities` | measured by 118-05 on the v2 leg | — | yes |
| G-7 | `server/discover` never emits `supportedVersions` | `grep -rn supportedVersions src/` returns **zero hits** (verified) | — | yes |
| G-8 | header/`_meta` version disagreement answers `-32022`, not `-32020` | measured by 118-05 on the v2 leg | — | yes |

### G-5 — v2 method retirement is narrower than the sunset policy claims

Found by 118-06, then widened and confirmed by 118-05 probing with well-formed params.

`v2_retired_method_of` (`src/server/streamable_http_server.rs:2951-2960`) matches **exactly
two** variants — `resources/subscribe` and `resources/unsubscribe`. Verified. Everything
else remains served on v2: `initialize`, `ping` and `logging/setLevel` all answer normally,
the last of them from the same catch-all arm as G-4 with no era branch anywhere in `src/`.

The suite's 404s for `initialize` and `logging/setLevel` are **parse failures** of its
`_meta`-only params that coincidentally yield `-32601`. Two of its four passes there
therefore pass for the wrong reason — a false green in the *suite*, not in the SDK. The
ERA-12 replacement mechanism (`ignored` → `honored`) does reproduce; only the retirement is
missing.

Also observed by 118-06 and reproduced by 118-05: `negotiate_protocol_version` never
returns `2026-07-28`, so `initialize` answers `protocolVersion: "2025-11-25"` even on the
v2 leg.

## The measurement

Both legs, one process (PID read three times identically), from
`target/debug/examples/s54_v2_dual_conformance`:

| Requirement set | Result | Exit | Scored scenarios failing | Total checks |
|---|---|---|---|---|
| `2025-11-25` | 51 passed, 15 failed | 1 | 11 | 66 |
| `2026-07-28` | 124 passed, 54 failed | 1 | 7 | 178 |

v2 is a large improvement (scored failures fall from 30 checks across 11 scenarios to 17
across 7) but it is **not** green. Zero-check scenarios are `2025-11-25:server-sse-polling`
and `2026-07-28:tasks-status-notifications`; neither is scored, so a scored-only zero-check
list is empty. Cross-era bleed was probed: `server-session-lifecycle` re-run after the v2
run still passes 3/3.

## What the gate does instead

**Correction.** An earlier revision of this file asserted the v2 leg was "measured green".
That was written before 118-05 measured it, and it is wrong — the v2 leg is 124/54 with 7
scored scenarios red. The gate is therefore scoped to *surfaces*, not to a whole leg.

- **Blocking** on the surfaces that genuinely pass — the MRTR surface, which measures
  entirely green (all 14 `input-required-result-*` scenarios pass), plus the era matrix.
- **Not** blocking on either full requirement set, because neither exits 0.
- **No** `--expected-failures`, no allowlist, no baseline-of-known-failures. The phase's
  anti-suppression rule stands and is now doubly binding.
- The structural non-conformance is stated here and in the CI job's own output, so the
  claim the repo makes is exactly the claim it can defend.

G-1 changes the wire format of the public `Content::Resource` type — a breaking change
requiring a semver decision, which is why it is not folded into this phase.

## Adjacent trap (not a conformance gap, but load-bearing)

`RequestHandlerExtra::set_result_meta` is silently dropped on the `ToolOutput::Result`
verbatim path — that arm returns before the dispatcher drains the slot. MRTR signals must
therefore be set directly on `CallToolResult._meta` / `GetPromptResult._meta`. Found by
118-05 while building the v2 fixture surface.

## Follow-up

These nine gaps are the natural scope of a follow-up phase. G-1, G-2, G-4, G-5, G-6, G-7
and G-8 are era-independent and will recur on any era the suite is pointed at. G-3 and G-9
are transport- and era-specific respectively.

**G-3 is localized by measurement** (118-07): the peer handle *is* present under
`Server::run()` and absent only under `StreamableHttpServer`. That narrows the fix to
wiring the seam on the HTTP path rather than changing the handle itself.

118-07 also proved the CONF-03 v1 *mechanism* claim separately, via a v1-only control with
its own two-tool server over `DuplexTransport` + `Server::run()` — so the v1 completion
path is sound; only the HTTP wiring is missing.

G-1 changes the wire format of a public type and needs a semver decision before it can be
scheduled.
