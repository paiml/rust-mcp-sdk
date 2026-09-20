---
phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b
plan: 04
subsystem: api
tags: [graphql, apollo-compiler, contract-test, pmcp-run, attestation, parked-live-leg]
status: complete

requires:
  - phase: 122-01
    provides: "Makefile target test-cargo-pmcp-integration with per-named-binary passed-count assertions (scripts/named-test-binary-count.awk), and the corrected gate claim in package_capture_contract.rs that this plan's module docs copy"
  - phase: 122-02
    provides: "MT_ATTESTATION = application/vnd.pmcp.attestation.v1 (kind-neutral) — the media type named in the SDL's transport-encoding comment"
provides:
  - "contracts/pmcp-run/attestation-v1.graphql — the SDK-PROPOSED, unratified verifyAttestation SDL"
  - "VERIFY_ATTESTATION_QUERY, GRAPHQL_AUTH_HEADER, verify_attestation_request_body, decode_verify_attestation_response, VerifyAttestationOutcome (cargo_pmcp::pmcp_run_graphql::*)"
  - "cargo-pmcp/tests/package_attestation_contract.rs — offline blocking contract test + the parked live leg"
  - "REQUIRED_TEST_BINARIES entry package_attestation_contract (+ matching --test selector)"
  - "Numbered ratification ask #5 in docs/platform-requests/package-portability-alignment.md"
affects:
  - "Phase 123 (import semantics; SC3's single-API-path constraint — this plan deliberately opened no second API path)"
  - "Phase 124 (release; cargo-pmcp ships the new pub seam under #[doc(hidden)] pmcp_run_graphql)"
  - "any future pmcp.run platform SDL export, which replaces attestation-v1.graphql wholesale"

actuals:
  tokens: 6752       # chars/4 over the realized diff (27,006 chars, 894 insertions / 11 deletions)
  tasks: 3
  commits: 3         # + 1 docs commit for this SUMMARY

tech-stack:
  added: []          # zero new external packages; apollo-compiler 1.32.0, reqwest, tokio, sha2 and base64 were already cargo-pmcp deps
  patterns:
    - "Vendored SDL whose header states NON-provenance — an unratified proposal deliberately distinguishable at a glance from a platform-exported sibling"
    - "Parked live leg whose request path is EXECUTABLE: production request builder + production response decoder, with only the transport in the test"
    - "Triple env gate (opt-in + endpoint + credential), each gate printing its own named skip message"
    - "Comment-stripped SDL body for shape assertions, so a header discussing a banned keyword cannot satisfy the check that bans it"

key-files:
  created:
    - contracts/pmcp-run/attestation-v1.graphql
    - cargo-pmcp/tests/package_attestation_contract.rs
  modified:
    - cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs
    - cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs
    - Makefile
    - docs/platform-requests/package-portability-alignment.md

key-decisions:
  - "Root type is Query, not Mutation — verification is a read; the choice is stated in an SDL comment so ratification can contest it explicitly."
  - "Attestation bytes travel base64-encoded (RFC 4648 §4, padded) under a String! argument, because GraphQL has no bytes scalar and the SDK will not invent one for a contract it does not own. Encoding is stated in BOTH the SDL comment and the builder's rustdoc."
  - "verify_attestation_request_body returns Result and rejects an empty payload or blank subject digest — that is what gives the Result meaning and what stops a request that asks the platform to verify nothing."
  - "The decoder surfaces a GraphQL errors array as Err naming the FIRST message, never a defaulted outcome: at the call site a defaulted empty verdict is indistinguishable from a pass."
  - "graphql_contract.rs gained a base64 import. The measurable acceptance criterion (no reqwest / oauth2 / crate::commands) holds; base64 is a leaf crate already in cargo-pmcp's [dependencies] and is mandated by the plan's own action text. Recorded as a documented tension, not a silent widening — the module docs now state the dependency rule explicitly."
  - "The live leg treats PMCP_API_URL as the GRAPHQL endpoint (the execute_graphql_at parameter), not the API base, because discovery (resolve_graphql_url) lives in the same bin-only tree as get_credentials(). Recorded as a measured limitation rather than papered over with a second variable, which Phase 123 SC3 forbids."

patterns-established:
  - "An SDK-authored contract file must state what it is NOT: no Source:, no Exported:, and an explicit contrast with the sibling whose provenance is real."
  - "A contract test over an SDK-written schema must state in its own module docs that it pins internal agreement only, so a green run is never mistaken for platform agreement."
  - "A parked test's unpark procedure is a one-sentence rustdoc instruction naming exactly what to delete."

