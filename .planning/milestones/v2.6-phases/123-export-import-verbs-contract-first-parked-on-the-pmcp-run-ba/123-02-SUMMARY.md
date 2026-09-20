---
phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba
plan: 02
subsystem: api
tags: [graphql, apollo-compiler, contract-first, pmcp-run, sdl, portability, cargo-pmcp]

# Dependency graph
requires:
  - phase: 123-01
    provides: "ORDERING ONLY — no content dependency. Plan 01 registered `package_save_load` in the same two `test-cargo-pmcp-integration` Makefile lines this plan appends to, and GSD's wave rule forbids same-wave `files_modified` overlap. Nothing this plan reads or extends comes from plan 01."
  - phase: 122
    provides: "`contracts/pmcp-run/attestation-v1.graphql` (the SDK-PROPOSED header to imitate), `tests/package_attestation_contract.rs` (the structural template incl. `sdl_body()`), and the parked-const pattern in `graphql_contract.rs`"
provides:
  - "`contracts/pmcp-run/portability-v1.graphql` — the third vendored SDL, SDK-PROPOSED, declaring `getPackageArtifact` with the three-field return shape handoff §5.1 specifies (D-02)"
  - "`GET_PACKAGE_ARTIFACT_QUERY` — the exact operation string the shipped `pull` client will send"
  - "`GetPackageArtifactOutcome` + `get_package_artifact_request_body` + `decode_get_package_artifact_response` — the pure, IO-free codec in the dependency-light contract leaf"
  - "`cargo-pmcp/tests/package_portability_contract.rs` — four offline blocking tests plus one parked, double-gated, self-announcing live leg"
  - "`package_portability_contract` registered in BOTH `test-cargo-pmcp-integration` lists, so `make quality-gate` executes the SC4 gate from this plan's own commit"
  - "The four open platform questions (reference grammar, `payloadDigest` meaning, presigned-URL auth, compression + `oci-layout` entry) written down as questions rather than guessed"
affects: [123-05, 123-07, pmcp-run platform team, package pull]

# Actuals (#2632) — same estimateTokens scale as the plan's `estimate` (chars/4
# over the files actually changed, NOT a harness token count).
actuals:
  tokens: 44172
  tasks: 3
  commits: 4

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Fourth instance of the vendored-SDL + parked-const + offline-blocking-test triad; composition, not invention"
    - "Selection-set drift read from the PARSED apollo-compiler selection set rather than by string matching"
    - "SATD-marker literals split across a `concat!` boundary so the check does not create the thing it bans"

key-files:
  created:
    - contracts/pmcp-run/portability-v1.graphql
    - cargo-pmcp/tests/package_portability_contract.rs
  modified:
    - cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs
    - Makefile

key-decisions:
  - "The SDL carries `attestation-v1.graphql`'s SDK-PROPOSED header and NO `Source:`/`Exported:` line — machine-asserted, not left to the eye — so a platform-exported contract and an SDK-authored proposal are distinguishable at a glance."
  - "The SDL records the platform's 2026-08-26 correction that a real cross-boundary drift net arrives with IMPLEMENTATION, not ratification; `attestation-v1.graphql` could not have said this."
  - "Test 4 reads the PARSED selection set (apollo-compiler `ExecutableDocument`) instead of grepping the query string, so formatting or an in-operation comment cannot hide an added field. This is stricter than the sibling's string-matching drift check."
  - "`required_response_str` gained an `operation: &str` parameter (three existing call sites updated) rather than duplicating the helper: this leaf now decodes two operations, and an error naming the wrong one would send a reader to the wrong contract file."
  - "The live leg deliberately does NOT fetch `downloadUrl`. Doing so before the platform confirms the auth shape (research A3) risks sending a pmcp.run token to another origin — the exact disclosure the SDL is asking about."
  - "M1 adopted: both Makefile appends land in the same commit as the test binary, per the Makefile's own recorded discipline. The append-only rule's hazard is a name landing BEFORE its binary; same-commit registration cannot produce it."

patterns-established:
  - "Parked-contract honesty is now machine-checked on BOTH halves in one test: the SDL's provenance header AND the Rust leaf's freedom from SATD markers."
  - "A `concat!`-split marker table lets a test ban a literal without becoming a hit for the scanners that read the test itself."

