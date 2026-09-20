---
phase: 112-version-plumbing-spine
plan: 07
subsystem: api
tags: [mcp-protocol, error-codes, versioning, dispatch, semver, zero-satd, single-source-of-truth]

# Dependency graph
requires:
  - phase: 112-03
    provides: "error_codes:: centralized table (INTERNAL_ERROR/METHOD_NOT_FOUND/INVALID_PARAMS/PARSE_ERROR/V1_TASK_PENDING) + per-name consistency test guard"
  - phase: 112-04
    provides: "final dispatch-arm shape on core.rs/mod.rs (ingress resolution threading) so this rewrite lands after the arms exist"
  - phase: 112-05
    provides: "server/discover dispatch + method-not-found (-32601) reuse path; the frozen -32002 golden fixtures"
provides:
  - "server dispatch layer (core.rs/mod.rs/task_dispatch.rs) + jsonrpc.rs production error-emission sites now READ every code from error_codes:: — the centralized table is the actual wire source of truth, not a shadow copy"
  - "frozen -32002 task-pending emitted via error_codes::V1_TASK_PENDING (byte-identical); frozen -32601 method-not-supported via error_codes::METHOD_NOT_FOUND"
  - "repo-wide VERS-06 completeness audit: only Plan 08's streamable_http_server.rs (25 sites) and the non-compiled orphan src/wasi.rs remain as recorded findings"
affects: [112-08-streamable-http, 113-stateless-http, 114-tasks-extension]

# Tech tracking
tech-stack:
  added: []
  patterns: [emit-sites-read-the-table-not-a-copy, name-for-value-swap-preserves-wire-bytes, cfg-test-literal-as-independent-oracle, repo-wide-semantic-audit-as-completeness-gate, frozen-code-by-named-constant]

key-files:
  created: []
  modified:
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/task_dispatch.rs
    - src/types/jsonrpc.rs
    - src/server/batch.rs
    - src/utils/parallel_batch.rs

key-decisions:
  - "Every PRODUCTION error-code integer literal in the dispatch layer + jsonrpc.rs is a NAME-for-VALUE swap to error_codes::* — the emitted wire value stays byte-identical; the table becomes the literal single source of truth for the emit path (closing checker Blocker 1: the shadow-copy disconnect)"
  - "Frozen -32002 (core.rs server-not-initialized + task_dispatch.rs task-not-completed) migrated to error_codes::V1_TASK_PENDING; frozen -32601 to error_codes::METHOD_NOT_FOUND — V1_TASK_PENDING==-32002 and METHOD_NOT_FOUND==-32601 keep the frozen wire values byte-identical; the pending_tasks_result_preserves_minus_32002 locking test is UNTOUCHED (git diff clean) and green"
  - "Fully-qualified crate::types::protocol::error_codes:: path used at every production site (matching the existing Plan-06 production reference in core.rs) — avoids ambiguity with the unrelated server-level error_codes module (src/server/error_codes.rs) that is in scope inside mod.rs"
  - "#[cfg(test)] assertion literals and /// doc-comment examples LEFT AS-IS — they are the independent oracle that proves the constant still equals the frozen wire value; migrating them would make the guard tautological"
  - "Repo-wide semantic audit (Rule 2): two genuine production findings owned by no plan (batch.rs parse-error -32700, parallel_batch.rs timeout -32603) migrated here to make VERS-06 completeness honest; streamable_http_server.rs (25 sites) recorded as Plan 08's explicit scope; src/wasi.rs recorded as a non-compiled orphan (no `mod wasi` declaration)"

requirements-completed: [VERS-06]

# Metrics
duration: 12min
completed: 2026-07-22
---

# Phase 112 Plan 07: Dispatch-Layer Error-Code Migration + VERS-06 Completeness Audit Summary

**The centralized `error_codes::` table from Plan 03 becomes the ACTUAL on-the-wire source of truth: every production error-emission site in the server dispatch layer (`core.rs`, `mod.rs`, `task_dispatch.rs`) and in `jsonrpc.rs` now reads its code from a named `error_codes::*` constant — including the two FROZEN `-32002` task-pending sites (via `V1_TASK_PENDING`, byte-identical) — and a repo-wide semantic audit confirms no production protocol-error literal escapes the table except Plan 08's streamable-HTTP file and the non-compiled orphan `wasi.rs`.**

## Performance

- **Duration:** ~12 min
- **Tasks:** 2
- **Files modified:** 6 (4 owned by the plan + 2 audit-finding files)

## Accomplishments

