---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 31
subsystem: testing
tags: [subscriptions, http-08, http-04, live-socket, coverage-gap, falsifiability, resources]

# Dependency graph
requires:
  - phase: 113 (plan 10)
    provides: the `subscriptions/listen` route, the `tests/v2_subscriptions.rs` harness (`advertising`, `server_with`, `spawn_shared`, `SseStream`, `listen_body`, `subscription_id_of`) this plan reuses without rebuilding
  - phase: 113 (plan 23)
    provides: the same file's deterministic-teardown doctrine and the `subscriptionId` emission pins these tests build on rather than duplicate
  - phase: 113 (plan 32)
    provides: the pinned `advertisesSubscriptions` predicate whose fourth disjunct is the `resources.subscribe` capability these tests advertise
provides:
  - live-socket coverage for the RESOURCES half of HTTP-08's four capability opt-ins — the half addendum Finding 14(b) measured at zero
  - a wire-level proof that `SubscriptionFilter::covers`'s `ResourceUpdated` arm is URI-SELECTIVE, with the unsubscribed URI fired FIRST
  - a wire-level proof that the two resources opt-ins are INDEPENDENT, asserted in both directions of the capability cross-product
  - a wire-level observation of the `MAX_AGREED_RESOURCE_SUBSCRIPTIONS` truncation in a real acknowledgement, driven from the imported public constant
  - D-113-T — a measured, attributed record of pre-existing nextest `LEAK` noise in this file
affects: [113-28 (requirement wording checkpoint), HTTP-08 closure, HTTP-04 closure]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Selectivity is proven by firing the NEGATIVE case first: a test that fires only the positive case cannot distinguish 'filtered' from 'slow' and passes against an unconditional predicate"
    - "An OMITTED agreed field is asserted as key ABSENCE, never as a falsy value — `skip_serializing_if = \"Option::is_none\"` means omission and `false` are different wire contracts"
    - "A bound under test is IMPORTED from production, never transcribed: a future change to the constant moves the test with it instead of silently making it vacuous"
    - "Negative controls are chosen to be ORTHOGONAL — each mutation fails exactly ONE new test and passes the other three, which proves the tests are not duplicates of one another"

key-files:
  created: []
  modified:
    - tests/v2_subscriptions.rs
    - .planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md

key-decisions:
  - "Four new tests, not six: two per capability half, with the cross-product asserted in BOTH directions rather than as a third server configuration"
  - "The `resourcesListChanged` omission is asserted with `.get(..).is_none()` plus a whole-object equality, so both an added field and a `null`/`[]` agreed value fail"
  - "The truncation test's probe URIs are selected by INDEX (0 and `MAX_AGREED_RESOURCE_SUBSCRIPTIONS`), so the test states truncation semantics rather than a coincidence about particular strings"
  - "FIVE negative controls were run, not the plan's four: the truncation test short-circuits at its length assertion, so its DELIVERY consequence needed a supplementary control to be proven load-bearing at all"
  - "The pre-existing `LEAK` noise was RECORDED (D-113-T) rather than swept: the scope boundary forbids fixing pre-existing issues, and an eleven-test mechanical sweep inside a coverage plan would bury the coverage change it exists to make reviewable"
  - "HTTP-04 and HTTP-08 stay `[~]`; `.planning/REQUIREMENTS.md` was not opened and no checkbox was flipped (verified: 0-byte diff)"

patterns-established:
  - "Pattern: prove a coverage plan's tests falsifiable with a MATRIX of controls — run every control against every new test and record the full pass/fail split, because a control that fails all of them proves the tests redundant rather than load-bearing"
  - "Pattern: when a negative control makes a test short-circuit before its most interesting assertion, run a SUPPLEMENTARY control that disables the short-circuiting assertion, so the deeper assertion is proven load-bearing too"

requirements-completed: []  # HTTP-04 and HTTP-08 remain [~] — the STATE.md publication gate binds

# Metrics
duration: 19min
completed: 2026-07-27
---

# Phase 113 Plan 31: Live-Socket Coverage for the Resources Half of HTTP-08 Summary

**All four HTTP-08 capability opt-ins now have end-to-end wire coverage: `resourceSubscriptions` is proven URI-selective over a real socket with the unsubscribed URI fired FIRST, the two resources opt-ins are proven independent in both directions of the capability cross-product, and the `MAX_AGREED_RESOURCE_SUBSCRIPTIONS` truncation is observed in an acknowledgement a client actually read — each backed by an orthogonal negative control, with zero production bytes changed.**

