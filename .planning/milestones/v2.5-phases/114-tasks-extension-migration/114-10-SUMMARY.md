---
phase: 114-tasks-extension-migration
plan: 10
subsystem: server-egress
tags: [reserved-fields, v2-envelope, tasks-extension, mrtr, row-23]
requires:
  - "server::core::inject_v2_result_envelope (Phase 112 VERS-07)"
  - "server::core::own_reserved_result_fields (Phase 113 T-113-59/T-113-60)"
  - "types::mrtr::{REQUEST_STATE_KEY, INPUT_REQUESTS_KEY}"
  - "schema/vendored/ext-tasks/schema.json @ 2c1425d9 (114-01)"
provides:
  - "server::core::ReservedFieldOwner{None,Mrtr,TasksDispatch} — explicit per-key per-owner grant"
  - "ReservedFieldOwner::may_emit — the single place a reserved key is granted"
  - "pmcp::testing::{v2_result_envelope, v1_result_envelope, EnvelopeOutcome, CapturedWarning, ReservedFieldEgress}"
  - "pmcp::testing::{RESERVED_INPUT_REQUESTS, RESERVED_REQUEST_STATE}"
  - "tests/v2_reserved_fields_tasks.rs — 6 properties, 6 negative controls"
affects:
  - "114-11 (v2 tasks/get result shape — may now emit inputRequests)"
  - "114-12 (v2 create trigger — owns removing the TasksDispatch dead_code allow, D-114-H)"
  - "114-19 (client era decoding)"
tech-stack:
  added: []
  patterns:
    - "explicit-ownership-input replacing a disposition-derived flag"
    - "per-key per-owner grant (may_emit) rather than an all-or-nothing boolean"
    - "reproduce-at-runtime-before-fixing (the reproduction becomes the regression test)"
    - "behaviour-observation test seam: production bytes AND the tracing warnings, together"
key-files:
  created:
    - tests/v2_reserved_fields_tasks.rs
  modified:
    - src/server/core.rs
    - src/server/mod.rs
    - src/server/streamable_http_server.rs
    - src/testing/mod.rs
decisions:
  - "DQ2 applied: ownership is an EXPLICIT input, not an extension of the disposition model"
  - "The grant is per-KEY per-OWNER: requestState stays MRTR-only, inputRequests gains a second minter"
  - "The ownership claim is made in seal_input_required, at the insert() calls it describes"
  - "The TasksDispatch dead_code allow is scoped not(feature=\"testing\"), NOT not(test) — measured"
  - "Six negative controls, not three: the plan's three left tests 4/5/6 unproven"
metrics:
  duration: "~55 min"
  completed: 2026-07-28
  tasks: 3
  commits: 3
  files_changed: 5
---

# Phase 114 Plan 10: Reserved-Field Ownership Per Egress Summary

**One-liner:** The v2 reserved-field registry now grants `inputRequests` per-OWNER via an explicit
`ReservedFieldOwner` threaded from the minting egress, so a v2 `tasks/get` on an `input_required`
task keeps the field the ext-tasks schema requires — while a handler forging it is still stripped
and `requestState` stays MRTR-only.

---

## What this plan was

`114-SPEC-RECHECK.md` **row 23**, flagged by 114-01 as the highest-severity finding in the phase
and deferred to here by every plan since. Phase 113's `own_reserved_result_fields` derived its
ownership test from the disposition:

```rust
let mrtr_owned = disposition == ResponseDisposition::InputRequired;
```

That was correct while `mrtr_egress` was the **only** minter of `requestState` / `inputRequests`.
Phase 114 introduces a second legitimate minter whose disposition is `complete`, and the removal
is silent by design — a `tracing::warn!`, not an error — so nothing in the repo would have noticed.

---

## The defect, reproduced at runtime BEFORE the fix

Task 1 wrote the reproduction against the unfixed tree. `cargo nextest run --features full -E
'binary_id(pmcp::v2_reserved_fields_tasks)'` → **exit 100**, `2 tests run: 1 passed, 1 failed`.

Verbatim failure output:

