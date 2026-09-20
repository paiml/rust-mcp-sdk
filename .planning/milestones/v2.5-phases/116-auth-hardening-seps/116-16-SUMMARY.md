---
phase: 116-auth-hardening-seps
plan: 16
subsystem: auth
tags: [oauth, sep-2352, credential-storage, filesystem, concurrency, atomicity, migration, tracing, wasm32]

# Dependency graph
requires:
  - phase: 116-auth-hardening-seps
    plan: 05
    provides: "the WHOLE format and migration as pure code (parse_credential_snapshot / CredentialSnapshot::to_bytes), both traits, and the binding instruction NOT to move either into the file impl"
  - phase: 116-auth-hardening-seps
    plan: 06
    provides: "the measured proof that D-116-KEYCHAIN is an environment artifact, so a red gate here would have been a real signal — and the clean-volume 1865/0 unit-test anchor this plan reproduced exactly"
provides:
  - "FileCredentialStore — the DEFAULT on-disk store, implementing CredentialStore AND CredentialStoreAdmin, that knows nothing about the JSON"
  - "A serialized read-modify-write (with_snapshot_mut) as the unit of work for EVERY mutation, so a lost update is prevented rather than merely a torn file"
  - "save_with_issuer overridden as ONE write under ONE lock, closing the window where the store names one issuer while holding another's credentials"
  - "An O_EXCL advisory lock with a documented 30s staleness break, adding ZERO dependencies"
  - "default_credential_path — the only `dirs` caller, a FREE function so construction stays I/O-free"
  - "CREDENTIAL_WRITE_EVENT_TARGET — one tracing DEBUG event per atomic write, which is what makes 'exactly one write' assertable at all"
  - "D-116-LINT-OAUTH — the measured finding that `make lint` compiles NONE of this phase's oauth-gated code, and that src/client/oauth.rs carries 29 pre-existing errors under the gate-equivalent command"
  - "The measured finding that a tokio::join! over two saves is NOT a lost-update detector"
affects: [116-11, 116-13, 116-15, 116-10, 116-12]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A serialized read-modify-write as the unit of work: an atomic rename and a lost update are DIFFERENT threats, and only the second needs a lock"
    - "Two lock layers with different jobs: tokio::sync::Mutex for tasks in one process, an O_EXCL file for processes — the second is the one the cross-instance test exercises"
    - "A cross-process lock built from std::fs::OpenOptions::create_new, so the dependency delta stays empty and the primitive is one every supported platform already guarantees"
    - "Write only when the closure actually changed the document, which byte-stable serialization makes exact — a no-op mutation neither creates nor churns the file"
    - "Emit the invariant you need to test: 'exactly one write' is invisible on the filesystem, so the store emits a tracing event and the test counts it — measuring the two-call baseline FIRST so the counter is proven able to see two"
    - "Widen a private helper to pub(crate), never to pub, when a gated sibling must share semantics rather than reimplement them"

key-files:
  created:
    - src/shared/credential_file.rs
    - tests/oauth_credential_file.rs
  modified:
    - src/shared/mod.rs
    - src/lib.rs
    - src/shared/credential_store.rs
    - .planning/phases/116-auth-hardening-seps/deferred-items.md

key-decisions:
  - "A SEPARATE gated module, not a gated half of credential_store.rs — measured: the pure tier still carries exactly 1 `cfg(`, and it is the `cfg(test)`"
  - "write_atomic ports cargo-pmcp's SEQUENCE but creates its temporary with OpenOptions::create_new rather than the tempfile crate, because tempfile is dev-only in pmcp and the phase's Cargo.toml must stay byte-identical to b2bf9157"
  - "CREDENTIAL_WRITE_EVENT_TARGET added (Rule 2): the plan's same-bytes criterion does NOT fail when the save_with_issuer override is deleted, so the override was untested until a write counter existed"
  - "CredentialSnapshot::forget_issuer widened private -> pub(crate) so delete_by_server shares InMemoryCredentialStore's logout semantics instead of forking them"
  - "The lock WAIT limit (45s) deliberately exceeds the STALENESS window (30s), so a crashed process's lock is always broken within one call"
  - "The rustdoc names CREDENTIAL_SCHEMA_VERSION and 'schema version' rather than the literal identifier, because two of the plan's own acceptance criteria contradict each other on that string — see Deviations"
  - "AUTH-03 is NOT booked complete: 116-07/09/10/11/12/13 all still claim it"

patterns-established:
  - "Run the gate-equivalent clippy command with `full,oauth` as well as `make lint`: `full` does not contain `oauth`, so the authoritative gate lints none of this phase's gated code (D-116-LINT-OAUTH)"
  - "A negative control's value is in the SURVIVORS: two of this plan's five breaks were caught only by tests added because the plan's own suggested assertions were measured NOT to detect them"

requirements-completed: []

# Metrics
duration: 215min
completed: 2026-08-04
---

# Phase 116 Plan 16: The Default On-Disk Credential Store Summary