## Performance

- **Duration:** 19 min
- **Started:** 2026-07-27T11:38:52Z
- **Completed:** 2026-07-27T11:57:46Z
- **Tasks:** 2/2
- **Files modified:** 1 test file (+ 1 planning record)

## The gap, and its closure

Addendum Finding 14(b) measured `grep -rn "resourceSubscriptions\|resources/updated" tests/ examples/` at **zero real hits** (binary `.wasm` matches only). Two of HTTP-08's four mandated opt-ins — `resourcesListChanged` and `resourceSubscriptions` — were exercised only by `#[cfg(test)]` unit tests inside `src/types/subscriptions.rs`, never over a socket, while the tools/prompts half already had live coverage.

**The measurement, re-run and inverted:**

```
$ grep -rn "resourceSubscriptions\|resources/updated" tests/ examples/
tests/v2_subscriptions.rs — 21 hits, 0 elsewhere
```

All 21 are in the four new tests and their section banner. `examples/` still returns nothing, which is correct — this is a wire-conformance surface, not an example-worthy API.

## Task Commits

1. **Task 1: URI-selective `resourceSubscriptions` delivery over a socket** — `09767af7` (test)
2. **Task 2: the resources capability cross-product and the agreed-list bound** — `233a0bba` (test)

## The four new tests

| Test | Pins |
|---|---|
| `resource_subscriptions_deliver_the_subscribed_uri_and_not_another` | `covers`'s `ResourceUpdated` arm is EXACT-STRING selective; the ack echoes the agreed URI list; the delivered frame carries the stream's `subscriptionId`; `mem://b` — fired FIRST — never arrives |
| `a_resource_subscriptions_stream_is_not_a_resources_list_changed_stream` | on a `resources.subscribe`-only server the requested `resourcesListChanged` is OMITTED (key absent) and `ResourcesChanged` — fired FIRST — is never delivered |
| `resources_list_changed_is_agreed_and_delivered_when_subscriptions_are_not` | the MIRROR: on a `resources.listChanged`-only server the list-changed half is agreed and delivered with its `subscriptionId`, and `resourceSubscriptions` is OMITTED (key absent, not `[]`, not `null`) |
| `an_over_bound_resource_subscriptions_list_is_truncated_and_reported` | a `MAX_AGREED_RESOURCE_SUBSCRIPTIONS + 1` request is ACCEPTED; the ack reports exactly `MAX` entries; index 0 survives and delivers, index `MAX` is absent and — fired FIRST — does not |

Every ordering assertion follows `no_unrequested_notification_types`'s discipline: **the negative case is fired first**, under one `server.lock()` so nothing can reorder the pair. A test that fired only the positive case would pass against a predicate that returned `true` unconditionally — which is exactly what negative control A demonstrates below.

Every test closes with the deterministic teardown 113-23 established for this file (`drop(stream); handle.abort(); let _ = handle.await;`).

## Falsifiability — the control matrix

Five controls were run. Each row records what was mutated, the resulting split across the four new tests, and the verbatim failure. **Every control failed exactly ONE test** — that orthogonality is itself the finding: it proves the four tests guard four different production arms rather than restating one behaviour four times.

| # | Production mutation | Split | Test that failed |
|---|---|---|---|
| A | `covers`'s `ResourceUpdated` arm → `true` unconditionally | 1 fail / 1 pass (run before task 2 existed) | `..._deliver_the_subscribed_uri_and_not_another` |
| B | `agreed_flag` ignores `supported` | 1 fail / 1 pass (run before task 2 existed) | `a_resource_subscriptions_stream_is_not_a_resources_list_changed_stream` |
| C | `intersect_with_capabilities`'s `resource_subscriptions` arm ignores the `resource_subscribe` flag | **1 fail / 3 pass** | `resources_list_changed_is_agreed_and_delivered_when_subscriptions_are_not` |
| D | the `.take(MAX_AGREED_RESOURCE_SUBSCRIPTIONS)` removed | **1 fail / 3 pass** | `an_over_bound_resource_subscriptions_list_is_truncated_and_reported` |
| D′ | D, **plus** the ack length/containment assertions temporarily weakened so the test reaches its delivery assertion | 1 fail | same test, at the DELIVERY assertion |

### A — `covers` made unconditional (T-113-150, the information-disclosure threat)

