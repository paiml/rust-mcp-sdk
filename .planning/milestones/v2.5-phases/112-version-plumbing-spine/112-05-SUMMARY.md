---
phase: 112-version-plumbing-spine
plan: 05
subsystem: api
tags: [mcp-protocol, versioning, era-gating, server-discover, result-type-envelope, internal-dispatch, additive-semver, zero-satd]

# Dependency graph
requires:
  - phase: 112-03
    provides: "classify_internal_method + InternalClientRequest routing seam; ServerDiscoverRequest; error_codes::METHOD_NOT_FOUND"
  - phase: 112-04
    provides: "resolved ProtocolContext threaded through both native dispatch sites; is_v2_opted_in(); with_extension() extensions map; Era on RequestHandlerExtra/context"
provides:
  - "crate-private parse_request_or_internal seam (IngressRequest::{Public,Internal}) that intercepts server/discover BEFORE the public-enum conversion; public parse_request maps it to -32601 (v1 byte-identical)"
  - "ServerCore::handle_discover — read-only V2-only capability projection (incl. extensions) via isolated discover_result_from_capabilities; ServerCore::dispatch_internal_client_request routing entry"
  - "internal ResponseDisposition (Complete/InputRequired/Task) + ONE shared inject_v2_result_envelope helper both native sites call — v2-object-only resultType + serverInfo, collision-safe, error/notification-excluded"
  - "ServerCore::handle_request and Server::handle_request both inject the v2 envelope at the serialization boundary; v1 output byte-identical"
affects: [112-07-dispatch, 112-08-streamable-http, 113-stateless-http, 114-tasks-extension]

# Tech tracking
tech-stack:
  added: []
  patterns: [method-string-interception-before-public-enum-conversion, crate-private-ingress-enum-no-public-variant, era-gated-serialization-boundary-injection, one-shared-helper-both-native-sites, isolated-wire-shape-conversion-fn, disposition-read-at-serialization-for-113-114-path, golden-wire-fixtures-for-v1-byte-identity]

key-files:
  created: []
  modified:
    - src/shared/protocol_helpers.rs
    - src/types/protocol/mod.rs
    - src/server/core.rs
    - src/server/mod.rs

key-decisions:
  - "server/discover routed via the crate-private parse_request_or_internal seam (IngressRequest::Internal) — NO public ClientRequest/Request variant added (Codex HIGH #4); the public parse_request maps it to -32601 so the v1 wire behavior is byte-identical and the seam is a live production consumer of classify_internal_method (Plan 03's dead_code allow removed)"
  - "handle_discover is a READ-ONLY projection of the already-computed self.capabilities (incl. extensions) via an isolated discover_result_from_capabilities fn (localized final-spec change); era-gated V2-only, v1 -> -32601 (D-10); no is_initialized side effect"
  - "resultType + serverInfo injected v2-only at the serialization boundary via ONE shared inject_v2_result_envelope both core.rs and server/mod.rs call (D-07/D-08); object-results-only, collision-safe (handler-set resultType preserved), error/notification excluded; v1 byte-identical"
  - "internal ResponseDisposition enum establishes the concrete 113/114 selection path: the serialization layer READS a disposition (defaulting Complete) + respects a handler-set inner-result resultType, so a later phase selects input_required/task without touching this envelope code"
  - "live transport wiring of server/discover deferred to Plan 07 (dispatch) / Plan 08 (streamable-http): those plans make the request path carry IngressRequest::Internal to dispatch_internal_client_request; this phase ships the seam + handler + shared envelope, unit-tested through the crate-private entry (scoped #[allow(dead_code)] with // Why: on the two ServerCore routing methods until then)"

requirements-completed: [VERS-03, VERS-04, VERS-07, VERS-08]

# Metrics
duration: 35min
completed: 2026-07-22
---

# Phase 112 Plan 05: server/discover Internal Dispatch + v2 resultType/serverInfo Envelope Summary

**`server/discover` becomes a v2-only read-only capability projection routed through Plan 03's crate-private internal-dispatch seam (v1 gets `-32601`), and every v2 OBJECT result gains a `resultType:"complete"` discriminator plus `serverInfo` — both injected v2-only at the serialization boundary through ONE shared helper both native dispatch sites call, with v1 wire output byte-identical (golden-fixtured).**

## Performance

