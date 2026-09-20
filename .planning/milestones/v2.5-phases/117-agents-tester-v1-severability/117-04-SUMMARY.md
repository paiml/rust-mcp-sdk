---
phase: 117-agents-tester-v1-severability
plan: 04
subsystem: testing
tags: [pmcp-agent, streamable-http, tasks, protocol-negotiation, tdd, red-tests, live-socket]

requires:
  - phase: 113-per-request-era-negotiation
    provides: "ClientBuilder::with_protocol_version + the per-request v2 era gate this harness spawns servers against"
  - phase: 114-tasks-extension-migration
    provides: "TaskStore / CallToolResult::with_related_task / wait_for_related_task — the task surface the fixture pins"
provides:
  - "crates/pmcp-agent/tests/common/v2_server.rs — the first live StreamableHttpServer harness inside pmcp-agent's test tree"
  - "A SERVER-side per-method + per-era request log (absence of `initialize` is now provable)"
  - "A ScriptedTaskStore that GUARANTEES a task-associated tool result needing >= 1 non-terminal poll"
  - "crates/pmcp-agent/tests/agent_v2_e2e.rs — the executable CLNT-03 contract, 3 RED + 1 green regression pin"
affects: [117-07, 117-08, 117-verification]

tech-stack:
  added: []
  patterns:
    - "In-crate shared test module via `#[path = \"common/<file>.rs\"] mod <name>;` (the tests/common/duplex.rs convention)"
    - "Server-side wire assertions via ServerHttpMiddleware::on_request — the only way to prove a request was NOT sent"
    - "Owner-normalizing TaskStore decorator so one fixture task is reachable from BOTH eras"
    - "Counter-driven (not clock-driven) task settle: deterministic non-terminal poll, zero timing dependency"
    - "Total test accessors (unwrap_or_else to a deliberately-empty stand-in) so an absent fact fails its own assertion instead of panicking"

key-files:
  created:
    - crates/pmcp-agent/tests/common/v2_server.rs
    - crates/pmcp-agent/tests/agent_v2_e2e.rs
  modified: []

key-decisions:
  - "spawn_v2 accepts BOTH 2025-11-25 and 2026-07-28 (not v2-only) — the dual-era server is the DISCRIMINATING fixture: a connector that silently keeps speaking v1 still gets a working connection, so only a server-side wire assertion catches it"
  - "All RED tests take the ASSERTION-FAILURE shape, never the compile-failure shape, so `cargo build -p pmcp-agent --tests --features url-connector` stays green and all four cases report independently"
  - "The negotiated era is read from OBSERVED WIRE BEHAVIOUR (a server-side per-request log), not from a new accessor on ConnectorClient — the research's reachability rule needs no new API and adding one would have widened the 117-07 surface"
  - "The unreachable-host case is a GREEN-now regression pin, not an artificial RED: its contract is 'this behaviour must not change', and 117-07's new fallback branch is exactly what could break it"
  - "The ScriptedTaskStore read counter is PER TASK, not global (Rule 1 fix) — a global counter lets a second minted task settle on its first read, silently deleting the non-terminal poll"

patterns-established:
  - "RED-test failure messages name their IMPLEMENTER plan (const IMPLEMENTER), so a red run says WHO must act"
  - "Fallback classification is asserted structurally (which requests the server received), never by inspecting error text — the file contains zero `contains(` occurrences"

# CLNT-03 is deliberately NOT booked here. This plan writes its contract RED; the
# tests FAIL by design until 117-07 implements client_for. Booking it now would be
# exactly the false-completion the phase exists to prevent. 117-07 (implementation)
# and 117-10 both carry CLNT-03 and one of them discharges it.
requirements-completed: []
requirements-contracted: [CLNT-03]

duration: 47min
completed: 2026-08-08
---

# Phase 117 Plan 04: CLNT-03 live-socket contract Summary

**A real in-process `StreamableHttpServer` harness for `pmcp-agent` — dual-era spawn, server-side per-method/per-era request log, and a task fixture that GUARANTEES one non-terminal poll — plus four end-to-end CLNT-03 cases written RED against the current `client_for`.**

## Performance

