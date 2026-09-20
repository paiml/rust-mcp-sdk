---
phase: 116-auth-hardening-seps
plan: 13
subsystem: auth
tags: [oauth, credential-store, cargo-pmcp, cli, semver, migration, keychain]

# Dependency graph
requires:
  - phase: 116-05
    provides: "CredentialKey / StoredCredentials / CredentialStore / CredentialStoreAdmin, normalize_server_key, and the pure schema 1 -> 2 migration in parse_credential_snapshot"
  - phase: 116-16
    provides: "FileCredentialStore + default_credential_path — the default on-disk store implementing BOTH traits"
  - phase: 116-11
    provides: "OAuthHelper::with_credential_store / with_account_scope; save_with_issuer as the single write"
  - phase: 116-12
    provides: "the refresh path that survives an omitted refresh_token, sends only the granted scope, and Interactivity::RefreshOnly"
provides:
  - "cargo-pmcp carries NO credential format, reader or writer of its own — auth_cmd is a thin adapter layer over pmcp's CredentialStore / CredentialStoreAdmin"
  - "the schema 1 -> 2 migration reaches real user data: ~/.pmcp/oauth-cache.json is migrated in place on first WRITE, byte-identical after a read-only command"
  - "a publishable pmcp 2.18.0 / cargo-pmcp 0.19.0 pair: the pin names the minor that actually ships CredentialStore"
  - "D-116-R1 proven end-to-end through the CLI: `auth logout <A>` leaves `auth token <B>` working when A and B share one authorization server"
affects: [116-14, 116-15, release-v2.18.0, pmcp-run-platform-store]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CLI subcommands as thin wrappers over a declared trait, never over ad-hoc file access"
    - "the empty account scope as the single-user CLI case — the CLI invents no identity"
    - "a scaffold version pin guarded by a test that compares it to the workspace version"

key-files:
  created:
    - .planning/phases/116-auth-hardening-seps/116-13-SUMMARY.md
  modified:
    - cargo-pmcp/src/commands/auth_cmd/cache.rs
    - cargo-pmcp/src/commands/auth_cmd/{login,logout,token,refresh,status,mod}.rs
    - cargo-pmcp/src/commands/auth.rs
    - cargo-pmcp/tests/auth_integration.rs
    - Cargo.toml
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/templates/workbook_server.rs
    - CHANGELOG.md

key-decisions:
  - "Cargo.lock is NOT git-tracked here (.gitignore:3), so the plan's lockfile step resolves to its not-tracked branch — and the plan's own files_modified frontmatter lists a file that could never be committed"
  - "the CHANGELOG's `## [2.18.0] - Unreleased` heading is not a release heading: the repo convention is that the dated heading is written on the release branch, and `## [2.17.0] - Unreleased` sits directly below it"
  - "templates/workbook_server.rs's PMCP_VERSION is part of Task 2, not a separate concern: the version bump trips its scaffold-pin tripwire in both the lib and bin targets"
  - "the gates in this plan were run across two sessions on a host that wedged mid-execution; every number below names the session it came from"

patterns-established:
  - "Adapter-not-implementation: cache.rs greps clean of serde_json, schema_version and write_atomic — a format-knowledge grep is the fence that keeps a second store from reappearing"
  - "Report what was OBSERVED, not what was asked for: `auth refresh` and `auth token` report whether the authorization server was actually contacted"

requirements-completed: [AUTH-03]

# Metrics
duration: ~2 days wall-clock (2 sessions, one environmental pause); ~3h of actual work
completed: 2026-08-05
---

# Phase 116 Plan 13: cargo-pmcp Converges on the SDK Credential Store Summary

**cargo-pmcp's parallel token cache is gone — all five `auth` subcommands are thin wrappers over pmcp's `CredentialStore` / `CredentialStoreAdmin` against the same `~/.pmcp/oauth-cache.json`, with the schema 1→2 migration inherited from core rather than reimplemented, and a pmcp 2.18.0 / cargo-pmcp 0.19.0 pair that is coherent at publish time.**

## Performance

- **Duration:** two sessions separated by an environmental pause (see Issues). Session 1 (Task 1): 2026-08-05 morning. Session 2 (Task 2 + gates): 2026-08-05 evening.
- **Tasks:** 2 of 2
- **Files modified:** 13 (9 in Task 1, 4 in Task 2)