```
thread 'resource_subscriptions_deliver_the_subscribed_uri_and_not_another' panicked at tests/v2_subscriptions.rs:767:5:
assertion `left == right` failed: and it is the SUBSCRIBED URI that arrives first, despite mem://b having been fired before it: {"method":"notifications/resources/updated","params":{"uri":"mem://b","_meta":{"io.modelcontextprotocol/subscriptionId":51}},"jsonrpc":"2.0"}
  left: String("mem://b")
 right: String("mem://a")
```

The leak is observed directly: a subscriber that named `mem://a` receives `mem://b`. Note the fire-the-negative-first ordering is what makes this fail at all — with the subscribed URI fired first, the test would have read the right frame and passed.

### B — `agreed_flag` ignores server support

```
thread 'a_resource_subscriptions_stream_is_not_a_resources_list_changed_stream' panicked at tests/v2_subscriptions.rs:823:5:
an unsupported requested type is OMITTED from the agreed filter, not agreed as `false` and not emitted as `null`: {"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged","params":{"notifications":{"resourcesListChanged":true,"resourceSubscriptions":["mem://a"]},"_meta":{"io.modelcontextprotocol/subscriptionId":52}}}
```

Controls A and B are the exact inverse of one another — A fails test 1 and passes test 2, B fails test 2 and passes test 1. That complementarity is what proves test 2 is not a restatement of test 1.

### C — the cross-product (T-113-152, agreeing to an unadvertised capability)

```
thread 'resources_list_changed_is_agreed_and_delivered_when_subscriptions_are_not' panicked at tests/v2_subscriptions.rs:902:5:
`resources.subscribe` is NOT advertised, so the requested URI list is OMITTED from the agreed filter — the key must be ABSENT, not present as `[]` and not present as `null`: {"jsonrpc":"2.0","method":"notifications/subscriptions/acknowledged","params":{"notifications":{"resourcesListChanged":true,"resourceSubscriptions":["mem://a"]},"_meta":{"io.modelcontextprotocol/subscriptionId":53}}}
```

The other three tests all passed under C, including both `resources.subscribe`-advertising tests — correct, since the mutation only matters when the capability is OFF.

### D — the truncation bound removed (T-113-151, the DoS threat)

```
thread 'an_over_bound_resource_subscriptions_list_is_truncated_and_reported' panicked at tests/v2_subscriptions.rs:1004:5:
assertion `left == right` failed: 1025 URIs were requested; the acknowledgement reports exactly MAX_AGREED_RESOURCE_SUBSCRIPTIONS of them
  left: 1025
 right: 1024
```

### D′ — the supplementary control, because D was not enough

Under D the test short-circuits at its FIRST assertion, so the delivery consequence — "a URI that survived truncation delivers; one truncated away does not" — was never reached and was therefore **unproven**. Re-running D with the ack assertions temporarily weakened:

```
thread 'an_over_bound_resource_subscriptions_list_is_truncated_and_reported' panicked at tests/v2_subscriptions.rs:1030:5:
assertion `left == right` failed: a URI that survived truncation delivers; the one truncated away does not, even though it was fired first: {"method":"notifications/resources/updated","params":{"uri":"mem://r/1024","_meta":{"io.modelcontextprotocol/subscriptionId":54}},"jsonrpc":"2.0"}
  left: String("mem://r/1024")
 right: String("mem://r/0")
```

`mem://r/1024` is the URI at index `MAX_AGREED_RESOURCE_SUBSCRIPTIONS` — the first one past the bound — arriving on a stream whose agreed list should never have contained it. Both halves of the truncation test are now proven load-bearing.

### Restoration

All five mutations were reverted with targeted edits (never `git clean`, never a blanket reset, no `git stash`). Verified with the raw `/opt/homebrew/bin/git` binary because the rtk proxy corrupts diff and `wc` reads:

```
$ git diff -- src/ | wc -c
0
$ grep -c 'SUPPLEMENTARY-CONTROL' tests/v2_subscriptions.rs
0
$ git diff dafde230..HEAD --stat
 tests/v2_subscriptions.rs | 398 +++++++++++++++++++++++++++++++++++++++++++++-
 1 file changed, 395 insertions(+), 3 deletions(-)
```

The suite was re-run green after every restoration.

## Production defects discovered

**None.** Every assertion the four tests make passed against unmodified production source on the first run, and each was then proven to be checking something real rather than passing vacuously. The scope fence's "record, do not fix" branch was therefore not exercised for any production behaviour.