- **Duration:** ~35 min
- **Completed:** 2026-07-22
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- **server/discover routing seam (VERS-04, Task 1):** added the crate-private `IngressRequest::{Public,Internal}` + `parse_request_or_internal` in `protocol_helpers.rs`. It consults `classify_internal_method` BEFORE the public-enum conversion — the single interception point. The public `parse_request` delegates to it and maps the internal variant to `Error::method_not_found` (`-32601`), so the v1 wire behavior for `server/discover` is byte-identical to before AND `classify_internal_method`/`InternalClientRequest` now have a live production consumer (Plan 03's `#[allow(dead_code)]` on both removed).
- **handle_discover projection (VERS-04, D-09/D-10):** `ServerCore::handle_discover` projects the ALREADY-COMPUTED `self.capabilities` (including the `.with_extension`-populated `extensions` map) read-only via the isolated `discover_result_from_capabilities` conversion fn (localized wire shape). It is era-gated V2-only; a v1 / non-opted-in request receives standard `-32601` (D-10). It triggers NO initialize-style side effect (proven: initialization state unchanged). `dispatch_internal_client_request` is the crate-private routing entry Plans 07/08 will call from the live transport path.
- **v2 resultType + serverInfo envelope (VERS-07, Task 2, D-07/D-08):** an internal `ResponseDisposition` (`Complete`/`InputRequired`/`Task`) plus ONE shared `inject_v2_result_envelope` helper that both `ServerCore::handle_request` (core.rs) and `Server::handle_request` (mod.rs) call at the serialization boundary. The envelope model is pinned: era==V2 + object result → insert `resultType` (from the disposition, `complete` this phase, UNLESS the handler already set one — preserved) and attach `serverInfo`; scalar/array/null and error/notification responses are untouched; v1 emits nothing new (byte-identical).
- **113/114 path established:** the two non-`Complete` dispositions + the "read a threaded disposition, respect a handler-set inner-result `resultType`" mechanism are the concrete path Phases 113 (`input_required`) and 114 (`task`) select at dispatch without touching the envelope code.
- **Golden fixtures + full coverage:** a golden fixture pins the discover wire shape; golden byte-identity assertions pin a v1 success AND a v1 `-32002` error response (frozen pending code survives); v2/v1/no-context envelope cases; twin-site parity on the high-level `Server`; an end-to-end `handle_request` test proving a v2 tool result gains the envelope.

## Task Commits

1. **Task 1 (+ shared envelope helper): server/discover internal dispatch + v2 result envelope at the ServerCore site** — `67b807aa` (feat)
2. **Task 2 twin-site: v2 result envelope parity on the high-level Server dispatch** — `1292c727` (feat)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS)_