- **Task 1 — server dispatch layer migrated:** replaced every production error-code integer literal in `core.rs`, `mod.rs`, and `task_dispatch.rs` with its named `error_codes::` constant. `-32603` → `INTERNAL_ERROR`, `-32601` → `METHOD_NOT_FOUND`, and the two FROZEN `-32002` task-pending sites (`core.rs` server-not-initialized, `task_dispatch.rs` task-not-completed) → `V1_TASK_PENDING`. The two `mod.rs` sites are struct-literal `code:` constructions on `JSONRPCError`; migrated in place.
- **Task 2 — jsonrpc.rs production From impl migrated + repo-wide audit:** the `impl From<crate::Error> for JSONRPCError` default arm `Self::new(-32603, ...)` → `error_codes::INTERNAL_ERROR`. Ran a repo-wide semantic audit (production only; struct literals + `::new` + `error_response` constructions; feature-gated modules considered) that surfaced two genuine production findings owned by no plan — migrated both.
- **Frozen discipline preserved (Pitfall 6 / T-112-06b):** `V1_TASK_PENDING == -32002` and `METHOD_NOT_FOUND == -32601`, so every frozen wire value is byte-identical after the swap. `src/server/task_dispatch_tests.rs` is untouched (`git diff --stat` empty) and `pending_tasks_result_preserves_minus_32002` is green.
- **Oracle literals preserved:** all `#[cfg(test)]` assertions (`core.rs`, `mod.rs`, `jsonrpc.rs`, `error/mod.rs`, the `_tests.rs` files) and `///` doc examples keep bare literals — the independent value oracle for the constants.

## Task Commits

1. **Task 1: migrate server-dispatch error literals (core.rs, mod.rs, task_dispatch.rs)** — `7a136654` (refactor)
2. **Task 2: migrate jsonrpc + batch production error literals + repo-wide audit** — `7093f7f4` (refactor)

**Plan metadata:** _(final docs commit — this SUMMARY + STATE/ROADMAP/REQUIREMENTS)_

## Files Created/Modified

- `src/server/core.rs` (modified) — 10× `-32603`→`INTERNAL_ERROR`, 2× `-32601`→`METHOD_NOT_FOUND`, frozen `-32002`→`V1_TASK_PENDING`.
- `src/server/mod.rs` (modified) — struct-literal `code: -32601`→`METHOD_NOT_FOUND`, `code: -32603`→`INTERNAL_ERROR`.
- `src/server/task_dispatch.rs` (modified) — `error_response` ladder `-32603`→`INTERNAL_ERROR` (×11), `-32601`→`METHOD_NOT_FOUND` (×5), frozen `-32002`→`V1_TASK_PENDING`.
- `src/types/jsonrpc.rs` (modified) — production `From<Error>` default `Self::new(-32603, …)`→`error_codes::INTERNAL_ERROR`.
- `src/server/batch.rs` (modified) — **audit finding:** production parse-error `code: -32700`→`error_codes::PARSE_ERROR`.
- `src/utils/parallel_batch.rs` (modified) — **audit finding:** production timeout `code: -32603`→`error_codes::INTERNAL_ERROR` (wire value preserved).

## Verification

- `cargo test --lib server::core` → 47 passed; `cargo test --lib server::task_dispatch` → 16 passed; `cargo test --lib jsonrpc` → 18 passed; `cargo test --lib batch` → 15 passed.
- `cargo test --lib pending_tasks_result_preserves_minus_32002` → 1 passed; `git diff --stat src/server/task_dispatch_tests.rs` empty (frozen locking test untouched, wire value byte-identical).
- `cargo build --lib` clean; `cargo clippy -p pmcp --lib --features full` → zero warnings; `cargo fmt --all` clean.
- Acceptance greps: `error_codes::V1_TASK_PENDING` in core.rs (1) AND task_dispatch.rs (1); `error_codes::` in core.rs (17), mod.rs (2), task_dispatch.rs (17), jsonrpc.rs (1); `error_response(id, -326…` bare == 0 in both core.rs and task_dispatch.rs; `Self::new(-32603` bare == 0 in jsonrpc.rs; the only bare `-32002` in core.rs is inside the `#[cfg(test)]` module (line 2322).

## Repo-Wide VERS-06 Audit (Task 2 acceptance gate)

**Audit command (production only — comments and `#[cfg(test)]`/`_tests.rs`/`///` oracle literals excluded):**

```bash
grep -rnE '(code:\s*-32[0-9]{3}|::new\(-32[0-9]{3}|error_response\([^)]*-32[0-9]{3}|^\s*-32[0-9]{3},\s*$)' \
  src/ --include='*.rs' | grep -vE ':\s*//'
```

**Result — all production protocol-error emission is now table-sourced except:**

