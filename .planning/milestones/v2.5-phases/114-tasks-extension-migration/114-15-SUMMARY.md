---
phase: 114-tasks-extension-migration
plan: 15
subsystem: api
tags: [tasks, mcp-2026-07-28, security, owner-binding, idor, anti-oracle, negative-controls, live-socket]

requires:
  - phase: 114-09
    provides: "the v2 owner binding (the three-row identity table) and the ordered five-case refusal chain"
  - phase: 114-11
    provides: "the v2 -32602 task-not-found mapping and the frozen V2_TASK_NOT_FOUND_MESSAGE"
  - phase: 114-12
    provides: "v2 task creation from a real tools/call, and the pausing_task_tool that makes input_required client-reachable"
  - phase: 114-14
    provides: "tasks/update delivery, so a cross-caller FEED is a reachable probe rather than a hypothetical"
  - phase: 114-02
    provides: "the shared live-socket harness — BearerSubjects, AuthPosture, spawn_tasks_server_with_store, teardown"
provides:
  - "tests/v2_tasks_security.rs — 8 live-socket tests proving no cross-caller task visibility on all three v2 tasks methods"
  - "measured indistinguishability: each refusal is compared, in-test, against the same method's answer for a never-minted id"
  - "a per-method negative control for each of the three owner guards, plus an oracle control, plus two isolating controls"
  - "an entropy/non-sequence/non-derivation lock on minted task ids that is NOT a UUID-format lock"
  - "the accepted no-auth-provider shared-bucket caveat asserted rather than implied"
affects: [114-16, 114-18, 114-20]

tech-stack:
  added: []
  patterns:
    - "Indistinguishability is MEASURED: compute both refusals in the same test against the same server and compare, never assert a literal"
    - "A refusal test re-reads the record AS ITS OWNER; lastUpdatedAt equality measures 'no write landed'"
    - "A test that asserts an ACCEPTED weakness, with the reason in its rustdoc, is what stops a future reader rediscovering it as a bug"
    - "Lock the PROPERTY the spec states (entropy, non-sequence, non-derivation), never the FORMAT that happens to satisfy it"
    - "A comparison helper asserts field PRESENCE before field EQUALITY — two absent fields compare equal"

key-files:
  created:
    - tests/v2_tasks_security.rs
  modified: []

key-decisions:
  - "tasks/update's cross-caller probe runs against a PAUSED task, not a working one: task_input_snapshot answers NotFound for a task with no recorded requests, so a working task would refuse EVERY caller and the refusal would prove nothing about ownership"
  - "The is_anonymous_owner half of test 7 is a SOURCE assertion because pmcp-tasks is not a dependency of pmcp in any profile; adding one is a manifest change this coverage-only plan may not make (deferred as D-114-O)"
  - "Test 6 asserts a per-position entropy LOWER bound rather than parsing a UUID version; the estimator measured exactly 122.0 bits on the real generator and exactly 10.0 on a 1024-sample counter"
  - "Six negative controls rather than the plan's four: NC-4 tripped the CODE assertion before the message-equality one could fire, and the cancel re-read assertion had no control at all"
  - "Zero production files touched, so cargo semver-checks / cargo public-api / make wasm-build are answered STRUCTURALLY by a byte-empty `git diff -- src/ crates/` rather than re-run"

patterns-established:
  - "Fire the refusal, then the absent-id control, then the owner's re-read, then the owner's own success — in that order, so no leg can mask the one before it"
  - "A control that fails a test on assertion N proves only assertion N; isolate the later ones with a control that passes N"

requirements-completed: []

duration: 95min
completed: 2026-07-31
---

# Phase 114 Plan 15: v2 Tasks Cross-Caller Security Summary

**Two authenticated callers, one server, one socket: B cannot read, feed or cancel A's task on any of
the three v2 `tasks/*` methods, its refusal is byte-identical to the one a never-minted id earns, and
six negative controls prove every one of those guards is load-bearing — with zero production files
touched.**

## Performance

- **Duration:** ~95 min
- **Tasks:** 2 of 2
- **Files created:** 1 · **Files modified:** 0 (production diff is byte-EMPTY)
- **Commits:** 2

## Accomplishments

### `tests/v2_tasks_security.rs` — 8 tests, 1123 lines, zero `ClientBuilder`

