---
phase: 116-auth-hardening-seps
plan: 09
subsystem: auth
tags: [oauth, rfc9207, rfc6749, csrf, pkce, browser-launcher, bounded-reads, mockito, semver, sep-2351]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 02
    provides: "validate_authorization_response, AuthorizationRequestRecord, IssPresence, iss_presence_from, parse_iss_env_value, MAX_CALLBACK_QUERY_BYTES, and the is_iss_mismatch/is_state_mismatch marker predicates"
  - phase: 116-auth-hardening-seps
    plan: 06
    provides: "OidcDiscoveryClient::discover_with_extras + AuthorizationServerExtras::iss_parameter_supported — the RFC 9207 flag, guaranteed None only when the key was ABSENT"
  - phase: 116-auth-hardening-seps
    plan: 07
    provides: "the restore-from-a-scratchpad-COPY discipline (never `git checkout --` on uncommitted work), applied three times here"
provides:
  - "The CLI authorization-code flow is the pure tier's first CALLER: state bound, iss anchored on metadata.issuer, validated INSIDE the listener before any response byte is committed"
  - "BrowserLauncher / SystemBrowserLauncher / with_browser_launcher — the platform seam that makes the interactive flow end-to-end testable for the first time in this crate's history"
  - "MAX_CALLBACK_REQUEST_LINE_BYTES (16 KiB) — the transport-level twin of MAX_CALLBACK_QUERY_BYTES"
  - "OAuthHelper::with_iss_validation + PMCP_OAUTH_ISS_VALIDATION — D-04's three-tier override, warn-on-unrecognised, absent means unchanged"
  - "is_terminal_authorization_refusal — a mix-up or CSRF refusal is no longer downgraded into a generic message, and no longer triggers the device-code fallback"
  - "D-116-GREP — two of this plan's own acceptance greps measure something other than their sentence says"
  - "D-116-LINT-OAUTH's TEST-side twin: make quality-gate runs ZERO of this plan's 25 tests"
