---
phase: 116-auth-hardening-seps
plan: 02
subsystem: auth
tags: [oauth, rfc9207, rfc3986, csrf, wasm32, semver, error-markers, proptest, sep-2352]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 01
    provides: "116-BASELINES.md — the doc-check 28-error anchor, the wasm 92-warning anchor, the `full` vs `full,oauth` A/B, the non-zero-count nextest form, and the PMAT write workflow clause (b)"
provides:
  - "src/shared/oauth_validation.rs — the RFC 9207 four-row `iss` table and the CSRF `state` check as ONE pure, I/O-free, wasm32-clean function"
  - "Error::iss_mismatch / state_mismatch / reauth_required + their markers, predicates and typed accessors — the stable programmatic identities 116-04, 116-06, 116-09, 116-11 and 116-12 branch on"
  - "AuthorizationRequestRecord — the specification's mandated single per-request record, with PRIVATE fields so a future field stays semver-minor"
  - "iss_presence_from / parse_iss_env_value — D-04 precedence, SPLIT so an unrecognized env value cannot fail open silently"
  - "MAX_CALLBACK_QUERY_BYTES + the duplicate-security-parameter rule — two fail-closed guards over peer-controlled callback bytes"
  - "tests/oauth_iss_validation.rs — 27 tests proven to run under BOTH --features full,oauth and --features full"
  - "A CLOSED measurement of RESEARCH assumption A2: `make quality-gate` exits 0 at this HEAD"