One nuance worth carrying to 113-28, already noted by 113-32 and re-confirmed here by observation rather than by reading: the capability that gates `resourceSubscriptions` is `resources.subscribe` (what `advertising(Some("resources.subscribe"))` sets and what `intersect_with_capabilities` reads), while HTTP-08's prose names `resourceSubscriptions`, a `SubscriptionFilter` field. Test 3's mutation-C failure message is the wire-level demonstration of that mapping. Implementation and conformance predicate agree; only the requirement's prose blends two vocabularies. **Not "fixed" here** — it is 113-28's.

## Deviations from Plan

### Auto-fixed / additive

**1. [Rule 2 — missing critical verification] A fifth negative control (D′), because the plan's fourth could not reach the assertion it was meant to falsify**

- **Found during:** Task 2, running control D.
- **Issue:** The plan asked for one control per new behaviour. Removing `.take(..)` fails the truncation test at its ACK-length assertion, which short-circuits the test before its delivery assertions ever run. The plan's own `<behavior>` requires "a URI that survived truncation still delivers; a URI that was truncated away does not" — under control D alone, that half was as unproven as it was before the test existed. This is precisely the vacuous-assertion failure mode T-113-154 exists to prevent, reappearing one level down.
- **Fix:** Re-ran control D with the ack assertions temporarily weakened, captured the delivery-side failure (recorded verbatim above), and restored both the test and the production source.
- **Files modified:** none permanently — both mutations reverted; `git diff -- src/` is 0 bytes and the scaffold marker count is 0.

### Recorded, NOT fixed

**2. [Scope boundary] Pre-existing nextest `LEAK` noise on four older tests in this file → D-113-T**

- **Found during:** Task 2's stability sweep.
- **Measurement:** 16 consecutive full-suite runs. **Zero** leaks on any of the four new tests. **4 leaks across 12 runs**, each on a different PRE-EXISTING test: `absent_capability_is_conformant`, `advertise_implies_serve`, `listen_stream_protocol`, `disconnect_releases_registry_slot`. Six runs of the 15 pre-existing tests alone produced zero leaks, so this is a load-dependent teardown race that a slightly busier suite makes visible rather than a defect the new tests introduce.
- **Cause:** eleven pre-existing tests end with a bare `handle.abort()` and never await the aborted accept loop — the exact condition 113-23 fixed on its own tests.
- **Disposition:** recorded in `deferred-items.md` as **D-113-T** with the measurement and the one-line remedy. NOT fixed: the scope boundary forbids fixing pre-existing issues, and an eleven-test mechanical sweep inside a coverage plan would bury the coverage change it exists to make reviewable. A `LEAK` is still a PASS, so this is noise, not failure.

---

**Total deviations:** 1 additive (a stronger control than the plan asked for), 1 recorded-not-fixed.
**Impact on plan:** none adverse. No scope creep, no production behaviour changed, the plan's `files_modified` list held exactly.

## Verification Results

| Check | Result |
|---|---|
| `cargo nextest run --features full --test v2_subscriptions` | **19 passed, 0 skipped** (15 pre-existing + 4 new); all four new names appear in the output |
| `cargo nextest run --features full --lib -- subscriptions` | **87 passed** |
| `cargo nextest run --features full --test v2_subscriptions_client` | **8 passed** — untouched |
| Negative controls | **5 run, 5 recorded verbatim, all restored**; each failed exactly one new test |
| `git diff -- src/` after restoration | **0 bytes** |
| `cargo clippy --features full --lib --tests -- -D clippy::all` | exit **0** |
| `make lint` (pedantic + nursery + cargo, `RUSTFLAGS=-D warnings`) | exit **0**, zero warnings — run explicitly per the standing constraint, not just the plan's narrower command |
| `cargo fmt --all -- --check` | clean (rustfmt applied once during task 1, then re-verified) |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **223 checks: 223 pass, 30 skip — no semver update required** |
| `make quality-gate` (background job, polled to completion) | **`QUALITY_GATE_EXIT=0`** — "ALL TOYOTA WAY QUALITY CHECKS PASSED" |
| quality-gate aggregate totals (read from the raw log) | **4416 passed; 0 failed; 80 ignored** — lib 1630, integration/doc 399 (+78 ignored) |
| `grep -rn "resourceSubscriptions\|resources/updated" tests/ examples/` | **21 hits in `tests/v2_subscriptions.rs`** — Finding 14(b)'s zero, inverted |
| `git diff dafde230..HEAD --stat` | exactly **1 file**, `tests/v2_subscriptions.rs`, +395/-3 |
| `.planning/REQUIREMENTS.md` | **untouched** (raw-git diff = 0 bytes); no checkbox flipped |
| `MAX_AGREED_RESOURCE_SUBSCRIPTIONS` in the test file | 7 occurrences — imported, never transcribed as `1024` |
| SATD scan (`TODO\|FIXME\|HACK\|XXX`) in the diff | **0** |

