---
phase: 116-auth-hardening-seps
plan: 11
subsystem: auth
tags: [oauth, sep-2352, credential-storage, issuer-keying, d-116-r1, d-17, d-18, rfc-9728, semver, mockito, tracing]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 05
    provides: "CredentialKey / StoredCredentials / CredentialStore / InMemoryCredentialStore / normalize_server_key — the three-part key and the platform seam this plan wires OAuthHelper onto"
  - phase: 116-auth-hardening-seps
    plan: 16
    provides: "FileCredentialStore + default_credential_path — the default on-disk store, with save_with_issuer counter-proven as ONE atomic write"
  - phase: 116-auth-hardening-seps
    plan: 10
    provides: "DcrOutcome.registered_application_type (the value this plan takes the last hop to the store), the 21-error full,oauth clippy anchor, and the restore-from-a-scratchpad-COPY discipline"
  - phase: 116-auth-hardening-seps
    plan: 02
    provides: "Error::reauth_required / is_reauth_required / reauth_issuer — the stable programmatic identity D-18's pre-registered refusal rides"
provides:
  - "OAuthHelper::with_credential_store / with_account_scope — the platform seam, as inherent builders backed by PRIVATE fields, so OAuthConfig gains no field and no semver event occurs"
  - "Credentials addressed by (issuer, account, server), carrying the effective client_id, the GRANTED scopes and the registered application_type"
  - "One persistence call — save_with_issuer — so the store cannot name one issuer while holding another's credentials"
  - "D-17: the flat issuer-less ~/.pmcp/oauth-tokens.json is never opened for reading, is warned about ONCE per helper, and is left on disk"
  - "D-18: announce_authorization_server_change — warn-and-proceed for DCR provenance, Error::reauth_required for pre-registered provenance, with the RFC 9728 gap written into its own rustdoc"
  - "ResolvedClientIdentity — the private carrier that gets 116-10's registered application_type to the store without a field on AuthorizationResult"
  - "unix_now_secs — one clock for the whole module, replacing a duration_since(UNIX_EPOCH).unwrap() that was a panic in library code"
  - "The measured finding that a plan <action> can name a config field whose in-repo callers make it unsatisfiable (D-116-PLANCONFLICT)"
  - "A re-measured D-116-LINT-OAUTH anchor: 17, down from 21, with the test-side twin at 0 of 24 and the gate population still 1880"
