---
phase: 114-tasks-extension-migration
plan: 02
subsystem: testing
tags: [golden-fixtures, byte-identity, v1-wire, test-harness, auth-posture, tasks, negative-control]

# Dependency graph
requires:
  - phase: 112-version-plumbing-spine
    provides: "tests/v2_required_headers.rs `assert_v1_byte_identical` — the only byte-identity helper in the suite, and the structure this file copies and then strengthens"
  - phase: 113-stateless-http-multi-round-trip-elicitation
    provides: "tests/common/v2.rs (the shared live-HTTP harness this plan EXTENDS), OptionalBearer (D-113-N), and the D-113-T teardown order"
provides:
  - "tests/v1_tasks_golden.rs — 6 v1 tasks/* byte-identity fixtures x 2 backend paths (InMemoryTaskStore + TaskRouter) = 12 live-HTTP golden tests, plus 2 normalizer self-tests"
  - "assert_v1_bytes — a RAW-STRING comparison after width-preserving placeholder substitution, so key order, whitespace and omission-vs-explicit-null are all in scope"
  - "tests/common/v2.rs: OptionalBearer (moved here, single definition), AuthPosture, spawn_tasks_server, v2_body_with_client_extensions, tasks_request_body, teardown"
affects: [114-03, 114-04, 114-05, 114-06, 114-07, 114-08, 114-09, 114-10, 114-11, 114-12, 114-13, 114-14, 114-verification]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Byte-identity golden = RAW-STRING compare after width-preserving placeholder substitution; the structural compare is kept only for the readable failure message"
    - "Width invariant as the anti-deletion proof: a same-width substitution whose output length and per-key occurrence counts must both equal the input's"
    - "Dynamic-value normalization carries a SHAPE predicate, so a reshaped value cannot slip through as 'just another string'"
    - "Every fixture parameterized over both task backends, because the D-11 claim is about a seam the store path never crosses"
    - "One explicit AuthPosture per spawned tasks server — no default that silently produces a no-auth server"

key-files:
  created:
    - tests/v1_tasks_golden.rs
  modified:
    - tests/common/v2.rs
    - tests/v2_subscriptions.rs

key-decisions:
  - "The golden literal is a RAW &str, not a serialized json! value: a json!-built expectation cannot pin key order at all, and key order is one of the three things a serde reshape changes silently"
  - "Timestamps are NOT fixed width (chrono to_rfc3339 emits 0/3/6/9 fractional digits, MEASURED: both .360 and .358382 appeared in one probe run), so the substitution pads the token to the value's own width for the length invariant and uses a bare token for the golden compare"
  - "A declared dynamic key that does not appear in the response is a hard FAILURE, not a no-op — a golden that normalizes an absent key proves nothing"
  - "The blanket !raw.contains(\"_meta\") assertion from v2_required_headers.rs is deliberately NOT copied: the v1 create envelope carries _meta.relatedTask by design, so _meta is asserted ABSENT on get/list/cancel/result and PRESENT-with-exactly-relatedTask on create"
  - "The store-backed fixture server sets default_ttl_ms: None so one fixture puts an explicit \"ttl\":null on the wire — without it, the omission-vs-null property would be unpinnable (ttl is Task's only Option field with no skip_serializing_if)"
  - "The router-backed path uses a LOCAL test TaskRouter, not pmcp-tasks' TaskRouterImpl: pmcp-tasks is not a dev-dependency of the root crate and adding it would violate the plan's byte-unchanged Cargo.toml constraint"
  - "OptionalBearer MOVED (not copied) into the shared harness — two divergent definitions of 'this server admits anonymous callers' is how a security test comes to pass for the wrong reason"
  - "teardown takes the sockets generically and callers with none pass `()`; one teardown order, no synonym helper"

patterns-established:
  - "Pattern 1: a byte-identity golden states in its own module rustdoc that a diff is a WIRE BREAK and that re-recording is the failure mode it exists to prevent"
  - "Pattern 2: the normalizer itself gets falsifiability guards (a width/occurrence self-test and a should_panic absent-key test), because a silently-vacuous normalizer would leave every fixture green while proving less"
  - "Pattern 3: legitimate backend differences are named AT the fixture, so a later diff is attributable rather than surprising"