requirements-completed: [PKGX-02]

coverage:
  - id: D1
    description: "A third vendored SDL exists under `contracts/pmcp-run/` with an honest SDK-PROPOSED header and no forged provenance lines"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#sdl_is_honest_about_its_provenance_and_parking"
        status: pass
      - kind: other
        ref: "negative control: deleting the SDK-PROPOSED line turns exactly this test red (recorded below)"
        status: pass
    human_judgment: false
  - id: D2
    description: "The exact operation string the client will send validates against the vendored SDL offline, with no backend"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#get_package_artifact_op_validates_against_contract"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/package_portability_contract.rs#selection_set_matches_expected_response_fields_exactly"
        status: pass
      - kind: other
        ref: "negative control: adding an SDL-undeclared field to the selection set turns BOTH red (recorded below)"
        status: pass
    human_judgment: false
  - id: D3
    description: "The pure codec has six named failure paths and lives in the dependency-light leaf with no reqwest/oauth2/crate::commands import"
    requirement: "PKGX-02"
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs#tests (6 get_package_artifact/decode_get_package_artifact tests)"
        status: pass
      - kind: other
        ref: "cargo build -p cargo-pmcp --lib (exit 0) + grep -nE '^use (reqwest|oauth2)|^use crate::commands' returns no matches"
        status: pass
    human_judgment: false
  - id: D4
    description: "The live leg is present, `#[ignore]`d AND env-gated, and prints why it skipped"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "cargo test --test package_portability_contract get_package_artifact_live -- --ignored --nocapture (skip transcript recorded below)"
        status: pass
    human_judgment: false
  - id: D5
    description: "`package_portability_contract` is inside the quality gate from this plan's own commit (review finding M1)"
    requirement: "PKGX-02"
    verification:
      - kind: integration
        ref: "make test-cargo-pmcp-integration → '✓ package_portability_contract passed 4 tests'"
        status: pass
      - kind: other
        ref: "negative control: removing it from the --test selector list produces the -1 'never RAN' verdict and exit 2 (recorded below)"
        status: pass
    human_judgment: false
  - id: D6
    description: "The four open platform questions are recorded in the SDL as questions rather than asserted facts, so the platform answers rather than inherits a guess"
    requirement: "PKGX-02"
    verification: []
    human_judgment: true
    rationale: "Whether the questions are phrased usefully FOR THE PLATFORM — clear enough that the pmcp.run team can answer them without a round trip — is a judgment about prose addressed to another team. No test can assert it. The mechanical part (each question is attached to the field/argument it concerns) is visible in the file; the adequacy is not."

# Metrics
duration: 21 min
completed: 2026-08-26
status: complete
---

# Phase 123 Plan 02: Contract-first `getPackageArtifact` Summary

**A third vendored SDL (`portability-v1.graphql`, SDK-PROPOSED with no forged provenance), the exact `getPackageArtifact` operation string plus its pure IO-free codec, and a four-test offline blocking contract test — registered in the quality gate in the same commit that created it — validating the real client constant against a schema with apollo-compiler and no backend in existence.**

## Performance

- **Duration:** 21 min
- **Started:** 2026-08-26T22:02:21Z
- **Completed:** 2026-08-26T22:22:57Z
- **Tasks:** 3 (one of them TDD: RED → GREEN)
- **Files modified:** 4 (2 created, 2 modified)

## Accomplishments