requirements-completed: [PKGX-01]

coverage:
  - id: D1
    description: "The SDK-proposed verifyAttestation SDL is vendored, honestly labelled as unratified, and scoped to one operation (D-07 + D-11)"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_attestation_contract.rs#sdl_shape_is_pinned_to_d11_scope"
        status: pass
      - kind: other
        ref: "grep -c '^# Source:\\|^# Exported:' contracts/pmcp-run/attestation-v1.graphql -> 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "The CLI's operation constant validates against the vendored SDL offline with apollo-compiler, and genuinely FAILS on drift"
    requirement: PKGX-01
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_attestation_contract.rs#verify_attestation_op_validates_against_contract"
        status: pass
      - kind: other
        ref: "negative control: verifiedIdentity -> verifiedIdentityTYPO in VERIFY_ATTESTATION_QUERY -> apollo-compiler 'type VerifyAttestationReturnType does not have a field' -> 1 failed; reverted -> green"
        status: pass
    human_judgment: false
  - id: D3
    description: "The contract test binary runs inside a real gate that asserts a NONZERO passed count for it by name"
    requirement: PKGX-01
    verification:
      - kind: other
        ref: "make quality-gate -> exit 0, '✓ package_attestation_contract passed 3 tests' at qg2.log:6678, 'ALL TOYOTA WAY QUALITY CHECKS PASSED' at :11820"
        status: pass
      - kind: other
        ref: "negative control: #[ignore] on all three passing tests -> make test-cargo-pmcp-integration exit 2, 'RAN but passed ZERO tests'; reverted -> exit 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "The parked live leg carries an EXECUTABLE request path built from production helpers, and never reaches the network on a default run"
    requirement: PKGX-01
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs — 6 offline tests over the builder + decoder (cargo test -p cargo-pmcp --lib -> 473 passed)"
        status: pass
      - kind: other
        ref: "negative control: PMCP_ATTESTATION_LIVE_TEST=1 PMCP_API_URL=https://example.invalid, PMCP_ACCESS_TOKEN unset -> third skip message, exit 0, no network"
        status: pass
      - kind: other
        ref: "source assertions: calls verify_attestation_request_body + decode_verify_attestation_response, POSTs with GRAPHQL_AUTH_HEADER, asserts 2 decoded fields; grep -c '\"Authorization\"' in the test -> 0"
        status: pass
    human_judgment: false
  - id: D5
    description: "A findable, concrete ratification ask exists for exactly one operation, with the SDK's boundary and the format addition stated alongside it"
    requirement: PKGX-01
    verification:
      - kind: other
        ref: "grep -c '^### [0-9]' docs/platform-requests/package-portability-alignment.md -> 6, contiguous 1..6; git diff shows only the heading line changed in the renumbered FYI block"
        status: pass
    human_judgment: true
    rationale: "Numbering and required phrases are machine-checked, but whether the ask reads as concrete and actionable to the pmcp.run platform team is a judgment no test can make — and the ask's whole purpose is to elicit a human response from another org."

metrics:
  duration: "~78 min"
  completed: 2026-08-25
  tasks: 3
  commits: 3
  files_created: 2
  files_modified: 4
---

# Phase 122 Plan 04: Attestation Contract (Contract-First, Parked) Summary

**An SDK-proposed `verifyAttestation` SDL that says plainly it is unratified, an apollo-compiler test that fails the build if the CLI's operation drifts from it, and a live leg whose request path already executes — so unparking is deleting an `#[ignore]` and three `if` blocks, not writing a client.**

## Performance

- **Duration:** ~78 min (first task commit 2026-08-25T20:28:41Z, SUMMARY 2026-08-25T20:47Z; includes one full `make quality-gate` run that died on disk exhaustion and a second that passed)
- **Tasks:** 3
- **Files created:** 2 · **modified:** 4

## Accomplishments

