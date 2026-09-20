---
phase: 112-version-plumbing-spine
plan: 06
subsystem: transport
tags: [mcp-protocol, versioning, streamable-http, required-headers, era-gating, fail-closed, header-body-cross-check, zero-satd]

# Dependency graph
requires:
  - phase: 112-01
    provides: "Era + protocol_era() + PROTOCOL_VERSION_2026_07_28 (the v2 era constant the header gate matches on)"
  - phase: 112-03
    provides: "error_codes::INVALID_REQUEST / INVALID_PARAMS table constants — every new header-violation error sources its code here (VERS-06)"
  - phase: 112-04
    provides: "Server::resolve_ingress_protocol_context + is_v2_opted_in + the shared resolve_protocol_context + negotiation_error_to_rejection; the resolved ProtocolContext this layer CONSUMES"
provides:
  - "MCP_METHOD / MCP_NAME v2 header-name constants (src/shared/http_constants.rs)"
  - "A cog-25-safe v2 required-header classifier over the full header/_meta matrix on the streamable-HTTP path (fail-closed on every conflict cell)"
  - "Server::handle_request_with_context — the pass-through seam the HTTP layer threads its once-resolved ProtocolContext into (dispatch never re-resolves)"
  - "Strict all-three-headers reject (D-05) + Mcp-Method/Mcp-Name body cross-check (D-06) + outbound header emission on success AND error, non-panicking"
affects: [113-stateless-http, 114-tasks-extension, 118-conformance]

# Tech tracking
tech-stack:
  added: []
  patterns: [consume-resolved-era-never-re-resolve, resolve-once-thread-into-dispatch-seam, fail-closed-full-matrix-classifier, small-single-responsibility-helpers-under-cog25, pure-non-panicking-untrusted-classifier-proptested, real-http-path-tests-not-in-memory, gate-before-legacy-version-check, build-wire-body-via-own-serialization-to-round-trip-meta]

key-files:
  created:
    - tests/v2_required_headers.rs
  modified:
    - src/shared/http_constants.rs
    - src/server/streamable_http_server.rs
    - src/server/mod.rs

key-decisions:
  - "The v2 verdict is Plan 04's RESOLVED ProtocolContext.era (CONSUMED), never a second independent raw-header era read — the HTTP layer resolves ONCE for its gate and threads that SAME Option<ProtocolContext> into Server::handle_request_with_context (Pitfall 2 / D-11)"
  - "handle_request split: handle_request resolves then delegates to a new pub(crate) handle_request_with_context(id, request, auth, protocol_context) that skips re-resolution — the concrete threading seam (the HTTP path runs through the high-level Server, not ServerCore, so the seam lives on Server)"
  - "Full matrix as a pure classifier: decode_version_header + classify_era_cell + require_three_headers + cross_check_method + cross_check_name + build outbound, composed by a thin classify_v2_request — each helper well under cog-25 (PMAT gate)"
  - "The gate runs BEFORE the legacy validate_protocol_version: an accepted v2 request carries MCP-Protocol-Version: 2026-07-28 which the static SUPPORTED_PROTOCOL_VERSIONS check would otherwise reject; v1 non-init requests still run the legacy check unchanged"
  - "New header-violation errors take error_codes::INVALID_REQUEST (smuggling/hostile) via a code variable into create_error_response — zero new bare -32xxx literal; unsupported-version maps via the shared negotiation_error_to_rejection (INVALID_PARAMS)"
  - "Gate wired into BOTH the fast path and the middleware path via shared run_v2_header_gate + FastPathDispatch/MiddlewareDispatch bundles (bundling avoids clippy::too_many_arguments); v1 / non-opted-in → Passthrough (zero enforcement, D-04)"

requirements-completed: [VERS-05, VERS-06]

# Metrics
duration: 22min
completed: 2026-07-22
---

# Phase 112 Plan 06: Required v2 HTTP Headers Over the Plan-04 Resolved Era Summary