| # | test | property |
|---|------|----------|
| 1 | `v2_cross_caller_tasks_get_is_not_found` | B cannot READ A's task |
| 2 | `v2_cross_caller_tasks_update_is_not_found` | B cannot FEED A's paused task |
| 3 | `v2_cross_caller_tasks_cancel_is_not_found` | B cannot CANCEL A's task |
| 4 | `v2_owner_isolation_holds_for_a_second_task_of_the_same_shape` | BOTH directions |
| 5 | `v2_a_guessed_task_id_is_not_found` | a never-minted id, on all three methods |
| 6 | `task_ids_are_unguessable` | the entropy / non-sequence / non-derivation PROPERTIES |
| 7 | `v1_local_and_v2_anonymous_buckets_are_disjoint` | one server, two eras, two buckets |
| 8 | `a_no_auth_provider_server_shares_one_v2_bucket` | D-07's accepted caveat, ASSERTED |

Every refusal is bytes off a loopback TCP socket through the real `StreamableHttpServer`, with
`AuthPosture::Required` (`BearerSubjects`) mapping two distinct bearers onto two distinct subjects.
`grep -c ClientBuilder` → **0**: the threat model here is a *valid* caller holding a guessed or
leaked id, which is exactly the frame a `pmcp::Client` cannot construct, because it only ever asks
about ids the server handed it.

### Indistinguishability is measured, not asserted

`assert_indistinguishable_not_found` checks **five independent facts** and the third is the one that
matters: it fires the SAME method a second time against `NEVER_MINTED`, in the same test, on the same
server, and compares `error.message` and `error.data` between the two. A hard-coded expected sentence
would prove only that the test and the code agree on a string; comparing two live answers means any
divergence — a different code, a different message, an extra payload — fails.

The other four: the code is `-32602`; both probes reach that code (or the equality would compare two
unrelated failures); the frame contains none of `owner` / `mismatch` / `forbidden` / the real owner's
subject; and the frame carries no `"status"`, `"createdAt"`, `"lastUpdatedAt"` or `"taskId"`.

### A refusal that still wrote would pass a code-only assertion

Each refusal test re-reads the task **as its owner** afterwards. `assert_record_untouched` compares
`taskId` / `status` / `createdAt` / `lastUpdatedAt`, and `lastUpdatedAt` is the load-bearing field:
every mutating path in `InMemoryTaskStore` rewrites it in the same write that changes the record, so
equality measures *no write landed* rather than *the one field I checked is unchanged*.

The helper asserts each field is **present** before comparing it. Two absent fields compare equal,
and the create payload and the `tasks/get` payload are built by two different projections that could
drift apart — without the presence check the comparison could silently become vacuous.

### The one fixture fact a future reader must not "simplify"

**Test 2 runs against a PAUSED task on purpose.** `TaskStore::task_input_snapshot` answers `NotFound`
for a task with no recorded `inputRequests`, so a `working` task refuses **every** caller — including
its owner — and a cross-caller refusal against one would be attributable to the missing request set
rather than to ownership. Negative control NC-2 confirms the distinction is real: with the update
path's owner scoping removed, B's delivery is ACKNOWLEDGED
(`{"result":{"resultType":"complete", …}}`), which is only observable because the task was feedable
in the first place. The paused fixture comes from 114-12's `pausing_task_tool`, reached through a
real `tools/call`.

Test 2 also reads the store directly after the refusal and asserts `input_responses` is **empty** and
the outstanding set still has one entry — `inputResponses` never appears in a `tasks/get` payload, so
the wire cannot show what a delivery persisted.

### Test 6 locks the property, never the format

The extension requires ids "generated with sufficient entropy that a third party cannot enumerate or
guess them". It does not mandate any identifier standard, and
`grep -c 'is_v4\|Version::Random' tests/v2_tasks_security.rs` → **0**. Three properties are measured
over 1024 ids minted through the real `TaskStore::create`:

1. **Entropy.** For each character position, count the DISTINCT characters observed and sum
   `log2(distinct)`. Constant positions — separators, a version marker, a fixed literal prefix —
   contribute exactly zero. The result is a LOWER bound on what the encoding realizes, and it is
   format-agnostic: a base32 or base64 id carrying the same entropy passes unchanged.
2. **Non-sequence.** The mint order is not the sort order (every monotonic generator — counter,
   timestamp, lexicographically-sortable id — produces one that is), no two ids are numerically
   adjacent, and the numeric conversion is asserted to have SUCCEEDED so a future encoding cannot
   make the check pass vacuously by returning `None`.
