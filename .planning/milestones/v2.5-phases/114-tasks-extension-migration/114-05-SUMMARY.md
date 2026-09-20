---
phase: 114-tasks-extension-migration
plan: 05
subsystem: api
tags: [tasks, capabilities, extensions, negotiation, era-projection, server-discover, v1-byte-identity]

# Dependency graph
requires:
  - phase: 114-01
    provides: "the vendored ext-tasks draft schema + the 39-row wire-value inventory that fixes the extension key spelling and the {} value shape"
  - phase: 114-02
    provides: "tests/common/v2.rs (spawn_tasks_server, AuthPosture, OptionalBearer, teardown) and tests/v1_tasks_golden.rs, the v1 byte lock this plan had to keep green"
  - phase: 114-03
    provides: "TASKS_EXTENSION_KEY, TasksExtensionCapability (braced, serializes as {}), ClientCapabilities.extensions"
provides:
  - "apply_tasks_capability_rule's v2 arm: a task backend auto-populates capabilities.extensions[io.modelcontextprotocol/tasks] = {} additively"
  - "task_dispatch::tasks_extension_value() — the ONE canonical spelling of the advertised value, pub(crate)"
  - "core::project_capabilities_for_v2 — the v2 server/discover projection (clears capabilities.tasks, removes experimental.tasks) on a clone"
  - "core::project_capabilities_for_v1 — the mirror projection that keeps the auto-advertised extension entry off the v1 initialize wire, wired at BOTH initialize sites"
  - "tests/v2_tasks_negotiation.rs — a 6-row live-socket negotiation matrix over era x backend x explicit-value"
