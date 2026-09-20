---
phase: 114-tasks-extension-migration
plan: 17
subsystem: examples
tags: [tasks, v2, examples, agent, tasks-update, runnable-proof, negative-controls]

requires:
  - phase: 114-14
    provides: "tasks/update delivery end to end, and the internal-route v2 result envelope the ack rides"
  - phase: 114-19
    provides: "Client::tasks_update, wait_for_task_with_inputs, tasks_get_detailed, era-aware v2 decoding, the local retirement fail-fast"
  - phase: 114-12
    provides: "the v2 create trigger (the client's declaration) and the create -> pause wiring in build_task_created_response"
  - phase: 114-06
    provides: "ClientBuilder::with_tasks_extension()"
provides:
  - "examples/s50_v2_tasks_server.rs — a runnable v2 tasks server that pauses on input and is resumed by tasks/update"
  - "examples/s51_v2_tasks_agent.rs — the autonomous agent half: five demo_* fns, each an executable assertion"
  - "an END-TO-END proof that create -> input_required -> update -> terminal works over a real socket"
  - "TWO recorded negative controls proving the example FAILS (exit 1) when the surface misbehaves"
affects: [114-18, 117]

tech-stack:
  added: []
  patterns:
    - "an example is an executable assertion: every demo_* returns Err on a divergence and main propagates"
    - "the example drives PRODUCTION client methods only — no poll loop, backoff or wire decode is written in an example"
    - "a round COUNTER, not just a terminal assertion, is what proves the pause actually happened"
    - "a raw transport FRAME (not a hand-rolled client) is the only honest way to observe a server answer the client refuses locally"

key-files:
  created:
    - examples/s50_v2_tasks_server.rs
    - examples/s51_v2_tasks_agent.rs
  modified: []

key-decisions:
  - "The server example spawns an in-process WORKER, because tasks/update leaves the task `working` and nothing in the SDK turns `working` into `completed` — that is the application's job and the example says so"
  - "The tool's fabricated id is NAMED `DISCARDED_TASK_ID`, so the store-mints-the-id rule is legible from the call site rather than only from a comment"
  - "demo 4's retirement half is proven from BOTH ends (local client refusal AND a raw frame showing the server's -32601), because 114-19 made the client refuse locally with zero bytes and the plan's `-32601` criterion is otherwise unobservable from a pmcp client"
  - "A FIFTH demo was added — the manual `tasks_get_detailed` + `tasks_update` pair — so the plan's key_links `tasks_update` pattern names a real CALL SITE rather than only prose"
  - "s50 and s51 both spell `127.0.0.1:8150`, following the s47/s48 precedent the plan told this plan to copy; the port-collision criterion is unsatisfiable for ANY paired example and was measured by intent instead"

patterns-established:
  - "Two negative controls on one route, chosen to fail DIFFERENTLY: route-disabled (fails fast at the call) and ack-without-delivery (the silent shape, caught only by the round bound)"
  - "Controls reverted from a /bin/cp snapshot with `shasum -a 256 -c`; `git checkout --` and `git stash` were NOT used (114-14's recorded loss)"

requirements-completed: []

duration: 150min
completed: 2026-07-31
---

# Phase 114 Plan 17: The Paired v2 Tasks Example Summary

**Two commands now demonstrate the whole v2 task lifecycle autonomously —
`create -> input_required -> tasks/update -> terminal` over a real socket, with
no human in the loop — and the demonstration EXITS NON-ZERO when the surface
does not behave as documented, proven by two negative controls that fail it in
two different ways.**

## Performance

- **Duration:** ~150 min
- **Tasks:** 2 of 2
- **Files created:** 2 · **Files modified:** 0
- **Commits:** 2
- **Production bytes changed: ZERO.** `git diff --stat HEAD~2 HEAD -- src/
  crates/ Cargo.toml Cargo.lock tests/ fuzz/ schema/` is **EMPTY**.

## Accomplishments