- **Duration:** ~47 min
- **Started:** 2026-08-08T04:35Z (approx.)
- **Completed:** 2026-08-08T05:22Z
- **Tasks:** 2 (+1 Rule-1 auto-fix)
- **Files created:** 2

## Accomplishments

- `crates/pmcp-agent/tests/` gained its FIRST harness driving a real `pmcp::StreamableHttpServer`. It spawns a dual-era server, a v1-only server, and a guaranteed-unreachable endpoint, and tears each down drop → `abort()` → `await`.
- **Absence is now provable.** A `ServerHttpMiddleware` records every request's JSON-RPC method AND its declared protocol version, so "no `initialize` reached the wire" is a server-side fact, not a client-side log line.
- **The CLNT-03 task-polling clause is GUARANTEED, not hoped for.** The fixture's task tool mints a real store-backed task whose settle is driven by a per-task READ COUNTER: exactly one `working` answer, then terminal. No clock, no sleep, no flake.
- Four executable cases: v2 happy path, unconditional task polling to terminal, v1 fallback, unreachable-host propagation. Three are RED for a recorded reason naming plan 117-07; the fourth is a green regression pin for T-117-10.

## Task Commits

1. **Task 1: Build the in-crate live v2/v1 server harness** — `43d1a6ad` (test)
2. **Rule-1 auto-fix: per-task read counter in `ScriptedTaskStore`** — `3ce14a54` (fix)
3. **Task 2: Write the CLNT-03 end-to-end cases RED** — `24cc67c1` (test)

## Files Created

- `crates/pmcp-agent/tests/common/v2_server.rs` (762 lines) — `spawn_v2`, `spawn_v1_only`, `spawn`, `pinned_server`, `teardown`, `endpoint`, `closed_loopback_endpoint`, `BOUNDED_WAIT`, `TERMINAL_TASK_STATUS`, `TERMINAL_RESULT_MARKER`, `NON_TERMINAL_POLLS_BEFORE_TERMINAL`, `RequestLog`, `ScriptedTaskStore`, `LiveServer`, `PinnedServer`. `grep -c '^pub '` = **23** (criterion: >= 8). Zero `TODO`/`FIXME`/`XXX`.
- `crates/pmcp-agent/tests/agent_v2_e2e.rs` (510 lines) — the four CLNT-03 cases. Line 1 is `#![cfg(all(feature = "url-connector", not(target_arch = "wasm32")))]`. `grep -c '#\[path = "common/v2_server.rs"\]'` = **1**. `grep -c 'contains('` = **0**. Zero `TODO`/`FIXME`/`XXX`.

## The exact RED output (verbatim)

`cargo test -p pmcp-agent --test agent_v2_e2e --features url-connector` → **exit 101**

```
   Compiling pmcp-agent v0.1.0 (/Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/pmcp-agent)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.83s
     Running tests/agent_v2_e2e.rs (target/debug/deps/agent_v2_e2e-130d2dfde9841ec4)
---- agent_falls_back_to_v1_when_the_server_answers_and_rejects_v2 stdout ----

thread 'agent_falls_back_to_v1_when_the_server_answers_and_rejects_v2' (4522455) panicked at crates/pmcp-agent/tests/agent_v2_e2e.rs:459:5:
plan 117-07 (UrlConnectorClientFactory::client_for) must ATTEMPT the v2 era before falling back — the server must observe a `server/discover` request that it then rejects. Without it the connector never tried v2 and the fallback is untested. Server observed: [initialize@<none>, notifications/initialized@2025-11-25, tools/list@2025-11-25, tools/call@2025-11-25]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- agent_drives_task_polling_to_terminal_on_v2 stdout ----

thread 'agent_drives_task_polling_to_terminal_on_v2' (4522454) panicked at crates/pmcp-agent/tests/agent_v2_e2e.rs:386:5:
assertion `left == right` failed: plan 117-07 (UrlConnectorClientFactory::client_for) must poll tasks over 2026-07-28, with no `initialize`. Server observed: [initialize@<none>, notifications/initialized@2025-11-25, tools/call@2025-11-25, tasks/get@2025-11-25, tasks/get@2025-11-25, tasks/result@2025-11-25]
  left: 1
 right: 0

---- agent_reaches_a_v2_server_end_to_end stdout ----

thread 'agent_reaches_a_v2_server_end_to_end' (4522456) panicked at crates/pmcp-agent/tests/agent_v2_e2e.rs:288:5:
assertion `left == right` failed: plan 117-07 (UrlConnectorClientFactory::client_for) must not send `initialize` to a server that accepts 2026-07-28 — v2 has no handshake. Server observed: [initialize@<none>, notifications/initialized@2025-11-25, tools/list@2025-11-25, tools/call@2025-11-25]
  left: 1
 right: 0


error: test failed, to rerun pass `-p pmcp-agent --test agent_v2_e2e`
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
```