- `contracts/pmcp-run/portability-v1.graphql` — the third vendored SDL, declaring `getPackageArtifact(reference: String!)` returning `payloadDigest` / `downloadUrl` / `expiresAt` (handoff §5.1, D-02). Its header imitates `attestation-v1.graphql`'s SDK-PROPOSED provenance and explicitly refuses `capture-v1.graphql`'s genuine-ownership header, and it adds one thing `attestation-v1` could not have said: the platform's 2026-08-26 correction that the real drift net arrives with **implementation**, not ratification.
- The four open platform questions written into ARGUMENT and FIELD comments **as questions** — reference grammar, whether `payloadDigest` is the OCI manifest digest or a tar-bytes digest (A4), whether `downloadUrl` takes a plain unauthenticated GET (A3), and whether the tar is uncompressed and tolerates an `oci-layout` marker entry (A2/A6).
- `GET_PACKAGE_ARTIFACT_QUERY` plus `GetPackageArtifactOutcome`, `get_package_artifact_request_body` and `decode_get_package_artifact_response` in the dependency-light contract leaf — the fourth instance of that file's established shape, all `#[allow(dead_code)]` with parked rustdoc, all IO-free, no new heavy imports.
- Six named unit tests, one per `behavior` row, each asserting on the **error message** rather than only on `is_err()` — so an authorization refusal, an unimplemented resolver, a renamed field and a retyped field stay four distinguishable things.
- `cargo-pmcp/tests/package_portability_contract.rs` — four offline blocking tests plus one parked, double-gated live leg that prints why it skipped.
- **M1:** both `test-cargo-pmcp-integration` appends landed in the same commit as the binary, so this plan's own `make quality-gate` run executed the SC4 gate it created (`✓ package_portability_contract passed 4 tests`).

## Task Commits

1. **Task 1: Vendor `portability-v1.graphql`** — `29a4adf8` (feat)
2. **Task 2 (TDD RED): failing tests for the codec** — `e5630680` (test)
3. **Task 2 (TDD GREEN): the operation constant and codec** — `dfb8982d` (feat)
4. **Task 3: offline blocking contract test + Makefile registration** — `bfea2a95` (test)

No REFACTOR commit: the GREEN implementation follows the sibling `verifyAttestation` shape line for line and had nothing to clean up.

## Files Created/Modified

- `contracts/pmcp-run/portability-v1.graphql` (new, 137 lines) — the SDK-PROPOSED SDL subset. One operation, no enum, SCOPE section naming both omissions with D-01 and D-03 as their reasons.
- `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` (+352/-7) — the fourth operation constant, its outcome type, request builder, response decoder, six unit tests, and an `operation` parameter added to `required_response_str`.
- `cargo-pmcp/tests/package_portability_contract.rs` (new, 459 lines) — four offline blocking tests, `sdl_body()`, `selected_response_fields()`, and the parked live leg.
- `Makefile` (+2/-2) — `package_portability_contract` appended to the `--test` selector list and to `REQUIRED_TEST_BINARIES`, last position in each. The `RUSTFLAGS=` assignment is untouched and no surrounding comment was reflowed.

### The Makefile diff, verbatim (M1 acceptance criterion)

```diff
@@ -393,7 +393,7 @@ test-cargo-pmcp:
-	@out=$$(RUSTFLAGS= RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p cargo-pmcp --test package_capture_contract --test package_attestation_contract --test package_inspect --test pmcp_package_pin --test package_save_load -- --test-threads=1 2>&1); \
+	@out=$$(RUSTFLAGS= RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) test -p cargo-pmcp --test package_capture_contract --test package_attestation_contract --test package_inspect --test pmcp_package_pin --test package_save_load --test package_portability_contract -- --test-threads=1 2>&1); \
@@ -402,7 +402,7 @@ test-cargo-pmcp-integration: test-openapi-server-guard-selftest
-	REQUIRED_TEST_BINARIES="package_capture_contract package_attestation_contract package_inspect pmcp_package_pin package_save_load"; \
+	REQUIRED_TEST_BINARIES="package_capture_contract package_attestation_contract package_inspect pmcp_package_pin package_save_load package_portability_contract"; \
```

Exactly two changed lines, as the criterion requires.

## Decisions Made

