---
phase: 116-auth-hardening-seps
plan: 05
subsystem: auth
tags: [oauth, sep-2352, credential-storage, migration, wasm32, semver, proptest, ci-gate]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 02
    provides: "the pure-tier pattern (ungated src/shared/ module + rationale comment on the pub mod + ungated crate-root re-export), the D-116-DOC intra-doc-link rule, and the both-feature-sets measurement discipline"
  - phase: 116-auth-hardening-seps
    plan: 04
    provides: "the INVERSE half of D-116-DOC (bare links in /// item docs), D-116-LINT's 'make lint is authoritative' finding, and D-116-KEYCHAIN's revert-in-place attribution method"
provides:
  - "CredentialKey — the (issuer, account, server) key, so BOTH collision classes miss with no enforcement branch"
  - "StoredCredentials — private fields, cargo-pmcp-compatible serde names, MANUAL redacting Debug"
  - "CredentialSnapshot + CREDENTIAL_SCHEMA_VERSION + parse_credential_snapshot — the whole on-disk story as pure, byte-stable, panic-free code"
  - "MigrationReport / DroppedEntry — a migration that drops an unkeyable login REPORTS it instead of guessing an issuer"
  - "CredentialStore — the narrow platform seam: 6 methods, 3 defaulted, no HTTP client, no refresh"
  - "CredentialStoreAdmin — the 4 operations 116-13's auth subcommands need, proven against an impl before 116-13 exists"
  - "InMemoryCredentialStore (+ from_bytes) — both traits, delegating to CredentialSnapshot so no semantics can drift"
  - "normalize_server_key — cargo-pmcp's normalize_cache_key ported into core with its idempotence property"
  - "wasm32-purity CI job wired into the org-required `gate` aggregate — D-06 is now enforced, not asserted"
  - "D-116-FAILFAST — the measured proof that nextest's default fail-fast silently truncates a negative control"
affects: [116-08, 116-11, 116-12, 116-13, 116-16, 116-15]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Format-and-migration in the PURE tier, I/O in the gated one: a platform storing the same blob in a KV store gets byte-identical migration behaviour to the CLI, and the migration becomes fuzzable with no filesystem"
    - "Two traits, not one: a narrow platform seam plus an administrative sibling, because a default `Ok(0)` on the narrow seam is a lie a CLI would print as a count"
    - "Nesting the document issuer -> server -> account instead of concatenating a composite string key, so no separator has to be invented that an issuer URL could contain"
    - "BTreeMap at every level as a CORRECTNESS choice, not a style one: byte-stability is what stops an atomic write churning the file on every save"
    - "A negative control that collapses the new key component back to the OLD design, so the failing set IS the set of tests that justify the change"
    - "Combined write (save_with_issuer) with a documented non-atomic default, so a transactional implementor can be atomic without breaking a minimal one"

key-files:
  created:
    - src/shared/credential_store.rs
    - tests/oauth_credential_store.rs
  modified:
    - src/shared/mod.rs
    - src/lib.rs
    - .github/workflows/ci.yml
    - Makefile
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "The key is (issuer, account, server) — D-116-R1, AUTH-03's amended text, NOT a planner preference; asserted on the live path, the migration path AND the trait level"
  - "clear() and delete_by_server() also forget the last-seen ISSUER records, because a logout must not leave behind a list of which authorization servers the user visited"
  - "parse_credential_snapshot's refusals carry serde's CLASSIFICATION plus line/column and never the serde message, because a serde data error echoes the offending input"
  - "InMemoryCredentialStore OVERRIDES save_with_issuer to be atomic; the trait default is documented as NOT atomic"
  - "No Makefile flag change — `make wasm-build` already existed; the Makefile edit is a note that the target is now CI-load-bearing"
  - "`oauth` was NOT added to the `full` feature (Pitfall 3's correct response is explicit --features full,oauth runs per plan)"
  - "AUTH-03 is NOT booked complete — 116-06/07/10/11/12/13/16 all still claim it"

patterns-established:
  - "Run negative controls with `cargo nextest run --no-fail-fast`: the default truncates and prints a plausible partition from a partial run (D-116-FAILFAST)"
  - "A grep-shaped acceptance criterion survives rustfmt only if the signature fits 100 columns — `pub fn new<I, A, S>(issuer: I, account: A, server: S)` does where `impl Into<String>` x3 does not"
  - "Both halves of D-116-DOC applied in one module with ZERO new doc-check errors, confirming the rule as amended by 116-04"

requirements-completed: []

# Metrics
duration: 310min
completed: 2026-08-03
---

