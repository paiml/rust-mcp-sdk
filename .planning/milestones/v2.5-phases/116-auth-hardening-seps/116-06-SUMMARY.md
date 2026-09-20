---
phase: 116-auth-hardening-seps
plan: 06
subsystem: auth
tags: [oauth, oidc, rfc8414, rfc9207, sep-2351, discovery, redirect-policy, bounded-reads, semver, mockito]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 04
    provides: "discovery_url_candidates, issuer_matches_metadata, same_origin, DiscoveryFailure/DiscoveryOutcome/classify_discovery_failure — the pure tier this plan wires into the network path"
  - phase: 116-auth-hardening-seps
    plan: 05
    provides: "the serde-refusal rule (classification + line/column, never the parser message) and the --no-fail-fast negative-control discipline (D-116-FAILFAST)"
provides:
  - "collect_reqwest_body_within_cap — the streaming two-refusal bounded whole-body read every auth response now goes through, with the Validation/Internal variant split that lets a caller tell an oversized body from a failed read"
  - "hardened_discovery_client — the origin-pinned, redirect-count-bounded reqwest client BOTH discovery call sites build from, so the policy cannot diverge"
  - "is_redirect_refusal / is_body_over_cap — the two classification seams 116-07 and 116-12 need"
  - "OidcDiscoveryClient::discover_with_extras + AuthorizationServerExtras — the RFC 9207 flag reaching callers with NO field added to OidcDiscoveryMetadata"
  - "The RFC 8414 3.3 anchor enforcement at the fetch site — T-116-09's other half, which 116-04 could only prepare"
  - "The SEP-2351 ordered probe wired to classify_discovery_failure, with three TERMINAL classes proven not to fall through"
  - "A per-issuer candidate-index cache whose three safety constraints are each asserted"
  - "D-116-KEYCHAIN RESOLVED by measurement: 1865 passed / 0 failed on a clean volume"
  - "D-116-TRIPWIRE — 116-05 left v2_bounded_reads_tripwire RED and make quality-gate runs it"
