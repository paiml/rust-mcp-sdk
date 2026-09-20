---
phase: 116-auth-hardening-seps
plan: 14
subsystem: auth
tags: [tripwire, bounded-reads, http-09, auth-03, d-113-v, scope-fence, anti-vacuity]

# Dependency graph
requires:
  - phase: 116-06
    provides: "collect_reqwest_body_within_cap + DEFAULT_AUTH_RESPONSE_BYTES in src/shared/http_body_cap.rs — the reqwest chunk()-accumulate bound the four auth files were rewritten onto"
  - phase: 116-07
    provides: "the bounded discovery/token/JWKS/UserInfo reads in the generic_oidc and cognito providers"
  - phase: 116-11
    provides: "deletion of struct TokenCache / load_cached_token, which removed src/client/oauth.rs's tokio::fs::read_to_string needle"
  - phase: 116-12
    provides: "the bounded refresh path in src/client/oauth.rs"
  - phase: 116-01
    provides: "116-BASELINES.md § D-15 Pre-Fix Violation Baseline — the 33-site violation list this plan drove to zero, observed with a temporary widening"
provides:
  - "the four auth-surface files are PERMANENTLY inside the bounded-read tripwire's scope, and the tripwire reports zero"
  - "REQUIRED_FILES holds FULL RELATIVE PATHS matched against rel(), so a base name can no longer be satisfied by the wrong file"
  - "the tripwire's module doc names AUTH-03 / D-15 as its second owner and D-113-V as the item closed"
  - "the whole-body failure message carries reqwest-shaped guidance for auth files and hyper-shaped guidance for the transport files"
  - "four accumulation ALLOWLIST entries covering the 13 push_str sites the widening drags into the change detector"
affects: [116-15, phase-113-deferred-items, release-v2.18.0]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "close the fence LAST: bound the reads across several waves, then widen scope, so the gate is never red in between"
    - "an anti-vacuity guard keyed on a FULL RELATIVE PATH, because a base name can be satisfied by the wrong file"
    - "run the control in the direction that can fail, and label the direction that cannot as a measured LIMIT"

key-files:
  created:
    - .planning/phases/116-auth-hardening-seps/116-14-SUMMARY.md
  modified:
    - tests/v2_bounded_reads_tripwire.rs

key-decisions:
  - "the accumulation change detector was the real work: the widening drags 13 push_str sites into it, not the 7 116-BASELINES.md measured, and the plan's action text never mentions that list at all"
  - "adding accumulation ALLOWLIST entries is NOT the forbidden move — WHOLE_BODY_ALLOWLIST is a prohibition with an empty floor, ALLOWLIST is a change detector whose designed closure IS a written justification"
  - "the plan's own base-name grep criterion was honoured literally: a doc comment that quoted the unsafe form was reworded so the mechanical fence stays clean for future readers"
  - "REQUIRED_FILES' matcher had to move from file_name() to rel() in the same edit — converting the constant alone would have failed every pre-existing entry"

patterns-established:
  - "Fence-closing order: fix, then fence. Widening scope ahead of the fixes would have left `make quality-gate` red for several waves, which the repo's zero-tolerance rule does not permit"
  - "A negative control that names a file AND a line is what separates a fence from decoration"

requirements-completed: []

# Metrics
duration: ~4h wall-clock (single session; ~3h of it gate/build time on a 4-job-capped host)
completed: 2026-08-06
---

# Phase 116 Plan 14: Bounded-Read Tripwire Widened onto the Auth Surface Summary

**D-113-V is closed by measurement: the four auth-surface files are permanently inside the bounded-read tripwire's scope, the fence that would have reported 33 whole-body sites now reports zero, the exemption allowlist is still empty, and the fence was observed to bite — naming a file and a line — rather than assumed to.**

## Performance

- **Duration:** ~4h wall-clock, single session. Almost all of it was build/gate time: `make quality-gate` alone runs ~45 min on this host at `CARGO_BUILD_JOBS=4`, and the plan's `-E 'binary(...)'` selector form builds every integration-test binary before running 13 tests.
- **Tasks:** 1 of 1
- **Files modified:** 1 (`tests/v2_bounded_reads_tripwire.rs`, 1153 → 1270 lines, +142/−25)