affects: [116-12, 116-13, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Lazy resolution behind &self via OnceLock, so a constructor stays I/O-free while a default can still be resolved on first use"
    - "A cache that cannot be read is a MISS with a warning, never a fatal error: credential storage must not be able to brick authentication"
    - "Detection that is diagnostic warns on every store failure and returns exactly ONE kind of Err — the specified refusal — so its error contract is one sentence"
    - "When a plan's <action> names a configuration field, measure what the in-repo callers actually pass to it before honouring the instruction literally"
    - "A scenario that is unreachable through the live flow gets SEEDED, and the reason is written in the test file's module doc rather than left for a reader to infer"

key-files:
  created:
    - tests/oauth_store_wiring.rs
  modified:
    - src/client/oauth.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "config.cache_file names the legacy file's DIRECTORY, not the credential store itself — both in-repo callers pass default_cache_path(), so honouring it literally would read AND overwrite the very file D-17 says to leave alone"
  - "cache_file: None with no injected store means NO persistence, which is what every previous cache read and write in this module was guarded by and what --no-cache means"
  - "Discovery now runs BEFORE the cache read, because credentials are addressed by the issuer that issued them and the issuer comes from discovery"
  - "authorize_with_details deliberately does NOT consult the store; get_access_token is the one that reads the cache"
  - "announce_authorization_server_change returns exactly one kind of Err — the pre-registered refusal; every store failure warns and proceeds"
  - "The D-116-R1 collision test SEEDS the second server because the case is not constructible through the live flow while RFC 9728 is deferred"
  - "AUTH-03 is NOT booked complete — 116-12, 116-13, 116-14 and 116-15 still claim it"

patterns-established:
  - "A negative control must break the rule the test claims to pin: 'the legacy file is never read' survived pointing the store AT the legacy file, because that row injects a store and never exercises default resolution"
  - "Rows that PASSED under PREFIX-RED are named as positive controls, not counted as coverage — four of Task 2's six did"

requirements-completed: []

# Metrics
duration: 265min
completed: 2026-08-04
---

# Phase 116 Plan 11: SEP-2352 Credential Storage Wired Into the OAuth Flow Summary

**`OAuthHelper` now addresses credentials by `(issuer, account, server)`, and both collision classes
miss with no enforcement branch anywhere — a different authorization server, and a different MCP
server sharing that authorization server and account. The record carries the DCR-issued `client_id`,
the GRANTED scopes and the `application_type` 116-10 registered, so SEP-2352's "MUST re-register
with the new authorization server" is automatic rather than enforced. The flat, issuer-less
`~/.pmcp/oauth-tokens.json` is never opened for reading, is announced once and is left on disk. And
an authorization-server substitution — previously safe AND invisible — now names both issuers and
the MCP server, and is fatal exactly where the specification asks and nowhere else.**

**The finding worth the plan is a plan defect, not a code defect.** The `<action>` says to honour
`config.cache_file` as the store path. The `<behavior>` says the legacy file is never read and is
left in place. **Both in-repo callers pass exactly that file as `cache_file`**
(`crates/mcp-tester/src/main.rs:594`, `cargo-pmcp/src/commands/auth.rs:76`), so the two instructions
cannot both hold — honouring the first parses a document `parse_credential_snapshot` rejects and
then overwrites it. A second conflict in the same sentence: `cargo pmcp auth login --no-cache` sets
`cache_file` to `None`, so resolving a default store there silently defeats the flag.

**`make quality-gate` exits 0 having run ZERO of this plan's 24 tests**, measured rather than
inferred: `--features full` reports `Starting 0 tests across 1 binary`, the inline module reports
`1880 filtered out`, and the gate's `test-unit` population is **1880** — byte-identical to 116-09's
and 116-10's, for the third consecutive plan that added inline tests. **The `full,oauth` clippy
anchor moved 21 → 17 with ZERO new errors**, compared as a multiset of `(error message, offending
source-line text)`.

## Performance

- **Duration:** ~265 min
- **Completed:** 2026-08-04
- **Tasks:** 2
- **Files:** 3 (1 created, 2 modified), **+2083 / −227** across the two task commits

## Accomplishments

- **The D-116-R1 collision is proved on CONTENTS, and the reason it has to be seeded is written
  down rather than glossed.** `two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1`
  builds two entries that differ in NOTHING but the server component — same issuer, same empty
  account — drives server A's flow, and asserts (a) A's load never returns B's canary, (b) both
  records survive with their own `access_token`, `client_id` and `granted_scopes`, and (c) deleting
  A leaves B. Under the two-part key this phase replaced, step (a) is a HIT and step (b) shows one
  entry. **It is seeded because the case is not constructible through the live flow:** pmcp derives
  the authorization server from the MCP base URL, and 116-07's RFC 8414 §3.3 anchor then forces two
  origins to two issuers, while two paths on one origin normalize to one server key. That is
  `D-116-PRM`, and it is in the test file's own module doc.

- **The negative control caught a test that was not a detector — again, and for the same reason
  116-10 recorded.** With the store pointed AT the legacy flat file (break B),
  `the_legacy_issuer_less_token_cache_is_never_read_and_is_left_in_place` **still PASSED**, because
  that row injects a store and therefore never exercises default resolution at all.
  `the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it` was added as the real
  detector: it uses no injected store, asserts the legacy file is byte-identical afterwards, asserts
  `oauth-cache.json` appeared beside it carrying the new token and not the canary, and asserts a
  second helper reads that document back.

- **Task 2's RED run was honest about what it did NOT prove.** 18 run, 16 passed, 2 failed — only
  the two CHANGE rows. The other four Task 2 rows passed pre-fix, and they are named as positive
  controls rather than counted: "no warning" and "no error" were already accidentally true when
  nothing detected anything, and A's issuer record already advanced through Task 1's
  `save_with_issuer`. Two targeted controls supply the attribution instead, and the second one is
  the D-18 argument in miniature: with every change made fatal, **the pre-registered row still
  PASSED** (it expects an error and got one) and only the two DCR-proceed rows caught it.

- **The warn path is asserted, not assumed.** A `tracing::Subscriber` in the suite captures WARN
  messages, so `an_issuer_change_with_dcr_credentials_warns_naming_both_issuers_and_proceeds`
  asserts **exactly one** substitution warning containing the OLD issuer, the NEW issuer and the
  normalized MCP server key. Without it the row would have been satisfied by a version that never
  warned at all.

- **The pre-registered refusal is proved to happen before anything interactive**, three ways at
  once: `err.is_reauth_required()` and `err.reauth_issuer()` (the programmatic identity, not a
  substring), a browser-open count of **0**, a mockito `.expect(0)` on `/token`, and — the sharpest
  one — `TcpListener::bind` on the callback port succeeding afterwards, which proves the loopback
  listener was never bound. Both public entry points are checked, so the refusal cannot be reached
  by picking the other one.

- **`make quality-gate` exits 0** at 107 GiB free: `fmt-check` ✓, `lint` ✓, `build` ✓, `test-unit`
  **1880 passed / 0 failed**, `test-doc` **445 passed / 0 failed / 79 ignored**, `test-integration`
  ✓, `test-property` ✓, `audit` ✓, `unused-deps` ✓, `check-todos` ✓, `check-unwraps` ✓,
  `pmcp-package` ✓, all 88 examples ✓. `cargo semver-checks --baseline-rev b2bf9157`: **223 checks,
  223 pass, 0 fail** — the ninth consecutive plan in this phase to see "no semver update required"
  despite two new public methods. **Zero packages added**; `git diff --exit-code b2bf9157..HEAD --
  Cargo.toml` exit **0**.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Route credential persistence through the issuer-keyed store and discard the legacy cache | `4d27db50` | feat |
| 2 | Announce an authorization-server change, fatal by credential provenance | `3b2a61e1` | feat |

## Files Created/Modified

- **`src/client/oauth.rs`** (**modified**, 2680 → **3272** lines, +1046/−227). New public items:
  `OAuthHelper::with_credential_store`, `OAuthHelper::with_account_scope` — **two methods, no new
  type, no new field on any public struct**. New private module-level items:
  `CREDENTIAL_STORE_FILE_NAME`, `unix_now_secs`, `ResolvedClientIdentity`. New private methods:
  `credential_store`, `server_key`, `credential_key`, `announce_authorization_server_change`,
  `record_issuer_best_effort`, `discard_legacy_token_cache`, `load_stored_credentials`,
  `persist_credentials`, `effective_issuer`, `token_from_store`, `authorize_with_fallback`.
  **Removed:** `struct TokenCache`, `load_cached_token`, `cache_token`, `cache_token_from_response`,
  `authorization_code_flow`. Renamed: `resolve_client_id_for_flow` → `resolve_client_identity_for_flow`.
  One new `#[cfg(test)]` module with **6** tests.
- **`tests/oauth_store_wiring.rs`** (**created**, **1264** lines — `min_lines` 170 ✓). **18** tests
  in five documented groups (A: the three-part key; B: the two collision classes; C: D-17, the
  legacy cache; D: what the record carries for 116-12; E: D-18, the substitution), plus a
  `WarnCapture` `tracing::Subscriber`, a counting callback-driving `BrowserLauncher` and a
  hand-written three-method `MinimalStore`.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (786 → **918**) — two new
  entries (`D-116-PRM`, `D-116-PLANCONFLICT`) and a re-measured `D-116-LINT-OAUTH` section.

## Decisions Made

- **`config.cache_file` names the legacy file's DIRECTORY, not the store.** The store is
  `<that directory>/oauth-cache.json` — `default_credential_path()`'s file name, beside the legacy
  one, pinned against drift by `the_credential_store_file_name_matches_default_credential_path`. So
  `cargo-pmcp` and `mcp-tester`, which both pass `~/.pmcp/oauth-tokens.json`, land on exactly
  `~/.pmcp/oauth-cache.json`, which is where 116-16 and 116-13 expect it — and the legacy file is
  neither parsed nor overwritten. A caller who wants a specific store path uses
  `with_credential_store(Arc::new(FileCredentialStore::new(path)))`, which is strictly better
  because it also admits a non-file store.
- **`cache_file: None` with no injected store means NO persistence.** Every previous cache read and
  write in this module was guarded by `if let Some(ref cache_file) = self.config.cache_file`, and
  `--no-cache` sets the field to `None` for exactly that reason. A pleasant side effect, measured:
  116-09's and 116-10's flow-driving suites do not set `cache_file`, so they do not write real
  credential documents into the developer's `~/.pmcp/` under an `O_EXCL` lock shared by ~10 parallel
  nextest processes.
- **Discovery now precedes the cache read, and it has to.** Credentials are addressed by the
  authorization server that ISSUED them, so the issuer must be known before the store can be asked
  anything. What a cache hit still avoids is the part that costs a human: no browser, no
  authorization request. This is stated in `get_access_token`'s rustdoc.
- **`authorize_with_details` deliberately does not consult the store.** It is the "log me in" entry
  point and `cargo pmcp auth login` means a fresh authorization; `get_access_token` is the one that
  reads the cache. Both persist, through one shared `authorize_with_fallback`, so a credential that
  reaches one caller and one that reaches the other cannot be stored differently.
- **A store that cannot be read is a cache MISS with a warning, never a fatal error.** A corrupt
  document, a stale lock or a path holding the legacy flat format would otherwise brick
  authentication entirely; degrading costs exactly one interactive login. The warning names the
  reason the store gave and never any credential content.
- **`announce_authorization_server_change` returns exactly ONE kind of `Err`** — the pre-registered
  refusal. Every store failure warns and proceeds, because the resulting behaviour is precisely
  today's (no detection), which is also what `CredentialStore::last_issuer`'s `Ok(None)` default
  promises an implementor who declines the tracking. That makes the function's error contract a
  single sentence, and it is asserted by the minimal-store row.
- **The first connection records the issuer at DETECTION time, not only on success.** A login that
  never completes still establishes the anchor a second connection needs. The authoritative record
  is still the one `save_with_issuer` writes, and only that failure is propagated.
- **A field on `AuthorizationResult` was again considered and REJECTED**, for 116-10's reason: it is
  public, all-`pub`-field and not `#[non_exhaustive]`, so a field is `constructible_struct_adds_field`
  = MAJOR. `ResolvedClientIdentity` is the private carrier, and its rustdoc says so in place.
- **`AUTH-03` is NOT booked complete.** `116-12`, `116-13`, `116-14` and `116-15` still claim it.
  `requirements-completed: []`, as in `116-01` through `116-10`.
- **RED was OBSERVED and logged, not COMMITTED as a broken build**, for the same reason as every
  plan in this phase. See *TDD Gate Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's `<action>` and `<behavior>` cannot both hold for the two in-repo callers**

- **Found during:** Task 1, reading what `config.cache_file` actually receives.
- **Issue:** the `<action>` says to resolve the default store "honoring `config.cache_file` when set";
  the `<behavior>` says `~/.pmcp/oauth-tokens.json` "is NEVER opened for reading" and "is left in
  place for the user to delete". Measured: `crates/mcp-tester/src/main.rs:594` and
  `cargo-pmcp/src/commands/auth.rs:76` both pass `Some(default_cache_path())`, i.e. that exact file.
  A `FileCredentialStore` over it would (a) hand the flat document to
  `parse_credential_snapshot`, which rejects it for having no `schema_version`, so every existing
  user's first call errors, and (b) overwrite it on the first save.
- **Fix:** the configured path's DIRECTORY is honoured and its file name is not — the store is
  `<dir>/oauth-cache.json`. Pinned by an inline drift test against `default_credential_path()` and
  by `the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it`, which asserts the
  legacy file is byte-identical after a full flow.
- **Committed in:** `4d27db50`. Logged as `D-116-PLANCONFLICT`.

**2. [Rule 1 — Bug] Resolving a default store when `cache_file` is `None` defeats `--no-cache`**

- **Found during:** Task 1, same reading.
- **Issue:** `cargo-pmcp/src/commands/auth.rs:73` sets `cache_file` to `None` when `--no-cache` is
  passed. The plan's "and `default_credential_path()` otherwise" would persist anyway.
- **Fix:** no `cache_file` and no injected store ⇒ no store, which is also what every previous cache
  guard in this module already meant. Two tests, one inline and one end to end.
- **Committed in:** `4d27db50`.

**3. [Rule 2 — Missing critical functionality] A store read failure would have been fatal**

- **Found during:** Task 1, writing `load_stored_credentials`.
- **Issue:** the plan says nothing about what happens when `store.load` fails. Propagating would let
  a corrupt document, a stale lock or a legacy-format file at a configured path make authentication
  impossible — a credential CACHE that can brick the thing it caches for.
- **Fix:** every store read failure is a warned cache MISS. The warning names the reason the store
  gave and no credential content. The same rule covers an unaddressable server key.
- **Committed in:** `4d27db50`.

**4. [Rule 1 — Bug] `.duration_since(UNIX_EPOCH).unwrap()` was a panic in library code**

- **Found during:** Task 1, replacing `cache_token`.
- **Issue:** the deleted `cache_token` and the live `get_access_token` both computed "now" with
  `.unwrap()`, which panics on a clock set before 1970 — a denial of service a caller cannot catch,
  inside a function whose whole job is to hand back a token.
- **Fix:** one module-level `unix_now_secs()` using `map_or(0, ..)`, shared by the
  authorization-code path, the refresh path and the device-code path, so the three cannot disagree
  about what "now" means. It also removed a pre-existing clippy error (see *Gate Results*).
- **Committed in:** `4d27db50`.

**5. [Rule 2 — Missing critical functionality] The plan's legacy-discard test is not a detector for the store's PATH**

- **Found during:** Task 1, the negative control.
- **Issue:** the plan's legacy row ("plant a syntactically valid legacy file with a distinctive
  token and assert that token never appears in any result") **PASSES** with the store pointed AT the
  legacy file, because the row injects a store and never reaches default resolution. Measured in the
  three-break control.
- **Fix:** `the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it` — no injected
  store, byte-identical legacy file asserted afterwards, `oauth-cache.json` asserted to exist beside
  it with the new token and without the canary, and a second helper asserted to read it back.
- **Committed in:** `4d27db50`.

**Total deviations:** 5 (3 × Rule 1, 2 × Rule 2). No Rule 4 situation arose; no architectural change
was needed. **Zero dependencies added** — `tempfile`, `mockito` and `tracing` are all pre-existing
dev/runtime dependencies.

## Issues Encountered

- **`make quality-gate` runs ZERO of this plan's 24 tests and exits 0.** Measured at `3b2a61e1`:
  `cargo nextest run --features full -E 'binary(oauth_store_wiring)'` reports `Starting 0 tests
  across 1 binary` then `error: no tests to run`; `cargo test --lib --features full
  credential_store_wiring` reports `0 passed … 1880 filtered out`. Under `full,oauth` the same two
  are **18** and **6**. The corroborating figure is the gate's own `test-unit` count: **1880**,
  identical to 116-09's and 116-10's, although this plan added **6** inline lib tests. Three
  consecutive plans, three non-moving populations.
- **The `full,oauth` clippy anchor has now moved three times in four plans: 29 → 24 → 21 → 17.**
  Every disappearance is a side effect of rewriting or deleting a line the plan had to touch. **The
  anchor for `116-12` is 17.**
- **`D-116-FUZZGATE` reconfirmed, unchanged.** Inside this plan's gate run, **21** targets each died
  on ``the option `Z` is only accepted on the nightly compiler``, each printed "completed", and the
  gate still exited 0.
- **`D-116-KEYCHAIN` did not reproduce**, a fifth independent clean observation: `1880 passed;
  0 failed`, with `grep -c "streamable_http.rs:4"` → **0** and `grep -c "Failed to load native root
  certificates"` → **0**, on a volume that never dropped below 107 GiB free.
- **`D-116-FAILFAST` applied throughout.** Every RED run, negative control and regression run used
  `--no-fail-fast` with the denominator asserted (9 compile errors, 11, 12, 12, 18, 18, 18, 18, 145,
  197, 13).
- **`git checkout -- <path>` was not attempted**; both restores used `cp` from a scratchpad COPY with
  `shasum -a 256 -c` returning **OK**, per 116-07's rule. `grep -c 'NEGATIVE CONTROL'` over the
  restored file returns **0** both times.
- **`rtk` corrupts `git` and `grep` output.** A `grep -c` through the proxy rendered as a
  multi-line "N matches in 1F" table rather than a number. Every command whose output this plan
  counted used `/usr/bin/grep`, `/usr/bin/git`, `/usr/bin/make` or `$HOME/.cargo/bin/cargo`.
- **`mockito::Mock` has no `forget()` in this version**, and a dropped `Mock` is REMOVED from the
  server. The harness returns `Vec<Mock>` alongside the `ServerGuard` so a row cannot silently
  assert against a server with no routes.
- **`Arc<dyn CredentialStore>` does not expose `CredentialStoreAdmin`**, which is 116-05's design
  working: the legacy row holds a concrete `Arc<InMemoryCredentialStore>` for `list_keys` and the
  `dyn` handle for everything else.

## Threat Flags

None. This plan adds no new network endpoint and no new socket. It adds FILE access only through
116-16's `FileCredentialStore`, whose register that is, and only when the caller opts in — the
default with no `cache_file` and no injected store touches nothing at all.

All `mitigate` dispositions in the plan's `<threat_model>` are discharged by a named test:

| Threat | Discharged by |
|---|---|
| T-116-39 (replaying a `client_id` or token issued by AS-A at AS-B) | `credentials_from_a_different_authorization_server_are_a_cache_miss_sep_2352` — a canary seeded under a foreign issuer, asserted never returned, asserted to SURVIVE (a miss is not a delete), and the flow asserted to have really run. It PASSED under the server-collapse break, so the issuer detector is independent of the server detector |
| T-116-39a (one MCP server reaching another's credentials under a shared AS and account) | `two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1` — asserts CONTENTS on both sides, asserts the two keys differ ONLY in the server component, and asserts deleting one leaves the other. OBSERVED failing under the two-part-key break |
| T-116-39b (the store naming one issuer while holding another's credentials) | one `save_with_issuer` call; `grep -n 'store\.save(' src/client/oauth.rs` returns **no output**, and `save_with_issuer` is counter-proven atomic in `FileCredentialStore` by 116-16 |
| T-116-40 (silent authorization-server substitution) | `an_issuer_change_with_dcr_credentials_warns_naming_both_issuers_and_proceeds` (a captured WARN asserted to name old issuer, new issuer and server) and `an_issuer_change_with_a_pre_registered_client_id_is_reauth_required_and_starts_no_flow` (`is_reauth_required` + `reauth_issuer`, 0 browser opens, `/token` `.expect(0)`, and the callback port still bindable). Both OBSERVED failing under PREFIX-RED |
| T-116-41 (attributing an issuer-less legacy token by guessing) | `the_legacy_issuer_less_token_cache_is_never_read_and_is_left_in_place` (canary absent from every stored record, file byte-identical) **plus** `the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it`, which is the one that actually detects the store being pointed at it |
| T-116-42 (credential store forced onto a home directory) | `an_injected_store_makes_the_flow_touch_no_file_at_all` (the directory holds exactly the one planted file afterwards), `constructing_a_helper_touches_no_filesystem` (a three-level missing directory stays missing), and `no_cache_file_and_no_injected_store_persists_nothing`. `grep -n 'default_credential_path' src/client/oauth.rs` shows **no call in any non-test line** |
| T-116-43 (AS-change detection not matching the spec's stated mechanism) | **accepted, and written down where a reader will hit it**: `announce_authorization_server_change`'s rustdoc names RFC 9728, says it is DEFERRED by owner decision (2026-08-02) and states what signal is used instead. Logged as `D-116-PRM` with the second, larger consequence — the D-116-R1 scenario is not constructible through the live flow either |
| T-116-SC (cargo installs) | zero packages; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0** |

## Known Stubs

None. Every item is fully implemented and exercised. `grep -nE 'TODO|FIXME|HACK|XXX'` over both
files returns **no output**, and `make check-todos` / `make check-unwraps` both exit 0 inside the
gate.

Two deliberate non-implementations, both documented decisions rather than stubs:

- **`refresh_token` still reads `config.client_id`** and therefore still cannot refresh a DCR flow.
  That is D-14's second defect, explicitly assigned to `116-12` by 116-05's contract
  (`StoredCredentials::client_id()` exists for it). This plan does not touch that function; it makes
  the fix possible by putting the DCR-issued id in the store, and the cached-refresh path already
  reads `cached.client_id()` for the record it writes back.
- **RFC 9728 Protected Resource Metadata** is not implemented, is deferred by owner decision, and
  its two consequences are named in `D-116-PRM`.

## TDD Gate Compliance

Both tasks carry `tdd="true"`. **RED was observed and logged for both.**

| Task | Control log | Result |
|---|---|---|
| 1 | `116-11-task1.RED.log` | **9 × E0599**, exit 101 — `with_credential_store` / `with_account_scope` / `list_keys` do not exist |
| 1 | `116-11-task1.NEGATIVE-CONTROL.log` | **12 run, 6 passed, 6 failed** — three breaks at once |
| 2 | `116-11-task2.RED.log` | **18 run, 16 passed, 2 failed** — only the two CHANGE rows |
| 2 | `116-11-task2.NEGCTL-EF.log` | **18 run, 14 passed, 4 failed** |
| 2 | `116-11-task2.NEGCTL-D.log` | **18 run, 16 passed, 2 failed** |

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01` through
`116-10`: in Rust a test naming a non-existent method fails to *compile*, so such a commit leaves a
non-building tree that breaks `git bisect` and contradicts CLAUDE.md's "ZERO TOLERANCE FOR
DEFECTS". A verifier looking for a `test(...)` → `feat(...)` pair will not find one; the evidence is
the five control logs above, each named in its commit body.

### Task 1 — three breaks at once (`--no-fail-fast`, denominator 12 asserted)

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| **A.** the key collapsed to the old two-part `(issuer, account)` form | `a_completed_flow_stores_credentials_under_the_three_part_key`, `the_server_component_is_normalized_so_a_path_does_not_fork_the_login`, `two_mcp_servers_sharing_one_authorization_server_and_account_stay_disjoint_d_116_r1`, `a_different_account_scope_is_a_cache_miss_and_stores_its_own_entry` | **`credentials_from_a_different_authorization_server_are_a_cache_miss_sep_2352` still PASSED** — the ISSUER detector is genuinely independent of the SERVER detector, reproducing 116-05's measurement one layer up. `a_second_call_with_the_same_key_hits_the_cache_and_opens_no_browser` also held, because a two-part key is self-consistent within one server |
| **B.** the store pointed AT the legacy flat file instead of beside it | `the_default_store_lives_beside_the_legacy_file_and_never_on_top_of_it` **only** | **`the_legacy_issuer_less_token_cache_is_never_read_and_is_left_in_place` still PASSED**, because it injects a store and never reaches default resolution. That measurement is why the first row exists |
| **C.** the registered `application_type` dropped on the way to the store | `a_dcr_flow_stores_the_issued_client_id_and_the_registered_application_type` **only** | every other row held — the `client_id` half of the same record is a separate assertion and was unaffected |

### Task 2 — two controls, because the RED run was not sufficient

The RED run's 2 failures are the two change rows. **Four of the six Task 2 rows PASSED pre-fix**, and
that is recorded rather than presented as coverage: "no warning" and "no error" were already
accidentally true when nothing detected anything, and `a_first_connection_records_…` and the
isolation row were already satisfied by Task 1's `save_with_issuer`.

| Control | Break | Tests that FAILED | Siblings that still PASSED |
|---|---|---|---|
| **E+F** | the provenance branch removed (a pre-registered change also merely warns) **+** the issuer record written to ONE shared slot instead of per server | `an_issuer_change_with_a_pre_registered_client_id_is_reauth_required_and_starts_no_flow`, `a_first_connection_records_the_discovered_issuer_against_the_normalized_server_key`, `an_issuer_change_with_dcr_credentials_warns_naming_both_issuers_and_proceeds`, `an_issuer_change_for_one_server_leaves_another_servers_issuer_record_untouched` | `an_unchanged_issuer_on_a_second_connection_neither_warns_nor_errors` and `a_store_that_does_not_track_issuers_still_works` — the two "must NOT fire" rows are independent of the "must fire" ones |
| **D** | every change made fatal, provenance ignored | `an_issuer_change_with_dcr_credentials_warns_naming_both_issuers_and_proceeds`, `an_issuer_change_for_one_server_leaves_another_servers_issuer_record_untouched` (A's flow now errors) | **`an_issuer_change_with_a_pre_registered_client_id_is_reauth_required_and_starts_no_flow` still PASSED** — it expects an error and got one. Only the DCR-proceed rows detect D-18's "never a hard fail for DCR", which is the whole RESEARCH A4 refinement |

Source restored from a scratchpad COPY after each control, never `git checkout --`.
`shasum -a 256 -c` returned **OK** both times, and `grep -c 'NEGATIVE CONTROL'` over the restored
file returns **0**.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| **clippy baseline, measured on the PRISTINE `70dc259f` tree BEFORE any edit** | `make lint`'s command with `--features "full,oauth"` | **21 errors, all 21 in `src/client/oauth.rs`**, exit 101 |
| clippy after Task 1 | same | **17** — **0 NEW**, 4 GONE, compared as a multiset of (message, offending source line) |
| **clippy after Task 2** | same | **17** — **0 NEW**, 4 GONE |
| Task 1 RED | `-E 'binary(oauth_store_wiring)'` | **9 × E0599**, exit 101 |
| Task 1 GREEN | same, `--no-fail-fast` | **12 run, 12 passed** |
| Task 1 negative control | same | **12 run, 6 passed, 6 failed** |
| Task 2 RED | same | **18 run, 16 passed, 2 failed** |
| Task 2 negative control E+F | same | **18 run, 14 passed, 4 failed** |
| Task 2 negative control D | same | **18 run, 16 passed, 2 failed** |
| **final suite** | `binary(oauth_store_wiring)`, `--features full,oauth` | **18 run, 18 passed** |
| **narrow-gate reality** | the same selector, `--features full` | **0 tests run**, `error: no tests to run` |
| inline lib tests | `cargo test --lib --features full,oauth credential_store_wiring` | **6 passed**; under `--features full`: **0 passed, 1880 filtered out** |
| no regression | `oauth_dcr_integration + oauth_iss_integration + oauth_state_csrf + oauth_credential_store + oauth_credential_file + oauth_discovery_validation + oauth_provider_discovery + v2_bounded_reads_tripwire + oauth_store_wiring` | **197 run, 197 passed** |
| **bounded-reads tripwire** | `-E 'binary(v2_bounded_reads_tripwire)'`, both feature sets | **13 run, 13 passed** ×2 |
| doctests | `cargo test --features full,oauth --doc client::oauth` | **6 passed, 0 failed** |
| lint (**authoritative**) | `/usr/bin/make lint` | **exit 0**, "No lint issues" (after each task) |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** (twice); `grep -c cognitive_complexity src/client/oauth.rs` → **0** |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** naming `client/oauth.rs`, first pass both times |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, **92** warnings (= the 116-BASELINES anchor) |
| flat cache type gone | `grep -n 'struct TokenCache' src/client/oauth.rs` | **no output** |
| legacy file never in a read path | `grep -n 'oauth-tokens.json' src/client/oauth.rs` | **5** hits: one in the discard warning's rustdoc, two in `default_cache_path`'s doc and its `push`, two in inline tests. **No read path** (`D-116-GREP`: the hits are reported, not claimed to be zero) |
| no default path in the constructor | `grep -n 'default_credential_path' src/client/oauth.rs` | **6** hits, all rustdoc or the inline drift test — **zero calls in non-test code**, and none in `OAuthHelper::new` |
| no new public field | `grep -n 'pub credential_store\|pub account' src/client/oauth.rs` | **no output** |
| three-component key | `grep -n 'CredentialKey::new' src/client/oauth.rs` | **1** construction, THREE arguments, third the `normalize_server_key` result |
| one persistence call | `grep -n 'store\.save(' src/client/oauth.rs` | **no output**; `save_with_issuer` is the only write |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over both files | **no output** |
| unwrap/expect | `grep -n 'unwrap()\|expect('` over the module | only inside `#[cfg(test)]` modules, plus one `///` line quoting the panic that was removed |
| gate: `test-unit` | inside `make quality-gate` | **1880 passed; 0 failed** — unchanged from 116-09 and 116-10, which IS the D-116-LINT-OAUTH proof |
| gate: `test-doc` | inside `make quality-gate` | **445 passed; 0 failed; 79 ignored** |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 0** at 107 GiB free |
| disk | `df -h /` before and after | 113 GiB → 107 GiB free (`D-116-DISK` never triggered) |

## User Setup Required

None. No external service, no credential and no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

There are two operator-visible behaviour changes for `116-13`'s CHANGELOG:

1. **An existing `~/.pmcp/oauth-tokens.json` is discarded, not migrated.** It records no issuer, so
   it cannot be re-keyed without the guessing SEP-2352 forbids. One re-login is required. The file
   is left on disk; a `tracing::warn!` names it and says so.
2. **A change of authorization server for a known MCP server is now announced**, and for a client
   configured with a pre-registered `client_id` it is an error (`Error::reauth_required`) rather
   than a silent login at the new identity provider.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-PRM` (new)** — RFC 9728 Protected Resource Metadata is a NAMED DEPENDENCY of two things
  this phase shipped, not merely a deferred nicety: (a) the D-116-R1 collision is not constructible
  through the live flow, because the authorization server is derived from the MCP base URL and
  116-07's RFC 8414 §3.3 anchor then forces two origins to two issuers; (b) D-18's detection is
  narrower than the specification's stated mechanism. Owner: `116-15`, to record it with a named
  owner rather than as a generic deferral, and to quote both consequences when AUTH-03 is booked.
- **`D-116-PLANCONFLICT` (new)** — a plan `<action>` whose two instructions cannot both be satisfied
  against the tree, with the `grep -rn` that settles it. Proposed convention for `D-116-GREP`'s
  list: when a plan's `<action>` names a configuration field, record what the in-repo callers
  actually pass to it. Owner: `116-15`.
- **`D-116-LINT-OAUTH` (both halves re-measured, appended)** — the clippy anchor is **17** at
  `3b2a61e1`, down from 21, with zero new errors from this plan; the test-side twin is **0 of 24**,
  and the gate's population is still **1880** for the third consecutive plan that added inline
  tests. **81** tests from 116-09, 116-10 and 116-11 are outside CI. Owner: `116-15`.
- **`D-116-FUZZGATE`** — reconfirmed inside this plan's gate run (21 nightly failures, all
  swallowed, gate still exits 0). Still open for `116-15`.
- **`D-116-FALLBACK`** — untouched and **provably not made worse**: the new `reauth_required`
  refusal is raised BEFORE `authorize_with_fallback`, so it never reaches the wrapper that
  downgrades a failure into "No supported OAuth flow available" and never triggers device-code
  fallback. Asserted by the pre-registered row's `is_reauth_required()` on both entry points.
- **`D-116-KEYCHAIN`** — did not reproduce; fifth clean observation. **`D-116-DISK`** — never
  triggered. **`D-116-FAILFAST`**, **`D-116-DOC`**, **`D-116-TRIPWIRE`**, **`D-116-SLASH`**,
  **`D-116-EX`**, **`D-116-GREP`** — unchanged; nothing here reopens any of them.

## Next Phase Readiness

| Consumer | What it can now rely on |
|---|---|
| `116-12` | the store holds the effective `client_id` — **DCR-issued when DCR fired** — so D-14's second defect is fixable: `refresh_token` still reads `config.client_id` and must be changed to read `cached.client_id()`. `granted_scopes()` is populated from `AuthorizationResult.scopes`, which 116-10 made the RFC 6749 §5.1-correct value, so refreshing with it is safe; do NOT add `offline_access` there (RFC 6749 §6, the rule is at `OFFLINE_ACCESS_SCOPE`). `token_from_store` is the call site to change and it already preserves the client id, the granted scopes and the registered application type across a refresh. The DCR **success**-path body read is still the pre-116-06 `bytes()`-then-measure form and is still yours. **The `full,oauth` clippy anchor is 17** |
| `116-13` | `with_credential_store` / `with_account_scope` are the injection seam; `cargo-pmcp`'s `auth` subcommands should build a `FileCredentialStore` explicitly rather than relying on `cache_file`. Both `cargo-pmcp` and `mcp-tester` currently pass `default_cache_path()`, which now resolves the store to `~/.pmcp/oauth-cache.json` beside it — the same path `default_credential_path()` returns, so `CredentialStoreAdmin` operates on the same document. Two CHANGELOG lines are owed (see *User Setup Required*) |
| `116-14` | `Error::reauth_required` is now raised from a second site with a message naming both issuers and the server key |
| `116-15` | `make quality-gate` **exit 0** at this HEAD, fourth consecutive clean full-gate run. Two new deferred entries; `D-116-LINT-OAUTH` re-measured with a third plan's numbers. **Do not book AUTH-03 on this plan's evidence alone** — `D-116-PRM` names what has no end-to-end coverage and why |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-12` | measure the `full,oauth` clippy baseline BEFORE editing; it is **17** at `3b2a61e1`, NOT 21, 24 or 29 |
| `116-12` | do NOT introduce `offline_access` (or any scope) at refresh — RFC 6749 §6 narrow-never-widen |
| `116-13` | CHANGELOG: the legacy `oauth-tokens.json` is discarded (one re-login, file left in place), and an authorization-server change is now announced and is fatal for a pre-registered `client_id` |
| `116-15` | close `D-116-LINT-OAUTH` as a PAIR (clear 17, then enable `oauth` in lint AND tests) — **81** tests are outside CI; record `D-116-PRM` with a named owner; do not book `AUTH-03` on this plan's evidence alone |
| every source-touching plan | `make lint` AND the `full,oauth` gate-equivalent; `--no-fail-fast` with the denominator asserted; restore from a scratchpad COPY, never `git checkout --`; absolute binary paths for anything whose output you count |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: tests/oauth_store_wiring.rs                                1264 lines (min_lines 170 ✓)
FOUND: src/client/oauth.rs                                        3272 lines (was 2680)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md  918 lines (was 786)
```

Commits claimed, verified in `git log`:

```
FOUND: 4d27db50  feat(116-11): route credential persistence through the issuer-keyed store
FOUND: 3b2a61e1  feat(116-11): announce an authorization-server change, fatal by provenance
```

`must_haves` verification:

```
✓ truths[1] tokens and DCR-issued client_ids are stored under (issuer, account, server), so neither
  an authorization-server switch NOR a second MCP server sharing that authorization server can reach
  another's credentials — credentials_from_a_different_authorization_server_are_a_cache_miss_sep_2352
  and two_mcp_servers_..._stay_disjoint_d_116_r1, the second asserting CONTENTS on both sides and
  that deleting one leaves the other; OBSERVED failing under the two-part-key break while the first
  held
✓ truths[2] the legacy issuer-less token file is never read as credentials, and its presence is
  announced once — two rows, the second (the_default_store_lives_beside_the_legacy_file_...) added
  BECAUSE the first was measured not to detect the store being pointed at it
✓ truths[3] a change of authorization server is announced by name and is fatal only where the spec
  says — a captured WARN asserted to name both issuers and the server for DCR provenance;
  is_reauth_required + reauth_issuer + 0 browser opens + /token expect(0) + an unbound callback port
  for pre-registered provenance; NEGCTL-D proves the DCR-proceed rows are the detector
✓ truths[4] a platform can supply its own credential store without the SDK reaching for a home
  directory — with_credential_store; an_injected_store_makes_the_flow_touch_no_file_at_all,
  constructing_a_helper_touches_no_filesystem, no_cache_file_and_no_injected_store_persists_nothing;
  grep: no default_credential_path call in any non-test line
✓ artifacts: src/client/oauth.rs contains "CredentialStore" and provides the wiring,
  with_credential_store / with_account_scope, the legacy discard and D-18 detection
✓ artifacts: tests/oauth_store_wiring.rs 1264 >= 170 — three-part-key round trip, cross-AS miss,
  two-servers-one-AS collision miss, legacy discard, and issuer-change warn vs error by provenance
✓ key_links: src/client/oauth.rs -> src/shared/credential_store.rs via load / save_with_issuer /
  last_issuer keyed by (issuer, account, server); pattern (CredentialKey|save_with_issuer|last_issuer)
  matches at :945, :1190, :1104
```

Plan-level verification block:

```
✓ binary(oauth_store_wiring) — 18 run / 18 passed under --features full,oauth, non-zero count
✓ binary(oauth_dcr_integration) 24/24 and binary(oauth_iss_integration) — 116-09 and 116-10
  unregressed; 197/197 across nine binaries
✓ make quality-gate exit 0; make lint exit 0; full,oauth clippy 17 vs a 21 PRE-MEASURED pristine
  baseline with ZERO new errors attributable
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations, no new allow
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable
✓ binary(v2_bounded_reads_tripwire) — 13 run, 13 passed, under both feature sets
✓ wasm32 build — exit 0, 92 warnings = the 116-BASELINES anchor
⚠ make quality-gate runs 0 of this plan's 24 tests (D-116-LINT-OAUTH test-side twin), measured
  rather than left implicit: 0 under --features full, 24 under --features full,oauth, and the
  gate's own test-unit population unchanged at 1880
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
