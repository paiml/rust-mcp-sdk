---
phase: 112-version-plumbing-spine
plan: 08
subsystem: transport
tags: [mcp-protocol, error-codes, versioning, streamable-http, semver, zero-satd, single-source-of-truth, phase-end-gate]

# Dependency graph
requires:
  - phase: 112-03
    provides: "error_codes:: centralized table (PARSE_ERROR/INVALID_REQUEST/INTERNAL_ERROR/METHOD_NOT_FOUND/AUTHENTICATION_REQUIRED) + per-name consistency-test guard"
  - phase: 112-06
    provides: "the wave-4 v2 header-gate edits already landed in streamable_http_server.rs (this plan migrates the file's pre-existing literals after those edits, no same-wave overlap) + Plan 06's own error_codes:: references"
  - phase: 112-01
    provides: "pinned cargo-semver-checks 0.49.0 + published 2.17.0 baseline for the authoritative phase-end additive gate"
provides:
  - "src/server/streamable_http_server.rs — the streamable-HTTP transport's 25 production error-code literals now READ from error_codes:: (the file carries ZERO bare -32xxx literal; the last live production protocol-error literal in the crate is closed)"
  - "repo-wide VERS-06 completeness audit: no production protocol-error EMISSION literal remains outside the centralized table across compiled src/"
  - "authoritative phase-end gate result: cargo semver-checks check-release vs 2.17.0 = no breaking change (no major, no enum_variant_added on ClientRequest/Request); make quality-gate green"
affects: [113-stateless-http, 114-tasks-extension, 118-conformance]

# Tech tracking
tech-stack:
  added: []
  patterns: [emit-sites-read-the-table-not-a-copy, name-for-value-swap-preserves-wire-bytes, line-range-restricted-scripted-migration, cfg-test-and-central-table-as-independent-oracle, repo-wide-semantic-audit-as-completeness-gate, authoritative-phase-end-semver-and-quality-gate]

key-files:
  created: []
  modified:
    - src/server/streamable_http_server.rs

key-decisions:
  - "Every PRODUCTION JSON-RPC error-code integer literal in the streamable-HTTP transport is a NAME-for-VALUE swap to crate::types::protocol::error_codes::* — wire value byte-identical; the fully-qualified path matches Plan 06's existing production reference in the same file and Plan 07's convention (sidesteps the unrelated src/server/error_codes.rs collision)"
  - "The migration was applied via a line-range-restricted scripted replacement (lines < first #[cfg(test)] boundary, skipping comment lines) because the 25 sites contain many byte-identical literals that the unique-match Edit tool cannot address safely; each mapping verified by added-constant vs removed-literal tally"
  - "This file's #[cfg(test)] module NEVER held bare -32xxx oracle literals (verified against pre-edit HEAD = 0) — the value oracle for these constants is Plan 03's error_codes.rs consistency tests (error_code_surface_delegates_to_table, standard_codes_match_protocol_error_code_enum). The plan's 'test module retains bare literals' acceptance line did not apply to this file; the stronger outcome (zero bare -32xxx anywhere in the file) holds"
  - "Repo-wide VERS-06 audit closed: the only remaining bare -32xxx across compiled src/ are the source-of-truth table (error_codes.rs), #[cfg(test)]/_tests.rs oracle literals, the Plan-03-owned consistency-test-guarded ProtocolErrorCode enum discriminants (a type definition, not an emit site), and the non-compiled orphan src/wasi.rs (no `mod wasi` declaration)"

requirements-completed: [VERS-06]

# Metrics
duration: 11min
completed: 2026-07-22
---

# Phase 112 Plan 08: Streamable-HTTP Error-Code Migration + Authoritative Phase-End Gate Summary

**The streamable-HTTP transport's 25 production error-code literals now read every code from Plan 03's centralized `error_codes::` table — the file carries ZERO bare `-32xxx` literal, closing the last live production protocol-error literal in the crate — and the authoritative phase-end gate confirms the whole phase stays additive: `cargo semver-checks check-release` reports no breaking change (no `major`, no `enum_variant_added` on `ClientRequest`/`Request`) and `make quality-gate` is green.**

## Performance

