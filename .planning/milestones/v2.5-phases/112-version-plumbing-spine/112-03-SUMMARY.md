---
phase: 112-version-plumbing-spine
plan: 03
subsystem: api
tags: [mcp-protocol, error-codes, versioning, semver, dispatch, server-discover, zero-satd]

# Dependency graph
requires:
  - phase: 112-01
    provides: "Era enum + protocol_era() + PROTOCOL_VERSION_2026_07_28 + pinned semver tooling (the version-gating substrate this table plugs into)"
provides:
  - "src/types/protocol::error_codes — single source of truth for every protocol error code (standard JSON-RPC + pmcp -320xx family + frozen V1_TASK_PENDING; v2 values structurally omitted)"
  - "error::ErrorCode's 11 associated consts delegate to error_codes:: (dominant ~210-site surface now table-sourced)"
  - "public ServerDiscoverRequest struct (#[non_exhaustive], VERS-04)"
  - "crate-private InternalClientRequest + classify_internal_method('server/discover') routing seam (public ClientRequest/Request UNCHANGED)"
affects: [112-05-dispatch-wiring, 112-07-dispatch, 112-08-streamable-http, 114-tasks-extension]

# Tech tracking
tech-stack:
  added: []
  patterns: [centralized-const-table-as-source-of-truth, delegate-associated-consts-to-table, per-name-consistency-test-guard, crate-private-internal-dispatch-to-avoid-public-enum-variant, structural-omission-instead-of-SATD-placeholder, why-annotated-scoped-dead_code-allow]

key-files:
  created:
    - src/types/protocol/error_codes.rs
  modified:
    - src/types/protocol/mod.rs
    - src/error/mod.rs

key-decisions:
  - "error::ErrorCode's 11 consts DELEGATE to error_codes:: (Self(error_codes::NAME)) — centralizes all ~210 ErrorCode:: call sites in one change without editing any of them; names/values byte-identical so semver stays minor"
  - "Both -32002 meanings preserved by name: V1_TASK_PENDING (frozen task-pending) and UNSUPPORTED_CAPABILITY (capability) share the number, never reconciled"
  - "v2 semantic error codes are STRUCTURALLY OMITTED (absent, not stubbed); no SATD token anywhere; VERS-06 finalization tracked in planning (112-VALIDATION.md), not in source"
  - "server/discover routed via crate-private InternalClientRequest + classify_internal_method BEFORE public-enum conversion; NO public ClientRequest/Request variant added (Codex HIGH #4 — enum_variant_added would break downstream exhaustive matches despite being 'minor')"
  - "ProtocolErrorCode C-style enum discriminants left UNCHANGED this phase (semver risk); the per-name consistency test is the binding guard that the enum and table agree — audit note below captures the follow-up"

patterns-established:
  - "Centralized const table + associated-const delegation as the single source of truth for a wire contract, guarded by a per-name value-equality test"
  - "Crate-private internal dispatch enum + method-string classifier to make a method routable without touching a public exhaustive enum"
  - "Deferred values expressed by structural omission + non-SATD doc, never by a placeholder/TODO"

requirements-completed: [VERS-06, VERS-04]

# Metrics
duration: 5min
completed: 2026-07-22
---

# Phase 112 Plan 03: Centralized Error-Code Table + server/discover Internal Dispatch Summary

**One centralized version-gated `error_codes` table becomes the real source of truth for the dominant `error::ErrorCode` surface (~210 sites, via per-name delegation), and `server/discover` is made routable through a crate-private internal dispatch seam with the public `ClientRequest`/`Request` enums left byte-identical.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-07-22T21:34:15Z
- **Completed:** 2026-07-22T21:38:54Z
- **Tasks:** 2
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments

- Created `src/types/protocol/error_codes.rs`: a module of `pub const i32` values (NOT new `ProtocolErrorCode` enum variants, avoiding the Pitfall-4 semver risk) covering the standard JSON-RPC codes, the pmcp `-320xx` family, and the frozen `V1_TASK_PENDING = -32002`.
- Made `error::ErrorCode`'s 11 associated consts DELEGATE to the table (`Self(error_codes::NAME)`), so the dominant ~210-call-site surface is now sourced from one place — without editing any of those 210 sites and without changing any name, value, `as_i32()`, or public signature.
- Preserved BOTH distinct `-32002` meanings by name: `V1_TASK_PENDING` (frozen task-pending) and `UNSUPPORTED_CAPABILITY` (capability). The numeric collision is documented as intentional, never reconciled. The frozen `pending_tasks_result_preserves_minus_32002` test is untouched and green (`git diff` clean).
- Represented deferred v2 error-code values by STRUCTURAL OMISSION with a plain non-SATD doc comment — zero `TODO`/`FIXME`/`XXX` tokens, so PMAT's zero-SATD gate passes. VERS-06 v2 finalization is tracked in the planning system, not in source.
- Added a public `ServerDiscoverRequest` struct (`#[non_exhaustive]`, empty-but-extensible) and a crate-private `InternalClientRequest` + `classify_internal_method("server/discover")` routing seam. The public exhaustive `ClientRequest`/`Request` enums are byte-identical — no downstream exhaustive-match break (Codex HIGH #4). Plan 05 wires the classifier into the request path.
- Full consistency-test coverage: per-name `error::ErrorCode::FOO.as_i32() == error_codes::FOO`, both `-32002` assertions, standard consts == `ProtocolErrorCode` enum discriminants, and the classifier `Some`/`None` behavior.

## Task Commits

Each task committed atomically:

1. **Task 1: centralize error codes + delegate error::ErrorCode surface** — `ab5ed16b` (feat)
2. **Task 2: route server/discover via crate-private internal dispatch** — `a8a66a75` (feat)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS)_