### Which RED shape each test takes

| Test | Shape | Where it fails today | What 117-07 must change |
|---|---|---|---|
| `agent_reaches_a_v2_server_end_to_end` | **assertion-failure** (`:288`) | server observed **1** `initialize`, expected **0** | `client_for` must negotiate v2 first and never hand-shake against a v2-accepting server |
| `agent_drives_task_polling_to_terminal_on_v2` | **assertion-failure** (`:386`) — its FOUR named facts all PASS today; only the trailing era assertion is red | server observed **1** `initialize` during the task flow | same; the four task facts must stay green |
| `agent_falls_back_to_v1_when_the_server_answers_and_rejects_v2` | **assertion-failure** (`:459`) | server observed **0** `server/discover` — the connector never ATTEMPTS v2 | `client_for` must probe v2, observe the answer, and only then fall back |
| `an_unreachable_host_propagates_and_is_not_reported_as_era_v1` | **GREEN regression pin** (deviation, see below) | — | must STAY green: the new fallback branch must not launder a connect failure into an era decision |

No test takes the compile-failure shape. That was a deliberate choice (see Decisions): a compile error would have broken the whole test binary, hidden the other three results, and made Task 1's `cargo build -p pmcp-agent --tests --features url-connector` acceptance criterion unsatisfiable.

## What plan 117-07 must provide to turn these GREEN

**No new public API is required.** The tests read the negotiated era from observed wire behaviour, so 117-07 only has to change the body of one function.

Target for `crates/pmcp-agent/src/invoker/factory.rs` `UrlConnectorClientFactory::client_for` (signature UNCHANGED):

```rust
async fn client_for(&self, endpoint: &str) -> Result<Arc<dyn ConnectorClient>, InvokerError>;
```

Required behaviour, in order:

1. Keep the `T-108-05-05` scheme policy at `factory.rs:131-136` (`http` / `https` only). `T-117-13` pins it; the tests exercise only `http`, so a regression there would be silent — do not touch it.
2. Build a **v2** client:
   `ClientBuilder::new(transport).with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?.with_tasks_extension().build()`.
   `ClientBuilder::build` sets `initialized = true` for a v2 client (`src/client/mod.rs:5214-5216`), so `ensure_initialized` passes with **zero** bytes on the wire.
3. Probe with `client.server_discover().await`.
   - `Ok(..)` ⇒ era **V2**. Keep this client. Measured projection from the pinned server:
     `{"protocolVersion":"2026-07-28","capabilities":{"tools":{"listChanged":false},"extensions":{"io.modelcontextprotocol/tasks":{}}},"serverInfo":{...},"ttlMs":0,"cacheScope":"private"}`
     — the `io.modelcontextprotocol/tasks` extension entry is what makes `assert_capability("tasks", ..)` pass on v2 after `server_discover` has stored a projection.
   - `Err(..)` ⇒ do **not** decide yet (see the measured blocker below).
4. On probe failure, build a fresh **v1** client and `initialize(ClientCapabilities::default())`.
   - `Ok(..)` ⇒ era **V1** (the server ANSWERED and speaks v1). This is the fallback.
   - `Err(..)` ⇒ **PROPAGATE the original error.** Both attempts failing is the reachability signal.

### ⚠ MEASURED BLOCKER for 117-07: the two failures are INDISTINGUISHABLE by type

Both an era rejection and an unreachable host arrive as the SAME `pmcp::Error` variant, so there is no type-level discriminator:

| Case | `client.server_discover()` error, verbatim |
|---|---|
| v2 probe against a v1-only server (server ANSWERED, HTTP 400) | `Transport error: Request error: Request failed with status: 400 Bad Request` |
| v2 probe against a closed loopback port (server did NOT answer) | `Transport error: Request error: client error (Connect)` |