# Phase 116 Plan 05: SEP-2352 Credential Storage as a Pure, Fenced Tier Summary

**Credentials are now addressed by `(issuer, account, server)`, and BOTH collision classes — a
different authorization server, and a different MCP server sharing one authorization server and
one account — miss with no enforcement code anywhere. The whole on-disk story (the document
format, the schema 1 → 2 migration and the migration report) is pure, byte-stable, panic-free
and wasm32-clean, so a platform keeping the same blob in a KV store gets identical migration
behaviour to the CLI. Every operation 116-13's five `auth` subcommands need is declared on
`CredentialStoreAdmin` and proven against an implementation before 116-13 is written. And D-06 —
which was an unenforced assertion until now — is a CI job wired into the org-required `gate`.**

## Performance

- **Duration:** ~310 min
- **Completed:** 2026-08-03
- **Tasks:** 3
- **Files:** 7 (2 created, 5 modified), **+2212 / −13**, **0 removed**

## Accomplishments

- **The two collision classes are proven independent, by breaking the key and watching WHICH
  tests fail.** The negative control collapsed `CredentialKey`'s `server` component back into the
  old two-part `(issuer, account)` form — literally the previous design. **17 of 54** tests
  failed, including D-116-R1 on **all three paths**: the live path
  (`two_keys_differing_only_in_server_are_distinct`), the **migration** path
  (`two_schema_1_servers_sharing_one_issuer_stay_independent` — where a two-part key could have
  silently overwritten one server's credentials with another's) and the trait level
  (`load_with_a_different_server_is_a_miss`). Meanwhile
  `two_keys_differing_only_in_issuer_are_distinct`, `two_keys_differing_only_in_account_are_distinct`,
  `load_with_a_different_issuer_is_a_miss` and `load_with_a_different_account_is_a_miss` **all
  still passed** — so the server detector is genuinely independent of the issuer and account
  detectors, and SEP-2352's own MUST is not what is carrying the new component.

- **A migration that cannot re-key an entry says so instead of guessing.** This is the refinement
  D-17's "every existing login is preserved" needed: every login that **records its issuer** is
  preserved losslessly — the schema-1 map key IS the normalized server URL, so populating the
  widened key needs no new information — but a schema-1 entry with **no** `issuer` cannot be
  re-keyed without guessing which authorization server issued it, which is precisely what
  SEP-2352 forbids. It is dropped and reported as a `DroppedEntry` naming the server key and the
  reason. Under a deliberate break that assigned `https://unknown.example` instead, exactly two
  tests failed — the drop-and-report test and `take_migration_report_yields_once_then_none` —
  while the corrupt-bytes, empty-input, unknown-version and empty-entries rows all held.

- **Both feature sets, again, in both directions.** `binary(oauth_credential_store)` reports
  **54 tests run, 54 passed** under `--features full,oauth` **and** **54 tests run, 54 passed**
  under plain `--features full`. The second is the one that matters: it proves the tier is
  genuinely ungated and therefore also covered by `make lint`, which 116-01 measured compiles
  *none* of this phase's `oauth`-gated code. `cargo build --target wasm32-unknown-unknown
  --no-default-features --features wasm` exits **0** with **92** warnings — exactly the
  116-BASELINES anchor — and **zero** of them name `credential_store.rs`.

- **D-06 stopped being an assertion.** Nothing in CI built for wasm32, so a contributor adding a
  native-only dependency to either ungated module would have broken the platform seam with a
  green build. The new `wasm32-purity` job installs the target explicitly (RESEARCH A5 records
  that its availability on a runner was never probed), invokes the **existing** `make wasm-build`
  target so the fence and the local command cannot diverge, and is listed in `gate`'s `needs:` —
  which is now a **strict superset** of its value at `b2bf9157`, verified by parsing the YAML.

- **The seam stayed narrow while the CLI got what it needs.** Splitting `CredentialStoreAdmin`
  off means a `DynamoDB`-backed store vending one user's token for one server implements three
  methods, and is never asked to `clear_all`. The four `auth logout` semantics are asserted
  **against the trait, here, before 116-13 exists** — the no-op for an unknown server returns
  `0` and is not an error, `delete_by_server` returns an exact `2` from a three-credential
  two-server fixture, `clear_all` returns the total, and `take_migration_report` yields once and
  then `None`.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | Three-part key, record, document format and schema 1 → 2 migration | `d03e6be4` | feat |
| 2 | `CredentialStore` seam, `CredentialStoreAdmin` and the in-memory impl | `ec80e5b1` | feat |
| 3 | wasm32 build fence wired into the org-required `gate` | `34b67482` | ci |

## Files Created/Modified

- **`src/shared/credential_store.rs`** (**created**, **1064** lines — `min_lines` 380 ✓). Ungated,
  I/O-free, one `#[cfg(test)]` and no other `cfg`. Public: `CREDENTIAL_SCHEMA_VERSION`,
  `CredentialKey`, `StoredCredentials`, `CredentialSnapshot`, `MigrationReport` (`#[non_exhaustive]`),
  `DroppedEntry`, `parse_credential_snapshot`, `normalize_server_key`, `CredentialStore`,
  `CredentialStoreAdmin`, `InMemoryCredentialStore`. Private: `AccountMap`/`ServerMap`/`IssuerMap`
  aliases, `DocumentRef`/`Document`/`VersionProbe`/`LegacyCache`/`LegacyEntry`, `parse_current`,
  `migrate_legacy`, `malformed_document`, `unsupported_schema_version`, `forget_issuer`,
  `credential_count`. 5 inline tests, **9** doctests (one `compile_fail`).
