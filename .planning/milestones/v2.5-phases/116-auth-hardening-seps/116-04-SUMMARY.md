---
phase: 116-auth-hardening-seps
plan: 04
subsystem: auth
tags: [oauth, oidc, sep-2351, sep-837, rfc8414, rfc8252, discovery, wasm32, proptest, semver]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 02
    provides: "src/shared/oauth_validation.rs — the ungated, wasm32-clean pure tier this plan extends, its module declaration and its crate-root re-export line"
  - phase: 116-auth-hardening-seps
    plan: 03
    provides: "DcrRequest::set_application_type — the non-validating override sink this plan's derivation feeds, and D-116-LINT (make lint is the authoritative clippy evidence)"
provides:
  - "discovery_url_candidates — SEP-2351's ORDERED candidate list, with the OIDC appended form proven present in EVERY list by a property test"
  - "validate_issuer_url — the hardened RFC 8414 §2 parse (scheme/userinfo/fragment/query/host), returning the parsed Url so callers do not re-parse"
  - "issuer_matches_metadata — RFC 8414 §3.3 / OIDC Discovery §4.3 no-normalization anchor comparison, the value AUTH-01's iss check is anchored on"
  - "same_origin — scheme + host + effective port, for rejecting a cross-origin discovery redirect in 116-06/116-07"
  - "DiscoveryFailure / DiscoveryOutcome / classify_discovery_failure — the discovery outcome matrix as ONE pure function, with three TERMINAL rows that can never trigger fallback"
  - "ApplicationType + derive_application_type — D-10's unanimous-or-error derivation, with the exact wire literals pinned by hand"
  - "A MEASURED correction to the plan's `https:///path` example: WHATWG parses it as host `path`, not as a host-less URL"
  - "D-116-KEYCHAIN — the measured proof that 14 streamable_http test failures predate this plan"