- **`contracts/pmcp-run/attestation-v1.graphql`** — an SDK-authored SDL carrying **no `Source:` and no `Exported:` line**, because it has no provenance and imitating one would misstate ownership. Its header leads with `STATUS: SDK-PROPOSED. NOT PLATFORM-EXPORTED. AWAITING RATIFICATION.`, points at the ratification ask by line 6, and contrasts itself explicitly with `capture-v1.graphql`'s genuine 2026-07-20 AppSync export. Scoped to **one operation** per D-11.
- **The production client half** — `VERIFY_ATTESTATION_QUERY`, `verify_attestation_request_body`, `decode_verify_attestation_response`, `VerifyAttestationOutcome` and `GRAPHQL_AUTH_HEADER`, all in the dependency-light `graphql_contract.rs` leaf, with 6 offline unit tests. This is what makes SC5's promise true rather than aspirational.
- **`GRAPHQL_AUTH_HEADER` now feeds the shipped client too** — `execute_graphql_at` no longer carries an inline `"Authorization"` literal, so the parked leg and the production path cannot drift into two auth shapes.
- **`cargo-pmcp/tests/package_attestation_contract.rs`** — three non-ignored tests plus the parked live leg, appended to `REQUIRED_TEST_BINARIES` **and** the `--test` selector list in the same commit that created the binary.
- **Ask #5 in the platform-requests doc** — promoted out of the FYI section whose own text said "Nothing to build now", with the former `### 5.` renumbered to `### 6.` and its body otherwise untouched.

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Vendor the SDL, the operation constant, and the pure request/response seam | `3b1742b1` | `contracts/pmcp-run/attestation-v1.graphql`, `graphql_contract.rs`, `graphql.rs` |
| 2 | Offline blocking contract test, parked live leg, gate append | `20471646` | `cargo-pmcp/tests/package_attestation_contract.rs`, `Makefile` |
| 3 | The ratification ask, promoted out of the FYI | `9941abf3` | `docs/platform-requests/package-portability-alignment.md` |

## The contract, as landed

**Operation signature** (`type Query`, not `Mutation` — verification is a read, and the choice is stated in an SDL comment so ratification can contest it explicitly):

```graphql
verifyAttestation(
  attestationPayloadBase64: String!
  subjectPayloadDigest: String!
): VerifyAttestationReturnType
# → { verdict: String!, verifiedIdentity: String!, verifiedAt: String! }
```

**Argument names** are `attestationPayloadBase64` and `subjectPayloadDigest`, matched character for character by the request builder's emitted variable keys — asserted by reading the keys out of the produced `serde_json::Value`, not by eyeballing.

**Payload encoding for transport:** base64, STANDARD alphabet, RFC 4648 §4, with padding. Stated in the SDL argument comment *and* the builder's rustdoc so the platform is not left guessing. A unit test round-trips deliberately non-UTF-8 bytes (`00 ff 7b 22 fe`) to prove the bytes survive unchanged — the platform signed those exact bytes, so canonicalizing them would break the signature.

**No enums anywhere.** `verdict` is `String!`, same discipline as `capture-v1.graphql`'s `status`, so a later proposal-versus-export diff does not show permanent drift.

**Production signatures:**

```rust
pub const GRAPHQL_AUTH_HEADER: &str = "Authorization";  // RAW token, no `Bearer ` prefix
pub fn verify_attestation_request_body(attestation_payload: &[u8], subject_digest: &str) -> Result<Value>;
pub fn decode_verify_attestation_response(body: &Value) -> Result<VerifyAttestationOutcome>;
pub struct VerifyAttestationOutcome { pub verdict: String, pub verified_identity: String, pub verified_at: String }
```

Both functions are pure and IO-free.

## Environment variables — which are created and which are reused

| Variable | Status | Note |
|---|---|---|
| `PMCP_ATTESTATION_LIVE_TEST` | **CREATED** (test-only) | Gate 1. Read by no production path. |
| `PMCP_API_URL` | **REUSED** | Gate 2. Already the highest-precedence endpoint source for the shipped client (`auth.rs:113-114`). No second base-URL variable was introduced — Phase 123 SC3 forbids a second API path. |
| `PMCP_ACCESS_TOKEN` | **REUSED** | Gate 3. One of `get_credentials()`'s own three documented sources (`auth.rs:527-538`, the CI/CD branch). No second credential source was introduced. |

## Control runs (the evidence this plan exists to produce)

Every control below was **executed**, not reasoned about. Exit codes are as observed.