3. **Non-derivation.** The global fixed literal prefix is measured first (the spec permits one), then
   every mint-order-adjacent pair must share at most 8 further characters — calibrated in both
   directions: a uniform 16-symbol alphabet trips it with probability ≈1.5e-8, while a
   timestamp-prefixed encoding shares the top ~38 bits for ids minted in the same second and trips it
   every time. Plus: one owner's ids share no more of a prefix than the whole population does.

**Both directions of that bound were measured, not argued.** Raising the floor to 999 reported
`122.0` — the real generator's exact realized capacity, so the `>= 122.0` assertion sits precisely on
the bar with no slack. Replacing the generator with a counter (NC-7) reported `10.0` = log2(1024),
the counter's true entropy. The estimator reports the right *number*, not merely the right side of a
threshold.

A supporting observation, recorded and deliberately not asserted: today's ids are 128-bit random
values rendered 8-4-4-4-12, of which 6 bits are fixed markers — hence exactly 122.

`tasks/list` is retired on v2 (114-09 case 1 answers `-32601` without consulting any backend), so
there is no enumeration surface at all; unguessability plus owner-keying is the entire answer to
guessing.

### Test 7 separates two statements that are easy to confuse

* **Disjointness** is about the storage KEY. `pmcp-tasks` prefixes every key with its owner
  (`make_key` → `"{owner_id}:{task_id}"`), so `":<id>"` and `"local:<id>"` are different keys. The
  in-crate `InMemoryTaskStore` — the store this live server actually runs — reaches the same outcome
  through `validate_access`'s owner comparison, so the wire half measures it *there*, and the test
  says so rather than implying the key-prefix mechanism is in play.
* **`is_anonymous_owner`** is about whether an owner counts as anonymous for the `allow_anonymous`
  refusal, and it treats `""` and `"local"` IDENTICALLY. Not a contradiction: a production backend
  refuses BOTH buckets by default while keeping them in separate namespaces.

A v1 caller (`"local"`) and a v2 caller (`ANONYMOUS_PRINCIPAL`) each create a task on ONE
no-auth-provider server; neither can read the other's over the wire, and neither can at the store
either, with each reading its OWN as the control.

### Test 8 asserts the weakness on purpose

On a server with no auth provider, two v2 callers **do** share one bucket, and the test asserts it —
with the reason (such a server has no notion of caller identity to separate), the scope (the
fail-closed guarantee is about auth-configured deployments, row 2 of the identity table) and the
independent mitigation (`TaskSecurityConfig` defaults `allow_anonymous` to `false`, so
`GenericTaskStore` refuses that bucket outright) all in its rustdoc. It also asserts a caller with NO
credential reads the same task — the bearer was never interpreted, which is precisely why the bucket
is shared. If it ever fails, that is a deliberate behaviour change needing its own plan, not a fix to
this file.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — blocking] The plan's verify command selects ZERO tests**

- **Found during:** Task 1, first verification run.
- **Issue:** `cargo nextest run --features full -E 'test(/v2_tasks_security/)'` matches nextest's
  `test()` predicate against test NAMES, not binary names. No test in this file is *named*
  `v2_tasks_security`, so the command reports
  `Starting 0 tests across 94 binaries` and then `error: no tests to run`. The acceptance criterion
  "exits 0 with at least 6 tests" is therefore unsatisfiable as written — and with `--no-tests=pass`
  it would exit 0 having run nothing, which is worse.
- **Measured both ways:** `-E 'test(/cross_caller/)'` selects **3** (the three tests whose names
  contain it); `-E 'test(/v2_tasks_security/)'` selects **0**.
- **Fix:** every run in this plan used `-E 'binary(v2_tasks_security)'`, which selects all 8. This is
  the **fifth** measured plan-text defect in this phase (114-12 recorded the fourth).

**2. [Rule 2 — missing critical functionality] `assert_record_untouched` could compare two absent fields**

- **Found during:** Task 1, reviewing the first green run.
- **Issue:** the helper compared `before[field]` to `after[field]` on two payloads produced by two
  different projections (`v2_create_result_value` and `v2_get_response`). If either stopped carrying
  a field, `Value::Null == Value::Null` would pass while measuring nothing — and the "no write
  landed" claim is the entire evidence for tests 1 and 3.
