---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 01
subsystem: protocol-foundations
tags: [spec-checkpoint, conformance-pin, dependencies, error-codes, v2, mrtr]
requires: []
provides:
  - "113-SPEC-RECHECK.md — enforcing spec verdict, conformance-suite pin, contract-first record, Mcp-Name rule + 2 DRIFT items"
  - "ring 0.17 + zeroize 1.8 as explicit optional deps reachable from pmcp production code"
  - "HEADER_MISMATCH / MISSING_REQUIRED_CLIENT_CAPABILITY / UNSUPPORTED_PROTOCOL_VERSION constants"
affects:
  - "plan 03 (requestState AEAD + T-113-05 key scrub — both crates now reachable)"
  - "plan 04 (v2 HTTP status mapper reads the three codes + DRIFT-1 adjudication)"
  - "plan 11 (scenario manifest MUST come from SPEC-RECHECK Section B, not RESEARCH)"
  - "plan 12 (binding re-verification of the three values against the published schema)"
tech-stack:
  added:
    - "ring 0.17.14 (optional, streamable-http) — AEAD for requestState (D-14, native-only)"
    - "zeroize 1.8.2 (optional, streamable-http, default=[alloc], derive OFF) — T-113-05 scrub"
  patterns:
    - "Measured lockfile package-name DELTA (not absolute count) as the zero-new-crates proof"
    - "Wire constants carry provenance-to-a-record in their doc comments"
key-files:
  created:
    - ".planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md"
  modified:
    - "Cargo.toml"
    - "src/types/protocol/error_codes.rs"
decisions:
  - "Verdict PENDING (no schema/2026-07-28); constants landed only under a written developer exception"
  - "Phase-112 D-05 held LOCKED despite measured spec drift; DRIFT-1 recorded rather than silently fixed"
  - "Plan 11 inventory rebased onto the conformance pin — RESEARCH table omits 4 check ids"
metrics:
  duration: 28min
  tasks: 3
  files: 3
  completed: 2026-07-25
---

# Phase 113 Plan 01: Foundations — Spec Checkpoint, Dependency Promotion & v2 Error Codes Summary

Landed the phase-wide foundations every other Phase-113 plan consumes: an enforcing three-state
spec checkpoint that re-pinned the conformance suite and surfaced two spec-drift items, the
`ring` + `zeroize` promotion proven to add zero crates via a measured lockfile delta, and the
three v2 transport error codes under a written, traceable developer exception.

## What Was Built

**Task 1 — `113-SPEC-RECHECK.md`** (594 lines, commit `e00171e9`). Four independent records:

- **Section A / `## Verdict`: `PENDING`.** No `schema/2026-07-28` directory exists upstream
  (three days before publication), so `schema/draft/schema.ts` @ `71e3069` (2026-07-16) was
  used. All 13 mandated tokens FOUND, 0 MISSING. The draft declares the three error codes under
  *exactly* the identifiers the plan specified, all mapping to HTTP 400, with
  `requiredCapabilities` typed `ClientCapabilities` (object) and `supported: string[]`.
  The verdict deliberately was **not** upgraded on that corroboration — it describes the
  *source*, not agreement.
- **Section B: conformance pin** `a865118206d4d8cc8dbc5f5201607839281d0c3b` (2026-07-23), with
  **23 `sep-2322` check ids across 14 scenario classes** enumerated as plan 11's authority.
- **Section C: contract-first environment**, including a `### Deviation from CLAUDE.md
  MANDATORY directives` subsection naming three deviations, their substitutes, and residual
  risk each.
- **Section D: the `Mcp-Name` rule** plus two DRIFT items.

Also recorded for plan 10: the exact declared shapes of `SubscriptionFilter.resourceSubscriptions`
(`string[]`, optional) and the acknowledged notification's `notifications` wrapper (**required**,
typed `SubscriptionFilter`), so plan 10 locks Rust types from evidence rather than prose.

**Task 2 — blocking-human checkpoint** (resolution recorded in commit `abbd9299`). Presented
measured evidence rather than claims; developer replied `approved` (both crates) +
`verdict: exception`.

**Task 3 — dependency promotion + error codes** (commit `558bc3bf`):

- `ring = { version = "0.17", optional = true }` and `zeroize = { version = "1.8", optional = true }`,
  both folded into `streamable-http`. `zeroize` keeps `default = [alloc]` ON (the `Zeroize` impl
  for `Vec<u8>` lives behind `alloc`) and `derive` OFF.
- Three constants with doc comments naming their HTTP-400 mapping and citing
  `113-SPEC-RECHECK.md` as provenance, plus five locking tests.

