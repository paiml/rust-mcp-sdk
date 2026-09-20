---
phase: 114-tasks-extension-migration
plan: 14
subsystem: api
tags: [tasks, mcp-2026-07-28, tasks-update, input-delivery, mrtr, kind-directed-decode, dos-bounds, fuzzing, proptest]

requires:
  - phase: 114-13
    provides: "the tasks/update ordered gate chain (era, backend, declaration, auth, params) and the RAW-params internal route"
  - phase: 114-07
    provides: "the pmcp-tasks generic-store half of deliver_task_inputs / task_input_snapshot"
  - phase: 114-04
    provides: "TaskStore::deliver_task_inputs, TaskStore::task_input_snapshot, TaskInputSnapshot::kind_of, supports_inputs, partition_input_delivery"
provides:
  - "tasks/update delivers client input end to end: RAW parse -> four DoS bounds -> kind-directed decode against the SERVER-recorded kinds -> the backend's single atomic write -> an EMPTY acknowledgement"
  - "check_input_responses_map_bounds — ONE whole-map bounds function shared by the MRTR ingress and the tasks route"
  - "the v2 result envelope on the internal-request route, so the ack carries resultType: complete (SPEC-RECHECK row 19, CLOSED)"
  - "tests/v2_tasks_update.rs — 17 raw-frame delivery-semantics tests"
  - "a depth-bounded proptest block plus two const _: () = assert! bound-relationship locks in src/server/task_dispatch.rs"
  - "fuzz/fuzz_targets/fuzz_tasks_update.rs — the untrusted params boundary, with a recorded and FALSIFIED campaign"
  - "pmcp::testing re-exports of the four inputResponses bounds (and deliberately NOT the fifth)"
affects: [114-15, 114-16, 114-17, 114-18, 114-20]

tech-stack:
  added: []
  patterns:
    - "Parse RAW, bound, THEN decode kind-directed — never deserialize into a type whose Deserialize impl is the untagged guess"
    - "The persisted task record is the tasks analogue of Phase 113's AEAD-sealed continuation: server-minted kinds the client cannot choose"
    - "A refusal names a key only when the key came from the RECORD (get_key_value on the trusted side); a client-chosen key is carried programmatically and never rendered"
    - "The internal-request route must inject the v2 result envelope itself — it bypasses process_client_request"
    - "A fuzz target re-derives its invariants from its OWN parse, so a green campaign is evidence rather than production agreeing with itself"

key-files:
  created:
    - tests/v2_tasks_update.rs
    - fuzz/fuzz_targets/fuzz_tasks_update.rs
  modified:
    - src/server/task_dispatch.rs
    - src/types/mrtr.rs
    - src/server/mod.rs
    - src/server/streamable_http_server.rs
    - src/testing/mod.rs
    - tests/v2_tasks_update_routing.rs
    - fuzz/Cargo.toml

key-decisions:
  - "inputResponses is parsed into a RAW serde_json::Map, never into InputResponses — that type's Deserialize IS try_from_value_untagged, so typing it at ingress reintroduces D-113-O one layer earlier AND before any bound has run"
  - "extract_input_responses was refactored to call the shared whole-map bounds check FIRST, making the MRTR ingress bounds-before-decode too; the only observable change is that an over-bound entry now wins over an earlier undecodable one"
  - "The router leg receives params VERBATIM and owns its own decode — a TaskRouter holds its own record and the trait has no snapshot accessor (deferred as D-114-M)"
  - "A store that cannot accept inputs on a router-less server reuses the FROZEN TASKS_NOT_ENABLED message rather than minting a fifth -32601 sentence (deferred as D-114-N)"
  - "Server::handle_tasks_update injects the v2 result envelope directly, as build_discover_response already does — measured, not predicted: the ack reached the wire as a bare {} with no resultType"
  - "The fuzz seam is gated cfg(any(feature = \"fuzzing\", test)) — doc(hidden) alone is invisible to cargo public-api and therefore a vacuous fence (113-19)"
  - "FOUR bounds tests, not five: MAX_REQUEST_STATE_LEN bounds the continuation token and tasks/update carries none"