- **`tests/oauth_credential_store.rs`** (**created**, **1041** lines — `min_lines` 200 ✓). 54 tests
  in nine documented groups plus a `minimal` module holding the three-method hand-written
  implementor. NOT `#![cfg(feature = "oauth")]`, which is the point.
- **`src/shared/mod.rs`** (+16) — `pub mod credential_store;` with the load-bearing "ungated on
  purpose" rationale naming the gated counterpart.
- **`src/lib.rs`** (+13) — ungated crate-root re-export of the eight call-site names.
  `normalize_server_key`, `DroppedEntry` and `CREDENTIAL_SCHEMA_VERSION` stay module-path-only,
  per the plan's `<interfaces>`.
- **`.github/workflows/ci.yml`** (+58/−3) — the `wasm32-purity` job and its `gate` wiring.
- **`Makefile`** (+8) — a note that `wasm-build` is now CI-load-bearing. **No flag change.**
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (237 → **278**) — one new entry,
  `D-116-FAILFAST`.

## Decisions Made

- **The three-part key is the requirement, not a preference.** AUTH-03's text was amended in
  `0aebf7f6` to `(issuer, account, server)` and CONTEXT's D-07 wording is superseded. The
  rustdoc states the reason in place, because the two-component version reads perfectly
  reasonable in isolation: two MCP servers can share one authorization server and one account
  while holding different registrations, different client IDs and different granted scopes. RFC
  8707's `resource` parameter would have bound the audience; it is deferred (`b2bf9157`), so the
  key carries the binding.
- **`clear()` and `delete_by_server()` also forget the last-seen ISSUER records (Rule 2 — not in
  the plan).** The plan specifies the credential counts and says `clear` "empties the snapshot",
  but does not say what happens to D-18's issuer tracking. Leaving it would mean `auth logout
  --all` retains a list of which authorization servers the user visited — data the user just
  asked to remove, and information in its own right. `delete_by_server` forgets only the named
  server's record, which keeps T-116-13b's scoping intact. Both are documented in place.
- **Refusals carry serde's *classification* and line/column, never serde's message.** A
  `serde_json` **data** error reproduces the offending value (`invalid type: string "…"`), which
  for a credential document is exactly the byte sequence that must not reach a log.
  `malformed_document` builds its own message from `err.classify()`, `err.line()` and
  `err.column()` — useful, and provably canary-free
  (`corrupt_bytes_are_an_error_that_echoes_no_input`).
- **The document nests issuer → server → account.** Concatenating a composite string key would
  require inventing a separator that an issuer URL could itself contain. `BTreeMap` at every
  level is a correctness choice: `to_bytes_is_byte_stable_across_calls` is what makes a diff of a
  credential file meaningful and stops an atomic write churning on every save.
- **`InMemoryCredentialStore` OVERRIDES `save_with_issuer`** to do both mutations under one write
  lock, demonstrating the pattern the plan asks a transactional implementor to follow. The
  trait's default is documented as **not** atomic, and the hand-written `MinimalStore` in the
  test file exercises it.
- **`CredentialKey::new` uses named generics (`<I, A, S>`), not three `impl Into<String>`.** The
  `impl Trait` form is 106 columns after rustfmt and therefore breaks across lines, which would
  have made the plan's `grep -n 'pub fn new'` acceptance criterion unable to show the
  three-argument signature. The generic form fits in 62 and greps as one line. Same semantics.
- **`AUTH-03` is NOT booked complete.** This plan lands the storage tier; `116-11`/`116-12` wire
  the OAuth helper, `116-13` wires cargo-pmcp, `116-16` lands the file implementation, and
  `116-06`/`116-07`/`116-10` own the other AUTH-03 clarifications. `requirements-completed: []`,
  as in `116-01` through `116-04`.
- **RED was OBSERVED and logged, but not COMMITTED as a broken build**, for the fourth time in
  this phase and for the same reason. See *TDD Gate Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Two `make lint` hard errors the phase's clause-(b) clippy command accepts — D-116-LINT, twice more**

- **Found during:** Task 1, running `make lint` per 116-03/116-04's standing obligation.
- **Issue:** `clippy::doc_markdown` on a bare `snake_case` in a `///` doc, and
  `clippy::needless_pass_by_value` on `malformed_document(err: serde_json::Error)`. Both are hard
  errors under `make lint`'s `RUSTFLAGS="-D warnings"`; neither appears under clause (b).