- **Duration:** ~11 min
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- **25 transport literals migrated (name-for-value swap):** `-32700`→`PARSE_ERROR` (×7), `-32600`→`INVALID_REQUEST` (×11), `-32603`→`INTERNAL_ERROR` (×4), `-32601`→`METHOD_NOT_FOUND` (×1), `-32003`→`AUTHENTICATION_REQUIRED` (×2). The migration touched only the middle JSON-RPC-code argument of `create_error_response(StatusCode::…, <code>, "msg")`; the added-constant tally exactly matches the removed-literal tally per value.
- **Zero drift on the rest of the call:** the `StatusCode::` count is unchanged (34 → 34) and no message string changed. The two originally-inline sites were wrapped by `rustfmt` (line-length) — a pure formatting reflow, strings identical.
- **File now carries zero bare `-32xxx` literal:** the strongest possible completeness for this file. The constant value oracle for these codes lives in Plan 03's `error_codes.rs` consistency tests, not in this file's test module (which never held oracle literals — verified against pre-edit HEAD).
- **Repo-wide VERS-06 completeness audit run and recorded** (see table below): no production protocol-error EMISSION literal remains outside the centralized table anywhere in compiled `src/`.
- **Authoritative phase-end gate run** (after both wave-5 migrations, Plan 07 + Plan 08): semver additive-guarantee gate + `make quality-gate` both green (see Semver / Phase-End Gate below).

## Task Commits

1. **Task 1: migrate 25 streamable-HTTP transport error literals to error_codes:: + repo-wide audit** — `a33246a0` (refactor)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS)_

## Files Created/Modified

- `src/server/streamable_http_server.rs` (modified) — 25 production `create_error_response` code args migrated to `crate::types::protocol::error_codes::{PARSE_ERROR, INVALID_REQUEST, INTERNAL_ERROR, METHOD_NOT_FOUND, AUTHENTICATION_REQUIRED}`; two inline sites reflowed by rustfmt.

## Verification