| Location | Sites | Disposition |
|----------|-------|-------------|
| `src/server/core.rs`, `mod.rs`, `task_dispatch.rs`, `types/jsonrpc.rs`, `server/batch.rs`, `utils/parallel_batch.rs` | migrated | ✅ read from `error_codes::` |
| `src/server/streamable_http_server.rs` | 25 | **Plan 08 scope** (Wave-5 streamable-HTTP migration) — recorded as the explicit owning file |
| `src/wasi.rs` | 2 | **Non-compiled orphan** — no `mod wasi` declaration anywhere (`grep -rn 'mod wasi' src/` finds only `wasi_adapter`/`wasi_protocol`/`wasi_http_world`); dead code, never reaches the wire. Recorded as a follow-up cleanup item, not a live gap. |
| `#[cfg(test)]` / `_tests.rs` / `///` oracle literals | — | intentionally left literal (independent value guard) |

VERS-06 "all error codes resolve from one centralized table" is satisfied on the wire for the dispatch layer + jsonrpc.rs production paths; combined with Plan 08's streamable-HTTP migration the phase leaves no live production protocol-error literal outside the table (orphan `wasi.rs` excepted, noted for cleanup).

## Decisions Made

- **Emit sites read the table, not a copy:** the migration is a pure name-for-value swap — no numeric value changes — so the table is now the literal single source of truth for the emit path (Plan 03's per-name consistency test remains the drift guard between the table and `ProtocolErrorCode`).
- **Frozen codes by named constant:** `V1_TASK_PENDING`/`METHOD_NOT_FOUND` preserve the exact `-32002`/`-32601` bytes; the frozen locking test is the byte-identity guard and was left untouched.
- **Fully-qualified path at every site:** `crate::types::protocol::error_codes::` (not a `use` import) sidesteps the name collision with the unrelated `src/server/error_codes.rs` module that is in scope inside `mod.rs`, and matches the existing Plan-06 production reference already in `core.rs`.
- **Audit-driven scope for batch.rs/parallel_batch.rs (Rule 2):** these two production literals belong to no plan; migrating them here (rather than deferring) makes the VERS-06 completeness claim honest without waiting on a follow-up, and both are trivial byte-identical swaps that touch no other plan's files.

## Deviations from Plan

### Auto-added (Rule 2 — completeness for VERS-06 correctness)

**1. [Rule 2 — Missing coverage] Migrated two production literals outside the plan's four owned files**
- **Found during:** Task 2 repo-wide audit
- **Issue:** `src/server/batch.rs` (parse-error `code: -32700`) and `src/utils/parallel_batch.rs` (timeout `code: -32603`) are genuine production error-construction sites owned by neither Plan 07 nor Plan 08, so leaving them would make the phase's "no production protocol-error literal outside the table" completeness claim false.
- **Fix:** name-for-value swap to `error_codes::PARSE_ERROR` and `error_codes::INTERNAL_ERROR` respectively (wire values byte-identical).
- **Files modified:** `src/server/batch.rs`, `src/utils/parallel_batch.rs`
- **Commit:** `7093f7f4`

Otherwise the plan executed exactly as written. `src/server/streamable_http_server.rs` (25 sites) was intentionally NOT touched — it is Plan 08's owned file — and `src/wasi.rs` (2 sites) was intentionally NOT touched — it is a non-compiled orphan; both are recorded in the audit table above.

## Semver / Phase-End Gate Note

This migration changes no public API, no type, and no numeric value — it is API-invisible and wire-invisible. The AUTHORITATIVE phase-end gate (`cargo semver-checks check-release` classifying MINOR with no `enum_variant_added` on the public request enums, plus `make quality-gate`) runs after BOTH Wave-5 migrations land (Plan 07 + Plan 08), per the plan's verification block and Plan 03's Semver Gate Note.

## Threat Flags

None — no new security-relevant surface. The register is addressed: T-112-06b (frozen `-32002`/`-32601` preserved byte-identical by named constant; frozen locking test re-run as the byte-identity guard) and T-112-06c (the emitting sites now READ the table — the shadow-copy regression is closed; the repo-wide audit confirms no production literal escapes except the recorded Plan-08 / orphan items).

## Next Phase Readiness

- Plan 08 (streamable-HTTP) is the remaining Wave-5 migration; its 25 `streamable_http_server.rs` sites are the last live production error-code literals. After it lands, run the phase-end semver + `make quality-gate` gate.
- Follow-up cleanup item (non-blocking): migrate or delete `src/wasi.rs`'s 2 orphan literals so the "one source of truth" statement is literally true across all of `src/` (currently true for all compiled production paths).
- No blockers.

---
*Phase: 112-version-plumbing-spine*
*Completed: 2026-07-22*

## Self-Check: PASSED

All six modified source files + this SUMMARY exist on disk; both task commits (7a136654, 7093f7f4) are present in git history.