**Every credential mutation is now ONE serialized read-modify-write — take the lock, read, mutate,
write atomically, release — so two writers can no longer discard one another's credentials, which
is a threat an atomic rename never addressed. `save_with_issuer` is one write under one lock,
proven by a counter rather than inferred. The store knows nothing about the JSON: a grep for
`serde::`, any `Serialize`/`Deserialize` derive, or the schema-version identifier returns nothing,
and 116-05's pure tier still carries exactly one `cfg(` — the `cfg(test)`. Zero dependencies were
added; the lock is `OpenOptions::create_new`. And `make quality-gate` exits 0 — the first time in
this phase.**

## Performance

- **Duration:** ~215 min
- **Completed:** 2026-08-04
- **Tasks:** 1
- **Files:** 5 (2 created, 3 modified), **+1787 / −1**, **0 removed**

## Accomplishments

- **The lost update is prevented, and the test that proves it is not the one the plan
  specified.** The plan asked for a `tokio::join!` over two saves on one store. Under the
  read-before-lock break, that test **still PASSED** — a single task has no await point between
  its read and its write, so `join!` cannot interleave them. The deterministic detector added
  instead, `a_waiter_reads_the_document_the_lock_holder_left_behind`, holds the lock externally,
  starts a save, asserts it is still pending, then writes the *holder's* document and releases:
  a store that read before it locked necessarily writes back a snapshot without the holder's
  credential. That test and `two_instances_over_one_path_saving_concurrently_both_survive` are the
  two that failed under the break. **A `join!` over two saves is not a lost-update detector**, and
  that is now measured rather than argued.

- **"Exactly one write" was unobservable, so the store was given something to observe.** The
  plan offered "a same-bytes check against a two-call baseline" as an alternative to counting
  writes. Measured: deleting the `save_with_issuer` override leaves
  `save_with_issuer_is_one_write_that_makes_both_observable` **passing**, because the default's two
  writes produce a byte-identical file. The override was therefore untested. A `tracing` DEBUG
  event per completed atomic write (`CREDENTIAL_WRITE_EVENT_TARGET`) plus a counting `Subscriber`
  in the suite closes it — and the test measures the two-call baseline as **2** before asserting
  **1**, so a counter that saw nothing could not pass vacuously. It is the only test the
  override-deletion break fails.

- **The format/I-O split held under grep, not just under intent.** `grep -n
  'serde::\|#\[derive(.*Serialize\|#\[derive(.*Deserialize\|schema_version'` over
  `src/shared/credential_file.rs` returns **nothing**. `grep -c 'cfg('` over
  `src/shared/credential_store.rs` still returns **1**, and it is the `cfg(test)` — so 116-05's
  fence survived the one edit this plan made to that file (a private → `pub(crate)` visibility
  widening, no new public surface, `semver-checks` **223 pass / 0 fail**). The wasm32 build exits
  **0** with **92** warnings — exactly the `116-BASELINES` anchor — and **zero** of them name
  `credential_file`.

- **`make quality-gate` exits 0, for the first time in Phase 116.** `1865 passed; 0 failed` at
  `test-unit` — the *identical* number `116-06` measured on a clean volume, because `make lint` and
  `make test-unit` run `--features "full"` and `full` does not contain `oauth`, so this plan's five
  inline tests are not compiled there. `445 passed; 0 failed; 79 ignored` at `test-doc`. Disk went
  56 GiB → 49 GiB across the whole plan; `df -h /` was run before and after every long build, per
  `D-116-DISK`.

- **`D-116-LINT` has a third, worse shape — and it is the one the remaining plans will hit.**
  `make lint` runs `--features "full"`. `full` does not contain `oauth`. So the authoritative
  clippy gate compiles **none** of `src/client/oauth.rs`, none of `src/shared/credential_file.rs`,
  and none of what `116-10`/`116-12`/`116-13` will add. Running `make lint`'s command verbatim with
  `full,oauth` substituted — same `RUSTFLAGS="-D warnings"`, same 28-entry `-A` list, same lint
  groups — exits **101** with **29 errors, all 29 in `src/client/oauth.rs`** and **0** in any file
  this plan touched. Clause (b) *does* enable `oauth` but omits `-D warnings`, so it reports those
  same 29 as warnings and exits 0. **The union of the two documented commands is green on 29 hard
  errors.** Logged as `D-116-LINT-OAUTH`.

## Task Commits

| # | Task | Commit | Type |
|---|---|---|---|
| 1 | `FileCredentialStore` with atomic 0o600 writes and a serialized read-modify-write | `2d769409` | feat |

## Files Created/Modified

- **`src/shared/credential_file.rs`** (**created**, **690** lines — `min_lines` 260 ✓). Gated
  `#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]`. Public:
  `FileCredentialStore` (with `new`, `path`, `lock_path`), `default_credential_path`,
  `CREDENTIAL_LOCK_SUFFIX`, `CREDENTIAL_LOCK_STALE_SECS`, `CREDENTIAL_WRITE_EVENT_TARGET`.
  Private: `with_snapshot_mut`, `read_snapshot`, `unreadable`, `unusable`, `write_atomic`,
  `temporary_sibling`, `create_private_dir`, `open_exclusive`, `create_private_file`,
  `restrict_file`, `io_failure`, `RemoveOnDrop`, `acquire_lock`, `break_stale_lock`, and the
  `LOCK_POLL_INTERVAL` / `LOCK_WAIT_LIMIT` / `PRIVATE_FILE_MODE` / `PRIVATE_DIR_MODE` constants.
  5 inline tests, 2 doctests. 7 `cfg(` sites: 5 × `cfg(unix)`, 1 × `cfg(not(unix))`,
  1 × `cfg(test)`.