affects: [116-04, 116-06, 116-08, 116-09, 116-11, 116-12, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure-tier extraction: the security decision is a function of (bytes, record), so a Workers/Lambda handler and the CLI share ONE implementation instead of two"
    - "Marker-const error identities on Error::Protocol — additive, semver-minor, matched by predicate rather than by message substring"
    - "Private-field public structs as the semver default for new types (a future field is minor, not major)"
    - "Split parse-then-resolve so an unrecognized operator value is distinguishable from an unset one, instead of failing open"
    - "Negative controls run by deliberately breaking the implementation and observing WHICH tests fail, then restoring byte-for-byte"

key-files:
  created:
    - src/shared/oauth_validation.rs
    - tests/oauth_iss_validation.rs
    - tests/oauth_iss_validation.proptest-regressions
    - .planning/phases/116-auth-hardening-seps/deferred-items.md
  modified:
    - src/error/mod.rs
    - src/shared/mod.rs
    - src/lib.rs

key-decisions:
  - "The three markers ride Error::Protocol, NOT Error::Authentication — RESEARCH A2 confirmed against the source and pinned by a test"
  - "ErrorCode::INVALID_REQUEST for all three: they are locally-produced fail-fasts describing a malformed/hostile RESPONSE, not a transport fault"
  - "One marker covers both failing iss rows; absence is expressed as iss_actual() == None rather than as a second marker"
  - "IssPresence has no Disabled variant — the D-01 floor is unconditional, so only ABSENCE is configurable"
  - "AuthorizationRequestRecord gets a REDACTING manual Debug (Rule 2) — a derived one would print the CSRF state and PKCE verifier"
  - "Evaluation order state -> iss -> error -> code is load-bearing, not stylistic: it is what implements the spec's MUST NOT on error_description"
  - "AUTH-01 is NOT booked complete — five other plans (116-04/06/08/09/15) also claim it"
  - "The proptest regression seed is committed: it is the minimal input the case-folding negative control shrank to, and it re-runs first"

patterns-established:
  - "A fence is not evidence until it has been observed failing: three deliberate breaks applied at once, 7 attributed failures, source restored byte-for-byte"
  - "An inner //! block in a module whose `pub mod` also carries an outer /// must fully qualify every intra-doc link, or `make doc-check` (-D warnings) fails"

requirements-completed: []

# Metrics
duration: 116min
completed: 2026-08-03
---

# Phase 116 Plan 02: Pure OAuth Validation Tier and Stable Error Identities Summary

**The RFC 9207 four-row `iss` table and the CSRF `state` check now exist ONCE, as a pure I/O-free
function that compiles for `wasm32-unknown-unknown` without the `oauth` feature — plus three
marker-const error identities on `Error::Protocol` (not `Error::Authentication`, per RESEARCH A2)
so every downstream caller branches on `err.is_iss_mismatch()` instead of on message text. All
three security properties were OBSERVED failing under deliberately broken implementations before
being booked.**

## Performance

- **Duration:** ~116 min
- **Started:** 2026-08-03T15:22:58Z
- **Completed:** 2026-08-03T17:19:49Z
- **Tasks:** 2
- **Files:** 7 (4 created, 3 modified), +1661 lines, **0 removed**

## Accomplishments

- **The security decision is now a function, not a flow.** `validate_authorization_response(raw_query,
  record)` takes the RAW query component and returns the authorization code or a typed refusal. It
  touches no socket, no browser, no environment, no clock and no disk. That is what makes a
  Cloudflare Workers / Lambda redirect handler and the CLI loopback listener able to share ONE
  implementation (D-05) — and it is why `authorization_code_flow_inner` in 116-09 can stay under the
  PR-blocking cog-25 gate. `pmat quality-gate --checks complexity`: **0 violations**.

- **The ungated claim is MEASURED in both directions, not asserted.** `binary(oauth_iss_validation)`
  reports **27 tests run, 27 passed** under `--features full,oauth` AND **27 tests run, 27 passed**
  under plain `--features full`. The second run is the one that matters: it proves the tier is
  genuinely ungated and therefore also covered by `make quality-gate`, which 116-01 measured compiles
  *none* of this phase's `oauth`-gated code. `cargo build --target wasm32-unknown-unknown
  --no-default-features --features wasm` exits **0** with **92** warnings — exactly the
  116-BASELINES anchor — and **zero** of them name `oauth_validation.rs`.

- **Three negative controls, run at once and each attributed.** Three deliberate breaks were applied
  to the implementation simultaneously, and the resulting **7** failures partition cleanly:

  | Deliberate break | Test that FAILED | Sibling that still PASSED (proving attribution) |
  |---|---|---|
  | `==` → `eq_ignore_ascii_case` on `iss` | `no_scheme_or_host_case_folding` | the other **three** normalization properties |
  | duplicate check → first-wins (`continue`) | all **five** `a_duplicated_*` tests | `a_duplicated_unknown_parameter_is_not_an_error` |
  | `error` surfaced before `iss` | `an_error_description_behind_a_wrong_iss_is_not_disclosed` | `an_error_description_behind_a_valid_iss_is_surfaced` |

  Log: `target/116-verify/oauth_iss_validation.NEGATIVE-CONTROL.log`. Source restored byte-for-byte
  (`shasum -a 256 -c` → `OK`). Without this, "27 passed" would have been compatible with a suite that
  tested nothing.

- **RESEARCH assumption A2 is CLOSED at this HEAD.** 116-01 carried it open ("`make quality-gate`
  currently exits 0 — **not re-measured**") and assigned it to 116-15. It was run here in full:
  **`make quality-gate` exits 0**, including `test-all` across the whole workspace, `pmcp-package-gate`,
  `audit`, `unused-deps`, `check-todos`, `validate-always`, `purity-check` and `comply`.

- **Two fail-closed guards over peer-controlled bytes that the plan's threat model demanded and that
  nothing upstream provides.** `MAX_CALLBACK_QUERY_BYTES` (8 KiB) refuses before parsing and echoes
  no query byte (a planted canary is asserted ABSENT from the message); any repeat of `state`, `iss`,
  `code`, `error` or `error_description` is refused, because a proxy taking the last occurrence and a
  client taking the first disagree about what was validated. The single-pass decoder makes the
  extraction and the duplicate check the *same* operation, so there is no window in which a
  first-wins value has already been adopted.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Three marker-const error identities on `Error::Protocol` | `72a83b10` | feat |
| 2 | The wasm-clean pure validation tier and its ALWAYS-coverage suite | `373e9c09` | feat |

## Files Created/Modified

- **`src/shared/oauth_validation.rs`** (**created**, 649 lines — `min_lines` 200 ✓). Ungated,
  I/O-free. Public: `MAX_CALLBACK_QUERY_BYTES`, `IssPresence` (`#[non_exhaustive]`),
  `AuthorizationRequestRecord` (private fields + `new` + four accessors + redacting `Debug`),
  `validate_authorization_response`, `parse_iss_env_value`, `iss_presence_from`. Private helpers:
  `ensure_query_within_bounds`, `parse_callback_parameters`, `duplicate_security_parameter`,
  `validate_state`, `validate_iss`, `authorization_server_error`, `missing_authorization_code`.
  8 inline tests, 5 doctests.
- **`tests/oauth_iss_validation.rs`** (**created**, 590 lines — `min_lines` 150 ✓). 27 tests in the
  eight documented groups; NOT `#![cfg(feature = "oauth")]`, which is the point.
- **`tests/oauth_iss_validation.proptest-regressions`** (**created**, 7 lines). The minimal case the
  case-folding negative control shrank to (`host = "aaa.aa", path = ""`). The repo tracks 9 such
  files; `.gitignore:39` ignores only the `proptest-regressions/` *directory* form.
- **`src/error/mod.rs`** (+393/-0). Three marker consts, three private `data`-key consts, three
  constructors, three predicates, three typed accessors, one `iss_field` helper mirroring
  `retired_field`, and 9 unit tests.
- **`src/shared/mod.rs`** (+13/-0) — `pub mod oauth_validation;` with the load-bearing "ungated on
  purpose" rationale. (rustfmt's `reorder_modules` moved the declaration to its alphabetical slot,
  carrying the comment with it.)