requirements-completed: [TASK-05, TASK-06]

# Metrics
duration: 62min
completed: 2026-07-28
---

# Phase 114 Plan 02: v1 Tasks Byte-Identity Baseline & Shared Tasks Harness Summary

**Today's v1 `tasks/*` response bytes are now pinned on BOTH backend paths by 14 live-HTTP tests, and the comparison is a raw-string one — proven load-bearing by a negative control in which a reordered v1 wire kept every structural assertion green.**

## What Was Built

### `tests/v1_tasks_golden.rs` (1012 lines, 14 tests)

Six v1 surfaces, each driven as a REAL loopback HTTP request against a
**not-opted-in** (v1-only accept-list) server, each run twice — once
store-backed, once router-backed:

| Fixture | Store-backed | Router-backed |
|---------|--------------|---------------|
| `tasks/get` on a `working` task | `v1_tasks_golden_get_working_store_backed` | `..._router_backed` |
| `tasks/get` on a `completed` task | `v1_tasks_golden_get_terminal_store_backed` | `..._router_backed` |
| `tasks/result` while pending | `v1_tasks_golden_result_pending_store_backed` | `..._router_backed` |
| `tasks/list` | `v1_tasks_golden_list_store_backed` | `..._router_backed` |
| `tasks/cancel` | `v1_tasks_golden_cancel_store_backed` | `..._router_backed` |
| task-augmented `tools/call` create envelope | `v1_tasks_golden_create_task_result_store_backed` | `..._router_backed` |

Plus two normalizer self-tests
(`v1_tasks_golden_normalizer_preserves_width_and_every_key`,
`v1_tasks_golden_normalizer_rejects_an_absent_dynamic_key`). **Every test name
carries the `v1_tasks_golden` prefix on purpose** — nextest's `test()` predicate
matches test NAMES, not binaries (the lesson 114-01 recorded the hard way), so
the plan's own verify command would otherwise have reported `0 tests`.

### `assert_v1_bytes` — what it actually asserts

Four steps, in this order:

