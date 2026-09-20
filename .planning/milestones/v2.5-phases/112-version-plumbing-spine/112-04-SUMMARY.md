---
phase: 112-version-plumbing-spine
plan: 04
subsystem: api
tags: [mcp-protocol, versioning, era-gating, opt-in-accept-list, dispatch-threading, ingress-resolution, extensions, additive-semver]

# Dependency graph
requires:
  - phase: 112-01
    provides: "Era + protocol_era() + PROTOCOL_VERSION_2026_07_28 + ProtocolContext + SUPPORTED_PROTOCOL_VERSIONS"
  - phase: 112-02
    provides: "RequestHandlerExtra.protocol_context field + with_protocol_context() + era()/trace_context() accessors"
  - phase: 112-03
    provides: "error_codes::INVALID_PARAMS table constant for structured negotiation rejections"
provides:
  - "ServerCoreBuilder.with_supported_protocol_versions() + ServerBuilder.with_supported_protocol_versions() v2 opt-in accept-list (v1-only default, dual, v2-only, empty->v1-fallback)"
  - "ServerCoreBuilder.with_extension() reverse-DNS extensions-map convenience (VERS-08)"
  - "Shared cfg-agnostic resolve_protocol_context(accept_list, meta) + ProtocolNegotiationError — the single source of era resolution"
  - "ServerCore/Server carry the accept-list; is_v2_opted_in() 'run era-detection at all' gate; ProtocolContext resolved once at ingress and threaded at BOTH native dispatch sites"
  - "handle_request_internal gains an Option<ProtocolContext> parameter (HTTP layer passes its already-resolved context in — Plan 06 consumes, never re-resolves)"
affects: [112-06-streamable-http, 113-stateless-http, 114-tasks-extension, 116-auth-hardening, 117-agents-tester]

# Tech tracking
tech-stack:
  added: []
  patterns: [opt-in-accept-list-not-boolean, one-shared-cfg-agnostic-resolver-called-from-both-native-sites, resolve-once-at-ingress-thread-explicit-parameter, fail-closed-on-malformed-reserved-meta, unknown-extension-keys-ignored, per-request-signal-authoritative-over-session, compiling-RED-then-GREEN-for-the-behavioral-resolver]

key-files:
  created: []
  modified:
    - src/types/protocol/context.rs
    - src/server/builder.rs
    - src/server/core.rs
    - src/server/mod.rs

key-decisions:
  - "v2 opt-in is an explicit ProtocolVersion accept-list (D-02), not a boolean/max-version knob: v1-only (default), dual, and v2-only are all one API; empty list falls back to v1-only (never all-reject)"
  - "ONE shared cfg-agnostic resolve_protocol_context lives in types/protocol/context.rs and is called from BOTH native dispatch sites (core.rs + server/mod.rs); it compiles on wasm32 with no wasm caller so the wasm build stays green (Codex MEDIUM 'wasm parity asserted more than designed' resolved honestly — WasmServerCore is out of scope)"
  - "Genuine accept-list enforcement + one authoritative ProtocolContext (Codex HIGH #1/#2): unsupported per-request version -> UnsupportedVersion; v2-only + no v2 signal -> UnsupportedVersion(\"\"); absent signal -> first v1 in the list"
  - "Malformed RESERVED _meta keys (non-string protocolVersion, non-deserializable clientInfo/clientCapabilities, non-object _meta) fail closed as typed MalformedMeta -> structured INVALID_PARAMS; unknown/unrelated extension keys are IGNORED (Phase-109 flatten passthrough)"
  - "resolved once at ingress (opted-in only, D-04) and threaded via a new handle_request_internal Option<ProtocolContext> parameter; core.rs native ingress resolves and passes it; the HTTP layer (Plan 06) resolves once for its header gate and passes the SAME value in — pass-through, never re-derived (D-11 _meta-authoritative, transport-agnostic)"
  - "A non-opted-in server runs ZERO era-detection (is_v2_opted_in()==false short-circuits to Ok(None)); its v1 request path is byte-for-byte unchanged"