- **`tests/oauth_credential_file.rs`** (**created**, **1069** lines — `min_lines` 180 ✓).
  **29 tests** in six documented groups, plus the `WriteCounter` `tracing::Subscriber`.
  `#![cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]` — the exact inverse of
  `oauth_credential_store.rs`'s deliberate ungatedness, and the reason `make quality-gate`'s
  `--features full` run reports zero from this binary rather than failing to compile.
- **`src/shared/mod.rs`** (+12) — the gated `pub mod credential_file;` with the load-bearing
  "separate module on purpose" rationale naming the pure counterpart.
- **`src/lib.rs`** (+7) — gated crate-root re-export of `FileCredentialStore` and
  `default_credential_path`. The three constants stay module-path-only, per the plan's
  `<interfaces>`.
- **`src/shared/credential_store.rs`** (+9/−1) — `forget_issuer` private → `pub(crate)`, with the
  reason in place. **No other change**; `cfg(` count still **1**.
- **`.planning/phases/116-auth-hardening-seps/deferred-items.md`** (377 → 434) — `D-116-TRIPWIRE`
  closed by measurement, `D-116-LINT-OAUTH` added.

## Decisions Made

- **A separate gated module, exactly as 116-05 instructed.** The format, the schema 1 → 2
  migration and the `MigrationReport` were NOT moved. This module calls
  `parse_credential_snapshot` and `CredentialSnapshot::to_bytes` and knows nothing else; the grep
  criterion above is the mechanical proof, and the pure tier's `cfg(` count of 1 is the proof that
  the fence did not move either.

- **`OpenOptions::create_new` instead of the `tempfile` crate — a deliberate, recorded departure
  from "port `write_atomic`, do not rewrite it".** `tempfile` is a `[dev-dependencies]` entry in
  `pmcp` (`Cargo.toml:182`), not a runtime one, so porting the line literally would have required
  adding an optional dependency — and `116-BASELINES` § 6 states the phase's dependency fence as
  `git diff --exit-code b2bf9157..HEAD -- Cargo.toml Cargo.lock` exiting **0**, which `116-05` also
  measured and this plan's own `T-116-SC` restates as "zero packages added". The SEQUENCE is
  ported verbatim — `create_dir_all` → parent `0o700` → same-directory temporary → file `0o600` →
  rename — and `cargo-pmcp`'s `write_sets_0600_perms_on_unix` is carried across as
  `save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates`, deliberately over a directory
  **this store creates** so the `0o700` half is not vacuous against an already-0o700 tempdir.
  `RemoveOnDrop` supplies `NamedTempFile`'s cleanup-on-failure property, and the same type serves
  the lock, so a `?` on the way out cannot leak either file. `git diff --exit-code
  b2bf9157..HEAD -- Cargo.toml` exits **0**.

- **The wait limit (45s) exceeds the staleness window (30s), on purpose.** With the reverse
  ordering a lock abandoned five seconds ago could never be broken within a single call — the
  caller would always time out first and the user would be told to retry. An inline test asserts
  the ordering so nobody "tidies" the two constants into agreement.

- **`load`, `list_keys` and `last_issuer` take no lock.** A document that was renamed into place is
  already consistent, so queuing readers behind a writer would buy nothing and would let one wedged
  writer stall every reader. Asserted by `a_load_succeeds_while_a_lock_file_exists`, which
  exercises all three while a lock file exists.

- **The advisory lock's two limits are stated in the rustdoc, not implied away.** It is
  cooperative — another program writing the file directly still clobbers it — and a process that
  stalls past 30 seconds can have its lock broken under it. Both are acceptable for a per-user file
  on a developer machine and **neither** would be acceptable for a multi-writer server, which is
  named as the reason `CredentialStore` is a trait. (`T-116-17d`.)

- **`forget_issuer` widened to `pub(crate)` (Rule 3 — blocking).** 116-05 decided that
  `delete_by_server` must also forget the server's last-seen issuer, and implemented it in
  `InMemoryCredentialStore` using a *private* helper. A file store in a different module could not
  reach it, so the alternatives were: diverge (a per-server logout leaves behind a record of which
  authorization server the user visited — the disclosure 116-05 deliberately closed), or
  reimplement (two implementations of one semantic, which is what the pure tier exists to prevent).
  `pub(crate)` adds no public surface: `semver-checks` reports **223 pass / 0 fail**.

- **The rustdoc names `CREDENTIAL_SCHEMA_VERSION`, not the literal `schema_version`.** Two of the
  plan's acceptance criteria are in direct conflict on this string — see *Deviations*.

- **RED was OBSERVED and logged, not COMMITTED as a broken build**, for the fifth time in this
  phase and for the same reason. See *TDD Gate Compliance*.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing critical functionality] The `save_with_issuer` override was untested, and the plan's own criterion could not detect that**