## Accomplishments

- **One machine, one credential store.** `TokenCacheV1`, `TokenCacheEntry`, `read`, `write_atomic`, `normalize_cache_key` and `refresh_and_persist` are deleted. `cargo-pmcp/src/commands/auth_cmd/cache.rs` is now 379 lines of adapters plus tests: `open_store`, `load_for_server`, `keys_for_server`, `report_migration`, `is_near_expiry`, `refresh_through_sdk`. `grep -rn 'serde_json\|schema_version\|write_atomic' cargo-pmcp/src/commands/auth_cmd/cache.rs` returns nothing — no format knowledge and no parallel I/O survived the port.
- **The migration reaches real user data without destroying it.** Two entries with issuers migrate and stay reachable; an entry with no issuer is dropped, named and counted through `take_migration_report`; two servers sharing one issuer stay independently addressable; a read-only command leaves the previous-format document byte-identical, and the first write carries every surviving login across.
- **D-116-R1 is closed through the CLI.** `logout_of_one_server_leaves_a_second_sharing_one_issuer_working_d_116_r1` asserts `auth token <B>` still works after `auth logout <A>` when both share one authorization server and account — the exact elevation-of-privilege the three-part key exists to prevent (T-116-54b).
- **Publishable version/pin pair.** `pmcp` 2.17.0 → 2.18.0 (MINOR: this phase is additive), `cargo-pmcp` 0.18.0 → 0.19.0, and cargo-pmcp's pin 2.9.0 → 2.18.0. `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` exits 0: `196 checks: 196 pass, 57 skip`, `Summary no semver update required`.

## Task Commits

1. **Task 1: Replace TokenCacheV1 with core's store** — `554a305e` (feat), 9 files, +1263/−704. Landed in session 1; verified but not re-implemented in session 2.
2. **Task 2: Version bump and dependency pin** — `be93951d` (chore), 4 files, +63/−6.

Adjacent, NOT part of this plan: `42f5c8f0` (fix) repaired `oauth_state_csrf::the_env_override_is_read_inside_the_flow_and_warns_on_an_unrecognised_value`, a stale source-inspection assertion broken by `75c4d088`'s /simplify hoist. It surfaced in this plan's `--features full,oauth` run and was committed separately because it is not this plan's change.

## Files Created/Modified

- `cargo-pmcp/src/commands/auth_cmd/cache.rs` — the whole parallel implementation replaced by adapters over `pmcp::shared::credential_store` / `credential_file`; `default_multi_cache_path()` still resolves `~/.pmcp/oauth-cache.json`, now via `default_credential_path()`
- `cargo-pmcp/src/commands/auth_cmd/logout.rs` — `delete_by_server` / `clear_all`, with all four semantics preserved verbatim
- `cargo-pmcp/src/commands/auth_cmd/{login,token,refresh,status}.rs` — `with_credential_store` + `with_account_scope`; `status` sources from `list_keys`
- `cargo-pmcp/src/commands/auth.rs` — `try_cache_token` resolves through the store instead of reading the file
- `cargo-pmcp/tests/auth_integration.rs` — 7 → 20 tests
- `Cargo.toml`, `cargo-pmcp/Cargo.toml` — the version/pin trio
- `cargo-pmcp/src/templates/workbook_server.rs` — `PMCP_VERSION` follows the bump
- `CHANGELOG.md` — `## [2.18.0] - Unreleased`

## Decisions Made

- **The key is three-part and the account is empty.** Every `CredentialKey::new` under `auth_cmd/` passes `(issuer, CLI_ACCOUNT_SCOPE, normalize_server_key(url))` with `CLI_ACCOUNT_SCOPE = ""`. The CLI is single-user; the account scope exists for multi-tenant platform callers, and inventing an identity here would make CLI keys unreachable from the platform seam.
- **`refresh` is no longer a force-refresh, and the CHANGELOG says so.** The SDK serves an unexpired token verbatim and spends a refresh only once it has expired. Both `auth token` and `auth refresh` now report what they OBSERVED. Task 1 fixed the draft's "Refreshing cached token for …" banner printed *before* the SDK call, and the `Force-refresh` help text.
- **`## [2.18.0] - Unreleased` is not a release heading.** Task 2's acceptance criterion forbids adding a release heading; the repo writes `- Unreleased` for unpublished versions (`## [2.17.0] - Unreleased` sits directly below) and adds the dated heading on the release branch. Landing this section also makes `554a305e`'s message ("the narrowing is recorded in the CHANGELOG") true — that file had been left uncommitted.