- **Test 4 parses instead of greps.** The plan asked the selection-set check to assert the operation "selects exactly `EXPECTED_RESPONSE_FIELDS` — no more, no fewer". The sibling `package_attestation_contract.rs` does its analogous check by `QUERY.contains(field)`, which can only prove "no fewer" — it is structurally blind to an extra field, which is exactly the "no more" half. So `selected_response_fields()` walks the parsed apollo-compiler `ExecutableDocument` selection set instead. Strictly stronger, same dependency, and the negative control below proves the "no more" half actually fires.
- **`required_response_str` was generalized rather than duplicated.** It hardcoded `"verifyAttestation"` in its error message. With a second operation decoding through it, that message would have named the wrong contract file. It now takes `operation: &str`; the three existing call sites pass `"verifyAttestation"` and the message text is unchanged for them (the existing test asserting on `verifiedIdentity` still passes).
- **SATD markers are `concat!`-split.** Test 2 scans `graphql_contract.rs` for debt markers. Written as plain literals, those markers would make *this test file* a hit for the repo-wide SATD scanners that read it — a check that creates the thing it bans. The split is the same class of reasoning as `sdl_body()`'s comment-stripping, and its rationale is in the constant's doc comment.
- **The plan's wording on the SATD scan was self-contradictory; resolved deliberately.** Task 3 asked to assert no SATD marker "in its non-comment-stripped body … a comment-stripped scan". SATD markers live *in* comments, so a comment-stripped scan of `graphql_contract.rs` would assert nothing at all. The self-invalidation risk the plan was reaching for is about the *test* file's own marker literals, not the scanned file's comments — so the scan reads the RAW `graphql_contract.rs` (where the markers would be) and the `concat!` split handles the test file. Recording this because the two readings differ in whether the check is load-bearing or vacuous.
- **The live leg does not fetch `downloadUrl`.** Fetching a presigned URL before the platform confirms the auth shape (A3) is exactly the operation whose safety the SDL is asking about. The leg asserts on the decoded outcome and stops there; its rustdoc says so and lists it among the questions the first live run answers.
- **Live-leg gate naming.** `PMCP_PORTABILITY_LIVE_TEST` is the one new test-only variable; `PMCP_API_URL` and `PMCP_ACCESS_TOKEN` are reused from the shipped client's own precedence chain, because SC3 forbids a second pmcp.run API path and inventing a second base-URL variable here would pre-break it. (An optional `PMCP_PORTABILITY_LIVE_REFERENCE` selects which package to fetch; it defaults rather than gating, so it cannot cause a silent skip.)

## Verification Evidence

All three negative controls were run against the committed tree and reverted; the working tree is clean.

### Negative control 1 — selection-set drift (Task 3 acceptance criterion)

Added `negativeControlUndeclaredField` to `GET_PACKAGE_ARTIFACT_QUERY`'s selection set:

```
test get_package_artifact_op_validates_against_contract ... FAILED
test sdl_is_honest_about_its_provenance_and_parking ... ok
test sdl_shape_is_pinned_to_d02_scope ... ok
test selection_set_matches_expected_response_fields_exactly ... FAILED
test result: FAILED. 2 passed; 2 failed; 1 ignored
```

Tests 1 and 4 go red together, exactly as predicted — test 1 because the SDL does not declare the field, test 4 because the SDK does not read it. Reverted; 4 passed / 1 ignored restored.

### Negative control 2 — deleted provenance marker

Deleted the `# STATUS: SDK-PROPOSED…` line from the SDL:

```
test sdl_is_honest_about_its_provenance_and_parking ... FAILED
portability-v1.graphql must mark itself SDK-PROPOSED — it was not exported from any live API
test result: FAILED. 3 passed; 1 failed; 1 ignored
```

Only test 2 fires, with its intended message. Restored.

### Negative control 3 (M1) — the Makefile append is load-bearing

Removed `package_portability_contract` from the `--test` selector list while leaving it in `REQUIRED_TEST_BINARIES`:

```
✓ package_save_load passed 25 tests
✗ required test binary 'package_portability_contract' never RAN — cargo printed no
  'Running tests/package_portability_contract.rs' target line. Likeliest causes: the
  file was renamed, or that tests/ entry stopped being a target.
exit=2
```

The `-1` "never RAN" verdict names the binary and the gate exits non-zero. Restored; the gate returns to exit 0 with all six binaries green.

### Live-leg skip transcript (literal output)