- **Found during:** Task 1, designing the negative control.
- **Issue:** The plan's acceptance criterion offers "a same-bytes check against a two-call
  baseline" as evidence that `save_with_issuer` is ONE write. It is not evidence: the trait's
  default (`save` then `record_issuer`) produces a **byte-identical** file, because
  `CredentialSnapshot::to_bytes` is byte-stable. Measured in the negative control — with the
  override deleted, `save_with_issuer_is_one_write_that_makes_both_observable` **PASSED**. The
  atomicity `T-116-17b` claims to mitigate had no detector.
- **Fix:** added `CREDENTIAL_WRITE_EVENT_TARGET` and one `tracing::debug!` per completed atomic
  write in `write_atomic`, plus a counting `tracing::Subscriber` in the suite installed with
  `tracing::dispatcher::set_default`. `save_with_issuer_writes_the_file_exactly_once` asserts
  **1** for the combined call and then measures the two-call form as **2** in the same test, so a
  counter that saw nothing could not pass vacuously. A second test,
  `a_mutation_that_changes_nothing_does_not_write`, uses the same counter to assert **0** writes
  for an identical save, an absent-key delete and an unknown-server logout.
- **Why this is Rule 2 rather than a nicety:** the event is also the operator-facing affordance
  (`RUST_LOG=pmcp::credential_file::write=debug`) and carries the path and byte count and **never**
  file content.
- **Committed in:** `2d769409`.

**2. [Rule 2 — Missing critical functionality] The specified concurrency test cannot fail under the bug it names**

- **Found during:** Task 1, negative control.
- **Issue:** the plan specifies driving the same-process case with `tokio::join!` over two `save`
  futures. Both futures run on ONE task, and there is no await point between a read and its write,
  so they cannot interleave — the test passes whether or not the read-modify-write is serialized.
  Measured: `two_concurrent_saves_on_one_store_both_survive` **PASSED** under the read-before-lock
  break.
- **Fix:** the `join!` test is kept exactly as specified, and
  `a_waiter_reads_the_document_the_lock_holder_left_behind` was added as the deterministic
  detector — the lock is held externally while a save is spawned, the save is asserted PENDING,
  the holder's document is then written and the lock released, and the final `list_keys()` must
  contain both keys. A store that read before it locked writes back a snapshot missing the
  holder's credential, every time. The cross-instance test was also promoted to
  `flavor = "multi_thread"` with real `tokio::spawn` parallelism, and it too failed under the
  break.
- **Committed in:** `2d769409`.

**3. [Rule 3 — Blocking] `tempfile` is dev-only in `pmcp`, so `write_atomic` could not be ported line-for-line**

Described under *Decisions Made*. The sequence, the permission bits and the ported unit test are
all carried across; only the temp-file creation primitive differs, and it is the same
`create_new` primitive the advisory lock rests on.

**4. [Rule 3 — Blocking] `CredentialSnapshot::forget_issuer` was private**

Described under *Decisions Made*.

**5. [Rule 1 — Bug] Two of the plan's acceptance criteria contradict each other on the string `schema_version`**

- **Issue:** criterion 2 requires that
  `grep -n '…\|schema_version' src/shared/credential_file.rs` return **nothing**; criterion 12
  requires that `FileCredentialStore`'s rustdoc "contain the forward-compat trap note naming
  `cargo-pmcp` and `schema_version`". Both cannot hold literally.
- **Resolution:** the machine-checkable one was satisfied exactly (criterion 2 returns nothing —
  its intent is that **no code** here knows the JSON), and criterion 12 was satisfied in substance:
  the rustdoc names `cargo-pmcp`, names the version through an intra-doc link to the public
  identifier
  [`CREDENTIAL_SCHEMA_VERSION`](crate::shared::credential_store::CREDENTIAL_SCHEMA_VERSION), and
  states the behaviour precisely — an installed `cargo-pmcp` 0.18.0 hard-errors on any document
  whose schema version is not the one it knows, its message already says to upgrade, and nothing
  in this repository can change an installed binary.
- **For `116-15`:** the criterion pair should be reconciled in favour of the grep, since it is the
  one a verifier can run.

**Total deviations:** 5 (1 × Rule 1, 2 × Rule 2, 2 × Rule 3). No Rule 4 situation arose; no
architectural change was needed. **Zero dependencies added** — `git diff --exit-code
b2bf9157..HEAD -- Cargo.toml` exits **0**, discharging `T-116-SC`.

### Non-deviation worth recording

`make lint` passed on the FIRST attempt for this plan's code, which is a first in this phase — but
only because `make lint` never compiles it (`D-116-LINT-OAUTH`). The gate-equivalent
`full,oauth` run is the one that actually covered `credential_file.rs`, and it too reported zero
attributable errors on the first pass.

## Issues Encountered