- **`src/lib.rs`** (+9/-0) — ungated crate-root re-export of the three call-site types.
  `iss_presence_from` and `parse_iss_env_value` stay module-path-only, per the plan's `<interfaces>`.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (**created**) — two out-of-scope
  findings, see *Deferred Issues*.

## Decisions Made

- **The carrier is `Error::Protocol`, and A2 was re-verified rather than trusted.**
  `Error::Authentication` is `Authentication(String)` at `src/error/mod.rs:40-42` — a bare tuple
  variant with no `data` member — and `protocol_data()` returns `None` for every non-`Protocol`
  variant. A marker on `Authentication` would make its own predicate return `false`.
  `the_authentication_variant_cannot_carry_an_iss_mismatch_marker` pins this: it builds an
  `Authentication` whose *string* contains the literal marker JSON and asserts all three predicates
  are still `false`.
- **`ErrorCode::INVALID_REQUEST` for all three**, justified inline the way `retired_on_v2` justifies
  `METHOD_NOT_FOUND`: these are locally-produced fail-fasts over a malformed or hostile authorization
  *response*, so the code describes what was rejected, not a transport fault.
- **One marker for both failing `iss` rows.** A caller that only wants "reject this response" needs
  one predicate; a caller that cares *which* row reads `iss_actual()`, where `None` is the
  advertised-but-absent case. `Option<&str>` serializes to JSON `null` and reads back as `None`, so
  no second marker is needed.
- **`IssPresence` has no `Disabled` variant.** D-01's floor is unconditional — a present `iss` is
  ALWAYS compared — so the only configurable thing is whether absence is fatal. This is the plan's
  "floor's teeth" and it is tested directly:
  `optional_with_a_present_but_different_iss_is_still_rejected`.
- **`AuthorizationRequestRecord` gets a hand-written redacting `Debug` (Rule 2).** A derived `Debug`
  would print the CSRF `state` and the PKCE `code_verifier` into any log line or panic message that
  formats a record — the same disclosure class as T-116-03, which the plan's threat model treats as
  a mitigation, not a nicety. All four fields are still named, so `missing_fields_in_debug` stays
  quiet and the shape stays legible.
- **`AUTH-01` is NOT marked complete.** Five other plans in this phase also claim it (`116-04`,
  `116-06`, `116-08`, `116-09`, `116-15`); this plan lands the semantics, not the wiring, the fuzzing
  or the conformance fixtures. Booking it now would be exactly the false-booking this phase's
  evidence discipline exists to prevent. `requirements-completed: []`, as in `116-01`.