| # | Control | Command | Exit | Observed |
|---|---------|---------|------|----------|
| 1 | **Contract drift** | `verifiedIdentity` → `verifiedIdentityTYPO` inside `VERIFY_ATTESTATION_QUERY`'s selection set, then run the binary | **FAILED** (1 failed / 2 passed / 1 ignored) | apollo-compiler: `` type `VerifyAttestationReturnType` does not have a field `verifiedIdentityTYPO` `` with a rendered span pointing at both `verify_attestation.graphql:11` and `attestation-v1.graphql:73` |
| 1r | Restored | same binary after revert | **ok** | 3 passed / 1 ignored |
| 2 | **Gate liveness** | `#[ignore]` added to all three passing tests, then `make test-cargo-pmcp-integration` | **2** | `✗ required test binary 'package_attestation_contract' RAN but passed ZERO tests… The summed total (9) stays nonzero from the other selected binaries, so the count guard above CANNOT catch this.` |
| 2r | Restored | `make test-cargo-pmcp-integration` | **0** | `✓ package_attestation_contract passed 3 tests` / `✓ cargo-pmcp integration tests passed (12 tests)` |
| 3 | **Auth gate** | `PMCP_ATTESTATION_LIVE_TEST=1 PMCP_API_URL=https://example.invalid`, `PMCP_ACCESS_TOKEN` unset, `-- --ignored --nocapture` | **0** | `verify_attestation_live skipped: set PMCP_ACCESS_TOKEN to a real pmcp.run access token (one of get_credentials()'s own three sources)` — **no network activity**: `example.invalid` produced no DNS or transport error, proving gate 3 is ordered before the request |
| 4 | **Endpoint gate** | `PMCP_ATTESTATION_LIVE_TEST=1` only | **0** | `verify_attestation_live skipped: set PMCP_API_URL to the pmcp.run GraphQL endpoint` |
| 5 | **Opt-in gate** | `-- --ignored --nocapture`, no variables set | **0** | `verify_attestation_live skipped: set PMCP_ATTESTATION_LIVE_TEST=1 to enable the live leg` |

**Note on control 1's shape, recorded because it is easy to misread as a defect:** `response_fields_match_selection_set` **still passed** under the injected typo, because it greps for `verifiedIdentity` — a substring of `verifiedIdentityTYPO`. That is exactly the limitation its own NOTE declares, and it is why the apollo-compiler test is the load-bearing one. The grep test is a supplement, and the control proved which of the two is actually doing the work.

## Open questions the first live run answers

Left in the live leg's rustdoc, and answering them means **tightening** the existing assertions, not writing new ones:

1. **The verdict vocabulary** — what strings the platform actually returns (`verdict` is opaque text to this phase).
2. **The verified-identity spelling** — key id, issuer URI, or org identifier.
3. **Whether a mismatched subject digest produces a `verdict` value or a GraphQL error.** The decoder handles both today; only the platform can say which is the contract.

**Unpark procedure, as stated in the rustdoc:** delete the `#[ignore]` attribute and the three early-return gate blocks. The request path below them already runs.

## Deviations from Plan

**1. [Rule 3 — Blocking] The plan's Task-1 `<verify>` command cannot pass on this baseline**