- **`gsd-sdk query state record-metric` rejects every argument form tried** — positional
  (`state.record-metric 116 16 215 1 5`), space-separated subcommand
  (`state record-metric 116 16 215min 1 5`), named flags, and a combined `116-16` id all return
  `{"error": "phase, plan, and duration required"}`, while `state record-session` and
  `state advance-plan` work in the space-separated form and `state add-decision` needs
  `--summary`. The Performance Metrics row was added to `STATE.md` directly. `add-decision`'s
  output is also missing its phase (`- [Phase ?]:`), which was corrected in place.
- **`roadmap update-plan-progress 116` does not tick a plan's checkbox**; it reports
  `summary_count` and leaves the list alone. `116-16`'s `- [ ]` was flipped to `- [x]` directly.
- **The `126`-line credential-file suite reports one `LEAK` under nextest** on some runs
  (`a_schema_1_entry_with_no_issuer_is_dropped_and_reported`). nextest counts a leaky test as a
  PASS; the cause is a tokio worker thread outliving the 100 ms grace window, not an open file
  handle the store failed to close — the suite asserts leftover-file absence explicitly in
  `a_save_leaves_neither_a_temporary_nor_a_lock_behind`.
- **`cargo semver-checks` reports "no semver update required"** for the fifth plan running. The
  requirement (*zero breaking findings*) is met — **223 checks, 223 pass, 0 fail**, exit 0 — but
  note the tool cannot see any of this plan's public items at all: the default feature set does not
  include `oauth`. `116-13` must not rest its version-bump reasoning on this verdict.
- **Both halves of `D-116-DOC` were needed and both applied cleanly on the first pass.** The
  inner `//!` block fully qualifies its four intra-doc links (`make doc-check` confirms the merged
  doc resolves in the DECLARING module's scope — the module doctest is reported as
  `src/shared/mod.rs - shared::credential_file (line 50)`), while every `///` item doc uses the
  bare form. `make doc-check` `^error` count: **28** — exactly the anchor — with **0** hits for
  `credential_file`.
- **No `.proptest-regressions` file was generated**; this plan adds no proptest block. The
  property coverage for the credential tier lives in `116-05`'s pure suite, which is where the
  fuzzable code is — this module is I/O, and its fence is the negative control below.

## Threat Flags

None. This plan adds no network endpoint, no socket and no schema change at a trust boundary. It
adds FILE access, which is the trust boundary its own register was written for.