### `examples/s50_v2_tasks_server.rs` — the server half (406 lines)

A v2-opt-in `StreamableHttpServer` with an in-crate `InMemoryTaskStore` and one
`TaskSupport::Required` tool whose handler returns a task-shaped value carrying
`status: "input_required"` and ONE `inputRequests` entry (an
`elicitation/create` asking which topic to research). 114-12's create → pause
wiring re-extracts that map and records it against the STORE-minted id, so the
handle the caller receives is **already paused and pollable** — there is no
window in which the task looks runnable and is not.

Three things the file states rather than implies:

**1. The handler's `taskId` is discarded, and the constant says so.** It is
spelled `DISCARDED_TASK_ID = "discarded-the-store-mints-the-real-one"`, because
`store.create()` mints the canonical id AFTER the handler has returned. Putting
the fact in the VALUE means a reader who never opens the rustdoc still cannot
miss it. `createdAt` / `lastUpdatedAt` are deliberately absent for the same
reason: the store owns the record's clock, and a handler that invented
timestamps would be publishing a second, disagreeing one.

**2. Who does the work.** `tasks/update` moves a task to `working`; nothing in
the SDK turns `working` into `completed`, and deliberately so — the tool handler
has already returned by the time the input arrives, so it cannot be the thing
that finishes the job. The example plays that role with a 25 ms in-process
worker that watches the store, reads the delivered answers through the
owner-scoped `TaskStore::task_input_snapshot`, and writes `set_result` +
`update_status(Completed)`. A production server hands the task to a queue, a
Lambda or a durable workflow. **The SDK owns the protocol; you own the work.**

**3. Both required caveats, in the source.**

- *Why the in-crate store (Pitfall 3 / DQ8):* `pmcp-tasks`' `GenericTaskStore`
  refuses the anonymous owner while `allow_anonymous` is `false` — its default —
  so pairing a no-auth example server with that backend would fail the very
  first time anyone ran it. The comment names `allow_anonymous` and points at
  114-07's `anonymous_owner_is_refused_by_default_on_this_backend`.
- *The shared owner bucket (D-07):* no auth provider means every v2 caller
  resolves to the same anonymous owner, so they share ONE task bucket and any
  caller can read, feed and cancel any other's task. The header says so plainly
  AND says what a real deployment does instead (OAuth, so the owner is the
  authenticated `sub`, after which a cross-owner request gets the SAME `-32602`
  as an id that never existed). The owner is a NAMED constant,
  `SHARED_ANONYMOUS_OWNER`, so the caveat has something concrete to point at.

A `# What a successful run prints` section lists the expected stdout, including
the `PMCP_REQUEST_STATE_KEY is not set` startup WARNING — expected here and
irrelevant (this server never mints a `requestState`; a task is resumed by its
id) — and the one `worker:` line per completed task, with the note that its
ABSENCE means the input never landed.

### `examples/s51_v2_tasks_agent.rs` — the agent half (615 lines)

Five `demo_*` async fns, each printing a numbered banner and **returning `Err`
when the surface misbehaves**, with `main` propagating via `?`. The process
exits non-zero on any regression.

| # | Demonstration | What it asserts |
|---|---------------|-----------------|
| 1 | `demo_negotiation` | an EXPLICIT `server_discover` carries `capabilities.extensions["io.modelcontextprotocol/tasks"]` |
| 2 | `demo_autonomous_task_round_trip` | the headline claim, through `wait_for_task_with_inputs`: the handle arrives ALREADY `input_required`, ≥1 input round ACTUALLY happened, the terminal result is inline and reflects the topic the agent supplied, and a final `tasks_get` reads `completed` |
| 3 | `demo_manual_update_for_your_own_scheduler` | the same exchange by hand: `tasks_get_detailed` → `TaskDetailV2::InputRequired` → `tasks_update` → the responder-LESS `wait_for_task` reaches a result |
| 4 | `demo_undeclared_client_is_refused` | a non-declaring client gets `ToolCallResponse::Result` with no `_meta.relatedTask`, and a direct `tasks/get` is refused `-32021` |
| 5 | `demo_retired_methods_are_gone_on_v2` | `tasks/list` / `tasks/result` refused LOCALLY (`is_retired_on_v2()`) **and** answered `-32601` by the server on a raw frame |