- **Fix:** backticked `snake_case`; `malformed_document` now takes `&serde_json::Error` and the
  three call sites pass `&e`. `make lint` → **✓ No lint issues** afterwards, and again after
  Task 2.
- **Why it is recorded rather than shrugged off:** this is the **fourth** plan in a row where
  clause (b) reported clean on gate-red code. D-116-LINT now has five independent measurements
  across three plans.
- **Committed in:** `d03e6be4`.

**2. [Rule 3 — Blocking] D-116-DISK, twice, mid-plan**

- **Found during:** Task 2's ungated run, and again after the negative control.
- **Issue:** `error: linking with 'cc' failed` and `rustc-LLVM ERROR: IO failure on output
  stream` in **test binaries and examples this plan never touched**
  (`v2_tasks_update_routing`, `structured_tool_output`, `m04_server_http_middleware`, …). The real
  cause, recoverable only by filtering: `ld: write() failed, errno=28 (No space left on device)`.
  `df -h /`: **132 Mi free at 99%**, then **532 Mi at 96%**.
- **Fix (not a code change):** `rm -rf target/debug/incremental target/semver-checks` (21 GB),
  then `… target/debug/examples` (8 GB) → 14 GiB free both times. Identical commands then passed.
- **Confirms the documented guidance:** `df -h /` **before** diagnosing any unexplained link
  failure. The volume went from 25 GiB free at plan start to 132 Mi after roughly six full-feature
  builds; `target/debug/incremental` regrew 21 GB in that window.

**3. [Rule 1 — Bug] The negative control's first run was TRUNCATED and looked like a result**

- **Found during:** Task 2's negative control.
- **Issue:** `cargo nextest run` fail-fast is **on by default**. The first run reported
  `Summary [0.025s] 15/54 tests run: 10 passed, 5 failed` — which reads as a five-failure
  partition but is a run that stopped after the fifth failure having executed 15 of 54 tests.
  **12 of the 17 real detectors never ran**, including all three D-116-R1 path tests, so the
  surviving-sibling argument would have been unsupported by the log while appearing supported.
- **Fix:** re-ran with `--no-fail-fast` → `54 tests run: 37 passed, 17 failed`, which is the
  partition reported below. The tell is the `15/54` fraction; a complete run prints
  `54 tests run` with no fraction.
- **Logged as `D-116-FAILFAST`** for the remaining plans, because it composes badly with 116-01's
  selector trap: both produce a plausible summary line from a run that did not do what the reader
  thinks.
- **Committed in:** `ec80e5b1` (the corrected log is the one cited).

**4. [Rule 2 — Missing critical functionality] Issuer records survive a logout**

Described under *Decisions Made*. Not in the plan's behaviour rows; added because a logout that
retains a record of which authorization servers the user visited is a disclosure the user just
asked to remove.

**Total deviations:** 4 (2 × Rule 1, 1 × Rule 2, 1 × Rule 3). No Rule 4 situation arose; no
architectural change was needed. **Zero dependencies added** — `git diff --exit-code
b2bf9157..HEAD -- Cargo.toml` exits **0**, discharging `T-116-SC`. `async-trait`, `parking_lot`,
`serde`, `serde_json` and `url` are all pre-existing non-optional dependencies already present in
the wasm32 build (`src/shared/event_store.rs` uses `parking_lot::RwLock` in the same ungated way).

### Non-deviation worth recording