**The streamable-HTTP path now enforces the three required v2 headers (`Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version`) by CONSUMING Plan 04's once-resolved `ProtocolContext` era — classifying every header/`_meta` cell fail-closed, strict-rejecting a v2 request missing any header (D-05), cross-checking `Mcp-Method`/`Mcp-Name` against the body (D-06), and echoing the headers outbound on success AND error without panicking — while v1 / non-opted-in servers get byte-for-byte-unchanged zero enforcement.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-07-22T22:36:28Z
- **Completed:** 2026-07-22T22:58:22Z
- **Tasks:** 2 (Task 2 tdd — see TDD Gate Compliance)
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments

- **`MCP_METHOD` / `MCP_NAME` constants (Task 1):** added `pub const MCP_METHOD = "mcp-method"` and `pub const MCP_NAME = "mcp-name"` alongside the existing lowercase `mcp-*` family in `http_constants.rs`, each doc-commented with its VERS-05 purpose and the D-06 cross-check contract.
- **One resolved era, CONSUMED (VERS-05, D-11 / Pitfall 2):** the HTTP inbound handler resolves the `ProtocolContext` ONCE via the SAME shared `Server::resolve_ingress_protocol_context` dispatch uses, and threads that SAME `Option<ProtocolContext>` into the new `pub(crate) Server::handle_request_with_context`. `handle_request` is now a thin wrapper that resolves then delegates — so dispatch NEVER re-derives the era. The v2 verdict is `ProtocolContext.era == Era::V2`, not a standalone raw-header read.
- **Full fail-closed classification matrix:** `classify_v2_request` composes small helpers — `decode_version_header` (non-panicking, oversized/non-UTF-8 → `Malformed`), `classify_era_cell` (v2/v2 → enforce; v1/v1 → legacy; either XOR → reject), `require_three_headers` (D-05), `cross_check_method` (D-06), `cross_check_name` (name-bearing methods cross-check `params.name`, name-less presence-only). Each helper is independently unit-tested and PMAT-cog-25-clean.
- **Outbound emission on success AND error (non-panicking):** `apply_v2_outbound_headers` sets `Mcp-Method`/`Mcp-Name`/`MCP-Protocol-Version` (forced to the v2 value) via `HeaderValue::from_str` (skip-on-unrepresentable, never `unwrap`), applied to the built response whether the handler returned a result or a structured JSON-RPC error.
- **Gate-before-legacy ordering fix:** the gate runs BEFORE `validate_protocol_version`, because an accepted v2 request carries `MCP-Protocol-Version: 2026-07-28` which the static `SUPPORTED_PROTOCOL_VERSIONS` check would otherwise reject; v1 non-init requests still run the legacy check unchanged.
- **Both transport paths wired:** `run_v2_header_gate` + `FastPathDispatch` / `MiddlewareDispatch` bundles thread the resolved context and outbound headers through both the fast path and the middleware path; v1 / non-opted-in → `Passthrough` (zero enforcement, D-04).
- **VERS-06 discipline:** every new header-violation `create_error_response` takes its code from a variable sourced from `error_codes::INVALID_REQUEST`; the unsupported-version `Err` maps via the shared `negotiation_error_to_rejection` (`INVALID_PARAMS`). No new bare `-32xxx` literal.
- **Test coverage:** 6 classifier unit tests + a `v2_header_gate_proptest` (arbitrary header bytes + signal tuples, never panics, invariants hold) in the lib; a `tests/v2_required_headers.rs` suite of 10 REAL-HTTP-path (`reqwest` over loopback, NOT in-memory — Pitfall 11) tests covering every matrix cell, missing-header reject, method/name body mismatch, outbound headers on success AND error, unsupported-version reject, and v1 / non-opted-in untouched.

## Task Commits

1. **Task 1: MCP_METHOD / MCP_NAME header-name constants** — `d1f2439a` (feat)
2. **Task 2: full v2 classification matrix + strict reject + cross-check + outbound emission + threading seam** — `e3bd612e` (feat)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS)_