### `Cargo.lock` — the recorded resolution, not a silent skip

Task 2 item 4 and the acceptance criterion "If `Cargo.lock` is git-tracked (per 116-BASELINES.md item 6), it is committed in the same commit" resolve to the **NOT-tracked** branch. Confirmed independently against the baseline:

```
$ git ls-files --error-unmatch Cargo.lock
error: pathspec 'Cargo.lock' did not match any file(s) known to git
$ grep -n 'Cargo.lock' .gitignore
3:Cargo.lock
```

`116-BASELINES.md` item 6 records the same result and states outright that "`116-13` must NOT list `Cargo.lock` among its modified files". **The plan's own `files_modified` frontmatter therefore lists a file it could never commit** — a defect in the plan, carried in from a cross-AI review comment that is right in general and wrong for this repo. Consequences: no lockfile commit; `git status --porcelain Cargo.lock` is vacuously empty; `cargo check --workspace --locked` still exits 0 because the local lockfile was regenerated by the preceding `cargo check`.

### The refined dependency fence, with its output

RESEARCH's fence (`git diff --exit-code b2bf9157..HEAD -- Cargo.toml Cargo.lock`, expecting EMPTY) is deliberately broken by this task, so it is refined to "no dependency line added, removed or changed — only the version line", verified by filtering the diff to `+`/`-` lines excluding the `+++`/`---` headers:

```
$ git diff b2bf9157 -- Cargo.toml | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)'
-version = "2.17.0"
+version = "2.18.0"

$ grep -rnE '^oauth2\s*=|^openidconnect\s*=' Cargo.toml
(exit 1 — no hits)

$ grep -rn 'oauth2::' cargo-pmcp/src/commands/
(exit 1 — no hits)
```

The `Cargo.lock` half of the original fence is dropped as inapplicable (untracked). `cargo-pmcp/Cargo.toml`'s diff is the version line, the pin line (`path` and `features = ["streamable-http", "oauth"]` unchanged), and two comment-only edits whose old text became FALSE when Task 1 landed: `tempfile`'s "Atomic token cache writes (auth_cmd::cache)" and `url`'s "URL normalization for cache keys". Both crates remain genuinely used (`url` at `commands/team/dev.rs:407`, `tempfile` at `commands/configure/config.rs:7` among others); neither dependency spec changed a byte. **T-116-SC holds: zero packages added.**

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The version bump tripped the scaffold-pin tripwire**

- **Found during:** Task 2 (version bump)
- **Issue:** `cargo-pmcp/src/templates/workbook_server.rs:53` hardcodes `PMCP_VERSION = "2.17.0"` for the `Cargo.toml` that `cargo pmcp new` emits, and `emitted_pmcp_version_matches_workspace_pin` asserts it equals the workspace version. Bumping the root to 2.18.0 failed it in BOTH the lib and bin targets. Left unfixed, `cargo pmcp new` would scaffold projects pinned to a pmcp with no `CredentialStore` — a silent version-skew defect, which is precisely what that tripwire exists to catch.
- **Fix:** `PMCP_VERSION` → `"2.18.0"`. It is the only such pin in the tree; `TOOLKIT_VERSION`'s twin test already passed.
- **Files modified:** `cargo-pmcp/src/templates/workbook_server.rs`
- **Verification:** both failures gone; `cargo nextest run -p cargo-pmcp` → 1413 passed
- **Committed in:** `be93951d` (Task 2 commit — it IS Task 2's change, not a fifth task)

**2. [Rule 1 - Bug] `CHANGELOG.md` had been left uncommitted by session 1**

- **Found during:** Task 2
- **Issue:** `554a305e`'s message states "the narrowing is recorded in the CHANGELOG", but the file was not in that commit — the claim was unbacked.
- **Fix:** landed the `## [2.18.0] - Unreleased` section with Task 2, which is also where the 2.18.0 heading belongs.
- **Committed in:** `be93951d`

Task 1's own two in-flight fixes (the premature "Refreshing…" banner and the `Force-refresh` help text), plus its two added migration-DESTRUCTION tests, are documented in `554a305e`'s message and were judged correct here rather than re-done.

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug). **Impact:** both are required for the commit's own claims to be true. No scope creep — the diff outside `auth_cmd/` is four version-ish lines.