patterns-established:
  - "Bounds-first ordering proven by payloads that violate a bound AND carry an undecodable value, so 'the bound won' is a statement about ORDER"
  - "The raw-map boundary proven from BOTH sides with one value: over-bound -> bounds error, within-bounds -> kind refusal"
  - "Negative-control masking check: nine controls, all failing sets pairwise distinct, every test failed by at least one"

requirements-completed: []

duration: 195min
completed: 2026-07-31
---

# Phase 114 Plan 14: tasks/update Input Delivery Summary

**`tasks/update` now delivers client input end to end — bounded before it is decoded, decoded only against the kinds the server itself recorded, transitioned atomically by the backend, and acknowledged empty — with nine negative controls, a depth-bounded property test, and a fuzz campaign proven falsifiable.**

## Performance

- **Duration:** ~195 min
- **Tasks:** 3 of 3
- **Files created:** 2 · **Files modified:** 7
- **Commits:** 4

## Accomplishments

### The delivery, in the order the order matters

`TaskDispatch::route_tasks_update` gained cases 5–7 after 114-13's four gates:

5. **Parse RAW.** `TasksUpdateParams<'a>` holds a `String` task id (resolved through
   `TASK_NAME_BEARING_METHODS`, not by re-spelling `taskId`) and a **borrowed**
   `&serde_json::Map<String, Value>`. It borrows rather than clones because a clone here would
   duplicate up to the 256 KiB total budget *before* that budget has been checked.
6. **Bound.** All four `inputResponses` MRTR bounds, over the raw map, **before any decode**.
7. **Deliver.** Owner-scoped snapshot → kind-directed decode → the backend's single atomic write →
   an empty ack.

### Why the map stays raw — the load-bearing decision

`InputResponses` is `BTreeMap<String, InputResponse>`, and `InputResponse`'s `Deserialize` impl
**is** `try_from_value_untagged`: try `ListRootsResult`, then `CreateMessageResult`, then
`ElicitResult`, first that fits. `ElicitResult` and `CreateMessageResult` structurally overlap.
That is D-113-O verbatim — an elicitation answer silently reclassified as sampling, the handler's
`Elicitation` arm never matching, sixteen re-elicitations, and a misleading death.

Deserializing `params` straight into `InputResponses` would run that guess **at ingress** — one
layer earlier than the route exists to prevent it, and before a single bound had fired. So the
route reads the map raw and never calls the untagged decoder:
`grep -c 'try_from_value_untagged' src/server/task_dispatch.rs` → **2**, both inside rustdoc
explaining that it must never be called here (non-comment count: **0**).

### The four bounds, shared rather than re-derived

`check_input_response_bounds` moved private → `pub(crate)`, and a new
`check_input_responses_map_bounds` in `src/types/mrtr.rs` composes it with the count and running-total
checks. `grep -c 'fn check_input_response_bounds' src/` → **1**. No new constant is introduced:
`git diff … | grep -cE '^\+.*const MAX_[A-Z_]+'` → **0** in `task_dispatch.rs`.

`MAX_REQUEST_STATE_LEN` is deliberately excluded and deliberately **not** re-exported beside the
other four — it bounds the continuation token and `tasks/update` carries none. Exporting it next to
its four siblings is how a fifth test gets written asserting a bound the route correctly does not
enforce.

**A bonus the refactor bought (and its one behavioural consequence):** `extract_input_responses`
now calls the shared whole-map check first instead of interleaving one bound with one decode, so
the **MRTR ingress is bounds-before-decode too**. The only observable change: for a payload where an
earlier entry is undecodable and a later one is over-bound, the answer is now the BOUND rather than
the decode. That is the cheaper refusal and the one an attacker is actually probing. All 212 mrtr +
protocol-context unit tests pass unchanged.

### Kinds come from the record, and only the record

`decode_inputs_against_record` reads each key's kind from `TaskInputSnapshot::input_requests` — the
owner-scoped 114-04 accessor, which is the *only* supported way to reach them (`TaskStore::get`
returns the wire `Task` alone; `TaskRecord` is private) — and types the value with
`InputResponse::decode_for`.