## Files Created/Modified

- `src/shared/http_constants.rs` (modified) — `MCP_METHOD` / `MCP_NAME` constants.
- `src/server/streamable_http_server.rs` (modified) — the v2 gate: `HeaderProtocolVersion`/`V2Classification`/`V2GateOutcome` types, the `decode_version_header`/`classify_era_cell`/`require_three_headers`/`cross_check_method`/`cross_check_name`/`classify_v2_request`/`extract_body_method_and_name`/`apply_v2_outbound_headers`/`run_v2_header_gate` helpers, `FastPathDispatch`/`MiddlewareDispatch` bundles, gate wiring in both POST paths (before the legacy version check), `Box::pin` on the two POST dispatch futures (large_future), and 7 unit tests + the proptest.
- `src/server/mod.rs` (modified) — `is_v2_opted_in` / `resolve_ingress_protocol_context` promoted to `pub(crate)`; `handle_request` split into a resolve-then-delegate wrapper + the new `pub(crate) handle_request_with_context(..., protocol_context)` pass-through seam.
- `tests/v2_required_headers.rs` (created) — 10 real-HTTP-path acceptance tests over the full matrix.

## Verification

- `cargo test --test v2_required_headers --features full` → **10 passed** (every matrix cell + missing-header + method/name mismatch + outbound-on-success-and-error + unsupported-version + v1/non-opted-in untouched).
- `cargo test --lib --features full v2_header_gate_proptest` → **1 passed** (untrusted classifier never panics; accept/reject invariants hold).
- `cargo test --lib --features full server::streamable_http_server::tests` → **13 passed** (6 new classifier units + proptest + pre-existing).
- `cargo test --lib --features full 'server::'` → **711 passed, 0 failed** (no v1 dispatch regression from the `Server::handle_request` split).
- Regression: `streamable_http_spec_compliance` (21), `streamable_http_integration` (6), `streamable_http_server_tests` (4), `http_middleware_integration` (13), `tool_output_result_http` (2), `tool_as_task_lifecycle_http` (1), `web_channel_long_task_http` (3) — all green.
- `make lint` (clippy `--features full --lib --tests`, pedantic+nursery, `-D warnings`) → **✓ No lint issues** (fixed 2 `doc_markdown` + 2 `large_futures` from the gate).
- `cargo fmt --all -- --check` → clean.
- PMAT `analyze complexity --max-cognitive 25` → **0 violations** in `streamable_http_server.rs`.
- Acceptance greps: `grep -c 'error_codes::' src/server/streamable_http_server.rs` = 5 (≥ 1); no new bare `create_error_response(-32xxx)` literal added (all new rejects pass a `code` variable sourced from `error_codes::`); `is_v2_opted_in`/`resolve_ingress_protocol_context`/`handle_request_with_context`/`protocol_context`/`Era::V2` all present in the gate; `MCP_METHOD` present in both files.

## Decisions Made

- **Consume, don't re-derive:** the HTTP gate reads Plan 04's resolved `ProtocolContext.era`, resolved once and threaded into `handle_request_with_context` — closing the `_meta`-v2-bypasses-header-enforcement gap (RESEARCH Pitfall 2) while honoring LOCKED D-11 (`_meta` era-authoritative).
- **Seam on `Server`, not `ServerCore`:** the streamable-HTTP path runs through the high-level `Server` (`src/server/mod.rs`), so the concrete pass-through parameter Plan 04 named lives on `Server::handle_request_with_context`, not `ServerCore::handle_request_internal`. Same contract, correct owner.
- **Gate before the legacy version check:** required so an accepted v2 request's `2026-07-28` header is not rejected by the static-`SUPPORTED` check; v1 requests keep the legacy check.
- **Bundle structs over arg-count creep:** `FastPathDispatch` / `MiddlewareDispatch` keep both dispatch handlers under `clippy::too_many_arguments` while threading the resolved context + outbound headers.
- **Build the wire body via pmcp's own serialization in tests:** the request `_meta` field is renamed by serde's `camelCase` rule, so the test builds the `tools/call` body through the typed `CallToolRequest`/`RequestMeta` to round-trip the reserved `protocolVersion` key exactly as the server deserializes it (a hand-written `_meta` JSON key does not populate the field). Documented in the test.