| Threat | Disposition | Discharged by |
|---|---|---|
| T-116-14 (credential file readable by other local users) | mitigate | ported `write_atomic`: temp-in-same-dir, file `0o600`, parent `0o700`, created with `mode(0o600)` AND re-restricted before the rename so a generous umask cannot widen it. `save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates` (over a directory THIS store creates) and `a_pre_existing_loose_file_is_tightened_by_the_next_save`, both OBSERVED failing under a `0o644` break |
| T-116-17 (torn file after a crash mid-write) | mitigate | same-directory temporary + `fs::rename`; `a_save_leaves_neither_a_temporary_nor_a_lock_behind` asserts the directory holds exactly one entry afterwards. A corrupt file returns an actionable refusal naming the path and saying to delete it, and reproduces **no** content — asserted with a planted canary |
| T-116-17a (LOST UPDATE) | mitigate | every mutation through `with_snapshot_mut`: `tokio::sync::Mutex` + `O_EXCL` lock, read INSIDE the lock. `a_waiter_reads_the_document_the_lock_holder_left_behind` and `two_instances_over_one_path_saving_concurrently_both_survive` both OBSERVED failing under a read-before-lock break — while the `join!` test did NOT, which is why the first one exists |
| T-116-17b (store naming one issuer while holding another's credentials) | mitigate | `save_with_issuer` overridden as one `with_snapshot_mut` closure. `save_with_issuer_writes_the_file_exactly_once` OBSERVED failing under override deletion; the same-bytes test did NOT, which is why the write counter exists |
| T-116-17c (a crashed process wedging the store) | mitigate | locks at least `CREDENTIAL_LOCK_STALE_SECS` (30) old are broken with a `tracing::warn!` naming the lock and its age; `a_stale_lock_is_broken_so_a_crash_cannot_wedge_the_store` forces an old mtime via `std::fs::FileTimes` and asserts both the elapsed bound and the lock's removal. The 45s wait limit exceeds the 30s window, asserted inline |
| T-116-17d (implying the file store is multi-writer safe) | accept | both limits stated as LIMITS in the rustdoc, with the trait named as the answer for a real multi-writer runtime |
| T-116-52 (installed `cargo-pmcp` 0.18.0 rejecting the new schema version) | accept | recorded as a forward-compatibility-trap section in `FileCredentialStore`'s rustdoc; the installed binary's own message already says to upgrade. `116-15` books it as a released-behaviour note |
| T-116-SC (cargo installs) | mitigate | zero packages added; `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` exit **0**; `grep -rnE '^fs2\s*=\|^file-lock\s*=\|^fd-lock\s*=' Cargo.toml` → no hits |

At-rest encryption remains explicitly out of scope (CONTEXT § Deferred): the platform uses KMS, and
a plaintext `~/.pmcp` file is the status quo this phase does not change.

## Known Stubs

None. Every public item is fully implemented and exercised. `restrict_file` has a `cfg(not(unix))`
arm that returns `Ok(())` — that is the SPECIFIED behaviour where the platform has no unix
permission bits, not a placeholder, and the permission tests are correspondingly `cfg(unix)`.

## TDD Gate Compliance

Task 1 carries `tdd="true"`.

**RED was observed and logged before any implementation existed:**
`target/116-verify/116-16-task1.RED.log` — 2 × `E0432` (`unresolved import
pmcp::shared::credential_file`, and `no default_credential_path` / `no FileCredentialStore` in the
root), exit **101**, produced from a tree in which `tests/oauth_credential_file.rs` existed and the
module did not.

**The RED state was NOT committed as a separate `test(...)` commit**, following `116-01` through
`116-05`: in Rust a test naming a non-existent type fails to *compile*, so such a commit leaves a
non-building tree that breaks `git bisect` and contradicts CLAUDE.md's "ZERO TOLERANCE FOR
DEFECTS". A verifier looking for a `test(...)` → `feat(...)` pair will not find one; the evidence
is the RED log above, the negative control below, and the log paths named in the commit body.

### Negative control (`target/116-verify/116-16.NEGATIVE-CONTROL.log`)

Five deliberate breaks applied **at once**, run with `--no-fail-fast`:
**`29 tests run: 20 passed, 9 failed`** — the denominator matches the full suite, so no truncation
(`D-116-FAILFAST`).

| Deliberate break | Tests that FAILED | Siblings that still PASSED (proving attribution) |
|---|---|---|
| **A.** the snapshot read moved BEFORE `acquire_lock` — the classic lost update | `a_waiter_reads_the_document_the_lock_holder_left_behind`, `two_instances_over_one_path_saving_concurrently_both_survive` | **`two_concurrent_saves_on_one_store_both_survive` still PASSED** — the `join!` shape the plan specified is NOT a detector, because one task has no await point between its read and its write. `a_load_succeeds_while_a_lock_file_exists` and `a_stale_lock_is_broken_…` also held, so the lock's other two behaviours are independent |
| **B.** the `save_with_issuer` override deleted, falling back to the two-call trait default | `save_with_issuer_writes_the_file_exactly_once` **only** | **`save_with_issuer_is_one_write_that_makes_both_observable` still PASSED** — the plan's same-bytes criterion cannot see the difference, which is the measurement that justified adding the write counter |
| **C.** files created `0o644` instead of `0o600` | `save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates`, `a_pre_existing_loose_file_is_tightened_by_the_next_save` | `a_save_leaves_neither_a_temporary_nor_a_lock_behind` still passed — atomicity is its own detector, independent of the permission bits |
| **D.** `delete_by_server` no longer forgets the last-seen issuer | `delete_by_server_removes_only_that_server_and_returns_the_count` **only** | `clear_all_returns_the_total_and_empties_the_file` still passed (the full wipe forgets issuers in the pure tier), and `delete_by_server_for_an_unknown_server_returns_zero_and_is_not_an_error` still passed — the count semantics and the issuer semantics are separate detectors |
| **E.** the `snapshot != before` guard removed, so every mutation writes | `a_mutation_that_changes_nothing_does_not_write`, `clear_all_on_a_missing_file_returns_zero_and_creates_no_file`, `deleting_a_key_that_is_not_present_is_not_an_error` | every migration row (`a_schema_1_file_is_read_by_migrating_it_not_by_failing`, `a_migrating_load_does_not_rewrite_the_file_but_the_next_save_does`, `a_schema_1_entry_with_no_issuer_is_dropped_and_reported`, `an_unknown_future_schema_version_…`, `take_migration_report_yields_once_then_none`) still passed — the read path is genuinely independent of the write-suppression rule |

Source restored byte-for-byte afterwards: `shasum -a 256 -c` → **OK** (both files), and the restored
tree re-measured at **29 run / 29 passed**.

## Gate Results

| Gate | Command | Result |
|---|---|---|
| suite | `cargo nextest run --features full,oauth -E 'binary(oauth_credential_file)'` | **29 run, 29 passed**, non-zero count asserted by the plan's `grep -qE` |
| 116-05 unregressed | `… -E 'binary(oauth_credential_store)'` | **54 run, 54 passed** |
| **bounded-reads tripwire** | `… --no-fail-fast -E 'binary(v2_bounded_reads_tripwire)'` | **13 run, 13 passed** — `credential_file.rs` adds **zero** accumulation sites, so no ALLOWLIST entry is owed |
| inline unit tests | `cargo test --lib --features full,oauth credential_file` | **5 passed** |
| doctests | `cargo test --features full,oauth --doc credential_file` | **3 passed** (module + `FileCredentialStore` + `default_credential_path`) |
| wasm32 | `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` | **exit 0**, 92 warnings (= `116-BASELINES` anchor), **0** naming this file |
| lint (authoritative for `full`) | `/usr/bin/make lint` | **✓ No lint issues**, first pass |
| lint (gate-equivalent **with `oauth`**) | `make lint`'s command, `--features "full,oauth"` | exit **101**, 29 errors — **all 29 in `src/client/oauth.rs`**, **0** in any file this plan touched (`D-116-LINT-OAUTH`) |
| fmt | `/usr/bin/make fmt` then `cargo fmt --all -- --check` inside the gate | **exit 0** |
| complexity | `pmat quality-gate --fail-on-violation --checks complexity` | **0 violations** |
| doc-check | `/usr/bin/make doc-check`, `grep -c '^error'` | **28** (= anchor), **0** attributable, **first pass** |
| semver | `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | 223 checks: **223 pass, 0 fail**, exit 0 |
| dependency fence | `git diff --exit-code b2bf9157..HEAD -- Cargo.toml` | **exit 0** |
| no locking crate | `grep -rnE '^fs2\s*=\|^file-lock\s*=\|^fd-lock\s*=' Cargo.toml` | **no hits** |
| **no JSON knowledge** | `grep -n 'serde::\|#\[derive(.*Serialize\|#\[derive(.*Deserialize\|schema_version' src/shared/credential_file.rs` | **no output** |
| pure tier still fenced | `grep -c 'cfg(' src/shared/credential_store.rs` | **1** (the `cfg(test)`) |
| I/O-free constructor | `grep -n 'fn new(' …` + body inspection | `pub fn new(path: PathBuf) -> Self` at `:198`; body has no `dirs`, no `fs::`, no `create_dir_all` |
| free-function default path | `grep -n 'default_credential_path' …` | `:395 pub fn default_credential_path() -> Result<PathBuf>`, module level |
| single mutation path | `grep -c 'with_snapshot_mut' …` | **7** (the private fn + all six mutating methods), criterion is `>= 7` |
| SATD | `grep -nE 'TODO\|FIXME\|HACK\|XXX'` over both new files | **no output** |
| unwrap/expect outside tests | `grep -n 'unwrap()\|expect('` over the module | **1 hit**, inside `#[cfg(test)] mod tests` |
| **FULL gate** | `/usr/bin/make quality-gate` | **exit 0** — `1865 passed; 0 failed` unit, `445 passed; 0 failed; 79 ignored` doctests. **First green gate in Phase 116** |
| disk | `df -h /` before and after | 56 GiB → 49 GiB free, never below 49 GiB (`D-116-DISK` never triggered) |

## User Setup Required

None. No external service, no credential and no package install — this plan installed **zero**
packages, so no package-legitimacy checkpoint applies.

## Deferred Issues

Logged to `.planning/phases/116-auth-hardening-seps/deferred-items.md`:

- **`D-116-LINT-OAUTH` (new)** — `make lint` runs `--features "full"`, and `full` does not contain
  `oauth`, so the authoritative gate lints **none** of this phase's gated code. The
  gate-equivalent command with `full,oauth` exits 101 with **29 pre-existing errors, all in
  `src/client/oauth.rs`** — the file `116-10` and `116-12` will edit. Measure that baseline before
  editing. Proposed owner: `116-15`. Do **not** resolve it by adding `oauth` to `full`.
- **`D-116-TRIPWIRE` — CLOSED by measurement.** `binary(v2_bounded_reads_tripwire)` is **13/13**
  at `5f1474e2` and after this plan added a file to the scanned directory. `credential_file.rs`
  contains no `extend_from_slice(`, no `push_str(`, no `.extend(` and no `.append(`, so no further
  ALLOWLIST entry is owed.
- **`D-116-KEYCHAIN`** — reconfirmed RESOLVED. `make quality-gate` exit **0**, `1865 passed;
  0 failed`, on a volume that never dropped below 49 GiB free. Same total as `116-06`, because
  `--features full` compiles none of this plan's inline tests.
- **`D-116-DISK`** — did NOT trigger; guidance followed (`df -h /` before and after every long
  build). The gate consumed roughly 7 GiB this time rather than 42, because `target/` was warm.
- **`D-116-LINT`** — reconfirmed, and extended by `D-116-LINT-OAUTH` above.
- **`D-116-FAILFAST`** — followed: the negative control ran with `--no-fail-fast` and the
  denominator (**29**) was asserted against the suite's full count before the partition was read.
- **`D-116-EX`** — still open. This plan's 3 doctests do **not** discharge it; `116-08` owns
  `examples/c11_oauth_iss_state_validation.rs`.
- **`D-116-DOC`** — applied as amended, both halves, zero new errors. No further action.

## Next Phase Readiness

**`116-11` and `116-13` are unblocked on this plan's contract.** Every symbol they name exists, is
public, is documented and is tested:

| Consumer | What it can now rely on |
|---|---|
| `116-11` | `FileCredentialStore` is the concrete `Arc<dyn CredentialStore>` for the CLI path; `save_with_issuer` is genuinely atomic here (counter-proven), so D-18's issuer record and the credentials cannot disagree. `default_credential_path()` is the ONLY `dirs` caller and must be invoked at the call site — do not push it into a constructor |
| `116-13` | all four `CredentialStoreAdmin` operations work against a real file with the same exact counts `InMemoryCredentialStore` asserts; `take_migration_report()` yields once and then `None`, and its `DroppedEntry` list is what a user must be told about. `FileCredentialStore::lock_path()` exists so `auth login` can name the lock when another process holds it |
| `116-15` | `make quality-gate` exit **0** is citable **at this HEAD** — the first time in the phase. `D-116-KEYCHAIN` and `D-116-TRIPWIRE` are both closed by measurement here |
| `116-10`, `116-12` | `src/client/oauth.rs` carries **29** pre-existing clippy errors under the gate-equivalent `full,oauth` command. Measure before editing |

**Carried obligations:**

| Owner | Obligation |
|---|---|
| `116-11` / `116-13` | do NOT move the format or the migration into this module; it must stay grep-clean of `serde::` and the schema-version identifier |
| `116-13` | surface `take_migration_report()`'s `DroppedEntry` list; mind the forward-compat trap the rustdoc records — an installed `cargo-pmcp` 0.18.0 rejects a document at the new schema version |
| `116-15` | reconcile the plan-pair contradiction on `schema_version` (grep vs rustdoc) in favour of the grep; close or waive `D-116-EX`; fold `D-116-LINT-OAUTH` into the phase conventions; do not book `AUTH-03` on this plan's evidence alone |
| every source-touching plan | run BOTH `make lint` AND the `full,oauth` gate-equivalent (`D-116-LINT-OAUTH`); run negative controls with `--no-fail-fast` and assert the denominator (`D-116-FAILFAST`) |

No blockers.

## Self-Check: PASSED

Files claimed created/modified, verified on disk:

```
FOUND: src/shared/credential_file.rs                              (690 lines, min_lines 260 ✓)
FOUND: tests/oauth_credential_file.rs                             (1069 lines, min_lines 180 ✓)
FOUND: src/shared/mod.rs                                          (+12)
FOUND: src/lib.rs                                                 (+7)
FOUND: src/shared/credential_store.rs                             (+9/-1, cfg( count still 1)
FOUND: .planning/phases/116-auth-hardening-seps/deferred-items.md (434 lines, was 377)
```

Commit claimed, verified in `git log`:

```
FOUND: 2d769409  feat(116-16): default on-disk credential store with a serialized read-modify-write
```

`must_haves` verification:

```
✓ truths[1] atomic 0o600-in-0o700 write and a schema-1 file read by MIGRATING it —
  save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates (over a directory this store
  creates, so the 0o700 half is not vacuous) + a_pre_existing_loose_file_is_tightened_by_the_next_save,
  both OBSERVED failing under a 0o644 break; a_schema_1_file_is_read_by_migrating_it_not_by_failing
  reads BOTH entries from a literal cargo-pmcp TokenCacheV1 fixture
✓ truths[2] two concurrent writers cannot silently lose one another's credentials — the read
  happens INSIDE the lock, proven by a_waiter_reads_the_document_the_lock_holder_left_behind,
  which fails deterministically under a read-before-lock break while the join! test does not
✓ truths[3] saving credentials and recording the issuer are ONE update — save_with_issuer
  overridden as one with_snapshot_mut closure; save_with_issuer_writes_the_file_exactly_once
  counts 1 write (and measures the two-call baseline as 2 in the same test) and is the ONLY test
  that fails when the override is deleted
✓ truths[4] construction performs no filesystem and no environment access —
  new_touches_nothing_and_a_load_does_not_either constructs inside a three-level missing
  directory and asserts nothing appears, then asserts a LOAD creates nothing either; grep over
  the new() body shows no dirs, no fs::, no create_dir_all
✓ artifacts: src/shared/credential_file.rs 690 >= 260 and contains "pub struct
  FileCredentialStore" (:186); provides FileCredentialStore (both traits), the atomic 0o600
  write, the lock discipline and default_credential_path
✓ artifacts: tests/oauth_credential_file.rs 1069 >= 180 — permission, atomicity, concurrency,
  migration and admin-op coverage against a real tempfile::tempdir()
✓ key_links: "parse_credential_snapshot" present in src/shared/credential_file.rs (3 refs) —
  the file impl knows no format, grep-proven
✓ key_links: "oauth-cache.json" present in src/shared/credential_file.rs (9 refs) —
  default_credential_path resolves ~/.pmcp/oauth-cache.json, and a schema-1 file there migrates
  in place without being rewritten on read
```

Plan-level verification block:

```
✓ binary(oauth_credential_file) — 29 run / 29 passed, non-zero count grep-asserted
✓ binary(oauth_credential_store) — 54 run / 54 passed (116-05 unregressed)
✓ binary(v2_bounded_reads_tripwire) — 13 run / 13 passed
✓ cargo build --target wasm32-unknown-unknown --no-default-features --features wasm — exit 0,
  92 warnings = anchor, 0 naming this file
✓ make quality-gate — exit 0 (1865 unit passed / 0 failed; 445 doctests passed / 0 failed)
✓ make lint — No lint issues; gate-equivalent full,oauth run has 0 errors attributable to this plan
✓ pmat quality-gate --fail-on-violation --checks complexity — 0 violations
✓ cargo semver-checks --baseline-rev b2bf9157 — 223 pass / 0 fail, zero breaking findings
✓ make doc-check — 28 ^error lines = the recorded anchor, 0 attributable, FIRST pass
✓ git diff --exit-code b2bf9157..HEAD -- Cargo.toml — exit 0, zero dependencies added
```

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-04*