```
thread 'tasks_minted_input_requests_survives_egress' (69414348) panicked at tests/v2_reserved_fields_tasks.rs:100:5:
expected `inputRequests` to survive egress, but the emitted bytes were:
{"jsonrpc":"2.0","id":1,"result":{"taskId":"task-row-23","status":"input_required","createdAt":"2026-07-28T00:00:00Z","lastUpdatedAt":"2026-07-28T00:00:01Z","ttlMs":60000,"resultType":"complete","_meta":{"io.modelcontextprotocol/serverInfo":{"name":"reserved-field-registry-probe","version":"1.0.0"}}}}
warnings: [
    CapturedWarning {
        target: "mcp.v2",
        field: Some(
            "inputRequests",
        ),
        message: "removed a handler-supplied reserved result field from a result this egress did not mint",
    },
]
```

**Yes, the `tracing::warn!` fired.** Target `mcp.v2`, `field = "inputRequests"`, message verbatim:
`removed a handler-supplied reserved result field from a result this egress did not mint`.

**The bytes are the point.** The emitted response still carries `"resultType":"complete"` and a
well-formed `_meta.io.modelcontextprotocol/serverInfo`. It looks like a perfectly good response.
It is missing a field `$defs.InputRequiredTask.required` lists, and every in-repo assertion of the
form "the request succeeded" or "`status == input_required`" passes against it. Note the warning
even calls the field *handler-supplied* — the registry had no vocabulary for a second server-side
minter, so it misattributed the server's own field to the handler.

The still-stripped control `handler_supplied_input_requests_is_still_stripped` **PASSED** against
the same pre-fix tree, which is what proves the fix must be per-OWNER and not "always allow the key".

### Vendored-artifact confirmation (checked on disk, not quoted from research)

`schema/vendored/ext-tasks/schema.json`:

