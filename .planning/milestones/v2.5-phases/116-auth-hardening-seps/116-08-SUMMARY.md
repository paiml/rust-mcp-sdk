---
phase: 116-auth-hardening-seps
plan: 08
subsystem: auth
tags: [oauth, fuzzing, libfuzzer, always-requirements, rfc9207, sep-2351, sep-2352, sep-837, example]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 02
    provides: "validate_authorization_response, AuthorizationRequestRecord, IssPresence, MAX_CALLBACK_QUERY_BYTES and the crate-root re-exports the example imports"
  - phase: 116-auth-hardening-seps
    plan: 04
    provides: "discovery_url_candidates and derive_application_type — the two other pure entry points fuzzed here"
  - phase: 116-auth-hardening-seps
    plan: 05
    provides: "parse_credential_snapshot, CredentialSnapshot::to_bytes, MigrationReport — the credential format fuzzed here"
provides:
  - "fuzz/fuzz_targets/oauth_authorization_response.rs — no-panic + DERIVED Ok-invariant coverage over the callback validator and the discovery-candidate derivation"
  - "fuzz/fuzz_targets/oauth_credential_and_dcr.rs — the AUTH-02/AUTH-03 half: credential parse, migration accounting, save/load round trip, application_type derivation, DcrResponse accessor"
  - "A hand-rolled x-www-form-urlencoded decoder inside the fuzz target, so the fence shares neither the crate's RULE nor the crate's DECODER"
  - "Two committed SEED corpora with gitignore exceptions, each with a MEASURED necessity argument"
  - "examples/c11_oauth_iss_state_validation.rs — the phase's ALWAYS-EXAMPLE, runnable with NO feature flags"
  - "D-116-EX RESOLVED — the EXAMPLE row was owned by this plan all along"
  - "D-116-FUZZGATE — the measured proof that make test-fuzz runs zero iterations and reports success on a stable default toolchain"