affects: [116-07, 116-09, 116-12, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A rewrite, not an annotation: the tripwire recognises no bounded form for any reqwest whole-body read, so a call site is fixed by routing through a helper that contains no needle — including in its own prose, so the auditing grep reports zero"
    - "Two contradicting lints inside a pub(crate) module: measure both directions, pick the crate-wide style choice, and record the measurement in place so nobody re-runs the experiment"
    - "Carry a constructor failure in a Result-typed field when the constructor cannot return Result without a MAJOR break — the alternative (falling back to a default client) silently discards the security policy"
    - "A negative control can CORRECT a test rather than only confirm it: the https->http row survived a scheme-dropping break because the default ports differ, so an explicit same-port row was added"
    - "Prove a probe ORDER with mock hit counts and expect(0) guards, never with the final result — a fall-through and a correct probe produce the same Ok"

key-files:
  created:
    - src/shared/http_body_cap.rs
    - tests/oauth_discovery_validation.rs
  modified:
    - src/client/auth.rs
    - src/shared/mod.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "The three TERMINAL classes abort the whole probe; each is fenced by a well-formed candidate 3 behind expect(0), so a fall-through fails the test"
  - "A present-but-non-boolean RFC 9207 flag is TERMINAL, never Ok(None) — None reads as Optional and would make an ABSENT callback iss acceptable"
  - "should_retry (string-sniffing) is REMOVED: the plan states the classification matrix is the only branch"
  - "new()/with_settings() keep their signatures; the hardened-client build failure is carried in a Result-typed field and surfaced by discover()"
  - "A peer-chosen document issuer is truncated at 256 chars before reaching a refusal message"
  - "The test file is gated on http-client, NOT oauth — so make lint compiles it and it runs under plain --features full"
  - "AUTH-01 and AUTH-03 are NOT booked complete — 116-07/09/10/11/12/13/16 all still claim them"

patterns-established:
  - "D-116-LINT now has a SIXTH measurement, and a new shape: clause (b) exits 0 on code where make lint's redundant_pub_crate and the crate's own unreachable_pub give OPPOSITE instructions"
  - "Assert the mockito fixture's own precondition (content_length() == Some(n) / == None) inside the test, or a body-cap row silently exercises the wrong refusal"
  - "Record a transient gate state explicitly when a plan splits define-helper and wire-helper across two commits: 13 dead_code errors, named, and green again one commit later"

requirements-completed: []

# Metrics
duration: 268min
completed: 2026-08-04
---

# Phase 116 Plan 06: RFC 8414 §3.3 Anchor Validation and the SEP-2351 Ordered Probe Summary

**AUTH-01's own anchor is now validated at the source, and the proof is that the fence was
OBSERVED firing on the pre-fix tree: `fetch_discovery` returned
`Ok(OidcDiscoveryMetadata { issuer: "https://honest.example", authorization_endpoint:
"http://127.0.0.1:63736/authorize", .. })` for a document fetched from `127.0.0.1` — the
specification's own worked attack, succeeding, in this SDK, today. Discovery now follows the
spec's ordered candidate list without regressing the appended form Microsoft Entra ID needs;
every failure is classified by the shared matrix and the three security classes ABORT rather
than downgrade; a path-bearing issuer pays its two 404s once; and no auth response body — on
success OR on an error path — is ever allocated beyond 1 MiB.**

**Separately, this plan settles a question three earlier plans could not: on a clean volume
`make test-unit` reports 1865 passed, 0 failed. `D-116-KEYCHAIN` is an environment artifact.**

## Performance

- **Duration:** ~268 min
- **Completed:** 2026-08-04
- **Tasks:** 2
- **Files:** 5 (2 created, 3 modified), **+1952 / −116** across the two task commits

## Accomplishments

- **The Pitfall 1 fence was observed FIRING before the fix, which is the only evidence that
  matters for a security check.** `target/116-verify/116-06-task2.RED.log`: 14 tests run, 2
  passed, **12 failed**, run with `--no-fail-fast` and the denominator asserted. The lying-
  document test failed by calling `unwrap_err()` on an `Ok` carrying
  `issuer: "https://honest.example"` — so pre-fix pmcp accepted, verbatim, the document RFC 8414
  §3.3's worked example says MUST be rejected. The two tests that PASSED pre-fix are the positive
  control and the non-JSON fallback row, both of which happened to be satisfied by the single
  appended-form URL that was pmcp's only behaviour.

- **A fall-through and a correct probe produce the same `Ok`, so ordering is proven by hit counts
  and `expect(0)` guards, never by the result.** Three TERMINAL rows — a lying issuer, a body over
  the cap, and a non-boolean RFC 9207 flag — each put a **perfectly valid** document behind
  candidate 3 with `expect(0)` and `assert_async()`. If the classification match were ever
  "simplified" back to `if !ok { continue }`, all three would fail, because the caller would get
  its metadata from the candidate the attacker chose. That is the silent-downgrade risk named in
  a comment at the classification site.

- **A malformed RFC 9207 flag is a fail-OPEN if you let it become `None`, and all four shapes are
  fenced.** `as_bool()` on the string `"true"` yields `None`, `None` reads as `Optional`
  downstream, and `Optional` makes an ABSENT callback `iss` acceptable — so a broken or hostile
  authorization server would silently RELAX the strictness it was trying to declare. `"true"`,
  `1`, `null` and `{}` are each asserted to abort discovery, at the integration level AND against
  the private classifier.

- **The candidate cache is a latency fix that cannot weaken a check, and each of its three
  constraints is its own test.** It stores an INDEX, never a document; a failed cache hit restarts
  the FULL ordered sequence from candidate 0 (asserted by a fixture where the remembered candidate
  starts 404ing and candidate 1 then serves — "fall forward to the next index" would find nothing
  after index 2 and fail the probe); and a cache hit runs the SAME anchor check, asserted by a
  fixture where the remembered candidate begins lying and candidate 1's valid document carries
  `expect(0)`.

- **`D-116-KEYCHAIN` is settled, and the arithmetic closes exactly.** 116-04 measured
  1830 + **14 failed** (total 1844) with the volume filling; 116-05 measured 1836 + **13 failed**
  (total 1849) at 96–99% capacity; this plan, first to run with **71 GiB free at 15%**, measured
  **1865 passed, 0 failed** (total 1865). `1849 + 16 = 1865`, where 16 is precisely this plan's new
  inline test count. Two greps over the whole gate log confirm the mechanism did not merely go
  quiet: `streamable_http.rs:4` → **0 hits**, `Failed to load native root certificates` → **0
  hits**. Written up in `deferred-items.md`, including the note that this single gate run consumed
  **42 GiB** (71 → 29 GiB free), so the failure regime is reproducible in both directions.

- **Zero new public API beyond the two items the plan specifies, and zero packages.**
  `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157`: **223 checks, 223 pass, 0
  fail**, exit 0. `git diff --exit-code b2bf9157..HEAD -- Cargo.toml`: exit **0**. `mockito` was
  already a dev-dependency, so `T-116-SC` is discharged again.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Streaming two-refusal bounded read + origin-pinned discovery client | `18faf398` | feat |
| 2 | Ordered probe, RFC 8414 §3.3 anchor, RFC 9207 flag, five bounded reads | `f4a48195` | feat |

## Files Created/Modified

- **`src/shared/http_body_cap.rs`** (**created**, **716** lines). `pub(crate)` inside a
  `pub(crate)` module — nothing reaches the public surface. Items:
  `DEFAULT_AUTH_RESPONSE_BYTES` (1 `MiB`), `MAX_DISCOVERY_REDIRECTS` (5),
  `REDIRECT_REFUSAL_MARKER`, `collect_reqwest_body_within_cap`, `is_body_over_cap`,
  `hardened_discovery_client`, `is_redirect_refusal`, plus six private helpers
  (`auth_body_over_cap`, `discovery_redirect_permitted`, `cross_origin_redirect_refusal`,
  `redirect_limit_refusal`, `unjudgeable_redirect_refusal`, `origin_of`). **10** inline tests.
- **`src/client/auth.rs`** (**modified**, 531 → **1101** lines, +802/−116). New public:
  `AuthorizationServerExtras` (+ `iss_parameter_supported`), `discover_with_extras`. New private:
  `fetch_discovery` (rewritten and moved to a free function), `probe_candidate`, `probe_order`,
  `remember_candidate`, `http_client`, `document_issuer_field`, `iss_parameter_flag`,
  `request_failure`, `status_failure`, `unparseable_document`, `issuer_mismatch`,
  `malformed_security_metadata`, `every_candidate_failed`, `truncate_for_message`,
  `rendered_source_chain`, `read_token_body`, `read_error_body_within_cap`,
  `parse_token_response`. Removed private: `should_retry`. Inline tests 5 → **11**.
- **`tests/oauth_discovery_validation.rs`** (**created**, **538** lines — `min_lines` 120 ✓).
  **19** tests in six documented groups. `#![cfg(feature = "http-client")]`, **not** `oauth` — see
  *Decisions Made*.
- **`src/shared/mod.rs`** (+12) — the gated `pub(crate) mod http_body_cap;` with its rationale.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (278 → **377**) — one new entry
  (`D-116-TRIPWIRE`) and a measured RESOLUTION appended to `D-116-KEYCHAIN`.

## Decisions Made

- **`should_retry` is REMOVED, not kept alongside the matrix.** The plan is explicit: map the
  failure to a `DiscoveryFailure`, pass it to `classify_discovery_failure`, "and there is no other
  branch." A second, string-sniffing retry predicate (`error_str.contains("CORS")`, `"network"`,
  `"timeout"`, `"connection"`) is exactly the divergence the shared matrix exists to prevent. It is
  private, so removing it is not a semver event; its unit test is replaced by
  `test_failure_classification_replaces_the_old_string_sniffing_retry`, which asserts the 404 /
  5xx / other-4xx rows through the same `status_failure` the probe uses.