The plan's `files_modified` lists `Makefile`, and its action says to "run the existing `make
wasm-build` target rather than a bespoke cargo line". `wasm-build` already existed with exactly
the required flags, so **no flag change was needed or made**. The Makefile edit is a comment
recording that the target is now CI-load-bearing, so a future editor narrowing its flags learns
that they are changing what CI enforces.

## Issues Encountered

- **`make quality-gate` exits 2 at `test-unit` — D-116-KEYCHAIN, unchanged and NOT attributable
  to this plan.** Measured: **`1836 passed; 13 failed`**, total **1849**. 116-04 measured
  `1830 passed; 14 failed`, total **1844**. The total moved by exactly **5** — this plan's five
  new inline tests — and **all 13** failures are in `shared::streamable_http::tests`, **all 13**
  panic at the same pre-existing line `src/shared/streamable_http.rs:458`, and **zero** name
  `credential_store`. (`grep -oE 'panicked at src/[a-z_/]*\.rs:[0-9]+' | sort | uniq -c` →
  `13 panicked at src/shared/streamable_http.rs:458`.) The failure count moved 14 → 13, which is
  the documented flakiness, not a fix.
- **Every OTHER gate stage exits 0**, run individually: `fmt-check`, `lint`, `build`,
  `pmcp-package-gate`, `audit`, `unused-deps`, `check-todos`, `check-unwraps`, `purity-check`,
  `comply`. `test-property` and `test-doc` were still running when the plan closed; the property
  and doctest evidence for **this plan's** code is cited directly instead (4 proptest blocks
  inside the 54-test suite, and `cargo test --features full,oauth --doc credential_store` →
  **9 passed**).
- **`ps aux | grep …` returns empty even while `make` is running** under this environment's
  command proxy — the same unreliable-process-liveness hazard 116-04 recorded. Wait on a marker
  written into the log instead.
- **`cargo semver-checks` reports "no semver update required"** for the fourth plan running,
  despite eleven new public items. The requirement (*zero breaking findings*) is met:
  **223 checks, 223 pass, 0 fail**, exit 0. `116-13` must not rest its version-bump reasoning on
  this tool's verdict.
- **Both halves of the amended `D-116-DOC` rule applied cleanly on the first pass.** The inner
  `//!` block fully qualifies its three intra-doc links; every `///` item doc uses the bare form.
  `make doc-check` `^error` count: **28** — exactly the anchor — with **0** hits for
  `credential_store`. This is the first plan in the phase to hit the anchor without a correction
  round, which is evidence the rule as 116-04 amended it is correct.
- **No `.proptest-regressions` file was generated**, because no property ever failed — including
  under the three deliberate breaks, none of which the four properties detect. That is expected:
  the properties assert totality, idempotence and component round-tripping, and the breaks
  changed addressing, migration policy and rendering. The hand-written tables are the real fence
  here, exactly as 116-04 measured for `issuer_matches_metadata`.

## Threat Flags

None. This plan adds no network endpoint, no socket, no file access and no schema change to any
trust boundary beyond the one it defines — the module is I/O-free by construction, which the
wasm32 build and the import greps both demonstrate.