affects: [114-06, 114-08, 114-10, 114-13, 114-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "era-projected serialization: the struct carries what both eras want, each serialization boundary decides what its era sees (D-02)"
    - "additive-only capability writes in BOTH directions (entry().or_insert_with mirroring the tasks.is_none() guard)"
    - "one shared rule, two builder call sites (HTASK-01) — unchanged, both reach the new behaviour for free"
    - "twin-site parity for the v1 projection: one shared helper, ServerCore + Server both call it, neither defines its own"

key-files:
  created:
    - tests/v2_tasks_negotiation.rs
  modified:
    - src/server/task_dispatch.rs
    - src/server/core.rs
    - src/server/mod.rs

key-decisions:
  - "The extension value is {} and default_tasks_capability()'s list/cancel/requests flags are deliberately NOT projected into it (D-03): advertising list:true on an era where tasks/list answers -32601 is the capability lie the endpoint-backed rule exists to prevent"
  - "The build-time rule stays era-BLIND. Making it era-conditional is what would move v1 bytes; era-awareness belongs to the two serialization boundaries"
  - "v2 discover clears capabilities.tasks and removes ONLY experimental.tasks — D-02 rejected suppressing the whole experimental block, and an emptied experimental map is emitted as {} rather than dropped (omit-empty is a second wire rule this phase does not own)"
  - "DEVIATION (Rule 2): a v1 projection was REQUIRED and is not in the plan. The build-time rule mutates the one struct initialize serializes, so without it a v1 initialize gained the extensions key — measured on the wire, not anticipated"
  - "The v1 projection removes the entry ONLY when its value is exactly the auto-advertised {}: an operator-authored non-empty value is distinguishable and is never silently deleted. If the removal empties the map, the map is dropped rather than emitted as \"extensions\":{}"
  - "WasmServer's initialize is deliberately NOT wired to the v1 projection: the whole task subsystem is cfg(not(wasm32)), so no wasm build can auto-gain the entry, and applying it there could only remove an operator's own key"
  - "V2_TASKS_NOT_NEGOTIATED's message VALUE is left byte-unchanged; only its rustdoc was rewritten. Rewording a live refusal from a plan that does not own the route is how two plans come to disagree about one wire string"

patterns-established:
  - "A doc claim that a change falsifies is rewritten in the SAME commit as the change (113-29's failure class), and the acceptance grep for it must be checked against the NEW text before committing — the replacement nearly reintroduced the banned phrase"
  - "Three orthogonal negative controls, one per guard: each disabled guard fails only its own tests"

requirements-completed: []

# Metrics
duration: 118min
completed: 2026-07-28
---

# Phase 114 Plan 05: Server-side tasks-extension negotiation Summary

A tasks-backed pmcp server now advertises `extensions["io.modelcontextprotocol/tasks"] = {}` to a v2
`server/discover` through the SAME endpoint-backed rule that already drives `capabilities.tasks`, and
each era's serialization boundary hides the other era's spelling — including a v1 projection the plan
did not have and the wire proved necessary.

## What shipped

**One knob, two advertisements.** `apply_tasks_capability_rule` gained a v2 arm. The same
`has_backend` fact that sets `capabilities.tasks` now also ensures
`capabilities.extensions[TASKS_EXTENSION_KEY]` exists, through
`entry(..).or_insert_with(tasks_extension_value)` — the extensions-map twin of the existing
`capabilities.tasks.is_none()` guard. Both builder call sites (`builder.rs:1056` and `mod.rs:4770`)
reach the new behaviour with **zero edits**, which is what HTASK-01's "one free fn, two callers, never
a re-derived copy" buys. No existing tasks server needs a code change to be discoverable by a v2
client.

**The value is `{}` and only `{}`.** `default_tasks_capability()`'s `list` / `cancel` /
`requests.tools.call` flags are deliberately not projected in (D-03). The vendored draft schema types
this capability `Record<string, never>`, and advertising `list: true` on an era where `tasks/list`
answers `-32601` is exactly the capability lie the endpoint-backed rule exists to prevent. Both the
unit test and live test 1 assert **equality with `{}`**, not presence — a presence-only assertion
passes on precisely the change that would break this.

**Two era projections, mirrors of one another.** `server/discover` emits `extensions` (tasks entry
included) and suppresses the v1 spellings (`capabilities.tasks`, `capabilities.experimental.tasks`).
`initialize` does the opposite: it keeps `capabilities.tasks` and suppresses the auto-advertised
extension entry. Both work on a **clone** — capabilities are per-server, a projection is
per-request-era, and mutating the stored struct would make the first v2 discover permanently change
what every subsequent v1 client sees. `two successive projections yield identical output` pins that.

## The measurement that changed the plan

The plan's Task 2 instructed: *"`initialize` does not flow through `discover_result_from_capabilities`
at all … the correct implementation is to leave the v1 path untouched — **verify that by measurement
rather than assumption**."*

Measured, and the premise is only half true. `initialize` genuinely does not share the discover
helper — it builds `InitializeResult` from `self.capabilities.clone()` at three sites
(`core.rs:558`, `server/mod.rs:1538`, `wasm_server.rs:129`). But it shares the **struct**, and Task 1
writes into that struct at build time. So leaving the v1 path untouched did **not** freeze v1 bytes.

The negotiation suite caught it before any of this was reasoned about. `v1_initialize_stays_byte_identical`
failed on its raw-string check with the response verbatim:

```
{"jsonrpc":"2.0","id":5,"result":{"protocolVersion":"2025-11-25","capabilities":{
  "tools":{"listChanged":false},
  "tasks":{"list":{},"cancel":{},"requests":{"tools":{"call":{}}}},
  "extensions":{"io.modelcontextprotocol/tasks":{}}},
  "serverInfo":{"name":"v2-tasks-negotiation","version":"1.0.0"}}}
```

That is a v1 `initialize` wire change on **every tasks server that exists today** — the lock D-02
holds and the mitigation T-114-16 names. It is recorded here as a deviation rather than accepted,
because the plan's own must-have truth is unambiguous ("A v1 client's initialize response is
byte-identical to today — no extensions key") and `tests/v1_tasks_golden.rs` could never have caught
it: that suite pins `tasks/*` bodies, not `initialize`.

Note also that the plan's test 5 as literally written ("against the SAME tasks-backed server" =
`spawn_tasks_server`) could not have detected it either — the shared harness pre-seeds an
`extensions` map with `io.example/experimental`, so a `!raw.contains("extensions")` assertion against
it fails for the wrong reason. Test 5 therefore builds a fixture with **no** configured extensions,
which is what makes the raw-string check mean "the auto-advertisement leaked" and nothing else.

### Why the removal is value-conditional

`project_capabilities_for_v1` drops the entry **only when its value is exactly what
`tasks_extension_value()` writes** — the empty object, compared against the same function the
advertisement uses, so the two cannot drift.

- A non-empty value under that key is unambiguously operator-authored. Deleting it from the wire is
  the mirror-image failure of overwriting it, and the additive-only discipline this phase is built on
  cuts both ways. Test 6's rustdoc argues that an operator's configured value is the operator's; the
  v1 projection honours the same rule instead of contradicting it one file away.
- If the removal empties the map, the map is dropped rather than emitted as `"extensions":{}`. A map
  that held nothing but the auto-advertised entry did not exist before the rule created it, so
  leaving an empty object behind would itself be the byte change the projection prevents.
- **One residual case is stated, not hidden:** an operator who explicitly configured exactly `{}`
  under that key *before* this plan loses it from the v1 `initialize` wire, because that value is by
  construction indistinguishable from the auto-advertised one. It is still served on v2, where the key
  means something.