> Note on the split: `handle_discover` (Task 1) consumes the shared `inject_v2_result_envelope` (Task 2's core helper) and both live in `core.rs`, so the envelope mechanism + its ServerCore-site wiring shipped in commit 1 alongside the discover work; commit 2 delivers the SECOND native dispatch site (mod.rs) — the twin-site parity that the "one helper, both sites" must-have requires. Each commit compiles and passes tests independently.

## Files Created/Modified

- `src/shared/protocol_helpers.rs` (modified) — `IngressRequest` enum + `parse_request_or_internal` interception seam; `parse_request` now delegates (internal → `-32601`, byte-identical); routing-seam test.
- `src/types/protocol/mod.rs` (modified) — removed the transient `#[allow(dead_code)]` on `classify_internal_method` and `InternalClientRequest` (live production consumer landed in `parse_request_or_internal`).
- `src/server/core.rs` (modified) — `ServerDiscoverResult` (`pub(crate)`, `#[non_exhaustive]`, camelCase) + isolated `discover_result_from_capabilities`; `ResponseDisposition` + shared `inject_v2_result_envelope`; `handle_discover` + `dispatch_internal_client_request`; envelope wiring in `handle_request`; 12 new tests.
- `src/server/mod.rs` (modified) — twin-site envelope injection in `Server::handle_request` via `crate::server::core::inject_v2_result_envelope`; twin-site parity + v1-byte-identity tests.

## Verification

- `cargo test --lib server_discover` → 7 passed (v2 projection incl. extensions; v1 `-32601`; public parse `-32601`; no-side-effect; golden wire shape).
- `cargo test --lib result_type_envelope` → 5 passed (v2 object complete + serverInfo; handler-set disposition preserved; scalar/null/error untouched; v1 byte-identity golden; e2e handle_request).
- `cargo test --lib -- test_server_dispatch parse_request_or_internal` → twin-site parity + v1-no-envelope + routing-seam all green.
- `cargo test --lib -- server::` → 543 passed (no v1 regressions); `protocol::` → 53 passed; `protocol_helpers` → 33 passed.
- `cargo build --lib` clean; `cargo clippy -p pmcp --lib --features full` → zero warnings; `cargo fmt --all -- --check` clean.
- `cargo build --lib --target wasm32-unknown-unknown` → exit 0 (core.rs/mod.rs excluded from wasm32; no new wasm warnings; `wasm_core.rs` untouched).

## Decisions Made

- **No public enum variant for server/discover:** routed via the crate-private `parse_request_or_internal` seam; the public `ClientRequest`/`Request` enums stay byte-identical (Codex HIGH #4 / the milestone's 2.x-minor promise).
- **Public `parse_request` preserves v1:** delegating to the seam and mapping internal → `-32601` keeps the v1 wire byte-identical AND turns `parse_request` into the live production consumer of `classify_internal_method` (Plan 03's allow removed as Plan 03 intended).
- **Injection at the serialization boundary, not on Result structs (D-08):** handlers keep returning today's types (semver-safe); the shared helper reads era + a threaded disposition and respects a handler-set `resultType`.
- **One helper, both sites:** `inject_v2_result_envelope` lives once in `core.rs` and is called from both `ServerCore::handle_request` and `Server::handle_request` — no per-site copy.
- **Wasm honesty:** `core.rs`/`mod.rs` are excluded from wasm32, so v2 (discover/envelope) is unreachable on wasm; `WasmServerCore` is out of scope and untouched — only the routing seam in `protocol_helpers.rs` compiles on wasm (pure, no new warnings).

## Deviations from Plan

Plan executed as written. Notes for the verifier:

1. **[Rule 3 — Blocking] Scoped `#[allow(dead_code)]` on the two ServerCore routing methods.** `handle_discover` and `dispatch_internal_client_request` are consumed by unit tests this phase; their LIVE transport caller (making the request path carry `IngressRequest::Internal`) lands in Plan 07 (dispatch) / Plan 08 (streamable-http), which explicitly modify the transport files this plan does not touch. Both carry a scoped `#[allow(dead_code)]` + `// Why:` note naming Plan 07/08 as the production consumer — mirroring the Plan 03/04 precedent. The routing SEAM itself (`parse_request_or_internal`, `IngressRequest`, `classify_internal_method`) is live (via public `parse_request`) and carries NO allow.
2. **`ResponseDisposition::{InputRequired,Task}` carry a scoped `#[allow(dead_code)]`** — this phase only emits `Complete`; the two non-default variants are the established 113/114 selection path and are exercised by the `as_wire_str` unit test. `// Why:` note names Phases 113/114.
3. **Envelope mechanism co-committed with Task 1** (see Task Commits note): `handle_discover` depends on the shared envelope helper and both live in `core.rs`, so the helper + ServerCore-site wiring shipped in commit 1; the twin-site (mod.rs) parity is commit 2. Each commit compiles and passes independently.

## Semver Gate Note (for Plan 07/08 phase-end gate)

The public `ClientRequest`/`Request` enums are UNCHANGED (server/discover is crate-private). The new `ServerDiscoverResult` is `pub(crate)` (not public API). `parse_request`'s public signature is unchanged. `inject_v2_result_envelope`/`ResponseDisposition`/`IngressRequest`/`parse_request_or_internal` are all `pub(crate)`. The phase-end `cargo semver-checks check-release` MUST still classify MINOR with no `enum_variant_added` against the public request enums.

## Threat Flags

None — no new external security surface. The register is addressed: T-112-10 (discover is a read-only projection of the SAME build-time capabilities — no recompute/drift, no auth-gated data, no side effects), T-112-07 (resultType + serverInfo injected ONLY when era==V2 into object results; v1 byte-identical, golden success + error fixtures guard regression), T-112-04b (server/discover era-gated to V2; V1/non-opted-in → `-32601` through the existing method-not-found path).

## Next Phase Readiness

- The discover handler + `dispatch_internal_client_request` entry + the `parse_request_or_internal` seam are ready for Plan 07 (dispatch) / Plan 08 (streamable-http) to wire the LIVE transport path (carry `IngressRequest::Internal` → `dispatch_internal_client_request`), removing the two scoped allows.
- The `inject_v2_result_envelope` + `ResponseDisposition` mechanism is ready for Phases 113 (`input_required`) and 114 (`task`) to select a non-default disposition at dispatch.
- Additive-only; wasm build green; no blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

All four modified files exist on disk (protocol_helpers.rs, types/protocol/mod.rs, core.rs, mod.rs) and both task commits (67b807aa, 1292c727) are present in git history.