**Everything drives a production client method.** There is no poll loop, no
backoff and no wire decode written in the file: `grep -c 'loop {'` → **0**,
`grep -c 'tokio::time::sleep'` → **0**. The 110-06 precedent (drive the
production seam, not a re-implemented engine) holds.

**The round counter is the load-bearing assertion of demo 2.** An
`Arc<AtomicUsize>` incremented inside the responder, asserted `> 0` afterwards.
Without it, a server that never paused — and a client that therefore never
answered — would satisfy every OTHER assertion in that demo, because the task
would still reach `completed` with a result. NC-2 below is the control that
proves this.

**Demo 3 exists for a reason beyond coverage.** `wait_for_task_with_inputs` is
right when the agent can sit on the task; an agent whose scheduler is elsewhere
(a Lambda woken by a queue message, a worker multiplexing thousands of tasks)
calls `tasks_get_detailed` + `tasks_update` directly. It is also what makes the
plan's `key_links` contract real: `grep -n '\.tasks_update(' ` finds an actual
CALL at line 414, not a mention in prose. And it is self-checking — the
responder-less `wait_for_task` that follows would return the input-required
error rather than a result if the update had not landed.

**No stdin, anywhere.** `grep -cE 'stdin|read_line'` → **0**. The first draft
scored **1** — on this file's own rustdoc sentence claiming there was no stdin
read. Reworded to "nothing in this file reads from standard input", which is
both true and greppable. Every answer is produced programmatically; that is what
makes the example scriptable and what makes it the agent shape.

**T-114-89 (a task is not a higher-trust channel) is stated and honoured.** The
module rustdoc carries the spec rule verbatim in substance, and `answer_all`
reads the request's `message` for DISPLAY only, answers from its own
configuration, and evaluates nothing the server supplied.

## END-TO-END PROOF

`s50` started, `s51` run against it, both exit 0. Verbatim `s51` stdout:

```text
=============================================================
  v2 (2026-07-28) AUTONOMOUS TASKS AGENT  ->  http://127.0.0.1:8150
=============================================================

[1] Negotiation — explicit server/discover
-------------------------------------------------------------
    server        : s50-v2-tasks-server
    extensions    : ["io.modelcontextprotocol/tasks"]
    tasks negotiated — configuring a TaskStore is what advertises it.

[2] Autonomous round trip — create, pause, update, terminal
-------------------------------------------------------------
    created       : d511d1cd-222e-44e5-b5aa-48a7aa002795 (input_required)
    server asks   : Which topic should I research?
    agent answers : post-quantum key exchange
    input rounds  : 1
    result        : research on post-quantum key exchange: 3 sources reviewed, no contradictions found
    final status  : completed (ttl Some(300000) ms)

[3] Manual update — tasks_get_detailed + tasks_update by hand
-------------------------------------------------------------
    created       : 09ff1ceb-485b-4d31-af5a-fe382e0f1590
    outstanding   : ["topic"]
    server asks   : Which topic should I research?
    agent answers : post-quantum key exchange
    tasks/update  : delivered (the ack is an EMPTY object)
    result        : research on post-quantum key exchange: 3 sources reviewed, no contradictions found

[4] Undeclared client — no declaration, no task
-------------------------------------------------------------
    ordinary CallToolResult, no _meta.io.modelcontextprotocol/related-task
    tasks/get     : -32021 — Protocol error: -32021 - the tasks extension was not declared on this request: send _meta["io.modelcontextprotocol/clientCapabilities"].extensions["io.modelcontextprotocol/tasks"]

[5] Retirements — tasks/list and tasks/result are gone on v2
-------------------------------------------------------------
    tasks/list    : refused locally -> use "client-side task tracking"
    tasks/result  : refused locally -> use "tasks/get"
    wire tasks/list   : -32601 from the server
    wire tasks/result : -32601 from the server

=============================================================
  All five demonstrations behaved as documented.
=============================================================
```

`s50`'s matching stdout, one line per task the worker completed:

```text
  worker: task d511d1cd-222e-44e5-b5aa-48a7aa002795 received its input, completing it
  worker: task 09ff1ceb-485b-4d31-af5a-fe382e0f1590 received its input, completing it
```

## FAILURE PROOF — two negative controls, both reverted

Each applied to `src/server/task_dispatch.rs`, `s50` rebuilt and restarted, `s51`
run, then reverted from a `/bin/cp` scratchpad snapshot and verified with
`shasum -a 256 -c` → **`src/server/task_dispatch.rs: OK`** after BOTH.
**`git checkout --` was not used** (114-14's recorded loss) and **`git stash` was
not used**.

| # | Control | `s51` exit | Where it failed |
|---|---------|-----------|-----------------|
| NC-1 | `route_tasks_update` returns `-32601` before every gate (the route is DISABLED) | **1** | demo 2, at the first `tasks/update`: `Protocol { code: ErrorCode(-32601), message: "NC-1: tasks/update route disabled" }` |
| NC-2 | `deliver_tasks_update` returns `update_ack(id)` WITHOUT delivering (acknowledge and drop) | **1** | demo 2, after **8** answered rounds: `MRTR round limit exceeded: gave up after 8 rounds without a complete result` (`pmcpError: MrtrRoundLimitExceeded`, `limit: 8`) |

**The two controls fail DIFFERENTLY, and that is the point.** NC-1 is the loud
shape — one refused call, immediate. NC-2 is the SILENT shape: the ack is
well-formed, the response bytes are indistinguishable from a successful
delivery, and the only thing that catches it is the task never leaving
`input_required`. An example that asserted only "the call succeeded" would pass
NC-2. Two additional facts fall out of NC-2:

- **T-114-91 / T-114-101 (unbounded client-side input round loop) is mitigated,
  observed rather than argued.** The loop stopped at exactly the configured
  `mrtr_round_limit` (8), which is 114-19's deliberate reuse of the MRTR knob
  rather than a second constant.
- The server's worker printed **zero** `worker:` lines under NC-2, which is the
  divergence `s50`'s own `# What a successful run prints` section tells a reader
  to look for.

## Deviations from Plan

### [Measured] Three plan-text defects — the SEVENTH, EIGHTH and NINTH in this phase

**1. The port-collision criterion is unsatisfiable for ANY paired example.**

> `grep -rho '127\.0\.0\.1:[0-9]*' examples/ | sort | uniq -d` does not list this
> example's port

`uniq -d` lists every port appearing more than once, and a paired
server/client example necessarily spells its port in BOTH files. Measured on the
precedent the plan told this plan to copy: `127.0.0.1:8147` (s47 + s48) is
ALREADY in that list, so s47/s48 fail their own successor's criterion.

**Intent measured instead:** `8150` appears in exactly the two files of this
pair and in no other example —
`grep -rl '127\.0\.0\.1:8150' examples/` → `s50_v2_tasks_server.rs`,
`s51_v2_tasks_agent.rs`. The full duplicate set is
`{127.0.0.1:, 127.0.0.1:0, 127.0.0.1:8080, 127.0.0.1:8147, 127.0.0.1:8150,
127.0.0.1:9001}` — every entry a deliberate pair or a wildcard bind.

A wording that would satisfy the literal criterion (spell the port ONCE and
`format!` the client's URL from a bare port constant) was considered and
REJECTED: the plan's Task-1 instruction is "Follow `s47`'s structure exactly",
and s47/s48 both carry a readable `DEFAULT_ADDR`.

**2. `grep -cE 'stdin|read_line'` must be 0 — and prose about stdin trips it.**
The first draft's own rustdoc sentence ("there is no `stdin` read anywhere in
this file") scored **1**. This is the same class of defect 114-19 recorded twice
in its own prose and 114-16 recorded once. Fixed at the source rather than
by re-defining the measurement: the sentence now reads "nothing in this file
reads from standard input", and the grep is **0**.

**3. `make lint` "(examples are linted)" is imprecise.** `make lint` clippies
`--lib --tests` and then runs `cargo check --features "full" --examples` —
examples are compile-CHECKED, not clippy-linted. Both were measured:
`make lint` exit **0** (inside the gate, `✓ No lint issues`), and an EXPLICIT
clippy run over the two examples with `make lint`'s full pedantic + nursery
flag set exits **0 with zero warnings naming either file**. That run found and
fixed one real `doc_markdown` warning before it was green.

### [Rule 2 — missing critical functionality] Demo 5 proves the retirement from BOTH ends

The plan's demo 4 says "assert `tasks/list` and `tasks/result` answer `-32601`".
That text predates its own dependency: **114-19 made both methods fail LOCALLY
on v2 with zero bytes on the wire**, and its summary says so explicitly
("Calling either from a v2 example produces `Error::is_retired_on_v2()` with
zero bytes sent. Do not 'fix' that by building a v1 client"). No pmcp v2 client
can observe the server's `-32601`.

Rather than drop the criterion or fake it, the demo asserts **both halves**,
because they answer different questions:

- the CLIENT refuses locally (`is_retired_on_v2()`, with `retired_replacement()`
  printed — `"client-side task tracking"` for `tasks/list`, `"tasks/get"` for
  `tasks/result`), which is what a pmcp caller will actually experience;
- the SERVER answers `-32601`, measured with one raw JSON-RPC frame per method
  through `StreamableHttpTransport::send_raw` + `receive()` — the same technique
  114-19 used for its `Mcp-Name` control. The transport still derives the v2
  routing headers and surfaces the JSON-RPC envelope riding the 404, so this is
  a raw FRAME, not a hand-rolled client. That half matters because clients from
  other SDKs will send these methods.

The reason both are needed is written into the fn's rustdoc.

### [Rule 2 — missing critical functionality] A FIFTH demo, so `key_links` names a call

The plan's frontmatter contracts a `key_link` from the example to
`src/client/mod.rs` `via` "the example drives the PRODUCTION client methods"
with `pattern: "tasks_update"`. With only the four planned demos the pattern
appeared **only in prose** — `wait_for_task_with_inputs` calls `tasks_update`
internally, which is correct (a hand-rolled loop is what the plan forbids) but
leaves the contract satisfied by a doc comment.

`demo_manual_update_for_your_own_scheduler` was added as demo 3 and is a real
`client.tasks_update(..)` call site. It also demonstrates `tasks_get_detailed`
(the one-round-trip status-conditional read) and the responder-less
`wait_for_task`, and it is the shape the enterprise/Lambda focus actually needs:
an agent whose scheduler is not the process holding the task.

All four PLANNED `demo_*` fns exist unchanged; this is additive.

### [Process] An in-process worker was required, and is not scaffolding

The plan does not mention one, but the lifecycle it asks for cannot complete
without it: `InMemoryTaskStore::deliver_task_inputs` transitions a fully-answered
task to `Working` and stops there (measured at `src/server/task_store.rs`, the
`delivery.complete && !delivery.accepted.is_empty()` arm). The tool handler
returned long before the input arrived. Something has to complete the task, and
in MCP's model that something is the application. The worker is 40 lines,
tolerates every store error (a task can expire or be cancelled between the
`list` and the write), and its rustdoc says plainly that it is NOT SDK machinery
and there is no SDK hook it should have been written against.

## Threat Model Coverage

| Threat | Disposition | Evidence |
|--------|-------------|----------|
| T-114-88 shared owner bucket | accepted, and STATED | a `# SHARED OWNER BUCKET — say it out loud` rustdoc section naming the consequence and the OAuth remedy; the owner is the named `SHARED_ANONYMOUS_OWNER`; binds loopback by default |
| T-114-89 treating a task payload as higher-trust | mitigated | the spec rule is in `s51`'s module rustdoc; `answer_all` reads `message` for display only and answers from its own configuration, executing nothing server-supplied |
| T-114-90 an example that passes regardless of behaviour | mitigated | every `demo_*` returns `Err` on divergence, `main` propagates; **TWO recorded controls show exit 1**, one loud and one silent |
| T-114-91 unbounded client-side input round loop | transferred to 114-19, and OBSERVED | the example drives `wait_for_task_with_inputs` rather than a local loop; NC-2 shows the bound firing at exactly 8 rounds |
| T-114-92 example drifting from the SDK via a re-implemented loop | mitigated | `grep -c 'loop {'` → 0, `grep -c 'tokio::time::sleep'` → 0 in `s51`; every step is a production client method |
| T-114-SC package installs | accepted | **ZERO** packages installed; `git diff --stat HEAD~2 HEAD -- Cargo.toml Cargo.lock` is EMPTY |

**Threat surface scan:** no new network endpoint, auth path, file access or
schema change beyond the register. The example server binds loopback by default
and is the surface T-114-88 already covers. **No threat flags.**

## Known Stubs

None. Every path either half describes is implemented and was exercised over a
real socket in the run pasted above.

## Verification

Every figure taken with `/usr/bin/make` / `/usr/bin/git` / `/usr/bin/grep` /
`~/.cargo/bin/cargo` — never the RTK proxy (114-15's recorded truncation trap) —
and only exit codes are treated as authoritative.

| Check | Command | Result |
|-------|---------|--------|
| build (server) | `cargo build --features full --example s50_v2_tasks_server` | **0**, no warnings |
| build (agent) | `cargo build --features full --example s51_v2_tasks_agent` | **0**, no warnings |
| clippy (examples, `make lint`'s flag set) | `cargo clippy --features full --example s50… --example s51…` | **0** — **zero** warnings naming either file |
| full gate | `make quality-gate` | **exit 0** |
| gate detail | — | **4899 passed / 0 failed / 81 ignored across 294 result lines**; 0 non-`ok.` lines; 0 truncation markers; **0** D-114-A keychain flakes |
| lint leg | inside the gate | `✓ No lint issues`, and `Checking examples...` |
| examples leg | inside the gate | `✓ Example s50_v2_tasks_server built successfully` and `✓ Example s51_v2_tasks_agent built successfully` — twice each (`make test-examples` runs standalone and again under `make validate-always`) |
| end to end | `s50` up, `s51` run | **both exit 0**, stdout pasted above |
| failure control NC-1 | route disabled | `s51` exit **1** |
| failure control NC-2 | ack without delivery | `s51` exit **1** |
| control revert | `shasum -a 256 -c` | **OK** after each |
| production diff | `git diff --stat HEAD~2 HEAD -- src/ crates/ Cargo.toml Cargo.lock tests/ fuzz/ schema/` | **EMPTY** |
| deletions | `git diff --diff-filter=D --name-only HEAD~2 HEAD` | **none** |

**Gate reconciliation: 294 result lines / 4899 passed is BYTE-IDENTICAL to
114-16's recorded figure.** That is the expected result and it is the check: this
plan adds no test binary and no lib test, so any movement in either number would
have been something else's.

**`cargo semver-checks` / `cargo public-api` / `make wasm-build` were NOT re-run,
deliberately** — the same reasoning 114-15 and 114-16 recorded. The production
diff is byte-empty, so those three answer questions about bytes that did not
move, and 114-14's results carry forward by construction. Additionally,
`make wasm-build` runs `cargo build --target wasm32-unknown-unknown` with no
`--examples`, so examples never participate in it; both files also carry
`#![cfg(not(target_arch = "wasm32"))]`.

**`.planning/REQUIREMENTS.md` is UNTOUCHED and `requirements mark-complete` was
deliberately NOT run.** TASK-01/02/04 flip as a GROUP with the rest of
TASK-01..06 under the phase's contract-first waiver, and
`114-SPEC-RECHECK.md`'s `## Verdict` is still `PENDING`. **114-18 owns the
flip.** `114-SPEC-RECHECK.md` was deliberately NOT edited either: this plan
landed no wire value; it CONSUMES 114-12's create trigger, 114-14's delivery and
114-19's client surface.

### One transient, diagnosed and NOT an example defect

A single `Error: Transport(Request("client error (Connect)"))` at demo 1 occurred
once, in the NC-2 harness, when the agent was launched **2 s after a `kill` +
immediate rebind of the same port**. The replacement server was verifiably
LISTENING (`lsof -nP -iTCP:8150 -sTCP:LISTEN` showed it, and it had printed
`Listening on : 127.0.0.1:8150`), and an immediate re-run of the agent against
the same process succeeded. This is a kill/rebind race in the harness — the
dying process can still hold the listener while the new one binds — not in
either example. The documented flow (start the server, see the banner, run the
agent in another terminal) has no such window, and the final confirmation run
used `sleep 3` after the kill and `sleep 4` after the start and passed.

## For the next plans

- **114-18 should cite the FILES, not this summary.** TASK-01's "one SDK serves
  both halves" and TASK-02's `tasks/update` round trip now have a runnable
  artifact: `examples/s50_v2_tasks_server.rs` + `examples/s51_v2_tasks_agent.rs`,
  with the end-to-end transcript reproducible by running two commands.
- **CLAUDE.md's ALWAYS list is satisfied for the tasks feature.** A runnable
  `cargo run --example` exists for the v2 tasks surface, and it is an assertion
  rather than a printout. Note that `make test-examples` only BUILDS examples
  (its own output says so) — the run above is the only executed proof, and a
  future plan that wants it enforced in CI has to add a harness, not a Makefile
  flag.
- **Phase 117 / CLNT-03 inherits a de-risked surface.** Demo 3 is the shape
  `pmcp-agent` will want (`tasks_get_detailed` + `tasks_update` under the
  agent's own scheduler); demo 2 is the shape a simple agent wants
  (`wait_for_task_with_inputs`). Both are exercised.
- **A trap worth carrying:** a task whose inputs are fully delivered stops at
  `Working`. Nothing in the SDK completes it. Every task-serving deployment needs
  an application-side worker, and a plan that assumes `tasks/update` completes a
  task will produce a demo that hangs. `s50::run_worker` is the reference shape.
- **A second trap:** on v2 the create trigger is the CLIENT's declaration. A
  `TaskSupport::Required` tool called by a non-declaring v2 client does not
  error — it logs a warning and returns an ordinary `CallToolResult` whose text
  contains the handler's FABRICATED task id. Any test or example asserting "no
  `taskId` appears anywhere in the response" will fail for the wrong reason;
  assert on `ToolCallResponse::Result` and on the absence of
  `_meta.relatedTask` instead.

## Commits

| Hash | Message |
|------|---------|
| `06842101` | `docs(114-17): add the v2 tasks server half — pause, update, resume` |
| `4af27759` | `docs(114-17): add the autonomous v2 tasks agent half` |

## Self-Check: PASSED

- `examples/s50_v2_tasks_server.rs` — **FOUND** (406 lines, `min_lines: 200`)
- `examples/s51_v2_tasks_agent.rs` — **FOUND** (615 lines, `min_lines: 200`)
- Commits `06842101`, `4af27759` — **FOUND** in `git log --oneline --all`
- `key_links` contract: `grep -n '\.tasks_update(' examples/s51_v2_tasks_agent.rs`
  → line **414**, a real call site
- `must_haves` truths: two commands drive create → `input_required` → update →
  terminal (transcript above); the client half has **0** stdin reads and **0**
  local loops; the examples exit **1** under both negative controls