| the record… | the value… | outcome |
|---|---|---|
| does NOT hold the key | anything | **IGNORED** (never issued / already answered / superseded) |
| HOLDS the key | decodes as the recorded kind | accepted |
| HOLDS the key | does NOT decode as that kind | **REFUSED** `-32602` |

The refused key is taken via `get_key_value` on the **record** side, so the rendered string is
provably server-assigned even though the two are equal by construction. An ignored key is
client-chosen by definition and is never rendered anywhere. No value is ever rendered.

### The empty acknowledgement, and the trap under it

`UpdateTaskResult = Result`. The ack is `{}` plus whatever the envelope writes; no wait and no
re-read was inserted to make it look synchronous (the same cooperative, eventually-consistent
reasoning `route_tasks_cancel` already records).

**The trap, measured off a real socket rather than predicted:** `tasks/update` rides the
crate-private internal-request route, which **bypasses `process_client_request`** — the site where
every `ClientRequest` result gets its `resultType` + `_meta.serverInfo`. The first green run of
`tasks_update_ack_is_empty` read `Null` where `"complete"` was required. `Server::handle_tasks_update`
now calls `inject_v2_result_envelope` directly, exactly as `build_discover_response` already did for
the other internal route (Phase 112). **Any future method added on that route inherits the same
trap.** Recorded in `114-SPEC-RECHECK.md` row 19, which this plan CLOSES.

### `tests/v2_tasks_update.rs` — 17 tests, all raw frames

`grep -c 'ClientBuilder' tests/v2_tasks_update.rs` → **0**. A `pmcp::Client` builds its
`inputResponses` from the very `inputRequests` the server sent, so it cannot produce a mismatched
answer by construction; no client-based test could ever have caught D-113-O.

Two fixture facts written into the file so a future reader does not "fix" them:

- **`sampling_shaped_answer` omits `action` deliberately.** `ElicitResult` carries no
  `deny_unknown_fields` and its `content` is `Option<HashMap<..>>`, so an `{action, content, model}`
  object **IS** a valid `ElicitResult` — the "obvious" three-key fixture would make the negative
  test vacuous. Dropping `action` makes the value exclusively a `CreateMessageResult`, which is
  also exactly what the untagged decoder classifies as `Sampling`.
- **The bounds tests build payloads FROM the production constants**, newly re-exported through
  `pmcp::testing`. A hand-typed `65` stops crossing the bound the moment the constant moves, and
  does so silently.

Test 2 asserts **both** halves of partial delivery — the status stays `input_required` **and** the
response is persisted. Asserting only the status would pass against an implementation that
acknowledged and dropped the payload on the floor.

### Property test and compile-time locks

An in-module `proptest!` block in `src/server/task_dispatch.rs` beside the deterministic tests it
generalizes (the `mrtr.rs` precedent), with a depth-bounded strategy and **two**
`const _: () = assert!` locks:

- `STRATEGY_RECURSION + 1 < MAX_INPUT_RESPONSE_DEPTH` — the generator can never build a value the
  depth bound would refuse, so "a bounded map is never refused" cannot fail for a reason unrelated
  to the code.
- `MAX_INPUT_RESPONSE_DEPTH < MAX_CANONICAL_DEPTH` — the ingress bound stays strictly tighter than
  the AAD canonicalization cap, so a value cannot pass ingress and then be refused deeper in.

A fifth test, `the_generator_cannot_cross_a_size_or_depth_bound`, states the generator's own
precondition explicitly, because the two properties depend on it.

### Fuzz target and a FALSIFIED campaign

`fuzz/fuzz_targets/fuzz_tasks_update.rs` drives arbitrary bytes through the route's pure prefix
against a FIXED synthetic record (one key per kind). Four numbered invariants in the module rustdoc
plus a corpus-seeding list. **Invariants 2 and 4 are re-derived inside the target** from its own
parse — its own depth walk, its own required-field sets — so a green campaign is evidence about the
shipped code rather than production agreeing with itself.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — missing critical functionality] The `UpdateTaskResult` ack carried no `resultType`**

