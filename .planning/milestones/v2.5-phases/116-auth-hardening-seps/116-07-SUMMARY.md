---
phase: 116-auth-hardening-seps
plan: 07
subsystem: auth
tags: [oauth, oidc, rfc8414, sep-2351, discovery, providers, cognito, generic-oidc, bounded-reads, redirect-policy, mockito, semver]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 04
    provides: "discovery_url_candidates, issuer_matches_metadata, DiscoveryFailure/DiscoveryOutcome/classify_discovery_failure — the pure tier both providers now drive"
  - phase: 116-auth-hardening-seps
    plan: 06
    provides: "collect_reqwest_body_within_cap, hardened_discovery_client, is_body_over_cap, is_redirect_refusal — and the reference probe-loop shape in src/client/auth.rs that this plan mirrors twice"
provides:
  - "All THREE discovery call sites in the crate now share one spec-ordered, anchor-validated, bounded path — the client (116-06) plus both server-side identity providers"
  - "The RFC 8414 §3.3 anchor enforced in generic_oidc.rs and cognito.rs, each OBSERVED accepting the specification's worked attack pre-fix"
  - "Zero reqwest whole-body needles in either provider file — 9 reads rewritten, success AND error paths"
  - "The cognito trailing-slash defect closed: a trailing-slash issuer no longer requests `...//.well-known/openid-configuration`"
  - "An anchor-rejected document is never written to the Cognito TTL cache, asserted by a second call re-attempting the fetch"
  - "D-116-SLASH — the measured, deliberate divergence between a normalising derivation and a non-normalising anchor, and the one CHANGELOG line 116-13 owes because of it"
  - "make quality-gate exit 0 end-to-end at this HEAD — the first plan in this phase to record it since 116-03"