- **Fix:** each field is asserted `is_string()` on BOTH sides before the equality, with the reason at
  the assertion.

**3. [Rule 3 — blocking] `make lint` is stronger than the test run**

- **Found during:** Task 2 verification. `doc_markdown` on `DynamoDB` in test 8's rustdoc.
- **Fix:** backticks. (`make lint` exit 0 thereafter, and again after every control revert.)

### Scope decisions, recorded rather than silently taken

- **The plan's Task 2 asked test 7 to assert `is_anonymous_owner` directly.** `pmcp-tasks` is a
  workspace member but **not a dependency of the root `pmcp` crate in any profile**, so a `pmcp`
  integration test cannot call it. The behavioural half was measured where it is reachable (the live
  socket + the in-crate store); the predicate half is a SOURCE tripwire over
  `crates/pmcp-tasks/src/store/generic.rs` and `.../backend.rs`. Adding
  `pmcp-tasks` to `[dev-dependencies]` is a manifest change that T-114-SC forbids this plan, so it is
  **deferred as D-114-O** with the interim mitigation named (114-07's
  `anonymous_owner_is_refused_by_default_on_this_backend` owns the behavioural twin from inside the
  crate that can execute it).

## Negative Controls — SIX, all reverted

Each applied to production source, measured with `--no-fail-fast`, then reverted from a `/bin/cp`
scratchpad snapshot verified with `shasum -a 256`. **`git checkout --` was not used** (114-14's
recorded self-inflicted loss) and **`git stash` was not used at any point**.

| # | Control | Site | Failing set | Count |
|---|---------|------|-------------|-------|
| NC-1 | owner scoping DISABLED on the `tasks/get` read | `InMemoryTaskStore::get` | 1, 4, 7 | 3 |
| NC-2 | owner scoping DISABLED on the `tasks/update` path | `task_input_snapshot` + `deliver_task_inputs` | **2** | 1 |
| NC-3 | owner scoping DISABLED on the `tasks/cancel` path | `InMemoryTaskStore::cancel` | **3** | 1 |
| NC-4 | wrong-owner made DISTINGUISHABLE from absent | `InMemoryTaskStore::validate_access` | 1, 2, 3, 4 | 4 |
| NC-5 | same CODE, varying MESSAGE | `store_error_response`'s v2 `NotFound` arm | 1, 2, 3, 4, 5 | 5 |
| NC-6 | "refused, but cancelled anyway" | `InMemoryTaskStore::cancel` | **3** | 1 |
| NC-7 | ids become SEQUENTIAL, same rendering | `InMemoryTaskStore::create` | **6** | 1 |