```
running 1 test
test get_package_artifact_live ... get_package_artifact_live skipped: set PMCP_PORTABILITY_LIVE_TEST=1 to enable the live leg
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

Run with `--ignored` and the env var unset, it prints which variable to set and exits without failing — a skip that announces itself rather than being indistinguishable from a pass.

### Plan-level verification

| Check | Result |
|---|---|
| `cargo test -p cargo-pmcp --lib` | exit 0, **496 passed**, 1 ignored; the six new codec tests named in the output |
| `cargo test -p cargo-pmcp --test package_portability_contract -- --test-threads=1` | exit 0, **4 passed, 1 ignored** |
| `cargo build -p cargo-pmcp --lib` | exit 0 — the contract leaf still compiles without the bin-only tree |
| `grep -nE '^use (reqwest\|oauth2)\|^use crate::commands' graphql_contract.rs` | no matches |
| `grep -c 'GET_PACKAGE_ARTIFACT_QUERY' graphql_contract.rs` | 6 (≥ 2) |
| `grep -c 'package_portability_contract' Makefile` | 2 |
| `grep -c 'eprintln' package_portability_contract.rs` | 3 (≥ 2) |
| `make test-cargo-pmcp-integration` | exit 0, `✓ package_portability_contract passed 4 tests` |
| `RUSTFLAGS="" make quality-gate` | **exit 0** — `ALL TOYOTA WAY QUALITY CHECKS PASSED`, with `package_portability_contract` running inside it |

## Deviations from Plan

None affecting scope or artifacts — every artifact in `<artifacts_this_phase_produces>` was produced as specified, and no file outside this plan's `files_modified` was touched.

Two implementation choices went beyond the letter of the plan and are recorded as decisions above rather than as auto-fixes, because neither was a bug being repaired:

**1. [Discretion] `required_response_str` gained an `operation` parameter**
- **Found during:** Task 2 (GREEN)
- **Situation:** The existing helper hardcoded `"verifyAttestation"` in its error message; reusing it for a second operation would have produced errors naming the wrong contract.
- **Choice:** Added the parameter and updated the three existing call sites, rather than duplicating ten lines. Existing behaviour and message text for `verifyAttestation` are unchanged.
- **Files modified:** `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs`
- **Verification:** All 496 lib tests pass, including the pre-existing `decode_verify_attestation_response_rejects_null_and_missing_fields`.
- **Committed in:** `dfb8982d`

**2. [Discretion] Test 4 parses the selection set instead of grepping it**
- **Found during:** Task 3
- **Situation:** The plan required "no more, no fewer"; the sibling's string-matching idiom can only prove "no fewer".
- **Choice:** Walk the parsed apollo-compiler selection set. Strictly stronger, no new dependency.
- **Verification:** Negative control 1 shows the "no more" half firing.
- **Committed in:** `bfea2a95`

---

**Total deviations:** 0 auto-fixed (no Rule 1/2/3 triggers — nothing was broken, missing or blocking).
**Impact on plan:** None. Both discretionary choices strengthen the gate the plan set out to build; neither expands scope.

## Issues Encountered

**The plan's `<automated>` verify blocks report a FALSE FAILURE in an `rtk`-hooked shell — the blocks are correct, the environment is not.**

This environment rewrites bare `cargo` and `make` invocations through the `rtk` token-reducing proxy, which **collapses cargo's `test result:` line into a summary of its own** (`cargo test: 496 passed, 1 ignored (1 suite, 5.06s)`) and truncates long output. Every verify block in this plan extracts its count with `awk '/^test result:/ { t += $4 }`, which then sums to `0` and fails the `-gt 0` / `-ge 4` assertion **even though the underlying test run exited 0 with 496 (resp. 4) passing tests.**

I did not weaken or substitute the checks. I ran each verify block exactly as written, against the real toolchain, by invoking the absolute binaries so the hook does not intercept them:

- `/Users/guy/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo` instead of `cargo`
- `/usr/bin/make` instead of `make`

With those, every block passes as written (`total_passed=496`, `passed=4`, `✓ package_portability_contract passed 4 tests`, `make quality-gate` exit 0). This matches the known `rtk` output-corruption gotcha already recorded for `git diff` / `gh pr checks`.

**Consequence for later plans in this phase:** plans 03–07 carry verify blocks of the same shape and will hit the same false failure. This is an environment artifact, not a defect in the blocks — a future executor should reach for the absolute binary path before concluding a build is broken.