`WasmServer::initialize` is deliberately not wired: the entire task subsystem — including the
capability rule — is `#[cfg(not(target_arch = "wasm32"))]`, so no wasm build can auto-gain the entry,
and applying the projection there could only ever remove an operator's own key.

## The doc that had to be rewritten, and the trap in rewriting it

`V2_TASKS_NOT_NEGOTIATED`'s rustdoc asserted **as fact** that "pmcp advertises no
`io.modelcontextprotocol/tasks` entry" — false the moment Task 1 landed. 113-29 records exactly this
failure class (two `-32002` sites "commented v1-scoped, neither ever traced"), so the rewrite ships in
the same commit as the change.

Rewriting it surfaced a tension the plan's prescribed replacement text would have got **backwards**.
The plan said the new doc should say "a v2 tasks call arrived at a server with NO task backend
configured, therefore no extension entry was advertised." That is not what the site does: the
constant is emitted on the `(store IS configured, era is v2)` row of `handle_tasks_result`'s final
match — a server that, after this plan, **does** advertise the extension. The doc now says what is
true: the extension is advertised, what has not landed is the v2 `tasks/*` **semantics** (TASK-03), and
until they do the site answers method-not-found rather than the spec-prohibited `-32002`. The
`(false, _)` no-backend row is named separately.

`is_v1_task_era`'s v2 truth-table cell ("the v2 task surface is not implemented **and not
negotiated**") was corrected for the same reason, with the correction's reason written next to it.

**Process note worth carrying:** the plan's acceptance grep is
`grep -A12 'V2_TASKS_NOT_NEGOTIATED' … | grep -c 'advertises no'` must be 0. The natural replacement
sentence — "a server with no task backend … advertises no extension entry either" — would have
reintroduced the banned phrase and passed review while failing the grep. It was rephrased to "carries
no extension entry either". Checking an acceptance grep against the **new** text before committing is
cheaper than discovering it in the gate.

## Test suite