- **The hardened-client build failure is CARRIED, not swallowed.** `hardened_discovery_client`
  returns `Result`, but `new()` and `with_settings()` cannot without a MAJOR break. Falling back to
  `reqwest::Client::new()` would silently discard the redirect policy — the security control would
  vanish on exactly the machines where TLS setup is already unhealthy. The field is
  `Result<reqwest::Client, String>` and `discover()` surfaces the reason.
- **A refused redirect is `MalformedSecurityMetadata`, not `Transport`.** It is a statement about
  who would have AUTHORED the document. Retrying returns the same answer, and falling through to
  another candidate is precisely the downgrade the policy exists to prevent — so it is TERMINAL.
- **The document's issuer IS named in the refusal, but bounded (Rule 2 — not in the plan).** The
  plan requires naming both values, and 116-04 established that refusals must not reproduce an
  issuer string because it can carry userinfo. Both hold here: the EXPECTED issuer has already
  passed `validate_issuer_url` (userinfo rejected), and the DOCUMENT's issuer is attacker-chosen
  arbitrary text, so it is truncated at 256 characters. A test asserts a 10 000-character issuer
  produces a message under 2 000 characters.
- **`serde` refusals carry `classify()` + line/column, never the parser's message** — 116-05's
  rule, applied here to the discovery document AND to token responses, where the body carries
  credentials. `test_token_response_parse_failure_echoes_no_input` asserts the field name from the
  offending body is absent from the message.
- **The test file is gated on `http-client`, not `oauth`.** `OidcDiscoveryClient` lives behind
  `http-client` and nothing in the file constructs an `OAuthHelper`. Measured consequence: the
  suite reports **19 run, 19 passed** under plain `--features full` as well as under
  `--features full,oauth`, and `make lint` (which runs `--features "full" --lib --tests`) compiles
  it — so the file is lint-gated, which an `oauth`-gated file would not be.
- **`AUTH-01` and `AUTH-03` are NOT booked complete.** This plan lands the anchor enforcement and
  the ordered probe at ONE of the three discovery call sites; `116-07` owns `generic_oidc.rs` and
  `cognito.rs`, `116-09` consumes `discover_with_extras`, and `116-10`–`116-16` own the rest.
  `requirements-completed: []`, as in `116-01` through `116-05`.