## Accomplishments

- **The fence is closed and reports zero.** `EXTRA_SCOPE` now carries `src/client/auth.rs`, `src/client/oauth.rs`, `src/server/auth/providers/generic_oidc.rs` and `src/server/auth/providers/cognito.rs` alongside HTTP-09's two. `no_unbounded_whole_body_read_over_peer_supplied_bytes` passes over all six: the 33 sites `116-BASELINES.md` observed in August are **0**. Plans 116-06, 116-07, 116-11 and 116-12 did the bounding; this plan made it permanent.
- **`REQUIRED_FILES` is full relative paths, and the matcher moved with it.** The constant went from five base names to nine full paths, and the guard at what is now `:170` changed from `p.file_name().is_some_and(|n| n == *required)` to `rel(p) == *required`. Both halves were required in one edit — converting the constant alone would have failed every pre-existing entry. This is the Codex-flagged ambiguity discharged on measurement: nine tracked files in this repo share `auth.rs`'s base name and two of them (`src/client/auth.rs`, `src/types/auth.rs`) live under `src/`.
- **The allowlist floor held.** `WHOLE_BODY_ALLOWLIST` is still `&[]`, and `every_whole_body_exemption_carries_a_substantive_justification` still asserts `len() == 0`. Not one read was exempted.
- **The file no longer over-claims its own authority.** The module doc now states outright that it has TWO owners, quotes HTTP-09 and AUTH-03/D-15 separately, and records that the four auth files entered scope in phase 116 to close D-113-V — including *why the ordering was fix-then-fence*.
- **The failure message tells the truth about which client you are in.** It splits into a hyper/axum branch (`http_body_util::Limited`, `collect_body_within_cap`) and a reqwest branch (`Response::chunk()`-accumulate, `collect_reqwest_body_within_cap` in `src/shared/http_body_cap.rs`, then `serde_json::from_slice`), and states plainly that `Limited` does not apply to reqwest. Verified by reading the actual panic output during the negative control, not by inspection.

## Task Commits

1. **Task 1: Widen both scope constants and update the fence's own justification** — `43b3dde8` (test), 1 file, +142/−25.

## Files Created/Modified

- `tests/v2_bounded_reads_tripwire.rs` — module doc (two owners, D-113-V closure, fix-then-fence ordering); `EXTRA_SCOPE` 2 → 6 entries; `REQUIRED_FILES` 5 base names → 9 full relative paths plus the `rel()` matcher and a widened failure message on the guard itself; the whole-body failure message split by HTTP client; four new accumulation `ALLOWLIST` entries (12 → 16).

## Decisions Made

### The accumulation change detector was the actual work, and the plan does not mention it

The plan's `<action>` names only `WHOLE_BODY_ALLOWLIST` and forbids growing it. Measured reality, from widening `EXTRA_SCOPE` alone:

