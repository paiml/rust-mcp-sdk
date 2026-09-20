---
phase: 116-auth-hardening-seps
plan: 12
subsystem: auth
tags: [oauth, d-14, d-08, d-15, rfc6749, sep-2207, refresh, headless, interactivity, bounded-reads, pkce, token-logging, mockito, proptest]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 11
    provides: "the issuer-keyed store wiring that puts the DCR-issued client_id and the GRANTED scopes in the record refresh now reads; the 17-error full,oauth clippy anchor; D-116-PLANCONFLICT"
  - phase: 116-auth-hardening-seps
    plan: 06
    provides: "collect_reqwest_body_within_cap / is_body_over_cap / DEFAULT_AUTH_RESPONSE_BYTES — the streaming bounded read all six whole-body sites now use"
  - phase: 116-auth-hardening-seps
    plan: 16
    provides: "FileCredentialStore, the default store behind the records these tests seed"
  - phase: 116-auth-hardening-seps
    plan: 02
    provides: "Error::reauth_required / is_reauth_required / reauth_issuer — the typed identity RefreshOnly rides"
provides:
  - "A refresh that survives an authorization server which omits refresh_token, works for a DCR-registered client, and carries exactly the GRANTED scope or none"
  - "Interactivity::RefreshOnly — headless operation as an explicit mode, with the browser path unreachable by construction and both public entry points guarded"
  - "StoreOutcome / StoreMiss — the credential-store miss now carries WHY, because a headless caller cannot go and look"
  - "token_fingerprint — sha256-prefix token logging, replacing a plaintext 20-character prefix of a live access token"
  - "src/client/oauth.rs with ZERO whole-body reads, ZERO PKCE duplicates, ZERO rand:: uses and ZERO plaintext token logs"
  - "The measured finding that the plan's own single-line grep missed THREE of the SIX whole-body reads, because rustfmt splits the chain (D-116-GREP, fifth instance)"
  - "D-116-KEYCHAIN REOPENED with the disk theory refuted: it reproduced at 92 GiB free, and reproduces identically against the PRE-PLAN source"