affects: [116-09, 116-12, 116-13, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "When a public constructor DERIVES its endpoint from configuration (CognitoProvider::new builds its issuer from region + pool), no integration test can aim it at a mock — put the wire-level rows inline rather than widening the public API to make them reachable"
    - "A vestigial test that re-implements the production expression it is supposed to check is green while measuring nothing, and it PINS the defect being removed; rewrite it to call the production derivation"
    - "Restore a deliberately broken file from a scratchpad COPY, never `git checkout --` — on uncommitted work that command discards the whole task, not the break"
    - "Two rules that disagree on purpose (a normalising URL derivation and a non-normalising security anchor) need a test asserting the DISAGREEMENT, or the next reader will 'fix' one of them"
    - "Prove a retry BUDGET was spent with expect(N) on the failing candidate — 'retried then fell back' and 'fell back immediately' produce the same Ok"

key-files:
  created:
    - tests/oauth_provider_discovery.rs
  modified:
    - src/server/auth/providers/generic_oidc.rs
    - src/server/auth/providers/cognito.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "NEITHER provider gets a candidate-index cache: the plan's premise that generic_oidc has no TTL cache is factually wrong (generic_oidc.rs:334-355 has one), so both are treated the same way for the same reason"
  - "Discovery gets its OWN hardened client rather than repurposing the provider's general-purpose one — the origin pin must not silently change token/UserInfo redirect behaviour"
  - "Cognito's wire-level rows are INLINE because its constructor derives its issuer from region + user_pool_id and this plan adds no public surface"
  - "The trailing-slash anchor refusal is KEPT, pinned by its own test, and handed to 116-13 as a CHANGELOG line — normalising either side would delete the fence this phase installs"
  - "Two vestigial generic_oidc URL tests were REWRITTEN, not preserved: they re-implemented the removed format! and pinned the single-URL shape being replaced"
  - "AUTH-03 is NOT booked complete — 116-09/10/11/12/13/16 all still claim it"

patterns-established:
  - "D-116-LINT now has SEVEN and EIGHT measurements, both in TEST code (doc_markdown, duration_suboptimal_units, items_after_statements)"
  - "A test can fail against a CORRECT implementation and be the more valuable outcome — the trailing-slash row exposed D-116-SLASH, which no amount of reading would have surfaced"

requirements-completed: []

# Metrics
duration: 214min
completed: 2026-08-04
---

# Phase 116 Plan 07: Both Server-Side Providers on the Ordered, Anchor-Validated, Bounded Discovery Path Summary

**Fixing one of three discovery call sites would have left a multi-tenant identity provider broken
in the other two and left two more unvalidated anchors in the tree. Both are now closed, and both
were OBSERVED accepting the specification's own worked attack first: `GenericOidcProvider::new`
returned `Ok` for a document declaring `issuer: "https://honest.example"` fetched from `127.0.0.1`,
and `CognitoProvider::discovery()` returned `Ok(OidcDiscovery { issuer:
"https://honest.example", authorization_endpoint: "http://127.0.0.1:63471/authorize", .. })` for the
same fixture. Both also returned `Ok` for a 1.2 MB body. Nine whole-body reads across the two files
were rewritten, so the reqwest needle count in each is 0 — error paths included, because a hostile
identity provider controls those too. Cognito's missing `trim_end_matches('/')`, which made a
trailing-slash issuer request `...//.well-known/openid-configuration`, is closed by routing through
the shared derivation and pinned by a test.**

**`make quality-gate` exits 0 end-to-end at this HEAD — `test-unit` 1880 passed / 0 failed,
`test-doc` 445 passed / 0 failed, and the `v2_bounded_reads_tripwire` binary green inside the gate.
That is the first clean full-gate run recorded in this phase since `116-03`.**

## Performance

- **Duration:** ~214 min
- **Completed:** 2026-08-04
- **Tasks:** 2
- **Files:** 4 (1 created, 3 modified), **+1824 / −101** across the two task commits

## Accomplishments

- **The Pitfall 1 fence was observed FIRING on the real shipped code, twice, and the diagnostics are
  quoted rather than paraphrased.** `116-07-task1.RED.log`: **15 tests run, 5 passed, 10 failed**,
  `--no-fail-fast`, denominator asserted. `116-07-task2.RED.log`: **27 run, 16 passed, 11 failed**.
  In both cases the "break" was not injected — it was the provider as it ships today. The five
  survivors in Task 1 and the sixteen in Task 2 are the positive controls and the pure-derivation
  rows, which is itself the attribution argument.

- **A fall-through and a correct probe produce the same `Ok`, so every ordering and terminality
  claim is proven by mock hit counts and `expect(0)` guards.** Three TERMINAL rows per provider — a
  lying issuer, a body over the 1 MiB cap, and a refused cross-origin redirect — each put a
  **perfectly valid** document behind candidate 3 with `expect(0)` and `assert_async()`. If either
  classification match were ever "simplified" back to `if !ok { continue }`, all six would fail,
  because the caller would take its metadata from the candidate the attacker chose. The risk is
  named in a comment at both classification sites.

- **The Cognito TTL cache is preserved in all three of its load-bearing properties, each its own
  test.** It still short-circuits BEFORE any candidate is probed (one hit across two in-TTL calls);
  a miss after expiry re-probes the ordered candidates **from the top** rather than resuming at the
  previously-successful one (two hits on each of the three candidates); and an anchor-REJECTED
  document is **never** written to it — asserted by a second call re-attempting the fetch, because
  caching a rejected document turns a one-shot spoof into a persistent one for the whole TTL.

- **A test that FAILED against a correct implementation produced the most valuable finding in the
  plan.** The trailing-slash row was written with the mock declaring a slash-free issuer, and the
  refusal it triggered is correct: the URL derivation normalises a trailing slash away (116-04's
  decision) while the RFC 8414 §3.3 anchor does not and must not (116-04's four normalisation rows).
  The two rules disagree by design, and the disagreement is operator-visible. It is now pinned by
  `a_trailing_slash_issuer_still_needs_a_byte_identical_document_issuer`, written up as
  **`D-116-SLASH`**, and handed to `116-13` as one CHANGELOG line. See *Deviations*.

- **Zero new public API and zero packages.** `cargo semver-checks check-release -p pmcp
  --baseline-rev b2bf9157`: **223 checks, 223 pass, 0 fail**, exit 0. `discovery_client` is a new
  PRIVATE field on two structs whose fields are all private and which are only constructible through
  their own `Result`-returning constructors, so the hardened client's build failure propagates with
  `?` and needed none of 116-06's carried-`Result` machinery.
  `git diff --exit-code b2bf9157..HEAD -- Cargo.toml`: exit **0**.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | `generic_oidc` — ordered probe, RFC 8414 anchor, bounded reads | `6b1ba528` | feat |
| 2 | `cognito` — the same three, with the TTL cache preserved | `3544318d` | feat |

## Files Created/Modified

- **`src/server/auth/providers/generic_oidc.rs`** (**modified**, 1140 → **1593** lines,
  +513/−59). New private items: `DISCOVERY_TIMEOUT`, `DISCOVERY_MAX_ATTEMPTS`,
  `DISCOVERY_RETRY_DELAY`, `MAX_ECHOED_DOCUMENT_ISSUER`, `probe_discovery_candidate`,
  `fetch_discovery_candidate`, `discovery_document_issuer`, `discovery_request_failure`,
  `discovery_status_failure`, `unparseable_discovery_document`, `discovery_issuer_mismatch`,
  `malformed_discovery_metadata`, `every_candidate_failed`, `truncate_for_message`,
  `rendered_source_chain`, `read_json_within_cap`, `read_error_body_within_cap`, plus the
  `discovery_client` field. `fetch_discovery_doc` rewritten in place with its signature unchanged.
  Inline tests 26 → **29** (2 rewritten, 5 added).
- **`src/server/auth/providers/cognito.rs`** (**modified**, 818 → **1607** lines, +832/−42). The
  same item set, mirroring its sibling exactly, plus the `discovery_client` field. `discovery()`'s
  fetch body replaced by one call; its TTL cache read and write are untouched. Inline tests
  14 → **26** (**12** new discovery rows).
- **`tests/oauth_provider_discovery.rs`** (**created**, **480** lines — `min_lines` 110 ✓).
  **15** tests in six documented groups. `#![cfg(feature = "http-client")]`, **not** `oauth`.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (531 → **~600**) — two new
  entries (`D-116-SLASH`, and the seventh/eighth `D-116-LINT` measurements).

## Decisions Made

- **NEITHER provider gets a candidate-index cache, and the plan's premise for that choice is
  corrected.** The plan says "`cognito.rs` already has a TTL cache above the fetch, so it does not
  need one; `generic_oidc.rs` has none, so add the same index-only cache". Measured:
  `generic_oidc.rs:334-355` (`fetch_discovery`) reads a TTL cache *directly above* the fetch, exactly
  as `cognito.rs:259-267` does. Applying the plan's own stated rule consistently, neither needs a
  second cache: the ordered candidates are probed at most once per `cache_ttl` (default **1 hour**),
  so a path-bearing issuer pays its two 404s once an hour rather than once per call. Adding an index
  cache would have introduced a second cache layer with three separate safety constraints to prove,
  for a path that runs 24 times a day. Recorded in both files' `fetch_discovery`/module docs so the
  reasoning is where the next reader looks.
- **Discovery gets its OWN `reqwest::Client`.** `hardened_discovery_client`'s redirect policy is a
  statement about who may AUTHOR a metadata document. Applying it to the provider's single shared
  client would also govern the token, `UserInfo`, revocation and DCR endpoints — several of which
  legitimately redirect at real providers — which is a behaviour change this plan does not own. Both
  providers build theirs from the same function, so 116-06's carried obligation ("both call sites
  must share `hardened_discovery_client` or the policy diverges silently") is met by construction.
- **Cognito's wire-level rows are INLINE, not in the integration file.**
  `CognitoProvider::new(region, user_pool_id, client_id)` DERIVES its issuer as
  `https://cognito-idp.{region}.amazonaws.com/{user_pool_id}`, so no public constructor can be aimed
  at a `mockito` server. The alternatives were to add a test-only public constructor — which this
  plan explicitly must not do, since it "creates no new public surface" — or to test Cognito's
  discovery only indirectly. Building the struct directly is something only a module-internal test
  can do, so that is where the 12 rows live. The reachable-from-outside rows (the REAL provider's
  issuer flowing into the shared derivation, and the trailing-slash arithmetic) are in
  `tests/oauth_provider_discovery.rs`, and the split is documented in both files' headers.
- **The probe loop is written out in BOTH providers rather than extracted into a shared module.**
  This is the plan's explicit instruction ("mirroring Task 1's implementation so the two providers
  stay reviewable as a pair") and its acceptance criteria depend on it: they grep for
  `classify_discovery_failure`, `discovery_url_candidates` and `hardened_discovery_client` **inside
  each file**. The security decisions themselves are NOT duplicated — they live in the shared pure
  tier (`discovery_url_candidates`, `issuer_matches_metadata`, `classify_discovery_failure`) and the
  shared I/O tier (`collect_reqwest_body_within_cap`, `hardened_discovery_client`); what is repeated
  is the glue. Each file's module doc names the other two implementations and states that a change
  to one belongs in all three. Consolidating the glue remains available to a later plan and is
  noted here rather than done unasked.
- **Two vestigial `generic_oidc` tests were REWRITTEN rather than left passing.**
  `test_discovery_url_format` and `test_discovery_url_format_with_trailing_slash` re-implemented the
  very `format!` this plan removes and asserted its output. They were green while measuring nothing
  about the provider, and worse, they *pinned the single-URL shape being replaced*. They now assert
  the ordered candidate list the provider actually derives. This is the one place the plan's "all
  existing `generic_oidc` tests still pass unchanged" is not literally satisfied, and leaving them
  would have been the defect.
- **`AUTH-03` is NOT booked complete.** This plan lands the ordered probe and the anchor at the last
  two of three discovery call sites; `116-09`–`116-16` still claim the requirement.
  `requirements-completed: []`, as in `116-01` through `116-06`.
- **RED was OBSERVED and logged for both tasks but NOT committed as a broken build**, for the
  seventh time in this phase and for the same reason. See *TDD Gate Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's premise for the candidate-index cache is factually wrong**

- **Found during:** Task 1, reading `generic_oidc.rs` in full as the plan's `<read_first>` requires.
- **Issue:** the plan's judgement call rests on "`generic_oidc.rs` has none [no TTL cache], so add
  the same index-only cache". `generic_oidc.rs:334-355` has one, structurally identical to
  `cognito.rs:259-267`: a read that returns early when unexpired, and a write after a successful
  fetch.
- **Why it mattered:** adding an index cache on a false premise would have shipped a second cache
  layer, with 116-06's three safety constraints to re-prove, protecting a path that runs at most
  once per hour. The plan's own rule, applied to the measured facts, says neither provider needs one.
- **Fix:** no candidate cache in either provider; the reasoning is written into both
  `fetch_discovery` doc comments and both module docs. The plan's instruction to "record in the
  summary which providers got one and why" is discharged here: **neither, and this is why.**
- **Committed in:** `6b1ba528` and `3544318d`.

**2. [Rule 1 — Bug] A trailing-slash test failed against a CORRECT implementation, exposing D-116-SLASH**

- **Found during:** Task 2, first green run of the Cognito suite: 55 tests, 54 passed, **1 failed**.
- **Issue:** the test configured the provider with `…/us-east-1_TEST/` and had the mock declare
  `…/us-east-1_TEST`. The implementation refused it, quoting both values. That refusal is CORRECT —
  RFC 8414 §3.3 requires the two to be identical with no normalisation — but the test's purpose was
  the URL arithmetic, not the anchor, so the fixture was wrong.
- **Why it mattered more than a red test:** it surfaced a real, operator-visible behaviour change
  that nothing in the plan, RESEARCH or CONTEXT anticipates. The derivation normalises a trailing
  slash; the anchor does not. An operator whose configured issuer differs from their provider's
  declared issuer by a slash now gets a hard refusal where they previously got a working provider.
- **Fix:** the row's mock now declares the issuer WITH its slash, exactly as Auth0 and every other
  trailing-slash issuer does, so it proves only the arithmetic. A **new sibling row**,
  `a_trailing_slash_issuer_still_needs_a_byte_identical_document_issuer`, pins the refusal
  deliberately and asserts it names both values. Written up as **`D-116-SLASH`** with the exposure
  analysis (every in-repo constructor — `auth0`, `google`, `okta`, `entra`, `CognitoProvider::new` —
  already matches its provider's declared form) and handed to `116-13` as one CHANGELOG line.
  Softening either rule was rejected: normalising the anchor deletes the fence, and normalising the
  configured issuer makes `https://attacker.example/` and `https://attacker.example` equivalent
  anchors, which is the reasoning §3.3 exists to forbid.
- **Committed in:** `3544318d`.

**3. [Rule 1 — Bug] Two `generic_oidc` tests pinned the defect being removed**

Described under *Decisions Made*. Committed in `6b1ba528`.

**4. [Rule 1 — Bug] `D-116-LINT`, seventh and eighth measurements — both in TEST code**

- **Found during:** Tasks 1 and 2, running `make lint` per the standing obligation.
- **Issue:** `make lint` exited **101** twice on code the phase's clause-(b) command accepts,
  because clause (b) omits `RUSTFLAGS="-D warnings"`:
  1. `clippy::doc_markdown` — bare `IdP` in `tests/oauth_provider_discovery.rs`'s module doc;
  2. `clippy::duration_suboptimal_units` (nursery) — `Duration::from_secs(3600)` in a `cognito.rs`
     test helper, where `Duration::from_hours(1)` is the required form;
  3. `clippy::items_after_statements` — `const ATTEMPTS` declared mid-function in a `cognito.rs`
     test.
- **Fix:** backticked `IdP`; `from_hours(1)`; the const moved to the head of its scope. `make lint`
  → **exit 0** after each.
- **Why it is recorded rather than shrugged off:** all three are in **test** code, reconfirming
  116-04's note that `make lint` covers `--lib --tests`, and this is now the seventh and eighth
  independent measurement across five plans.
- **Committed in:** `6b1ba528` and `3544318d`.

**Total deviations:** 4 (4 × Rule 1). No Rule 2 addition was needed — 116-06 had already added the
message-truncation and timeout hardening this plan mirrors. No Rule 4 situation arose. **Zero
dependencies added.**

### Process incident, recorded rather than hidden

**`git checkout -- <file>` destroyed Task 1's uncommitted implementation.** After the Task 1
negative control, the three deliberate breaks were reverted with
`git checkout -- src/server/auth/providers/generic_oidc.rs`. Because the task's work had not yet
been staged, that restored the file to **HEAD — the pre-plan version** — discarding the whole
implementation, not just the breaks. `shasum -a 256 -c` caught it immediately (`FAILED`,
`WARNING: 1 computed checksum did NOT match`), and the work was reapplied from the exact edit
strings, re-formatted, and re-verified to the identical suite result (15/15) before committing.

The executor's own guidance names `git checkout -- path` as the safe way to discard changes to a
specific file; it is safe only for changes made **since the last commit of that file's other
content**. For a negative control on uncommitted work the correct restore is a scratchpad **copy**,
which is what Task 2 used (`cognito.rs.bak` + a `shasum -c` that returned `OK`). Recorded because
the failure mode is silent — the command succeeds, and only a checksum reveals what it took.

## Issues Encountered

- **`make quality-gate` exits 0, end to end.** `fmt-check` ✓, `lint` ✓ ("No lint issues"),
  `build` ✓, `test-unit` **1880 passed / 0 failed**, `test-doc` **445 passed / 0 failed / 79
  ignored**, `test-integration` ✓ (including `v2_bounded_reads_tripwire`), `test-examples` ✓ (all
  examples built), `pmcp-package-gate` ✓, team-servers binding check ✓. Log:
  `target/116-verify/116-07-quality-gate.log`. This is the first plan in the phase to record a clean
  full gate since `116-03`.
- **The unit-test arithmetic closes exactly, which is what makes `D-116-KEYCHAIN`'s resolution hold
  up under a second observation.** `116-06` measured 1865 on a clean volume; this plan measures
  **1880**. `1865 + 15 = 1880`, where 15 is precisely this plan's inline contribution (12 new
  `cognito` rows, plus `generic_oidc` going 26 → 29). Two greps over the whole gate log confirm the
  keychain mechanism did not merely go quiet: `streamable_http.rs:4` → **0** hits,
  `Failed to load native root certificates` → **0** hits. Disk was 159 GiB free at start and
  136 GiB at finish, so the gate consumed ~23 GiB — consistent with `116-06`'s 42 GiB across a
  colder tree.
- **`D-116-FUZZGATE` reconfirmed, unchanged.** Inside this gate run, `make test-fuzz` produced
  **21** × ``the option `Z` is only accepted on the nightly compiler`` and **21** ×
  `Fuzz target … completed`, and the gate still exited 0. The FUZZ stage executed zero iterations.
  Not this plan's to fix; `116-08` already met the real obligation explicitly.
- **`D-116-LINT-OAUTH` measured again and unchanged.** The gate-equivalent command with
  `--features "full,oauth"` (same `RUSTFLAGS`, same 28-entry allow-list) exits **101** with
  **29 errors, all 29 in `src/client/oauth.rs`** — exactly the recorded anchor — and **0** in any
  file this plan touched. Log: `target/116-verify/116-07-clippy-oauth.raw.log`.
- **The `rtk` command proxy aggregates `cargo clippy` output and silently hides the `^error` lines.**
  A first attempt at the `oauth` lint measurement reported `0 matches for '^error'` while the command
  exited 101, because the proxy had rewritten the output into a per-rule summary. Re-running through
  `$HOME/.cargo/bin/cargo` gave the raw diagnostics. This is the project-memory "rtk output
  corruption" hazard, hit again — **use the absolute binary path for any command whose output you
  intend to count.**
- **`cargo semver-checks` again reports "no semver update required"** — the sixth plan in this phase
  to observe it. The requirement (*zero breaking findings*) is met: 223 checks, 223 pass, 0 fail,
  exit 0. `116-13` must not rest its version-bump reasoning on this tool's verdict.
- **Both halves of `D-116-DOC` applied cleanly.** One `///` item doc initially carried a
  fully-qualified intra-doc link (the `//!` rule applied to the wrong placement); it was corrected
  to the bare form before the first `doc-check`. `make doc-check` `^error` count: **28** — exactly
  the anchor — with **0** hits for `generic_oidc` or `cognito`.
- **`mockito` answers an unmatched request with 501, which the matrix classifies as `Retry`.** With
  a 200 ms retry delay and a 3-attempt budget that is ~400 ms of silent sleeping per unmocked
  candidate. Every candidate expected to fall through in this plan's suites is mocked as an explicit
  **404**, per 116-06's guidance, and the fixtures say so in a comment.

## Threat Flags

None. This plan adds no network endpoint and no schema; it CONSTRAINS two existing outbound request
paths.

| Threat | Disposition | Discharged by |
|---|---|---|
| T-116-23 (provider discovery-document issuer spoofing) | mitigate | `issuer_matches_metadata` called inside each provider's `fetch_discovery_candidate` before the document escapes; **OBSERVED failing pre-fix in BOTH providers**, with the accepted `Ok` values quoted in *TDD Gate Compliance*. Each fenced additionally by a differ-in-one-field pair (`generic_oidc`) and by the anchor-not-cached row (`cognito`) |
| T-116-24 (a spoofed document persisting in the Cognito TTL cache) | mitigate | the anchor check runs inside the fetch helper, so `?` short-circuits before the cache write; `an_anchor_rejected_cognito_document_is_never_cached` asserts a second call re-attempts the fetch (`expect(2)` on the lying mock) |
| T-116-25 (oversized provider response body) | mitigate | all **9** whole-body reads across the two files routed through `collect_reqwest_body_within_cap` at 1 MiB; reqwest needle count is **0** in both; an over-cap discovery body is TERMINAL, asserted with a valid candidate 3 behind `expect(0)` in both providers |
| T-116-26 (multi-tenant IdP unreachable due to a wrong discovery URL) | mitigate | both providers on the shared `discovery_url_candidates`; probe ORDER proven by hit counts in both; the Cognito trailing-slash defect closed and pinned |
| T-116-26a (silent downgrade at a provider) | mitigate | `classify_discovery_failure` drives both probe loops and is the only branch; three TERMINAL rows per provider, each with a well-formed candidate 3 behind `expect(0)` + `assert_async()`. The negative controls confirm: removing the `Terminal` arm failed exactly those rows while probe-order and retry rows survived |
| T-116-26b (a provider following a cross-origin discovery redirect) | mitigate | both providers issue discovery through `hardened_discovery_client`; each has a wire-level row using a SECOND mockito server (a genuinely different origin) with `expect(0)` on the target, and asserts candidate 3 was never reached either — a refused redirect is TERMINAL |
| T-116-SC (cargo installs) | mitigate | zero packages added; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0**; `mockito` was already a dev-dependency |

## Known Stubs

None. Every item is fully implemented and exercised. No placeholder value, empty collection or
"not available" string was introduced; `grep -nE 'TODO|FIXME|HACK|XXX'` over all three source files
returns no output.

## TDD Gate Compliance

Both tasks carry `tdd="true"`. **RED was observed and logged for both, against the SHIPPED pre-fix
providers rather than against a stub:**

| Task | RED log | Diagnostics |
|---|---|---|
| 1 | `target/116-verify/116-07-task1.RED.log` | **15 tests run, 5 passed, 10 failed**, `--no-fail-fast` |
| 2 | `target/116-verify/116-07-task2.RED.log` | **27 tests run, 16 passed, 11 failed**, `--no-fail-fast` |

Both suites were written to use only APIs that already existed, so they COMPILED against the unfixed
tree and the anchor fences could be observed FAILING rather than merely failing to build. The
recorded diagnostics:

```
panicked at tests/oauth_provider_discovery.rs:216:10:
called `Result::unwrap_err()` on an `Ok` value: GenericOidcProvider {
  id: "under-test", display_name: "Provider Under Test",
  issuer: "http://127.0.0.1:62230/tenant1", client_id: "client-id" }
```

```
panicked at src/server/auth/providers/cognito.rs:719:14:
called `Result::unwrap_err()` on an `Ok` value: OidcDiscovery {
  issuer: "https://honest.example",
  authorization_endpoint: "http://127.0.0.1:63471/authorize",
  token_endpoint: "http://127.0.0.1:63471/token", … }
```

The second is the specification's worked attack succeeding verbatim: a document fetched from
`127.0.0.1` claiming to be `https://honest.example`, accepted and returned to the caller. The
oversized-body rows produced the same `Ok` shape with a 1.2 MB body.

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01`
(`ea1d2d68`) through `116-06`: in Rust a test naming a non-existent item fails to *compile*, and
Task 2's RED tests live INSIDE `cognito.rs`, so a RED commit would leave a non-building tree that
breaks `git bisect` and contradicts CLAUDE.md's "ZERO TOLERANCE FOR DEFECTS". A verifier looking for
a `test(...)` → `feat(...)` pair will not find one; the evidence is the RED logs above and the two
negative controls below.

### Negative control — Task 1 (`target/116-verify/116-07-task1.NEGATIVE-CONTROL.log`)

Three deliberate breaks applied **at once**, `--no-fail-fast`, denominator asserted:
**15 tests run: 10 passed, 5 failed.**

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| the `Terminal` arm removed (the `if !ok { continue }` simplification) | `a_lying_document_aborts_the_generic_probe_…`, `an_oversized_body_aborts_the_generic_probe_…`, `a_cross_origin_discovery_redirect_is_not_followed_by_the_generic_provider` — all three fell through to the valid candidate 3 | both probe-ORDER rows, the retry-budget row and the non-JSON-fallback row all held: ordering and retry are separate detectors from terminality |
| the RFC 8414 §3.3 anchor comparison dropped | `a_generic_document_that_lies_about_its_issuer_…`, `only_the_issuer_field_decides_…`, and the lying-abort row | `a_document_whose_issuer_matches_the_url_it_came_from_is_accepted` held — it is the positive control and must NOT move |
| the body cap raised to `usize::MAX` | `an_oversized_body_aborts_the_generic_probe_…` | the three pure-derivation rows and the TTL-cache row all held |

### Negative control — Task 2 (`target/116-verify/116-07-task2.NEGATIVE-CONTROL.log`)

Three deliberate breaks applied at once: **28 tests run: 21 passed, 7 failed.**

| Deliberate break | Tests that FAILED | Siblings that still PASSED |
|---|---|---|
| the `Terminal` arm removed | `a_lying_cognito_document_aborts_…`, `an_oversized_cognito_body_aborts_…`, `a_cross_origin_cognito_discovery_redirect_is_not_followed` | `cognito_probes_the_spec_candidates_before_the_appended_form` and `a_cognito_candidate_one_success_never_requests_candidate_three` — probe order is unaffected by terminality |
| the anchor comparison dropped | `a_lying_cognito_discovery_document_is_rejected_naming_both_values`, `an_anchor_rejected_cognito_document_is_never_cached`, `a_trailing_slash_issuer_still_needs_a_byte_identical_document_issuer` | `a_trailing_slash_cognito_issuer_produces_no_doubled_slash` held — the URL-arithmetic row is NOT an anchor detector, which is exactly the separation Deviation 2 established |
| the TTL cache read no longer short-circuits | `the_cognito_ttl_cache_still_short_circuits_the_ordered_probe` **only** | `a_cognito_cache_miss_reprobes_the_ordered_candidates_from_the_top` and `a_cognito_five_xx_is_retried_to_the_budget_then_falls_back` both held — the cache-hit and cache-miss rows are independent detectors |

Source restored from a scratchpad copy after each; `shasum -a 256 -c` → **OK** for `cognito.rs`, the
three break sites re-verified absent by grep, and both suites re-ran fully green.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| plan suite (gated) | `-E 'binary(oauth_provider_discovery)'`, `--features full,oauth` | **15 run, 15 passed** |
| plan suite (**narrow-gate proof**) | `nextest list`, `--features full` only | **15** (non-zero) |
| `generic_oidc` inline | `-E 'binary(pmcp) and test(auth::providers::generic_oidc)'` | **29** (was 26) |
| `cognito` inline | `-E 'binary(pmcp) and test(auth::providers::cognito)'` | **26** (was 14) |
| combined + regression | `+ binary(v2_bounded_reads_tripwire) + binary(oauth_discovery_validation) + binary(oauth_dcr_integration) + binary(oauth_discovery_urls)` | **118 run, 118 passed** |
| **bounded-reads tripwire** | `-E 'binary(v2_bounded_reads_tripwire)'` | **13 run, 13 passed** — green standalone AND inside `make quality-gate` |
| needle count | `grep -c '<4 reqwest needles>' src/server/auth/providers/generic_oidc.rs` | **0** |
| needle count | same, `src/server/auth/providers/cognito.rs` | **0** |
| no hand-built discovery URL | `grep -n 'format!("{}/.well-known' <both files>` | only `jwks.json` (out of scope) and one comment |
| matrix drives the loop | `grep -c 'classify_discovery_failure'` | **8** (`generic_oidc`), **3** (`cognito`) |
| hardened client wired | `grep -c 'hardened_discovery_client'` | **4** (`generic_oidc`), **5** (`cognito`) |
| shared derivation | `grep -c 'discovery_url_candidates'` | **8** (`generic_oidc`), **3** (`cognito`) |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over all three files | **no output** |
| lint (**authoritative**, D-116-LINT) | `/usr/bin/make lint` | **✓ No lint issues** (after Deviation 4) |
| lint with `oauth` (D-116-LINT-OAUTH) | gate-equivalent, `--features "full,oauth"` | **29 errors, all in `src/client/oauth.rs`** = the anchor; **0** attributable |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** (twice) |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** attributable |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, **92** warnings (= 116-BASELINES anchor), **0** naming either provider |
| gate: `test-unit` | inside `make quality-gate` | **1880 passed; 0 failed** (1865 + 15) |
| gate: `test-doc` | inside `make quality-gate` | **445 passed; 0 failed; 79 ignored** |
| gate: `test-integration` | inside `make quality-gate` | ✓, including the tripwire binary |
| gate: `test-examples` | inside `make quality-gate` | all examples built ✓ |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 0** |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-SLASH` (new)** — the URL derivation normalises a trailing slash and the RFC 8414 §3.3
  anchor does not, on purpose. Operator-visible: a configured issuer that differs from the
  provider's declared issuer by a slash is now refused. Pinned by its own test; owed to `116-13` as
  **one CHANGELOG line**, not a code change.
- **`D-116-LINT` — reconfirmed, seventh and eighth measurements**, both in TEST code. See
  Deviation 4.
- **`D-116-LINT-OAUTH`** — measured again, unchanged at 29 pre-existing errors in
  `src/client/oauth.rs`, 0 attributable. Still open for `116-15`.
- **`D-116-FUZZGATE`** — reconfirmed inside this plan's own gate run: 21/21 nightly failures, 21/21
  swallowed, gate still 0. Still open for `116-15`.
- **`D-116-EX`** — closed by `116-08`; this plan adds no `examples/` binary and does not reopen it.
- **`D-116-KEYCHAIN`** — remains RESOLVED; second clean observation recorded above with the
  arithmetic closing exactly.
- **`D-116-TRIPWIRE`** — remains RESOLVED; 13/13 both standalone and inside the gate.
- **The duplicated probe glue** — the `Retry`/`Fallback`/`Terminal` loop and its refusal
  constructors now exist in three files. The security decisions are shared; the glue is not.
  Consolidating it into a private `src/server/auth/providers/discovery.rs` is available to a later
  plan, and was NOT done here because this plan's acceptance criteria require
  `classify_discovery_failure`, `discovery_url_candidates` and `hardened_discovery_client` to appear
  inside each provider file. Each file's module doc names the other two so a change to one is
  visibly owed to all three.

## Next Phase Readiness

| Consumer | What it can now rely on |
|---|---|
| `116-09` | every discovery call site in the crate validates the RFC 8414 §3.3 anchor, so the RFC 9207 `iss` comparison downstream is anchored on a value the served document did NOT choose for itself. That was `T-116-09`'s other half and it is now complete at all three sites |
| `116-12` | `read_json_within_cap` / `read_error_body_within_cap` in both provider files are the shape `src/client/oauth.rs:281-291`'s DCR read should become; the same `DEFAULT_AUTH_RESPONSE_BYTES` applies, so it is a change of mechanism only. Note the 29-error `D-116-LINT-OAUTH` baseline in that exact file — measure it BEFORE editing |
| `116-13` | owes **one CHANGELOG line** for `D-116-SLASH`, and must not rest a version-bump argument on `cargo semver-checks`' "no semver update required" verdict (sixth observation) |
| `116-15` | `make quality-gate` is **exit 0** at this HEAD — the first clean full-gate run in the phase since `116-03`. `D-116-KEYCHAIN` and `D-116-TRIPWIRE` are both confirmed resolved by a second independent measurement. `D-116-LINT`, `D-116-LINT-OAUTH` and `D-116-FUZZGATE` remain open |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-13` | the `D-116-SLASH` CHANGELOG line |
| every source-touching plan | run `make lint`, not clause (b) alone (`D-116-LINT`, now 8× measured); use an ABSOLUTE binary path for any command whose output you intend to count (`rtk` aggregates and hides `^error` lines); run `df -h /` before believing any gate failure |
| every plan running a negative control | restore from a scratchpad COPY, never `git checkout --` on uncommitted work — see *Process incident* |
| `116-15` | do not book `AUTH-03` on this plan's evidence alone; close or waive `D-116-LINT-OAUTH` and `D-116-FUZZGATE` |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: src/server/auth/providers/generic_oidc.rs                  1593 lines (was 1140)
FOUND: src/server/auth/providers/cognito.rs                       1607 lines (was  818)
FOUND: tests/oauth_provider_discovery.rs                            480 lines (min_lines 110 ✓)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md  (+2 entries)
```

Commits claimed, verified in `git log`:

```
FOUND: 6b1ba528  feat(116-07): ordered probe, RFC 8414 anchor and bounded reads in generic_oidc
FOUND: 3544318d  feat(116-07): the same ordered probe, anchor and bounded reads in cognito
```

`must_haves` verification:

```
✓ truths[1] both server-side providers resolve discovery through the same spec-ordered candidate
  list as the client — discovery_url_candidates is the only derivation in either file; probe ORDER
  proven by mock hit counts in BOTH providers, not by the result
✓ truths[2] both reject a document whose issuer does not match the issuer used to build the URL —
  issuer_matches_metadata inside each fetch helper, before the document escapes; each OBSERVED
  accepting the lying document pre-fix, with the Ok value quoted
✓ truths[3] neither allocates a whole peer-supplied response body beyond its cap — 9 reads
  rewritten, needle count 0 in both files, over-cap TERMINAL rows fenced with expect(0)
✓ truths[4] a trailing-slash issuer no longer produces a doubled slash in the Cognito discovery
  URL — a_trailing_slash_cognito_issuer_produces_no_doubled_slash reaches a mock whose path has no
  doubled slash, which the pre-fix provider could not do
✓ artifacts: src/server/auth/providers/generic_oidc.rs provides the ordered probe + 8414 3.3
  anchor + bounded reads, and contains "discovery_url_candidates" (8 references)
✓ artifacts: src/server/auth/providers/cognito.rs provides the same with the TTL cache preserved,
  and contains "discovery_url_candidates" (3 references)
✓ artifacts: tests/oauth_provider_discovery.rs 480 >= 110, with probe-order, lying-issuer and
  TTL-cache coverage (Cognito's wire-level rows inline — see Decisions Made)
✓ key_links: generic_oidc.rs -> shared/oauth_validation.rs via discovery_url_candidates
✓ key_links: cognito.rs -> shared/http_body_cap.rs via collect_reqwest_body_within_cap
```

Plan-level verification block:

```
✓ binary(oauth_provider_discovery) green with a non-zero count (15/15), under BOTH feature sets
✓ both lying-issuer fences recorded as OBSERVED failing before their fixes, with the Ok values quoted
✓ grep -c of the reqwest whole-body needles is 0 in BOTH provider files
✓ make quality-gate — exit 0; make lint exit 0; the gate-equivalent oauth clippy shows 0
  attributable errors against a 29-error pre-existing anchor
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations
✓ cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157 — 223 pass / 0 fail
✓ binary(v2_bounded_reads_tripwire) — 13 run, 13 passed, standalone AND inside the gate
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
✓ wasm32 build — exit 0, 92 warnings = the 116-BASELINES anchor
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