requirements-completed: [VERS-01, VERS-02, VERS-03, VERS-08]

# Metrics
duration: 30min
completed: 2026-07-22
---

# Phase 112 Plan 04: Version Plumbing Spine — Ingress Resolution & Dispatch Threading Summary

**The load-bearing spine: a v2 opt-in accept-list on both builders, ONE shared cfg-agnostic resolver that resolves a per-request `ProtocolContext` ONCE at ingress (opted-in only) and threads it through BOTH native dispatch sites so every handler reads the negotiated era + self-reported identity + W3C trace-context — with malformed reserved `_meta` failing closed and non-opted-in servers byte-for-byte unchanged.**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-07-22T21:45:14Z
- **Completed:** 2026-07-22T22:15:08Z
- **Tasks:** 2 (Task 2 followed TDD RED -> GREEN)
- **Files modified:** 4

## Accomplishments

- **v2 opt-in accept-list (VERS-01/02, D-02/D-04):** added `.with_supported_protocol_versions()` to BOTH `ServerCoreBuilder` (builder.rs) and the high-level `ServerBuilder` (mod.rs). The default (unset) is v1-only — the accept-list EXCLUDES `2026-07-28`; dual and v2-only are expressible with the same API; an explicitly-empty list falls back to the v1-only default (never an all-reject server). Stored on `ServerCore`/`Server` with a slice accessor + an `is_v2_opted_in()` "run era-detection at all" gate.
- **`.with_extension()` (VERS-08):** reverse-DNS-keyed convenience populating `ServerCapabilities.extensions` without changing the type.
- **One shared cfg-agnostic resolver (VERS-01):** `resolve_protocol_context(accept_list, meta) -> Result<Option<ProtocolContext>, ProtocolNegotiationError>` in `types/protocol/context.rs` — the single source of era resolution. It enforces the accept-list, classifies the era via `protocol_era`, parses the reserved `_meta` keys (`protocolVersion`/`clientInfo`/`clientCapabilities`), and is PURE/deterministic. It compiles on wasm32 with no wasm caller, keeping that build green.
- **Resolved once at ingress + threaded at BOTH native dispatch sites (VERS-01/03, Pitfall 3):** `core.rs`'s `ProtocolHandler::handle_request` and `mod.rs`'s `Server::handle_request` each resolve once (opted-in only) via the SAME shared resolver, map `Err` to a structured `INVALID_PARAMS` rejection, and thread the resulting `Option<ProtocolContext>` down to the `RequestHandlerExtra` chain via `.with_protocol_context(...)`. `handle_request_internal` gained an explicit `Option<ProtocolContext>` parameter so the HTTP layer (Plan 06) can pass its already-resolved context in without re-deriving.
- **Fail-closed on corruption, ignore unknowns (Codex MEDIUM T-112-12):** malformed reserved keys (non-string `protocolVersion`, non-deserializable `clientInfo`/`clientCapabilities`, non-object `_meta`) return typed `MalformedMeta`; unrelated extension keys pass through untouched.
- **Non-opted-in servers unchanged (D-04):** `is_v2_opted_in()==false` short-circuits to `Ok(None)` — zero era-detection, byte-for-byte v1 path.
- **Full test coverage:** resolver unit tests for every behavior bullet; an END-TO-END handler-visibility test dispatching a real `tools/call` carrying v2 `_meta` + `traceparent` and asserting the handler reads `era()==Some(V2)` AND `trace_context()`; a cross-site parity test proving core.rs and mod.rs resolve the same era; non-opted-in -> None on both sites; an unsupported-version rejection test.

## Task Commits

1. **Task 1: v2 opt-in accept-list builder + v1-only default + with_extension** — `fadc46b2` (feat)
2. **Task 2 (RED): failing resolver tests for resolve_protocol_context** — `0bc71a8f` (test)
3. **Task 2 (GREEN): resolve ProtocolContext once at ingress + thread both native sites** — `3f3f7dba` (feat)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS)_