Totals were read from the raw log files, not from proxied terminal output, because the rtk shell proxy swallows `test result:` lines.

## Threat Model Disposition

| Threat ID | Disposition | Evidence |
|---|---|---|
| T-113-150 (information disclosure — over-broad `covers`) | **mitigated** | `..._deliver_the_subscribed_uri_and_not_another`, with control A observing the leak directly (`mem://b` delivered to a `mem://a` subscriber) |
| T-113-151 (DoS — unbounded retained URI list) | **mitigated** | `an_over_bound_..._truncated_and_reported`, with controls D and D′ proving BOTH the reported bound and its delivery consequence load-bearing |
| T-113-152 (spoofing — agreeing to an unadvertised capability) | **mitigated** | the cross-product asserted in both directions, key ABSENCE not falsity, with control C |
| T-113-153 (a coverage plan silently fixing what it discovers) | **mitigated** | zero production bytes changed (`git diff -- src/` = 0); no defect was found to tempt a fix, and the one pre-existing test-hygiene issue was recorded as D-113-T rather than swept |
| T-113-154 (vacuously-passing new assertions) | **mitigated** | five controls, each failing exactly one test; the orthogonality is recorded because a control failing all four would have proven the tests redundant. D′ exists because control D alone left half of one test unproven |

## Scope fence — held

- **Zero production files changed.** `git diff -- src/` is 0 bytes; the whole plan diff is one test file.
- **No requirement checkbox flipped.** `.planning/REQUIREMENTS.md` was never opened for editing; HTTP-04 and HTTP-08 remain `[~]`.
- **No URI prefix or normalisation semantics asserted.** `covers` is exact string equality, and every test says so.
- **113-23's additions were extended, not duplicated.** The `subscriptionId` assertions here are per-frame presence-and-equality checks incidental to delivery; the three-class emission pin remains 113-23's and is not restated.
- **No `git stash` subcommand was run** at any point.

## Next Phase Readiness

- **HTTP-08's coverage asymmetry is closed.** All four opt-ins now have live-socket coverage. What remains open on HTTP-08 is not coverage but **provenance** — the advertise-implies-serve rule is conformance-suite policy pinned by 113-32's second gate arm, and the ⚠ caveat block stands.
- **HTTP-04 and HTTP-08 stay `[~]`.** The STATE.md publication gate is unchanged by this plan.
- **New for the next owner:** **D-113-T** (pre-existing `LEAK` teardown noise, measured and attributed). Still open and untouched here: D-113-Q, D-113-R, D-113-S, D-113-F, D-113-G, D-113-H, WR-01/02/04, UNAS-01.

## Known Stubs

None. Every new test drives a real server over a real loopback socket and asserts on frames that crossed it. No placeholder values, no mocked delivery, no assertion on a constructed-but-unsent notification — which is the specific failure mode this plan existed to avoid reproducing.

## Threat Flags

None. This plan added test code only: no network endpoint, no auth path, no file-access pattern, no schema change.

## Self-Check: PASSED

- `tests/v2_subscriptions.rs` — FOUND, contains `resourceSubscriptions` (the artifact's declared `contains` marker), `intersect`-arm coverage, and 7 references to `MAX_AGREED_RESOURCE_SUBSCRIPTIONS`.
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md` — FOUND, carries the new `D-113-T` section.
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-31-SUMMARY.md` — FOUND.
- Commit `09767af7` — FOUND in `git log`.
- Commit `233a0bba` — FOUND in `git log`.
- All three `key_links` patterns verified present in the test file: `resources/updated` (the `covers` link), `intersect` (the capability-intersection link, in the section banner and control-C prose), `MAX_AGREED_RESOURCE_SUBSCRIPTIONS` (the truncation link).

---
*Phase: 113-stateless-http-multi-round-trip-elicitation*
*Completed: 2026-07-27*
</content>
</invoke>