affects: [116-13, 116-14, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Make a capability unreachable by CONSTRUCTION, not by an `if`: the refusing arm calls an associated function with no `self`, so it holds no launcher and no port and cannot reach the listener"
    - "Pass the record the caller already loaded rather than re-reading the store: a refresh token and the client_id it was issued to are ONE pairing, and a second load can pair them wrongly"
    - "A line-oriented grep over a rustfmt-formatted file is not an audit — three of six violations were split across lines and invisible"
    - "A property test derived from a specification sentence (RFC 6749 §6) rather than from the implementation, generated with proptest strategies and driven through a real wire body"
    - "When a negative control masks a second break, isolate the second break and run it alone rather than reporting the pair"

key-files:
  created:
    - tests/oauth_refresh.rs
  modified:
    - src/client/oauth.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "refresh_token takes the stored client_id and granted scopes as PARAMETERS from the record the caller already loaded, rather than performing the second store.load the plan's <action> specifies — same key, same source, no TOCTOU window"
  - "authorize_with_details also refuses under RefreshOnly, before any I/O: without it the mode's guarantee has a hole a caller escapes by picking the other public entry point"
  - "token_from_store returns StoreOutcome/StoreMiss instead of Option<String>, because RefreshOnly must report WHICH of three conditions occurred — their operator fixes differ"
  - "The DCR over-cap refusal keeps its DCR-specific framing so the existing dcr_rejects_response_larger_than_1mib stays a real detector for the 1 MiB VALUE"
  - "The four-stage SEP-2207 comment KEEPS the identifier `config.scopes`, so the plan's acceptance grep is reported as 3 PROSE hits rather than forced to zero — see Deviations"
  - "AUTH-03 is NOT booked complete: 116-13, 116-14 and 116-15 still claim it"

patterns-established:
  - "A test that PASSED pre-fix is a positive control, not coverage — six of this plan's thirteen Task 1 rows did, and each is named"
  - "Prove a security-mode guarantee with TWO observables (a recording launcher count AND port bindability), never with wall clock, which passes on a fast machine for the wrong reason"

requirements-completed: []

# Metrics
duration: 300min
completed: 2026-08-05
---

# Phase 116 Plan 12: The D-14 Refresh Defects and Headless Operation Summary

**An unattended agent can now keep working. A DCR-registered client can refresh at all — its
`client_id` is read from the credential record instead of from a config field it was never in. A
refresh carries exactly the scope the authorization server GRANTED, or none, never a scope RFC
6749 §6 forbids it to ask for. And a headless caller that cannot complete a browser login gets a
typed `reauth_required` in milliseconds instead of a loopback listener nothing can reach and a
five-minute wait — with the interactive path unreachable by construction rather than skipped by an
`if`, at BOTH public entry points.**

**The finding worth the plan is that the plan's own audit grep was blind to half the population it
was auditing.** Task 3's acceptance criterion greps for four single-line needles and its
`<action>` names "the six unbounded whole-body reads" but then lists three sites. There were
**six**. `rustfmt` splits a long chain across lines, so `.bytes()\n.await` never matched
`\.bytes\(\)\.await` — and the grep returned **0 both before and after** those three were fixed.
It could never have distinguished a clean file from a dirty one.

**Two of D-14's three defects were real; the first was already closed and is reported as such.**
`an_omitted_refresh_token_in_the_response_preserves_the_stored_one` **PASSED against the pre-fix
tree**: 116-11's `token_from_store` rewrite had already made an omitted `refresh_token` preserve
the stored one. It is kept as a regression fence and PROVEN to be a detector by negative control A,
rather than being counted as this plan's work.

## Performance

- **Duration:** ~300 min
- **Completed:** 2026-08-05
- **Tasks:** 3
- **Files:** 3 (1 created, 2 modified), **+2014 / −79** across the three task commits

## Accomplishments

- **`Interactivity::RefreshOnly` is unreachable-by-construction, and that is structural rather
  than asserted.** `get_access_token` ends in a two-arm `match` whose `RefreshOnly` arm calls
  `refresh_only_refusal` — an **associated function with no `self`**, so it holds no
  `BrowserLauncher`, no `redirect_port` and no route to `authorization_code_flow_inner`. The
  interactive tail moved into `interactive_token`, called from exactly one place. A reviewer reads
  one `match` and one call site. Negative control D (making `RefreshOnly` fall through anyway)
  fails the three refusal rows.

- **The guarantee is proved with two observables, never with wall clock.** Every `RefreshOnly`
  refusal row asserts a recording `BrowserLauncher` count of **0** AND that the `redirect_port` is
  still bindable immediately afterwards — the second catches a listener bound and dropped, which
  the count alone would miss. The elapsed-time assertion is present but LAST and loose, labelled
  in the source as corroboration: a timing assertion passes on a fast machine for the wrong reason.

- **The RFC 6749 §6 property test is derived from the specification and is not decorative.**
  "The scope of the access request MUST NOT include any scope not originally granted" is asserted
  as set containment over the WIRE BODY, over 24 `proptest`-generated grant sets driven through
  real refreshes. Under negative control B (a fall-back to `config.scopes` on an empty grant) it
  FAILED with `sent [...] includes "openid", which was never granted ([])` — so it detects a real
  widening rather than passing vacuously.

- **Six whole-body reads are bounded, not three.** All six now go through
  `collect_reqwest_body_within_cap`: the refresh error and success bodies, the DCR success body,
  the device-authorization error and success bodies, and the device-code POLL body — the last of
  which is inside a loop, so an unbounded read there was an unbounded read *per poll* for the life
  of the device code. The DCR site also stopped being a post-hoc `.bytes()`-then-measure check,
  which bounded what was ACCEPTED and not what was ALLOCATED (D-113-V).

- **No token prefix is logged.** `create_middleware_chain` printed the first 20 characters of a
  LIVE access token at debug level; it now logs `sha256:` + 12 hex digits of the digest. Two of the
  six inline tests assert **absence** over every token prefix of 4 characters or more, in both
  directions — a presence assertion is not a detector for a leak channel (116-10). Under negative
  control H (reverting to the plaintext prefix) five of six fail; the survivor is
  `a_fingerprint_is_stable_for_one_token`, correctly, because a plaintext prefix is also stable.
  That is recorded rather than presented as coverage.

- **`make quality-gate` exits 2 on `D-116-KEYCHAIN`, and the attribution is settled three ways.**
  14 failures, all in `shared::streamable_http::tests`, all panicking at the pre-existing
  `.expect` at `src/shared/streamable_http.rs:458` on macOS keychain `Os(Error { code: -36 })`.
  See *Issues Encountered* — including the measurement that **refutes the disk theory**.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | The three D-14 refresh defects, plus scope on refresh | `70a88d7e` | feat |
| 2 | `Interactivity::RefreshOnly`, the browser path unreachable by construction | `be9705fe` | feat |
| 3 | Close the remaining hygiene debt in `src/client/oauth.rs` | `464533ac` | feat |

## Files Created/Modified

- **`src/client/oauth.rs`** (**modified**, 3272 → **3760** lines, **+567 / −79**). New public:
  `Interactivity` (`#[non_exhaustive]`, `Interactive` = `#[default]`, `RefreshOnly`),
  `OAuthHelper::with_interactivity` — **one enum and one method, no new field on any public
  struct**. New private: `StoreOutcome`, `StoreMiss`, `token_fingerprint`,
  `FINGERPRINT_HEX_CHARS`, `HEX_DIGITS`, `OAuthHelper::interactive_token`,
  `OAuthHelper::refresh_only_refusal`, and the private `interactivity` field. **Removed:**
  `OAuthHelper::generate_code_verifier`, `OAuthHelper::generate_code_challenge` (the PKCE
  duplicates), the `rand::RngExt` and `base64` imports. Changed signature:
  `refresh_token(&self, refresh_token, stored_client_id, granted_scopes)`;
  `token_from_store -> Result<StoreOutcome>`. One new `#[cfg(test)]` module with **6** tests.
- **`tests/oauth_refresh.rs`** (**created**, **1447** lines — `min_lines` 160 ✓). **21** tests in
  five documented groups (A: defect 1 and the refresh-token lifecycle; B: defect 2, the DCR
  client_id; C: defect 3, the granted scope, including the RFC 6749 §6 property; D: the bounded
  error path; E: D-08, `Interactivity::RefreshOnly`), plus a `WireBodies` recorder, a
  `CountingCallbackLauncher`, a `WarnCapture` `tracing::Subscriber` and the `HelperSpec` /
  `SeedSpec` builders.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (918 → **1032**) — three
  entries: `D-116-KEYCHAIN` REOPENED, `D-116-GREP` fifth instance, `D-116-LINT-OAUTH` fourth
  measurement.

## Decisions Made

- **`refresh_token` takes the stored `client_id` and granted scopes as PARAMETERS, not from a
  second `store.load`.** The plan's `<action>` specifies re-reading the store inside
  `refresh_token` via a three-part `CredentialKey`. Measured: `refresh_token` has exactly ONE
  caller, `token_from_store`, which has already loaded that record under exactly that key. A
  second load would read the same key from the same store — but a refresh token and the
  `client_id` it was issued to are ONE pairing, and re-reading opens a window in which another
  process's write pairs this refresh token with a different record's `client_id`. The parameters
  come from the record whose `refresh_token` is being presented, which is the only correct source.
  The plan's INTENT — "read it from the store, not from config" — is satisfied exactly.
- **`authorize_with_details` refuses under `RefreshOnly` too, before any I/O.** The plan's
  behaviour rows only mention `get_access_token`. Without the second guard a headless platform
  that calls the "log me in" entry point still binds a listener and waits five minutes — the mode's
  guarantee would hold at one entry point and not the other. Negative control F shows it is its
  own independent detector.
- **`token_from_store` returns `StoreOutcome`/`StoreMiss` rather than `Option<String>`.** A
  `RefreshOnly` caller cannot go and look, so the refusal has to say which of three conditions
  occurred: nothing stored (seed the store), expired with no refresh token (re-authorize asking
  for `offline_access`), or the refresh was refused (re-authorize). Negative control G — collapsing
  the variants — fails exactly one row.
- **The DCR over-cap refusal keeps DCR-specific framing.** Routing the DCR success body through
  the shared helper changes the message, and `dcr_rejects_response_larger_than_1mib` asserts on
  `"exceeds"` and `"byte cap"`. Rather than weaken that test, the over-cap error is re-framed via
  `is_body_over_cap` while the shared refusal's rule is preserved (names the cap and the observed
  size, reproduces no body byte). The cap VALUE stays 1 MiB, and negative control I (raising it to
  `usize::MAX`) fails exactly that test — so it is a real detector for the value.
