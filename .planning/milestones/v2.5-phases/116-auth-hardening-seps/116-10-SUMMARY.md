---
phase: 116-auth-hardening-seps
plan: 10
subsystem: auth
tags: [oauth, dcr, rfc7591, sep-837, sep-2207, rfc6749, application-type, offline-access, granted-scope, bounded-reads, semver, mockito]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 03
    provides: "DcrRequest::application_type / set_application_type, DcrResponse::application_type, DCR_APPLICATION_TYPE_KEY — the flatten-carrier accessors that make this plan's wiring semver-free"
  - phase: 116-auth-hardening-seps
    plan: 04
    provides: "derive_application_type + ApplicationType::as_str — D-10's unanimous-or-error derivation"
  - phase: 116-auth-hardening-seps
    plan: 06
    provides: "collect_reqwest_body_within_cap + DEFAULT_AUTH_RESPONSE_BYTES — the streaming bounded reader the DCR REJECTION path now uses"
  - phase: 116-auth-hardening-seps
    plan: 09
    provides: "BrowserLauncher / with_browser_launcher (the seam every authorization-URL assertion here goes through), the 24-error full,oauth clippy anchor, and the restore-from-a-scratchpad-COPY discipline"
provides:
  - "Every DCR request pmcp sends carries a DERIVED application_type (SEP-837), asserted on the WIRE"
  - "grant_types declares refresh_token, and offline_access is declared in client metadata AND requested at the authorization request — both only when scopes_supported advertises it (SEP-2207)"
  - "compose_scopes_with_offline_access — one composition used by BOTH stages, so the recorded 'requested' scope cannot drift from the value sent"
  - "The GRANTED scope is recorded from the token response, with RFC 6749 §5.1's omission rule applied against the COMPOSED request rather than config.scopes"
  - "application_type_divergence — D-11's rule as a private pure function; warns, never fails"
  - "A registration rejection that names the status, the server's parsed error fields, the application_type sent and the redirect_uris sent — read under the 1 MiB cap and echoing no other body content"
  - "D-116-GREP's fourth instance: this plan's own `grep -n 'retry'` criterion cannot pass without deleting the documentation the same task requires"
  - "A re-measured D-116-LINT-OAUTH anchor: 21, down from 24, with the test-side twin re-confirmed at 0/38"