affects: [116-10, 116-12, 116-13, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A security decision and the page that reports it must be consequences of ONE Result — validate before the first response byte, never after"
    - "A test seam that is also a platform seam: BrowserLauncher is documented as production API for headless runners, not hidden behind #[doc(hidden)]"
    - "When a wire-level test cannot distinguish a correct value from a plausible wrong one (both are 43 random base64url chars), assert the SOURCE property instead — and prove it by observing the shipped defect pass every wire test"
    - "Reproduce the PRE-FIX implementation as the negative control: the break is then the real defect, not an invented one"
    - "A new public field on an all-pub-field non-exhaustive struct is MAJOR; the same knob as an inherent consuming builder backed by a private field is semver-free"

key-files:
  created:
    - tests/oauth_state_csrf.rs
    - tests/oauth_iss_integration.rs
  modified:
    - src/client/oauth.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "The RFC 9207 anchor is metadata.issuer alone — NOT config.issuer and NOT the effective issuer reported to cache consumers, which is left untouched"
  - "SystemBrowserLauncher returns Ok even when the platform browser fails, preserving today's manual-URL fallback; the trait contract reserves Err for 'could not reach a human at all'"
  - "The record is CLONED into the listener task rather than shared behind an Arc — two short strings and a Copy enum"
  - "A validation refusal is TERMINAL: it propagates verbatim and never falls back to device code (Rule 2)"
  - "The two page literals were hoisted to consts; byte-identity with b2bf9157 verified by extraction and comparison, not by eyeballing a diff"
  - "AUTH-01 is NOT booked complete — 116-10/12/13/15/16 still claim it"

patterns-established:
  - "A grep-based acceptance criterion must have its baseline count measured when the plan is written, or it cannot distinguish a regression from a prefix collision (D-116-GREP)"
  - "D-116-LINT-OAUTH now has a test-side twin: a suite whose subject is oauth-gated cannot be made reachable by the current gate at all"

requirements-completed: []

# Metrics
duration: 173min
completed: 2026-08-04
---

# Phase 116 Plan 09: The CLI Flow Becomes a Caller of the Pure Tier Summary

**The CSRF `state` at `src/client/oauth.rs:712` was not an unchecked value — it was a
STRUCTURALLY UNCHECKABLE one. `.append_pair("state", &Self::generate_code_verifier())` never bound
the value to a variable, so no comparison was possible even in principle, and it reused the PKCE
verifier generator for a CSRF token. Both halves are closed, and the proof is that the shipped
pre-fix flow was OBSERVED serving the green "Authentication Successful!" page for a callback
carrying `state=wrong` and returning `Ok` from `authorize_with_details()` — a forged state
accepted, a page that lied about it, and the code exchanged. 11 of 13 end-to-end rows fail against
that ordering; the 2 survivors are the positive controls.**

**The `full,oauth` clippy baseline for this file was measured BEFORE the first edit: 29 errors, all
29 in `src/client/oauth.rs`, exactly the `D-116-LINT-OAUTH` anchor. After both tasks it is 24, with
ZERO new errors attributable to this plan — compared by error identity and source-line text, not by
line number, since every line in the file moved.**

## Performance

- **Duration:** ~173 min
- **Completed:** 2026-08-04
- **Tasks:** 2
- **Files:** 4 (2 created, 2 modified), **+1702 / −97** across the two task commits

## Accomplishments

- **The pre-fix ordering was reproduced verbatim and observed failing, which is stronger evidence
  than an invented break.** `target/116-verify/116-09-task2.PREFIX-RED.log`: **13 tests run, 2
  passed, 11 failed**, `--no-fail-fast`, denominator asserted. The listener body was replaced with
  the shipped pre-116-09 logic — extract only `code`, choose the page from `code.is_some()`, WRITE
  IT, then hand the value on with no validation. The quoted diagnostic is the whole plan in one
  assertion:

  ```
  assertion `left == right` failed: the failure page must be byte-identical …
    left: "HTTP/1.1 200 OK … <h1 style='color: green;'>Authentication Successful!</h1> …"
   right: "HTTP/1.1 400 Bad Request … <h1 style='color: red;'>Authentication Failed</h1> …"
  ```

  Six other rows failed on `this row must be a refusal` — i.e. `authorize_with_details()` returned
  `Ok` for a forged `state`, for an `iss` naming a different authorization server, and for an
  advertised-but-absent `iss`. T-116-32 (redeeming an unvalidated code) and T-116-32a (a success
  page for a rejected callback), both observed succeeding.

- **The headline `state` defect is INVISIBLE at the wire level, which is why the source assertions
  are load-bearing rather than decorative.** A second control restored the exact
  `.append_pair("state", &Self::generate_code_verifier())` line: **12 run, 10 passed, 2 failed** —
  and the two failures are both source-level. `the_authorization_url_carries_a_state_parameter`
  (43 chars, URL-safe base64) and `two_consecutive_flows_produce_different_state_values` **both
  still passed**, because a PKCE verifier and a `generate_state()` state are indistinguishable on
  the wire. A suite built only from wire assertions would have been green over the shipped defect.

- **The interactive flow is end-to-end testable for the first time.** `webbrowser::open()` was
  called directly, so no test could see the generated `state`, stop a real browser window opening
  on the developer's machine, or deliver a legitimate callback. The `BrowserLauncher` seam changes
  that: all 13 integration rows drive the REAL `authorize_with_details()`, with the launcher
  lifting the `state` out of the captured URL and performing a raw loopback GET against the flow's
  own listener. Every rejection row asserts all three of the marker predicate, the FAILURE page
  BYTES the browser received, and `expect(0)` + `assert_async()` on `/token`.

- **`make quality-gate` exits 0 end to end**, at 127 GiB free. `fmt-check` ✓, `lint` ✓
  ("No lint issues"), `build` ✓, `test-unit` **1880 passed / 0 failed** (unchanged from 116-07,
  as expected — this plan adds no inline lib tests), `test-doc` **445 passed / 0 failed / 79
  ignored**, `test-integration` ✓, `test-examples` ✓ (all examples built), team-servers binding
  check ✓.

- **Zero packages, zero breaking findings.** `cargo semver-checks check-release -p pmcp
  --baseline-rev b2bf9157`: **223 checks, 223 pass, 0 fail**, exit 0 — the seventh plan in this
  phase to see "no semver update required" despite genuinely new public API (a trait, a unit
  struct, a const and two inherent methods). `git diff --exit-code b2bf9157..HEAD -- Cargo.toml`:
  exit **0**.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Bind `state` into a per-request record, the D-04 override, the browser seam **and the listener rewrite** | `6bdf2afd` | feat |
| 2 | Terminal refusals + the 13-row end-to-end AUTH-01 suite | `c03cfe87` | feat |

## Files Created/Modified

- **`src/client/oauth.rs`** (**modified**, 1467 → **1993** lines, +623/−97). New public:
  `MAX_CALLBACK_REQUEST_LINE_BYTES`, `BrowserLauncher`, `SystemBrowserLauncher`,
  `OAuthHelper::with_iss_validation`, `OAuthHelper::with_browser_launcher`. New private:
  `ISS_VALIDATION_ENV_VAR`, `CALLBACK_SUCCESS_RESPONSE`, `CALLBACK_FAILURE_RESPONSE`,
  `resolve_iss_presence`, `is_terminal_authorization_refusal`, `get_metadata_with_extras`,
  `bind_callback_listener`, `build_authorization_url`, `read_request_line_within_cap`,
  `callback_query_from_request_line`, `serve_one_callback`,
  `await_validated_authorization_code`, plus two private `OAuthHelper` fields
  (`iss_validation`, `browser_launcher`). `discover_metadata` renamed to
  `discover_metadata_with_extras`; `get_metadata` kept its signature and now delegates.
- **`tests/oauth_state_csrf.rs`** (**created**, **479** lines — `min_lines` 110 ✓). **12** tests in
  four documented groups.
- **`tests/oauth_iss_integration.rs`** (**created**, **697** lines — `min_lines` 160 ✓). **13**
  tests in five documented groups.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (602 → **~740**) — three new
  entries (`D-116-GREP`, `D-116-FALLBACK`, and `D-116-LINT-OAUTH`'s test-side twin).

## Decisions Made

- **The RFC 9207 anchor is `metadata.issuer`, and nothing else.** Not `config.issuer` (a user-typed
  discovery seed), and not the `effective_issuer` that `authorize_with_details` reports to cache
  consumers — that value is left exactly as it was, with a comment at the site saying it is
  deliberately not the comparison anchor. The attack is "this response came from a different
  authorization server than the one whose metadata I fetched", so the DISCOVERED issuer is the
  semantically correct anchor; `116-06` made it trustworthy by validating it against the URL it was
  fetched from.
- **`SystemBrowserLauncher::open` returns `Ok(())` even when `webbrowser::open` fails.** Two
  behaviour rows in the plan pull in opposite directions — "the default is today's behaviour,
  unchanged" and "a launcher that returns `Err` propagates". Today's behaviour is to warn and
  continue, because the flow has already logged "If the browser doesn't open, visit: …" and a human
  can complete the flow by pasting the URL. Propagating there would delete a working manual path.
  The trait's contract is therefore stated as "`Err` means the URL could not be delivered to a
  human **at all**", which the system launcher never is, and which a queue-posting launcher might
  be. Both rows hold.
- **The record is CLONED into the listener task.** `AuthorizationRequestRecord` is two short
  strings, an issuer and a `Copy` enum; an `Arc` would add a second ownership story for no benefit,
  and the task needs `'static` either way. The clone is also what keeps the parent's copy available
  for `record.code_verifier()` at the token exchange, so the verifier that is sent is provably the
  one the challenge was derived from.
- **`BrowserLauncher` is documented as a PLATFORM seam and deliberately not `#[doc(hidden)]`.**
  D-05 says in as many words that the interactive CLI flow is one caller and not the only one; a
  headless CI runner, a display-less container, or a hosting platform relaying the URL through its
  own UI all want to print or forward rather than open a window. The rustdoc says so, so a later
  reader does not mistake it for test scaffolding and hide it.
- **The two page literals were hoisted to module consts.** Byte-identity with `b2bf9157` is
  therefore not visible as "no diff" — the literals moved. It was verified by EXTRACTING both
  literals from `git show b2bf9157:src/client/oauth.rs` and from the current file, collapsing Rust
  string continuations, and comparing: **success 263 chars BYTE-IDENTICAL, failure 261 chars
  BYTE-IDENTICAL**. `both_pages_are_byte_identical_to_the_pages_this_module_has_always_served`
  pins the same thing at the wire level against a hard-coded copy of the pre-fix bytes.
- **Complexity was managed by extraction, not by annotation.** `authorization_code_flow_inner` was
  ~150 lines; the listen-read-validate-respond block became four private helpers and the flow
  function is now ~55 lines of straight-line steps. `pmat quality-gate --checks complexity`:
  **0 violations**, and `grep -c cognitive_complexity src/client/oauth.rs` is **0** — no new
  `#[allow]` was added.
- **`AUTH-01` is NOT booked complete.** This plan lands the CLI wiring; `116-10`, `116-12`,
  `116-13`, `116-15` and `116-16` still claim it. `requirements-completed: []`, as in `116-01`
  through `116-07`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's `grep -n 'pub iss'` acceptance criterion cannot pass at any HEAD**

- **Found during:** Task 1, on the first run of the suite: `12 run, 11 passed, 1 failed`.
- **Issue:** the criterion is *"`grep -n 'pub iss' src/client/oauth.rs` shows no new public field on
  `OAuthConfig`"*. `OAuthConfig` has always had `pub issuer: Option<String>`, and `pub iss` is a
  PREFIX of `pub issuer`. A test written to the criterion's literal wording fails against a
  perfectly correct tree — which is what happened.
- **Why it mattered more than a red test:** written the other way round (a positive "the field is
  absent" check phrased as a `contains`) the same collision would have made it pass VACUOUSLY.
- **Fix:** `oauth_config_gained_no_public_iss_field` now parses the `OAuthConfig` body and asserts
  **the exact eight-field set by name**, plus that no field starts with `iss_`. That is the
  invariant the criterion was reaching for, and it is a real detector.
- **Committed in:** `6bdf2afd`. Written up as **`D-116-GREP`**, together with a second instance:
  `grep -n 'validate_authorization_response'` returns **2**, not 1, because line 38 is the `use`
  declaration and line 1064 is the call.

**2. [Rule 2 — Missing critical functionality] A security refusal was downgraded into "no supported OAuth flow available"**

- **Found during:** Task 2, writing the behaviour rows that assert `err.is_iss_mismatch()` on the
  error the flow RETURNS.
- **Issue:** both callers of the authorization-code flow wrapped ANY failure — falling back to
  device code when the server advertises one, otherwise replacing the error with the fixed string
  *"No supported OAuth flow available."*. Once the flow started returning `Error::iss_mismatch` /
  `Error::state_mismatch`, that wrapper did two harmful things at once: it destroyed the stable
  programmatic identity `116-02` built the markers for (pushing callers straight back to substring
  matching, against a substring that does not even mention `iss`), and it re-attempted
  authentication through a different grant against a server whose response had just failed a
  mix-up or CSRF check.
- **Fix:** `OAuthHelper::is_terminal_authorization_refusal` — an `iss` or `state` mismatch
  propagates verbatim from both `get_access_token` and `authorize_with_details`. Every other
  failure keeps its existing fallback behaviour untouched, so no working deployment changes.
- **Committed in:** `c03cfe87`. The un-marked refusals (duplicate parameter, over-cap query,
  over-cap request line, unparseable target) are still wrapped — they are still refusals and the
  `expect(0)` proofs hold, but a caller cannot yet tell them from "the endpoint was unreachable".
  Deferred as **`D-116-FALLBACK`**, because a fourth error identity is `116-02`'s subsystem.

**3. [Rule 3 — Blocking] Task 2's source-side work landed inside Task 1's commit**

- **Found during:** Task 1, implementing the record.
- **Issue:** the plan splits "bind the record + open the seam" (Task 1) from "rewrite the listener"
  (Task 2). Task 1's own behaviour rows require the flow to RUN end to end through the launcher, so
  the callback path has to already carry the record and the `Result`-typed channel. Committing
  Task 1 alone would have left an intermediate commit whose `record` is constructed and unused —
  the `dead_code`-red transient `116-06` had to record for its own two-commit split, and this time
  it would have been avoidable.
- **Fix:** `6bdf2afd` contains the full listener rewrite; `c03cfe87` contains the 13-row suite that
  proves it plus the Rule 2 fix that writing the suite exposed. Both commits build clean and both
  pass `make lint`. Recorded here rather than hidden, because a reader diffing the plan's task
  boundaries against `git log` will notice.

**Total deviations:** 3 (1 × Rule 1, 1 × Rule 2, 1 × Rule 3). No Rule 4 situation arose; no
architectural change was needed. **Zero dependencies added.**

## Issues Encountered

- **`make quality-gate` runs ZERO of this plan's 25 tests, and still exits 0.** Measured at
  `c03cfe87`: `cargo nextest list --features full` selects **0** tests from
  `binary(oauth_iss_integration) + binary(oauth_state_csrf)`, and **25** under `--features
  full,oauth`. `116-06`'s and `116-07`'s suites select **34** under plain `full`, because they gate
  on `http-client`; this plan's subject is `OAuthHelper`, which lives behind `oauth`, so there is
  no gating choice that makes them reachable. This is `D-116-LINT-OAUTH`'s test-side twin and it is
  worse than the lint side: the un-measured thing is a security proof, including every
  `expect(0)`-on-`/token` assertion. Logged with the measurement and a PAIRED resolution — clear
  the 24 remaining pre-existing clippy errors in `src/client/oauth.rs` FIRST, then add
  `--features "full,oauth"` to `make lint` and the gate's test stage, because doing the second
  without the first turns the gate red.
- **`D-116-LINT-OAUTH`'s count moved from 29 to 24, downward, with no fixes attempted.** The 5 that
  disappeared are `doc_markdown` hits on three doc lines inside `authorization_code_flow_inner`
  that this plan rewrote (`/// Returns (TokenResponse, resolved_client_id) …` and its two
  continuations). They were not "fixed" as a side quest — the doc had to be rewritten because the
  function gained a parameter, and the rewrite happened to backtick the identifiers. The anchor for
  the next plan touching this file is therefore **24**, not 29.
- **Attribution across a file where every line moved required comparing error IDENTITY, not line
  number.** Both clippy logs were reduced to `(error message, offending source line text)` pairs
  and diffed as multisets. NEW: **0**. GONE: **5**. A line-number diff would have reported all 29
  as changed and proved nothing.
- **`D-116-FUZZGATE` reconfirmed, unchanged.** Inside this plan's gate run, `make test-fuzz`
  produced **21** × ``the option `Z` is only accepted on the nightly compiler`` and the gate still
  exited 0. Not this plan's to fix.
- **`D-116-FAILFAST` applied.** Every negative control and every regression run used
  `--no-fail-fast` with the denominator asserted (12, 12, 13, 13, 79, 25). Two of the four controls
  would have been truncated without it.
- **One `(1 leaky)` warning from nextest** on the combined `oauth_iss_integration +
  oauth_state_csrf` run. It is the callback-driving launcher's spawned reader task outliving the
  test that spawned it by a few milliseconds; the suites are green standalone and inside the
  combined run. Named so a later reader does not treat it as a new signal.
- **`git commit -m` with a multi-paragraph message is unreliable in this environment** — `116-06`
  recorded it and it was hit again. Both task commits used `git commit -F <file>`.
- **`rtk` aggregates `cargo clippy` output and hides `^error` lines.** Every command whose output
  this plan counted was invoked through `$HOME/.cargo/bin/cargo` or `/usr/bin/make`.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: new-extension-point | `src/client/oauth.rs` | `BrowserLauncher` is a new PUBLIC trait that receives the full authorization URL — which carries the CSRF `state` and the PKCE `code_challenge`. Before this plan only `webbrowser::open` saw that string. A third-party launcher implementation therefore sees a per-request secret. This is inherent to the seam (a launcher that cannot see the URL cannot deliver it) and is the same exposure the pre-existing `tracing::info!("If the browser doesn't open, visit: {}")` line already has, but it is NEW public surface and is named here rather than left for a reader to discover. |

All `mitigate` dispositions in the plan's `<threat_model>` are discharged by a named test:

| Threat | Discharged by |
|---|---|
| T-116-30 (authorization-server mix-up) | `iss` compared against `metadata.issuer` inside the listener; 4 table rows end to end, each `expect(0)` on `/token`; OBSERVED returning `Ok` pre-fix |
| T-116-31 (CSRF / code injection) | `generate_state()`, bound in the record, compared FIRST; the `:712` defect grep-asserted absent AND observed passing every wire test, which is why the source assertion exists |
| T-116-32 (redeeming an unvalidated code) | the channel carries `Result`; 10 rejection rows each with `expect(0)` + `assert_async()` on `/token`; observed violated under both negative controls |
| T-116-32a (a success page for a rejected callback) | validation at `:1064`, first response byte at `:1075`; every rejection row asserts the FAILURE bytes, the happy path asserts the SUCCESS bytes, and `both_pages_are_byte_identical…` asserts both verbatim. OBSERVED failing: the pre-fix flow served the green page for `state=wrong` |
| T-116-33 (`error_description` disclosure) | evaluation order; a planted `CANARY-1f4a9c-DO-NOT-DISCLOSE` asserted absent from BOTH the returned error and the served bytes |
| T-116-33a (unbounded request line) | `MAX_CALLBACK_REQUEST_LINE_BYTES` applied by a `take()`-limited reader; `grep -c read_line` → **0**; the 64 KiB row asserts the refusal reproduces none of the bytes |
| T-116-33b (duplicate `state`/`iss`) | refused by the pure validator; exercised end to end with the FAILURE page and `expect(0)` |
| T-116-34 (strictness breaking v1 deployments) | `an_absent_iss_against_a_silent_server_proceeds` — the D-01 floor, asserted to REACH `/token` with `expect(1)` |
| T-116-34a (unrecognised env value failing open) | `parse_iss_env_value` + a `tracing::warn!` naming the variable and both accepted values; 7 plausible-but-wrong values asserted to fall through without defeating the builder |
| T-116-SC (cargo installs) | zero packages; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0** |

## Known Stubs

None. Every item is fully implemented and exercised. `grep -nE 'TODO|FIXME|HACK|XXX'` over all three
source files returns **0**. No placeholder value, empty collection or "not available" string was
introduced.

## TDD Gate Compliance

Both tasks carry `tdd="true"`. **RED was observed and logged for both.** For Task 2 the "break" was
not injected at all — it was the shipped pre-116-09 listener, reproduced verbatim.

| Task | Control log | Result |
|---|---|---|
| 1 | `116-09-task1.NEGATIVE-CONTROL.log` | **12 run, 8 passed, 4 failed** — three breaks at once |
| 1 | `116-09-task1.D12-RESTORED.log` | **12 run, 10 passed, 2 failed** — the exact shipped defect |
| 2 | `116-09-task2.PREFIX-RED.log` | **13 run, 2 passed, 11 failed** — the pre-fix ordering |
| 2 | `116-09-task2.REDEEM-ANYWAY.log` | **13 run, 2 passed, 11 failed** — correct page, redeem anyway |

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01`
(`ea1d2d68`) through `116-07`: in Rust a test naming a non-existent item fails to *compile*, so such
a commit leaves a non-building tree that breaks `git bisect` and contradicts CLAUDE.md's "ZERO
TOLERANCE FOR DEFECTS". A verifier looking for a `test(...)` → `feat(...)` pair will not find one;
the evidence is the four control logs above.

### Negative control — Task 1, three breaks at once (`--no-fail-fast`, denominator asserted)

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| `state` becomes the PKCE **challenge** (a wrong value that is ALSO 43 base64url chars) | `the_flow_uses_the_shared_state_generator_and_the_bound_record_value`, `the_authorization_url_also_carries_the_pkce_challenge` | `the_authorization_url_carries_a_state_parameter` and `two_consecutive_flows_produce_different_state_values` both held — a length-and-uniqueness test is NOT a detector for this defect class |
| the env override read in `OAuthHelper::new` instead of the flow | `the_env_override_is_read_inside_the_flow_and_warns_on_an_unrecognised_value` **only** | both pure precedence tests and the end-to-end env test held — constructor purity is its own independent detector |
| `webbrowser::open` mentioned a second time, in a COMMENT | `webbrowser_is_reached_only_through_the_launcher_seam` **only** | everything else held. This also reproduces `116-06`'s "prose trips the audit grep" hazard on purpose |

### Negative control — Task 1, the shipped D-12 defect restored verbatim

`.append_pair("state", &Self::generate_code_verifier()); // Random state for CSRF protection`
put back byte-for-byte: **10 passed, 2 failed**. The two failures are
`the_state_is_no_longer_an_unnamed_temporary_from_the_verifier_generator` and
`the_flow_uses_the_shared_state_generator_and_the_bound_record_value` — **both source-level**.
Every wire-level assertion passed. This is the single most important attribution result in the
plan: it proves the defect this phase exists to remove is undetectable from the wire.

### Negative control — Task 2, the pre-fix ordering

11 of 13 failed. The 2 survivors are `the_happy_path_serves_the_success_page_and_calls_the_token_endpoint`
and `an_absent_iss_against_a_silent_server_proceeds` — precisely the two rows where "extract `code`,
serve success, redeem" happens to be the correct behaviour. That is the attribution argument.

### Negative control — Task 2, "serve the right page, redeem anyway"

11 of 13 failed. **Recorded honestly: this control did NOT isolate the `expect(0)` mock as an
independent detector**, because making redemption reachable also makes `authorize_with_details()`
return `Ok`, so the outcome assertions fire first. The page assertions ARE independent — they held
under this break and failed under the pre-fix break. A future plan wanting to isolate the token
mock alone would need a break that keeps the flow returning `Err` while still calling `/token`.

Source restored from a scratchpad COPY after each control, never `git checkout --` (116-07's
process incident). `shasum -a 256 -c` returned **OK** all four times, and the break sites were
re-verified absent by grep.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| Task 1 suite | `-E 'binary(oauth_state_csrf)'`, `--features full,oauth` | **12 run, 12 passed** |
| Task 2 suite | `-E 'binary(oauth_iss_integration)'`, `--features full,oauth` | **13 run, 13 passed** |
| both, combined | same | **25 run, 25 passed** (1 leaky) |
| **narrow-gate reality** | `nextest list --features full`, both binaries | **0** — see *Issues Encountered* |
| no regression | `oauth_dcr_integration + oauth_iss_validation + oauth_discovery_validation + oauth_provider_discovery + v2_bounded_reads_tripwire` | **79 run, 79 passed** |
| DCR suite specifically | `-E 'binary(oauth_dcr_integration)'` | **5 run, 5 passed** |
| **bounded-reads tripwire** | `-E 'binary(v2_bounded_reads_tripwire)'` | **13 run, 13 passed** |
| **clippy baseline, measured BEFORE any edit** | `make lint`'s command with `--features "full,oauth"` | **29 errors, all 29 in `src/client/oauth.rs`**, exit 101 = the `D-116-LINT-OAUTH` anchor |
| **clippy after both tasks** | same | **24 errors, all 24 in `src/client/oauth.rs`** — **0 NEW**, 5 GONE, compared by error identity |
| lint (**authoritative**) | `/usr/bin/make lint` | **exit 0**, "No lint issues" (run after each task) |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** (twice); `grep -c cognitive_complexity` → **0** |
| one call site | `grep -n 'validate_authorization_response' src/client/oauth.rs` | **2** hits: `:38` the `use`, `:1064` the call. Validation at **:1064**, first response byte at **:1075** |
| no unbounded read | `grep -c 'read_line' src/client/oauth.rs` | **0** |
| cap applied | `grep -c 'MAX_CALLBACK_REQUEST_LINE_BYTES'` | **5** (const, doc, `take()`, bound check, message) |
| channel carries `Result` | `grep -c 'tx.send(code)'` | **0** |
| single browser call | `grep -c 'webbrowser::open'` | **1**, inside `SystemBrowserLauncher::open` |
| HTML byte-identity | literals extracted from `b2bf9157` and from HEAD, continuations collapsed, compared | **success 263 chars IDENTICAL, failure 261 chars IDENTICAL** |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over all three files | **no output** |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** attributable |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, **92** warnings (= 116-BASELINES anchor) |
| gate: `test-unit` | inside `make quality-gate` | **1880 passed; 0 failed** |
| gate: `test-doc` | inside `make quality-gate` | **445 passed; 0 failed; 79 ignored** |
| gate: `test-examples` | inside `make quality-gate` | all examples built ✓ |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 0** |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

Operators gain one optional lever: `PMCP_OAUTH_ISS_VALIDATION=strict` (or `lenient`). **Absent
means unchanged behaviour**, so no action is required of anyone.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-LINT-OAUTH` — test-side twin (new, and the most consequential).** `make quality-gate`
  runs **0** of this plan's 25 tests. Resolution must be taken as a PAIR: clear the 24 remaining
  pre-existing `src/client/oauth.rs` clippy errors FIRST, then add `--features "full,oauth"` to
  `make lint` and to the gate's test stage. Owner: `116-15`.
- **`D-116-GREP` (new)** — two of this plan's own acceptance greps measure something other than
  their sentence says (`pub iss` collides with the pre-existing `pub issuer`; a bare symbol grep
  also matches its `use`). Proposed convention: an acceptance grep must have its baseline count
  measured when the plan is written. Owner: `116-15`.
- **`D-116-FALLBACK` (new)** — the four un-marked callback refusals are still wrapped in the
  generic "no supported OAuth flow available" message, so a caller cannot distinguish "the callback
  arrived and was refused" from "the flow never ran". Needs a fourth error identity, which is
  `116-02`'s subsystem. Owner: `116-15` or a `116-02` follow-up.
- **`D-116-FUZZGATE`** — reconfirmed inside this plan's gate run (21 nightly failures, all
  swallowed, gate still 0). Still open for `116-15`.
- **`D-116-LINT`** — no new measurement: `make lint` was **exit 0** on the first run after each
  task here, which is the first time in this phase that has happened. The standing obligation was
  met, not violated.
- **`D-116-SLASH`, `D-116-KEYCHAIN`, `D-116-TRIPWIRE`, `D-116-DISK`, `D-116-EX`, `D-116-DOC`** —
  unchanged; nothing here reopens any of them. `D-116-KEYCHAIN` did not reproduce (1880/0 at
  127 GiB free), a third independent clean observation.

## Next Phase Readiness

| Consumer | What it can now rely on |
|---|---|
| `116-10` | `src/client/oauth.rs` has a **24**-error `full,oauth` clippy anchor (not 29 — measure against 24). The flow now threads a resolved `IssPresence`; `application_type` wiring should follow the same "resolve in the caller, pass as a parameter" shape rather than reading state inside `authorization_code_flow_inner` |
| `116-12` | the DCR read at `src/client/oauth.rs:281-291` is still the pre-116-06 `bytes()`-then-measure form and is untouched by this plan; `read_request_line_within_cap` here is the *transport-line* analogue, not a substitute for `collect_reqwest_body_within_cap`. `is_terminal_authorization_refusal` is the hook for making a refresh-scope refusal terminal too |
| `116-13` | new public API to name in release notes: `BrowserLauncher`, `SystemBrowserLauncher`, `MAX_CALLBACK_REQUEST_LINE_BYTES`, `OAuthHelper::with_iss_validation`, `OAuthHelper::with_browser_launcher`, and the `PMCP_OAUTH_ISS_VALIDATION` env var. Plus **one behaviour-change line**: an `iss`/`state` refusal now propagates instead of falling back to device code. `cargo semver-checks` again says "no semver update required" (seventh observation) — do not rest the bump on it |
| `116-15` | `make quality-gate` **exit 0** at this HEAD, second consecutive clean full-gate run. Three new deferred entries, one of which (`D-116-LINT-OAUTH` test-side) is a genuine CI coverage hole rather than an advisory |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-15` | close `D-116-LINT-OAUTH` as a PAIR (clear 24 errors, then enable `oauth` in lint AND tests) — 25 security tests are currently outside CI |
| `116-15` | close or waive `D-116-GREP` and `D-116-FALLBACK`; do not book `AUTH-01` on this plan's evidence alone |
| `116-10`, `116-12` | measure the `full,oauth` clippy baseline for `src/client/oauth.rs` BEFORE editing; it is **24** at `c03cfe87` |
| every source-touching plan | `make lint`, not clause (b) alone; `--no-fail-fast` with the denominator asserted; restore from a scratchpad COPY, never `git checkout --`; absolute binary paths for anything whose output you count |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: src/client/oauth.rs                                        1993 lines (was 1467)
FOUND: tests/oauth_state_csrf.rs                                   479 lines (min_lines 110 ✓)
FOUND: tests/oauth_iss_integration.rs                              697 lines (min_lines 160 ✓)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md  (+3 entries)
```

Commits claimed, verified in `git log`:

```
FOUND: 6bdf2afd  feat(116-09): bind the CSRF state into a per-request record and open the browser seam
FOUND: c03cfe87  feat(116-09): a validation refusal is terminal, and 13 end-to-end AUTH-01 rows
```

`must_haves` verification:

```
✓ truths[1] state generated, bound to a per-request record, compared on the callback before
  anything else — AuthorizationRequestRecord::new at :1142; validate_state runs first inside
  validate_authorization_response; OBSERVED accepting a forged state pre-fix
✓ truths[2] iss validated against the issuer the AS published in its OWN discovery document —
  the record's expected_issuer is metadata.issuer, not config.issuer, with the reasoning at
  the construction site; four table rows end to end
✓ truths[3] validation completes BEFORE any byte of the browser response is committed —
  validate_authorization_response at :1064, first write_all at :1075; every rejection row
  asserts the FAILURE bytes and the happy path asserts the SUCCESS bytes
✓ truths[4] on an iss or state failure the code is NEVER exchanged — the oneshot carries
  Result; 10 rejection rows with expect(0) + assert_async() on /token; OBSERVED violated
✓ truths[5] operator can force strictness with an env var, no redeploy, unrecognised value
  announced — PMCP_OAUTH_ISS_VALIDATION read at the call site, parse_iss_env_value, warn on
  the None branch naming the variable and both accepted values; 7 wrong values covered
✓ truths[6] deterministic test seam — BrowserLauncher; 25 tests drive the real flow with no
  browser and no human
✓ artifacts: src/client/oauth.rs provides the record, bound state, validate-then-respond
  listener, with_iss_validation, PMCP_OAUTH_ISS_VALIDATION and BrowserLauncher, and contains
  "AuthorizationRequestRecord" (5 references)
✓ artifacts: tests/oauth_state_csrf.rs 479 >= 110, D-12 covered, mismatch half in the sibling
✓ artifacts: tests/oauth_iss_integration.rs 697 >= 160, expect(0) negative control and the
  served-HTML branch assertions both present
✓ key_links: validate_authorization_response called INSIDE the listener before the response is
  written (:1064 vs :1075)
✓ key_links: discover_with_extras supplies the RFC 9207 flag (2 call sites, :650 and :694)
```

Plan-level verification block:

```
✓ binary(oauth_iss_integration) 13/13 and binary(oauth_state_csrf) 12/12, non-zero counts
✓ binary(oauth_dcr_integration) still green — 5/5, no regression
✓ four fences recorded as OBSERVED failing, including the shipped pre-fix implementation twice
✓ make quality-gate exit 0; make lint exit 0; full,oauth clippy 24 vs a 29 pre-measured
  baseline with ZERO new errors attributable
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations, no new allow
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
✓ binary(v2_bounded_reads_tripwire) — 13 run, 13 passed
✓ wasm32 build — exit 0, 92 warnings = the 116-BASELINES anchor
⚠ the 25 tests above are NOT reachable by make quality-gate (D-116-LINT-OAUTH test-side twin),
  measured and logged rather than left implicit
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