- **RED was OBSERVED and logged, but not COMMITTED as a broken build.** Both tasks are `tdd="true"`.
  Task 1's failing state was captured first (`target/116-verify/task1-error-markers.RED.log`, 27
  `E0599` method-not-found diagnostics) before any constructor existed. In Rust a test naming a
  non-existent function fails to *compile*, so a separate `test(...)` commit would leave a
  non-building tree in history — which contradicts CLAUDE.md's zero-defect rule and follows 116-01's
  own precedent (`ea1d2d68` landed the bindings and the gate extension together "since either alone
  leaves the suite red"). See *TDD Gate Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The redaction test's sentinel collided with the field name it was checking**

- **Found during:** Task 2, on the first run of the inline test module.
- **Issue:** `the_record_debug_redacts_both_secrets` asserted `!rendered.contains("verifier")`, but
  the shared test fixture built the record with `code_verifier = "verifier"` — and the Debug output
  necessarily contains the *field name* `code_verifier:`, which contains that substring. The test
  failed (`8 tests run: 7 passed, 1 failed`) against a redaction implementation that was in fact
  correct.
- **Why it mattered more than a red run:** had the assertion been written the other way round (a
  positive "the field is present" check), the same collision would have made it pass *vacuously*
  against a Debug that leaked everything.
- **Fix:** the test now builds its own record with sentinels chosen not to be substrings of any field
  name (`pkce-c0d3-s3cr3t`, `csrf-t0k3n`), and a doc comment records why so the next editor does not
  reintroduce a colliding value.
- **Committed in:** `373e9c09`.

**2. [Rule 1 — Bug] Four NEW `make doc-check` errors, caught only because the plan required the gate**

- **Found during:** Task 2, running the acceptance criterion.
- **Issue:** `^error` count went **28 → 32**, breaking 116-BASELINES' accepted-delta anchor and
  breaking the obligation 116-01 recorded by name for this plan. Three were unqualified
  `[`IssPresence…`]` links in the module's inner `//!` block; one was a link to
  `crate::client::OAuthConfig`, a path that does not exist (the type is at
  `crate::client::oauth::OAuthConfig`, behind a feature gate).
- **Root cause, which generalizes:** this module carries an outer `///` rationale on its `pub mod`
  declaration (the plans *require* that comment) **as well as** an inner `//!` block. rustdoc merges
  them and resolves the result in the DECLARING module's scope, so a bare `IssPresence` reports "no
  item named `IssPresence` in scope". `make doc-check` runs `RUSTDOCFLAGS="-D warnings"`, so each is
  a hard error.
- **Fix:** every intra-doc link in the inner block is now fully qualified, with a note in the module
  doc saying why; the non-existent path became a plain code span rather than a link, since an ungated
  module must not link items behind gates it cannot assume. Re-measured: **28**, with **0** hits for
  `oauth_validation` and `src/error/mod.rs` still holding exactly its pre-existing 1 (the
  `[`Error`]` enum/derive-macro ambiguity, now at `:613`).
- **Logged for the next two plans** as `D-116-DOC` in `deferred-items.md`, because `116-04` and
  `116-05` create `src/shared/` modules exactly this way.
- **Committed in:** `373e9c09`.

**Total deviations:** 2 auto-fixed (2 × Rule 1). No Rule 4 situations arose; no architectural change
was needed. **Zero dependencies added** — `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exits
**0**, discharging `T-116-SC`.

## Issues Encountered

- **`make quality-gate`'s captured log is rtk-filtered.** `target/116-verify/116-02-quality-gate.log`
  is 692 lines and contains a literal `... (7027 lines truncated)` marker, so per-binary pass counts
  are **not** recoverable from it. The **exit code is 0** and is citable (it is captured by an `echo
  "$?"` outside the filter, and a control run of `make check-todos` through both the proxy and
  `/usr/bin/make` produced identical output and exit status). A later plan that needs the counts
  should invoke `/usr/bin/make quality-gate > log 2>&1` to bypass the proxy. This is the recorded
  rtk-output-corruption hazard, hit again.
- **A grep for forbidden imports in `oauth_validation.rs` returns hits — all of them in PROSE.**
  `grep -n 'reqwest\|webbrowser\|dirs::\|rand::'` matches line 13, and
  `grep -n 'cfg(.*wasm32\|feature = "oauth"'` matches lines 12 and 18. Every hit is inside the module
  doc explaining *what this module deliberately contrasts with* — which the plan explicitly required
  the doc to say. There are no such imports and no such `cfg` attributes. A future automated check
  over these greps must exclude comment lines or it will report a false positive.
- **`cargo semver-checks` reports "no semver update required", not the "minor bump required" the plan
  predicted.** 223 checks pass, 0 fail, exit 0 against `--baseline-rev b2bf9157`. The plan's
  requirement — *zero MAJOR/breaking findings* — is met; the tool simply does not classify this
  particular set of additions as minor-triggering. Recorded as a correction so `116-13`'s version-bump
  reasoning rests on the change set rather than on this tool's verdict.
- **`--features full` selects 27 tests here but 0 for `oauth_dcr_integration`.** Both facts are true
  simultaneously and are not in tension: this tier is ungated by design, that one is not. Cited so a
  later reader does not "correct" one number against the other.

## Threat Flags

None. The plan's `<threat_model>` covers every trust boundary this plan touches, and each `mitigate`
disposition is discharged by a named test:

| Threat | Discharged by |
|---|---|
| T-116-01 (`iss` spoofing) | four no-normalization properties, each observed failing under a case-folding break |
| T-116-02 (`state` CSRF) | `state` compared first; absence is a mismatch; `state_is_evaluated_before_iss` |
| T-116-03 (state disclosure) | `Error::state_mismatch()` takes no arguments; asserted on `to_string()` |
| T-116-04 (`error_description` after mismatch) | evaluation order; observed failing when the order is swapped |
| T-116-05 (marker moved to `Authentication`) | `the_authentication_variant_cannot_carry_an_iss_mismatch_marker` |
| T-116-05a (parameter smuggling) | 5 duplicate tests + 1 unknown-parameter control, all observed |
| T-116-05b (unbounded callback) | `MAX_CALLBACK_QUERY_BYTES` + planted-canary absence assertion |
| T-116-05c (env value fails open) | `parse_iss_env_value` split out; 8 rejected values asserted `None` |
| T-116-SC (cargo installs) | `Cargo.toml` byte-identical to `b2bf9157` |

## Known Stubs

None. Every public item this plan adds is fully implemented and exercised; nothing returns a
placeholder, an empty collection or a "not available" string.

## TDD Gate Compliance

Both tasks carry `tdd="true"`. **RED was observed for both, and logged, before any implementation
existed** — Task 1 at `target/116-verify/task1-error-markers.RED.log` (27 `E0599` diagnostics), Task 2
by the three-way negative control described above, which is a stronger RED than a
never-compiled test file (it proves the assertions can distinguish a correct implementation from a
plausible wrong one, which a not-yet-written function cannot).

**The RED state was NOT committed as a separate `test(...)` commit.** In Rust, a test naming a
function that does not exist fails to *compile*, so such a commit leaves a non-building tree that
breaks `git bisect` and would be red in CI — which contradicts CLAUDE.md's "ZERO TOLERANCE FOR
DEFECTS" and follows this phase's own precedent (`116-01`/`ea1d2d68` landed a test change and the
code it depended on together for the same reason). Each task is therefore one `feat(...)` commit
containing tests + implementation, with the RED log path named in the commit message. A verifier
looking for a `test(...)` → `feat(...)` pair in `git log` will not find one; the evidence is in
`target/116-verify/` and in the commit bodies.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| suite (gated) | `cargo nextest run --features full,oauth -E 'binary(oauth_iss_validation)'` | **27 run, 27 passed** |
| suite (UNGATED proof) | `cargo nextest run --features full -E 'binary(oauth_iss_validation)'` | **27 run, 27 passed** |
| inline + markers | `-E 'binary(pmcp) and (test(oauth_validation)+test(iss_mismatch)+test(state_mismatch)+test(reauth_required))'` | **17 run, 17 passed** |
| doctests | `cargo test --features full,oauth --doc oauth_validation` / `… --doc error` | 5 passed / 15 passed |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, 92 warnings (= baseline), 0 naming this file |
| D-15 tripwire | `-E 'binary(v2_bounded_reads_tripwire)'` | **13 run, 13 passed** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** |
| lint | `make lint` (`--features full`, pedantic+nursery) | **exit 0** |
| clippy (clause b) | `cargo clippy --features full,oauth --lib --tests -- -D clippy::all -W pedantic -W nursery` | **exit 0**, 0 hits in any file this plan touched |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| doc-check | `make doc-check`, `grep -c '^error'` | **28** (= anchor), 0 attributable |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 pass / **0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| FULL gate | `make quality-gate` | **exit 0** (closes RESEARCH A2) |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero** packages,
so no package-legitimacy checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`, neither fixed here:

- **`D-116-EX` — no plan in Phase 116 owns CLAUDE.md's ALWAYS-**EXAMPLE** requirement.** FUZZ has an
  owner (`116-08`, which names `validate_authorization_response` explicitly), PROPERTY and UNIT are
  discharged here, but a grep for `cargo run --example` / `examples/oauth` across all sixteen plans
  returns **zero** hits and no plan's `files_modified` names anything under `examples/`. The five
  rustdoc doctests are runnable demonstrations but are not `cargo run --example`, and
  `make validate-always`'s `test-examples` step does not reach them. Proposed owner: `116-15`.
  **Phase 116 must not book "ALWAYS requirements satisfied" until this is closed or waived in
  writing.**
- **`D-116-DOC` — the outer-`///`-plus-inner-`//!` intra-doc-link trap** described under *Deviations*,
  written up for `116-04` and `116-05`, which create `src/shared/` modules the same way.

## Next Phase Readiness

**Wave 3 is unblocked.** Every downstream consumer named in the plan's `<interfaces>` block now has a
real, tested symbol to import, at exactly the paths `116-01` committed to `contracts/binding.yaml`
as `status: planned`:

| Consumer | What it can now rely on |
|---|---|
| `116-04` | may add `discovery_url_candidates`, `issuer_matches_metadata`, `classify_discovery_failure`, `derive_application_type` to the SAME ungated module — the file, its declaration and its re-export line all exist |
| `116-06` | conformance fixtures branch on `is_iss_mismatch()` / `is_state_mismatch()`, not on message text |
| `116-08` | `validate_authorization_response` is a `(&str, &Record) -> Result<String>` with no I/O — a `libfuzzer` target needs no harness beyond building a record |
| `116-09` | wires the CLI as the FIRST caller; reads `PMCP_OAUTH_ISS_VALIDATION` at the call site and must `tracing::warn!` on `Some(raw)` + `None` parse — the split exists precisely so it can |
| `116-11`/`116-12` | `Error::reauth_required` + `reauth_issuer()` are live for the SEP-2352 issuer-change and refresh paths |
| `116-15` | may cite `make quality-gate` **exit 0** measured here rather than re-running it; must still flip the eight bindings to `implemented` by hand |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-04`, `116-05` | fully qualify intra-doc links in inner `//!` blocks; diff `make doc-check` against **28** BEFORE committing (`D-116-DOC`) |
| `116-15` | close `D-116-EX` or waive it in writing; do not book AUTH-01 complete on this plan's evidence alone |
| any plan needing gate counts | invoke `/usr/bin/make quality-gate` to bypass the rtk output filter |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: src/shared/oauth_validation.rs                    (649 lines, min_lines 200 ✓)
FOUND: tests/oauth_iss_validation.rs                     (590 lines, min_lines 150 ✓)
FOUND: tests/oauth_iss_validation.proptest-regressions   (7 lines)
FOUND: src/error/mod.rs                                  (1233 lines, +393)
FOUND: src/shared/mod.rs                                 (136 lines, +13)
FOUND: src/lib.rs                                        (349 lines, +9)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md (79 lines)
```

Commits claimed, verified in `git log`:

```
FOUND: 72a83b10  feat(116-02): three marker-const error identities on Error::Protocol
FOUND: 373e9c09  feat(116-02): wasm-clean pure OAuth authorization-response validation tier
```

`must_haves` verification:

```
✓ truths[1] programmatic discrimination — is_iss_mismatch/is_state_mismatch/is_reauth_required
  + iss_expected/iss_actual/reauth_issuer, 9 unit tests, zero message-substring matching
✓ truths[2] pure four-row table — no TcpListener, no browser, no reqwest; 27 tests, 5 table tests
✓ truths[3] no normalization — 4 proptest properties, each asserting MISMATCH, each OBSERVED
  failing under a case-folding break while its three siblings held
✓ truths[4] wasm32 without oauth — cargo build --target wasm32-unknown-unknown exit 0
✓ artifacts: oauth_validation.rs 649 >= 200 and contains "pub fn validate_authorization_response"
✓ artifacts: error/mod.rs contains "ISS_MISMATCH_MARKER" (16 references)
✓ artifacts: tests/oauth_iss_validation.rs 590 >= 150
✓ key_links: "Error::(iss_mismatch|state_mismatch)" present in src/shared/oauth_validation.rs
✓ key_links: "pub use shared::oauth_validation::" present in src/lib.rs (1 line)
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-03*