- **RED was OBSERVED and logged for both tasks but NOT committed as a broken build**, for the
  sixth time in this phase and for the same reason. See *TDD Gate Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The negative control CORRECTED a weak assertion instead of only confirming a strong one**

- **Found during:** Task 1's three-break negative control.
- **Issue:** the pure `https://as.example` → `http://as.example` row was written as the
  https-to-http detector. Under a break that dropped the SCHEME from the origin comparison
  (comparing host + effective port only) that row **still passed** — because the two DEFAULT ports,
  443 and 80, differ, so the port component refused it and the scheme was never consulted.
- **Why it mattered:** the row read like a scheme fence and was not one. A later edit that quietly
  dropped the scheme from `same_origin` would have been caught only by the wire-level test.
- **Fix:** an explicit same-port row was added
  (`https://as.example:8443` → `http://as.example:8443`), where only the scheme can decide. The
  re-run then produced **4 failed / 6 passed** instead of 3/7, with the pure test as an
  independent detector. The measurement is recorded in the test's own comment.
- **Committed in:** `18faf398`.

**2. [Rule 1 — Bug] `D-116-LINT`, sixth measurement — and a NEW shape: two gate lints that contradict each other**

- **Found during:** Task 2, running `make lint` per the standing obligation from 116-03/04/05.
- **Issue:** `make lint` reported **7 hard errors**, all `clippy::redundant_pub_crate` (nursery):
  a `pub(crate)` item inside an already-private module should be plain `pub`. The phase's clause-(b)
  clippy command exits **0** on the identical tree.
- **The new part:** switching all seven items to `pub` traded them for **7
  `unreachable_pub` errors** — `src/lib.rs:6-11` turns that rustc lint on crate-wide with
  `#![warn(...)]`, and `make lint` runs `RUSTFLAGS="-D warnings"`. **The two lints give opposite
  instructions for every item in a `pub(crate)` module and only one can be satisfied.**
- **Fix:** `unreachable_pub` wins — it is the crate's own deliberate, crate-wide style choice,
  while `redundant_pub_crate` is a nursery lint. The items stay `pub(crate)` and
  `#![allow(clippy::redundant_pub_crate)]` is applied to **that module only**, with a `// Why:`
  comment and the both-directions measurement written into the module doc so nobody re-runs the
  experiment. `make lint` → **exit 0**.
- **Committed in:** `f4a48195`.

**3. [Rule 1 — Bug] The module's own PROSE tripped the acceptance grep**

- **Found during:** Task 1, checking
  `grep -c '\.text()\.await\|\.bytes()\.await\|\.json()\.await\|\.json::<' src/shared/http_body_cap.rs`,
  which must be **0**. It was **2**.
- **Issue:** the module doc explained the tripwire by SPELLING the four needles literally. Nothing
  was unbounded — the explanation itself matched the audit grep.