- **Found during:** Task 2, first run of `tasks_update_ack_is_empty`
- **Issue:** `tasks/update` rides the internal-request route and bypasses `process_client_request`,
  so the ack reached the wire as `{"jsonrpc":"2.0","id":2,"result":{}}` — no `resultType` at all,
  while the extension says it MUST be `"complete"` (SPEC-RECHECK row 19). The 114-19 client
  tolerates an absent value (Phase 112's absent-means-complete), which is exactly why this would
  have shipped unnoticed.
- **Fix:** `Server::handle_tasks_update` calls `inject_v2_result_envelope` with
  `ResponseDisposition::Complete` + `ReservedFieldOwner::None`, mirroring `build_discover_response`.
  A no-op on v1 and for every error payload, so all seven refusals are byte-unchanged.
- **Files:** `src/server/mod.rs` · **Commit:** `6cc608bd`

**2. [Rule 3 — blocking] `tests/v2_tasks_update_routing.rs` asserted the deleted placeholder**

- **Found during:** Task 1
- **Issue:** `tasks_update_reaches_dispatch_on_v2` asserted `-32603 "input delivery is not
  implemented"`, and its `inputResponses` fixture was an `ElicitResult` shape under the harness's
  `roots/list` key — which the new kind-directed decode correctly refuses.
- **Fix:** fixture corrected to `{"roots": {"roots": []}}`; the test now asserts an ack plus every
  refusal that must NOT happen, named individually, so it stays a *routing* test rather than
  becoming a second copy of the delivery suite.
- **Files:** `tests/v2_tasks_update_routing.rs` · **Commit:** `aa651f74`

**3. [Rule 3 — blocking] The bound constants were unreachable from an integration crate**

- **Found during:** Task 2
- **Issue:** the four bounds are `pub(crate)`; the plan requires the tests to import them from
  production rather than spell the numbers.
- **Fix:** four `pub const` re-exports in `src/testing/mod.rs` (feature-gated behind `testing`, so
  `cargo public-api` with default features reports **zero** added items). `MAX_REQUEST_STATE_LEN` is
  deliberately NOT re-exported, with the reason written at the block.
- **Files:** `src/testing/mod.rs` · **Commit:** `6cc608bd`

**4. [Rule 3 — blocking] `make lint` is stronger than `cargo clippy -- -D warnings`**

- **Found during:** verification. Bare clippy was clean; `make lint` (pedantic + nursery,
  `--lib --tests`) found three: two `doc_markdown` backticks, one `items_after_statements`, and
  `unreachable_pub` + `dead_code` on the fuzz seam under `cfg(test)` alone.
- **Fix:** backticks; constant hoisted to module scope; the seam's allow scoped to
  `not(feature = "fuzzing")` so the **real** fuzz configuration keeps full dead-code and
  reachability analysis (the 114-10 scoping discipline).
- **Files:** `src/server/task_dispatch.rs`, `tests/v2_tasks_update.rs` · **Commit:** `e3f4dbd1`

**5. [Rule 1 — self-inflicted, recovered] `git checkout --` destroyed uncommitted work**

Reverting negative control NC-1 with `git checkout -- src/server/task_dispatch.rs` reverted the file
to the last COMMIT, discarding the not-yet-committed property-test module. Recovered by
re-appending, then verified green. Every subsequent control reverted from a `/bin/cp` snapshot in the
scratchpad, with `shasum -a 256 -c` after each. **`git stash` was not used at any point.** The
lesson generalises: `git checkout --` is only a safe revert when the pristine state is the last
commit.

### Behaviour deliberately changed (not a deviation, but worth naming)

`extract_input_responses` now bounds the whole map before decoding any entry. For a payload with an
earlier undecodable entry and a later over-bound one, the reported error moves from `Undecodable` to
the bound. No existing test asserted that combination; all 212 mrtr/protocol-context unit tests and
the full gate pass.

## Negative Controls — NINE, all reverted

Each applied, measured with `--no-fail-fast`, then reverted from a scratchpad snapshot with
`shasum -a 256 -c` OK on all touched files.

| # | Control | Failing set | Count |
|---|---------|-------------|-------|
| NC-1 | `decode_for` → `try_from_value_untagged` | `kind_directed_refuses_a_sampling_shape…`, `never_runs_the_untagged_decoder_on_ingress`, *(lib)* `ignore_and_refuse_are_different_answers` | 3 |
| NC-2 | bounds moved AFTER the decode | all four `bounds_fire_before_the_decode_*`, `never_runs_the_untagged_decoder_on_ingress` | 5 |
| NC-3 | transition on a PARTIAL set (`delivery.complete` dropped) | `partial_set_stays_input_required`, `ignores_an_already_answered_key` | 2 |
| NC-4 | unrecorded key becomes an ERROR instead of IGNORED | `ignores_a_key_that_was_never_issued` | 1 |
| NC-5 | bypass `store_error_response` (verbatim store error on v2) | `for_another_owner_is_not_found` | 1 |
| NC-6 | ack carries a task body | `ack_is_empty`, *(lib)* `the_update_ack_carries_no_fields` | 2 |
| NC-7 | store's terminal-status guard removed | `on_a_completed_task…`, `on_a_failed_task…`, `on_a_cancelled_task…`, `cas_first_writer_wins` | 4 |
| NC-8 | kind lookup ignores the record (always `Roots`) | `kind_directed_accepts_an_elicitation_answer…` | 1 |
| NC-9 | store never transitions | `completes_the_outstanding_set`, `kind_directed_accepts…`, `cas_first_writer_wins` | 3 |

**Masking check: RUN, did not fire.** All nine failing sets are pairwise **distinct**. Two tests are
failed by more than one control by design — `never_runs_the_untagged_decoder_on_ingress` is one
property asserted from two sides (NC-1 the kind side, NC-2 the bounds side), and
`cas_first_writer_wins` depends on both the terminal guard and the transition.

**Every one of the 17 integration tests is failed by at least one control.** That was not true after
the plan's three; NC-4 through NC-9 exist because ignore-semantics, the oracle-free refusal, the
empty ack, the terminal refusals, the kind-direction POSITIVE case and the transition itself would
otherwise have been recorded as "locked" with no evidence they are load-bearing.

**Two of the plan's three predictions were WRONG.**

- NC-1: predicted "test 5 fails, tests 1–3 stay green". It fails **two** integration tests plus a
  lib test — `never_runs_the_untagged_decoder_on_ingress` is the second, by construction. Tests 1–4
  do stay green, as predicted.
- NC-3: predicted "test 2 FAILS **only**". It fails **two**. With the guard dropped, the first
  (partial) delivery in `ignores_an_already_answered_key` resumes the task, so the replay hits
  `InvalidTransition` instead of the ack it asserts. A fixture dependency, not a second independent
  property — which is precisely why the isolating controls exist.
- NC-2 was the one correct prediction (it fails the bounds group).

## Fuzz Campaign — recorded, and FALSIFIED

**Toolchain note.** `cargo fuzz` requires nightly; this repo defaults to stable, where
`cargo fuzz build` fails with *"the option `Z` is only accepted on the nightly compiler"*. A nightly
toolchain **is** installed here (`nightly-aarch64-apple-darwin`), so the campaign was really run
under `cargo +nightly`. `make quality-gate` does not build `fuzz/`, which is why the
`cfg(test)` half of the seam's gate exists — `the_fuzz_seam_answers_every_verdict` drives all four
verdicts through the same entry point the target uses, so the seam cannot rot silently.

| item | value |
|------|-------|
| build | `cargo +nightly fuzz build fuzz_tasks_update` — **exit 0**, 0 warnings |
| runs | **20 000** (`-runs=20000`) |
| seed | `-seed=114014` (fixed, so the campaign replays) |
| exit status | **0** |
| final coverage | `cov: 1960 ft: 5379 corp: 389/6757Kb`, ~10 000 exec/s |
| corpus | 19 hand-seeded cases → **616** after the campaign |
| artifacts dir | exists and is **EMPTY** (`find fuzz/artifacts/fuzz_tasks_update -type f \| wc -l` → 0) |

**Falsifiability control (113-19's discipline: a green campaign can coexist with an open gap).**
Deleting the seam's bounds pre-check makes the campaign **CRASH**:

```
thread '<unnamed>' panicked at fuzz_targets/fuzz_tasks_update.rs:141:5:
accepted 65 entries, over the 64 bound — the bounds pre-check did not run
==56456== ERROR: libFuzzer: deadly signal
Test unit written to fuzz/artifacts/fuzz_tasks_update/crash-babbf5d4a1d894ea1314f9bf593893f3bef6efed
```

Exit **1**, a named assertion, a written artifact. Reverted; the artifacts directory was removed and
the campaign re-run to exit 0 with an empty artifacts directory.

`fuzz/Cargo.toml` gains a `[[bin]]` block only —
`git diff fuzz/Cargo.toml | grep -cE '^\+.*=.*version'` → **0**, and
`git diff --stat -- Cargo.lock` is **empty**.

## Verification

| gate | result |
|------|--------|
| `make quality-gate` | **exit 0** — 4866 passed / **0 failed** / 81 ignored across 288 result lines; 0 non-`ok.` lines; 0 truncation markers; **0** occurrences of the D-114-A keychain flake |
| `make lint` | **exit 0** |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo nextest run --features full` (tasks suites) | 119/119 |
| `tests/v2_tasks_update.rs` | **17/17** |
| `tests/v2_tasks_update_routing.rs` | 18/18 |
| `src/server/task_dispatch.rs` unit + property | 8/8 |
| `cargo nextest run -p pmcp-tasks` | **514/514**, `crates/pmcp-tasks` zero-byte diff |
| `cargo semver-checks --baseline-rev aa651f74^` | **223/223, no semver update required** |
| `cargo public-api diff aa651f74^..HEAD` | Removed **(none)**, Changed **(none)**, Added **(none)** |
| `make wasm-build` | **exit 0**, 92 lib warnings |
| `pmat analyze complexity --max-cognitive 25` | **0 violations in `src/`**; 5 at `.summary.violations`, all inherited |
| `cargo +nightly fuzz build/run fuzz_tasks_update` | exit 0 / exit 0, artifacts empty |
| `Cargo.toml` / `Cargo.lock` | byte-unchanged |

**Gate reconciliation.** The new `v2_tasks_update` binary is `Running` in **three** gate legs — two
filter to `running 0 tests`, one runs all 17 → **+3 result lines, +17 passed**. The 8 new lib tests
are counted **twice** (16 rows) → **+16**. Total attributable to this plan: **+3 lines, +33 passed**.

**The one `make wasm-build` warning naming a touched symbol, and why it is not a regression.**
`check_input_responses_map_bounds is never used` — a NEW member of a pre-existing, 38-strong dead
block. On wasm32 essentially all of `src/types/mrtr.rs` is dead (D-14 locks MRTR to native +
`streamable-http`): `extract_mrtr_params`, `extract_input_responses`, `check_input_response_bounds`,
`logical_name_of`, all four bound constants and `InputResponseTypingError` all warn identically and
all predate this plan. `-D warnings` is not applied to the wasm build; exit is 0.

**The pmat delta, and why it is not this plan's.** `.summary.violations` reports 5, one more than
114-19's recorded 4. The extra is
`tests/v2_tasks_update_routing.rs:1081 no_source_site_routes_tasks_update_through_the_mrtr_ingress`
at cog 33 — **114-13's**, not this plan's. Verified rather than argued: the function was extracted
from `git show aa651f74^:` and from the tree and the two hash **IDENTICALLY**
(`3da6ca02185f999b5b0d7f29e75831eb1c72c74ef6f0d3c0de17f21680cc045d`). This plan's own new route
helpers are all under the gate; `src/` reports **0**.

**Complexity discipline applied up front, not discovered at PR time.** The route's seven steps are
five short functions — `route_tasks_update`, `parse_tasks_update_params`,
`decode_inputs_against_record`, `deliver_tasks_update`, `deliver_update_through_store`,
`store_error_or_fall_through` — rather than one body, exactly as the plan's T-114-76 asked.

## Threat Model Coverage

| Threat | Disposition | Evidence |
|--------|-------------|----------|
| T-114-67 mis-typed input injection | mitigated | `decode_for` against record kinds; `try_from_value_untagged` absent from the route (non-comment count 0); NC-1 fails 3 tests |
| T-114-68 `inputResponses` exhaustion | mitigated | four bounds over the RAW map before any decode; four tests each with an also-undecodable payload; NC-2 fails 5; fuzz invariant 2 |
| T-114-69 refusal renders a key/value | mitigated | record-sourced keys only; `assert!(!raw.contains(UNSOLICITED))` and `assert!(!raw.contains("test-model"))` on response BYTES |
| T-114-70 lost update | mitigated | one CAS in the backend; dispatch does not read-then-write; `cas_first_writer_wins` under `tokio::join!`; NC-7 and NC-9 both fail it |
| T-114-71 replay of an answered key | mitigated | `ignores_an_already_answered_key` asserts the FIRST answer survives a differing replay |
| T-114-72 feeding a terminal task | mitigated | three tests (completed/failed/cancelled); NC-7 fails all three |
| T-114-73 cross-owner delivery | mitigated | owner from the identity table only; `for_another_owner_is_not_found` asserts the message is byte-identical to an absent id and renders neither id nor owner; NC-5 |
| T-114-74 client choosing its own kind | mitigated | fuzz invariant 3 + the `the_decode_accepts_only_recorded_keys` property; NC-8 |
| T-114-75 vacuously-green campaign | mitigated | recorded falsifiability control: campaign CRASHES with a named assertion |
| T-114-76 complexity-gate breach | mitigated | six named helpers; `pmat` 0 violations in `src/` |
| T-114-SC package installs | accepted | **zero** packages installed; `fuzz/Cargo.toml` gains a `[[bin]]` block only; `Cargo.lock` byte-unchanged |

**Threat surface scan:** no new network endpoint, auth path, file access or schema change outside the
`<threat_model>`'s register. No threat flags.

## Known Stubs

None. Every path the plan describes is implemented and exercised over a real socket.

## For the next plans

- **`.planning/REQUIREMENTS.md` is UNTOUCHED (0-byte diff)** and `requirements mark-complete` was
  deliberately NOT run. TASK-01…06 flip as a GROUP under the phase's contract-first waiver; the
  `## Verdict` stays `PENDING`.
- **`114-SPEC-RECHECK.md` row 19 is CLOSED** and now records the internal-route envelope trap.
  Rows 16/17/18/20 are 114-11's and are untouched. No `tasks/*` row was added to `MRTR_METHODS`;
  both method tables are byte-identical to plan-start, and
  `tests/v2_tasks_update_routing.rs`'s three substitute guards stay green.
- **`crates/pmcp-tasks` has a zero-byte diff** and its 514 tests pass — a `GenericTaskStore`-backed
  deployment (DynamoDB/Redis) gets `tasks/update` delivery with no change, because the decode and the
  bounds live above the `serde_json::Value` seam and the transition was already the backend's.
- **114-17 (the example)** now has a complete create → pause → update → resume loop to demonstrate.
  The client half is `Client::tasks_update` (114-19) and the server half is this plan; a paused task
  is reachable from a real `tools/call` via the harness's `pausing_task_tool`.
- **New deferrals: D-114-M** (a `TaskRouter` performs its own decode, unaided — the kind-direction
  property is enforced for `TaskStore` and only documented for `TaskRouter`) and **D-114-N** (a
  store that cannot accept inputs on a router-less server reuses the frozen `TASKS_NOT_ENABLED`
  message rather than minting a fifth `-32601` sentence).
- **A trap worth carrying:** any method added on the crate-private internal-request route
  (`server/discover`, `tasks/update`, and whatever comes next) must inject the v2 result envelope
  ITSELF. It bypasses `process_client_request`. The failure is silent on a client that treats an
  absent `resultType` as complete — which pmcp's own v2 client does.

## Self-Check: PASSED

- Artifacts on disk: `tests/v2_tasks_update.rs` (986 lines, `min_lines: 220`),
  `fuzz/fuzz_targets/fuzz_tasks_update.rs`, `114-14-SUMMARY.md` — all FOUND.
- Commits reachable: `aa651f74`, `6cc608bd`, `066d9c60`, `e3f4dbd1` — all FOUND.
- `must_haves` contract greps: `decode_for` in `src/server/task_dispatch.rs` → present (2, one
  rustdoc + the call site); `supports_inputs` → present; `task_input_snapshot` → present at the
  delivery route.
