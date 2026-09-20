---
phase: 112-version-plumbing-spine
plan: 02
subsystem: api
tags: [mcp-protocol, versioning, era-gating, request-handler-extra, w3c-trace-context, self-reported-identity, additive-semver]

# Dependency graph
requires:
  - phase: 112-01
    provides: "Era enum, ProtocolContext, TraceContext value types (crate::types::protocol)"
provides:
  - "RequestHandlerExtra.protocol_context field (native, additive) + with_protocol_context() builder"
  - "extra.era() -> Option<Era> accessor"
  - "extra.protocol_version() -> Option<&ProtocolVersion> accessor"
  - "extra.client_info() -> Option<&Implementation> accessor (self-reported)"
  - "extra.client_capabilities() -> Option<&ClientCapabilities> accessor (self-reported)"
  - "extra.trace_context() -> Option<TraceContext> accessor over request_meta (VERS-09, no new field)"
affects: [112-04-dispatch-threading, 113-stateless-http, 114-tasks-extension, 116-auth-hardening]

# Tech tracking
tech-stack:
  added: []
  patterns: [additive-field-with-builder-and-accessor on #[non_exhaustive], accessor-over-existing-field (no new field for trace-context), self-reported-not-for-authz rustdoc contract, compiling-RED TDD (stub returns None -> runtime-fail) under a build-must-pass quality gate]

key-files:
  created:
    - .planning/phases/112-version-plumbing-spine/deferred-items.md
  modified:
    - src/server/cancellation.rs

key-decisions:
  - "protocol_context added ONLY to the native field-carrying RequestHandlerExtra (src/server/cancellation.rs); the wasm32 zero-field stub (src/server/mod.rs:162) and the dead orphan src/shared/cancellation.rs left untouched (out of scope)"
  - "trace_context() is a method over the existing request_meta — NO dedicated field added (VERS-09 keys live in the _meta JSON, Pattern 3)"
  - "identity/trace accessors rustdoc'd SELF-REPORTED / not-for-authorization; real identity binds to the OAuth token (Phase 114 / TASK-05); no auth decision made in this phase (threat T-112-02 accept-documented)"
  - "purely additive: no existing field type mutated; new()/with_* callers compile unchanged; wasm32 build stays green (threat T-112-08 mitigate)"
  - "TDD RED committed as a COMPILING skeleton (field + stub accessors returning None -> tests fail at runtime), mirroring Plan-01's realized RED, so every commit compiles under the ZERO-TOLERANCE build gate"

requirements-completed: [VERS-01, VERS-03, VERS-09]

# Metrics
duration: 6min
completed: 2026-07-22
---

# Phase 112 Plan 02: Version Plumbing Spine — Handler Surface Summary

**Handlers can now read the resolved protocol era, self-reported client identity, and raw W3C trace-context off the native `RequestHandlerExtra` via typed accessors (`era()`/`protocol_version()`/`client_info()`/`client_capabilities()`/`trace_context()`) — the additive VERS-01/03/09 handler surface, wasm build stays green.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-07-22T21:24:55Z
- **Completed:** 2026-07-22T21:30:49Z
- **Tasks:** 2 (both TDD RED -> GREEN)
- **Files modified:** 1 (`src/server/cancellation.rs`) + 1 planning doc created

## Accomplishments

- Added the additive `pub protocol_context: Option<ProtocolContext>` field to the `#[non_exhaustive]` native `RequestHandlerExtra`, defaulted to `None` in both `new()` and `Default`, plus a `#[must_use] with_protocol_context(Option<..>)` builder (builder-chain compatible with the existing `with_auth_context`/`with_request_meta` style).
- Added the four typed era/identity accessors — `era() -> Option<Era>`, `protocol_version() -> Option<&ProtocolVersion>`, `client_info() -> Option<&Implementation>`, `client_capabilities() -> Option<&ClientCapabilities>` — each reading through `self.protocol_context.as_ref()`.
- Added `trace_context() -> Option<TraceContext>` (VERS-09) delegating to `TraceContext::from_meta` over the **existing** `request_meta` JSON — **no new struct field** (Pattern 3: trace keys live in `_meta`).
- Rustdoc'd `client_info()`/`client_capabilities()`/`era()`/`protocol_version()` as SELF-REPORTED, informational-only, MUST-NOT-be-an-authorization-anchor (Codex MEDIUM addressed); rustdoc'd `trace_context()` values as raw/bounded/untrusted per the Plan-01 contract.
- Added `protocol_context` to the `Debug` impl.
- Full additive-guarantee held: no existing field type changed, all current `new()`/`with_*` callers compile unchanged, and the `wasm32-unknown-unknown` lib build stays green (native-only change).
- Left the wasm32 zero-field stub and the dead orphan `src/shared/cancellation.rs` untouched (out of scope, per the plan's verified scope correction).

## Task Commits

Each task followed TDD RED -> GREEN, committed atomically:

1. **Task 1 (RED): failing protocol_context era/identity accessor test** — `6a1275d3` (test)
2. **Task 1 (GREEN): wire era/protocol_version/client_info/client_capabilities** — `13faa944` (feat)
3. **Task 2 (RED): failing trace_context() accessor test** — `46cfeefe` (test)
4. **Task 2 (GREEN): surface W3C trace_context over request_meta** — `721cee9f` (feat)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS + deferred-items.md)_

## Files Created/Modified

- `src/server/cancellation.rs` (modified) — `protocol_context` field + `with_protocol_context()` builder + `era()`/`protocol_version()`/`client_info()`/`client_capabilities()`/`trace_context()` accessors + Debug field + 2 unit tests.
- `.planning/phases/112-version-plumbing-spine/deferred-items.md` (created) — logged 2 pre-existing broken intra-doc links in `src/shared/pkce.rs` (out of scope).

## Verification

- `cargo test --lib cancellation::` → 11 passed (incl. new `test_protocol_context_era_and_identity_accessors` and `test_trace_context_from_request_meta`).
- `grep -c 'protocol_context' src/server/cancellation.rs` → 12 (≥3: field, default×2, builder, accessors, Debug, test).
- `src/server/cancellation.rs` contains `pub fn with_protocol_context` and `pub fn era(`; `pub trace_context` field count = 0 (trace_context is a method only).
- `grep -n 'pub auth_context' src/server/cancellation.rs` → unchanged (`Option<crate::server::auth::AuthContext>`); no existing field mutated.
- `git diff --stat src/shared/cancellation.rs` → empty (dead orphan untouched).
- `cargo build --lib` clean; `cargo build --lib --target wasm32-unknown-unknown` finished (exit 0; only pre-existing missing-doc warnings, none from this change).
- `cargo clippy -p pmcp --lib --features full` → zero warnings; `cargo fmt --all -- --check` clean.
- All Plan-02 intra-doc links resolve (the 2 strict-mode broken-link errors are pre-existing in `src/shared/pkce.rs`, logged as deferred).

## Decisions Made

- **Native-only surface:** the field + all accessors land once on `src/server/cancellation.rs` (the only compiled field-carrying `RequestHandlerExtra`). The wasm32 zero-field stub and the dead orphan `src/shared/cancellation.rs` are out of scope this phase.
- **No new field for trace-context:** `trace_context()` reads the existing `request_meta` and delegates to `TraceContext::from_meta`, keeping VERS-09 keys in the `_meta` JSON (Pattern 3) and the additive surface minimal.
- **Self-reported, not-for-authorization:** identity accessors are documented as informational only; the dispatch layer that populates `protocol_context` and the end-to-end handler-visibility test land in Plan 04.
- **Compiling RED:** to respect the ZERO-TOLERANCE build-must-pass gate, each RED commit is a compiling skeleton (accessor stubs return `None`) whose tests fail at runtime, mirroring Plan 01's realized RED rather than committing a non-compiling tree.

## TDD Gate Compliance

Both tasks show the full `test(...)` -> `feat(...)` gate sequence in git log (`6a1275d3` -> `13faa944`, `46cfeefe` -> `721cee9f`). RED was verified failing before each GREEN. No REFACTOR commit needed (accessors are trivial one-liners). RED commits compile (stub bodies return `None`) so no commit leaves the tree unbuildable — a deliberate adaptation to the project's build-passing quality gate, matching the Plan-01 precedent.

## Deviations from Plan

None affecting scope — plan executed as written. One note for the verifier:

1. **Two pre-existing broken intra-doc links surfaced (out of scope):** a strict `-D rustdoc::broken_intra_doc_links` doc build flagged `crate::client::oauth` and `assert_roundtrips_through_client` links in `src/shared/pkce.rs`. Neither is in the Plan-02 change surface (confirmed absent from `cancellation.rs`); all Plan-02 links resolve. Logged in `deferred-items.md`, not fixed (scope boundary).

## Issues Encountered

None.

## Threat Flags

None — no new security surface introduced. The plan's threat register (T-112-02 spoofing via self-reported `client_info`, T-112-08 semver elevation) is addressed: identity accessors are rustdoc'd as informational-only/not-for-authz (accept-documented), and the change is purely additive with the wasm build kept green (mitigate).

## User Setup Required

None.

## Next Phase Readiness

- Handler-facing accessors (`era`/`protocol_version`/`client_info`/`client_capabilities`/`trace_context`) exist on the native `RequestHandlerExtra`, ready for Plan 04 to wire the dispatch layer that populates `protocol_context` (and adds the end-to-end handler-visibility test).
- Additive-only; wasm build green; no blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

All created/modified files exist on disk (112-02-SUMMARY.md, deferred-items.md, src/server/cancellation.rs) and all four task commits (6a1275d3, 13faa944, 46cfeefe, 721cee9f) are present in git history.