`tests/v2_tasks_negotiation.rs` (416 lines, 6 `#[tokio::test]`s) drives REAL frames over the shared
`tests/common/v2.rs` harness — **extended, never forked**; `git diff` on that file is empty for this
plan. `spawn_tasks_server(AuthPosture::None)` is used where it fits (tests 1 and 6's control); rows
needing a specific pre-existing capability shape build a local fixture rather than mutating the shared
one.

| # | test | asserts |
|---|------|---------|
| 1 | `v2_tasks_extension_advertised` | value **equals** `json!({})`; and lands alongside the pre-existing `io.example/experimental` key |
| 2 | `v2_discover_omits_the_v1_tasks_keys` | key ABSENCE of `tasks` and `experimental.tasks`; fixture sets `experimental.tasks` on purpose |
| 3 | `v2_discover_preserves_an_unrelated_experimental_key` | a non-tasks `experimental` key survives verbatim |
| 4 | `v2_no_backend_advertises_no_tasks_extension` | no entry on a store-less, router-less server |
| 5 | `v1_initialize_stays_byte_identical` | full inline golden literal + raw `!contains("extensions")` |
| 6 | `an_explicitly_configured_..._nonconformant_escape_hatch` | operator value served verbatim **and** pmcp itself never auto-writes a non-empty value |

Discipline points that are load-bearing rather than stylistic:

- **Absence is asserted as key absence**, never against `null`. Both `ServerCapabilities::tasks` and
  `::extensions` carry `skip_serializing_if = "Option::is_none"`, so a value-based check would accept
  the exact falsy shape a regression would emit.
- **The non-vacuity half fires first.** Test 2's server is built with `experimental.tasks` present,
  and the test establishes that `experimental` survived before denying `tasks` inside it — against the
  shared harness (which has no `experimental` map at all) that assertion would have passed against a
  projection that suppressed nothing. Test 4 asserts its server's own `extensions` key is projected
  before denying the tasks key inside it.
- **Test 6 names the tension in its own rustdoc** rather than leaving it for a reviewer: the vendored
  schema types this capability `Record<string, never>`, so a non-empty value is **not conformant**. The
  additive-only rule preserves it deliberately — an operator's configuration is the operator's — but the
  test classifies it as *a nonconformant escape hatch a deployment opts into*, not a supported
  extension shape. Its second half is what stops the first half reading as permission: pmcp's own
  auto-advertisement is always `{}`, so a non-empty value on a pmcp wire is always an operator opt-in.
- Teardown is drop-sockets → `abort()` → `await` through the shared `teardown` helper (D-113-T).

## Negative controls — three, all orthogonal, all reverted

Orthogonality is the evidence: a control failing more tests than its own would prove the tests
redundant rather than load-bearing. Files were backed up and restored byte-for-byte
(`shasum -a 256` equal before and after: `task_dispatch.rs` `dedc9be9…`, `core.rs` `b9d8c718…`).
**`git stash` was not used at any point.**

| control | edit | result |
|---------|------|--------|
| NC-1 | disable the extensions insert in `apply_tasks_capability_rule` | tests **1 and 6 FAIL**, tests 2, 3, 4, 5 PASS (`6 tests run: 4 passed, 2 failed`) |
| NC-2 | disable the v1-key suppression in `project_capabilities_for_v2` | test **2 FAILS alone** (`6 tests run: 5 passed, 1 failed`) |
| NC-3 | disable `project_capabilities_for_v1` | test **5 FAILS** plus its 2 unit tests; all five v2 rows PASS (`9 tests run: 6 passed, 3 failed`) |

NC-3 is the control for the deviation, and it also demonstrates a distinction worth keeping: under
NC-3 `v1_projection_preserves_an_operator_configured_tasks_extension_in_capabilities` **passes**, as it
must — that test guards against over-removal, not for removal, so a disabled projection trivially
satisfies it. Only the two tests that assert removal fail.

## Verification — run verbatim, exit codes recorded

| check | command | result |
|-------|---------|--------|
| aggregate gate | `make quality-gate` | **exit 0** |
| tests (clean re-run) | `/usr/bin/make test-all` | **exit 0** — 170 `test result:` lines, **2777 passed, 0 failed**, 0 non-`ok.` lines, 0 truncation markers |
| ALWAYS requirements | `/usr/bin/make validate-always` | **exit 0** — 86 lines, **1695 passed, 0 failed** |
| lint | `make lint` (pedantic + nursery) | **exit 0**, "No lint issues" — run after Task 1 and again after Task 2 |
| format | `cargo fmt --all -- --check` | **exit 0** |
| semver | `cargo semver-checks check-release --baseline-rev 27364eb1` | **exit 0** — **223 checks: 223 pass**, "no semver update required" |
| complexity | `pmat analyze complexity --format json --max-cognitive 25` | 4 violations total, **0 in `src/`**, **0 in either changed file** |
| plan selection | `cargo nextest run --features full -E 'test(/v1_tasks_golden/) or test(/v2_tasks_negotiation/) or test(/capabilities/) or test(/discover/) or test(/capability_rule/)'` | **107 tests run: 107 passed** |
| v1 lock | `tests/v1_tasks_golden.rs` inside the above | green — **v1 `tasks/*` bytes did not move** |
| serde lock | `default_serializes_without_extensions_key` | green and **unedited** |

`Cargo.toml` / `Cargo.lock` are **byte-unchanged** (`git diff --stat` against the plan base is empty)
and **zero packages were installed** — T-114-SC held.

Diff against the plan base `3e2f66ef`, measured with `/usr/bin/git` (not through the RTK proxy):

```
338  3  src/server/core.rs
  8  1  src/server/mod.rs
231  7  src/server/task_dispatch.rs
416  0  tests/v2_tasks_negotiation.rs   (new)
```

All **11** deletion lines were inspected individually: 3 replaced call-site lines (two `initialize`
sites + the discover projection line), 1 replaced rustdoc heading, 1 replaced import, and the 6 lines
of the false `V2_TASKS_NOT_NEGOTIATED` rustdoc plus 1 truth-table cell. **No code was lost.**

### Two counting caveats, stated rather than papered over

1. **The `make quality-gate` log is not a measurement.** RTK truncated it to 611 lines with
   `... (6347 lines truncated)`, and every `test result:` line fell inside the truncated region —
   `grep -c '^test result:'` returned **0** on a run that passed. The **exit code (0)** is the
   authoritative signal, exactly as the phase context warns. The counts above come from clean re-runs
   of the two test-bearing components through `/usr/bin/make`, which produced **0** truncation markers.
2. **256 lines / 4472 passed is not comparable to 114-04's 258 / 4576**, and no arithmetic
   reconciliation is claimed. That figure came from the full aggregate, which additionally runs
   `pmcp-package-gate` over the workspace-excluded `pmcp-package` crate — a component not re-run here.
   The delta (2 result lines, 104 tests) is consistent with that being the whole difference, but it was
   not measured, so it is recorded as a hypothesis rather than a corroboration.

## Deviations from Plan

### 1. [Rule 2 — missing critical functionality] The v1 `initialize` projection

- **Found during:** Task 3, by the failing `v1_initialize_stays_byte_identical` test
- **Issue:** Task 1's build-time write lands in the one `ServerCapabilities` that `initialize`
  serializes verbatim, so a v1 `initialize` against a tasks-backed server gained
  `"extensions":{"io.modelcontextprotocol/tasks":{}}`. The plan's must-have truth #2 and the T-114-16
  `mitigate` disposition both forbid that, and the plan contained no mechanism to hold it.
- **Fix:** `core::project_capabilities_for_v1`, value-conditional as described above, called from
  BOTH `initialize` sites (twin-site parity); `task_dispatch::tasks_extension_value()` promoted to
  `pub(crate)` so the removal compares against the same value the advertisement writes.
- **Files modified:** `src/server/core.rs`, `src/server/mod.rs` (**one file beyond the plan's declared
  `files_modified`** — the high-level `Server` is the twin dispatch site and is the path the tests
  exercise; fixing only `ServerCore` would have left the leak live on the common path),
  `src/server/task_dispatch.rs`
- **Guarded by:** 3 unit tests + integration test 5 + negative control NC-3
- **Commit:** `0feda247`

### 2. [Rule 1 — false documentation] `is_v1_task_era`'s v2 truth-table cell

- **Found during:** Task 1, while rewriting the neighbouring `V2_TASKS_NOT_NEGOTIATED` doc
- **Issue:** the cell read "the v2 task surface is not implemented **and not negotiated**". The second
  clause became false the moment the advertisement landed. 114-PATTERNS Pitfall 8 flags this exact
  sentence as one that "becomes false in this phase".
- **Fix:** the clause was removed and the reason for its removal written next to the table, so a future
  reader does not "restore" it.
- **Files modified:** `src/server/task_dispatch.rs`
- **Commit:** `85560cf3`

### 3. [plan-text correction] The prescribed replacement text for `V2_TASKS_NOT_NEGOTIATED` was backwards

- **Issue:** the plan directed the new doc to say the constant means "a v2 tasks call arrived at a
  server with NO task backend configured". The constant is emitted on the row where a store **IS**
  configured; the no-backend case takes a different row with a different message.
- **Resolution:** the doc states what the site actually does. Writing the plan's sentence would have
  replaced one false claim with another.

## Threat Flags

None. No new network endpoint, auth path, file access pattern or trust-boundary schema change was
introduced — this plan writes one capability key and reads it back at two serialization boundaries.
T-114-16 through T-114-19 are all mitigated and each has a named test; T-114-SC held with a
byte-unchanged `Cargo.toml`/`Cargo.lock`.

## Notes for the plans that follow

- **114-06 (client half)** can now negotiate against a real advertisement. Its `assert_capability`
  `"tasks"`-on-v2 arm should read `extensions[TASKS_EXTENSION_KEY]` — the key constant, never a
  literal — and should expect `{}`, since that is the only value pmcp mints.
- **The advertisement and the semantics are now out of step, deliberately.** A tasks-backed v2 server
  advertises the extension while `tasks/result` still answers `-32601` on v2. That gap is TASK-03's
  (114-08 / 114-13 / 114-14) and is documented at the constant. A plan that closes it should replace
  both `V2_TASKS_NOT_NEGOTIATED`'s value and its match row together.
- **`project_capabilities_for_v1` / `_for_v2` are the two places an era-visible capability decision
  belongs.** Anything a later plan wants a v2 client to see, or wants kept off the v1 wire, goes
  through one of them — not through a serde attribute in `src/types/capabilities.rs`, which would move
  v1 bytes for every existing server.
- `.planning/REQUIREMENTS.md` is **untouched (0-byte diff)**. TASK-01 is implemented on the server side
  but stays `[~]` and `requirements mark-complete` was deliberately NOT run: `114-SPEC-RECHECK.md`
  flips TASK-01..06 as a GROUP and only on a `PUBLISHED-CONFIRMED` landing, and `## Verdict` is still
  `PENDING`.
- No contract YAML was authored (114-20's option-b waiver). No `tasks/update` row was added to
  `MRTR_METHODS` (row 34's trap). Row 23 (`own_reserved_result_fields` deleting `inputRequests`) was
  not designed around and remains 114-10's.
- `deferred-items.md` gained nothing: no out-of-scope issue was discovered.

## Self-Check: PASSED

- `src/server/task_dispatch.rs` — FOUND
- `src/server/core.rs` — FOUND
- `src/server/mod.rs` — FOUND
- `tests/v2_tasks_negotiation.rs` — FOUND (416 lines)
- `85560cf3` — FOUND
- `f60f337e` — FOUND
- `0feda247` — FOUND
- `7d365f82` — FOUND