(Seven rows; NC-1…NC-4 are the plan's four, NC-5…NC-7 are the three this plan added.)

### The plan's per-method predictions: two of three correct

- **NC-2 and NC-3 each fail EXACTLY their own test**, as predicted — full orthogonality for the
  `tasks/update` and `tasks/cancel` guards.
- **NC-1 fails three, not one.** Not a finding of non-independence: tests 4 and 7 both make claims
  that *depend on* `tasks/get` owner scoping by construction (test 4 reads cross-caller through
  `tasks/get`; test 7's cross-era reads go through it too). The evidence NC-1 was run for is intact
  and stronger than the prediction — its failing set is DISJOINT from NC-2's and NC-3's, which is
  what "each method's guard is proven by its OWN control" requires. Tests 2, 3, 5, 6 and 8 stay
  green, so the three guards are separable.

### NC-4's prediction was wrong in an instructive way, and NC-5 exists because of it

The plan predicted NC-4 would make "tests 1-3 fail on the message equality assertion". Measured: all
four fail on the **CODE** assertion (fact 1), because `TaskStoreError::Internal` maps to `-32603`,
not `-32602` — the message-equality assertion never got the chance to fire. A control that fails a
test on assertion N proves only assertion N.

**NC-5** isolates it: `store_error_response`'s v2 `NotFound` arm renders `error.to_string()` instead
of the frozen constant, so the code stays `-32602` and only the message varies. Measured, tests 1–4
now fail on **fact 3** with the two live messages printed side by side:

```
wrong-owner: "task not found: 5c809e4c-7956-4b8a-a8fa-5a3edcd563b1"
absent:      "task not found: 3f2504e0-4f89-41d3-9a0c-0305e82c3301"
```

NC-5 also fails **test 5**, on its `!raw.contains(NEVER_MINTED)` id-echo assertion — evidence that
assertion is load-bearing too. Test 5 stays GREEN under NC-4, which is the "the oracle control leaves
test 5 alone" behaviour the plan asked for.

### NC-6 exists because test 3's whole point had no control

NC-3 fails test 3 at the *response* assertion (B gets an empty ack), so it never exercises the line
the test's own rustdoc calls load-bearing. NC-6 is the implementation that rustdoc describes: the
cancel WRITE lands under the record's real owner and the caller is then handed the byte-identical
`-32602` refusal. Every assertion about the response bytes passes; the test fails at line 511:

```
THE load-bearing assertion of this test: a refusal that still cancelled would pass every
assertion above it
  left: String("cancelled")   right: String("working")
```

### Masking check: RUN, did not fire

All seven failing sets are pairwise **distinct**. Two tests are failed by more than one control by
design (test 3 by NC-3 at the response and NC-6 at the re-read; tests 1–4 by both oracle controls at
two different facts), and in each case the two controls are separable by the failing ASSERTION, not
merely by the failing test — which is the stronger separation 114-19's split had to be forced into.

**Every one of the 8 tests is failed by at least one control except test 8** — which is failed by
none, BY CONSTRUCTION: it asserts an ACCEPTED behaviour, so its evidence value is that it stays
green. A control that made it fail would be a deliberate behaviour change, not a guard removal. This
is the same reasoning 114-19 recorded for its four v1 controls.

## Verification

| gate | result |
|------|--------|
| `make quality-gate` | **exit 0** — **4874 passed / 0 failed / 81 ignored across 291 result lines**; 0 non-`ok.` lines; 0 truncation markers; **0** occurrences of the D-114-A keychain flake |
| `make lint` | **exit 0** (run after every control revert) |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo nextest run -E 'binary(v2_tasks_security)'` | **8/8**, three consecutive runs, **0 LEAK** |
| all 12 `tasks` suites together | **136/136** |
| `git diff --stat HEAD~2..HEAD -- src/ crates/ Cargo.toml Cargo.lock fuzz/ examples/` | **EMPTY** |
| `git diff --stat HEAD~2..HEAD` | `tests/v2_tasks_security.rs \| 1123 ++++` — one file, one direction |
| `pmat analyze complexity --max-cognitive 25` | **5** at `.summary.violations`, **0 in `src/`**, **0** in the new file — identical to 114-14's set |
| `grep -c 'is_v4\|Version::Random' tests/v2_tasks_security.rs` | **0** |
| `grep -c ClientBuilder tests/v2_tasks_security.rs` | **0** |

**Gate reconciliation, exact.** The new `v2_tasks_security` binary is `Running` in **three** gate
legs — two filter to `running 0 tests`, one runs all 8. 288 + 3 = **291** result lines and
4866 + 8 = **4874** passed, matching 114-14's recorded baseline to the test. No lib tests were added,
so there is no doubled-row term this time.

**Why `cargo semver-checks`, `cargo public-api` and `make wasm-build` were NOT re-run.** The
production diff across both commits is **byte-empty** (`git diff --stat HEAD~2..HEAD -- src/ crates/
Cargo.toml Cargo.lock` prints nothing), and the only added file is `tests/v2_tasks_security.rs`,
which is `#![cfg(not(target_arch = "wasm32"))]` and is not part of any published surface. Those three
gates answer questions about production bytes that did not move; running them would produce a
tautology, and 114-14's results (223/223 no update required; Removed/Changed/Added none; wasm exit 0)
carry forward unchanged by construction rather than by re-measurement.

**Disk:** 86% used, 129 GiB free at plan start and end. Zero occurrences of the D-114-A keychain
panic in the full gate.

## Threat Model Coverage

| Threat | Disposition | Evidence |
|--------|-------------|----------|
| T-114-77 cross-caller task read (IDOR) | mitigated | three live-socket per-method probes with two real bearer principals; NC-1/NC-2/NC-3 each fail their own method's test with the other two green |
| T-114-78 owner-existence oracle | mitigated | message + `error.data` equality MEASURED in-test between wrong-owner and absent-id; NC-4 proves the code assertion catches a variant divergence, NC-5 proves the MESSAGE assertion catches a message-only one |
| T-114-79 cross-caller cancel/feed succeeding despite a refusal | mitigated | every refusal test re-polls as the owner; `lastUpdatedAt` equality measures "no write landed"; NC-6 is the exact "refused but cancelled anyway" implementation and fails only that assertion |
| T-114-80 v1/v2 anonymous bucket collision | mitigated | test 7 asserts the wire refusal in both directions, the store-level owner scoping, AND the `make_key` owner prefix — with `is_anonymous_owner`'s predicate equality asserted SEPARATELY and the distinction named |
| T-114-81 shared bucket on a no-auth server | **accept** | test 8 asserts it as accepted behaviour, with the reason, the scope and the `allow_anonymous: false` mitigation in its rustdoc |
| T-114-82 nextest LEAK noise masking real failures | mitigated | shared `teardown` (drop sockets → `abort()` → `await`); three consecutive runs, 0 LEAK |
| T-114-SC package installs | accepted | **zero** packages installed; `Cargo.toml` / `Cargo.lock` byte-unchanged |

**Threat surface scan:** no new network endpoint, auth path, file access or schema change — this plan
adds one test file and changes no production byte. No threat flags.

## Known Stubs

None.

## BLOCKING: TASK-05 security defect

**NONE FOUND.** All three v2 `tasks/*` methods are closed to a cross-caller over a real socket, the
refusals are indistinguishable from an absent id on both code and message, and no refusal performed
its write anyway. `deferred-items.md` gained no production-defect entry, and
`git diff --stat -- src/ crates/` is empty because nothing needed fixing.

The plan mandates that this heading exist and that **114-18 MUST NOT book TASK-05 (or run its
sign-off checkpoint) until any defect recorded here is closed by a follow-up security-fix plan**.
Recorded explicitly so the obligation is discoverable from the artifact: **there is no such defect,
so 114-18 is not blocked by this plan.** The one deferral this plan filed (D-114-O) is a test-reach
limitation, not a production defect, and it does not gate TASK-05.

## For the next plans

- **`.planning/REQUIREMENTS.md` is UNTOUCHED (0-byte diff)** and `requirements mark-complete` was
  deliberately NOT run. TASK-01…06 flip as a GROUP under the phase's contract-first waiver; the
  `## Verdict` stays `PENDING`. **114-18 owns the flip**, and TASK-05's live-socket evidence is now
  `tests/v2_tasks_security.rs` — cite the file, not this summary.
- **`114-SPEC-RECHECK.md` was deliberately NOT edited.** This plan landed no wire value; it CONSUMES
  114-09's identity table and 114-11's `-32602` mapping. Rows 16/17/18/19/20 are untouched.
- **`crates/pmcp-tasks` has a zero-byte diff.** Its owner scoping is asserted here at the SOURCE
  only, for the reason in D-114-O; the behavioural half lives in that crate's own suite.
- **A trap worth carrying, for anyone writing a `tasks/update` test:** `task_input_snapshot` answers
  `NotFound` for a task with no recorded `inputRequests`. A test that feeds a `working` task gets a
  refusal that is indistinguishable from a real security refusal and proves nothing. Use 114-12's
  `pausing_task_tool`, or `record_input_requests`, to get a genuinely feedable task first.
- **A nextest selector trap:** `-E 'test(/<file-stem>/)'` matches test NAMES and silently selects
  ZERO tests for a file whose tests are not named after it. Use `-E 'binary(<file-stem>)'`. Three
  plan files in this phase now carry the wrong form.
- **New deferral: D-114-O** — `pmcp-tasks` is a workspace member but not a dependency of `pmcp` in
  any profile, so a `pmcp` integration test cannot execute its code; cross-crate security claims
  split across two suites with a source tripwire as the seam.

## Self-Check: PASSED

- Artifacts on disk: `tests/v2_tasks_security.rs` (**1123 lines**, `min_lines: 250`) — FOUND.
- Commits reachable: `c05f562d`, `9522d3cc` — both FOUND in `git log`.
- `must_haves` contract greps: `Bearer` in `tests/v2_tasks_security.rs` → present (1, the two-principal
  header helper both callers use); `task_dispatch` referenced → present; 8 `#[tokio::test]` functions
  against a `min_lines: 250` / 6-test floor.
- Production diff `HEAD~2..HEAD -- src/ crates/` → **EMPTY**, as the plan's coverage-only fence requires.