affects: [116-06, 116-07, 116-08, 116-10, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Derive-then-probe split: the pure function returns the ORDERED candidate list and the caller owns the network, which is what makes a spec MUST-ordering testable offline and in a wasm32 build"
    - "A spec fallback order is a LIST assertion, never a single expected value — a suite that asserts one URL per issuer cannot see an ordering defect or a dropped candidate"
    - "Refusals name the RULE and the non-secret parts of the input (scheme, host), never the raw issuer — an issuer string can carry a userinfo password"
    - "Security properties written as the security claim ('an untrusted document never falls through') rather than as a restatement of the match arms"
    - "Attribute an unexplained pre-existing test failure by REVERTING this plan's source in place and re-running the identical command, then comparing the failing SET, not the pass count"

key-files:
  created:
    - tests/oauth_discovery_urls.rs
    - tests/oauth_discovery_urls.proptest-regressions
    - tests/oauth_application_type.rs
  modified:
    - src/shared/oauth_validation.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "The appended form is candidate 3, not a deleted branch — RESEARCH Pitfall 2's measured Entra ID 200/404/404 makes 'replace append with insert' a regression, and a property test fences it for every issuer shape"
  - "For a path-less issuer the list is TWO, not three: the inserted and appended openid-configuration forms coincide, so a third entry would duplicate and waste a probe"
  - "validate_issuer_url refusals do NOT reproduce the issuer string (Rule 2) — userinfo can carry a password and error text reaches logs"
  - "Issuer/redirect refusals use Error::validation, not the module's Error::protocol: these validate a configured identifier, not a hostile protocol response"
  - "classify_discovery_failure maps any non-5xx HttpStatus to Fallback so the function is total over a peer-controlled u16"
  - "derive_application_type puts the loopback rule FIRST, so https-on-loopback is Native — a locally-hosted TLS app is still a native application"
  - "The plan's `https:///path` no-host example is factually wrong and was replaced by `https://`, with the WHATWG behaviour pinned by its own test"
  - "AUTH-02 and AUTH-03 are NOT booked complete — this plan derives values; 116-06/07/10 wire them"

patterns-established:
  - "D-116-DOC has an INVERSE: fully-qualify intra-doc links in an inner `//!` block, but use the BARE form in `///` item docs — each placement is a hard error under the other's rule"
  - "A negative control is only evidence when a SIBLING still passes: 7 failures across 3 breaks (Task 1) and 4 across 3 breaks (Task 2), each with a named surviving sibling"
  - "A random-string property is a WEAKER detector than a hand-written normalization table, and was observed passing under a case-folding break the table caught"

requirements-completed: []

# Metrics
duration: 326min
completed: 2026-08-04
---

# Phase 116 Plan 04: SEP-2351 Ordered Discovery Probe and D-10 `application_type` Summary

**The three call sites that each build the same single, wrong discovery URL now have ONE shared,
spec-ordered derivation — and the fix is fenced against becoming the regression it was meant to
prevent: a property test asserts the OIDC appended form (the only form RESEARCH measured as HTTP
200 against Microsoft Entra ID) is present in EVERY candidate list, for every issuer shape. The
RFC 8414 §3.3 anchor comparison AUTH-01 depends on exists as a pure function with the
specification's own attacker worked example as a test, and `application_type` derivation is
unanimous-or-error with the wire literals pinned by hand.**

## Performance

- **Duration:** ~326 min
- **Started:** 2026-08-03T19:20:49Z
- **Completed:** 2026-08-04T00:47Z
- **Tasks:** 2
- **Files:** 5 (3 created, 2 modified), **+1555 / −3**, **0 removed**

## Accomplishments

- **The MUST-ordered probe is testable with no network, because the derivation and the probing are
  separate.** `discovery_url_candidates` returns the ordered list; the caller owns the socket. Both
  of the specification's worked examples are asserted as full ORDERED vectors — three entries for
  `https://auth.example.com/tenant1`, two for `https://auth.example.com` — rather than as "the
  expected URL". RESEARCH Pitfall 2 records that a one-URL-per-issuer suite is exactly what let the
  "replace append with insert" reading look correct.

- **The Pitfall 2 regression is fenced twice: by name and by property.**
  `the_microsoft_entra_id_form_survives_as_the_last_candidate` pins the measured 200 form
  literally; `the_oidc_appended_form_is_present_for_every_issuer` asserts it for every generated
  issuer shape. Under a deliberate "drop the appended form" break, **both** failed — along with the
  two worked-example tests — while the path-less list, the trailing-slash case and the
  host/scheme-preservation property all still passed. That partition is the evidence the fence is
  attributable rather than incidental.

- **The issuer parse is hardened well past "parses as an absolute URL", and every rule has a named
  refusal.** Scheme (`https`, or `http` only on a loopback host per RFC 8252 §7.3), userinfo,
  fragment, query and host each have a written rule, a rustdoc row and a test asserting the refusal
  names the rule it enforced. `ftp`, `file`, `data` and `javascript` are each rejected **by name**.
  `discovery_url_candidates` delegates to it, asserted directly, so a hostile issuer never reaches
  candidate construction.

- **The discovery outcome matrix is one function with a security property no restatement could
  satisfy.** `IssuerMismatch`, `BodyOverCap` and `MalformedSecurityMetadata` are `Terminal`;
  the property asserts, over an arbitrary peer-controlled `u16`, that an untrusted-document class
  never yields `Fallback` **and** that an availability class never yields `Terminal` — both
  directions, because a silent downgrade and a spurious abort are different defects. Under a break
  that made `IssuerMismatch` a fallback trigger, only that row's test and that property failed;
  `row_body_over_cap_is_terminal` and `row_malformed_security_metadata_is_terminal` both still
  passed, proving the three terminal rows are three detectors and not one.

- **`application_type` fails loudly on every case where guessing would decide where an
  authorization code is delivered.** Mixed vector, empty vector, unparseable URI, and cleartext
  `http` to a remote host are each an error naming the offending URIs. The mixed-vector refusal
  names **both** URIs and **both** classifications, and is asserted to hold in **either order** — a
  first-wins implementation would answer `Web` for one ordering and `Native` for the other.

- **The tier is genuinely ungated, measured in both directions again.** Both new suites report
  their full counts under `--features full,oauth` **and** under plain `--features full` — the
  feature set `make quality-gate` actually uses, which 116-01 measured compiles *none* of this
  phase's `oauth`-gated code. `cargo build --target wasm32-unknown-unknown --no-default-features
  --features wasm` exits **0** with **92** warnings (the 116-BASELINES anchor) and **zero** naming
  this file.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | SEP-2351 ordered discovery-URL candidates + RFC 8414 §3.3 anchor + hardened parse + outcome matrix | `119eeaea` | feat |
| 2 | D-10 `application_type` derivation from `redirect_uris` | `715f557b` | feat |

## Files Created/Modified

- **`src/shared/oauth_validation.rs`** (**modified**, 649 → **1376** lines, +733/−3). Nine new
  public items — `validate_issuer_url`, `discovery_url_candidates`, `issuer_matches_metadata`,
  `same_origin`, `DiscoveryFailure`, `DiscoveryOutcome`, `classify_discovery_failure`,
  `ApplicationType` (+ `as_str`), `derive_application_type` — plus ten private helpers
  (`issuer_scheme_permitted`, `is_loopback_host`, `unparseable_issuer`, `forbidden_issuer_scheme`,
  `issuer_carries_userinfo`, `issuer_carries_component`, `well_known_inserted`,
  `well_known_appended`, `classify_redirect_uri`, and four refusal constructors). 4 new inline
  tests (12 total), 7 new doctests (12 total).
- **`tests/oauth_discovery_urls.rs`** (**created**, **607** lines — `min_lines` 90 ✓). 38 tests in
  seven documented groups. NOT `#![cfg(feature = "oauth")]`, which is the point.
- **`tests/oauth_application_type.rs`** (**created**, **210** lines — `min_lines` 70 ✓). 14 tests.
- **`tests/oauth_discovery_urls.proptest-regressions`** (**created**, 8 lines). Two seeds the
  negative control shrank to. The repo tracks 10 such files; `.gitignore:39` ignores only the
  *directory* form.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (164 → **237** lines) — one new
  entry, `D-116-KEYCHAIN`.

## Decisions Made

- **The appended form survives as the LAST candidate, and a path-less issuer gets TWO candidates.**
  For a path-less issuer the inserted and appended `openid-configuration` forms are the same URL,
  so emitting three would duplicate an entry — which the no-duplicates property would catch and
  which would waste a network round trip proving the same thing twice. The spec's own path-less
  example lists exactly two, so this is the specification's shape, not an optimisation.
- **Refusals never reproduce the issuer string (Rule 2 — auto-added, not in the plan).** The plan
  said each rejection must name the rule violated. It did not consider that the *input* can be
  `https://user:hunter2@auth.example.com`: echoing it would move a credential into every log line
  that records the error. The refusals name the scheme and host instead — neither is a secret — and
  `the_userinfo_refusal_does_not_reproduce_the_credential` asserts the password is absent. The
  redirect-URI refusals *do* name the URI, because the plan requires it and a redirect URI is not a
  credential carrier.
- **`Error::validation`, not the module's existing `Error::protocol`.** The existing functions
  refuse a hostile authorization *response*; these validate a configured identifier the operator or
  the discovery layer supplied. `Error::validation` is what the rest of the crate uses for that,
  and 116-04's plan names it explicitly for `derive_application_type`.
- **`classify_discovery_failure` is total over a peer-controlled `u16`.** Any `HttpStatus` outside
  500–599 maps to `Fallback`, including 1xx/2xx/3xx values a hostile or broken server could produce.
  The alternative — a narrower match with a panic or a fourth outcome — would put a peer in control
  of whether the function completes.
- **The loopback rule is checked BEFORE the scheme rule in `derive_application_type`.** So
  `https://localhost:8443/callback` classifies `Native`: a locally-hosted application serving TLS
  over loopback is still a native application under SEP-837's own wording ("locally-hosted web
  applications accessed via `localhost`"). This is the one row the integration suite cannot reach
  through a natural pmcp flow, so it is pinned by an inline test against the private classifier.
- **`AUTH-02` and `AUTH-03` are NOT booked complete.** This plan derives values; `116-06` and
  `116-07` wire the discovery call sites and `116-10` wires the DCR construction site. `T-116-07`
  (redirect-URI / application-type mismatch), which `116-03` **transferred** to this plan and
  `116-10`, is now *half* discharged: the derivation refuses a mismatch, but nothing in the tree
  calls it yet. `requirements-completed: []`, as in `116-01`, `116-02` and `116-03`.
- **The RED state was NOT committed as a separate `test(...)` commit**, for the same reason
  `116-01`, `116-02` and `116-03` gave: in Rust a test naming a non-existent function fails to
  *compile*, so such a commit leaves a non-building tree that breaks `git bisect`. See *TDD Gate
  Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's `https:///path` no-host example is not host-less**

- **Found during:** Task 1, on the first run of the suite —
  `an_issuer_with_no_host_is_rejected_naming_the_host_rule` **FAILED** against a correct
  implementation.
- **Issue:** the plan's behaviour row says "an issuer with no host (e.g. `https:///path`) → Err".
  Measured: `Url::parse("https:///path")` returns **`Ok`** with `host = Some(Domain("path"))` and
  `path = "/"`. WHATWG's *special authority ignore slashes* state consumes the third slash, so the
  authority becomes `path`. Every conforming parser and every browser agrees.
- **Why it mattered more than a red test:** had the assertion been written loosely (e.g. only
  `is_err()`), the empty-host rule would have looked covered while being exercised by an input that
  never reaches it — and the branch would have been dead code nobody noticed.
- **Fix:** the rejection test now uses `https://`, `https://:8443` and `https://?x=1`, which
  genuinely have no authority (`url` returns `EmptyHost`); a new test
  `a_third_slash_is_authority_syntax_not_a_missing_host` pins the WHATWG behaviour so a later
  reader does not "fix" the branch against an input that cannot reach it; and the implementation
  carries a comment recording that the explicit host check is **defense in depth**, unreachable
  from an accepted scheme with today's `url` crate.
- **Committed in:** `119eeaea`.

**2. [Rule 1 — Bug] Seven NEW `make doc-check` errors — the INVERSE of D-116-DOC**

- **Found during:** Task 1, running the acceptance criterion. `^error` count went **28 → 35**.
- **Issue:** all seven were `rustdoc::redundant_explicit_links` in **`///` item docs**, on links
  written in the fully-qualified form D-116-DOC prescribes
  (`[`classify_discovery_failure`](crate::shared::oauth_validation::classify_discovery_failure)`).
- **Root cause, which generalizes and CORRECTS D-116-DOC's guidance:** the trap 116-02 documented
  applies **only** to the module's inner `//!` block, whose merged docs resolve in the *declaring*
  module's scope. In an item-level `///` doc the bare `[`X`]` resolves fine, so the explicit target
  is redundant — and `make doc-check` runs `RUSTDOCFLAGS="-D warnings"`, so it is a hard error.
  **The two placements need opposite forms, and each is a hard error under the other's rule.**
- **Fix:** item-doc links unqualified (7 lines); the inner `//!` block's links left fully qualified
  and re-verified. Re-measured: **28**, with **0** attributable to `oauth_validation.rs`.
- **Committed in:** `119eeaea`.

**3. [Rule 1 — Bug] `make lint` rejected three things the phase's clause-(b) clippy command accepts — D-116-LINT, three more times**

- **Found during:** Tasks 1 and 2, running `make lint` per `116-03`'s standing obligation.
- **Issue:** three separate hard errors that clause (b) does not produce, because `make lint` sets
  `RUSTFLAGS="-D warnings"`:
  1. `clippy::doc_markdown` × 4 — bare `OpenID` in source doc comments (Task 1);
  2. `clippy::needless_collect` — a `Vec<DiscoveryOutcome>` collected only to assert its length in
     the totality property (Task 1);
  3. `clippy::trivially_copy_pass_by_ref` — `&url::ParseError` (1 byte) passed by reference in
     `unparseable_redirect_uri`, while its Task 1 sibling `unparseable_issuer` already took it by
     value (Task 2);
  plus a fourth `doc_markdown` on `OpenID` in `tests/oauth_application_type.rs` (Task 2) — note
  `make lint` lints `--lib --tests`, so the new **test files are gated too**.
- **Fix:** backticked `OpenID` (5 sites); the totality property rewritten to assert each outcome
  is one of the three documented variants, which is **stronger** than the length check clippy
  objected to; `ParseError` taken by value. `make lint` → **✓ No lint issues** after each.
- **Why it is recorded rather than shrugged off:** this is the third plan in a row where clause (b)
  reported clean on gate-red code. `D-116-LINT`'s recommendation is now backed by three
  independent measurements across two plans.
- **Committed in:** `119eeaea` and `715f557b`.

**Total deviations:** 3 auto-fixed (3 × Rule 1). No Rule 4 situation arose; no architectural change
was needed. **Zero dependencies added** — `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exits
**0**, discharging `T-116-SC`.

## Issues Encountered

- **`make quality-gate` does NOT exit 0 at this HEAD — and does not exit 0 without this plan
  either.** `make test-unit` fails 14 `shared::streamable_http` tests on a macOS keychain error
  (`Os(code: -36)`) at a pre-existing `.expect()` in `src/shared/streamable_http.rs:458`. This was
  attributed by **measurement**: reverting this plan's source in place and re-running the identical
  command gave `1826 passed; 14 failed` versus `1830 passed; 14 failed` with it — the same failing
  set, and a pass count differing by exactly this plan's four new inline tests. Written up as
  `D-116-KEYCHAIN`. **This corrects the phase's working assumption**: `116-02` and `116-03` both
  recorded `make quality-gate` **exit 0**, so the condition arose between `1b0e2f75` and now, or is
  intermittent enough to have missed both.
- **The failure is flaky, and the flakiness is itself a trap.** The same 14 tests were observed
  **passing** twice in this session with identical source (a 33-test filtered run, and one full
  `make test-unit` reporting `1844 passed; 0 failed` = 1830 + 14). Disk was not the trigger (29 GiB
  free at 29%), and `ulimit -n` is 1048576. The only correlate found is concurrency — it appears
  when the whole 1844-test binary runs and not when a subset does. `CLAUDE.md` says CI runs with
  `--test-threads=1`; `make test-unit` does not pass it.
- **Every OTHER `make quality-gate` stage passes**, run individually: `fmt-check`, `lint`, `build`,
  `pmcp-package-gate` (exit 0), `audit` (exit 0, "No vulnerabilities found"), `unused-deps` (0),
  `check-todos` (0, "No technical debt comments"), `check-unwraps` (0, "No unwrap() calls in
  production code"), `purity-check` (0), `comply` (0), plus `test-doc` (all doctests passed) and
  `make test-property` (**✓ Property tests passed**). Log:
  `target/116-verify/116-04-gate-stages.log`, `…-gate-stages2.log`.
- **Two concurrent `make quality-gate` runs wrote to the same log path and interleaved**, producing
  `Blocking waiting for file lock on build directory` and a `Terminated: 15`. Compounding it, an
  `until … grep -q` wait loop fired instantly against the *previous* run's log — the exact stale-log
  trap `116-03` recorded. **Delete the log before starting, and use a unique filename per run.**
- **`ps aux | grep …` returns empty even while `make` is running** under this environment's command
  proxy, so process-liveness checks are unreliable; wait on a marker written into the log instead.
- **`cargo semver-checks` again reports "no semver update required"** despite nine new public
  items — the third plan in this phase to observe it. The requirement (*zero breaking findings*) is
  met: **223 checks, 223 pass, 0 fail**, exit 0. `116-13` must not rest its version-bump reasoning
  on this tool's verdict.
- **A random-string property is a weaker detector than a hand-written table.** Under the
  case-folding break, `no_normalization_of_any_kind_is_applied` (four explicit RFC 3986 variants)
  failed while `the_anchor_comparison_matches_only_identical_strings` (generated
  `[!-~]{1,40}` pairs) still **passed** — two random 40-character strings essentially never differ
  only by case. Both are kept, but the table is the real fence; a plan that shipped only the
  property would have shipped an undetected normalization.

## Threat Flags

None. This plan adds no network endpoint, no socket, no file access and no schema change — the
module is I/O-free by construction, which the `wasm32` build and the import grep both demonstrate.

| Threat | Disposition | Discharged by |
|---|---|---|
| T-116-09 (discovery-document issuer spoofing — the AUTH-01 anchor) | mitigate | `issuer_matches_metadata` as a no-normalization comparison; `the_spec_worked_attack_is_rejected` uses the specification's own `attacker.example` / `honest.example` example; four normalization rows asserted `false`. **Enforcement at the fetch site remains 116-06's** |
| T-116-10 (discovery URL construction from a path-bearing issuer) | mitigate | candidates built by `Url::set_path` arithmetic on the *validated* URL; `every_candidate_keeps_the_issuer_scheme_and_host` asserts scheme, host, no query and no fragment for every generated issuer; unparseable issuers `Err` before any URL exists |
| T-116-11 (regressing discovery for append-only authorization servers) | mitigate | `the_oidc_appended_form_is_present_for_every_issuer` (property) + the named Entra ID test; both observed failing under a drop-the-appended-form break |
| T-116-12 (open redirect via inconsistent `application_type`) | mitigate | unanimity required; mixed / cleartext-`http`-remote / unparseable / empty are all hard errors naming the offending URIs; refusal asserted in **both** orders |
| T-116-12a (userinfo authority confusion) | mitigate | rejected with and without a password (2 tests), and the refusal is asserted **not** to reproduce the credential |
| T-116-12b (query or fragment surviving into the candidate URL) | mitigate | both rejected per RFC 8414 §2; additionally, every generated candidate is asserted to carry neither |
| T-116-12c (silent downgrade via forced candidate fallback) | mitigate | three `Terminal` rows + the two-directional security property over an arbitrary `u16`; observed failing under an `IssuerMismatch → Fallback` break while its two terminal siblings held |
| T-116-12d (cleartext discovery to a non-loopback host) | mitigate | `http` permitted only for loopback; every other `http` issuer errors naming the scheme rule |
| T-116-SC (cargo installs) | mitigate | zero packages added; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0** |
| T-116-07 (inherited from 116-03, **transferred**) | partial | the derivation now refuses a mismatch, but **nothing in the tree calls it yet** — still open for `116-10` |

## Known Stubs

None. Every public item is fully implemented and exercised; nothing returns a placeholder, an empty
collection or a "not available" string. The one deliberately unreachable branch — the empty-host
check in `validate_issuer_url` — is documented in place as defense in depth with the measurement
that makes it unreachable, and is not a stub.

## TDD Gate Compliance

Both tasks carry `tdd="true"`. **RED was observed and logged for both, before any implementation
existed:**

| Task | RED log | Diagnostics |
|---|---|---|
| 1 | `target/116-verify/116-04-task1.RED.log` | `E0432` — 7 unresolved imports, exit 101 |
| 2 | `target/116-verify/116-04-task2.RED.log` | `E0432` — 2 unresolved imports, exit 101 |

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01`
(`ea1d2d68`), `116-02` and `116-03`: a Rust test naming a non-existent function fails to *compile*,
so such a commit leaves a non-building tree that breaks `git bisect` and contradicts CLAUDE.md's
"ZERO TOLERANCE FOR DEFECTS". A verifier looking for a `test(...)` → `feat(...)` pair will not find
one; the evidence is the RED logs above, the negative controls below, and the log paths named in
each commit body.

### Negative control — Task 1 (`116-04-task1.NEGATIVE-CONTROL.log`)

Three deliberate breaks applied **at once**, `38 tests run: 31 passed, 7 failed`:

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| appended form dropped (the Pitfall 2 regression) | `a_path_bearing_issuer_yields_the_three_spec_candidates_in_order`, `a_multi_segment_path_is_carried_through_every_candidate`, `the_microsoft_entra_id_form_survives_as_the_last_candidate`, `the_oidc_appended_form_is_present_for_every_issuer` | the path-less list, the trailing-slash case, the no-duplicates property and the host/scheme-preservation property — all correctly unaffected, since a path-less issuer has no appended candidate to drop |
| `issuer_matches_metadata` case-folds | `no_normalization_of_any_kind_is_applied` | `the_spec_worked_attack_is_rejected`, `an_identical_issuer_matches`, **and the generated-string property** — see *Issues Encountered* |
| `IssuerMismatch` → `Fallback` (the silent downgrade) | `row_issuer_mismatch_is_terminal`, `an_untrusted_document_never_falls_through_and_availability_is_never_terminal` | `row_body_over_cap_is_terminal`, `row_malformed_security_metadata_is_terminal`, `classification_is_total_over_arbitrary_status_codes` — three terminal rows, three detectors |

### Negative control — Task 2 (`116-04-task2.NEGATIVE-CONTROL.log`)

Three deliberate breaks applied at once, `14 tests run: 10 passed, 4 failed`:

| Deliberate break | Tests that FAILED | Siblings that still PASSED |
|---|---|---|
| mixed vector silently picks `Native` (first-wins) | `a_mixed_vector_is_an_error_naming_both_uris_and_both_classifications`, `a_mixed_vector_is_refused_in_either_order` | every single-classification row, and both properties |
| wire literals capitalised to `"Native"`/`"Web"` | `the_wire_literals_are_exactly_native_and_web` **only** | the mixed-vector message test still passed — its `"native"`/`"web"` assertions read the *error text*, not `as_str()`, so the wire-literal test is its own independent detector |
| private-use scheme classified `Web` | `a_custom_scheme_is_native` **only** | the three loopback rows and both `web` rows |

Source restored byte-for-byte after each: `shasum -a 256 -c` → **OK** (twice).

## Gate Results

| Gate | Command | Result |
|---|---|---|
| Task 1 suite (gated) | `cargo nextest run --features full,oauth -E 'binary(oauth_discovery_urls)'` | **38 run, 38 passed** |
| Task 1 suite (**UNGATED proof**) | `cargo nextest run --features full -E 'binary(oauth_discovery_urls)'` | **38 run, 38 passed** |
| Task 2 suite (gated) | `cargo nextest run --features full,oauth -E 'binary(oauth_application_type)'` | **14 run, 14 passed** |
| Task 2 suite (**UNGATED proof**) | `cargo nextest run --features full -E 'binary(oauth_application_type)'` | **14 run, 14 passed** |
| both suites together | `-E 'binary(oauth_application_type) + binary(oauth_discovery_urls)'` | **52 run, 52 passed** |
| inline | `-E 'binary(pmcp) and test(oauth_validation)'` | **12 run, 12 passed** (8 pre-existing + 4 new) |
| doctests | `cargo test --features full,oauth --doc oauth_validation` | **12 passed** (5 pre-existing + 7 new) |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, 92 warnings (= 116-BASELINES anchor), **0** naming this file |
| lint (**authoritative**, D-116-LINT) | `/usr/bin/make lint` | **✓ No lint issues** |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** attributable |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| no forbidden imports | `grep -nE '^\s*use .*(reqwest\|webbrowser\|dirs)' src/shared/oauth_validation.rs` | **no output** (the only textual hits are 116-02's prose at line 26) |
| no `cfg` gates | `grep -nE '^\s*#\[cfg' src/shared/oauth_validation.rs` | only `#[cfg(test)]` |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over both new files and the module | **no output** |
| property (ALWAYS) | `/usr/bin/make test-property` | **✓ Property tests passed** |
| package gate | `/usr/bin/make pmcp-package-gate` | exit **0** |
| audit | `/usr/bin/make audit` | exit **0** — "No vulnerabilities found" |
| unused deps | `/usr/bin/make unused-deps` | exit **0** |
| SATD gate | `/usr/bin/make check-todos` | exit **0** — "No technical debt comments" |
| unwraps | `/usr/bin/make check-unwraps` | exit **0** — none in production code |
| purity | `/usr/bin/make purity-check` | exit **0** |
| comply | `/usr/bin/make comply` | exit **0** |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 2 — `test-unit` only**, and **equally red with this plan's source reverted** (see `D-116-KEYCHAIN`) |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`, not fixed here:

- **`D-116-KEYCHAIN` (new)** — `make test-unit` fails 14 `shared::streamable_http` tests on a macOS
  keychain `ioErr -36` at a pre-existing `.expect()` (`src/shared/streamable_http.rs:458`).
  **Measured pre-existing**: identical failing set with this plan's source reverted. Flaky
  (observed passing 3×), not disk-related this time, and correlated with running the full
  1844-test `--lib` binary — while `CLAUDE.md` documents `--test-threads=1` and `make test-unit`
  does not pass it. Proposed owner: `116-15`.
- **`D-116-DOC` (amended by measurement)** — the rule has an **inverse**: fully-qualify in inner
  `//!` blocks, use the BARE form in `///` item docs. Seven hard errors were produced by applying
  the `//!` rule to item docs. `116-05` creates a `src/shared/` module the same way and should
  apply both halves.
- **`D-116-LINT` (reconfirmed, 3 more measurements)** — clause (b) accepted `doc_markdown`,
  `needless_collect` and `trivially_copy_pass_by_ref` violations that `make lint` rejected as hard
  errors. Note `make lint` covers `--lib --tests`, so new **test files** are gated too.
- **`D-116-EX`** — still open and **not** discharged by this plan's 7 new doctests, for the same
  reason it was not discharged by 116-02's 5 or 116-03's 3.

## Next Phase Readiness

**116-06, 116-07 and 116-10 are unblocked.** Every symbol their plans name now exists, is public,
is documented and is tested at `pmcp::shared::oauth_validation::*`:

| Consumer | What it can now rely on |
|---|---|
| `116-06` (`src/client/auth.rs`) | replace the single `format!` at `:137-140` with `discovery_url_candidates`; wrap the existing `while attempts < max_retries` loop with `classify_discovery_failure` (the `Retry`-then-`Fallback` composition rule is written in `DiscoveryOutcome`'s rustdoc); call `issuer_matches_metadata` inside `fetch_discovery` **before the metadata escapes the function** (RESEARCH Pitfall 1); use `same_origin` to refuse a cross-origin discovery redirect |
| `116-07` (`generic_oidc.rs`, `cognito.rs`) | the same three, so all three call sites share one derivation and one matrix — which is the only way they cannot drift |
| `116-08` | `discovery_url_candidates(&str) -> Result<Vec<Url>>` and `derive_application_type(&[String])` are I/O-free and total; a `libfuzzer` target needs no harness. The generated proptest seeds are already committed |
| `116-10` | `derive_application_type` on whatever `redirect_uris` it is about to send, then `DcrRequest::set_application_type(t.as_str())`; a mixed vector fails loudly instead of guessing |
| `116-15` | must NOT cite `make quality-gate` exit 0 for this HEAD — see `D-116-KEYCHAIN`. Every other stage is green and individually cited above |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-06` | `T-116-09` is only *half* discharged — the comparison exists, the enforcement at the fetch site does not |
| `116-10` | `T-116-07` is still open — the derivation refuses a mismatch, but nothing calls it |
| `116-05` | apply **both halves** of the amended `D-116-DOC` rule; diff `make doc-check` against **28** before committing |
| every source-touching plan | run `make lint`, not clause (b) alone (`D-116-LINT`, now 3× measured) |
| `116-15` | resolve `D-116-KEYCHAIN`; close or waive `D-116-EX`; do not book `AUTH-02`/`AUTH-03` on this plan's evidence alone |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: src/shared/oauth_validation.rs                       (1376 lines, +733/-3)
FOUND: tests/oauth_discovery_urls.rs                        (607 lines, min_lines 90 ✓)
FOUND: tests/oauth_application_type.rs                      (210 lines, min_lines 70 ✓)
FOUND: tests/oauth_discovery_urls.proptest-regressions      (8 lines, tracked by git)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md (237 lines, was 164)
```

Commits claimed, verified in `git log`:

```
FOUND: 119eeaea  feat(116-04): SEP-2351 ordered discovery-URL probe and the RFC 8414 anchor
FOUND: 715f557b  feat(116-04): D-10 application_type derivation from redirect_uris
```

`must_haves` verification:

```
✓ truths[1] ORDERED candidate list, appended form last — both spec examples asserted as full
  ordered vectors (len 3 and len 2); the appended form is candidate 3 / candidate 2
✓ truths[2] path-bearing issuer emits the RFC 8414 inserted form FIRST and the OIDC appended
  form LAST — a_path_bearing_issuer_yields_the_three_spec_candidates_in_order
✓ truths[3] anchor mismatch detectable by a pure comparison with no normalization —
  issuer_matches_metadata + the spec worked attack + 4 normalization rows, all false
✓ truths[4] a mixed redirect_uris vector is an explicit error, never a silent pick —
  refused in BOTH orders, message names both URIs and both classifications
✓ truths[5] every discovery failure class has ONE written outcome; anchor mismatch, oversized
  body and malformed security metadata are TERMINAL — 8 matrix rows + a 2-directional property
✓ truths[6] userinfo / fragment / query / non-loopback http rejected before any URL is built —
  validate_issuer_url is called FIRST by discovery_url_candidates, asserted directly
✓ artifacts: src/shared/oauth_validation.rs contains "pub fn classify_discovery_failure" (:978)
  and provides all nine named symbols
✓ artifacts: tests/oauth_discovery_urls.rs 607 >= 90
✓ artifacts: tests/oauth_application_type.rs 210 >= 70
✓ key_links: "discovery_url_candidates" present in src/shared/oauth_validation.rs (7 references),
  one shared derivation for the 116-06 and 116-07 call sites
```

Plan-level verification block:

```
✓ both suites green with non-zero counts under --features full,oauth (38 and 14)
✓ both ALSO select non-zero under --features full alone (38 and 14) — the tier is ungated
✓ cargo build --target wasm32-unknown-unknown --no-default-features --features wasm — exit 0
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
⚠ make quality-gate — exit 2 at test-unit ONLY, measured EQUALLY red with this plan's source
  reverted (1826+14 vs 1830+14, identical failing set). Every other stage exits 0. D-116-KEYCHAIN
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