1. **Width invariant.** A same-width substitution (token padded with `#` to the
   replaced value's own byte width) must leave `raw.len()` unchanged AND leave
   each dynamic key's occurrence count unchanged. This is what makes "the
   normalization never deletes a key" a checked property rather than a comment.
2. **RAW-STRING comparison** of the canonically-normalized text against a golden
   `&str` literal. This is the load-bearing assertion.
3. **Structural comparison** of the parsed frame against a `json!` literal —
   `jsonrpc` + `id` + `result`/`error`, the FULL frame, never a single field.
4. **Leak guards:** `resultType` and `serverInfo` must not appear; plus the
   `_meta` rule.

A measured detail that shaped step 3: this crate builds `serde_json` with
`preserve_order`, so `Map` is an `IndexMap` whose `PartialEq` is
order-INDEPENDENT. Step 3 is therefore genuinely structural and CANNOT see key
order — which is exactly why step 2 exists and why it is not redundant. That is
stated in the helper's own rustdoc rather than left for a reader to rediscover.

### The `_meta` asymmetry (the plan's explicit instruction, honoured)

`tests/v2_required_headers.rs`'s `assert_v1_byte_identical` ends with a blanket
`!raw.contains("_meta")`. Copying that verbatim would fail here for the WRONG
reason: `build_task_created_response` (`src/server/task_dispatch.rs:527-532`)
deliberately emits `_meta.relatedTask` on the v1 create envelope. So
`MetaExpectation` has two variants — `Absent` for
`tasks/get`/`list`/`cancel`/`result`, and `RelatedTaskOnly` for the create
envelope, which asserts `_meta`'s key set is EXACTLY
`["io.modelcontextprotocol/related-task"]` (read from the production
`RELATED_TASK_META_KEY`, never re-spelled) and that its value is exactly
`{"taskId": <the store-minted id>}`.

### `tests/common/v2.rs` — additions only (zero deletion lines in the diff)

| Item | What it is |
|------|-----------|
| `pub struct OptionalBearer` | The `Ok(None)` auth provider, MOVED from `tests/v2_subscriptions.rs` with its rustdoc. `grep -c "struct OptionalBearer" tests/v2_subscriptions.rs` is now **0**. |
| `pub enum AuthPosture` | `None` / `Optional` / `Required`. No `Default`, no defaulted overload — every caller must state its posture (T-114-05). |
| `pub async fn spawn_tasks_server(posture)` | v2-opted-in (V1+V2 accept-list), `InMemoryTaskStore`-backed, one task-capable tool, caller-chosen posture. Returns the same `(SocketAddr, JoinHandle<()>)` shape as `spawn_default_config`, so `post`/`post_raw`/`Resp`/`teardown` work unchanged. |
| `pub fn long_task_tool()` + `TASKS_TOOL_NAME` | A `TaskSupport::Required` tool whose task stays `working` (a pending task can still be polled, updated and cancelled; a synchronously-completed one has already left those states). |
| `pub fn v2_body_with_client_extensions(method, id, params, keys)` | Additive sibling of `v2_body_with_caps`, which is untouched. Writes `params._meta["io.modelcontextprotocol/clientCapabilities"].extensions[key] = {}` per key, sourcing the reserved key from the crate's own `META_CLIENT_CAPABILITIES`. |
| `pub fn tasks_request_body(method, id, task_id)` | One `taskId`-bearing body builder for the `tasks/*` matrix. |
| `pub async fn teardown(handle, sockets)` | The single drop-sockets → `abort()` → `await` order (D-113-T). `tests/v1_tasks_golden.rs` routes its `shutdown` through it. |

`spawn_tasks_server` deliberately uses the **in-crate** `InMemoryTaskStore`:
114-RESEARCH Pitfall 3 measured that `pmcp-tasks`' `GenericTaskStore` refuses the
anonymous owner unless `allow_anonymous` is set, and a SHARED harness must not
bake that configuration decision into every test that merely wants a task
backend.

## Negative Controls (all run against the real tree, all reverted)

`git diff --stat -- src/ Cargo.toml Cargo.lock` is **empty** after every
restoration.

### NC-1 — key REORDER in a v1 response struct's serde declaration

Swapped `Task::status` and `Task::ttl` in `src/types/tasks.rs`.

**Result: 5 store-backed fixtures FAILED** at step 2 (the raw comparison), each
naming the exact byte difference, e.g.

```
left:  {"jsonrpc":"2.0","id":2,"result":{"task":{"taskId":"<TASK-ID>","ttl":null,"status":"working",…
right: {"jsonrpc":"2.0","id":2,"result":{"task":{"taskId":"<TASK-ID>","status":"working","ttl":null,…
```

**NC-1 SUPPLEMENT — the load-bearing measurement.** With the reorder still in
place, step 2 was temporarily disabled so only the structural comparison ran:
**all 14 tests PASSED.** A structural-only golden suite is therefore blind to a
v1 key reorder — measured, not assumed. This is the single piece of evidence that
the raw-string comparison earns its place.

### NC-2 — explicit-null → `skip_serializing_if`

Added `#[serde(skip_serializing_if = "Option::is_none")]` to `Task::ttl`.

**Result: 4 store-backed fixtures FAILED** (`"ttl":null` vanished from the wire).
`v1_tasks_golden_get_terminal_store_backed` correctly kept PASSING — its `ttl` is
`Some(60000)` and so was unaffected. That orthogonality is itself informative:
the failure set is exactly the fixtures whose `ttl` is `None`.

**Honest scope note.** The NC-2 supplement was also run (step 2 disabled): the
structural comparison **also caught it** (`the full JSON-RPC frame … must match
the golden`). So NC-2 proves the omission-vs-null property is *pinned*, but —
unlike NC-1 — it does NOT prove the raw comparison is what catches it. Recorded
this way rather than claimed as two independent proofs of the same thing.

### NC-3 (normalizer) — the absent-dynamic-key guard

Shipped as a permanent `#[should_panic(expected = "does not appear in the
response")]` test rather than a one-off manual control, so the guard cannot rot.

## Legitimate Backend Differences (so a later diff is attributable)

Three of the six fixtures differ between the two backends **today**. Each
difference is documented at its fixture in-source, and none is a defect this plan
introduced.

1. **`tasks/result` while pending.** Store-backed emits the FROZEN `-32002`
   `V1_TASK_PENDING` body (`"task result not available: task not completed"`).
   Router-backed never reaches that branch: `handle_tasks_result`'s router
   fall-through returns first, and a router `Err` becomes
   `-32603 INTERNAL_ERROR`. Both are pinned.
2. **The create envelope.** Store-backed answers the NESTED `CreateTaskResult`
   plus `_meta.relatedTask`. On a **router-only** high-level `Server` the create
   gate cannot open — `maybe_build_task_created` requires
   `task_store.is_some()` — so the call falls through to an ordinary
   `CallToolResult` (`{"content":[…],"isError":false}`). That fall-through IS
   today's v1 behaviour for that configuration and is pinned as such. The
   router's own create envelope
   (`build_task_created_response`'s no-store branch) is reachable only through
   `ServerCore`, which this file does not drive.
3. **Dynamic values.** The store path mints a UUID `taskId` and two RFC-3339
   timestamps, so three values are normalized. The router path returns values the
   test router chose, so **nothing** is normalized and those goldens are pinned
   verbatim, byte for byte — which is the sharper form of the same assertion.

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 - Blocking] `assert_v1_bytes` takes a golden STRUCT, not `(raw, expected_result)`**
- **Found during:** Task 1
- **Issue:** The plan's signature `assert_v1_bytes(raw, expected_result)` cannot
  carry a raw-string golden, an id, a dynamic-field list and an `_meta` rule —
  all of which the same plan requires the helper to assert.
- **Fix:** Kept the name; the second parameter is a `&V1Golden` struct with named
  fields (`id`, `raw`, `frame`, `dynamics`, `meta`).
- **Commit:** `a986ffe9`

**2. [Rule 3 - Blocking] The plan's own acceptance grep could not match a rustfmt-formatted `assert_eq!`**
- **Found during:** Task 1
- **Issue:** Acceptance required `grep -c 'assert_eq!(raw\|assert_eq!(normalized'` ≥ 1,
  but `rustfmt` splits a long `assert_eq!` so the macro name and its first
  argument land on different lines — the grep returned **0** while the raw
  comparison was present and passing.
- **Fix:** Extracted the failure text into `wire_break_message(raw)` so the
  assertion fits on one line: `assert_eq!(normalized, golden.raw, "{}", wire_break_message(raw));`.
  The rustdoc on that helper states plainly that its purpose is to keep the
  byte-comparison greppable. Criterion now returns 1.
- **Commit:** `a986ffe9`

**3. [Rule 3 - Blocking] `tests/v2_subscriptions.rs` is not in the plan's `files_modified`**
- **Found during:** Task 2
- **Issue:** The plan's frontmatter lists only `tests/v1_tasks_golden.rs` and
  `tests/common/v2.rs`, but Task 2's action ("Move the shape … rather than
  keeping two copies") and its acceptance criterion
  (`grep -c "struct OptionalBearer" tests/v2_subscriptions.rs` is 0) both require
  editing a third file.
- **Fix:** Edited it — the acceptance criterion is unambiguous and keeping two
  copies is the outcome the instruction exists to prevent. Recorded here as a
  frontmatter gap, not a scope expansion.
- **Commit:** `4a84bf58`

**4. Argument order of the two new body builders follows the file's convention, not the plan's text**
- **Found during:** Task 2
- **Issue:** The plan writes `v2_body_with_client_extensions(id, method, …)` and
  `tasks_request_body(id, method, task_id)`, but all four pre-existing builders in
  `tests/common/v2.rs` are `(method, id, …)`.
- **Fix:** Used `(method, id, …)` for both, matching `v2_body` /
  `v2_body_with_caps` / `v1_body` / `jsonrpc_envelope`. A swapped call would not
  compile anyway (`&str` vs `Value`), so this is a readability choice, not a
  safety one.
- **Commit:** `4a84bf58`

**5. `pmcp-tasks`' `TaskRouterImpl` replaced by a local test router**
- **Found during:** Task 1
- **Issue:** The plan says "backed by a `TaskRouterImpl`-shaped router".
  `TaskRouterImpl` lives in `crates/pmcp-tasks`, which is NOT a dev-dependency of
  the root crate; adding it would edit `Cargo.toml`, which the plan's own threat
  register (`T-114-SC`) requires to stay byte-unchanged.
- **Fix:** A local `GoldenRouter` implementing `pmcp::server::tasks::TaskRouter`
  with FIXED `serde_json::Value` payloads. This is arguably the better fixture:
  the dispatcher is supposed to pass the router's value through verbatim, so
  constant bytes make any envelope injection above the `Value` seam show up as a
  diff with nothing else moving.
- **Commit:** `a986ffe9`

**6. `default_ttl_ms: None` on the store fixture (not in the plan)**
- **Found during:** Task 1
- **Issue:** With `StoreConfig::default()` (`default_ttl_ms: Some(3_600_000)`),
  `Task::ttl` is always `Some`, so the omission-vs-explicit-null property would
  have been **unpinnable** — `ttl` is `Task`'s only `Option` field without
  `skip_serializing_if`, and NC-2 would have been vacuous.
- **Fix:** The store fixture uses `default_ttl_ms: None` and `stay_pending`
  requests no TTL, putting a real `"ttl":null` on the wire; `complete_now` still
  carries a numeric TTL, so both shapes are pinned. NC-2 fires on exactly the
  four `null` fixtures.
- **Commit:** `a986ffe9`

**7. `enable_json_response: true` (via `spawn_stateless_config`) for the golden fixtures**
- **Found during:** Task 1
- **Issue:** `spawn_default_config` has `enable_json_response: false`, so `raw`
  is an SSE-framed copy of the frame and a raw-string golden would be pinning the
  framing as much as the JSON-RPC frame.
- **Fix:** Golden fixtures use `spawn_stateless_config` — the same choice
  `tests/v2_required_headers.rs`'s existing byte-identity tests make. Documented
  in-source: "The framing is not what this file pins; the frame is." Era is
  unaffected: the fixture server has no `2026-07-28` in its accept-list, so era
  resolves v1 regardless of config. `spawn_tasks_server` (Task 2) keeps
  `spawn_default_config`, because the later plans that consume it DO test the
  per-request era gate.

**8. Two clippy findings under `make lint` that a bare `cargo clippy` misses**
- **Found during:** Task 1
- **Issue:** `clippy::doc_markdown` on `DynamoDB` in the module rustdoc, and
  `clippy::needless_pass_by_value` on `store_task(status, ttl: Value)`.
- **Fix:** Backticked `DynamoDB`; `ttl: &Value`. Recorded because this is now the
  Nth consecutive plan where `make lint` caught what a plain `-D warnings` run
  did not — `make lint` remains mandatory.
- **Commit:** `a986ffe9`

### Not done, deliberately

- **`.planning/REQUIREMENTS.md` is UNTOUCHED (0-byte diff) and
  `requirements mark-complete` was NOT run.** `114-SPEC-RECHECK.md` §
  `Requirements held` puts TASK-01…TASK-06 under the D-18 hold, flips them **as a
  group**, and only on a `PUBLISHED-CONFIRMED` landing; the current `## Verdict`
  is `PENDING`. Flipping TASK-05/TASK-06 here would contradict the hold this
  phase exists to respect. The frontmatter's `requirements-completed` field lists
  the plan's DECLARED requirements (mirroring 114-01's record), not flipped
  checkboxes.
