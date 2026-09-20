---
phase: 112-version-plumbing-spine
plan: 01
subsystem: api
tags: [mcp-protocol, versioning, era-gating, w3c-trace-context, semver, proptest, fuzz]

# Dependency graph
requires:
  - phase: (baseline)
    provides: pmcp 2.17.0 protocol types (ProtocolVersion, Implementation, ClientCapabilities, RequestMeta)
provides:
  - "PROTOCOL_VERSION_2026_07_28 const (opt-in only, not in SUPPORTED set)"
  - "Era enum (V1/V2) + protocol_era() classifier (unknown -> V1 conservative fallback)"
  - "ProtocolContext value type (era + negotiated version + optional client_info/capabilities)"
  - "TraceContext value type (bounded W3C traceparent/tracestate/baggage from _meta)"
  - "Pinned additive-guarantee tooling (cargo-semver-checks 0.49.0, cargo-public-api 0.52.0) + baseline snapshot"
affects: [113-stateless-http, 114-tasks-extension, 115-json-schema, 116-auth-hardening, 117-agents-tester, dispatch-threading]

# Tech tracking
tech-stack:
  added: [cargo-semver-checks@0.49.0 (dev/CI), cargo-public-api@0.52.0 (dev/CI)]
  patterns: [newtype-with-builder value types, non_exhaustive on all new public structs, bounded-ingress validation for untrusted _meta, conservative-unknown-to-V1 era classification, TDD RED/GREEN, proptest + fuzz for untrusted parsers]

key-files:
  created:
    - src/types/protocol/context.rs
    - fuzz/fuzz_targets/trace_context_from_meta.rs
    - .planning/phases/112-version-plumbing-spine/112-01-public-api-baseline.md
  modified:
    - src/types/protocol/version.rs
    - src/types/protocol/mod.rs
    - fuzz/Cargo.toml

key-decisions:
  - "LATEST_PROTOCOL_VERSION stays pinned at 2025-11-25; 2026-07-28 is NOT added to SUPPORTED_PROTOCOL_VERSIONS (reached only via Plan-04 opt-in accept-list) — Pitfall 1 guard"
  - "protocol_era() classifies ONLY exact 2026-07-28 as V2; every other/unknown string conservatively classifies as V1"
  - "TraceContext bounds all W3C values at MAX_TRACE_VALUE_LEN=8192: over-bound traceparent => None, over-bound tracestate/baggage silently dropped (threat T-112-09)"
  - "TraceContext values are RAW/UNVALIDATED/self-reported (only length-bounded), documented as untrusted; no strict W3C syntax parsing"
  - "Semver tooling pinned (not floating latest) and recorded for CI reuse; authoritative check-release MINOR assertion deferred to Plan 07/08 over full phase diff"

patterns-established:
  - "Newtype-with-builder value type (new() + with_* chain, #[non_exhaustive]) mirroring RequestMeta"
  - "Bounded-ingress helper (bounded_trace_value) for untrusted _meta string extraction"
  - "Locking/tripwire tests for version-pinned constants (LATEST pin held; v2 not in legacy set; negotiate never upgrades to v2)"
  - "Untrusted parser gets both a proptest (invariants) and a fuzz target (no-panic + bound)"

requirements-completed: [VERS-01, VERS-02, VERS-09]

# Metrics
duration: 11min
completed: 2026-07-22
---

# Phase 112 Plan 01: Version Plumbing Spine — Foundation Types Summary

**The keystone v2.5 foundation: PROTOCOL_VERSION_2026_07_28 + Era classifier + ProtocolContext/TraceContext value types, with LATEST pinned at 2025-11-25 and pinned semver tooling capturing an additive baseline.**

## Performance

- **Duration:** ~11 min
- **Started:** 2026-07-22T20:57:57Z
- **Completed:** 2026-07-22T21:08:17Z
- **Tasks:** 2
- **Files modified:** 6 (3 created, 3 modified)

## Accomplishments