Nothing else. No auth gates, no blocked tasks, no unresolved failures.

## Known Stubs

None. Every item this plan created is production code with executable assertions behind it. The two intentionally *parked* items are parked by design and are not stubs:

- `GET_PACKAGE_ARTIFACT_QUERY` and its codec are `#[allow(dead_code)]` with rustdoc explaining that no shipped path sends the operation yet and that plan 05 wires it. They are exercised by six unit tests and one contract test today.
- `get_package_artifact_live` is `#[ignore]`d and env-gated. Its body already calls the real builder, header constant and decoder; unparking is deleting two gate blocks, not writing a test. It announces its own skip (transcript above), so it cannot masquerade as a pass.

Neither is expressed with a `TODO`/`FIXME`/SATD marker — and test 2 now asserts that mechanically.

## Threat Flags

None. The three trust boundaries in the plan's `<threat_model>` are the ones this plan created, and each `mitigate` disposition landed:

| Threat | Where the mitigation lives |
|---|---|
| T-123-11 (download URL disclosure) | `GetPackageArtifactOutcome`'s rustdoc states the URL is a bearer credential that must never be logged, persisted, or fetched with the pmcp.run `Authorization` header; the live leg's assertions deliberately do not print it, and it does not fetch the URL at all. |
| T-123-12 (hostile endpoint) | The decoder validates all three fields and surfaces a GraphQL `errors` array distinctly; four failure causes, four messages. |
| T-123-13 (query/SDL drift) | The blocking test validates the PRODUCTION constant and pins the selection set exactly — both proven load-bearing by negative controls 1 and 3. |
| T-123-14 (forged provenance) | The SDK-PROPOSED marker and the absence of `Source:`/`Exported:` lines are both machine-asserted — negative control 2. |
| T-123-15 (accidental live CI call) | Double gate plus a printed skip reason. |

No new security-relevant surface was introduced beyond what the threat model already names: no endpoint is called by any shipped path, and no credential is read outside the `#[ignore]`d leg.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

**Ready for plan 05**, which joins this plan's remote contract to plan 01's local file round trip by wiring `pull`. Everything plan 05 needs from this side exists and is reachable:

- `cargo_pmcp::pmcp_run_graphql::{GET_PACKAGE_ARTIFACT_QUERY, GetPackageArtifactOutcome, get_package_artifact_request_body, decode_get_package_artifact_response, GRAPHQL_AUTH_HEADER}`.
- The bearer-credential handling instruction travels **on the type**, so plan 05 enforces it at the call site rather than rediscovering it.
- The transport belongs in `graphql.rs`; `graphql_contract.rs` must stay free of `reqwest`/`oauth2`/`crate::commands` or every `tests/` consumer breaks at once.

**Ready for plan 07**, which owns the consolidated Makefile phase paragraph. Note that the two *list appends* it may have expected to make are **already done here** (review finding M1) — plan 07 should add its comment paragraph and must NOT re-append the name, which would break `grep -c … Makefile == 2`.

**Open, and not this plan's to close:** the four questions in the SDL are unanswered. Until the platform answers A4 in particular (`payloadDigest`'s meaning), plan 05's local-vs-remote digest comparison rests on an SDK assumption that the file records as an assumption. And per the module docs, until `getPackageArtifact` is *implemented* and its SDL exported, a green run of this contract test means only that the SDK agrees with itself.

## Self-Check: PASSED

- `contracts/pmcp-run/portability-v1.graphql` — FOUND
- `cargo-pmcp/tests/package_portability_contract.rs` — FOUND
- `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` — FOUND (modified)
- `Makefile` — FOUND (modified)
- Commit `29a4adf8` — FOUND
- Commit `e5630680` — FOUND
- Commit `dfb8982d` — FOUND
- Commit `bfea2a95` — FOUND
- All task `<acceptance_criteria>` re-run and passing (table above)
- Plan `<verification>` block re-run: `RUSTFLAGS="" make quality-gate` exit 0
- Working tree clean after all three negative controls reverted

---
*Phase: 123-export-import-verbs-contract-first-parked-on-the-pmcp-run-ba*
*Completed: 2026-08-26*