## Files Created/Modified

- `src/types/protocol/context.rs` (modified) — `default_accept_list()` helper, reserved `_meta` key consts, `ProtocolNegotiationError`, the shared `resolve_protocol_context()` + `resolve_negotiated_version`/`parse_reserved_object` helpers, and 8 resolver unit tests.
- `src/server/builder.rs` (modified) — `supported_protocol_versions` field/default, `.with_supported_protocol_versions()`, `.with_extension()`, build() threading, and 5 builder tests.
- `src/server/core.rs` (modified) — `ServerCore.supported_protocol_versions` field + slice accessor + `is_v2_opted_in()`, `resolve_ingress_protocol_context()`, the `extract_request_meta_value`/`negotiation_error_to_rejection` `pub(crate)` helpers (shared with mod.rs), the new `Option<ProtocolContext>` parameter on `handle_request_internal`/`handle_call_tool`, `.with_protocol_context(...)` in the extra chain, and 3 e2e dispatch tests.
- `src/server/mod.rs` (modified) — twin wiring on the high-level `Server`: `supported_protocol_versions` field on `Server` + `ServerBuilder` + `.with_supported_protocol_versions()` setter, `is_v2_opted_in()`/`resolve_ingress_protocol_context()`, ingress resolution threaded through `handle_client_request` -> `process_client_request` -> `handle_call_tool`, `.with_protocol_context(...)` in the extra chain, and 2 cross-site parity tests.

## Verification