- Added the opt-in `PROTOCOL_VERSION_2026_07_28` constant, `Era` enum (V1/V2), and `protocol_era()` classifier — the whole v2.5 milestone era-gates off these.
- Kept `LATEST_PROTOCOL_VERSION` pinned at `2025-11-25` and `SUPPORTED_PROTOCOL_VERSIONS` at length 4 (2026-07-28 deliberately excluded) — the single most important backward-compat guard (Pitfall 1). Added tripwire tests proving legacy negotiation never upgrades a client to v2.
- Added `ProtocolContext` (era + negotiated version + optional client_info/capabilities) and `TraceContext` (W3C traceparent/tracestate/baggage from `_meta`) as additive `#[non_exhaustive]` value types, re-exported from `crate::types::protocol`.
- Bounded `TraceContext::from_meta` at `MAX_TRACE_VALUE_LEN=8192` (threat T-112-09): over-bound traceparent → `None`, over-bound tracestate/baggage silently dropped. Documented all values as RAW/UNVALIDATED/self-reported (untrusted).
- Full ALWAYS-testing coverage per CLAUDE.md: unit tests, a proptest over arbitrary untrusted `_meta` JSON, and a registered fuzz target (`trace_context_from_meta`).
- Installed pinned additive-guarantee tooling (`cargo-semver-checks 0.49.0`, `cargo-public-api 0.52.0`) and captured a baseline public-API snapshot showing the Plan-01 delta is entirely additive (minor).

## Task Commits

Each task was committed atomically (Task 2 followed TDD RED → GREEN):

1. **Task 1: 2026-07-28 constant + Era + protocol_era()** - `f521bf29` (feat)
2. **Task 2 (RED): failing tests for ProtocolContext/TraceContext** - `0ddb3290` (test)
3. **Task 2 (GREEN): bounded TraceContext extraction + fuzz target** - `8418ef99` (feat)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS)_

## Files Created/Modified

- `src/types/protocol/version.rs` (modified) - Added `PROTOCOL_VERSION_2026_07_28`, `Era` enum, `protocol_era()`; added tripwire tests. LATEST/SUPPORTED untouched.
- `src/types/protocol/context.rs` (created) - `ProtocolContext` + `TraceContext` value types, `bounded_trace_value` helper, unit tests + proptest.
- `src/types/protocol/mod.rs` (modified) - `pub mod context;` + re-export `ProtocolContext`/`TraceContext`.
- `fuzz/fuzz_targets/trace_context_from_meta.rs` (created) - Untrusted `_meta` → `from_meta` fuzz target (no-panic + bounded-length invariant).
- `fuzz/Cargo.toml` (modified) - Registered the `trace_context_from_meta` bin.
- `.planning/phases/112-version-plumbing-spine/112-01-public-api-baseline.md` (created) - Pinned tool versions, baseline (pmcp 2.17.0), and the additive Plan-01 public-API delta.

## Semver Tooling & Baseline (acceptance-criteria record)

- **Pinned tools (record for CI reuse):**
  - `cargo-semver-checks --version 0.49.0` — `cargo install cargo-semver-checks --version 0.49.0 --locked`
  - `cargo-public-api --version 0.52.0` — `cargo install cargo-public-api --version 0.52.0 --locked`
  - Both `command -v` succeed and are runnable.
- **Semver baseline:** published `pmcp 2.17.0` on crates.io.
- **Public-API surface snapshot:** captured via `cargo public-api --no-default-features -p pmcp` (42,702 public items). The full default-feature rustdoc-JSON build OOM-killed `rustdoc` (SIGKILL) on this machine; the `--no-default-features` surface was captured as the reference. All Plan-01 additions are NEW `pub` symbols with no removals/signature changes → additive **minor**. Detail in `112-01-public-api-baseline.md`.
- **Note:** the authoritative `cargo semver-checks check-release` MINOR assertion runs at phase end (Plan 07/08) over the full diff — NOT this plan.

## Verification