- `$defs.InputRequiredTask.required` = `["taskId","status","createdAt","lastUpdatedAt","ttlMs","inputRequests"]`
- `$defs.GetTaskResult` = `allOf[ Result, anyOf[WorkingTask|InputRequiredTask|…] ]` — a **flat**
  `allOf`, **not** a `{"task": …}` wrapper. So `inputRequests` is a TOP-LEVEL key of the v2
  `tasks/get` result, exactly as row 23 states. (v1's `GetTaskResult` in `src/types/tasks.rs` DOES
  wrap under `task` — the two eras differ here, and that difference is 114-11's.)

---

## The fix

`ReservedFieldOwner{None, Mrtr, TasksDispatch}` in `src/server/core.rs`, threaded from the egress
that did the minting into `inject_v2_result_envelope` → `own_reserved_result_fields`.

**The grant is per-KEY per-OWNER**, via one function:

```rust
fn may_emit(self, field: &str) -> bool {
    match self {
        Self::None => false,
        Self::Mrtr => field == REQUEST_STATE_KEY || field == INPUT_REQUESTS_KEY,
        Self::TasksDispatch => field == INPUT_REQUESTS_KEY,
    }
}
```

The strip loop now visits **every** reserved key on **every** path and asks `may_emit`, so a
future owner added to the enum cannot silently gain a key by omission — it has to say so.
"Always allow `inputRequests`" would have fixed the tasks case and simultaneously handed every
tool handler the ability to forge an input-request set (T-114-45); granting the tasks owner
`requestState` would let a surface with **no continuation token** publish something shaped like
one (T-114-44 — the persisted task record replaces the sealed continuation, D-17).

**The ownership claim is made where the write happens.** `seal_input_required` returns
`Ok((ResponseDisposition::InputRequired, ReservedFieldOwner::Mrtr))` at the two `insert()` calls
it describes. `mrtr_egress`, `fail_mrtr_egress` and `MrtrRound::finish` return the pair; both
dispatch sites destructure it and hand both facts to the envelope. Nothing downstream re-derives
ownership — that derivation *was* the defect.

**No default, every call site names its owner.** Five production call sites:

| site | owner |
|---|---|
| `ServerCore::handle_request` (`core.rs`) | from `mrtr.finish()` |
| `Server::handle_request_with_context` (`mod.rs`) | from `mrtr.finish()` |
| `build_discover_response` (`core.rs`) | `None` |
| `listen_terminal_result_frame` (`streamable_http_server.rs`) | `None` |
| both `not(streamable-http)` fallbacks | `(Complete, None)` |

**The registry rustdoc table** now names both legitimate minters of `inputRequests` and states
that `requestState` has exactly one. The sentence claiming the derivation was a mere convenience
is **deleted** — `git diff` shows `-/// \`mrtr_owned\` is derived from the disposition rather than
passed separately:` and its two continuation lines.

---

## Six properties, six negative controls

`cargo nextest run --features full -E 'binary_id(pmcp::v2_reserved_fields_tasks)'` → **exit 0,
6 tests run: 6 passed**. Every test asserts on **raw response bytes** (never a parsed struct —
these result types carry `skip_serializing_if`, so a deleted key and an absent key are the same
`None`) and on the **warning set** (absence alone cannot distinguish "stripped" from "never
there", which is the exact ambiguity that let row 23 ship). The STRIP case of each pair fires
before the SURVIVE case.

| control | edit | tests FAILED | tests green |
|---|---|---|---|
| NC-1 | restore the derived `mrtr_owned` | `tasks_minted_input_requests_survives_egress`, `tasks_minted_request_state_is_still_stripped` | 4 |
| NC-2 | grant `TasksDispatch` the `requestState` key | `tasks_minted_request_state_is_still_stripped` | 5 |
| NC-3 | `may_emit` returns `true` unconditionally (owns-everything default) | `handler_supplied_input_requests_is_still_stripped`, `tasks_minted_request_state_is_still_stripped` | 4 |
| NC-4 | narrow the `Mrtr` grant to `inputRequests` only | `mrtr_minted_input_requests_still_survives` | 5 |
| NC-5 | delete the v2-only era gate in `inject_v2_result_envelope` | `a_v1_result_is_untouched_by_the_registry` | 5 |
| NC-6 | delete the object-results-only guard | `a_non_object_v2_result_is_untouched` | 5 |

Six distinct failing sets. Every one of the six tests is named by at least one control, so none of
them is redundant.

**Two honest departures from the plan's predicted control results, both measured:**

1. **NC-1 fails TWO tests, not one.** The plan predicted "test 1 fails, tests 2-6 stay green".
   `tasks_minted_request_state_is_still_stripped` also fails, because its fixture deliberately
   carries BOTH reserved keys and asserts `inputRequests` **survives** while `requestState` is
   removed — proving in a single call that the two keys are decided independently. That makes it a
   second, independent detector of the row-23 regression. The plan's prediction was written before
   that test existed; the overlap is a strengthening, not a defect, and the three sets are still
   distinct. NC-2 and NC-3 matched the plan's predictions exactly.
2. **Three controls were not enough.** The plan's NC-1/2/3 leave `mrtr_minted_input_requests_still_survives`,
   `a_v1_result_is_untouched_by_the_registry` and `a_non_object_v2_result_is_untouched` unfailed by
   any control — i.e. three of the six properties would have been recorded as "locked" without any
   evidence that they are load-bearing. NC-4/5/6 were added to close that, and each fails exactly
   its own test.

All six controls were reverted from a byte-for-byte backup; `shasum -a 256 -c` reports
`src/server/core.rs: OK` and `tests/v2_reserved_fields_tasks.rs: OK`, and
`git diff --stat -- src/` after the reverts is **empty**. **`git stash` was not used at any point.**

---

## Deviations from Plan

### [Rule 3 - Blocking] `src/testing/mod.rs` — the E0365 seam (files beyond `files_modified`)

- **Found during:** Task 1
- **Issue:** `inject_v2_result_envelope` and `own_reserved_result_fields` are `pub(crate)`. The
  plan mandates an INTEGRATION test (`tests/v2_reserved_fields_tasks.rs`), which compiles as a
  separate crate and cannot reach them. A `pub use` of a `pub(crate)` item does not compile
  (E0365) — the established wall this module already works around for `encode_mcp_name`,
  `routing_name_key` and `mint_request_state`.
- **Fix:** added `v2_result_envelope` / `v1_result_envelope`, returning an `EnvelopeOutcome
  { bytes, warnings }`. Because the registry's removals are silent by design, the warnings are
  returned WITH the bytes: a test that cannot see the warning cannot distinguish "removed" from
  "never present", which is the ambiguity under investigation. Capture uses a ~60-line hand-written
  `tracing::Subscriber` scoped by `tracing::subscriber::with_default` (thread-local, so parallel
  test threads cannot observe each other), deliberately NOT `tracing-subscriber` — that would have
  tied the seam to the `logging` feature, and `Cargo.toml`/`Cargo.lock` had to stay byte-unchanged
  (T-114-SC).
- **Files modified:** `src/testing/mod.rs`
- **Commits:** `920cb36c`, `44fdcf67`

### [Rule 3 - Blocking] `src/server/mod.rs`, `src/server/streamable_http_server.rs`

- **Issue:** `inject_v2_result_envelope` gained a parameter with **no default** (the plan requires
  this), so its two call sites outside `core.rs` had to name their owner.
- **Fix:** both name `ReservedFieldOwner::None` with a one-line reason; `mod.rs`'s dispatch site
  destructures the pair from `mrtr.finish()`.
- **Commit:** `44fdcf67`

### [Rule 2 - Correctness] `ReservedFieldOwner` needs its own `dead_code` guards — with a scope the plan's `Task`-variant precedent gets wrong

- **Found during:** Task 2
- **Issue:** the new enum has the same problem the plan's ordering correction describes for
  `ResponseDisposition::Task`: nothing in production constructs `TasksDispatch` until 114-12.
- **Measured** (all `RUSTFLAGS="-D warnings" cargo clippy --lib`, allows REMOVED):
  - `--features full` → **exit 0**. The `pmcp::testing` seam is compiled under `full` and
    constructs all three variants, so the gate is clean without any allow.
  - `--no-default-features --features streamable-http` → **exit 101**, exactly one error:
    `variant TasksDispatch is never constructed`. `Mrtr` is NOT flagged (`seal_input_required` is
    live on that selection).
  - `--no-default-features` → exit 101, includes `variants Mrtr and TasksDispatch are never
    constructed` (alongside the 54 pre-existing D-114-E rows).
- **Fix:** `#[cfg_attr(not(any(feature = "streamable-http", feature = "testing")), allow(dead_code))]`
  on `Mrtr`; `#[cfg_attr(not(feature = "testing"), allow(dead_code))]` on `TasksDispatch`. The
  second is scoped to the FEATURE and not to `not(test)` **on purpose**: `make lint` runs
  `--lib --tests` with `full`, and the `--lib` half is a non-test build with `testing` ON, so a
  `not(test)` scope would deactivate the lint for exactly that half. Under the feature scope, both
  halves lint it. Logged as **D-114-H** with the closure condition.
- **Commit:** `44fdcf67`

### [Process] A measurement that was invalid and had to be redone

An `&&`-chained probe aborted at a non-existent `/usr/bin/cp` (macOS has `/bin/cp`), so the
heredoc that was supposed to REMOVE the allows never ran — and the subsequent `clippy` "exit 0"
was measured against the unmodified tree. The `grep -c` in the same invocation printed `6`, which
looked like a plausible post-edit count and masked it. **Verify the mutation landed before
trusting the measurement it enables**; the redone probe is the table above. Adjacent: `/usr/bin/ls`
and `/usr/bin/cat` also do not exist here (`/bin/ls`, `/bin/cat` do).

---

## Plan-text corrections (measured, not assumed)

1. **Both of the plan's `nextest` selectors are wrong — the FIFTH time in this phase.**
   - `-E 'test(/v2_reserved_fields_tasks/)'` → `Starting 0 tests across 88 binaries (2436 tests
     skipped)`, **exit 4**. No test NAME carries that token; the correct selector is
     `binary_id(pmcp::v2_reserved_fields_tasks)` → exit 0, 6/6.
   - `-E 'test(/v2_mrtr/)'` is worse because it **exits 0 vacuously**: it selects **5** lib tests
     whose names happen to contain `v2_mrtr`, not the 46-test `pmcp::v2_mrtr` integration binary
     the plan means. `binary_id(pmcp::v2_mrtr) or binary_id(pmcp::v2_mrtr_ingress) or
     binary_id(pmcp::v1_tasks_golden)` → exit 0, **60/60**. A green `test(/…/)` run is not evidence
     the intended suite ran.
2. **The acceptance grep `git diff src/server/core.rs | grep -c 'allow(dead_code)'` is 0** cannot
   pass as written — it is a PROXY for "the `Task` variant's allow is untouched", and it cannot
   distinguish a change to that variant from a new enum's own guards. The count is **2**.
   The INTENT was measured directly instead, the 114-06 way: the `ResponseDisposition` block was
   extracted from `git show HEAD:src/server/core.rs` and from the working tree and compared —
   **IDENTICAL**, `allow(dead_code)` count inside it unchanged at 2. The `Task` variant's
   `#[cfg_attr(not(test), allow(dead_code))]` is byte-for-byte where 114-12 will find it.
3. **`cargo semver-checks` — "no update required (all touched items are private/`pub(crate)`)"**
   is right in verdict, wrong in reason. The E0365 seam adds 7 PUBLIC `pmcp::testing` items. They
   are purely additive, so the verdict holds.

---

## Verification — every check run, verbatim

| check | command | exit |
|---|---|---|
| Quality gate | `make quality-gate` | **0** |
| Lint | `make lint` (×4: after Task 1, ×2 in Task 2, after Task 3) | **0** each |
| Format | `cargo fmt --all -- --check` | **0** |
| Row-23 suite | `cargo nextest run --features full -E 'binary_id(pmcp::v2_reserved_fields_tasks)'` | **0** — 6/6 |
| MRTR + v1 goldens | `… -E 'binary_id(pmcp::v2_mrtr) or binary_id(pmcp::v2_mrtr_ingress) or binary_id(pmcp::v1_tasks_golden)'` | **0** — 60/60 |
| Wider sweep | `… -E 'binary_id(pmcp::v2_reserved_fields_tasks) or …v2_mrtr… or …v1_tasks_golden… or binary_id(pmcp)'` | **0** — 1789/1789 |
| Semver | `cargo semver-checks check-release --baseline-rev 164f5f15` | **0** — 223 checks: 223 pass, 30 skip, **no semver update required** |
| Public API | `cargo public-api --features full diff 164f5f15..HEAD` | **0** — Removed **(none)**, Changed **(none)** |
| wasm | `make wasm-build` | **0** — 93 warnings, **0** naming any touched symbol |
| PMAT | `pmat analyze complexity --format json --max-cognitive 25`, queried at `.summary.violations` | **0** — **4** violations, **0 in `src/`** |

**Quality gate, reconciled to a single line.** Exit **0**; **273** `test result:` lines, **4700
passed / 0 failed / 80 ignored**, **0** truncation markers, **0** non-`ok.` result lines. Against
114-09's baseline of 270 lines / 4694 passed / 80 ignored: **+3 lines, +6 passed, ignored
unchanged**. Measured cause, not inferred — `tests/v2_reserved_fields_tasks.rs` is `Running` in
**three** gate legs (log lines 2773, 4066, 5648); two of them apply a filter and report
`running 0 tests`, one runs all six. So **+3 result lines** and **+6 passed**, exactly.

**PMAT's output shape is still the trap 114-05 recorded:** `violations` lives under `summary`, and
a naive top-level query returns "0 violations" vacuously. Queried correctly: 4, all in
`crates/**/tests/`, **0 in `src/`** — a 291-line new test file, a 275-line testing-seam addition
and a 433-line `core.rs` change introduced none, and `may_emit` is a 4-arm match.

**Cargo manifests byte-unchanged**, zero packages installed (T-114-SC):
`git diff 164f5f15..HEAD --stat -- Cargo.toml Cargo.lock` is empty.

**All 84 deletion lines enumerated** with `/usr/bin/git` (the RTK proxy has returned 0 for this
count on a diff that really had 28 — 114-04's finding) and every one is an intended replacement:
signature widenings, call-site updates, assertion pairings, and the three deleted rustdoc lines.
**REMOVED: (none)** on the public API confirms it independently.

---

## Requirements

**`.planning/REQUIREMENTS.md` is untouched — 0-byte diff — and `requirements mark-complete` was
deliberately NOT run.** TASK-04 is implemented at the egress layer but stays unflipped:
`114-SPEC-RECHECK.md` flips TASK-01…TASK-06 as a **GROUP** and only on a `PUBLISHED-CONFIRMED`
landing, and `## Verdict` is still `PENDING`.

`114-SPEC-RECHECK.md` **row 23 was updated** with what is resolved (pmcp's behaviour) and what is
not (the draft wire value, still held; and 114-11 still owns emitting the flat `InputRequiredTask`
shape). No contract YAML (114-20 waiver). **No `tasks/*` row was added to `MRTR_METHODS`** —
`git diff` touches neither that table nor `mrtr_eligible`.

---

## For 114-11 and 114-12

- **The permission exists; nothing dispatches yet.** `ReservedFieldOwner::TasksDispatch` has no
  constructor in `src/` — its only constructor today is the `pmcp::testing` seam (D-114-H).
- **114-12: how to supply the owner.** `ServerCore::handle_request` and
  `Server::handle_request_with_context` currently get the pair from `mrtr.finish()`. A tasks result
  is produced inside `handle_request_internal`, which returns only the response — so 114-12 needs
  either to return the owner alongside it or to call `inject_v2_result_envelope` from the tasks
  route. Do **not** re-derive the owner at the call site from the disposition or the method string;
  both were considered and rejected during planning (DQ2) precisely because they re-create the
  per-site divergence the single registry exists to prevent.
- **114-12 also owns removing `#[cfg_attr(not(feature = "testing"), allow(dead_code))]` from
  `TasksDispatch`**, and `#[cfg_attr(not(test), allow(dead_code))]` from `ResponseDisposition::Task`
  (which this plan left byte-identical, verified by block extraction).
- **114-11: the shape.** `$defs.GetTaskResult` is a flat `allOf`, so on v2 every task field is a
  TOP-LEVEL result key. v1 wraps under `task`; the two eras genuinely differ and
  `tests/v1_tasks_golden.rs` (14 fixtures, green here inside 60/60) freezes the v1 side.
- **`requestState` is denied to the tasks owner and always will be** (D-17). If a future tasks
  surface needs resumable state, it is a task record, not a continuation token.
- **The test seam is reusable.** `pmcp::testing::v2_result_envelope(result, egress)` returns bytes
  AND warnings; `RESERVED_INPUT_REQUESTS` / `RESERVED_REQUEST_STATE` are the crate's own key
  constants. Assert on bytes — `skip_serializing_if` makes a struct assertion blind to a deletion.

---

## Deferred

New: **D-114-G** (`pmcp::testing`'s module charter now under-describes the file — it gained a
behaviour-observation seam alongside its shape-conformance one) and **D-114-H**
(`ReservedFieldOwner::TasksDispatch` has no production constructor; owned by 114-12, with the
measured feature-scoping table).

Still open and untouched here: **D-114-A**, **D-114-B**, **D-114-C**, **D-114-E**, **D-114-F**,
**D-113-U**.

---

## Commits

| hash | message |
|---|---|
| `920cb36c` | `test(114-10): reproduce the silent inputRequests strip at runtime` |
| `44fdcf67` | `fix(114-10): own reserved result fields per OWNER, not per disposition` |
| `44c9828a` | `test(114-10): lock the per-owner grant with six orthogonal properties` |

---

## Self-Check: PASSED

- `tests/v2_reserved_fields_tasks.rs` — FOUND (291 lines, 6 tests, all green)
- `src/server/core.rs` — FOUND (`ReservedFieldOwner` ×55, `own_reserved_result_fields` ×6, `INPUT_REQUESTS_KEY` ×4)
- `src/testing/mod.rs` — FOUND (the E0365 seam)
- `.planning/phases/114-tasks-extension-migration/114-10-SUMMARY.md` — FOUND
- Commits `920cb36c`, `44fdcf67`, `44c9828a` — all FOUND in `git log --all`