affects: [116-11, 116-12, 116-13, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A protocol value that means different things at different stages gets ONE composition function and an explicit table of which stages write it — and which deliberately do not"
    - "Never mutate a public field to add a derived value: take a slice, return a fresh Vec, and prove non-accumulation with a two-flow test"
    - "A presence assertion is NOT a detector for an echo channel — only an absence assertion is, and a negative control that echoes the raw body proves which is which"
    - "When a behaviour was already accidentally true pre-fix, say so: the PREFIX-RED run is not the attribution argument for it, a targeted negative control is"
    - "Thread a value out of the function that computes it and consume it at the first real caller; do not build plumbing for a consumer that does not exist yet, because unused plumbing is dead code under -D warnings"

key-files:
  created: []
  modified:
    - src/client/oauth.rs
    - tests/oauth_dcr_integration.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "A field on AuthorizationResult was considered and REJECTED as a MAJOR semver break (constructible_struct_adds_field); the registered application_type leaves through the private DcrOutcome instead"
  - "SEP-837's OPTIONAL retry with an adjusted application_type was deliberately NOT adopted — an automatic retry silently registers the client under a type its operator did not choose"
  - "offline_access is written at exactly two stages (DCR client metadata, authorization request) and deliberately at neither refresh nor the device-code grant"
  - "The rejection body is read through collect_reqwest_body_within_cap; the SUCCESS body's pre-116-06 bytes()-then-measure form is left untouched, because 116-09 assigned it to 116-12"
  - "Parsed error/error_description are truncated to 200 characters (MAX_DCR_ERROR_FIELD_CHARS) — a 1 MiB error_description is still an echo channel wearing a specification-approved hat"
  - "AUTH-02 and AUTH-03 are NOT booked complete — 116-08 and 116-15 still claim AUTH-02, and six later plans still claim AUTH-03"

patterns-established:
  - "An inherited uncommitted draft is judged against the plan first and the draft second; every piece kept is re-verified from zero, because an uncommitted tree carries no evidence"
  - "A negative control must break the rule the test claims to pin, not merely make the feature absent — three divergence rows passed the PREFIX-RED run and needed their own control"

requirements-completed: []

# Metrics
duration: 118min
completed: 2026-08-04
---

# Phase 116 Plan 10: SEP-837 `application_type` and SEP-2207 Refresh Metadata on the DCR Wire Summary

**Two SEPs landed at one edit site and the wire is what the tests check.** Every dynamic client
registration pmcp sends now carries `application_type: "native"` — DERIVED from the
`http://127.0.0.1:{port}/callback` it is actually registering, not assumed — plus
`grant_types: ["authorization_code", "refresh_token"]`, and `offline_access` at the two protocol
stages where writing it means something. **The `offline_access` lifecycle was the finding worth the
plan:** declaring it in DCR client metadata says what the client *may* ask for and asks for nothing;
the request happens at the authorization URL; and adding it at refresh would violate RFC 6749 §6's
narrow-never-widen rule. All three rows are now explicit in the source, and the third is a written
non-action that 116-12 owns.

**The inherited draft was judged, not trusted.** A previous executor left ~981 uncommitted lines
implementing Task 1 and **none** of Task 2. Nothing had been linted, run, or gate-checked. The Task 1
work was measured from zero — pristine clippy baseline, PREFIX-RED, GREEN, gates — and kept because
it held up. Task 2 was written from scratch, including the whole rejection path the draft left in its
pre-existing shape: an unbounded `response.text()` interpolated **verbatim** into the error message,
which is T-116-37's echo channel sitting in the tree.

**The `full,oauth` clippy anchor moved 24 → 21 with ZERO new errors**, compared as a multiset of
`(error message, offending source-line text)` because every line in the file moved again.
**`make quality-gate` exits 0 having run ZERO of this plan's 38 tests** — measured, not inferred:
`--features full` reports `Starting 0 tests across 2 binaries`, and the gate's `test-unit` count is
**1880**, byte-identical to 116-09's, despite this plan adding 14 inline lib tests.

## Performance

- **Duration:** ~118 min
- **Completed:** 2026-08-04
- **Tasks:** 2
- **Files:** 3 modified (2 source, 1 planning), **+1550 / −41** across the two task commits

## Accomplishments

- **RED was the shipped pre-fix implementation, twice, with named surviving siblings both times.**
  Task 1: `target/116-verify/116-10-task1.PREFIX-RED.log` — the pristine `86fbb70a` source with the
  new suite, `--no-fail-fast`, **16 tests run, 9 passed, 7 failed**. Task 2:
  `116-10-task2.PREFIX-RED.log` — the Task 1 source with the Task 2 suite, **24 tests run, 19
  passed, 5 failed**. Neither break was invented; both are what the tree actually did.

- **The one place the PREFIX-RED run was NOT evidence is recorded rather than glossed.** Task 2's
  three echo-divergence rows all **PASSED** pre-fix, because "registration never fails on a
  divergent echo" was already accidentally true when nothing read the echo at all. A dedicated
  negative control supplies the attribution: making divergence fatal produced **30 run, 29 passed,
  1 failed**, and the single failure is
  `an_echoed_application_type_that_diverges_still_registers_the_client` while its omitted-echo and
  identical-echo siblings both held.

- **The second negative control proved that a presence assertion is not a detector for an echo
  channel.** Three breaks at once — raw body interpolated into the rejection message, the bounded
  read replaced by `response.text()`, and an absent echo reported as divergence — gave **30 run, 24
  passed, 6 failed**. The surviving sibling is the whole argument:
  `a_rejected_registration_names_the_status_the_sent_type_and_the_sent_redirect_uri` **still
  passed**, because every substring it asserts is present in a message that also echoes the entire
  hostile body. Only the four absence rows caught it.

- **`config.scopes` cannot accumulate, and that is asserted across two flows over one config.** The
  single-flow rows cannot see an in-place `push`; `two_consecutive_flows_do_not_accumulate_scopes_on_the_shared_config`
  runs the flow twice against an advertising server and asserts both that the two authorization URLs
  request the SAME scope and that `config.scopes` is untouched afterwards. `OAuthConfig::scopes` is
  a public field on a public struct, so a caller reusing one config must not watch it grow.

- **The granted-scope fix is subtle and would have been invisible.** `build_auth_result` already
  applied RFC 6749 §5.1's omission rule — but against `config.scopes`. The moment `offline_access`
  is added to the authorization request, `config.scopes` stops being "what was requested", so an
  AS that omits `scope` from its token response would have had its full grant recorded as narrower
  than it was, silently narrowing every refresh 116-12 performs. The composed value is now passed
  in, and both branches of the rule name themselves in a comment because they look interchangeable.

- **`make quality-gate` exits 0 end to end** at 113 GiB free: `fmt-check` ✓, `lint` ✓ ("No lint
  issues"), `build` ✓, `test-unit` **1880 passed / 0 failed**, `test-doc` **445 passed / 0 failed /
  79 ignored**, `test-integration` ✓, `test-examples` ✓. `cargo semver-checks check-release -p pmcp
  --baseline-rev b2bf9157`: **223 checks, 223 pass, 0 fail** — the eighth consecutive plan in this
  phase to see "no semver update required" despite genuinely new behaviour. **Zero packages added**;
  `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0**.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Derive and send `application_type`, the `refresh_token` grant and `offline_access` at both request stages | `defc2eb5` | feat |
| 2 | Echo divergence warns but never fails; a registration rejection names what was sent | `87f1f648` | feat |

## Files Created/Modified

- **`src/client/oauth.rs`** (**modified**, 1993 → **2680** lines, +687/−26 across both commits).
  New private module-level items: `OFFLINE_ACCESS_SCOPE`, `MAX_DCR_RESPONSE_BYTES` (hoisted from a
  function-local literal and now defined as `DEFAULT_AUTH_RESPONSE_BYTES`, value-identical at
  1 MiB), `MAX_DCR_ERROR_FIELD_CHARS`, `compose_scopes_with_offline_access`,
  `apply_application_type`, `application_type_divergence`, `DcrRejectionFields`,
  `dcr_rejection_fields`, `bounded_error_field`, `registration_rejected`, `DcrOutcome`. Changed
  private signatures: `do_dynamic_client_registration` gained a `&OidcDiscoveryMetadata` parameter
  and returns `DcrOutcome`. Two new `#[cfg(test)]` modules with **14** tests. **No new public item
  of any kind.**
- **`tests/oauth_dcr_integration.rs`** (**modified**, 253 → **1075** lines — `min_lines` 220 ✓).
  **5 → 24** tests in five documented groups (A: the DCR wire body; B: the authorization request;
  C: the granted scope; D: the override and the `web` derivation pmcp's own flow cannot reach;
  E: the response half).
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (719 → **~800**) — a re-measured
  `D-116-LINT-OAUTH` section (both halves) and a new `D-116-GREP` fourth instance.

## Decisions Made

- **A field on `AuthorizationResult` was considered and REJECTED as a MAJOR semver break.** It is
  public, all-`pub`-field and not `#[non_exhaustive]`, so a new field is
  `cargo-semver-checks`' `constructible_struct_adds_field` — exactly the class of break this phase
  exists to avoid. The registered `application_type` leaves `do_dynamic_client_registration`
  through the private `DcrOutcome` struct instead. This is recorded in `DcrOutcome`'s own rustdoc,
  not only here, so a later reader does not "simplify" it back.
- **The value is threaded out one hop and consumed at the first real caller, not plumbed to a
  consumer that does not exist.** `resolve_client_id_for_flow` reports it in the `tracing::info!`
  that already announced DCR success. Threading it further — through
  `authorization_code_flow_inner` and into `authorize_with_details` — would have created a
  parameter nothing reads, which is `dead_code` under `RUSTFLAGS="-D warnings"` and would have to
  be silenced with `_`-prefixes. **116-11 owns the remaining hop**, and
  `StoredCredentials::with_registered_application_type` (116-05, private fields) is what it
  persists through, so that hop also costs no semver event.
- **SEP-837's OPTIONAL retry MAY was deliberately NOT adopted.** The specification permits a client
  to retry with an adjusted `application_type` after a rejection. An automatic retry would silently
  register the client under a type its operator did not choose — the opposite of "surface a
  meaningful error to the user or developer", which is the same paragraph's obligation. The
  non-adoption and its reason are written into `registration_rejected`'s rustdoc.
- **`offline_access` is written at two stages and deliberately at neither of two others.** DCR
  client metadata (what the client MAY ask for) and the authorization request (the ask). NOT at
  refresh: RFC 6749 §6 permits narrowing and never widening, so introducing a scope there that was
  never granted can have the AS refuse a refresh that would otherwise succeed — 116-12 owns that
  stage. NOT on the device-code path either, because that grant never builds an authorization URL,
  so recording `offline_access` as requested there would be a lie; the device branch keeps
  `config.scopes` with a comment saying why.
- **The rejection path was migrated to the bounded reader; the SUCCESS path was not.** 116-09
  explicitly assigned the success-path `bytes()`-then-measure form to 116-12, and the pre-existing
  `dcr_rejects_response_larger_than_1mib` pins its message. This plan's Task 2 behaviour row covers
  only the rejection body, which was genuinely unbounded (`response.text()`), so that is what
  changed.
- **Parsed error fields are truncated to 200 characters.** The body is already capped at 1 MiB, but
  a 1 MiB `error_description` is still an echo channel wearing a specification-approved hat. The
  truncation notice names how many characters were withheld and reproduces none of them.
- **`AUTH-02` and `AUTH-03` are NOT booked complete.** `116-08` and `116-15` still claim AUTH-02;
  `116-11`, `116-12`, `116-13`, `116-14`, `116-15` and `116-16` still claim AUTH-03.
  `requirements-completed: []`, as in `116-01` through `116-09`.

## Verdict on the Inherited Draft

A previous executor left `src/client/oauth.rs` (+378/−26) and `tests/oauth_dcr_integration.rs`
(+603/−1) uncommitted, with no commits and no recorded verification. Judged against the plan:

| Draft component | Verdict | Reasoning |
|---|---|---|
| `OFFLINE_ACCESS_SCOPE` + the three-stage doc table | **KEPT** | It is the plan's own `<action>` argument, written down at the constant rather than buried in a commit message |
| `compose_scopes_with_offline_access` (slice in, fresh `Vec` out, dedup, order-stable) | **KEPT** | Satisfies every behaviour row including non-accumulation; verified by the two-flow test and re-verified under PREFIX-RED |
| `apply_application_type` with an override-preserving early return | **KEPT** | The plan's `<action>` says only "derive and set"; the draft's early return is what makes D-09's documented override path survive, and the acceptance criteria require an override test. Correct beyond the letter |
| `MAX_DCR_RESPONSE_BYTES = DEFAULT_AUTH_RESPONSE_BYTES` (hoisted to module scope) | **KEPT** | Value-identical (1 MiB) and `http_body_cap`'s own rustdoc names DCR as the site this constant was lifted from. It also became load-bearing for Task 2, whose over-cap refusal must name one number |
| Passing the composed scope into `build_auth_result` | **KEPT** | This is the non-obvious half of the granted-scope work and the draft got it right |
| The 11 new integration rows | **KEPT** | Every one is a real detector: 7 of the 11 failed under PREFIX-RED and the 4 that held are the correct positive controls |
| The 8 inline composition tests | **KEPT** | Pin the rules at the level they are decided at |
| **All of Task 2** | **ABSENT — written from scratch** | No `application_type_divergence`, no divergence warning, no actionable rejection, and the rejection path still read the body unbounded via `response.text()` and interpolated it **verbatim** into the error. That last one is T-116-37 unmitigated |

**Nothing was kept on the draft's authority.** The pristine clippy baseline was measured against
`86fbb70a` (not against the dirty tree), the PREFIX-RED run used the pristine source, and every gate
was re-run. Restores used the scratchpad COPIES throughout — `git checkout --` was attempted once
for the baseline measurement, denied by the environment, and replaced with `cp` from a `git show`
extraction, which is the safer form 116-07 established anyway.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical functionality] The rejection path's `error_description` is still an unbounded echo channel after parsing**

- **Found during:** Task 2, writing the T-116-37 behaviour row.
- **Issue:** the plan's rule is "carry only the parsed `error`/`error_description`, never raw body
  content". Parsing bounds the SHAPE but not the SIZE: a hostile registration endpoint can put
  ~1 MiB of text in `error_description` (the body cap is 1 MiB) and it would reach the developer's
  terminal and every log aggregator downstream, through a field the rule explicitly permits.
- **Fix:** `MAX_DCR_ERROR_FIELD_CHARS = 200` with a character-boundary-safe truncation whose notice
  names how many characters were withheld and reproduces none of them. Two tests pin it — one
  inline over `dcr_rejection_fields`, one end to end asserting a 5000-character description does
  not reach the message in full.
- **Committed in:** `87f1f648`.

**2. [Rule 2 — Missing critical functionality] The rejection body was read with no cap at all**

- **Found during:** Task 2, reading the response-handling path the plan's `<read_first>` names.
- **Issue:** the plan's behaviour row says "a rejection whose body exceeds the cap is refused by the
  bounded read". There was no bounded read on that path: `response.text().await.unwrap_or_default()`
  allocates whatever the peer sends. The 1 MiB cap applied only to the SUCCESS path.
- **Fix:** the rejection path now uses `collect_reqwest_body_within_cap` (116-06), whose refusal
  names the cap and reproduces no body content, and which propagates verbatim.
- **Committed in:** `87f1f648`. Verified by `a_rejection_body_over_the_cap_is_refused_without_reproducing_it`,
  which plants a canary in a 1.3 MiB rejection body and asserts its absence.

**3. [Rule 1 — Bug] The plan's `grep -n 'retry'` acceptance criterion cannot pass without deleting documentation the same task requires**

- **Found during:** Task 2, running the acceptance criteria literally.
- **Issue:** the criterion is *"`grep -n 'retry' src/client/oauth.rs` shows no automatic
  `application_type` retry was added"*. It returns **3** hits — lines 310–312, all `///` doc lines
  recording that SEP-837's retry MAY was deliberately NOT adopted, which the same task's `<action>`
  requires be written down. Satisfying the grep literally means deleting the required
  documentation.
- **Fix:** the meaningful check was performed and reported instead — all three hits are comment
  lines, and `do_dynamic_client_registration` issues exactly one `POST` with no loop and no second
  `apply_application_type` call. Written up as a fourth `D-116-GREP` instance with a proposed
  convention (exclude comment lines, or assert the absence of the CODE construct).
- **Committed in:** `87f1f648`; the deferred entry lands with this summary.

**Total deviations:** 3 (2 × Rule 2, 1 × Rule 1). No Rule 4 situation arose; no architectural change
was needed. **Zero dependencies added.**

## Issues Encountered

- **`make quality-gate` runs ZERO of this plan's 38 tests and exits 0.** Measured at `87f1f648`:
  `cargo nextest run --features full` over this plan's selector reports
  `Starting 0 tests across 2 binaries` followed by `error: no tests to run`; under `full,oauth` it
  is **38 run, 38 passed**. The corroborating figure is the gate's own `test-unit` count: **1880**,
  identical to 116-09's, although this plan added **14** new inline lib tests — every one of them
  behind `oauth`. This is `D-116-LINT-OAUTH`'s test-side twin, re-measured with a second plan's
  numbers.
- **The `full,oauth` clippy anchor has now moved twice in three plans: 29 → 24 → 21.** Every
  disappearance is a side effect of rewriting a line the plan had to touch anyway, never a side
  quest. **The anchor for 116-11 and 116-12 is 21.** A plan that measures against 24 will report
  three phantom fixes.
- **`git checkout -- <path>` is denied by this environment's action classifier.** The baseline
  measurement needed the pristine file; `git show 86fbb70a:src/client/oauth.rs > copy && cp copy
  src/client/oauth.rs` did the same job. This is strictly safer than `git checkout --` and matches
  116-07's process rule, so the denial cost nothing.
- **`rtk` corrupts `git`-subcommand invocations and truncates diffs.** `git log --format=...` under
  the proxy produced `git: 'rtk' is not a git command`. Every command whose output this plan counted
  used `/usr/bin/git`, `/usr/bin/make` or `$HOME/.cargo/bin/cargo`.
- **`cargo nextest list --message-format json` ignores `-E` in its `testcases` map**, so counting
  matched tests from the JSON silently returns either the whole population (1880/1945) or zero
  depending on which key you read. The honest measurement is `nextest run`'s own
  `Starting N tests across M binaries` line. This is the same "reports a plausible number from a
  run that did not do what you think" shape as `D-116-FAILFAST` and the selector trap.
- **`D-116-FUZZGATE` reconfirmed, unchanged.** Inside this plan's gate run, **21** targets each died
  on ``the option `Z` is only accepted on the nightly compiler``, each printed "completed", and the
  gate still exited 0.
- **`D-116-FAILFAST` applied throughout.** Every RED run, negative control and regression run used
  `--no-fail-fast` with the denominator asserted (16, 24, 30, 30, 38, 25, 13).
- **Two `(leaky)` warnings from nextest** on the full 24-row integration run — the callback-driving
  launcher's spawned reader task outliving the test by a few milliseconds, exactly as 116-09
  recorded. Named so a later reader does not treat it as a new signal.
- **`git commit -m` with a multi-paragraph message remains unreliable here.** Both task commits used
  `git commit -F <file>`.

## Threat Flags

None. This plan adds no new public API, no new network endpoint and no new file access. It changes
what two existing requests carry and how one existing response is handled. The one direction worth
naming — a hostile registration endpoint's text reaching a developer's terminal — got *narrower*,
not wider: the rejection message previously echoed the whole body verbatim and now carries two
parsed fields truncated to 200 characters each.

All `mitigate` dispositions in the plan's `<threat_model>` are discharged by a named test:

| Threat | Discharged by |
|---|---|
| T-116-35 (loopback redirect registered under the OIDC default `web`) | `the_dcr_wire_body_carries_the_derived_native_application_type` — a `Matcher::PartialJsonString` mock that answers 501 unless `application_type: "native"` is on the wire; observed FAILING under PREFIX-RED |
| T-116-36 (registration failing because the AS legally modified metadata) | **accepted, and asserted**: `an_echoed_application_type_that_diverges_still_registers_the_client` asserts SUCCESS; NEGCTL-A made divergence fatal and this row was the ONLY failure of 30 |
| T-116-37 (a hostile endpoint echoing bytes through the error path) | four absence rows — a canary in an unparsed JSON field, a canary in a non-JSON body, a 5000-character description truncated, and a 1.3 MiB body refused by the cap. NEGCTL-B echoed the raw body and all four failed while the presence row held |
| T-116-38 (silent divergence between requested and registered metadata) | `application_type_divergence` over all four cases inline (equal, divergent, absent, non-string), each asserting the exact `Option` shape; NEGCTL-B's absent-echo break failed exactly two of them; the AS's value is carried out through `DcrOutcome` for 116-11's store rather than through a new public field |
| T-116-38a (`offline_access` requested where requesting is meaningless) | `the_authorization_url_requests_offline_access_when_the_server_advertises_it` and its not-advertised sibling, both through 116-09's `BrowserLauncher` seam, plus both DCR client-metadata rows |
| T-116-38b (assuming the requested scope was granted) | `a_token_response_with_a_scope_records_exactly_what_was_granted` (server granted 2 of 3; `offline_access` requested and NOT granted is asserted absent) and `a_token_response_without_a_scope_records_the_requested_scope_rfc6749_5_1` |
| T-116-07 (inherited from 116-03 → 116-04, **transferred twice**) | **now discharged.** The derivation refused a mismatch since 116-04 but nothing called it; `apply_application_type` is that call, and the wire assertion proves the value reaches the server |
| T-116-SC (cargo installs) | zero packages; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0** |

## Known Stubs

None. Every item is fully implemented and exercised. `grep -nE 'TODO|FIXME|HACK|XXX'` over both
source files returns **no output**. No placeholder value, empty collection or "not available" string
was introduced.

The one deliberate non-implementation — SEP-837's optional retry — is a documented decision recorded
in `registration_rejected`'s rustdoc with its reason, not a stub. The one deliberate scope boundary
— the DCR SUCCESS-path body read still uses the pre-116-06 `bytes()`-then-measure form — is 116-12's
by 116-09's explicit assignment, and the cap it enforces is unchanged at 1 MiB.

## TDD Gate Compliance

Both tasks carry `tdd="true"`. **RED was observed and logged for both, and in both cases the
"break" was the shipped implementation reproduced rather than an invented one.**

| Task | Control log | Result |
|---|---|---|
| 1 | `116-10-task1.PREFIX-RED.log` | **16 run, 9 passed, 7 failed** — the pristine `86fbb70a` source |
| 2 | `116-10-task2.PREFIX-RED.log` | **24 run, 19 passed, 5 failed** — the Task 1 source |
| 2 | `116-10-task2.NEGCTL-A.log` | **30 run, 29 passed, 1 failed** — divergence made fatal |
| 2 | `116-10-task2.NEGCTL-B.log` | **30 run, 24 passed, 6 failed** — three breaks at once |

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01`
(`ea1d2d68`) through `116-09`: in Rust a test naming a non-existent item fails to *compile*, so such
a commit leaves a non-building tree that breaks `git bisect` and contradicts CLAUDE.md's "ZERO
TOLERANCE FOR DEFECTS". A verifier looking for a `test(...)` → `feat(...)` pair will not find one;
the evidence is the four control logs above, each named in its commit body.

### Task 1 — the pre-116-10 implementation (`--no-fail-fast`, denominator 16 asserted)

7 of 16 failed. The **9 survivors are the attribution argument**:

| Surviving sibling | Why it correctly did NOT fire |
|---|---|
| all 5 pre-existing rows | this plan is additive on the wire; `dcr_body_matches_rfc7591`'s weaker `grant_types: ["authorization_code"]` partial match still holds against the two-entry array |
| `an_explicit_application_type_reaches_the_wire_instead_of_the_derivation` | exercises 116-03/116-04 API directly; independent of this plan's wiring |
| `an_https_non_loopback_registration_derives_web` | same |
| `the_authorization_url_omits_offline_access_when_the_server_does_not_advertise_it` | the pre-fix flow never added `offline_access`, so "absent when not advertised" was already true — a positive control, not a detector |
| `a_token_response_with_a_scope_records_exactly_what_was_granted` | the `scope`-PRESENT branch of RFC 6749 §5.1 was already correct; only the omission branch changed |

### Task 2 — three controls, because the PREFIX-RED run was not sufficient

The PREFIX-RED run's 5 failures are all rejection rows. **The three divergence rows PASSED**, and
that is recorded rather than presented as coverage: "registration never fails on a divergent echo"
was accidentally true when nothing read the echo. Two targeted controls supply the attribution:

| Control | Break | Tests that FAILED | Siblings that still PASSED |
|---|---|---|---|
| A | divergence returns `Err` | `an_echoed_application_type_that_diverges_still_registers_the_client` **only** (1 of 30) | `an_omitted_application_type_echo_is_not_divergence…` and `an_identical_application_type_echo_registers_without_incident` — an omitted and an equal echo genuinely do not enter the branch, so they are independent |
| B | raw body interpolated into the rejection message **+** `response.text()` instead of the bounded read | `a_rejected_registration_does_not_echo_unparsed_fields_of_the_body`, `…_with_a_non_json_body_reproduces_none_of_it`, `…_truncates_an_oversized_error_description`, `a_rejection_body_over_the_cap_is_refused_without_reproducing_it` | **`a_rejected_registration_names_the_status_the_sent_type_and_the_sent_redirect_uri`** — every substring it asserts is still present in a message that also echoes the whole hostile body. A presence assertion is not a detector for an echo channel |
| B | an ABSENT echo reported as divergence | `an_absent_echo_is_not_a_divergence`, `a_non_string_echo_reaches_application_type_divergence_as_an_absence` | `an_equal_echo_is_not_a_divergence`, `a_different_echo_is_a_divergence_naming_both_values`, and end to end `an_omitted_application_type_echo_is_not_divergence_and_registration_succeeds` — the last proving the warning is genuinely non-fatal even when it fires wrongly |

Source restored from a scratchpad COPY after each control, never `git checkout --`.
`shasum -a 256 -c` returned **OK** all three times, and `grep -c 'NEGATIVE CONTROL'` over the
restored file returns **0**.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| **clippy baseline, measured on the PRISTINE `86fbb70a` tree** | `make lint`'s command with `--features "full,oauth"` | **24 errors, all 24 in `src/client/oauth.rs`**, exit 101 |
| clippy after Task 1 | same | **21**, all in the same file — **0 NEW**, 3 GONE |
| **clippy after Task 2** | same | **21**, all in the same file — **0 NEW**, 3 GONE, compared by error identity |
| Task 1 RED | `-E 'binary(oauth_dcr_integration)'`, pristine source | **16 run, 9 passed, 7 failed** |
| Task 1 GREEN | same, draft source | **16 run, 16 passed** |
| Task 2 RED | same, Task 1 source | **24 run, 19 passed, 5 failed** |
| Task 2 negative control A | integration + inline divergence | **30 run, 29 passed, 1 failed** |
| Task 2 negative control B | same | **30 run, 24 passed, 6 failed** |
| **final suite** | `binary(oauth_dcr_integration)` + the two inline modules, `--features full,oauth` | **38 run, 38 passed** |
| **narrow-gate reality** | the same selector, `--features full` | **0 tests run**, `error: no tests to run` |
| DCR suite count vs baseline | `-E 'binary(oauth_dcr_integration)'` | **24** vs the recorded **5** — strictly greater, asserted numerically |
| no regression (116-09) | `binary(oauth_iss_integration) + binary(oauth_state_csrf)` | **25 run, 25 passed** |
| **bounded-reads tripwire** | `-E 'binary(v2_bounded_reads_tripwire)'` | **13 run, 13 passed** |
| lint (**authoritative**) | `/usr/bin/make lint` | **exit 0**, "No lint issues" (after each task) |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** (twice); `grep -c cognitive_complexity src/client/oauth.rs` → **0** |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** naming `client/oauth.rs` |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, **92** warnings (= the 116-BASELINES anchor), **0** naming this file |
| no new public field | `grep -n 'pub registered_application_type\|pub application_type' src/client/oauth.rs` | **no output** |
| old grant literal gone | `grep -n 'vec!\["authorization_code".to_string()\]' src/client/oauth.rs` | **no output** |
| no automatic retry | `grep -n 'retry' src/client/oauth.rs` | **3** hits, all `///` doc lines documenting the NON-adoption — see `D-116-GREP` |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over both files | **no output** |
| gate: `test-unit` | inside `make quality-gate` | **1880 passed; 0 failed** — unchanged from 116-09, which is itself the D-116-LINT-OAUTH proof |
| gate: `test-doc` | inside `make quality-gate` | **445 passed; 0 failed; 79 ignored** |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 0** at 113 GiB free |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

There is one operator-visible behaviour change worth a CHANGELOG line for 116-13: **every dynamic
client registration now declares `application_type` and `refresh_token`**, and requests
`offline_access` when the authorization server advertises it. An authorization server that
previously issued no refresh token because the client never asked may now issue one. No
configuration is required and no existing configuration changes meaning.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-LINT-OAUTH` (both halves re-measured, not a new entry).** The clippy anchor is **21** at
  `87f1f648`, down from 24, with zero new errors from this plan; the test-side twin is confirmed at
  **0 of 38**. The paired resolution is unchanged: clear the 21 first, then add
  `--features "full,oauth"` to `make lint` and the gate's test stage. Owner: `116-15`.
- **`D-116-GREP` — a fourth instance (new).** This plan's own `grep -n 'retry'` criterion cannot
  pass without deleting the documentation the same task requires. Proposed convention addition: an
  audit grep over a file that documents its own non-adoptions must exclude comment lines, or assert
  the absence of the CODE construct rather than of the word. Owner: `116-15`.
- **`D-116-FUZZGATE`** — reconfirmed inside this plan's gate run (21 nightly failures, all
  swallowed, gate still exits 0). Still open for `116-15`.
- **`D-116-FALLBACK`** — untouched and not made worse: this plan adds no new refusal to the
  authorization-callback path. Still open.
- **`D-116-LINT`** — no new measurement: `make lint` was **exit 0** on the first run after each task
  here, as in `116-09`. The standing obligation was met.
- **`D-116-DOC`, `D-116-DISK`, `D-116-KEYCHAIN`, `D-116-SLASH`, `D-116-TRIPWIRE`, `D-116-EX`,
  `D-116-FAILFAST`** — unchanged; nothing here reopens any of them. `D-116-KEYCHAIN` did not
  reproduce (1880/0 at 113 GiB free), a fourth independent clean observation.

## Next Phase Readiness

| Consumer | What it can now rely on |
|---|---|
| `116-11` | `do_dynamic_client_registration` returns a private `DcrOutcome` carrying `registered_application_type` — the AS's echoed value when it echoed one, otherwise the value sent. One hop remains: surface it out of `resolve_client_id_for_flow` and persist it through `StoredCredentials::with_registered_application_type`. `AuthorizationResult.scopes` already carries the GRANTED scope, so `with_granted_scopes` has a correct source |
| `116-12` | the composed request scope is what `build_auth_result` records when the token response omits `scope`, so `AuthorizationResult.scopes` is safe to refresh with directly. Refresh must send only that value or no `scope` at all — `OFFLINE_ACCESS_SCOPE`'s rustdoc states the rule at the constant. The DCR **success**-path body read is still the pre-116-06 `bytes()`-then-measure form and is yours; the rejection path is already on `collect_reqwest_body_within_cap` and is a working example |
| `116-13` | release-note lines: DCR now sends `application_type`, declares the `refresh_token` grant, and requests `offline_access` when advertised — an AS may now issue a refresh token where it previously did not. A rejected registration now produces an actionable error instead of a bare status. **No new public API**, and `cargo semver-checks` again says "no semver update required" (eighth observation) — do not rest the bump on it |
| `116-15` | `make quality-gate` **exit 0** at this HEAD, third consecutive clean full-gate run. The `full,oauth` clippy anchor is **21**. One new deferred entry (`D-116-GREP` #4); `D-116-LINT-OAUTH` re-measured with a second plan's numbers |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-11`, `116-12` | measure the `full,oauth` clippy baseline for `src/client/oauth.rs` BEFORE editing; it is **21** at `87f1f648`, NOT 24 and NOT 29 |
| `116-12` | do NOT introduce `offline_access` (or any scope) at refresh — RFC 6749 §6 narrow-never-widen; the rule is written at `OFFLINE_ACCESS_SCOPE` |
| `116-15` | close `D-116-LINT-OAUTH` as a PAIR (clear 21, then enable `oauth` in lint AND tests) — **38** more tests joined the 25 already outside CI |
| `116-15` | do not book `AUTH-02` or `AUTH-03` on this plan's evidence alone |
| every source-touching plan | `make lint`, not clause (b) alone; `--no-fail-fast` with the denominator asserted; restore from a scratchpad COPY, never `git checkout --`; absolute binary paths for anything whose output you count |

No blockers.

## Self-Check: PASSED

Files claimed modified, verified on disk:

```
FOUND: src/client/oauth.rs                                         2680 lines (was 1993)
FOUND: tests/oauth_dcr_integration.rs                              1075 lines (min_lines 220 ✓, was 253)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md  (+2 sections)
```

Commits claimed, verified in `git log`:

```
FOUND: defc2eb5  feat(116-10): derive and send application_type, the refresh grant and offline_access
FOUND: 87f1f648  feat(116-10): echo divergence warns but never fails, and a rejection names what was sent
```

`must_haves` verification:

```
✓ truths[1] every DCR carries an application_type derived from the redirect URIs it registers —
  apply_application_type calls derive_application_type(&request.redirect_uris); asserted on the
  WIRE by a PartialJsonString mock that 501s otherwise; OBSERVED failing pre-fix
✓ truths[2] registration never fails on a divergent echo — application_type_divergence drives a
  warn only; asserted end to end; NEGCTL-A made it fatal and exactly that row failed
✓ truths[3] a rejection names what was sent — status, parsed error/error_description, the
  application_type and the redirect_uris, with four absence rows proving nothing else leaks
✓ truths[4] grant_types declares refresh_token — full ordered array asserted on the wire
✓ truths[5] offline_access is REQUESTED at the authorization request when advertised — asserted
  on the authorization URL's scope parameter through 116-09's BrowserLauncher seam, both
  advertised and not-advertised
✓ truths[6] the GRANTED scope is recorded — both RFC 6749 §5.1 branches tested, and the omission
  branch now uses the COMPOSED request rather than config.scopes
✓ artifacts: src/client/oauth.rs contains "set_application_type" (via apply_application_type)
  and provides the DCR body composition, authorization-request composition, granted-scope
  recording, divergence warning and actionable rejection
✓ artifacts: tests/oauth_dcr_integration.rs 1075 >= 220, with wire-body assertions for
  application_type and grant_types, echo-divergence, rejection and offline_access coverage
✓ key_links: derive_application_type over the redirect_uris being registered — present
✓ key_links: DcrRequest::set_application_type / DcrResponse::application_type — both reached
```

Plan-level verification block:

```
✓ binary(oauth_dcr_integration) 24 run / 24 passed — strictly greater than the recorded 5
✓ binary(oauth_iss_integration) + binary(oauth_state_csrf) 25/25 — 116-09 unregressed
✓ make quality-gate exit 0; make lint exit 0; full,oauth clippy 21 vs a 24 PRE-MEASURED pristine
  baseline with ZERO new errors attributable
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations, no new allow
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
✓ binary(v2_bounded_reads_tripwire) — 13 run, 13 passed
✓ wasm32 build — exit 0, 92 warnings = the 116-BASELINES anchor
⚠ the 38 tests above are NOT reachable by make quality-gate (D-116-LINT-OAUTH test-side twin),
  measured (0 under --features full) rather than left implicit
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