Both are `Error::Transport(TransportError::Request(String))` (`src/shared/streamable_http.rs:1175-1184` for the status path, `:1071` for the connect path). `TransportError::Request` carries only a `String`.

**Therefore the classification MUST be behavioural, not textual.** The "try v2, then try v1, then propagate" sequence in step 4 above IS the reachability rule expressed without string matching and without a new `Error` variant (which would be semver-major — `Error` has no `#[non_exhaustive]`, 116 D-03). It satisfies `T-117-11` and the § Q4.3 rule directly:

- server answers, rejects v2 ⇒ the v1 attempt succeeds ⇒ fall back;
- server never answers ⇒ the v1 attempt ALSO fails ⇒ propagate, never era V1.

Do **not** add a `pmcp-agent` predicate over the error string; the tests will not accept it, and the file's zero-`contains(` criterion exists to enforce that.

## The related-task envelope the harness's task tool produces (verbatim)

Captured from a manual `tools/call` against `pinned_task` on a live spawned server.

**Over v2 (2026-07-28):**

```json
{
  "content": [
    {
      "type": "text",
      "text": "pinned task started"
    }
  ],
  "isError": false,
  "_meta": {
    "io.modelcontextprotocol/related-task": {
      "taskId": "37ae7cee-72e2-4e21-ae55-df70dd339355",
      "pollInterval": 50
    },
    "io.modelcontextprotocol/serverInfo": {
      "name": "pmcp-agent-pinned-harness",
      "version": "1.0.0"
    }
  }
}
```

`related_task()` decodes it to
`TaskMetadata { task_id: "37ae7cee-…", poll_interval: Some(50), max_poll_duration_secs: None }`.

**Over v1 (2025-11-25):** byte-identical apart from the id, minus the v2-only
`io.modelcontextprotocol/serverInfo` sibling:

```json
{
  "content": [{ "type": "text", "text": "pinned task started" }],
  "isError": false,
  "_meta": {
    "io.modelcontextprotocol/related-task": {
      "taskId": "455f2914-b67a-491f-9ced-4a08bf726742",
      "pollInterval": 50
    }
  }
}
```

### The ACTUAL Phase-114 surface found (it differs from what the plan assumed)

The plan pointed at `examples/s50_v2_tasks_server.rs`; the shape actually used is the one in
`examples/s47_task_augmented_result.rs`, and three concrete facts differ from a naive read:

1. **The envelope is attached via `CallToolResult::with_related_task(TaskMetadata)` on a
   `ToolOutput::Result`, not via any `tasks/*`-named API.** A `ToolOutput::Payload` `Value` is
   stringified by the dispatcher's text-wrap tail and the top-level `_meta` is LOST. The harness
   therefore implements `ToolHandler::handle_output` returning `ToolOutput::Result` (which needs no
   `schemars` bound, unlike `ServerBuilder::tool_with_result`).
2. **Task ownership is ERA-SPLIT and the constants are crate-private.** `TaskDispatch::resolve_owner`
   (`src/server/task_dispatch.rs:1226-1243`) binds an unauthenticated v1 caller to
   `V1_UNAUTHENTICATED_OWNER` (`"local"`) and an unauthenticated v2 caller to `ANONYMOUS_PRINCIPAL`
   (`""`, `pub(crate)` at `src/server/core.rs:2027`). A task minted under one bucket is `NotFound`
   from the other era. **This is a real trap for any dual-era task fixture** and is why the harness
   ships `ScriptedTaskStore`, which normalizes every call onto a single `PINNED_OWNER`.
3. **The two eras use different terminal-fetch paths, and both work against the same store.**
   Measured server-side request logs from the harness:
   - v2: `server/discover, tools/list, tools/call, tasks/get, tasks/get` — the terminal result is
     INLINE in the second `tasks/get` payload.
   - v1: `initialize, notifications/initialized, tools/list, tools/call, tasks/get, tasks/get, tasks/result`
     — v1 takes a second round trip. `supports_results()` must return `true` for this to work.

### How the non-terminal poll is arranged

`ScriptedTaskStore::get` increments a **per-task read counter** and settles the task only once that
counter exceeds `NON_TERMINAL_POLLS_BEFORE_TERMINAL` (= 1):