```
Summary [0.061s] 13 tests run: 12 passed, 1 failed
FAIL every_peer_byte_accumulation_is_reviewed
  NEW accumulation site(s): src/client/auth.rs `push_str(` at line(s) [602, 603]
  NEW accumulation site(s): src/client/oauth.rs `push_str(` at line(s) [1689]
  NEW accumulation site(s): src/server/auth/providers/cognito.rs `push_str(` at line(s) [407, 411, 419, 857, 858]
  NEW accumulation site(s): src/server/auth/providers/generic_oidc.rs `push_str(` at line(s) [739, 740, 881, 885, 893]
```

`no_unbounded_whole_body_read_over_peer_supplied_bytes` **passed on that same run** — so the 33 reads were already bounded and the ONLY thing standing between the plan and closure was the accumulation population.

`116-BASELINES.md` predicted this and warned `116-14` not to miss it, but predicted **7** sites and a closure total of "33 + 7 = 40". The observed population is **13**, giving 33 + 13 = 46. The six extra sites are the `rendered_source_chain` helpers (2 each in `auth.rs`, `generic_oidc.rs`, `cognito.rs`) that plans 116-06/07/12 introduced *after* the baseline was taken — the baseline was measured 2026-08-02 against a tree four plans older. **Neither 33, nor 40, nor 46 is the closure condition**; the tripwire reporting zero is, exactly as `116-BASELINES.md` says.

**Adding entries to `ALLOWLIST` is not the forbidden move.** The two lists are different mechanisms and the file says so in its own doc: `WHOLE_BODY_ALLOWLIST` is a *prohibition* whose empty state is its written floor, so an entry there means a read was exempted rather than fixed. `ALLOWLIST` (accumulations) is a *change detector over a justified population* — its designed closure IS a written justification naming the mechanism that bounds the site, because "whether appending to a growable buffer is bounded depends on the drain downstream of it, which no lexical scan can see". Four entries were added, one per (file, needle) key, and `every_allowlist_justification_is_substantive` (≥40 chars, no verbatim reuse between entries) passes.

The two mechanisms justified:

| Mechanism | Sites | What bounds it |
|---|---|---|
| `rendered_source_chain` (auth.rs ×2, generic_oidc ×2, cognito ×2) | 6 | One append per `source()` LINK of a finite reqwest error chain. A reqwest error's `Display` names the URL, kind and status — it never carries the response body, which is the only thing an IdP chooses the length of. The bound is the chain's DEPTH, not a drain. |
| `authorization_url` query building (generic_oidc ×3, cognito ×3) | 6 | `AuthorizationParams` is supplied by the calling server, not read off a socket. The nonce and PKCE appends sit under single-shot `if let` guards; the `extra` loop appends once per caller-supplied map entry, all `urlencoding::encode`d. **Recorded honestly:** if an operator forwards a remote client's query map into `extra` verbatim, the bound becomes that map's size — that is the operator's decision, not a ceiling this code carries. |
| `extract_base_url` port append (oauth.rs ×1) | 1 | `Url::port()` is `Option<u16>` after `Url::parse` already accepted the URL, so at most a colon and five digits, once per call. TYPE-bounded, same shape as `normalize_server_key`'s existing entry reached from a different direction. |

`generic_oidc.rs`'s entry additionally records that its URL prefix — the discovery document's `authorization_endpoint`, which IS IdP-chosen text — arrived through `collect_reqwest_body_within_cap` under `DEFAULT_AUTH_RESPONSE_BYTES` (verified at `src/server/auth/providers/generic_oidc.rs:556`), so even the peer-chosen part was capped at 1 MiB before it was ever appended to.

### The plan's own base-name grep criterion was honoured literally

The first draft of the `REQUIRED_FILES` doc comment explained the hazard by quoting the unsafe form, which made the plan's acceptance grep return a hit:

```
$ grep -nE '"(auth|oauth|generic_oidc|cognito)\.rs"' tests/v2_bounded_reads_tripwire.rs
112:/// `src/types/auth.rs`), so a bare `"auth.rs"` entry would be satisfied by the
```

That hit was prose, not an entry — but leaving it would have degraded a mechanical fence into one a future reviewer has to reason about. The comment was reworded to name the hazard without the quoted literal, and the doc now *carries the grep itself* as a self-check a reviewer can run. The criterion now returns nothing (exit 1).

### `make quality-gate` had to be run twice, and only the second run is citable

The first attempt was launched under two concurrent writers to one log path (a backgrounded task plus a `nohup` relaunch), which interleaved a killed run's `make: *** [quality-gate] Terminated: 15` into the same file as a later success banner. **That log was discarded rather than interpreted.** The gate was re-run single-writer with its exit code captured to a separate file: `target/116-verify/116-14-qg.exit` contains `0`, `target/116-verify/116-14-qg.log` contains the success banner exactly once, zero `Terminated` lines and zero `FAILED` lines.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] The widening drags 13 `push_str(` accumulation sites into the change detector; the plan's action text does not mention the accumulation allowlist**

- **Found during:** Task 1, first measurement (`EXTRA_SCOPE` widened alone)
- **Issue:** `every_peer_byte_accumulation_is_reviewed` fails with four NEW-site reports. Without addressing it the plan's "the tripwire reports ZERO violations" behavior is unreachable, and `make quality-gate` would go red.
- **Fix:** four `ALLOWLIST` entries with distinct written justifications naming the bounding mechanism for each (file, needle) population. `WHOLE_BODY_ALLOWLIST` untouched at `&[]`.
- **Files modified:** `tests/v2_bounded_reads_tripwire.rs`
- **Verification:** `every_peer_byte_accumulation_is_reviewed` and `every_allowlist_justification_is_substantive` both pass; 13/13 green.
- **Committed in:** `43b3dde8`

**2. [Rule 2 - Missing critical functionality] `REQUIRED_FILES`' matcher had to change with the constant**

- **Found during:** Task 1
- **Issue:** The guard matched `p.file_name() == required`. Converting the constant to full relative paths without changing the matcher would have made *every* entry fail, including the five pre-existing ones. The plan anticipated this ("if so, convert the pre-existing entries in the same edit") but the matcher change is the load-bearing half.
- **Fix:** matcher → `rel(p) == *required`; all five pre-existing entries converted to full paths in the same edit; the guard's failure message now states that the constant holds full paths and that a fire means a path was dropped or mistyped.
- **Verification:** every pre-existing test still passes, including `scanner::scope_discovery_finds_the_named_files_at_runtime` (which does its own independent base-name check over `src/shared/`) and `scanner::a_rustfmt_broken_chain_is_matched_and_reports_its_first_line` at what is now `:1167`.
- **Committed in:** `43b3dde8`

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing-functionality). **Impact:** both are required for the plan's stated behavior to be reachable. No scope creep — the commit touches exactly one file, and `git diff --exit-code` on all four source files exits 0.

## Verification

All runs on this host used `SSL_CERT_FILE="$PWD/target/116-verify/cacert.pem"` (158 certs exported from the system keychain by the Apple-signed `security` tool) and `CARGO_BUILD_JOBS=4` — see Issues.

| Gate | Result |
|---|---|
| `cargo nextest run --features full,oauth -E 'binary(v2_bounded_reads_tripwire)'` | exit 0 — `Summary [0.098s] 13 tests run: 13 passed, 0 skipped` |
| `cargo nextest run --features full -E 'binary(v2_bounded_reads_tripwire)'` | exit 0 — `Summary [0.089s] 13 tests run: 13 passed, 0 skipped` |
| The plan's `<verify><automated>` chain, run verbatim end to end | exit 0 (both nextest runs, both non-zero-count greps, and the `git diff --exit-code`) |
| `git diff --exit-code -- src/client/auth.rs src/client/oauth.rs src/server/auth/providers/` | exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `make quality-gate` | **exit 0** (`target/116-verify/116-14-qg.exit`), banner present once, 0 `Terminated`, 0 `FAILED` |

**Selector discipline:** the plan's `binary(v2_bounded_reads_tripwire)` form was used for the two citable runs and both were asserted NON-ZERO from their `Summary [..] 13 tests run` line, not from exit 0 alone. The faster `--test v2_bounded_reads_tripwire` cargo-target form was used for the iterate loop and for the three controls; it selects the same 13 tests in the same binary and was cross-checked against the `-E` form.

Static acceptance checks:

```
$ for p in src/client/auth.rs src/client/oauth.rs \
           src/server/auth/providers/generic_oidc.rs \
           src/server/auth/providers/cognito.rs; do grep -c "$p" tests/v2_bounded_reads_tripwire.rs; done
7   5   5   5          # all >= 2 (EXTRA_SCOPE + REQUIRED_FILES, plus doc/message mentions)

$ grep -nE '"(auth|oauth|generic_oidc|cognito)\.rs"' tests/v2_bounded_reads_tripwire.rs
(exit 1 — no bare base name anywhere in the file)

$ grep -n -A2 'const WHOLE_BODY_ALLOWLIST' tests/v2_bounded_reads_tripwire.rs
636:const WHOLE_BODY_ALLOWLIST: &[Allowed] = &[];

$ grep -c 'AUTH-03\|D-15\|D-113-V' tests/v2_bounded_reads_tripwire.rs
7
```

### The three controls — what fired, and where

**Control 1 — negative control (the fence bites).** `read_token_body` in `src/client/auth.rs` was temporarily reverted from `collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES)` to the pre-116-06 `response.bytes().await.map(|b| b.to_vec())`.

*Assertion that fired:* `no_unbounded_whole_body_read_over_peer_supplied_bytes`, panicking at `tests/v2_bounded_reads_tripwire.rs:695:5`. Verbatim:

```
HTTP-09 / AUTH-03: unbounded whole-body read(s) over peer-supplied bytes:
  src/client/auth.rs:828 — unbounded `.bytes().await`
    statement: ...response
```

Two things this proves beyond "it fails": the violation names the **file and the line**, and the needle matched a chain rustfmt had broken across lines (`.bytes()` on 828, `.await` on 829) — the split-chain handling `D-113-V` relied on, now demonstrated on an auth file rather than inferred from the scanner's own unit test. The reqwest-branch guidance printed in full, naming `collect_reqwest_body_within_cap` and `src/shared/http_body_cap.rs`.

*Restored:* byte-identical (`shasum -a 256 -c` → OK; `git diff --exit-code -- src/client/auth.rs` → 0). The committed diff contains no source change.

**Control 2 — anti-vacuity control, in the direction that can fail.** `src/server/auth/providers/cognito.rs` removed from `EXTRA_SCOPE`, its full path **retained** in `REQUIRED_FILES`.

*Result:* `13 tests run: 9 passed, 4 failed`. All four fail at `tests/v2_bounded_reads_tripwire.rs:170:9` — the `REQUIRED_FILES` guard inside `scope_files()`:

```
scope discovery lost src/server/auth/providers/cognito.rs; discovered: ["src/client/auth.rs",
"src/client/oauth.rs", "src/client/subscriptions.rs",
"src/server/auth/providers/generic_oidc.rs", "src/server/streamable_http_server.rs", ...]
```

The four: `no_unbounded_whole_body_read_over_peer_supplied_bytes`, `every_peer_byte_accumulation_is_reviewed`, `the_two_known_capped_whole_body_reads_are_found_and_classified_bounded`, and `scanner::scope_discovery_finds_the_named_files_at_runtime`. This is the control that proves `REQUIRED_FILES` detects coverage lost by omission — the silent failure a path typo would cause. Restored.

**Control 3 — counter-control, and it is a LIMIT, not evidence.** `src/server/auth/providers/cognito.rs` removed from `REQUIRED_FILES`, `EXTRA_SCOPE` left intact.

*Result:* `13 tests run: 13 passed, 0 skipped`, exit 0 — **as expected**.

Recorded explicitly as the measured limit of what this guard can protect: `REQUIRED_FILES` guards **discovery**, not the requirement list itself. Shrinking the requirement list is a silent weakening that no assertion in this file can catch and that only code review will. Running this direction and reporting a pass proves *nothing about the fence* — the previous revision of the plan specified the control this way round, cross-AI review caught it, and it is written down here so nobody re-derives it as evidence. Restored.

## Issues Encountered

### 1. The harness kills detached background builds when a monitor loop times out

`make quality-gate` takes ~45 min here, far past the 600 s tool ceiling. Two attempts to run it in a monitored background task were killed mid-flight (the log stopped at `Compiling pmcp-code-mode` with no process left and no exit line). What survives is `nohup CMD > log 2>&1 < /dev/null & disown` issued at the **top level** of a Bash call — inside a subshell, `disown` fails with "no current job" and the process dies with the next monitor timeout. This cost roughly two wasted 40-minute gate runs.

**Never let two writers share one log path.** The first `make quality-gate` log ended up containing a killed run's `Terminated: 15` *and* a later run's success banner, because both had the file open from their own `>` redirect. That log was discarded, not interpreted, and the gate re-run single-writer with `echo $? > .exit`.

### 2. `SSL_CERT_FILE` was required again, and the export is not persistent

Same environment fault 116-13 recorded. `target/116-verify/cacert.pem` did not exist at the start of this session and was regenerated with `security find-certificate -a -p /System/Library/Keychains/SystemRootCertificates.keychain` — 158 certificates, matching 116-13's recorded count exactly. Every number in this summary is green **under that variable**. No test was skipped and no code changed. A future executor must expect to regenerate it, not to find it.

### 3. `116-BASELINES.md`'s accumulation count is stale by four plans

Its "7 push_str sites / 33 + 7 = 40" was measured 2026-08-02 against a tree without 116-06/07/12's `rendered_source_chain` helpers. Observed 2026-08-06: **13** sites. The baseline's *warning* was right and load-bearing; its *number* was not, and a plan that had trusted the number without re-measuring would have written three allowlist entries and gone red on the fourth.

### 4. A rust-analyzer was running, from serena's MCP server rather than Zed

`pgrep -x rust-analyzer` found one process — parent `.../uv/archive-v0/.../bin/python`, i.e. the serena language server, not Zed. It had accumulated **15 seconds** of CPU over 2h17m, versus the +965 s that characterised the Zed fault, so it was judged idle and left alone. Builds progressed normally throughout. Recorded so the next executor does not read a bare non-zero `pgrep` count as the documented fault.

### 5. Pre-existing untracked file, deliberately left alone

`tests/streamable_http_oauth_properties.proptest-regressions` is untracked and is **not** gitignored. It predates this session (it is in the conversation-opening `git status`) and is outside this plan's scope, so it was not committed. Proptest regression seeds are normally worth tracking; flagged here for whoever owns that suite.

## Threat Flags

None. This plan adds no network endpoint, no auth path, no file access and no schema change — it edits one test file and installs no packages (T-116-SC holds trivially).

## Self-Check: PASSED

Files asserted present:
- `.planning/phases/116-auth-hardening-seps/116-14-SUMMARY.md` — FOUND
- `tests/v2_bounded_reads_tripwire.rs` — FOUND
- `target/116-verify/116-14-qg.exit` (contains `0`) — FOUND
- `target/116-verify/tripwire.log`, `target/116-verify/tripwire-ungated.log` — FOUND

Commit asserted present in `git log --oneline --all`: `43b3dde8` — FOUND.

**AUTH-03 was NOT marked complete.** Consistent with 116-13's deliberate decision: 12 of this phase's 16 plans declare it, and `116-15` is the plan that books it with precise scoping. It stays `Pending` in `REQUIREMENTS.md`.

## User Setup Required

None.

## Next Phase Readiness

- **D-113-V can be moved from OPEN to CLOSED** in `.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md`, owner Phase 116, closed by commit `43b3dde8`. Its two recorded exclusions are both gone on the merits rather than by exclusion: `src/client/oauth.rs:282`'s post-hoc `MAX_DCR_RESPONSE_BYTES` check was replaced by the streaming bound, and `:953`'s `tokio::fs::read_to_string` disappeared with 116-11's deletion of `struct TokenCache`.
- **T-116-55 is now mitigated by a mechanism, not a discipline.** A new unbounded read in any of the four files fails `no_unbounded_whole_body_read_over_peer_supplied_bytes` by name — demonstrated, not asserted (Control 1).
- **Carried forward for `116-15`'s gate classification, both re-confirmed this session:**
  1. **`make quality-gate` does not reach the phase's `#![cfg(feature = "oauth")]` integration binaries.** This plan's tripwire *is* inside the gate's reach (it needs no `oauth` feature — proven by the `--features full` run), but the phase's `tests/oauth_*.rs` suites are not. That is how a stale `oauth_state_csrf` source assertion survived a green gate until 116-13's explicit `--features full,oauth` run (fixed as `42f5c8f0`).
  2. **There is NO pre-commit hook in this clone.** `CLAUDE.md`'s "ALL commits are blocked until quality gates pass" is a discipline, not a mechanism. This plan's commit was made after a green gate by choice, not by enforcement, and `116-15` must not book it as an enforced control.
- **`116-BASELINES.md` § D-15 should be annotated** with the corrected accumulation population (13, not 7) and the reason it moved, so the next reader does not trust the stale count.

---
*Phase: 116-auth-hardening-seps*
*Completed: 2026-08-06*