## Verification

Two sessions, because the host wedged between them. Which number came from when is stated explicitly.

| Gate | Result | Session |
|---|---|---|
| `cargo nextest run -p cargo-pmcp` | exit 0 — `1413 tests run: 1413 passed, 9 skipped` | 2 (round 2, post-fixes) |
| `cargo nextest run -p cargo-pmcp -E 'binary(auth_integration)'` | exit 0 — `Starting 20 tests across 1 binary`, `20 tests run: 20 passed` | 2 |
| `cargo nextest run -p cargo-pmcp -E 'binary(cargo_pmcp) and test(auth_cmd)'` | exit 0 — `6 tests run: 6 passed, 454 skipped` | 2 (also green mid-wedge, twice) |
| `cargo check --workspace --all-features` | exit 0 | 2 |
| `cargo check --workspace --locked` | exit 0 | 2 |
| `cargo clippy -p cargo-pmcp` | exit 0 | 2 |
| `cargo nextest run --features full,oauth` | exit 100 — `3104 run: 3103 passed, 1 failed`; the single failure is the pre-existing `oauth_state_csrf` assertion fixed separately in `42f5c8f0` | 2 |
| `cargo semver-checks check-release -p pmcp --baseline-rev b2bf9157` | exit 0 — `196 checks: 196 pass, 57 skip` | 2 |
| `make quality-gate` | **exit 0** | 2 |

Static fences (session 2, unchanged since): no `struct TokenCacheV1`/`TokenCacheEntry`; no `oauth2::` under `cargo-pmcp/src/commands/`; no `serde_json`/`schema_version`/`write_atomic` in `cache.rs`; every `CredentialKey::new` three-arg; `~/.pmcp/oauth-cache.json` still the path.

**Where the two must-not-be-lost tests live now** (plan asks this to be named): the 0600/0700 permission behaviour is `tests/oauth_credential_file.rs:317 save_sets_0600_on_the_file_and_0700_on_the_parent_it_creates`, whose doc comment reads "Ported from `cargo-pmcp`'s `write_sets_0600_perms_on_unix`" (T-116-50 discharged in core). The normalizer idempotence property is `cargo-pmcp/tests/auth_integration.rs:170 normalize_is_idempotent_and_folds_slash_and_case_variants`.

**Selector discipline:** every command above uses `binary(auth_integration)` or `binary(cargo_pmcp) and test(auth_cmd)`; no bare `test(auth)` selector appears anywhere in this plan's verification. Both selectors were asserted NON-ZERO from their `Starting N tests` line, not merely from exit 0.

## Issues Encountered

### 1. A macOS `syspolicyd`/`amfid` wedge stopped every gate mid-plan (environmental, ~4h)

Between Task 1 landing and Task 2's gates, the host stopped being able to `exec()` **any newly-created file**. Three independent probes, all hanging with 0:00.00 CPU:

```
cp /bin/date /tmp/probe && /tmp/probe        # a COPY of an Apple-signed system binary — hung >45s
rustc -o /tmp/h /tmp/h.rs && /tmp/h          # fresh hello-world; rustc exits 0, the binary hangs >180s
./target/debug/cargo-pmcp --version          # hung >90s
```

`sample` on the hung process showed only `dyld` and the executable mapped — no libSystem — i.e. the kernel never released it past signature validation. `syspolicyd` (PID 73074) had accumulated 2892 minutes of CPU; the log showed `Error checking with notarization daemon: 3` / `MacOS error: -67062` and `amfid … Code=-423`. Already-validated binaries still ran, which is why `cargo` and `rustc` worked and only *new* outputs hung — and why the CLI-spawning tests in `auth_integration.rs` hung while its 6 non-spawning tests passed.

Handled as a `checkpoint:human-action`: the executor refused to write a SUMMARY asserting gates it could not run. Cleared by the operator (`sudo killall -9 syspolicyd`), confirmed by the `cp /bin/date` probe, and every gate then re-run to completion. **This is the second instance of the class already in memory as "macOS syspolicyd wedge fakes hung tests" — the `cp /bin/date` probe is the cheap discriminator and should be run BEFORE debugging any "hung test".**