```rust
async fn get(&self, task_id: &str, _owner_id: &str) -> Result<Task, TaskStoreError> {
    if self.record_read(task_id) > NON_TERMINAL_POLLS_BEFORE_TERMINAL {
        self.settle(task_id).await?;
    }
    self.inner.get(task_id, PINNED_OWNER).await
}
```

Read 1 answers `working`; read 2 persists the terminal `CallToolResult` (`TERMINAL_RESULT_MARKER`)
and transitions to `TERMINAL_TASK_STATUS` (`Completed`). It is a **counter, not a clock** — there is
no sleep and no timing dependency — and the measured server logs above confirm exactly **two**
`tasks/get` requests on both eras.

## `agent_drives_task_polling_to_terminal_on_v2` — the four unconditional assertions

Pasted verbatim from `crates/pmcp-agent/tests/agent_v2_e2e.rs:342-392`. There is **no `if` and no
`let ... else`** anywhere in this block; the only fallible extraction (`recorder.first_wait()`) is
TOTAL, returning a deliberately-empty stand-in so an absent wait fails FACT 1 with its own message
rather than panicking.

```rust
    let (meta, opts) = recorder.first_wait();
    let task_polls = live.requests.count(TASK_POLL_METHOD);
    let min_polls = NON_TERMINAL_POLLS_BEFORE_TERMINAL + 1;
    let settled = live
        .tasks
        .get(&meta.task_id, PINNED_OWNER)
        .await
        .unwrap_or_else(|err| panic!("the harness task {:?} must exist: {err}", meta.task_id));

    // FACT 1 — a task id was DISCOVERED from the tool result.
    assert!(
        !meta.task_id.is_empty(),
        "the `{TASK_TOOL}` result must carry a related-task envelope the invoker discovers via \
         CallToolResult::related_task(); the invoker never reached wait_for_related_task. \
         Server observed: [{}]",
        live.requests.render()
    );
    // FACT 2 — the seam was driven with BOUNDED options.
    assert_eq!(
        opts.max_poll_duration_secs,
        Some(POLL_CAP_SECS),
        "ClientToolInvoker must hand wait_for_related_task a hard max_poll_duration_secs cap"
    );
    // FACT 3 — the SERVER observed the polling. A client-side "I called it" proves
    // the call, not the poll.
    assert!(
        task_polls >= min_polls,
        "the server must observe at least {min_polls} `{TASK_POLL_METHOD}` requests (the harness \
         serves {NON_TERMINAL_POLLS_BEFORE_TERMINAL} non-terminal read before it settles, so a \
         short-circuit on an immediately-terminal task is impossible); saw {task_polls}. \
         Server observed: [{}]",
        live.requests.render()
    );
    // FACT 4 — the task reached the harness's terminal contract, and that terminal
    // result is what the agent received.
    assert_eq!(
        (settled.status, outcome.content.clone()),
        (TERMINAL_TASK_STATUS, terminal_payload()),
        "the polled task must reach the harness's pub terminal-state constant and the agent must \
         receive its terminal result within the {POLL_CAP_SECS}s cap; invoker error = {:?}",
        outcome.error
    );

    // ---- And all of that must have happened on v2. ----
    assert_eq!(
        live.requests.count(V1_HANDSHAKE_METHOD),
        0,
        "{IMPLEMENTER} must poll tasks over 2026-07-28, with no `{V1_HANDSHAKE_METHOD}`. \
         Server observed: [{}]",
        live.requests.render()
    );
```

`min_polls` = `NON_TERMINAL_POLLS_BEFORE_TERMINAL + 1` = **2**, which strictly implies the plan's
"at least 1" criterion while additionally proving the loop did not short-circuit on an
immediately-terminal task. FACTS 1–4 all PASS today (the v1 path already drives tasks correctly);
only the trailing era assertion is red, which isolates the single missing piece for 117-07.

## Decisions Made

1. **`spawn_v2` accepts BOTH eras, not v2 only.** A v2-only server would make an incorrect
   v1-preferring implementation fail loudly at `client_for` (obvious, low information). The dual-era
   server is what the milestone actually ships ("one binary serves both"), and it catches the subtle
   silent-v1-preference bug that only a wire assertion can see.