- **Found during:** Task 1 verification.
- **Plan specified:** `cargo clippy -p cargo-pmcp --all-targets -- -D warnings`.
- **Measured:** it fails for two pre-existing, out-of-scope reasons. (a) `crates/pmcp-workbook-runtime/src/render/mod.rs:420,511` carry two `clippy::too_many_arguments` **errors** under `-D warnings`, and the run dies there before cargo-pmcp is linted at all. (b) cargo-pmcp itself carries **44** pre-existing clippy diagnostics (dead-code in `pentest/*`, `field_reassign_with_default`, etc.). Neither file is touched by this plan (`git status --porcelain` showed only my files modified throughout). The repo's real clippy gate is `make lint`, which carries no `-p` and therefore lints only the root `pmcp` package — cargo-pmcp is not clippy-gated, so this command has never passed and was never going to.
- **Fix:** did NOT touch the pre-existing warnings (SCOPE BOUNDARY). Substituted an achievable check that actually proves something about this plan's code: capture clippy output for `-p cargo-pmcp --all-targets` and assert **zero** diagnostics attributable to the files this plan touched.
- **Verification:** `grep -n "graphql_contract.rs\|pmcp_run/graphql.rs"` over a 370-line clippy log → **no match** (the log's 44 diagnostics prove the grep had real input to search). Same check for `package_attestation_contract.rs` after Task 2 → **0**.
- **Impact:** none on shipped artifacts. `make quality-gate` — the gate that actually governs this repo — passed end to end.

**2. [Rule 2 — Missing critical] The live leg's endpoint semantics needed stating, not assuming**

- **Found during:** Task 2.
- **Issue:** the plan says to POST to `PMCP_API_URL`. Measured: `PMCP_API_URL` is the **API base**, and the shipped client derives the GraphQL endpoint from it by running discovery (`resolve_graphql_url`) before calling `execute_graphql_at`. Discovery lives in the same bin-only tree as `get_credentials()`, so `tests/` cannot reach it. Following the plan literally without saying so would leave an operator's first live run failing against the wrong URL, and the failure would look like a contract finding.
- **Fix:** followed the plan (no second variable — SC3), and wrote the constraint into the live leg's rustdoc under its own heading: the test is the analogue of `execute_graphql_at`, so for a live run the operator exports `PMCP_API_URL` pointing at the **GraphQL endpoint**, and threading discovery back in belongs with Phase 123's single-API-path work.
- **Files modified:** `cargo-pmcp/tests/package_attestation_contract.rs`. **Commit:** `20471646`.

**3. [Rule 2 — Missing critical] Shape assertions had to read a comment-stripped SDL**

- **Found during:** Task 2.
- **Issue:** the plan models test 2 on the sibling's `status_field_is_string_not_enum`, which does `!sdl.contains("enum CaptureStatusValue")`. That shape is unsafe here: this file's header **deliberately discusses** the words `enum`, `getAttestation` and `issueAttestation` in order to record why they are absent (the plan itself requires that prose). A naive `contains` would be satisfied by the very comment explaining the ban — a self-invalidating check, the same class as 122-01's `grep 'allow = ['` bypass.
- **Fix:** added an `sdl_body()` helper that strips everything from `#` onward per line, and asserted structurally (`no line starts with "enum "`, neither deferred name appears in the body, exactly one `verifyAttestation(` declaration). Documented the reason in the helper's own rustdoc.
- **Verification:** the test passes; independent greps over a `sed 's/#.*//'`-stripped copy returned `enum` → 0, `getAttestation|issueAttestation` → 0, and the operation count → 1.
- **Commit:** `20471646`.

---

**Total deviations:** 3 (1 blocking-issue substitution, 2 missing-critical additions).
**Impact:** no scope change. All three increased evidence quality; none widened the plan's surface.

## Issues Encountered

**1. Disk exhaustion killed the first `make quality-gate` run — environment, not code.**
The first full-gate run died at `test-examples` with `error: failed to write to …/full.rmeta: No space left on device (os error 28)`, exit 2. `df -h /System/Volumes/Data` read **898Gi used / 6.5Gi avail / 100%**; the harness itself briefly could not write to its own scratch directory. Per the standing guidance this was NOT misread as a code regression and **nothing outside this worktree was deleted** — in fact nothing was deleted at all. Space recovered on its own (the sibling agent shares this volume), and the retry passed end to end. Recorded because a reader seeing two `quality-gate` invocations should know why.

**2. `mv`-based restore after a negative control produces a stale test binary.**
After control 1, restoring `graphql_contract.rs` from its `.bak` with `mv` left the file with the **backup's older mtime**, so cargo reused the binary compiled with the injected typo and the test reported FAILED on already-correct source. This is a false red of exactly the shape that gets a good control disbelieved. Fix: `touch` the file after any `mv`-based restore, then re-run. Both restores in this plan were re-verified green after touching.

**3. Redirecting command output to a file was silently swallowed in this sandbox.**
`cmd > file 2>&1` repeatedly produced a **zero-line** file while the command clearly ran (exit 0, work performed). Piping through `/usr/bin/tee` produced the full 370 lines. Combined with the known rtk-proxy truncation, every evidence-gathering run in this plan used absolute binary paths plus `tee`. Recorded because a zero-line log read as "no warnings" would have been a confident false green — the same instrument fault 122-01 hit from a different direction.

## Known Stubs

**None.** No placeholder values, no TODO/FIXME, no unrun `<verify>` blocks. Both task-level `<verify>` commands were executed (Task 1's under the documented substitution above; Tasks 2 and 3 verbatim), and the plan-level `make quality-gate` was executed end to end.

The `#[ignore]`d `verify_attestation_live` is **not** a stub and is deliberately not listed as one: its body is a complete, executable request path built from production helpers, and its three gates are measured (controls 3–5). What is missing is the **backend**, which is the phase's declared parked boundary — not unwritten client code.

## Threat Flags

**None.** No new network surface reachable on any default path, no new auth path, no schema change to a shipped format, and **zero new external packages** (`apollo-compiler`, `reqwest`, `tokio`, `sha2` and `base64` were all already `cargo-pmcp` dependencies).

Threats from the plan's register that this plan mitigates, with the evidence:

| Threat | Mitigation evidence |
|---|---|
| T-122-04 (a "blocking" test that runs in no gate) | Control 2: all-`#[ignore]` sweep → exit 2. Gate reach confirmed inside `make quality-gate` at `qg2.log:6678`. |
| T-122-16 (an SDL implying ownership it lacks) | `grep -c '^# Source:\|^# Exported:'` → 0; `STATUS:` line at line 4; test module docs state what the test cannot prove. |
| T-122-17 (live leg reaching pmcp.run on a default run) | Controls 3–5: each gate fires with its own named message, exit 0, no network. |
| T-122-18 (a second API path or credential source) | Both `PMCP_API_URL` and `PMCP_ACCESS_TOKEN` reused; `GRAPHQL_AUTH_HEADER` now consumed by BOTH `execute_graphql_at` and the live leg; `grep -c '"Authorization"'` in `graphql.rs` → 0. |
| T-122-35 (a "parked" leg that is a description) | The leg calls `verify_attestation_request_body`, POSTs with `GRAPHQL_AUTH_HEADER`, calls `decode_verify_attestation_response`, and asserts on two decoded fields — verified by source grep at lines 303/309/326. |
| T-122-36 (a token echoed into CI logs) | Assertions read only typed `VerifyAttestationOutcome` fields; skip messages name variable NAMES only, never values. |
| T-122-08 (an ask implying the SDK verifies identity) | Ask #5 states the SDK "cannot verify a signature offline" and gives that as the reason the platform-side call is needed. |

## Verification

| Check | Result |
|---|---|
| `cargo test -p cargo-pmcp --test package_attestation_contract` | **PASS** — 3 passed / 0 failed / 1 ignored |
| `cargo test -p cargo-pmcp --lib` | **PASS** — 473 passed (467 before + 6 new), 0 failed |
| `make test-cargo-pmcp-integration` | **PASS** — exit 0, four named binaries at 3/3/5/1, `✓ cargo-pmcp integration tests passed (12 tests)` |
| `cargo test … -- --ignored` | **PASS** — skips loudly, no network |
| All five control runs | **PASS** — reproduced above with exit codes |
| `cargo fmt --all -- --check` | **PASS** — 0 diff lines |
| `make quality-gate` | **PASS** — exit 0; `ALL TOYOTA WAY QUALITY CHECKS PASSED` at `qg2.log:11820` |
| `COVERAGE.md` present and matching D-11 | **PASS** — verified, not rewritten; matrix reads verifyAttestation INTEGRATE / getAttestation OPT-OUT / issueAttestation OPT-OUT |

## Success Criteria

- **SC1 is satisfied and measured.** The vendored contract exists, the CLI's operation validates against it offline with `apollo_compiler`, and the binary runs in a gate that asserts a nonzero passed count for it **by name** — proven from both ends (drift → red; all-ignored → red).
- **SC5 is satisfied.** The live leg is `#[ignore]`d, triple-env-gated, carries an executable request path built from production helpers, names what the backend must ship, and unparking removes gates rather than adding tests.
- **D-07's ratification ask is a numbered, findable ask** (`### 5.`) rather than an FYI, reachable from the document's opening summary.

## Next Phase Readiness

- Phase 123 inherits **one** pmcp.run API path, not two: no new base-URL variable and no new credential source were introduced, and the header name is now a single shared constant. SC3 is not pre-broken.
- The live leg's rustdoc names the endpoint-versus-base-URL reconciliation as Phase 123 work, so it will not be rediscovered as a surprise.
- Phase 124 (release) should note that `cargo_pmcp::pmcp_run_graphql` gained four public items under the existing `#[doc(hidden)]` mount — an internal test-facing seam, not a stable API.
- **Nothing here blocks the `pull` release.** The ask asks the platform for a design response, not a delivery date.

## Self-Check: PASSED

- `contracts/pmcp-run/attestation-v1.graphql` — FOUND
- `cargo-pmcp/tests/package_attestation_contract.rs` — FOUND
- Commit `3b1742b1` — FOUND
- Commit `20471646` — FOUND
- Commit `9941abf3` — FOUND

---
*Phase: 122-attestation-carriage-contract-first-parked-on-the-pmcp-run-b*
*Completed: 2026-08-25*