- **The four-stage SEP-2207 comment keeps the identifier `config.scopes`.** See *Deviations*.
- **`AUTH-03` is NOT booked complete.** `116-13`, `116-14` and `116-15` still claim it.
  `requirements-completed: []`, as in `116-01` through `116-11`.
- **RED was OBSERVED and logged, not COMMITTED as a broken build**, following `116-01` through
  `116-11`. See *TDD Gate Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's acceptance grep is blind to three of the six sites it audits**

- **Found during:** Task 3, enumerating the whole-body reads.
- **Issue:** the criterion is
  `grep -c '\.text()\.await\|\.bytes()\.await\|\.json()\.await\|\.json::<'` = 0, and the
  `<action>` lists three remaining sites on that basis. `rustfmt` renders long chains as
  `response\n    .bytes()\n    .await`, so the DCR success body, the device-code poll body and the
  refresh success body matched nothing. The grep returned **0 before and after** those three were
  fixed.
- **Fix:** all six routed through `collect_reqwest_body_within_cap`, and a multi-line-aware scan
  (`re.finditer(r'\.\s*(text|bytes|json)\s*(::<[^>]*>)?\s*\(\s*\)\s*\n?\s*\.\s*await')`) reported
  in *Gate Results* alongside the plan's grep. Both are **0**.
- **Committed in:** `464533ac`. Logged as `D-116-GREP` instance 5, with the general rule and two
  further examples measured in this plan.

**2. [Rule 2 — Missing critical functionality] `authorize_with_details` escaped the RefreshOnly guarantee**

Described under *Decisions Made*. Without it, `RefreshOnly` is a property of one method rather
than of the helper.

**3. [Rule 2 — Missing critical functionality] A refresh failure was completely silent**

- **Found during:** Task 1, writing the defect-2 error path.
- **Issue:** `token_from_store` discarded the refresh error with `let Ok(..) = .. else { return
  Ok(None) }`. In an unattended runtime that hides the single event that requires a human — the
  flow then falls back to a browser nobody is watching.
- **Fix:** `tracing::warn!("OAuth token refresh failed: {e}")`, carrying the authorization
  server's own reason and no credential content. It is also the only observable the Task 1 error
  rows have, and three of them assert against it.
- **Committed in:** `70a88d7e`.

**4. [Rule 1 — Bug] `unix_now_secs() + ttl` could panic on a peer-supplied `expires_in`**

- **Issue:** the refresh path computed `expires_at` with unguarded `+`. `expires_in` is
  authorization-server-supplied, so `u64::MAX` panics in a debug build — a denial of service
  inside the function whose job is to hand back a token. The sibling `build_auth_result` already
  used `saturating_add`.
- **Fix:** `saturating_add`, matching the sibling. **Committed in:** `70a88d7e`.

**5. [Rule 1 — Bug] The plan's `<action>` and its acceptance criterion contradict each other on `config.scopes`**

- **Issue:** the `<action>` says "Write that four-stage list as a comment next to the form
  construction. It is the single easiest thing in this phase to 'fix' wrongly, because reaching for
  `config.scopes` looks obviously right at the call site". The acceptance criterion says
  `grep -n 'config.scopes' src/client/oauth.rs` "shows no occurrence inside `refresh_token`". A
  comment that warns against `config.scopes` without naming it is not the warning the `<action>`
  asks for.
- **Resolution:** the machine-checkable INTENT is satisfied exactly — **zero CODE occurrences**,
  verified by a comment-stripping scan reported in *Gate Results* — and the three PROSE hits are
  REPORTED rather than removed. Rewording would weaken the comment precisely where the plan says
  the risk is highest. This is `D-116-PLANCONFLICT`'s shape and 116-16's Deviation 5's shape
  (`schema_version`: grep vs rustdoc), so the same resolution is applied: satisfy the grep's
  intent, report the prose.
- **For `116-15`:** the criterion should read "no occurrence in a non-comment line".