2. **Assertion-failure RED, never compile-failure RED.** Keeps `cargo build -p pmcp-agent --tests
   --features url-connector` at exit 0 (a Task 1 acceptance criterion), and lets all four cases
   report independently instead of one compile error masking three results.
3. **No new `ConnectorClient` accessor for the era.** § Q4.3 states the rule is decidable from the
   existing `InvokerError` shape; adding an accessor would have widened 117-07's surface and the
   D-09 seam bound for no measurement gain. The era is read from the server's request log.
4. **`ScriptedTaskStore` normalizes the task owner.** Without it a dual-era fixture is impossible,
   because the SDK binds unauthenticated v1 and v2 owners to different (crate-private) buckets.
5. **`ToolHandler::handle_output` over `ServerBuilder::tool_with_result`.** The latter requires a
   `schemars::JsonSchema` bound and the `schema-generation` feature; the former needs neither and
   gives the same `ToolOutput::Result` verbatim-envelope path. Zero new dependencies (`T-117-SC`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] `spawn_*` / `pinned_server` signatures widened to return the server-side observation handles**

- **Found during:** Task 1
- **Issue:** The plan specified `spawn_v2(server: Server) -> (SocketAddr, JoinHandle<()>)` and
  `pinned_server() -> Server`. Neither can deliver the SERVER-side request log or the task store to
  the test — but Task 2's acceptance criteria REQUIRE both ("the server observed zero `initialize`
  calls", "the server-observed task-poll count is asserted to be at least 1"). The recorder lives in
  `StreamableHttpServerConfig.http_middleware`, which is constructed inside `spawn_*`, and the
  accept-list is a BUILD-time property of `Server`, so `spawn_v2(server)` could not have set it
  either.
- **Fix:** Kept every prescribed NAME `pub` (the acceptance criterion is name-based:
  `grep -c '^pub '` = 23 ≥ 8) and adapted the shapes:
  `pinned_server(accept: Accept) -> PinnedServer { server, requests, tasks }`,
  `spawn(pinned) -> LiveServer { addr, handle, requests, tasks }`,
  `spawn_v2() -> LiveServer`, `spawn_v1_only() -> LiveServer`.
  `teardown<S: Send>(handle, sockets)` follows the ROOT harness's two-argument form so the
  drop → abort → await order is expressible.
- **Files modified:** `crates/pmcp-agent/tests/common/v2_server.rs`
- **Verification:** `cargo build -p pmcp-agent --tests --features url-connector` exit 0; all four
  e2e cases consume the handles.
- **Committed in:** `43d1a6ad`

**2. [Rule 1 - Bug] `ScriptedTaskStore` read counter was global, not per task**

- **Found during:** Task 2 (designing the polling case)
- **Issue:** A single global counter meant a SECOND minted task would settle on its FIRST read —
  silently deleting the non-terminal poll the whole fixture exists to guarantee. Any test calling
  the task tool more than once would then have proved nothing while still passing.
- **Fix:** `reads_by_task: StdMutex<HashMap<String, usize>>` plus a `total_reads` counter for the
  `reads()` accessor; added `reads_for(task_id)`; poisoned-lock recovery via
  `PoisonError::into_inner` so a fixture never aborts the test process.
- **Files modified:** `crates/pmcp-agent/tests/common/v2_server.rs`
- **Verification:** `cargo test -p pmcp-agent --test agent_v2_e2e --features url-connector` — FACT 3
  observes exactly 2 `tasks/get` requests; `make quality-gate` exit 0.
- **Committed in:** `3ce14a54`

### Documented departure from an acceptance criterion

**`an_unreachable_host_propagates_and_is_not_reported_as_era_v1` is GREEN, not RED.**

The plan's Task-2 criterion says "a test that passes at this point is a DEFECT — it means it asserts
nothing". That is true of the three tests whose behaviour must CHANGE. It is not true of this one:
its contract is "this behaviour must NOT change", and it is the direct mitigation for `T-117-10` /
Pitfall 7, which is a risk introduced BY 117-07's new fallback branch. Manufacturing an artificial
RED would have required inventing an error-classification predicate that § Q4.3 explicitly says is
unnecessary ("decidable from the existing `InvokerError` shape without a new error variant").