- **No contract YAML, no `contracts/` edit** — settled by 114-20's option-b
  Phase-114 waiver.
- **No production source change.** `git diff --stat -- src/` is empty; the whole
  plan is test code.

## Verification

| Check | Command | Result |
|-------|---------|--------|
| Golden fixtures | `cargo nextest run --features full -E 'test(/v1_tasks_golden/)'` | **14 tests run, 14 passed** (≥12 required) |
| No harness regression | `cargo nextest run --features full -E 'test(/v2_/)'` | **127 passed, 0 failed** |
| Every harness consumer, by binary id (the `test()` predicate does NOT match binary names) | `-E 'binary_id(pmcp::common_harness_smoke) or … or binary_id(pmcp::v2_subscriptions)'` (all 10 files declaring `mod common`) | **181 passed, 0 failed** |
| Formatting | `cargo fmt --all -- --check` | exit 0 |
| Lint (the real gate) | `make lint` | exit 0, zero warnings |
| **Full project gate** | `make quality-gate` | **exit 0 — 258 test-result lines, 4522 passed, 0 failed** |
| Dependency drift | `git diff --stat -- Cargo.toml Cargo.lock` | **empty** |
| Production drift | `git diff --stat -- src/` | **empty** |

`make quality-gate` was run in full (`fmt-check`, `lint`, `build`, `test-all`,
`pmcp-package-gate`, `audit`, `unused-deps`, `check-todos`, `check-unwraps`,
`validate-always`, `purity-check`, `comply` + the team-servers binding-drift
check). Nothing was substituted. PMAT is not part of it by design (Phase 75
D-07: PMAT runs in CI only), and this plan changed no `src/` file, so it cannot
have moved a complexity number.