**6. [Rule 1 — Bug] Two clippy errors introduced by the new helper were FIXED, not allowed**

`token_fingerprint`'s first form tripped `clippy::format_collect`, and the fix tripped
`clippy::items_after_statements`. Both were resolved (a module-level `HEX_DIGITS` table and a
single-allocation loop) rather than annotated, keeping the phase's anchor at 17.

**Total deviations:** 6 (4 × Rule 1, 2 × Rule 2). No Rule 4 situation arose; no architectural
change was needed. **Zero dependencies added** — `git diff --exit-code b2bf9157..HEAD --
Cargo.toml` exits **0**; `mockito` and `proptest` are pre-existing dev-dependencies.

## Issues Encountered

- **`make quality-gate` exits 2 — `D-116-KEYCHAIN`, REOPENED, with the disk theory REFUTED.**
  `test-unit` reports `1866 passed; 14 failed`. All 14 are in `shared::streamable_http::tests`, a
  module this plan never touched, and every one panics at the same pre-existing line:

  ```
  panicked at src/shared/streamable_http.rs:458:18:
  Failed to load native root certificates: ... Os(Error { code: -36, message: "I/O error." })
  ```

  Three measurements settle the attribution, and none of them is inference:

  | Measurement | Result |
  |---|---|
  | An earlier `make quality-gate` in the SAME session on the identical tree | `test result: ok. **1880 passed; 0 failed**` |
  | `df -h /` at the moment of failure | **92 GiB free, 12% used** |
  | The same 14 tests against the **PRE-PLAN** `src/client/oauth.rs` (`git show 73d95880:...`) | `81 passed; **14 failed**` — identical, with none of this plan's code present |

  `14` is exactly the count `116-04` measured. `116-06`'s resolution should be read as "it did not
  reproduce that day", not "it is fixed", and `D-116-DISK` is **not** the mechanism. The real
  defect is the `.expect` at `streamable_http.rs:458` turning a transient OS condition into a
  panic in a transport constructor. Owner: `116-15`, as a NAMED item.

- **`mockito` serves the LAST matching mock, not the first.** `server.rs`'s `handle_request`
  collects every matching mock and takes `matching_mocks.last_mut()` unless one has an unmet
  `expect(n)`. A catch-all `/token` mock created BEFORE a specific refresh-matching one therefore
  wins for refresh requests too. The failure is silent — every refresh row simply receives the
  code-exchange answer — and it is how this suite's first GREEN run failed. The ordering rule is
  written into the harness in place.

- **One `/token` mock cannot serve two grants.** The first RED run's two error-path rows failed on
  `No supported OAuth flow available` rather than on their own assertions, because a mock that
  refuses the refresh also refuses the authorization-code exchange the fall-through then performs.
  Routing by `grant_type` keeps the two independent.

- **The working tree was modified by something OUTSIDE this plan after the final task commit, and
  those changes are deliberately NOT committed here.** At 00:01–00:02, after `464533ac` and after
  every verification run below, `src/client/oauth.rs`, `src/error/mod.rs` and
  `src/shared/credential_store.rs` acquired edits this executor did not make — including a new
  `Error::marker_field` helper consolidating the marker families. They are plausible cleanups, they
  are unverified by this plan, and they are left uncommitted for their author. **Consequence for a
  verifier: run this plan's greps against `git show HEAD:src/client/oauth.rs`, not against the
  working tree.** Every figure in *Gate Results* was either produced before 00:01 (timestamps in
  `target/116-verify/`) or re-measured against the committed object; the acceptance greps were
  re-run against `git show HEAD:` specifically for this reason.