The test is genuinely load-bearing: it asserts the call is bounded by `BOUNDED_WAIT`, that no
connector is produced, and that the failure is `InvokerError::Transport` — matched on the VARIANT,
never on the message text. **117-07 must keep it green**; if the new fallback branch ever returns
`Ok(connector)` for a host that never answered, this test is what catches it.

---

**Total deviations:** 2 auto-fixed (1 missing-critical, 1 bug) + 1 documented criterion departure.
**Impact on plan:** Both auto-fixes were required for the plan's OWN acceptance criteria to be
satisfiable. No scope creep — zero new dependencies, zero source-crate changes, zero public API added.

## Issues Encountered

- **`pmcp::Client::initialize` is a NO-OP on a v2 client** (`src/client/mod.rs:551-562`: it sets
  `initialized = true` and sends nothing). So the plan's framing — "the unconditional `initialize` at
  `factory.rs:141` is the whole bug" — is half the story: the call is harmless *once the client is in
  v2 mode*. The actual defect is that `client_for` never puts the client in v2 mode
  (`ClientBuilder::with_protocol_version` is never called), so `initialize` runs its full v1 path.
  Recorded here because it changes what 117-07 has to edit.
- **`server_discover` failures are type-indistinguishable from connect failures** — see the MEASURED
  BLOCKER section above. This is the single biggest input to 117-07's design.
- **`cargo build -p pmcp-agent --tests` does not compile `tests/common/*.rs`** (they are not test
  binaries). Task 1 was therefore verified with a temporary consumer,
  `tests/zz_harness_compile_probe.rs`, which also captured the verbatim envelopes above; it was
  deleted before the Task 1 commit and is not in the tree. From Task 2 onward `agent_v2_e2e.rs` is
  the permanent consumer, so `cargo fmt -p pmcp-agent -- --check` now reaches the harness too.

## Verification

| Check | Result |
|---|---|
| `cargo build -p pmcp-agent --tests --features url-connector` | **exit 0** |
| `cargo test -p pmcp-agent --test agent_v2_e2e --features url-connector` | **exit 101 — RED as designed** (3 failed, 1 passed) |
| `cargo build -p pmcp-agent --target wasm32-unknown-unknown` | **exit 0** |
| `cargo test -p pmcp-agent` (default features) | **exit 0** — 76 passed, 13 suites; `agent_v2_e2e` compiles to zero tests (cfg'd off) |
| `cargo fmt -p pmcp-agent -- --check` | **exit 0** |
| `make quality-gate` | **exit 0** |
| `grep -c '^pub ' .../common/v2_server.rs` | 23 (>= 8) |
| `grep -c '#\[path = "common/v2_server.rs"\]' .../agent_v2_e2e.rs` | 1 |
| `grep -c 'contains(' .../agent_v2_e2e.rs` | 0 |
| `grep -cE 'TODO\|FIXME\|XXX'` on both new files | 0 / 0 |

## Threat Flags

None. No new network endpoint, auth path, file access pattern, or schema change was introduced —
both files are test-only, and the harness binds only ephemeral loopback ports it tears down.

## User Setup Required

None.

## Next Phase Readiness

- **Plan 117-07 has an executable contract.** Its definition of done is: these three RED assertions
  turn green, the fourth stays green, and the four task FACTS stay green.
- The measured blocker (era-rejection and connect-failure are the same `Error` variant) is the one
  design input 117-07 must not rediscover the hard way; the recommended "try v2 → try v1 → propagate"
  sequence is recorded above and needs no new SDK API.
- Watch item for 117-07: the `T-108-05-05` scheme policy at `factory.rs:131-136` is NOT covered by a
  test in this file (only `http` endpoints are exercised). A regression there would be silent.

---
*Phase: 117-agents-tester-v1-severability*
*Completed: 2026-08-08*

## Self-Check: PASSED

- `crates/pmcp-agent/tests/common/v2_server.rs` — FOUND
- `crates/pmcp-agent/tests/agent_v2_e2e.rs` — FOUND
- `.planning/phases/117-agents-tester-v1-severability/117-04-SUMMARY.md` — FOUND
- commit `43d1a6ad` — FOUND
- commit `3ce14a54` — FOUND
- commit `24cc67c1` — FOUND