- `cargo test --lib protocol::context` -> 15 passed (resolver behavior bullets + Plan-01 context tests).
- `cargo test --lib server::core` -> 37 passed (incl. e2e handler-visibility, non-opted-in None, unsupported-version rejection).
- `cargo test --lib server::builder` -> 24 passed (default/dual/v2-only/empty/with_extension).
- `cargo test --lib test_server_dispatch` -> 2 passed (mod.rs cross-site parity + non-opted-in None).
- `cargo test --lib --features full` -> **1194 passed, 0 failed** (no v1 regressions).
- `cargo build --lib` clean; `cargo clippy -p pmcp --lib --features full` -> zero warnings; `cargo fmt --all -- --check` clean.
- `cargo build --lib --target wasm32-unknown-unknown` -> exit 0 (native-only additions don't break wasm; cfg-agnostic resolver compiles with no wasm caller).
- Acceptance greps: `resolve_protocol_context` in core.rs (>=1), `with_protocol_context` in core.rs (1) AND mod.rs (1); `git diff --stat src/server/wasm_core.rs src/shared/cancellation.rs` empty (both untouched / out of scope).
- `make quality-gate`: formatting + lint (root pmcp, pedantic+nursery, `--features full`) + examples all PASS. (The `test-fuzz` sub-step fails to build for ALL fuzz targets — including pre-existing ones like `auth_flows`/`jsonrpc_handling` — with `failed to run rustc to learn about target-specific information`, a pre-existing local ASAN/sancov nightly-toolchain environment issue unrelated to this plan; see Deferred Issues.)

## Decisions Made

- **Accept-list, not a boolean:** v2 opt-in is an explicit `ProtocolVersion` list so v1-only/dual/v2-only are one API and Phase-117 severability is a natural consequence (D-02).
- **One resolver, called from both sites:** the resolver is cfg-agnostic and lives in `context.rs`; `core.rs` and `mod.rs` both call it and thread the result — no second/hand-copied resolver (Pitfall 3). Shared `extract_request_meta_value`/`negotiation_error_to_rejection` `pub(crate)` helpers keep even the meta-extraction + rejection-mapping single-sourced.
- **Threading mechanism named concretely:** `handle_request_internal` takes an `Option<ProtocolContext>` alongside `auth_context`; the native ingress resolves and passes it; Plan 06's HTTP layer resolves once for its header gate and passes the SAME value in — core.rs's use is a pass-through (D-11 honored, one authoritative result).
- **Fail closed on reserved-key corruption; ignore unknowns:** a corrupt reserved key would make handler-visible context disagree with the wire, so it is a typed error; unknown extension keys are passthrough per the Phase-109 flatten map.
- **wasm honesty:** this phase delivers NO wasm server functionality; `WasmServerCore` is out of scope and untouched. The only wasm obligation — keep the build green — is met by the cfg-agnostic resolver.

## TDD Gate Compliance

- **Task 2 (the behavioral resolver)** shows the full `test(...)` -> `feat(...)` gate in git log (`0bc71a8f` RED -> `3f3f7dba` GREEN). RED was verified failing (7 resolver tests fail against the `Ok(None)` stub) before GREEN.
- **Task 1 (additive builder plumbing)** was committed as a single `feat` with its tests: RED was witnessed live (3 builder tests failed before the setter/threading were wired), but an intermediate RED commit would carry an unused-field/unused-method warning that the project's ZERO-TOLERANCE `-D warnings` gate rejects (the wiring is unused until `build()` threads it). Committing the wired GREEN with its tests is the gate-compatible, honest adaptation — matching the Plan-01/02 compiling-RED precedent where a clean intermediate build is achievable, and collapsing where it is not.

## Deviations from Plan

Plan executed as written. Notes for the verifier:

1. **[Rule 3 — Blocking] Transient `#[allow(dead_code)]` in Task 1, removed in Task 2.** `is_v2_opted_in()`/`supported_protocol_versions()` are consumed by the Task-2 ingress; the Task-1 commit carried a scoped `#[allow(dead_code)]` + `// Why:` note (Plan-03 precedent), REMOVED in the Task-2 GREEN commit once the ingress caller landed. No `#[allow]` survives in the final tree.
2. **High-level `Server` accept-list added (twin-site fidelity).** To wire the mod.rs dispatch site as a genuine twin (not a stub), `supported_protocol_versions` + `.with_supported_protocol_versions()` were added to `Server`/`ServerBuilder` as well — beyond the plan's Task-1 `ServerCoreBuilder`-only text, but required by the plan's "threaded at BOTH native dispatch sites" must-have and the parity test. Both builders share the one resolver and the one `default_accept_list()`.

## Deferred Issues

- **cargo-fuzz build broken in this local environment (pre-existing, out of scope).** `make test-fuzz` / the quality-gate fuzz sub-step fails for ALL 15 fuzz targets (including pre-existing `auth_flows`, `jsonrpc_handling`, `pkce_helper`, etc.) with `Error: failed to build fuzz script: ... failed to run rustc to learn about target-specific information` — an ASAN/sancov nightly-toolchain environment problem, NOT introduced by this plan (the Plan-01 `trace_context_from_meta` fuzz target was already registered and compiles under `cargo check`). No new fuzz target was required by this plan. Logged here rather than fixed (scope boundary).

## Threat Flags

None — no new external security surface. The plan's register is addressed: T-112-01 (non-opted-in servers run zero era-detection), T-112-03b (one shared resolver, threaded, parity-tested; Plan 06 consumes not re-resolves), T-112-12 (malformed reserved `_meta` -> typed error -> structured rejection; never panics), T-112-05 (ProtocolContext is per-request, never keyed by session id). Client identity carried in `ProtocolContext` remains rustdoc'd self-reported/not-for-authz (Plan 02).

## Next Phase Readiness

- The spine is live: v2 opt-in + one-shot ingress resolution + dual-site threading are in place, so Plan 05 (server/discover dispatch), Plan 06 (streamable-HTTP header gate CONSUMING this resolved context), and the parallel phases (113/114/116) can era-gate off `extra.era()` / the threaded `ProtocolContext`.
- Plan 06 note: `handle_request_internal` already takes the `Option<ProtocolContext>` parameter — the HTTP layer resolves once for its header gate and passes THAT value in; do NOT add a second resolver.
- Additive-only; wasm build green; no blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

All modified files exist on disk (context.rs, builder.rs, core.rs, mod.rs) and all three task commits (fadc46b2, 0bc71a8f, 3f3f7dba) are present in git history.