affects: [116-15, 116-13, 116-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A fuzz fence that decodes its input with its OWN implementation rather than the crate's library, so a shared decoder defect is visible instead of cancelling out"
    - "Seed corpora whose necessity is MEASURED: break the implementation, then run the same binary from an empty corpus and from the seeds, and compare what each finds"
    - "Per-seed replay against a deliberately broken build as the negative-control partition — one crash per detector, with named surviving siblings, instead of one fuzz run that stops at the first failure"
    - "An example that EXECUTES the shipped security decision rather than narrating it, which is only possible because the tier is a pure function"
    - "Guarding an Ok-side fuzz assertion on a SOUNDNESS condition (valid UTF-8, no JSON escape) rather than weakening it, so the check stays sharp where it applies"

key-files:
  created:
    - fuzz/fuzz_targets/oauth_authorization_response.rs
    - fuzz/fuzz_targets/oauth_credential_and_dcr.rs
    - fuzz/corpus/oauth_authorization_response/README.md
    - fuzz/corpus/oauth_credential_and_dcr/README.md
    - examples/c11_oauth_iss_state_validation.rs
  modified:
    - fuzz/Cargo.toml
    - fuzz/.gitignore
    - fuzz/fuzz_targets/dcr_response_parser.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "The independent decoder is HAND-ROLLED, not url::form_urlencoded — the acceptance criteria forbid a new [dependencies] entry, and a hand-rolled decoder is STRICTLY stronger for T-116-29 because the crate decodes with that very library"
  - "The candidate list is NOT asserted distinct: an issuer whose own path is /.well-known/openid-configuration legitimately produces two identical candidates, and a distinctness assertion would have been a false-positive crash"
  - "Invariant 4's verbatim-substring check is guarded on 'no backslash in the input' — the plan's literal wording admits a false positive on any JSON escape"
  - "Both seed corpora are COMMITTED with gitignore exceptions, following the 115-09 fuzz_schema_draft_pin precedent, because 200000 runs from an empty corpus found 0 of 9 deliberate breaks"
  - "dcr_response_parser.rs was EXTENDED with application_type() as the plan instructs, AND invariant 4 also lives in the new target — different corpora, different input distributions, and the plan's must_haves name the accessor as the new target's artifact"
  - "The example is NOT gated on the oauth feature: running without it IS the D-05/D-06 demonstration"
  - "AUTH-01/AUTH-02/AUTH-03 are NOT booked complete — this plan discharges the ALWAYS policy, not the wiring"

patterns-established:
  - "A seed corpus is not documentation, it is the fence: measure what an empty corpus finds before claiming a campaign proves anything"
  - "Verify the binary path exists before a per-seed replay loop — a missing binary makes every seed 'pass' and reads exactly like a clean result"

requirements-completed: []

# Metrics
duration: 51min
completed: 2026-08-04
---

# Phase 116 Plan 08: ALWAYS FUZZ and EXAMPLE for the Pure OAuth Tier Summary

**All five pure entry points across AUTH-01, AUTH-02 and AUTH-03 now have no-panic fuzz coverage
whose `Ok`-side invariants are derived from the specifications and from the input bytes rather than
restated from the code — the callback target even decodes the query with its OWN
`x-www-form-urlencoded` implementation, so it shares neither the crate's rule nor the crate's
decoder. Nine deliberate breaks across three source files were each caught by a distinct assertion,
with named surviving siblings. And the phase's ALWAYS-EXAMPLE row, which `D-116-EX` recorded as
unowned, turns out to have been this plan's all along: `cargo run --example
c11_oauth_iss_state_validation` exits 0 with no feature flags and actually EXECUTES the accept and
both reject paths.**

## Performance

- **Duration:** ~51 min
- **Started:** 2026-08-04T16:03:08Z
- **Completed:** 2026-08-04T16:54Z
- **Tasks:** 3
- **Files:** 45 changed (**+991 / −1**), 0 removed — of which 34 are one-line seed corpus inputs

## Accomplishments

- **The fuzz fences do not restate the implementation, and one of them does not even share its
  library.** `validate_authorization_response` decodes the callback with
  `url::form_urlencoded::parse`. Reusing that function in the target would have made the fence blind
  to a defect in the decoder itself, and `url` is not a `fuzz/Cargo.toml` dependency (adding one is
  forbidden by `T-116-SC`). So the target carries a 40-line hand-rolled WHATWG decoder — split on
  `&`, skip empty sequences, split on the first `=`, `+` → space, `%HH` for ASCII hex only, lossy
  UTF-8 — written against the vendored `form_urlencoded` 1.2.2 and `percent-encoding` 2.3.2 sources
  so the two agree byte for byte. This is the stronger reading of `T-116-29`, not a workaround.

- **Nine deliberate breaks, nine distinct detectors, every partition attributable.** Four breaks
  applied at once to `oauth_validation.rs` for Task 1, and five at once across
  `credential_store.rs`, `oauth_validation.rs` and `provider.rs` for Task 2. Each was caught by a
  named assertion on a named seed while its siblings still passed — including the two siblings that
  had to pass: `seed_issuer_pathless` (a path-less issuer has no appended candidate to drop) and
  `seed_dcr_escaped_app_type` (whose backslash guard could have been a way of switching the check
  off, and is not).

- **The seed corpora are load-bearing, and that is MEASURED rather than argued.** With the same
  broken binaries, **200 000 runs from an EMPTY corpus found 0 of 4 and 0 of 5**. The seeds found
  4 of 4 and 5 of 5, in 34 single-input replays. Random bytes essentially never reproduce a fixed
  15-byte `state`, and never build a schema-1 document with an `entries` object and a per-entry
  `access_token`. Both corpora are therefore committed with `fuzz/.gitignore` exceptions, following
  the `115-09` `fuzz_schema_draft_pin` precedent, each with a README stating the measurement.

- **A false-positive assertion was caught by ANALYSIS before it ever fired.** The obvious invariant
  "the discovery candidates are all distinct" is wrong: an issuer whose own path is
  `/.well-known/openid-configuration` makes candidates 2 and 3 identical. It is not asserted, and
  `seed_issuer_wellknown_path` exists specifically to keep a later editor from adding it. Similarly,
  the plan's literal invariant 4 — "the returned `&str` is a substring of the input" — is false for
  any JSON escape (`"native"` decodes to `native`), so the check is guarded on the input
  containing no backslash. Both corrections are documented in place.

- **The example runs the security decision instead of describing it.** `c08_oauth_dcr.rs` can only
  narrate what DCR would do; this one validates four real callback queries and prints what happened,
  because the tier is a pure function. Identical stdout with and without `--features full,oauth`,
  both **exit 0**. That is what makes it a real `cargo run --example` under CLAUDE.md rather than
  one that passes by compiling.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Fuzz the authorization-response validator and the discovery candidates | `bf7cd35c` | test |
| 2 | Fuzz the credential document, the migration and the DCR metadata | `42b3b0ae` | test |
| 3 | Runnable example for `iss` and `state` validation | `8b41f7b0` | feat |

## Files Created/Modified

- **`fuzz/fuzz_targets/oauth_authorization_response.rs`** (**created**, **285** lines —
  `min_lines` 45 ✓). Two entry points, six `Ok`-side assertions: size cap, no repeated security
  parameter, `state` equals the record's, `iss` absent-or-equal with the `Required` row enforced, no
  `error` on a success, and the returned code is the query's own `code`. Plus the candidate list:
  length 2 or 3, every element an absolute http(s) URL with a host and no query or fragment, RFC
  8414's inserted form FIRST and the OIDC appended form LAST.
- **`fuzz/fuzz_targets/oauth_credential_and_dcr.rs`** (**created**, **238** lines — `min_lines`
  55 ✓). Four invariants: the credential parse with a schema-1 issuer rule and an entry-count
  accounting derived from an independent `serde_json::Value` parse; save/load round-trip key
  stability; `derive_application_type` order-independence, element-wise unanimity, the two wire
  literals and the always-refused empty vector; and the `DcrResponse` accessor's verbatim rule.
- **`examples/c11_oauth_iss_state_validation.rs`** (**created**, **213** lines — `min_lines` 70 ✓).
  Four labelled scenarios. NOT gated; the module doc explains why without naming the attribute, so
  an audit grepping for a gate over this file finds **nothing**.
- **`fuzz/Cargo.toml`** (+31/−0) — two `[[bin]]` stanzas with `test/doc/bench = false`, each
  commented with the phase, the requirement IDs and the "adds NO dependency" claim. **Zero**
  `[dependencies]` entries added.
- **`fuzz/.gitignore`** (+38/−0) — two seed-corpus exceptions, each carrying its measurement.
- **`fuzz/fuzz_targets/dcr_response_parser.rs`** (+27/−1) — EXTENDED with `application_type()`.
- **`fuzz/corpus/oauth_authorization_response/`** (14 seeds + README) and
  **`fuzz/corpus/oauth_credential_and_dcr/`** (20 seeds + README).
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (395 → **530** lines) —
  `D-116-EX` RESOLVED, one new entry `D-116-FUZZGATE`.

## Decisions Made

- **The independent decoder is hand-rolled, and that is the stronger choice.** `T-116-29` exists
  because Phase 115 measured that a fence restating the code's own rule cannot see a shared rule
  defect. The plan proposed `url::form_urlencoded::parse` as the independent decode — but that is
  the very function `parse_callback_parameters` calls, so a defect *in the decoder* would cancel
  out. It is also not a `fuzz` dependency, and Task 1's own acceptance criteria plus `T-116-SC`
  forbid adding one. Writing the decode inside the target satisfies both the letter (independent
  decode) and the spirit (the fence can disagree with the implementation).
- **`dcr_response_parser.rs` was extended AND invariant 4 lives in the new target.** The plan's
  action says to extend rather than duplicate; the plan's `must_haves` name "DcrResponse deserialize
  plus `application_type()`" as `oauth_credential_and_dcr.rs`'s own artifact, and its acceptance
  criteria require all four invariants "in the target". Both were done because they are not
  redundant: `dcr_response_parser` carries a DCR-shaped corpus and its own seeds, while the new
  target reaches the same accessor from credential-document-shaped bytes. Different input
  distributions over the same code.
- **No distinctness assertion on the candidate list**, for the reason above. Recorded here because
  it looks like an obvious omission and is not one.
- **Invariant 4 is guarded, not weakened.** The guard (`valid UTF-8` and `no backslash`) is a
  SOUNDNESS condition: outside it the assertion is simply false for a correct implementation.
  `seed_dcr_escaped_app_type` is kept precisely so a reader can see the guarded case still exercised
  and still passing.
- **`AUTH-01`, `AUTH-02` and `AUTH-03` are NOT booked complete**, for the sixth plan running. This
  plan discharges the house ALWAYS policy over the pure tier; `116-06`, `116-07`, `116-09`,
  `116-10`, `116-11`, `116-12` and `116-13` own the wiring. `requirements-completed: []`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical functionality] Both seed corpora, and their `.gitignore` exceptions**

- **Found during:** Task 1, immediately after the first clean 200 000-run campaign.
- **Issue:** the campaign was green, and meaningless. The `Ok`-side invariants — the entire
  `T-116-29` mitigation — only run when the validator ACCEPTS, which needs a callback query whose
  `state` reproduces a fixed 15-byte token exactly. `fuzz/.gitignore:7` ignores `corpus/*`, so a
  target shipped without seeds would have been a bare no-panic check in every environment except
  the one where it was written.
- **Measured, not assumed:** four deliberate breaks were applied and the SAME binary run two ways.
  Empty corpus, 200 000 runs: **0 defects**. Seeds: **4 of 4**. Task 2 repeated it with five breaks:
  **0 of 5** vs **5 of 5**.
- **Fix:** 14 + 20 hand-written seeds committed with two `fuzz/.gitignore` exceptions modelled on
  `115-09`'s `fuzz_schema_draft_pin` block, plus a README per corpus recording the measurement and
  what each seed detects. `fuzz/.gitignore` was not in `files_modified`.
- **Committed in:** `bf7cd35c`, `42b3b0ae`.

**2. [Rule 1 — Bug] The plan's invariant 4 admits a false positive on any JSON escape**

- **Found during:** Task 2, writing the target.
- **Issue:** "assert the returned `&str` is a substring of the lossy-UTF-8 rendering of the input
  bytes" is false for `{"application_type":"native"}` — the accessor correctly returns
  `native`, and `native` never appears in the input. As written, the invariant would have crashed on
  a CORRECT implementation, and the crash would have looked like a real finding.
- **Fix:** the check is guarded on the input being valid UTF-8 and containing no backslash, with the
  reason written in place and marked as a correction to the plan. `seed_dcr_escaped_app_type` pins
  the guarded case. The same guard was applied to the `dcr_response_parser` extension.
- **Committed in:** `42b3b0ae`.

**3. [Rule 1 — Bug] A per-seed replay loop reported "all pass" against a binary that did not exist**

- **Found during:** Task 1's negative control.
- **Issue:** the first replay loop pointed at `../target/aarch64-apple-darwin/release/…`, but
  `cargo fuzz` writes to `fuzz/target/…`. Every one of the 14 seeds "passed" — against four
  deliberate breaks. The output is indistinguishable from a genuinely clean partition.
- **Fix:** the loop now asserts `test -x "$BIN"` and aborts otherwise, and the binary path is
  resolved from `find`. Re-run: 6 crashes, 8 passes, cleanly partitioned.
- **Why it is recorded:** this is `D-116-FAILFAST` and `116-01`'s selector trap in a third shape —
  a harness that produces a plausible summary from a run that did nothing. It is the reason the
  negative control exists at all.

**4. [Rule 2 — Missing critical functionality] The module-doc heading was reworded so the gate grep is genuinely empty**

Task 3's acceptance criterion is `grep -n 'cfg(feature = "oauth")' … returns nothing`. The first
draft explained the absence of the gate by naming the attribute, which produced a PROSE hit — the
same false-positive class `116-02` recorded for `oauth_validation.rs`. The heading now avoids the
literal, and says so, so the criterion is satisfied literally as well as in spirit.

**Total deviations:** 4 (2 × Rule 1, 2 × Rule 2). No Rule 4 situation arose. **Zero dependencies
added** — `git diff b2bf9157..HEAD -- fuzz/Cargo.toml` adds **0** `[dependencies]` entries, and root
`Cargo.toml` is untouched.

### Non-deviation worth recording

The plan's Task 1 says to check whether `auth_flows.rs` or `dcr_response_parser.rs` already houses
this surface. `auth_flows.rs` imports **no `pmcp` symbol at all** — it fuzzes hand-rolled
`Arbitrary` structs and `serde_json` — so it is not a home for the validator; a new target was
minted. `dcr_response_parser.rs` DOES already fuzz `DcrResponse` deserialization, so it was extended
rather than replaced.

## Issues Encountered

- **`cargo fuzz` does not run on this machine's default toolchain, and `make test-fuzz` hides it.**
  `cargo fuzz build` fails with ``the option `Z` is only accepted on the nightly compiler``; every
  build and campaign here used `cargo +nightly fuzz`. The module docs still give the plain
  `cargo fuzz run <name>` form, per the plan and per `pkce_helper.rs`'s convention, which is correct
  for a machine whose default toolchain is nightly. Measured inside this plan's own gate run:
  `make test-fuzz` invoked **21 of 21** targets, **all 21 failed with that error**, all 21 printed
  `Fuzz target … completed`, and `make quality-gate` exited **0**. Logged as `D-116-FUZZGATE`.
- **`target/` was deleted mid-plan by something outside this agent** — free space jumped from 48 GiB
  to 178 GiB between two commands, `target/116-verify/` vanished, and `fuzz/target/` went with it.
  Git state was unaffected (both fuzz commits present, seeds tracked). Every campaign and both
  example runs were **re-executed from scratch afterwards**, and the logs cited below are from those
  re-runs, not from the lost set. Recorded because a reader comparing timestamps will otherwise find
  the evidence younger than the commits.
- **`ls -A … | wc -l` reported `1 files` for an empty directory** under this environment's command
  proxy; `find … -type f | wc -l` reported `0`, which is correct. The recorded rtk output-corruption
  hazard, hit again — every artifact-emptiness claim below is measured with `find`.
- **Disk was never a factor this time.** `df -h /` before the campaigns: **49 GiB free at 20%**;
  after the full gate: **~179 GiB**. No `D-116-DISK` symptom appeared, and `make quality-gate`'s unit
  stage reported `1865 passed; 0 failed` — exactly `116-16`'s figure, as expected for a plan that
  adds no `src/` code.

## Threat Flags

None. This plan adds no network endpoint, no socket, no file access, no schema change and no
`src/` code at all — it adds two fuzz binaries, 34 corpus files and one example.

| Threat | Disposition | Discharged by |
|---|---|---|
| T-116-27 (panic in `validate_authorization_response`) | mitigate | 200 000-run campaign, `fuzz/artifacts/oauth_authorization_response/` **empty** (`find`-counted) |
| T-116-28 (panic in `discovery_url_candidates`) | mitigate | same campaign; the same bytes double as a hostile issuer |
| T-116-29 (a rule defect the crate and its mirrors SHARE) | mitigate | the target decodes `state`/`iss` with its OWN decoder and asserts Ok-implies-match itself; OBSERVED failing under a case-folding break, an absent-state break and a first-wins duplicate break, while three accept seeds held |
| T-116-27a (panic in `parse_credential_snapshot`) | mitigate | 200 000-run campaign over arbitrary bytes, artifacts **empty**; plus the round-trip invariant, OBSERVED failing under a writer emitting the legacy version |
| T-116-27b (a migration attributing an entry to an issuer it never recorded) | mitigate | every schema-1 key asserted to carry a non-empty issuer, and `migrated + dropped` reconciled against an INDEPENDENT `serde_json::Value` entry count; both OBSERVED failing under their own breaks |
| T-116-27c (panic or stringification in the `application_type` surfaces) | mitigate | invariants 3 and 4; the mixed-vector break was caught by the element-wise unanimity check and the stringify break by the verbatim check, while `seed_dcr_native`, `seed_dcr_web` and the ESCAPED seed all held |
| T-116-SC (cargo installs) | mitigate | **0** `[dependencies]` entries added to `fuzz/Cargo.toml`; root `Cargo.toml` byte-identical to `b2bf9157` |

## Known Stubs

None. Both fuzz targets and the example are fully implemented and were each observed distinguishing
a correct implementation from a broken one. No placeholder, no empty collection, no
"not available" string.

## ALWAYS Requirements — the phase's audit point

`D-116-EX` is **RESOLVED** in `deferred-items.md`, with the correction that the EXAMPLE row was
owned by this plan all along (`examples/c11_oauth_iss_state_validation.rs` is `116-08`'s fourth
`files_modified` entry; the original grep searched plan BODIES and the hit is in the frontmatter).

| ALWAYS requirement | Owner | Evidence |
|---|---|---|
| FUZZ | `116-08` | two new targets + one extension, 200 000 runs each, all artifacts dirs empty |
| PROPERTY | `116-02`, `116-04`, `116-05` | `make test-property` ✓ inside this plan's gate run |
| UNIT | `116-02` onward | `1865 passed; 0 failed` |
| EXAMPLE | **`116-08`** | `cargo run --example c11_oauth_iss_state_validation` **exit 0**, no features |

**The one remaining gap is named and located, not left implicit:** the bounded-read CAP BOUNDARY
cannot be fuzzed purely, because it needs a `reqwest::Response`. It is covered by the
exactly-at-cap / one-under / one-over mockito triple in **`116-06` Task 1**. Recorded per the plan's
Task 3 instruction so the ALWAYS audit has one place to look.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 0** |
| fmt | (within the gate) | ✓ Code formatting OK |
| lint (**authoritative**, D-116-LINT) | (within the gate) | ✓ No lint issues |
| build | (within the gate) | ✓ Build successful |
| unit | (within the gate) | **1865 passed; 0 failed** (= 116-16's figure) |
| doctests | (within the gate) | **445 passed; 0 failed**, 79 ignored |
| property (ALWAYS) | (within the gate) | ✓ Property tests passed |
| examples (ALWAYS) | (within the gate) | ✓ Example `c11_oauth_iss_state_validation` built successfully (×2) |
| D-15 tripwire | `cargo nextest run --features full --no-fail-fast -E 'binary(v2_bounded_reads_tripwire)'` | **13 run, 13 passed** |
| fuzz build (target 1) | `cargo +nightly fuzz build oauth_authorization_response` | exit **0** |
| fuzz campaign (target 1) | `cargo +nightly fuzz run … -- -runs=200000 -max_total_time=180` | **Done 200000 runs**, artifacts **0 files** |
| fuzz build (target 2) | `cargo +nightly fuzz build oauth_credential_and_dcr` | exit **0** |
| fuzz campaign (target 2) | same flags | **Done 200000 runs**, artifacts **0 files** |
| fuzz campaign (extended) | `cargo +nightly fuzz run dcr_response_parser …` | **Done 200000 runs**, artifacts **0 files** |
| registration | `cargo fuzz list \| grep oauth` | `oauth_authorization_response` (14), `oauth_credential_and_dcr` (15) of 21 |
| example (UNGATED proof) | `cargo run --example c11_oauth_iss_state_validation` | **exit 0** |
| example (gated) | `… --features full,oauth` | **exit 0**, stdout **identical** to the ungated run |
| example (all-features build) | `cargo build --example c11… --all-features` | exit **0** |
| dependency fence | `git diff b2bf9157..HEAD -- fuzz/Cargo.toml`, `[dependencies]` entries added | **0** |
| gate grep | `grep -n 'cfg(feature = "oauth")' examples/c11_…` | **no output** |
| seed replay (restored source) | 34 seeds, one at a time, both targets | **34/34 pass** |

Campaign logs: `target/116-verify/116-08-fuzz-authresponse-FINAL.log`,
`…-fuzz-credential-dcr-FINAL.log`, `…-fuzz-dcr-response-parser.log`. Negative-control logs:
`…-fuzz-authresponse-BROKEN-unseeded.log`, `…-fuzz-credential-dcr-BROKEN-unseeded.log`. Gate log:
`…-116-08-quality-gate.log`. Example logs: `target/116-verify/c11-ungated.log`, `c11-oauth.log`.

## Negative Controls

### Task 1 — four breaks in `src/shared/oauth_validation.rs`, applied AT ONCE

Per-seed replay against the broken build: **6 crashed, 8 passed**.

| Deliberate break | Seed(s) that CRASHED | Siblings that still PASSED |
|---|---|---|
| `iss` comparison case-folds | `seed_reject_iss_case` | `seed_accept_iss_plain`, `seed_accept_iss_encoded`, `seed_accept_iss_lowerhex` — three accepting `iss` shapes, so the case detector is its own |
| an ABSENT `state` treated as a match | `seed_reject_no_state`, `seed_reject_no_state_with_iss` | `seed_accept_row4`, `seed_reject_as_error` |
| a repeated security parameter resolved first-wins | `seed_reject_dup_state` | every accept seed, and `seed_accept_plus_and_bad_pct` (which carries an empty pair and an invalid `%zz`) |
| the OIDC appended discovery form dropped | `seed_issuer_path`, `seed_issuer_loopback` | `seed_issuer_pathless` — CORRECTLY unaffected, a path-less issuer has no appended candidate to drop — and `seed_issuer_wellknown_path` |

### Task 2 — five breaks across three files, applied AT ONCE

Per-seed replay: **9 crashed, 11 passed**.

| Deliberate break | Seed(s) that CRASHED | Siblings that still PASSED |
|---|---|---|
| an empty recorded issuer no longer filtered (`credential_store.rs`) | `seed_schema1_dropped_empty_issuer` | `seed_schema1_no_entries_key`, `seed_unsupported_version` |
| an unkeyable entry skipped without being REPORTED (`credential_store.rs`) | `seed_schema1_dropped_no_issuer`, `seed_schema1_mixed_migrate_and_drop` | `seed_schema1_no_entries_key` — zero entries account for themselves |
| the writer emits the LEGACY schema version (`credential_store.rs`) | `seed_schema2_two_servers`, `seed_schema2_empty_issuer_key`, `seed_schema1_migrates`, `seed_schema1_two_servers_one_issuer`, `seed_dcr_and_schema1` | `seed_schema2_empty` — an empty key set round-trips trivially, which is what makes the other five attributable |
| a mixed `redirect_uris` vector picks `Native` (`oauth_validation.rs`) | `seed_uris_mixed` | `seed_uris_native_only`, `seed_uris_web_only`, `seed_uris_cleartext_remote`, `seed_uris_localhost_https` |
| `application_type()` stringifies a non-string (`provider.rs`) | `seed_dcr_nonstring_app_type` | `seed_dcr_native`, `seed_dcr_web`, **and `seed_dcr_escaped_app_type`** — proving the backslash guard is a soundness condition, not an off switch |

### Seed necessity — the second half of both controls

| Target | Corpus | Runs | Defects found |
|---|---|---|---|
| `oauth_authorization_response` | empty | 200 000 | **0 of 4** |
| `oauth_authorization_response` | seeds | 14 replays | **4 of 4** |
| `oauth_credential_and_dcr` | empty | 200 000 | **0 of 5** |
| `oauth_credential_and_dcr` | seeds | 20 replays | **5 of 5** |

All three sources restored byte-for-byte afterwards: `shasum -a 256 -c` → **OK** for
`src/shared/oauth_validation.rs`, `src/shared/credential_store.rs` and
`src/server/auth/provider.rs`; `git status --short src/` clean; all 34 seeds replay green against
the restored build.

## User Setup Required

None for the shipped artifacts. **For a developer who wants to RUN the fuzz targets:** the default
toolchain must be nightly, or the command must be `cargo +nightly fuzz …`. `fuzz/README.md` already
states nightly as a requirement. This plan installed **zero** packages, so no package-legitimacy
checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-FUZZGATE` (new)** — `make test-fuzz` invoked 21 targets, all 21 died on
  ``the option `Z` is only accepted on the nightly compiler``, all 21 printed "completed", and the
  gate exited 0. The ALWAYS-FUZZ stage cannot fail on a stable default toolchain. Three candidate
  resolutions offered; the `Makefile` is not this plan's file. Proposed owner: `116-15`.
- **`D-116-EX` — RESOLVED here**, with the correction that this plan owned it.
- **`D-116-KEYCHAIN`** — no symptom in this plan's gate run (`1865 passed; 0 failed`), consistent
  with `116-06`'s clean-volume measurement.

## Next Phase Readiness

**Nothing is blocked on this plan** — it adds no public API and no `src/` code. What it gives the
remaining plans is evidence and one carried obligation:

| Consumer | What it can now rely on |
|---|---|
| `116-15` | the ALWAYS table above, with FUZZ and EXAMPLE both **done** and the one remaining gap located in `116-06`; may cite `make quality-gate` **exit 0** measured here |
| `116-10` | `derive_application_type` and `DcrResponse::application_type()` are fuzzed, including the mixed-vector refusal it is about to depend on |
| `116-13` | `parse_credential_snapshot` and the schema 1 → 2 migration are fuzzed, including the drop-and-REPORT accounting whose `DroppedEntry` list `116-13` must surface |
| `116-16` and any later `src/shared/` plan | if you add an accumulation site, the 13-test tripwire still passes here — re-run it |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-15` | do NOT close the ALWAYS-FUZZ row on `make quality-gate`'s exit code (`D-116-FUZZGATE`) |
| any plan editing `src/shared/oauth_validation.rs`, `credential_store.rs` or `provider.rs` | 34 committed seeds now fence those files; run `cargo +nightly fuzz run <target>` after changing them |
| any plan adding a discovery candidate | do not assert candidate distinctness — `seed_issuer_wellknown_path` documents why |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: fuzz/fuzz_targets/oauth_authorization_response.rs            (285 lines, min_lines 45 ✓)
FOUND: fuzz/fuzz_targets/oauth_credential_and_dcr.rs                (238 lines, min_lines 55 ✓)
FOUND: examples/c11_oauth_iss_state_validation.rs                   (213 lines, min_lines 70 ✓)
FOUND: fuzz/Cargo.toml                                              (228 lines)
FOUND: fuzz/fuzz_targets/dcr_response_parser.rs                     (44 lines, was 17)
FOUND: fuzz/.gitignore                                              (60 lines)
FOUND: fuzz/corpus/oauth_authorization_response/README.md           (60 lines) + 14 tracked seeds
FOUND: fuzz/corpus/oauth_credential_and_dcr/README.md               (63 lines) + 20 tracked seeds
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md   (530 lines, was 395)
```

Commits claimed, verified in `git log`:

```
FOUND: bf7cd35c  test(116-08): fuzz the authorization-response validator and the discovery candidates
FOUND: 42b3b0ae  test(116-08): fuzz the credential document, the migration and the DCR metadata
FOUND: 8b41f7b0  feat(116-08): runnable example for iss and state validation, no network needed
```

`must_haves` verification:

```
✓ truths[1] arbitrary callback query bytes never panic the validator — 200000 runs, artifacts empty
✓ truths[2] arbitrary issuer strings never panic the candidate derivation — same campaign
✓ truths[3] arbitrary credential-file bytes never panic the parser or the 1->2 migration —
  200000 runs, artifacts empty
✓ truths[4] arbitrary DCR bytes and arbitrary redirect_uris never panic the AUTH-02 surfaces —
  same campaign, plus the extended dcr_response_parser campaign
✓ truths[5] one command, no network, no browser, both accept and reject execute —
  cargo run --example c11_oauth_iss_state_validation, exit 0, four scenarios in stdout
✓ artifacts: oauth_authorization_response.rs 285 >= 45
✓ artifacts: oauth_credential_and_dcr.rs 238 >= 55
✓ artifacts: c11_oauth_iss_state_validation.rs 213 >= 70
✓ key_links: fuzz/Cargo.toml -> oauth_authorization_response via a [[bin]] stanza adding NO
  dependency (2 references; 0 [dependencies] entries added)
```

Plan-level verification block:

```
✓ cargo fuzz build succeeds for BOTH targets; >=200k-run campaigns leave fuzz/artifacts/ EMPTY
  for each (find-counted: 0 files, for all three targets)
✓ cargo run --example c11_oauth_iss_state_validation exits 0 both with and without
  --features full,oauth, with identical stdout
✓ make test-examples passes; the example builds successfully twice inside the gate
✓ make quality-gate exits 0 — noting explicitly that it covers NOTHING under fuzz/ (workspace
  exclude, D-115-AB), and additionally that its test-fuzz stage ran ZERO iterations (D-116-FUZZGATE)
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