| Threat | Disposition | Discharged by |
|---|---|---|
| T-116-13 (cross-authorization-server credential reuse) | mitigate | the key includes the issuer, so a different AS is a miss BY CONSTRUCTION; `two_keys_differing_only_in_issuer_are_distinct` + `load_with_a_different_issuer_is_a_miss`, both of which SURVIVED the server-collapse break, proving they are their own detector |
| T-116-13a (cross-MCP-server reuse under a shared AS) | mitigate | D-116-R1's third component; asserted on the live, migration AND trait paths, all three OBSERVED failing under the two-part-key break |
| T-116-13b (logout on one server deleting another's) | mitigate | `delete_by_server` operates on the key's `server` component and returns an exact count; `delete_by_server_removes_only_that_server_and_returns_the_count` asserts `2` from a three-credential two-server fixture and names the survivor |
| T-116-15 (tokens leaked through a derived `Debug`) | mitigate | manual `Debug`; `debug_on_stored_credentials_redacts_both_tokens` OBSERVED failing under a leaking `Debug` while `debug_distinguishes_a_present_refresh_token_from_an_absent_one` still passed — two independent detectors. Sentinels chosen not to be substrings of any field name, per 116-02's recorded collision |
| T-116-16 (schema-1 entry re-keyed by guessing) | mitigate | unkeyable entries are DROPPED and reported; OBSERVED failing under a guess-an-issuer break |
| T-116-16a (parser panic on hostile bytes) | mitigate | `parse_credential_snapshot` is total and free of `unwrap`/`expect`/indexing (grep-asserted); two proptest blocks (arbitrary bytes + mutated/truncated valid documents); refusals echo no input (planted-canary absence assertion) |
| T-116-16b (a migration silently dropping a login) | mitigate | `MigrationReport` carries a count and a per-entry dropped list; `take_migration_report` yields once then `None` |
| T-116-18 (a native dep added to the ungated tier) | mitigate | the `wasm32-purity` CI job, wired into the org-required `gate`; plus grep criteria that the module carries no `cfg` other than `cfg(test)` and no forbidden import |
| T-116-SC (cargo installs) | mitigate | zero packages added; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0** |

At-rest encryption remains explicitly out of scope (the platform uses KMS; a plaintext file is
the status quo this phase does not change). File permissions, atomicity and concurrent-write
safety are 116-16's register.

## Known Stubs

None. Every public item is fully implemented and exercised; nothing returns a placeholder, an
empty collection or a "not available" string. `CredentialStore::last_issuer` and `record_issuer`
have defaults that return `Ok(None)` / `Ok(())`, but those are **specified** behaviour for an
implementor that declines D-18's tracking, documented as such, and asserted by
`a_minimal_implementor_gets_working_defaults` — including the assertion that the default
`record_issuer` does **not** pretend to have stored anything.

## TDD Gate Compliance

Tasks 1 and 2 carry `tdd="true"`; Task 3 is a CI-configuration task and does not.

**RED was observed and logged before any implementation existed:**
`target/116-verify/116-05-task1.RED.log` — 3 × `E0432` (`unresolved import
pmcp::shared::credential_store`, and the two crate-root import lists), exit **101**, produced
from a tree in which `tests/oauth_credential_store.rs` existed and the module did not.

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01`
(`ea1d2d68`), `116-02`, `116-03` and `116-04`: in Rust a test naming a non-existent function
fails to *compile*, so such a commit leaves a non-building tree that breaks `git bisect` and
contradicts CLAUDE.md's "ZERO TOLERANCE FOR DEFECTS". A verifier looking for a `test(...)` →
`feat(...)` pair will not find one; the evidence is the RED log above, the negative control
below, and the log paths named in each commit body.

### Negative control (`target/116-verify/116-05.NEGATIVE-CONTROL.log`)

Three deliberate breaks applied **at once**, run with `--no-fail-fast`:
`54 tests run: 37 passed, 17 failed`.

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| the key's `server` component collapsed — literally the old two-part `(issuer, account)` design | `two_keys_differing_only_in_server_are_distinct` (live), `two_schema_1_servers_sharing_one_issuer_stay_independent` (**migration**), `load_with_a_different_server_is_a_miss` (**trait**), plus `get_on_a_key_differing_in_any_component_returns_none`, `keys_for_server_…`, `keys_reflects_insertions_and_removals`, `remove_returns_true_then_false`, `delete_removes_the_entry_…`, `delete_by_server_removes_only_that_server_…`, `delete_by_server_for_an_unknown_server_…`, `list_keys_is_empty_then_…`, `clear_all_returns_the_total_…`, `the_schema_1_map_key_becomes_the_server_component`, `a_schema_1_document_migrates_every_entry_…` | `two_keys_differing_only_in_issuer_are_distinct`, `two_keys_differing_only_in_account_are_distinct`, `load_with_a_different_issuer_is_a_miss`, `load_with_a_different_account_is_a_miss` — the issuer and account detectors are **independent** of the server detector |
| an unkeyable schema-1 entry assigned `https://unknown.example` instead of being dropped | `a_schema_1_entry_without_an_issuer_is_dropped_and_reported`, `take_migration_report_yields_once_then_none` | `a_schema_1_document_with_no_entries_…`, `an_unknown_future_schema_version_…`, `corrupt_bytes_are_an_error_that_echoes_no_input`, `empty_input_is_an_error_…` — four other parser rows, all unaffected |
| `Debug` prints the real access token | `debug_on_stored_credentials_redacts_both_tokens` **only** | `debug_distinguishes_a_present_refresh_token_from_an_absent_one` still passed — the redaction test is its own independent detector, not a side effect of the presence test |

Source restored byte-for-byte afterwards: `shasum -a 256 -c` → **OK** (both files).

## Gate Results

| Gate | Command | Result |
|---|---|---|
| suite (gated) | `cargo nextest run --features full,oauth -E 'binary(oauth_credential_store)'` | **54 run, 54 passed** |
| suite (**UNGATED proof**) | `cargo nextest run --features full -E 'binary(oauth_credential_store)'` | **54 run, 54 passed** |
| Task 1 suite, both sets | same, at `d03e6be4` | **37 run, 37 passed** ×2 |
| doctests | `cargo test --features full,oauth --doc credential_store` | **9 passed** (incl. 1 `compile_fail`) |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, 92 warnings (= 116-BASELINES anchor), **0** naming this file |
| wasm32 via Makefile | `/usr/bin/make wasm-build` | **exit 0** |
| CI fence wiring | `python3 -c "yaml.safe_load(...)"` | 9 jobs; `gate.needs` = `[test, quality-gate, purity-check, pmcp-agent-targets, wasm32-purity]` — a **strict superset** of its `b2bf9157` value |
| lint (**authoritative**, D-116-LINT) | `/usr/bin/make lint` | **✓ No lint issues** (Task 1 and Task 2) |
| fmt | `cargo fmt --all -- --check` | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** (Task 1 and Task 2) |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** attributable, **first-pass** |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| no `cfg` gates | `grep -n 'cfg(' src/shared/credential_store.rs` | **1 hit**, `#[cfg(test)]` |
| no forbidden imports | `grep -n 'reqwest\|webbrowser\|dirs::\|std::fs\|tokio::fs\|std::env'` | **no output** — including no prose hits, unlike 116-02 |
| no public fields | `grep -n 'pub access_token\|pub refresh_token\|pub issuer\|pub account\|pub server'` | **no output** |
| no unwrap/expect | `grep -n 'unwrap()\|expect('` | **no output** (whole file, not just outside `cfg(test)`) |
| no refresh on the seam | `grep -n 'async fn refresh'` | **no output** |
| three-arg key constructor | `grep -n 'pub fn new'` | `:149 pub fn new<I, A, S>(issuer: I, account: A, server: S) -> Self` |
| trait shape | `sed -n '/^pub trait CredentialStore:/,/^}/p' \| grep 'async fn'` | **6** methods, **3** with default bodies |
| admin trait shape | `sed -n '/^pub trait CredentialStoreAdmin:/,/^}/p' \| grep 'async fn'` | **4** methods, **0** defaulted |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over both new files | **no output** |
| package gate | `/usr/bin/make pmcp-package-gate` | exit **0** |
| audit | `/usr/bin/make audit` | exit **0** |
| unused deps | `/usr/bin/make unused-deps` | exit **0** |
| SATD gate | `/usr/bin/make check-todos` | exit **0** |
| unwraps | `/usr/bin/make check-unwraps` | exit **0** |
| purity | `/usr/bin/make purity-check` | exit **0** |
| comply | `/usr/bin/make comply` | exit **0** |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 2 — `test-unit` only**; `1836 passed; 13 failed`, all 13 at `streamable_http.rs:458`, total moved by exactly this plan's 5 inline tests (**D-116-KEYCHAIN**) |

## User Setup Required

None. No external service, no credential, no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`, none fixed here:

- **`D-116-FAILFAST` (new)** — `cargo nextest run` fail-fast is on by default and truncates a
  negative control into a plausible-looking partial partition (`15/54 tests run: 10 passed,
  5 failed`). Use `--no-fail-fast` and assert the denominator. Composes badly with 116-01's
  selector trap. Proposed owner: informational; `116-15` may fold it into the phase's conventions.
- **`D-116-KEYCHAIN`** — unchanged, re-measured here with the arithmetic above. Proposed owner:
  `116-15`.
- **`D-116-DISK`** — hit **twice** in this plan; the volume went 25 GiB → 132 Mi across roughly
  six full-feature builds. Guidance confirmed, no fix.
- **`D-116-LINT`** — reconfirmed with two more measurements (5 total across 3 plans).
- **`D-116-EX`** — still open. This plan's 9 doctests do **not** discharge it, for the same reason
  116-02's 5, 116-03's 3 and 116-04's 7 did not: they are not `cargo run --example`.
- **`D-116-DOC`** — applied as amended, both halves, with zero new errors. No further action.

## Next Phase Readiness

**Waves 4+ are unblocked on this plan's contract.** Every symbol the downstream plans name now
exists, is public, is documented and is tested:

| Consumer | What it can now rely on |
|---|---|
| `116-08` | `parse_credential_snapshot(&[u8]) -> Result<(CredentialSnapshot, MigrationReport)>` is I/O-free and total; a `libfuzzer` target needs no harness at all. Two proptest blocks already cover arbitrary and mutated-document inputs |
| `116-11` | `OAuthHelper::with_credential_store` takes `Arc<dyn CredentialStore>`; `save_with_issuer` is the one call that stores credentials and the D-18 issuer record together |
| `116-12` | `StoredCredentials::client_id()` (D-14 defect 2 — DCR flows can now source a client id from the store) and `granted_scopes()` (D-14 defect 3 — the scope to send on refresh) |
| `116-13` | all four administrative operations exist on `CredentialStoreAdmin` with the exact `auth logout` semantics asserted; `normalize_server_key` is the ported `normalize_cache_key`; `StoredCredentials`' serde names match `TokenCacheEntry` field-for-field so `oauth-cache.json` migrates without data loss |
| `116-16` | `parse_credential_snapshot` + `CredentialSnapshot::to_bytes` are the WHOLE format and migration, so `FileCredentialStore` reduces to lock + read + parse + mutate + serialize + atomic write. Declare it in `src/shared/credential_file.rs`, which the CI comment already names as the gated counterpart |
| `116-15` | must NOT cite `make quality-gate` exit 0 for this HEAD (D-116-KEYCHAIN); every other stage is green and individually cited above |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-16` | do NOT move the format or the migration into the file impl — that would delete D-06/D-07's whole point and un-fuzz the parser |
| `116-13` | surface `take_migration_report()`'s `DroppedEntry` list to the operator; a dropped login is a forced re-login the user must be told about. Also mind the RESEARCH forward-compat trap: an older installed `cargo-pmcp` hard-errors on `schema_version: 2` |
| every source-touching plan | run `make lint`, not clause (b) alone (`D-116-LINT`, now 5× measured); run negative controls with `--no-fail-fast` (`D-116-FAILFAST`) |
| `116-15` | resolve `D-116-KEYCHAIN`; close or waive `D-116-EX`; do not book `AUTH-03` on this plan's evidence alone |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: src/shared/credential_store.rs                             (1064 lines, min_lines 380 ✓)
FOUND: tests/oauth_credential_store.rs                            (1041 lines, min_lines 200 ✓)
FOUND: .github/workflows/ci.yml                                   (461 lines, +58/-3)
FOUND: Makefile                                                   (1134 lines, +8)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md (278 lines, was 237)
```

Commits claimed, verified in `git log`:

```
FOUND: d03e6be4  feat(116-05): three-part credential key, record and schema 1 to 2 migration
FOUND: ec80e5b1  feat(116-05): CredentialStore seam, CredentialStoreAdmin and the in-memory impl
FOUND: 34b67482  ci(116-05): wasm32 build fence so the ungated OAuth tier cannot regress
```

`must_haves` verification:

```
✓ truths[1] addressed by (issuer, account, server) — neither a different AS nor a different MCP
  server sharing that AS can reach another's credentials; asserted on the live, migration AND
  trait paths, each OBSERVED failing under the two-part-key break while the issuer and account
  detectors held
✓ truths[2] a platform can implement the store without the SDK dictating identity, a filesystem
  or an environment-touching constructor — grep: no std::env, no dirs::, no std::fs, no
  tokio::fs, no reqwest; account scope stored verbatim (5 hostile shapes + a property)
✓ truths[3] format, migration and report are pure, wasm-clean and fuzzable — wasm32 build exit 0
  with 0 warnings naming this file; parse_credential_snapshot is total, unwrap-free and covered
  by two proptest blocks
✓ truths[4] enumeration, delete-by-server, clear-all with a count and a migration report exist on
  a DECLARED trait — CredentialStoreAdmin, 4 methods, none defaulted, all four asserted with
  EXACT counts against InMemoryCredentialStore
✓ truths[5] a CI gate fails if the ungated tier stops compiling for wasm32 — wasm32-purity job in
  gate.needs, verified by parsing the YAML
✓ artifacts: src/shared/credential_store.rs 1064 >= 380 and contains "pub trait
  CredentialStoreAdmin" (:861); provides all 11 named symbols
✓ artifacts: tests/oauth_credential_store.rs 1041 >= 200 — key-shape (incl. the two-servers-one-
  issuer collision), round-trip, admin-op, migration and property coverage
✓ key_links: "parse_credential_snapshot" present in src/shared/credential_store.rs (5 refs) —
  the format + migration 116-16's file impl will wrap with I/O only
✓ key_links: "wasm32-unknown-unknown" present in .github/workflows/ci.yml (5 refs, 2 in the new
  job) — the build fence over the ungated tier
```

Plan-level verification block:

```
✓ 54 run / 54 passed under --features full,oauth (non-zero count)
✓ 54 run / 54 passed under --features full alone — the tier is genuinely ungated
✓ make wasm-build exit 0; the CI job is wired into gate's needs:
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable, on the FIRST pass
⚠ make quality-gate — exit 2 at test-unit ONLY: 1836 passed / 13 failed, all 13 in
  shared::streamable_http at the same pre-existing streamable_http.rs:458 .expect(), total moved
  by exactly this plan's 5 new inline tests (1844 -> 1849). D-116-KEYCHAIN, unchanged.
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-03*