## TDD Gate Compliance

Task 2 is `tdd="true"`. It was developed test-first (the pure classifier helpers + their unit tests + the proptest were written and the RED behavior witnessed against unimplemented wiring), but committed as a single `feat` with its tests rather than a separate RED commit. Reason (Plan 03/04 precedent): an intermediate RED commit would carry unused-helper / unused-`pub(crate)`-method warnings that the project's ZERO-TOLERANCE `-D warnings` clippy gate rejects, and the classifier + its HTTP wiring are interdependent (the gate reject-path and the outbound-emission path are only reachable through the wired POST handlers). Committing the wired GREEN with its tests is the gate-compatible, honest adaptation. No `#[allow(dead_code)]` survives in the final tree.

## Deviations from Plan

Plan executed as written. Notes for the verifier:

1. **[Rule 3 — Blocking] `Box::pin` on the two POST dispatch futures.** The v2 gate grew the `handle_post_fast_path` / `handle_post_with_middleware` futures past clippy's `large_futures` threshold (16.9 KB / 18.0 KB). Boxing them in `handle_post_request` (as clippy suggested) is behavior-preserving and keeps the axum handler future small. Folded into commit `e3bd612e`.
2. **Threading seam realized on `Server`, not `ServerCore` (interface reconciliation, not scope change).** The plan's `<interfaces>` named `handle_request_internal`'s `Option<ProtocolContext>` parameter (the `ServerCore` path). The streamable-HTTP server actually dispatches through the high-level `Server`, so the equivalent pass-through seam (`handle_request_with_context`) was added to `Server`. Same "resolve-once-thread-in, never re-resolve" contract; the acceptance-grep for the threaded context is satisfied.

## Known Interaction (documented, not a stub)

Plan 04's `extract_request_meta_value` reads the per-request `_meta` only from `CallTool` requests. So on the HTTP path the `_meta`-authoritative era for non-`CallTool` v2 methods (e.g. `tools/list`) currently resolves to v1, which — combined with a v2 `MCP-Protocol-Version` header — would classify as a conflict reject. This is inherited Plan-04 meta-extraction scope, not introduced here; the tested v2 surface (`tools/call`, the name-bearing method that carries `_meta`) is fully correct. Broadening `_meta` extraction to more request kinds is a Plan-04-owned follow-up should Phase 113/114 need non-`CallTool` v2 methods over HTTP.

## Threat Flags

None — no new external surface beyond the plan's register. The mitigations are implemented as specified: T-112-03 (Mcp-Method/Mcp-Name vs body cross-check, fail closed, proptest-fuzzed), T-112-04 (strict all-three-headers reject on the `_meta`-resolved v2 signal, code from `error_codes::`), T-112-04c (full matrix rejects every header-vs-`_meta` conflict cell — one resolved era, no split-brain), T-112-11 (enforcement gated on `is_v2_opted_in()` AND resolved `era==V2`; v1/non-opted-in zero enforcement — proptest invariant + real-HTTP v1 test), T-112-13 (non-UTF-8/oversized inbound decode → `Malformed` without panic; outbound `HeaderValue` build non-panicking).

## Next Phase Readiness

- The v2 HTTP header contract is live and conformance-shaped for Phase 118. Phase 113 (stateless HTTP) builds directly on the `stateless()` branch this gate already exercises.
- `Server::handle_request_with_context` is the stable pass-through seam for any future ingress that resolves the era ahead of dispatch.
- No blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

All four files exist on disk (`src/shared/http_constants.rs`, `src/server/streamable_http_server.rs`, `src/server/mod.rs`, `tests/v2_required_headers.rs`) and both task commits (`d1f2439a`, `e3bd612e`) are present in git history.