## Key Decisions

**The gate was allowed to actually bite.** The draft matched the expected values perfectly —
identical identifiers, identical numbers. The tempting move was to call that
`PUBLISHED-CONFIRMED` and skip the checkpoint. Doing so would have made the gate decorative,
which is precisely the failure Codex blocking finding 1 flagged. The verdict stayed `PENDING`
and the constants landed only under a written exception naming a person, a date, a source
commit, and a binding re-verification obligation.

**Phase-112 D-05 held LOCKED despite measured drift.** Relaxing `require_three_headers` would
be a security-relevant loosening of a fail-closed gate on evidence from a draft that may still
move. Holding strict and recording the failure is reversible; loosening and discovering the
spec kept the requirement is not.

**Zero-new-crates proven as a delta, not an absolute.** The root `Cargo.lock` is
workspace-shared, so an absolute absence assertion for `zeroize_derive` would be false both
before and after (it is present at 1 via `secrecy` and the `aws-*` crates in other members).
The real proof is a byte-identical package-name set (728 → 728) plus `cargo tree -p pmcp`
cleanliness (0).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] `cargo fmt` violation in the new locking test**

- **Found during:** Task 3, at the `make quality-gate` fmt-check stage
- **Issue:** the `("UNSUPPORTED_PROTOCOL_VERSION", UNSUPPORTED_PROTOCOL_VERSION)` tuple was
  hand-wrapped across four lines; rustfmt wanted it on one
- **Fix:** ran `cargo fmt --all`; re-ran the gate to green
- **Files modified:** `src/types/protocol/error_codes.rs`
- **Commit:** `558bc3bf`

### Plan Assumptions That Did Not Hold

**2. `Cargo.lock` is gitignored in this repo and therefore was NOT committed**

The plan listed `Cargo.lock` in `files_modified` and instructed "commit the resulting lockfile
change" (a MEDIUM Codex finding asked for it to be added). In this repository `Cargo.lock` is
ignored at `.gitignore:3` and is untracked. Force-adding it would silently reverse a repo-wide
policy decision, so it was left uncommitted. The plan already anticipated the consequence and
prescribed the workaround used here: because no `git show` baseline exists, the zero-new-crates
property was measured in-task via `target/113-01-lock-names.{before,after}`. **Net effect: the
verification is unaffected; only the commit contents differ from the plan's expectation.**

**3. `## Verdict` / heading names had to satisfy literal `grep` acceptance criteria**

Section headings were initially written as `## Section B — Conformance Suite Pin` etc., which
failed the acceptance greps for `## Conformance Suite Pin`. Renamed to
`## Conformance Suite Pin (Section B)` (and likewise C/D), which satisfies the substring grep
while preserving the in-document cross-references. No content change.

### Findings Surfaced (not defects in this plan)

**4. DRIFT-1 (HIGH) — pmcp is stricter than the spec on `Mcp-Name` presence**

The draft transport spec requires `Mcp-Name` only for `tools/call` / `resources/read` /
`prompts/get` ("Required For" column; clients append it "if applicable"). pmcp's
`require_three_headers` demands it on **every** v2 request. The conformance suite is measurably
such a client — its `tools/list` probes in `http-standard-headers.ts` send `Mcp-Method` alone —
so those header scenarios cannot pass today. Adjudicated at the checkpoint: **D-05 stays
locked**, plan 04 keeps the rule, plan 11 marks the affected scenarios KNOWN-FAILING against
the drift record rather than loosening the gate.

**5. DRIFT-2 (MEDIUM, open) — OWS trimming of `Mcp-Name`**