### Acceptance criteria, measured

| Criterion | Measured |
|-----------|----------|
| ≥12 tests in the golden filter | 14 |
| `grep -c 'assert_eq!(raw\|assert_eq!(normalized'` ≥ 1 | 1 |
| Same-width placeholders, never a deleted key, length asserted | width invariant + per-key occurrence invariant, both on every call |
| Contains `-32002` or `V1_TASK_PENDING` | 4 occurrences |
| Contains `relatedTask` | 5 occurrences |
| `grep -c 'resultType'` ≥ 1 | 3 occurrences |
| Every fixture asserts the FULL frame | yes — `Frame::Result`/`Frame::Error` always compared inside `{jsonrpc,id,payload}` |
| Module rustdoc says a diff is a v1 wire break | yes, first paragraph, plus the failure message itself |
| `pub struct OptionalBearer` + `pub async fn spawn_tasks_server` in the harness | both present |
| `v2_body_with_client_extensions` + `tasks_request_body` present | both present |
| `io.modelcontextprotocol/clientCapabilities` in the harness | 1 (in the rustdoc; the code reads `META_CLIENT_CAPABILITIES`) |
| `grep -c "struct OptionalBearer" tests/v2_subscriptions.rs` is 0 | 0 |
| `git diff tests/common/v2.rs` is additions only | **zero deletion lines** |