## Files Created/Modified

- `src/types/protocol/error_codes.rs` (created) — centralized version-gated error-code constants (standard + pmcp -320xx + frozen `V1_TASK_PENDING`; v2 omitted, zero SATD) + consistency tests.
- `src/types/protocol/mod.rs` (modified) — `pub mod error_codes;`; `ServerDiscoverRequest` struct; crate-private `InternalClientRequest` + `classify_internal_method`; round-trip/classifier tests.
- `src/error/mod.rs` (modified) — the 11 `ErrorCode` consts now delegate to `crate::types::protocol::error_codes::*` (names/values unchanged).

## Verification

- `cargo test --lib protocol::error_codes` → 4 passed (both -32002 assertions, standard-vs-`ProtocolErrorCode` agreement, per-name `error::ErrorCode`==`error_codes` delegation, capability-≠-task-pending-by-name).
- `cargo test --lib protocol::` → 46 passed (incl. `server_discover_request_round_trips`, `classify_internal_method_routes_server_discover`).
- `cargo test --lib pending_tasks_result_preserves_minus_32002` → 1 passed; `git diff` clean on `src/server/task_dispatch_tests.rs`.
- `cargo build --lib` clean — all ~210 `ErrorCode::` call sites compile unchanged through the delegation.
- `cargo clippy -p pmcp --lib --features full` → zero warnings; `cargo fmt --all -- --check` clean.
- Acceptance greps: no `V2_` const; no SATD token in `error_codes.rs`; `grep -c 'crate::types::protocol::error_codes::' src/error/mod.rs` == 11; `grep -cE '= Self\(-32[0-9]{3}\)' src/error/mod.rs` == 0; `pub mod error_codes` present; no public `ClientRequest::ServerDiscover`/`Request::` variant (word-boundary check confirms the only `ServerDiscover` enum refs are `InternalClientRequest::`).

## Decisions Made

- **Delegation over duplication:** `error::ErrorCode` consts become `Self(error_codes::NAME)`. This makes `error_codes::` the literal single source of truth for the 210-site surface in one change; the per-name consistency test makes any future drift on either side fail CI.
- **-32002 collision preserved:** two distinct meanings, two names, same number — documented as intentional, not a bug.
- **Structural omission, not SATD:** v2 values absent + non-SATD doc; finalization tracked in the planning system so the zero-SATD gate passes.
- **No public enum variant for server/discover:** routed crate-privately (invisible to `cargo-semver-checks`/`cargo-public-api`); the public request enums stay byte-identical to keep the milestone's hard 2.x-minor promise.

## Deviations from Plan

None affecting scope — plan executed as written. One implementation note:

1. **[Rule 3 — Blocking] `dead_code` allows on the internal dispatch seam.** `InternalClientRequest` and `classify_internal_method` are consumed only by unit tests until Plan 05 wires them into the server request path, so the non-test build reported them as unused — which the zero-tolerance `-D warnings` clippy gate would reject. Added `#[allow(dead_code)]` on both with a `// Why:` note stating Plan 05 is the production consumer and the allow is removed then. Files: `src/types/protocol/mod.rs`. Folded into commit `a8a66a75`.

## Audit Note (Codex MEDIUM — surviving ProtocolErrorCode parallel definition)

`ProtocolErrorCode` (the C-style enum at `src/types/protocol/mod.rs`) still carries its own discriminant literals (`-32600/-32601/-32602/-32603`), a parallel numeric definition of the four standard codes. Its public discriminant values were deliberately **not** changed this phase (redefining `enum Variant = <expr>` discriminants risks a semver/const-eval surprise and the enum has near-zero external refs). The binding guard that the two representations agree is the `standard_codes_match_protocol_error_code_enum` test in `error_codes.rs`. **Follow-up for a later phase:** either deprecate `ProtocolErrorCode` from production use or redefine its discriminants to reference `error_codes::` consts, so the "one source of truth" statement is literally (not just test-enforced) true.

## Semver Gate Note (for Plan 07/08 phase-end gate)

The public request enums (`ClientRequest`, `Request`) are UNCHANGED and the `error::ErrorCode` delegation keeps every const name + value identical (API-invisible). The phase-end `cargo semver-checks check-release` MUST report NO `major` AND NO `enum_variant_added` against `ClientRequest`/`Request`. If it reports either, a public-enum change was introduced and the executor must STOP and escalate — the 2.x-minor promise would be at risk.

## Issues Encountered

- None. (The `dead_code` warnings above were expected given the Plan-05 split and handled with a scoped, justified allow.)

## Threat Flags

None — no new security-relevant surface. The change is internal source-of-truth consolidation + a crate-private routing seam; the mitigations for T-112-06 / T-112-06d / T-112-08 (frozen -32002 verbatim, per-name consistency test, no public enum variant) are all implemented as specified.

## Next Phase Readiness

- `error_codes::` table + `error::ErrorCode` delegation ready for Plan 07 (dispatch) and Plan 08 (streamable-HTTP) to migrate their remaining bare-literal EMIT sites onto the table.
- `classify_internal_method` + `ServerDiscoverRequest` ready for Plan 05 to wire into the server request path (era-gated `server/discover` handler).
- No blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

Created file `src/types/protocol/error_codes.rs` and this SUMMARY exist on disk; both task commits (ab5ed16b, a8a66a75) are present in git history.