- `cargo test --lib protocol::version` → 10 passed (incl. `latest_version_is_2025_11_25`, `supports_four_versions_including_2024`, new `protocol_era` + negotiate-never-upgrades tests).
- `cargo test --lib protocol::context` → 8 passed (constructor/builders, from_meta full/absent/non-object, over-bound traceparent/tracestate/baggage, proptest).
- `grep -c '2026-07-28' src/types/protocol/version.rs` → 11 (≥1); `2026-07-28` NOT a member of the `SUPPORTED_PROTOCOL_VERSIONS` slice (0 matches inside slice); `SUPPORTED_PROTOCOL_VERSIONS.len() == 4` assertion retained.
- `cargo build --lib` clean; `cargo clippy -p pmcp --lib --features full` zero warnings; `cargo fmt --all -- --check` clean.
- `cargo check --manifest-path fuzz/Cargo.toml --bin trace_context_from_meta` compiles.
- New `protocol_era` doctest passes.

## Decisions Made

- **v2 is opt-in only:** `2026-07-28` is never returned by `negotiate_protocol_version` and never a member of `SUPPORTED_PROTOCOL_VERSIONS`; it is reached only via the Plan-04 accept-list. `LATEST_PROTOCOL_VERSION` stays `2025-11-25`.
- **Conservative era classification:** `protocol_era()` returns `V2` only for the exact `2026-07-28` string; every unknown/malformed/forward-dated input falls back to `V1` so nothing accidentally reaches v2 behavior.
- **Bounded, untrusted trace values:** `MAX_TRACE_VALUE_LEN=8192`; required `traceparent` over the bound rejects the whole context, optional `tracestate`/`baggage` over the bound are dropped. Values documented as raw/self-reported/untrusted; no strict W3C syntax parsing (SDK intentionally surfaces raw values).
- **Fields, not accessor methods, for readable state:** both value types expose `pub` fields per the newtype-with-builder pattern (RequestMeta analog); field-level rustdoc carries the untrusted-data warning.

## Deviations from Plan

None affecting scope — plan executed as written. Two notes for the verifier:

1. **Public-API snapshot method adjusted (environment):** the plan asked for a `cargo public-api` surface snapshot; the full default-feature rustdoc-JSON build was OOM-killed (SIGKILL) on this machine, so the snapshot was captured with `--no-default-features`. The captured surface still confirms all Plan-01 additions are additive. This does not affect the authoritative phase-end (Plan 07/08) gate.
2. **Pre-existing `TraceContext` name (DX note, not a break):** a different `TraceContext` already exists at `pmcp::server::observability::TraceContext` (internal span tracking: `trace_id`/`span_id`/`depth`). The new `pmcp::types::protocol::TraceContext` (W3C header passthrough) is a distinct type in a distinct module, named per the Plan-01 spec. No path collision; downstream glob-imports of both would need disambiguation. Flagged for the phase verifier.

## Issues Encountered

- `cargo public-api` default-feature rustdoc-JSON build OOM-killed `rustdoc`. Resolved by capturing the `--no-default-features` surface as the baseline reference (tool install + runnability still proven).

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: untrusted-parser | src/types/protocol/context.rs | `TraceContext::from_meta` parses untrusted client `_meta` JSON; mitigated via `MAX_TRACE_VALUE_LEN` bound + proptest + fuzz (T-112-09). Raw values documented as untrusted. |

## User Setup Required

None — no external service configuration required. (Dev/CI tool pins recorded above for CI reuse.)

## Next Phase Readiness

- `Era`, `protocol_era()`, `PROTOCOL_VERSION_2026_07_28`, `ProtocolContext`, `TraceContext` all exist and are re-exported from `crate::types::protocol` — ready for the dispatch-threading plans (Plan 02+) and the parallel phases (113/114/115/116) to era-gate off.
- Semver tooling pinned + baseline captured for the phase-end additive-guarantee gate.
- No blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

All created files exist on disk (context.rs, trace_context_from_meta.rs, baseline.md, SUMMARY.md) and all three task commits (f521bf29, 0ddb3290, 8418ef99) are present in git history.