### 2. The gates were run with a non-standard `SSL_CERT_FILE` (must be known by a reviewer)

`SSL_CERT_FILE=target/116-verify/cacert.pem` (158 certs exported from the system keychain by the Apple-signed `security` tool). This host denies freshly built binaries the keychain read that `rustls-native-certs` performs, so every test that builds a TLS client panicked at the **pre-existing** `.expect` at `src/shared/streamable_http.rs:458` with `ioErr -36`. Without the override: 106 failures in the core run and 14 in `make quality-gate`. With it: 1 and 0. First-hand corroboration from this executor's own pre-fix run — `scaffold_sql_server::test_tools_list_and_call_against_scaffolded_server` panicked at exactly `src/shared/streamable_http.rs:458:18`. **No test was skipped and no code changed**, but "green" here means green under that environment variable.

### 3. Zed's rust-analyzer made the same compile 254x slower

Measured on the identical compile: with Zed running, exit 124 (timed out at 1800s) and `syspolicyd` consumed +965s CPU; with Zed quit, exit 0 in 209s and `syspolicyd` consumed +3.8s. It was also the cause of the earlier multi-hour cargo build-lock deadlock observed in this session (a `cargo` process holding `target/debug/.cargo-lock` with no visible progress).

### 4. Two gate-scope findings to transfer to 116-15

- **`make quality-gate` does not reach these integration test binaries.** That is how the stale `oauth_state_csrf` assertion survived a green gate — it was only caught by an explicit `--features full,oauth` run. This is the fourth measurement in this phase of the same shape (D-116-LINT-OAUTH's test-side twin).
- **There is NO pre-commit hook installed in this clone** (`.git/hooks/pre-commit` does not exist). CLAUDE.md's "ALL commits are blocked until quality gates pass" is **not enforced locally** — the gate is a discipline, not a mechanism, and `116-15` must not book it as an enforced control.

## Self-Check: PASSED

Files asserted present: `116-13-SUMMARY.md`, `cargo-pmcp/src/commands/auth_cmd/cache.rs`,
`cargo-pmcp/tests/auth_integration.rs`, `cargo-pmcp/src/templates/workbook_server.rs` — all FOUND.
Commits asserted present in `git log --oneline --all`: `554a305e`, `be93951d`, `be1a782f`,
`42f5c8f0` — all FOUND.

**AUTH-03 was NOT marked complete, deliberately.** The plan's frontmatter declares it, but so do
**12 of this phase's 16 plans**, including the two that have not run (`116-14`, `116-15`). No prior
executor marked it, and `116-15` is the plan that books it with the precise scoping this summary
states under Next Phase Readiness. It was marked by this executor and then reverted; the
requirement stays `Pending` in `REQUIREMENTS.md` until `116-15` closes it.

## User Setup Required

None.

## Next Phase Readiness

- **AUTH-03 is bookable by `116-15`, with its scope stated precisely:** the `pmcp` core crate has no direct `oauth2`/`openidconnect` dependency and adds none; cargo-pmcp's pre-existing direct `oauth2 = "5.0"` is confined to `src/deployment/targets/pmcp_run/auth.rs` and there are **zero** `oauth2::` references under `cargo-pmcp/src/commands/`, before and after. A reviewer who greps will find that line, so the claim must be stated this way and never as "no oauth2 crate anywhere".
- **`T-116-52` remains ACCEPTED, not mitigated:** an already-installed cargo-pmcp 0.18.0 hard-errors on a `schema_version: 2` document with a message that already says "upgrade cargo-pmcp". Nothing in this repo can change a binary in the field. `116-15` should record it as a released-behaviour note.
- **Release readiness:** the tree would publish a coherent pair. Per CLAUDE.md's publish order, `pmcp` is item 2 and `cargo-pmcp` item 12; no tag was created and no dated CHANGELOG heading was added — both are release-branch activities.
- **Carried forward for 116-15:** the two gate-scope findings above, the `SSL_CERT_FILE` caveat on this session's green results, and the plan-frontmatter defect (`Cargo.lock` listed in `files_modified`).

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-05*