## What the Next Plans Can Now Rely On

- **A v1 byte break is a test failure, not a review item** — on the store path
  AND the router path. 114-05/06/09/10/12 (the reshape plans) inherit 12 tripwires
  they did not have to write.
- **`spawn_tasks_server(AuthPosture::Optional)` reaches the `(None, has_auth_provider = true)`
  row** that TASK-05's fail-closed refusal branch lives on. Without an `Ok(None)`
  provider that row is unreachable from a test, and the refusal could have been
  "proven" by a 401 from the transport instead.
- **`v2_body_with_client_extensions` needs no edit when 114-03 lands** the
  `io.modelcontextprotocol/tasks` constant — callers pass the constant instead of
  a literal.
- **One teardown order**, so a later tasks suite does not re-introduce D-113-T's
  intermittent nextest `LEAK`.

## Observations for Later Plans (not defects, not fixed here)

1. **Router-path `tasks/result` answers `-32603` for a pending task.** Pinned as
   today's v1 bytes. Pitfall 5 already says a v2 tasks path must not emit
   `-32603` for not-found; the router path's v1 behaviour is out of this plan's
   scope, but 114-05/114-10 should know the two paths disagree on this code
   before they gate it.
2. **A router-only high-level `Server` cannot mint a task from `tools/call`.**
   The gate requires a `TaskStore`. If any Phase-114 plan assumes router-backed
   task creation over the `Server` path, that assumption is false today —
   `handle_task_call` is reachable only via `ServerCore`.

## Self-Check: PASSED

- `tests/v1_tasks_golden.rs` — FOUND (1012 lines)
- `tests/common/v2.rs` — FOUND (732 lines, +192 lines this plan)
- `tests/v2_subscriptions.rs` — FOUND (modified, `OptionalBearer` definition removed)
- commit `a986ffe9` — FOUND in `git log`
- commit `4a84bf58` — FOUND in `git log`