The suite asserts "Server MUST accept leading/trailing whitespace in `Mcp-Name`
(RFC 9110 §5.5)" and sends `Mcp-Name: "  <tool>  "`. pmcp's `bounded_header_str` performs no
trimming. Whether the value reaches `cross_check_name` un-trimmed depends on whether hyper
strips OWS first — **not verified here**, because verifying it requires a live server (plan
04's surface). Deliberately recorded as an open verification item rather than asserted as a
defect.

**6. Conformance inventory drift — `113-RESEARCH.md` is not a safe source for plan 11**

At the pin, the suite has 23 `sep-2322` check ids. The research table omits four entirely
(`sep-2322-respect-client-capabilities`, `-ignore-unexpected-params`, `-validate-input-responses`,
`-error-on-protocol-error`) and lists `input-required-result-capability-check` as if it were a
check id when it is a scenario *class name* (its id is `sep-2322-respect-client-capabilities`).
The four newly-surfaced ids carry real server obligations (tolerate extra params, validate the
`inputResponses` map, surface genuine protocol errors as JSON-RPC errors rather than
re-prompts). This directly validates the plan's instruction that plan 11 build from Section B.

**7. `-32002` open item resolved by the draft**

The `-32002` → `-32602` rename targets *resource not found*, NOT task-pending. pmcp's
proprietary `V1_TASK_PENDING` squat is unaffected and stays frozen — confirming Phase 112's
decision to keep both `-32002` meanings by name.

## Deferred / Out-of-Scope Observations

Working-tree changes present but untouched by this plan (pre-existing or tool-generated):
`pmcp-course/src/part2-design/ch06-03-workflows.md`,
`pmcp-course/src/part3-deployment/ch08-aws-lambda.md`, and `.pmat/*` cache files (rewritten by
the `pmat comply` stage of `make quality-gate`). None were staged.

## Verification

| Check | Result |
|-------|--------|
| Task 1 automated verify (6 greps) | PASS |
| Task 2 automated verify | PASS |
| Task 3 acceptance criteria (14 discrete checks) | 14/14 PASS |
| `cargo build --lib --features streamable-http` | green |
| `cargo build --lib --no-default-features` | green (both crates genuinely optional) |
| `cargo build --lib --target wasm32-unknown-unknown` | green (neither crate reaches wasm) |
| `cargo test --lib --features full -- error_codes` | 12 passed, 0 failed (5 new) |
| Lockfile package-name delta | **byte-identical, 728 → 728** |
| `cargo tree -p pmcp --depth 1` | lists `ring v0.17.14` + `zeroize v1.8.2` as DIRECT |
| `zeroize_derive` in pmcp's own tree | **0** |
| `make quality-gate` | **ALL TOYOTA WAY QUALITY CHECKS PASSED** |

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharged it |
|-----------|-------------|------------------------------|
| T-113-SC | mitigate | Blocking human checkpoint with measured evidence; both crates confirmed pre-existing in lockfile at expected versions with real upstream repos; byte-identical package-name delta |
| T-113-05 | mitigate (enabling) | `zeroize::Zeroize` now reachable from pmcp production code **with `alloc` on** — the precondition for plan 03's key-buffer scrub |
| T-113-08 | mitigate | `HEADER_MISMATCH` (-32020) + documented HTTP 400 available to plan 04's gate |
| T-113-13 | mitigate | Each constant's doc fixes HTTP status and payload shape (`requiredCapabilities` = object) |
| T-113-43 | mitigate | Three-state verdict held at PENDING; written `## Recorded Exception` with a binding plan-12 re-verification whose failure mode is explicitly "phase-reopening event, not a warning" |

## Known Stubs

None. No placeholder constants, no TODO/FIXME tokens (zero-SATD gate green).

## Threat Flags

None. This plan added two dependencies and three integer constants; it introduced no network
endpoint, auth path, file access pattern, or schema change at a trust boundary.

## Follow-ups for Later Plans

1. **Plan 03** — `ring::aead` and `zeroize::Zeroize` are now importable; must NOT enable
   zeroize's `derive` feature (would pull `zeroize_derive` into pmcp's tree).
2. **Plan 04** — read the three codes and their HTTP-400 mappings from `error_codes.rs`; keep
   `require_three_headers` strict per the DRIFT-1 adjudication; the `-32601` → **404** mapping
   (currently `-32601@200`) is still owned by plan 04.
3. **Plan 10** — lock `SubscriptionFilter` / acknowledged-notification Rust types from Section
   A.6, noting `notifications` is a **required** field on both request params and the
   acknowledgement.
4. **Plan 11** — build the scenario manifest from Section B (23 ids @ pin `a8651182`); mark
   `Mcp-Name`-presence header scenarios KNOWN-FAILING citing DRIFT-1.
5. **Plan 12** — the `## Recorded Exception` re-verification is **binding**: re-check all three
   values against the published `schema/2026-07-28` and upgrade the verdict to
   `PUBLISHED-CONFIRMED`/`PUBLISHED-DRIFT` *before* flipping HTTP-01/HTTP-02 or any other
   requirement to complete. Also resolve DRIFT-2.

## Self-Check: PASSED

- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md` — FOUND
- `Cargo.toml` — FOUND (contains both dep lines + both feature entries)
- `src/types/protocol/error_codes.rs` — FOUND (contains all three constants)
- Commit `e00171e9` — FOUND
- Commit `abbd9299` — FOUND
- Commit `558bc3bf` — FOUND