- `cargo build --lib` clean.
- `cargo test --lib --features full server::streamable_http_server` → **13 passed** (Plan 06 classifier units + `v2_header_gate_proptest`; transport behavior unchanged).
- `cargo test --test v2_required_headers --features full` → **10 passed** (Plan 06 header gate still green after the migration — every matrix cell + missing-header + method/name mismatch + outbound-on-success-and-error + unsupported-version + v1/non-opted-in untouched).
- `cargo fmt --all -- --check` clean (after the reflow of the two inline sites).
- Acceptance greps: production `-32xxx` literals before the (recomputed) `#[cfg(test)]` boundary (line 2116) = **0**; `grep -c 'error_codes::' src/server/streamable_http_server.rs` = **30** (25 migrated + Plan 06's 5, ≥ 25); `StatusCode::` count = **34** (unchanged); no message-string change in the diff.

## Repo-Wide VERS-06 Audit (final completeness gate)

**Audit command (all bare protocol-error literals across `src/`, excluding `//`/`///` comment lines):**

```bash
grep -rnE '\-32[0-9]{3}' src/ --include='*.rs' | grep -vE ':\s*//' | grep -vE '///'
```

Plus the Plan-07 production-pattern probe for struct-literal / `::new` / `error_response` constructions:

```bash
grep -rnE '(code:\s*-32[0-9]{3}|::new\(-32[0-9]{3}|error_response\([^)]*-32[0-9]{3})' \
  src/ --include='*.rs' | grep -vE ':\s*//'
```

**Result — every surviving bare `-32xxx` across compiled `src/` is a non-emission literal; no production error-emission site carries a bare literal:**

| Location | Disposition |
|----------|-------------|
| `src/server/streamable_http_server.rs` (was 25) | ✅ **migrated this plan** — reads from `error_codes::`; zero bare `-32xxx` remain |
| `src/types/protocol/error_codes.rs` | ✅ the **source-of-truth table** (const definitions) + its own oracle tests |
| `src/types/jsonrpc.rs` (13), `src/client/mod.rs` (7), `src/server/core.rs` (5), `src/server/mod.rs` (1), `src/error/mod.rs` (3), `src/server/observability/events.rs` (2), `src/server/observability/backend.rs` (1), `src/server/task_dispatch_tests.rs` (2), `src/server/core_tests.rs` (2) | ✅ **`#[cfg(test)]` / `_tests.rs` oracle literals** — all verified inside a test module boundary (jsonrpc @404, client @3167, core @1787, mod.rs @4503, events @364, backend @603); the Plan-07 production probe returned 5 hits, all confirmed inside these test modules |
| `src/types/protocol/mod.rs` (4: `ProtocolErrorCode` enum discriminants `-32600/-32601/-32602/-32603`) | ⚠️ **Plan-03-owned, consistency-test-guarded** — a parallel *type definition*, NOT an emit site; deliberately unchanged (redefining enum discriminants carries semver/const-eval risk). The `standard_codes_match_protocol_error_code_enum` test binds it to the table. Follow-up (Plan 03 Audit Note) tracks eventual deprecation/redefinition. |
| `src/wasi.rs` (2) | ⚠️ **non-compiled orphan** — confirmed no `mod wasi` declaration anywhere (`grep -rn 'mod wasi\b' src/ | grep -v wasi_` empty); dead code, never reaches the wire. Recorded (Plan 07) as a follow-up cleanup, not a live gap. |

**VERS-06 satisfied on the wire across all compiled production paths.** Combined with Plan 07's dispatch/jsonrpc/batch migration, the phase leaves no live production protocol-error EMISSION literal outside the centralized table (the `ProtocolErrorCode` type-definition parallel and the orphan `wasi.rs` are the two recorded, non-emission follow-ups).

## Semver / Phase-End Gate (AUTHORITATIVE — after both wave-5 migrations)

- **`cargo semver-checks check-release` (v0.49.0, baseline published `pmcp 2.17.0`):** `223 checks: 223 pass, 30 skip` → **`Summary no semver update required`**. No breaking change detected → **no `major`, and no `enum_variant_added` on the public `ClientRequest`/`Request` enums** — the milestone's hard 2.x-minor promise (Plan 03 Semver Gate Note) holds.
- **`make quality-gate`:** ✅ ALL TOYOTA WAY QUALITY CHECKS PASSED (fmt/clippy pedantic+nursery/build/test/audit + team-servers binding-drift check). `pmat comply` reported project-level advisories which are **informational per CLAUDE.md D-07** (PMAT runs authoritatively in CI); the gate itself is green.

## Decisions Made

- **Emit sites read the table, not a copy:** pure name-for-value swap — the table is now the literal single source of truth for the streamable-HTTP emit path (Plan 03's per-name consistency test remains the drift guard).
- **Fully-qualified `crate::types::protocol::error_codes::` path at every site:** matches Plan 06's existing production reference already in this file and Plan 07's convention; sidesteps the name collision with the unrelated `src/server/error_codes.rs` module.
- **Scripted, line-range-restricted migration:** the 25 sites contain many byte-identical literals (`-32700,` appears 7×) that the unique-match Edit tool cannot safely disambiguate; a `perl` replacement bounded to lines before the `#[cfg(test)]` boundary and skipping comment lines is the correct tool. Correctness verified by an added-constant-vs-removed-literal tally per value (7/11/4/1/2 = 25).
- **Oracle lives in the central table's tests, not this file:** this file's test module never held bare `-32xxx` literals; migrating would not have removed an oracle. The phase's value oracle is `error_codes.rs`'s consistency tests plus the frozen `pending_tasks_result_preserves_minus_32002` locking test (untouched).

## Deviations from Plan

Plan executed as written (single task). Two notes for the verifier:

1. **[Rule 3 — Blocking] `rustfmt` reflow of two inline `create_error_response` calls.** The two originally single-line sites (`"Unknown session ID"` / `"No session ID provided"`) exceeded the line-length limit once the short `-32600` literal became the longer `crate::types::protocol::error_codes::INVALID_REQUEST` path, so `cargo fmt --all` wrapped them multi-line. This is a pure formatting reflow — `StatusCode::` and message strings are byte-identical. Required because the ZERO-TOLERANCE `cargo fmt --all -- --check` gate (part of `make quality-gate` and the pre-commit hook) would otherwise fail. Folded into commit `a33246a0`.
2. **Plan acceptance line "test module retains bare literals ≥ 1" did not apply to this file.** Verified against pre-edit HEAD that this file's `#[cfg(test)]` module contained **zero** bare `-32xxx` literals — the constant value oracle is Plan 03's `error_codes.rs` consistency tests, not this file. The stronger outcome (zero bare `-32xxx` anywhere in the file) holds and is the honest completeness result. Not a scope change.

## Threat Flags

None — no new security-relevant surface. The register is addressed: T-112-06e (transport error-code wire value preserved byte-identical by named constant; per-value added/removed tally is the byte-identity guard; the file's zero-bare-literal state plus Plan 03's consistency test prove name/value agreement) and T-112-06f (only the JSON-RPC code arg changed — `StatusCode::` count unchanged at 34, no message-string diff, transport + Plan 06 header-gate tests re-run green).

## Next Phase Readiness

- **Phase 112 execution complete** (8/8 plans). The version-plumbing spine is landed: opt-in v2 constant + Era classifier + `ProtocolContext`/`TraceContext`, dispatch threading, centralized `error_codes::` table with every compiled production emit site sourced from it, v2 HTTP header gate, and the authoritative additive gate green.
- Two recorded non-blocking follow-ups (both non-emission): redefine/deprecate `ProtocolErrorCode`'s parallel discriminants (Plan 03 Audit Note), and migrate/delete the orphan `src/wasi.rs` literals.
- Ready for phase verification; downstream Phases 113/114/118 build on the spine. No blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

`src/server/streamable_http_server.rs` (modified) and this SUMMARY exist on disk; the task commit `a33246a0` and the docs commit `07c6375a` are both present in git history. VERS-06 is `[x]` complete in REQUIREMENTS.md (idempotent — also completed by Plans 03/07).