- **`wc -l` in this environment is wrapped and mis-reported `src/client/oauth.rs` by one line**
  (3759 vs the git object's 3760 — its output carried a stray `Σ` total line). Line counts here are
  taken from `git show <rev>:<path> | grep -c ''`. Same family as `D-116-GREP`: the tool that
  reports the number is not neutral.

- **`D-116-FUZZGATE` unchanged.** Not re-measured in this plan's own gate run, which aborted at
  `test-unit` before `test-fuzz`. No claim is made about it here.

- **`D-116-FAILFAST` applied throughout.** Every RED run, negative control and regression run used
  `--no-fail-fast` with the denominator asserted (13, 13, 13, 21, 21, 21, 21, 210, 218, 13).

- **Source restored from a scratchpad COPY after every negative control**, never `git checkout --`.
  `shasum -a 256 -c` returned **OK** each time and `grep -c 'NEGATIVE CONTROL'` over the restored
  file returns **0**.

- **`grep -c '^error'` over a clippy log and `grep -c '^warning'` over a build log both
  over-count by one**, because each counts the `could not compile ... due to N previous errors` /
  `generated N warnings` summary line. The honest figures are **17** and **92**; the raw greps say
  18 and 93. 116-11's 17 and this plan's raw 18 are the same number.

## Threat Flags

None. This plan adds no new network endpoint, no new socket and no schema change at a trust
boundary. It CONSTRAINS existing outbound requests and REMOVES a log-based disclosure channel.

All `mitigate` dispositions in the plan's `<threat_model>` are discharged by a named test:

| Threat | Discharged by |
|---|---|
| T-116-44 (refresh-token destruction forcing re-auth after one cycle) | `an_omitted_refresh_token_in_the_response_preserves_the_stored_one` — asserts the stored value AND drives a SECOND refresh cycle with it. It PASSED pre-fix (116-11 had closed the defect) and is OBSERVED failing under negative control A, so it is a proven detector rather than assumed coverage |
| T-116-44a (a refresh widened with a never-granted scope, rejected as `invalid_scope`) | `the_refresh_body_carries_exactly_the_stored_granted_scopes_in_order`, `empty_stored_granted_scopes_omit_the_scope_key_entirely`, `an_advertised_but_never_granted_offline_access_is_absent_from_the_refresh`, and `a_refresh_never_widens_beyond_the_granted_scope_rfc6749_section_6` (24 generated grant sets, containment over the wire body). `config.scopes` and `scopes_supported` have **zero non-comment occurrences** in `refresh_token` |
| T-116-45 (DCR-registered clients unable to refresh at all) | `a_dcr_registered_client_refreshes_with_the_stored_issued_client_id` (asserts the DCR id on the wire and a browser count of 0), `the_stored_client_id_is_preferred_over_the_configured_one`, and `a_refresh_with_no_client_id_anywhere_names_both_places_it_looked` (asserts the refusal names both `OAuthConfig::client_id` and the stored record, and that NO request reached the token endpoint) |
| T-116-46 (five-minute hang per attempt in a headless runtime) | four `RefreshOnly` refusal rows, each asserting `is_reauth_required()`, `reauth_issuer()`, a launcher count of **0** and that the `redirect_port` is still BINDABLE afterwards. OBSERVED failing under negative controls D, E and F |
| T-116-47 (memory exhaustion from an oversized token/refresh/DCR body) | all **six** whole-body reads streamed under a 1 MiB running-total cap; `an_oversized_refresh_error_body_is_refused_naming_the_cap_and_no_content`; `dcr_rejects_response_larger_than_1mib` still green and OBSERVED failing under negative control I; `binary(v2_bounded_reads_tripwire)` **13/13** |
| T-116-48 (access token logged in plaintext at debug level) | `token_fingerprint` + 6 inline tests, two of which assert ABSENCE over every token prefix ≥ 4 characters in both directions. OBSERVED failing (5 of 6) under negative control H. `grep -c 'access_token\[\.\.'` → **0** |
| T-116-49 (refusal messages echoing refused body bytes) | the oversized-body row plants a canary and asserts both it and its padding are absent from the message, which names only the cap |
| T-116-SC (cargo installs) | **zero packages added**; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0**. Retiring the PKCE duplicates REMOVED this file's reliance on the optional `rand` dependency (`grep -c 'rand::'` → **0**) |

## Known Stubs

None. Every item is fully implemented and exercised. `grep -nE 'TODO|FIXME|HACK|XXX'` over both
files returns **no output**, and `make check-todos` exits 0.

One deliberate non-implementation, a documented decision rather than a stub: **`D-116-FALLBACK` is
LEFT OPEN** — see *Deferred Issues*.

## TDD Gate Compliance

Tasks 1 and 2 carry `tdd="true"`; Task 3 does not (it is `type="auto"` with no `tdd` attribute)
and was nevertheless given both inline tests and a two-break negative control.

**RED was observed and logged for both TDD tasks.**

| Task | Control log | Result |
|---|---|---|
| 1 | `116-12-task1.RED.log` | **13 run, 6 passed, 7 failed** — a BEHAVIOURAL red against the shipped pre-fix code |
| 1 | `116-12-task1.NEGATIVE-CONTROL.log` | **13 run, 9 passed, 4 failed** — three breaks at once |
| 2 | `116-12-task2.RED.log` | **2 compile errors** (`E0432` `Interactivity`, `E0599` `with_interactivity`) |
| 2 | `116-12-task2.NEGCTL-DF.log` | **21 run, 17 passed, 4 failed** |
| 2 | `116-12-task2.NEGCTL-EG.log` | **21 run, 18 passed, 3 failed** |
| 2 | `116-12-task2.NEGCTL-G.log` | **21 run, 20 passed, 1 failed** — G isolated because E masked it |
| 3 | H+I applied together | fingerprint suite **1 passed, 5 failed**; `oauth_dcr_integration` **23 passed, 1 failed** |

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01` through
`116-11`: in Rust a test naming a non-existent method fails to *compile*, so such a commit leaves a
non-building tree that breaks `git bisect` and contradicts CLAUDE.md's "ZERO TOLERANCE FOR
DEFECTS". A verifier looking for a `test(...)` → `feat(...)` pair will not find one; the evidence
is the seven control logs above, each named in its commit body.

### Task 1 — six rows PASSED pre-fix, and each is named as a positive control

The RED run's 7 failures are the genuine change rows: both defect-2 rows, both defect-3 rows, the
missing-`client_id` message row, and the two bounded-error-path rows. The **six** that passed are
recorded rather than counted as coverage:

| Row that PASSED pre-fix | Why |
|---|---|
| `an_omitted_refresh_token_..._preserves_the_stored_one` | 116-11's `token_from_store` rewrite had ALREADY closed D-14 defect 1 |
| `a_refresh_response_that_supplies_a_new_refresh_token_replaces_the_stored_one` | same rewrite |
| `a_refresh_response_that_omits_expires_in_does_not_corrupt_the_stored_expiry` | already correct |
| `an_authorization_that_issued_no_refresh_token_stores_none_and_falls_through` | already correct |
| `empty_stored_granted_scopes_omit_the_scope_key_entirely` | **vacuously** — no `scope` was ever sent |
| `a_refresh_never_widens_beyond_the_granted_scope_rfc6749_section_6` | **vacuously** — a request that sends no scope trivially contains nothing ungranted |

The last two are the reason the negative control matters: both are proven detectors under break B.

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| **A.** the stored refresh token no longer preserved when the response omits one | `an_omitted_refresh_token_..._preserves_the_stored_one` **only** | `a_refresh_response_that_supplies_a_new_refresh_token_replaces_the_stored_one` still PASSED — rotation and preservation are separate detectors |
| **B.** `scope` falls back to `config.scopes` on an empty grant | `empty_stored_granted_scopes_omit_the_scope_key_entirely` **and** `a_refresh_never_widens_beyond_the_granted_scope_rfc6749_section_6` | the two NON-empty scope rows still PASSED, because break B only fires on an empty grant — so they are independent detectors |
| **C.** an omitted `expires_in` carries the stale expiry forward | `a_refresh_response_that_omits_expires_in_does_not_corrupt_the_stored_expiry` **only** | everything else held |

### Task 2 — three controls, because D masked E and E masked G

| Control | Break | Tests that FAILED | Siblings that still PASSED |
|---|---|---|---|
| **D+F** | `RefreshOnly` falls through to the interactive path anyway **+** the `authorize_with_details` guard removed | the three `RefreshOnly` refusal rows (D) and `refresh_only_refuses_the_explicit_login_entry_point_too` (F) | **the two `RefreshOnly` HAPPY-path rows still PASSED** — a live cached token and a working refresh return BEFORE the interactivity `match`, so they are not detectors for the guarantee. 116-11's issuer-change refusal also PASSED, because it fires before `token_from_store` |
| **E+G** | the typed refusal replaced by `Error::internal` **+** the three `StoreMiss` variants collapsed | the three refusal rows, on `is_reauth_required()` | **`refresh_only_refuses_the_explicit_login_entry_point_too` still PASSED** — `authorize_with_details` builds its OWN refusal, so the two refusal SITES are independent detectors |
| **G alone** | the three `StoreMiss` variants collapsed into one message | `refresh_only_with_an_expired_token_and_no_refresh_token_refuses_distinctly` **only** | the other 20 held |

### Task 3 — two breaks

| Deliberate break | Tests that FAILED | Siblings that still PASSED |
|---|---|---|
| **H.** `token_fingerprint` reverted to the old 20-character plaintext prefix | 5 of the 6 inline rows, including BOTH leak rows | **`a_fingerprint_is_stable_for_one_token` still PASSED** — a plaintext prefix is also stable, so stability alone is NOT a detector for the leak |
| **I.** the DCR cap raised to `usize::MAX` | `dcr_rejects_response_larger_than_1mib` **only** | the other 23 tests in `oauth_dcr_integration` held |

## Gate Results

| Gate | Command | Result |
|---|---|---|
| **clippy baseline, measured on the PRISTINE tree BEFORE any edit** | `make lint`'s command with `--features "full,oauth"` | **17 errors, all 17 in `src/client/oauth.rs`**, exit 101 |
| clippy after Task 1 | same | **17** — 0 NEW, 0 GONE |
| clippy after Task 2 | same | **17** — 0 NEW, 0 GONE |
| **clippy FINAL, re-measured at HEAD `464533ac` (including the new inline test module)** | same | **17** — **0 NEW, 0 GONE**, compared as a multiset of `(error message, offending source-line text)`. **This is the phase's final figure for this file** |
| Task 1 RED | `-E 'binary(oauth_refresh)'`, `--no-fail-fast` | **13 run, 6 passed, 7 failed** |
| Task 1 GREEN | same | **13 run, 13 passed** |
| Task 1 negative control | same | **13 run, 9 passed, 4 failed** |
| Task 2 RED | same | **2 compile errors**, exit 101 |
| Task 2 GREEN | same | **21 run, 21 passed** |
| Task 2 negative control D+F / E+G / G | same | **17/4**, **18/3**, **20/1** |
| Task 3 negative control H / I | inline suite / `binary(oauth_dcr_integration)` | **1 passed, 5 failed** / **23 passed, 1 failed** |
| **final suite** | `-E 'binary(oauth_refresh)'`, `--features full,oauth` | **21 run, 21 passed** |
| **narrow-gate reality** | the same selector, `--features full` | **0 tests run**, `error: no tests to run` |
| inline lib tests | `cargo test --lib --features full,oauth token_fingerprint` | **6 passed**; under `--features full`: **0 passed, 1880 filtered out** |
| no regression | 10 oauth binaries | **218 run, 218 passed** |
| **bounded-reads tripwire** | `-E 'binary(v2_bounded_reads_tripwire)'` | **13 run, 13 passed** under `--features full` AND under `full,oauth` |
| doctests | `cargo test --features full,oauth --doc client::oauth` | **8 passed, 0 failed** (6 → 8; the two new ones are `Interactivity` and `with_interactivity`) |
| lint (**authoritative for `full`**) | `/usr/bin/make lint` | **exit 0**, "No lint issues" (after each task) |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations**; `grep -c cognitive_complexity src/client/oauth.rs` → **0** |
| SATD | `make check-todos` / `grep -nE 'TODO\|FIXME\|HACK\|XXX'` | **exit 0** / **no output** |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** naming `client/oauth.rs` |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 — re-run at HEAD |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, **92** lib warnings (= the `116-BASELINES` anchor), **0** naming this file |
| whole-body reads, plan's grep | `grep -c '\.text()\.await\|\.bytes()\.await\|\.json()\.await\|\.json::<'` | **0** — but it was **0 before the fix too** (`D-116-GREP`) |
| whole-body reads, **multi-line aware** | `re.finditer(r'\.\s*(text\|bytes\|json)\s*(::<[^>]*>)?\s*\(\s*\)\s*\n?\s*\.\s*await')` | **0** — the check that actually holds |
| PKCE duplicates gone | `grep -c 'fn generate_code_verifier' src/client/oauth.rs` | **0** |
| `rand` retired | `grep -c 'rand::' src/client/oauth.rs` | **0** |
| no plaintext token log | `grep -c 'access_token\[\.\.' src/client/oauth.rs` | **0** |
| no new public field | `grep -c 'pub interactivity' src/client/oauth.rs` | **0** |
| `config.scopes` in `refresh_token` | comment-stripping scan | **0 code occurrences**, **3 PROSE occurrences** (reported, see Deviation 5) |
| DCR cap value | `grep -n 'const MAX_DCR_RESPONSE_BYTES'` | `= DEFAULT_AUTH_RESPONSE_BYTES` (1 MiB), unchanged |
| gate: `test-unit` | inside `make quality-gate` | **1866 passed; 14 failed** — `D-116-KEYCHAIN`, all 14 in `shared::streamable_http`, **reproduced identically against the PRE-PLAN source** |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 2 at `test-unit` only**, on `D-116-KEYCHAIN`. An earlier run in the same session on the identical tree reported **1880 passed / 0 failed** |
| disk | `df -h /` before and after | 106 → 92 GiB free (`D-116-DISK` never triggered) |

## User Setup Required

None. No external service, no credential and no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

Two operator-visible additions for `116-13`'s CHANGELOG:

1. **`OAuthHelper::with_interactivity(Interactivity::RefreshOnly)`** is new, opt-in, and turns a
   five-minute headless hang into an immediate `Error::reauth_required`. Default behaviour is
   unchanged.
2. **A refresh now sends `scope`** (exactly the granted set) and **sources `client_id` from the
   credential store**. A DCR-registered client that previously required a full browser login on
   every expiry now refreshes.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-KEYCHAIN` — REOPENED, and the most urgent item for `116-15`.** It reproduced at **92
  GiB free**, and the same 14 tests fail identically against the PRE-PLAN source, so neither
  `D-116-DISK` nor this plan is the cause. The real defect is the `.expect` at
  `src/shared/streamable_http.rs:458` turning a transient macOS keychain `ioErr (-36)` into a panic
  in a transport constructor. It will fail CI on any macOS runner in the same state. Do **not**
  "fix" it by pinning `rustls` to `webpki-roots` — that changes which CAs the SDK trusts in
  production to work around a test-environment fault.
- **`D-116-GREP` — fifth instance, and the first where the plan's grep HID real violations.**
  Three of six whole-body reads were split across lines by `rustfmt` and matched nothing; the grep
  read **0 both before and after**. `116-14`'s fence must be multi-line aware or a single
  `cargo fmt` can silently reopen every site it guards. Two further examples measured here: `grep
  -c '^error'` over a clippy log and `grep -c '^warning'` over a build log each over-count by one.
- **`D-116-LINT-OAUTH` — fourth consecutive measurement.** The gate selected **0 of this plan's 27
  tests** (21 integration + 6 inline) and its `test-unit` population is still **1880** — unmoved
  across four consecutive plans that each added inline tests. **102 tests** from `116-09` through
  `116-12` are outside CI. The clippy half is **17**, unchanged. Owner: `116-15`, to close as a
  PAIR.
- **`D-116-FALLBACK` — LEFT OPEN, and provably NOT made worse.** This plan does not close it:
  `is_terminal_authorization_refusal` is unchanged (`is_iss_mismatch() || is_state_mismatch()`,
  **0** diff lines), and the four un-marked authorization refusals are still wrapped into "No
  supported OAuth flow available". What this plan adds cannot reach that wrapper — every
  `RefreshOnly` refusal is returned from `get_access_token`'s `RefreshOnly` arm or from
  `authorize_with_details`' entry guard, both of which are BEFORE `authorize_with_fallback`, so
  none can trigger device-code fallback. Asserted by the four refusal rows' launcher count of 0.
  Owner: `116-15`.
- **`D-116-PLANCONFLICT` — second instance**, now in an acceptance CRITERION rather than an
  `<action>`: the plan requires a comment naming `config.scopes` and a grep showing no occurrence
  of it. Resolved in favour of the grep's INTENT (0 code occurrences) with the 3 prose hits
  reported. `116-15` should restate the criterion as "no occurrence in a non-comment line".
- **`D-116-FUZZGATE`** — NOT re-measured; this plan's gate run aborted at `test-unit` before
  `test-fuzz`. No claim is made. **`D-116-DISK`** — never triggered (106 → 92 GiB).
  **`D-116-FAILFAST`**, **`D-116-DOC`**, **`D-116-TRIPWIRE`**, **`D-116-PRM`**, **`D-116-EX`** —
  unchanged; nothing here reopens any of them.

## Next Phase Readiness

| Consumer | What it can now rely on |
|---|---|
| `116-13` | `Interactivity` / `with_interactivity` are public and documented — `cargo-pmcp`'s non-interactive paths should select `RefreshOnly`. Two CHANGELOG lines are owed (see *User Setup Required*). `cargo semver-checks` again reports "no semver update required" despite a new public enum and method, for the tenth consecutive plan in this phase; **do not rest the version-bump reasoning on that verdict** — a new public enum is a MINOR bump |
| `116-14` | `src/client/oauth.rs` is clean for the fence: **0** whole-body reads under a multi-line-aware scan, **0** PKCE duplicates, **0** `rand::`, **0** plaintext token logs, **0** SATD. **Use a multi-line-aware matcher, not a line-oriented grep** — `D-116-GREP` instance 5 is the measurement that says why. `Error::reauth_required` is now raised from two further sites |
| `116-15` | **The `full,oauth` clippy anchor for this file is FINAL at 17** — this was the last plan to edit it, and it neither added nor removed an error. `make quality-gate` is **exit 2 at this HEAD on `D-116-KEYCHAIN` only**, reproduced against the pre-plan source. Three deferred entries updated. **Do not book `AUTH-03`** — `116-13` and `116-14` still claim it, and `D-116-PRM` still names what has no end-to-end coverage |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-14` | the whole-body-read fence MUST be multi-line aware; a line-oriented grep reads 0 on a dirty file |
| `116-15` | `D-116-KEYCHAIN` is a real, reproducible gate-red condition on a healthy machine — fix the `.expect` at `streamable_http.rs:458`, do not pin `webpki-roots` |
| `116-15` | close `D-116-LINT-OAUTH` as a PAIR — the clippy 17 AND the test gate; **102** tests are outside CI |
| `116-15` | close or waive `D-116-FALLBACK`; restate the `config.scopes` criterion as non-comment-lines-only; do not book `AUTH-03` on this plan's evidence |
| every source-touching plan | `make lint` AND the `full,oauth` gate-equivalent; `--no-fail-fast` with the denominator asserted; restore from a scratchpad COPY; absolute binary paths for anything whose output you count |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: tests/oauth_refresh.rs                                     1447 lines (min_lines 160 ✓)
FOUND: src/client/oauth.rs                                        3760 lines (was 3272, +567/−79)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md 1032 lines (was 918)
```

Commits claimed, verified in `git log`:

```
FOUND: 70a88d7e  feat(116-12): the three D-14 refresh defects, plus scope on refresh
FOUND: be9705fe  feat(116-12): Interactivity::RefreshOnly, the browser path unreachable by construction
FOUND: 464533ac  feat(116-12): close the hygiene debt in src/client/oauth.rs
```

`must_haves` verification:

```
✓ truths[1] an AS that omits refresh_token no longer destroys the stored one —
  an_omitted_refresh_token_in_the_response_preserves_the_stored_one, which also drives a SECOND
  refresh with the preserved token. It PASSED pre-fix (116-11 had closed it) and is OBSERVED
  failing under negative control A, so it is a proven detector and is reported as such
✓ truths[2] a DCR-registered client can refresh, reading the issued client_id from the store —
  a_dcr_registered_client_refreshes_with_the_stored_issued_client_id asserts the DCR id on the
  WIRE and a browser count of 0; the_stored_client_id_is_preferred_over_the_configured_one pins
  the precedence; both OBSERVED failing pre-fix
✓ truths[3] a refresh sends only the GRANTED scope, or none — four rows plus a 24-case RFC 6749
  §6 containment property over the wire body, the property OBSERVED failing under break B;
  config.scopes and scopes_supported have ZERO non-comment occurrences in refresh_token
✓ truths[4] a headless caller gets a typed reauth-required immediately — four RefreshOnly rows,
  each asserting is_reauth_required() + reauth_issuer() + a launcher count of 0 + the redirect
  port still BINDABLE; OBSERVED failing under breaks D, E and F; both public entry points guarded
✓ truths[5] no whole peer-supplied body is allocated beyond its cap and no access token is
  logged in plaintext — SIX (not three) reads routed through collect_reqwest_body_within_cap,
  multi-line-aware scan 0, tripwire 13/13; token_fingerprint with two ABSENCE tests, OBSERVED
  failing under break H
✓ artifacts: src/client/oauth.rs contains "Interactivity" and provides the D-14 fixes, the mode,
  the bounded reads and sha256-prefix token logging
✓ artifacts: tests/oauth_refresh.rs 1447 >= 160 — mockito coverage for the three refresh
  defects, scope on refresh, and RefreshOnly behaviour
✓ key_links: src/client/oauth.rs -> src/shared/credential_store.rs — refresh sources client_id
  from the stored record (pattern "client_id" at the refresh_token signature and its call site)
✓ key_links: src/client/oauth.rs -> src/shared/http_body_cap.rs — pattern
  "collect_reqwest_body_within_cap" present at all six bounded whole-body reads
```

Plan-level verification block:

```
✓ binary(oauth_refresh) — 21 run / 21 passed under --features full,oauth, non-zero count
✓ the four earlier oauth binaries still green — 218/218 across 10 binaries
✓ whole-body needle count in src/client/oauth.rs is 0 under BOTH the plan's grep and a
  multi-line-aware scan (the plan's grep was 0 before the fix too — D-116-GREP instance 5)
✓ make lint exit 0; full,oauth clippy 17 at HEAD vs a 17 PRE-MEASURED pristine baseline, with
  ZERO new errors as a multiset of (message, offending source line). FINAL figure for this file
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations, no new allow
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
✓ binary(v2_bounded_reads_tripwire) — 13 run, 13 passed, under BOTH feature sets
✓ wasm32 build — exit 0, 92 lib warnings = the 116-BASELINES anchor, 0 naming this file
✗ make quality-gate — exit 2 at test-unit on D-116-KEYCHAIN (14 failures in
  shared::streamable_http, a module this plan never touched). NOT attributable, proven three
  ways: an earlier run on the identical tree gave 1880 passed / 0 failed; the volume had 92 GiB
  free; and the same 14 fail identically against the PRE-PLAN src/client/oauth.rs
⚠ make quality-gate selects 0 of this plan's 27 tests (21 integration + 6 inline), and its
  test-unit population is still 1880 — the FOURTH consecutive plan (D-116-LINT-OAUTH)
○ D-116-FUZZGATE NOT re-measured: the gate aborted at test-unit before test-fuzz
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-05*