- **Fix:** the needles are described rather than spelled ("its awaited text, bytes and json
  accessors, including the turbofished json form"), with a sentence recording that the omission is
  deliberate so a later editor does not "helpfully" put them back. Count → **0**.
- **Committed in:** `18faf398`.

**4. [Rule 2 — Missing critical functionality] A peer-chosen issuer could flood every log line**

Described under *Decisions Made*. The plan requires the refusal to name both issuers; it does not
consider that one of them is arbitrary attacker-supplied text of unbounded length.

**5. [Rule 2 — Missing critical functionality] A discovery request had no timeout**

`hardened_discovery_client` takes a timeout, and `with_settings` configures a RETRY budget rather
than one, so both constructors pass `DEFAULT_DISCOVERY_TIMEOUT` (30s). Before this plan
`OidcDiscoveryClient` used a bare `reqwest::Client::new()` with no timeout at all, so a hostile or
hung authorization server held the probe open indefinitely — and the ordered probe makes that
worse, not better, by adding candidates.

**Total deviations:** 5 (3 × Rule 1, 2 × Rule 2). No Rule 4 situation arose; no architectural
change was needed. **Zero dependencies added.**

### Known transient, recorded rather than hidden

The plan splits "define the helper" (Task 1) and "wire the helper" (Task 2) into two commits, so
at `18faf398` the thirteen items Task 2 consumes are unreferenced. Measured exactly:
`RUSTFLAGS="-D warnings" cargo build --features full --lib` → **13 `dead_code` errors**, one per
item, `target/116-verify/116-06-task1-deadcode.log`. `make lint` is **exit 0** at `f4a48195`.
Task 1 was verified with clause-(b) clippy (exit 0), its own 10-test suite, `pmat quality-gate`
(0 violations) and the `wasm32` build (exit 0). Recorded here because a bisect landing on
`18faf398` will see it.

## Issues Encountered

- **`make quality-gate` exits 2 — but at `test-integration`, on a failure this plan did not
  cause and which is NOT `D-116-KEYCHAIN`.** The single failure is
  `v2_bounded_reads_tripwire::every_peer_byte_accumulation_is_reviewed`, whose message names
  exactly one new site: `src/shared/credential_store.rs` `push_str(` at line 742 — 116-05's
  `normalize_server_key`. The other 12 tests in that binary pass, **including**
  `no_unbounded_whole_body_read_over_peer_supplied_bytes`, so this plan's new `src/shared/` file is
  clean and is not named. Logged as **`D-116-TRIPWIRE`**. It was not fixed here: the tripwire's
  allowlist is a REVIEWED-EXEMPTION register, and adding an entry on behalf of another plan's code
  is the silent exemption that file's own doc warns against.
- **Every OTHER `make quality-gate` stage passes**, in one run:
  `fmt-check` ✓, `lint` ✓ ("No lint issues"), `build` ✓, `test-unit` **1865 passed / 0 failed**,
  `test-doc` **445 passed / 0 failed / 79 ignored**, `test-examples` (all examples built) ✓.
  Log: `target/116-verify/116-06-quality-gate.log`.
- **mockito answers an unmatched request with 501, which the matrix classifies as `Retry`.** That
  is correct behaviour (501 is 5xx) but it means the pre-existing `oauth_dcr_integration` suite
  now spends ~1s per test retrying candidate 1 before falling back to candidate 2. All 5 tests
  still pass. Later plans writing mockito discovery fixtures should mock an explicit **404** for
  the candidates they want to fall through, which is what this plan's own suite does.
- **`cargo semver-checks` again reports "no semver update required"** despite two new public items
  — the fifth plan in this phase to observe it. The requirement (*zero breaking findings*) is met:
  223 checks, 223 pass, 0 fail, exit 0. `116-13` must not rest its version-bump reasoning on this
  tool's verdict.
- **Both halves of `D-116-DOC` applied cleanly on the first pass.** `make doc-check` `^error`
  count: **28** — exactly the anchor — with **0** hits for `http_body_cap` or `client/auth`.
- **`git commit -m` with a long message containing braces failed to parse under this environment's
  shell**, silently leaving the changes staged while printing what looked like commit output. Use
  `git commit -F <file>` for any multi-paragraph message here.

## Threat Flags

None. This plan adds no new network endpoint and no new schema; it CONSTRAINS an existing outbound
request path.

| Threat | Disposition | Discharged by |
|---|---|---|
| T-116-19 (discovery-document spoofing — forging AUTH-01's own anchor) | mitigate | `issuer_matches_metadata` called inside `fetch_discovery` before the metadata escapes; the lying-document fixture was **OBSERVED failing pre-fix**, returning `Ok` with `issuer: "https://honest.example"`. Also fenced by a differ-in-one-field pair, so a suite that could not distinguish "validated" from "not validated" would fail |
| T-116-20 (memory exhaustion from an oversized discovery/token body) | mitigate | every whole-body read routed through `collect_reqwest_body_within_cap` at 1 MiB, streaming with a running total checked before each append; reqwest whole-body needle count in `src/client/auth.rs` is **0**, and `no_unbounded_whole_body_read_over_peer_supplied_bytes` passes |
| T-116-21 (refusal message echoing refused bytes) | mitigate | both refusals name only the limit and the observed size; a planted canary is asserted absent from each, and the integration row asserts a 1.2 MB body's padding does not appear |
| T-116-22 (discovery regression for append-only authorization servers) | mitigate | the appended form is the LAST candidate, reached by the ordered probe; `the_probe_tries_the_spec_candidates_in_order_before_the_appended_form` asserts candidates 1 and 2 were attempted first by hit count, and `test_discovery_url_construction` pins the ordered list including the Entra-ID-shaped issuer |
| T-116-22a (silent downgrade — one candidate's security failure steering the client) | mitigate | `classify_discovery_failure` drives the loop and is the only branch; three integration rows put a VALID candidate 3 behind `expect(0)` + `assert_async()` for the lying issuer, the oversized body and the non-boolean flag |
| T-116-22b (a malformed RFC 9207 flag relaxing strictness) | mitigate | present-but-non-boolean aborts as `MalformedSecurityMetadata`; four shapes (`"true"`, `1`, `null`, `{}`) asserted at both the integration and unit level, and the same rule covers a missing or non-string `issuer` |
| T-116-22c (a discovery redirect leaving the issuer's origin) | mitigate | `hardened_discovery_client`'s custom policy; 5 tests — cross-origin refused with the target's `expect(0)` on a SECOND mockito server, scheme change refused, same-origin followed, loop bounded, and the pure decision rows including the same-port https→http case |
| T-116-22d (a poisoned or stale candidate-index cache) | mitigate | index-only storage; a failed cache hit restarts from candidate 0; a cache hit still runs the anchor check. Three tests, one per constraint |
| T-116-SC (cargo installs) | mitigate | zero packages added; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0**; `mockito` was already a dev-dependency |

## Known Stubs

None. Every item is fully implemented and exercised. The one deliberately unreachable branch — the
`None => attempt.error(unjudgeable_redirect_refusal())` arm in the redirect policy — is documented
in place as fail-closed, with the measurement that makes it unreachable (reqwest pushes the
redirecting URL onto `previous` before consulting the policy, `redirect.rs:316`), and is not a
stub.

## TDD Gate Compliance

Both tasks carry `tdd="true"`. **RED was observed and logged for both, before any implementation
existed:**

| Task | RED log | Diagnostics |
|---|---|---|
| 1 | `target/116-verify/116-06-task1.RED.log` | **24** × `E0425`/`E0433`, exit **101** — the module held only its `#[cfg(test)]` block |
| 2 | `target/116-verify/116-06-task2.RED.log` | **14 tests run, 2 passed, 12 failed** against the PRE-FIX `fetch_discovery` — a behavioural RED, not a compile error |

Task 2's RED is the stronger of the two and is the plan's own acceptance criterion: the test file
was written to use only `discover()`, which already existed, so it COMPILED against the unfixed
tree and the lying-document fence could be observed failing rather than merely failing to build.
The recorded diagnostic:

```
panicked at tests/oauth_discovery_validation.rs:132:55:
called `Result::unwrap_err()` on an `Ok` value: OidcDiscoveryMetadata {
  issuer: "https://honest.example",
  authorization_endpoint: "http://127.0.0.1:63736/authorize", ... }
```

The `discover_with_extras` tests were appended after the implementation landed, since a test naming
a non-existent method cannot compile.

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01`
(`ea1d2d68`), `116-02`, `116-03`, `116-04` and `116-05`: in Rust a test naming a non-existent
function fails to *compile*, so such a commit leaves a non-building tree that breaks `git bisect`
and contradicts CLAUDE.md's "ZERO TOLERANCE FOR DEFECTS". A verifier looking for a `test(...)` →
`feat(...)` pair will not find one; the evidence is the RED logs above and the negative control
below.

### Negative control — Task 1 (`target/116-verify/116-06-task1.NEGATIVE-CONTROL.log`)

Three deliberate breaks applied **at once**, run with `--no-fail-fast` and the denominator
asserted: `10 tests run: 6 passed, 4 failed`.

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| refusal 1 (the `Content-Length` early exit) removed | `within_cap_refuses_a_declared_content_length_over_the_cap` — the mid-flight refusal fired instead and does not name the DECLARED size | `within_cap_refuses_a_chunked_body_that_exceeds_the_cap_mid_flight` still passed, so refusal 2 is its own independent detector |
| the boundary changed to `>=` (one byte off) | `within_cap_admits_a_body_exactly_at_the_cap` **only** | both refusal rows, the under-cap row and the empty-body row all held — the boundary test is not a side effect of the refusal tests |
| the SCHEME dropped from the origin comparison | `hardened_discovery_client_refuses_a_scheme_change_on_the_same_host` (wire), and — **only after the row described in Deviation 1 was added** — `hardened_discovery_client_permits_only_same_origin_redirect_targets` (pure) | `hardened_discovery_client_refuses_a_cross_origin_redirect` (different PORT), `..._follows_a_same_origin_redirect` and `..._bounds_a_redirect_loop_within_one_origin` all held — host, port and count are separate detectors from scheme |

Source restored afterwards; the three break sites were re-verified absent by grep, and the suite
re-ran **10 passed / 0 failed**.

### Negative control — Task 2

Task 2's negative control **is** its RED run, and it is stronger than an injected break: the
"break" was the real, shipped pre-fix implementation. `12 of 14` failed, including every anchor
row, every probe-order row, all three TERMINAL rows and all three cache rows. The 2 survivors are
named in *Accomplishments* and are exactly the two rows the pre-fix single-URL behaviour happened
to satisfy — which is itself the attribution argument.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| Task 1 suite | `-E 'binary(pmcp) and (test(within_cap) + test(hardened_discovery_client))'` | **10 run, 10 passed** |
| Task 2 suite (gated) | `-E 'binary(oauth_discovery_validation)'`, `--features full,oauth` | **19 run, 19 passed** |
| Task 2 suite (**narrow-gate proof**) | same, `--features full` only | **19 run, 19 passed** |
| `nextest list` count | `--features full,oauth -E 'binary(oauth_discovery_validation)'` | **19** (non-zero) |
| no regression | `oauth_dcr_integration + oauth_discovery_urls + oauth_credential_store + oauth_iss_validation + (binary(pmcp) and test(client::auth))` | **135 run, 135 passed** |
| DCR suite specifically | `-E 'binary(oauth_dcr_integration)'` | **5 run, 5 passed** (≥ 5 ✓) |
| bounded-reads tripwire | `-E 'binary(v2_bounded_reads_tripwire)'` | **12 passed, 1 failed** — the failure is `credential_store.rs:742`, **D-116-TRIPWIRE**, pre-existing |
| doctests | `cargo test --features full,oauth --doc client::auth` | **8 passed** |
| needle count | `grep -c '<4 reqwest needles>' src/client/auth.rs` | **0** |
| needle count | same, `src/shared/http_body_cap.rs` | **0** (including its prose) |
| no field added | `grep -n 'pub authorization_response_iss_parameter_supported' src/server/auth/oauth2.rs` | **no output** |
| `discover` signature | `grep -n 'pub async fn discover' src/client/auth.rs` | `:249 discover(&self, issuer_url: &str) -> Result<OidcDiscoveryMetadata>` (unchanged) + `:270 discover_with_extras` |
| classification drives the loop | `grep -n 'classify_discovery_failure\|silent DOWNGRADE' src/client/auth.rs` | `:356` in `probe_candidate`, with the risk named at `:286` |
| hardened client wired | `grep -n 'hardened_discovery_client' src/client/auth.rs` | `:215` in `with_settings` |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over all three files | **no output** |
| lint (**authoritative**, D-116-LINT) | `/usr/bin/make lint` | **✓ No lint issues** (after Deviation 2) |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** attributable, first pass |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, 92 warnings (= 116-BASELINES anchor), **0** naming either file |
| gate: `test-unit` | inside `make quality-gate` | **1865 passed; 0 failed** — **D-116-KEYCHAIN does not reproduce** |
| gate: `test-doc` | inside `make quality-gate` | **445 passed; 0 failed; 79 ignored** |
| gate: `test-examples` | inside `make quality-gate` | all examples built ✓ |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 2 — `test-integration` only**, on `D-116-TRIPWIRE` (116-05's `credential_store.rs:742`). Every other stage green |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-TRIPWIRE` (new, and the most urgent)** —
  `v2_bounded_reads_tripwire::every_peer_byte_accumulation_is_reviewed` has been RED since
  `ec80e5b1` because of `src/shared/credential_store.rs:742`. `make quality-gate` runs
  `test-integration`, so this is a **gate-red condition introduced inside this phase** and it would
  fail CI. The fix is one reviewed ALLOWLIST entry naming the bound (`port` is a `u16`, so at most
  six bytes are appended once), not a code change. Proposed owner: `116-15` or an immediate
  `116-05` follow-up — every later plan touching `src/shared/` now inherits it.
- **`D-116-KEYCHAIN` — RESOLVED by measurement, no source change owed.** 1865 passed / 0 failed on
  a clean volume; the arithmetic and both greps are in the deferred-items entry. It is an
  environment artifact of the same family as `D-116-DISK`. The entry also records that this one
  gate run consumed 42 GiB, so the failure regime is reproducible in both directions, and revises
  the two candidate resolutions: do NOT change `streamable_http.rs:458` on this evidence.
- **`D-116-LINT` — reconfirmed, sixth measurement, with a NEW shape** (two gate lints that
  contradict each other inside a `pub(crate)` module). See Deviation 2.
- **`D-116-DISK`** — hit indirectly: the volume went 71 GiB → 29 GiB across a single
  `make quality-gate`. Guidance confirmed.
- **`D-116-FAILFAST`** — applied: both negative controls and every regression run used
  `--no-fail-fast` with the denominator asserted.
- **`D-116-EX`** — still open. This plan adds no `examples/` binary and its doctests do not
  discharge it, for the same reason 116-02's 5, 116-03's 3, 116-04's 7 and 116-05's 9 did not.
- **`D-116-DOC`** — applied as amended, both halves, zero new errors. No further action.

## Next Phase Readiness

| Consumer | What it can now rely on |
|---|---|
| `116-07` (`generic_oidc.rs`, `cognito.rs`) | `hardened_discovery_client` and `collect_reqwest_body_within_cap` exist and are `pub(crate)` in `crate::shared::http_body_cap`. **Copy the probe loop shape from `discover_with_extras`/`probe_candidate` rather than reinventing it** — the `Retry`-then-`Fallback` composition and the three-TERMINAL abort are subtle enough that a second implementation will drift. `is_body_over_cap` and `is_redirect_refusal` are the two classification seams |
| `116-09` | `discover_with_extras(&str) -> Result<(OidcDiscoveryMetadata, AuthorizationServerExtras)>`, with `iss_parameter_supported() -> Option<bool>` guaranteed to be `None` only when the key was ABSENT — a malformed value never reaches it |
| `116-12` | `collect_reqwest_body_within_cap` is the shape the DCR read at `src/client/oauth.rs:281-291` should become; `DEFAULT_AUTH_RESPONSE_BYTES` is the same 1 MiB that site already applies, so it is a change of mechanism only |
| `116-15` | `make quality-gate`'s only red stage at this HEAD is `test-integration` on `D-116-TRIPWIRE`; `test-unit` and `test-doc` are both **fully green**, which is new for this phase |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-15` / `116-05` follow-up | close `D-116-TRIPWIRE` — it is a CI-blocking condition, not an advisory |
| `116-07` | do NOT build a second `reqwest::Client` for discovery; both call sites must share `hardened_discovery_client` or the policy diverges silently |
| every source-touching plan | run `make lint`, not clause (b) alone (`D-116-LINT`, now 6× measured); run `df -h /` before believing any gate failure |
| `116-15` | close or waive `D-116-EX`; do not book `AUTH-01`/`AUTH-03` on this plan's evidence alone |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: src/shared/http_body_cap.rs                                716 lines
FOUND: src/client/auth.rs                                        1101 lines (was 531)
FOUND: tests/oauth_discovery_validation.rs                        538 lines (min_lines 120 ✓)
FOUND: src/shared/mod.rs                                          +12
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md 377 lines (was 278)
```

Commits claimed, verified in `git log`:

```
FOUND: 18faf398  feat(116-06): streaming bounded auth body read and an origin-pinned discovery client
FOUND: f4a48195  feat(116-06): RFC 8414 anchor validation, the SEP-2351 ordered probe and five bounded reads
```

`must_haves` verification:

```
✓ truths[1] a document whose issuer differs from the issuer used to build the URL is REJECTED
  before the metadata escapes the fetch function — issuer_matches_metadata is called inside
  fetch_discovery between the deserialize and the Ok; OBSERVED failing pre-fix
✓ truths[2] the probe falls through ONLY on discovery-eligible failures — three TERMINAL rows,
  each with a VALID candidate 3 behind expect(0) + assert_async()
✓ truths[3] a path-bearing issuer does not re-pay two 404s, and a cache hit still re-runs the
  full anchor check — three cache tests, one per constraint
✓ truths[4] a discovery redirect that leaves the issuer's origin is refused — 5 tests, the
  cross-origin one proven by expect(0) on a SECOND mockito server
✓ truths[5] the RFC 9207 flag is observable without a field on OidcDiscoveryMetadata —
  AuthorizationServerExtras + discover_with_extras; grep for the field name in oauth2.rs is empty
✓ truths[6] no whole discovery or token response body is allocated beyond its cap — needle count
  0 in src/client/auth.rs, five reads rewritten, tripwire's whole-body test passes
✓ artifacts: src/shared/http_body_cap.rs provides collect_reqwest_body_within_cap and
  hardened_discovery_client and contains "content_length" (:125)
✓ artifacts: src/client/auth.rs contains "discover_with_extras" (:270) and provides the ordered
  probe, the cache, the anchor check, AuthorizationServerExtras and the bounded reads
✓ artifacts: tests/oauth_discovery_validation.rs 538 >= 120, with the lying-document negative
  control and the probe-order assertions
✓ key_links: src/client/auth.rs references discovery_url_candidates and issuer_matches_metadata
✓ key_links: src/client/auth.rs references collect_reqwest_body_within_cap
```

Plan-level verification block:

```
✓ binary(oauth_discovery_validation) green with a non-zero count (19/19), under BOTH feature sets
✓ no regression in binary(oauth_dcr_integration) — 5/5 (requirement: 5 or more)
✓ the lying-document fence recorded as OBSERVED failing before the fix, with the Ok value quoted
✓ make lint exit 0; clause (b) clippy exit 0
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
⚠ make quality-gate — exit 2 at test-integration ONLY, on D-116-TRIPWIRE (116-05's
  credential_store.rs:742). test-unit 1865/0, test-doc 445/0, every other stage green.
  D-116-KEYCHAIN did NOT reproduce on the clean volume
○ HUMAN-CHECK (network, optional): the Microsoft Entra ID re-probe was NOT re-run. RESEARCH's
  200/404/404 measurement of 2026-08-02 stands, and the plan makes the re-run conditional on the
  probe-order tests behaving unexpectedly, which they did not
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
