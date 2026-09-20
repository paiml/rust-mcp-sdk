# Phase 114: Tasks Extension Migration - Research

**Researched:** 2026-07-28
**Domain:** MCP protocol wire-API migration (Rust SDK, dual-version server + client)
**Confidence:** HIGH for wire shapes (authoritative schema located and read); HIGH for codebase facts (measured); MEDIUM for the schema-hold policy interaction (see F1/D-18)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

Copied verbatim from `114-CONTEXT.md` `## Implementation Decisions`.

**Extension negotiation surface (TASK-01)**

- **D-01:** **Auto-advertise, inheriting the endpoint-backed rule.** The existing
  `apply_tasks_capability_rule` (`src/server/task_dispatch.rs`) gains a v2 arm: a configured
  `TaskStore` **or** `TaskRouter` auto-populates the
  `extensions["io.modelcontextprotocol/tasks"]` entry, under the same additive-only
  discipline (an explicitly-configured extensions value is preserved verbatim). One knob,
  not two — matching 112 D-09's `server/discover` reasoning. Rejected: a separate explicit
  builder opt-in (a v2 server with a working store would silently serve nothing until
  someone found the second knob, and every existing tasks server would need a code change).

- **D-02:** **Era-projected capability surfaces.** The struct carries everything, but the
  serialization boundary projects per era: a **v1 `initialize` response is byte-identical to
  today** (no `extensions` key — Phase 112's `default_serializes_without_extensions_key`
  test stays true), while a **v2 `server/discover` shows the extensions entry and drops the
  v1 `tasks`/`experimental.tasks` keys**. Same shape as 112 D-07's "`resultType` on v2
  responses only". Rejected: additive-in-both-eras (changes v1 bytes for every existing
  tasks server and advertises a v2 extension to clients that cannot use it); also rejected:
  suppressing the whole `experimental` block on v2 (this phase does not own the other keys
  in it).

- **D-03:** **The extension value is `{}` on the wire, typed structure-ready in Rust.**
  Serializes as an empty object today — spec-literal, invents nothing before the final
  schema — while the Rust type behind it is a struct whose future fields are
  `skip_serializing_if`, so the published 2026-07-28 schema can fill it without a
  public-API break. This is the Phase-112 error-code-table discipline (structure now, values
  from the published schema only) applied to a capability. Rejected: projecting
  `default_tasks_capability()`'s list/cancel flags — it would advertise `list: true` on an
  era where `tasks/list` answers `-32601`, i.e. exactly the capability lie the endpoint-backed
  rule exists to prevent.

- **D-04:** **The client gates on the extensions map.** On v2 the `Client`'s existing
  era-aware `assert_capability` (landed by 113-05) reads
  `extensions["io.modelcontextprotocol/tasks"]` instead of `capabilities.tasks`, and a call
  to an un-negotiated tasks method **fails fast with a typed error before the round trip**.
  Reuses the 113-05 mechanism rather than adding a second one, and gives Phase 117's agent
  task polling a clean precondition. Symmetric with how the server refuses.

- **D-05:** **Dual-surface proof = a paired runnable example with an AGENT-shaped client.**
  The phase ships a server example plus a client example whose client half is an
  **autonomous poll loop** (create → poll `tasks/get` → `tasks/update` → terminal), not an
  interactive chat client — proving the surface works for the ReAct/agent client shape that
  113 D-07 made first-class, and de-risking CLNT-03 before Phase 117 starts. Follows the
  113-11 precedent (`examples/s47_v2_stateless_mrtr.rs` + `examples/s48_v2_mrtr_client.rs`)
  and satisfies CLAUDE.md's ALWAYS-an-example rule.

- **D-06:** **Boundary with Phase 117.** This phase's client half is agent-*shaped*; wiring
  it into the actual `pmcp-agent` crate remains CLNT-03 / Phase 117, which inherits a
  de-risked surface rather than a blank one.

**v2 owner binding, fail-closed (TASK-05)**

- **D-07:** **Reuse Phase 113's three-row identity table verbatim.** `resolve_mrtr_principal`
  (`src/server/core.rs:1579`) is the pattern the roadmap means by "the stateless
  per-request-identity pattern owner-binding reuses":

  | authenticated_subject | has_auth_provider | owner |
  |---|---|---|
  | `Some(sub)` | any | `sub` |
  | `None` | `true` | **refuse** |
  | `None` | `false` | `ANONYMOUS_PRINCIPAL` |

  One identity table for every v2 ingress path on the server. **State the caveat plainly in
  docs, do not imply it:** on a server with no auth provider at all, every v2 caller still
  shares one bucket — defensible only because such a server has no notion of caller
  identity. Rejected: hard fail-closed on every v2 server (a plain `cargo pmcp dev` / stdio
  loop could not exercise v2 tasks, and D-05's paired example gets much harder to ship);
  rejected for now: a configurable proxy-header identity source (see Deferred).

- **D-08:** **The refusal is `AUTHENTICATION_REQUIRED` (-32003) at HTTP 200, fired before
  the params parse.** Exactly the shape 113-23 landed for `subscriptions/listen`: the
  original id is echoed, and the refusal sits **after** the `-32601` era/negotiation gates
  and **before** deserializing params — so a non-negotiated server still answers
  "no such method" first, and a refused caller's body is never parsed. An existing constant:
  no new wire value is minted under the schema hold. Rejected: `-32602` (nothing is wrong
  with the params) and `-32601`/`V2_TASKS_NOT_NEGOTIATED` (untruthful, collides with a
  genuinely different condition, and makes the "just authenticate" fix undiscoverable).

- **D-09:** **The TASK-05 security test is live-socket and per-method.** Two distinct
  authenticated principals over a real socket: A creates; B attempts **`tasks/get`,
  `tasks/update` AND `tasks/cancel`** with A's `taskId` and gets `NotFound` — never
  `OwnerMismatch` (PROJECT.md's standing no-info-leak key decision). Plus an explicit
  assertion that the **v1 `"local"` bucket and v2's `ANONYMOUS_PRINCIPAL` are disjoint**, so
  a v1 caller cannot reach a v2 caller's tasks on a no-auth server. Each method's guard is
  proven load-bearing by **its own** negative control. Rejected: a unit matrix over the
  identity table alone — that is precisely the shape 113-31 caught as insufficient
  ("the tests that would have failed did not exist").

- **D-10:** **v1 owner binding stays byte-identical, plus a migration warn.** The `"local"`
  fallback and all v1 wire behavior are untouched — the new rule is reachable only through
  the era gate — but an unauthenticated task owner on v1 emits a `tracing::warn!` naming the
  shared bucket and pointing at v2. Zero wire/fixture change; makes the security improvement
  a reason to migrate rather than a surprise. Rejected: tightening v1 too (breaks the
  milestone's v1-untouched promise; that is a major-version decision).

**Which backend serves v2 (TASK-06)**

> **Grounding the planner should not re-derive:** the two backend paths are NOT peers.
> `pmcp::server::task_store::TaskStore` (`src/server/task_store.rs`) has exactly one
> production impl, `InMemoryTaskStore`. **DynamoDB and Redis live only in
> `crates/pmcp-tasks/src/store/`**, behind *pmcp-tasks' own* `TaskStore` trait +
> `GenericTaskStore<B>` + `StorageBackend`, and reach pmcp solely through
> `TaskRouterImpl: TaskRouter` across the `serde_json::Value` seam. TASK-06's "TaskStore
> trait, state machine, and DynamoDB/Redis/in-memory backends" refers to the **pmcp-tasks**
> side.

- **D-11:** **Both paths serve v2; the reshape lives entirely in `task_dispatch.rs`, above
  the `Value` boundary.** v2 routes through the same store-first → router-fallthrough
  dispatch v1 uses; the era gate, status projection and envelope all sit above the seam.
  `crates/pmcp-tasks` needs **no change to serve v2**, so DynamoDB/Redis-backed tasks work
  on v2 from day one. This is the literal reading of "a wire-API reshape behind the
  `TaskRouter` boundary", and it is that boundary earning its keep (a PROJECT.md key
  decision already marked ✓ Good). Rejected: serving v2 only from the in-crate store — v2
  tasks would be in-memory-only in practice, and the milestone's "the v1.x DynamoDB/Redis
  investment survives" claim would not hold for the v2 path.

- **D-12:** **`tasks/update` reaches the backends via defaulted methods + a capability probe
  on BOTH traits.** `TaskRouter::handle_tasks_update` defaulted to a not-supported error
  (the existing `create_workflow_task` shape); `TaskStore` gains a defaulted
  input-delivery method alongside a `supports_inputs()` probe (the existing
  `set_result`/`get_result`/`supports_results()` shape). Purely additive — every existing
  implementor still compiles and `cargo semver-checks` stays green (223/223 today) — and
  dispatch can answer honestly when a backend cannot accept inputs instead of pretending.
  Rejected: a separate opt-in trait (a second registration knob, cutting against the
  one-knob-per-side steer) and expressing update through existing methods only (pushes typed
  inputs into a channel designed as a shared scratchpad).

- **D-13:** **Two impls, three production backends.** Input delivery is implemented in
  **`GenericTaskStore<B>`** — which is what that type exists for ("domain logic once,
  backends are dumb KV stores"; "3 backends share identical domain logic; zero divergence")
  so memory + DynamoDB + Redis all gain `tasks/update` — **and** in the in-crate
  `InMemoryTaskStore`, so the core path and D-05's paired example work standalone.
  **Stated explicitly because TASK-06's text says "unchanged":** this touches
  `crates/pmcp-tasks` **additively** (one new method, no rewrite), which is inside the
  reshape-not-rewrite line but should not be discovered in review. Rejected: in-crate only
  (the one genuinely-new v2 feature would not work on the production backends) and
  seam-only (TASK-02 says a client *can* feed input — with no implementor that cannot be
  demonstrated end to end).

- **D-14:** **TASK-06's evidence bar is four-part**, all required:
  1. the existing `pmcp-tasks` storage/tasks suite green **unmodified**;
  2. v1 `tasks/*` responses asserted **byte-identical** against golden fixtures — not merely
     "tests pass"; a reshape is exactly the change that alters bytes while keeping
     assertions true;
  3. `cargo semver-checks` still **223/223**;
  4. a **dev-dependency-free build** across the `dynamodb`/`redis` feature combinations, so
     the Phase-109 feature-unification false-green (`cargo test --all-features` masks
     feature-flag gaps because the dev-dep on `pmcp` with `full` unifies features) **cannot
     recur on the very crate where it already occurred once**.

**v2 retrieval + `tasks/update` (TASK-02, TASK-03, TASK-04)**

> **Grounding:** today's `Task` carries `taskId/status/ttl/createdAt/lastUpdatedAt/
> pollInterval/statusMessage/diagnosticDetail` and has **no `result`, `error` or
> `inputRequests`** fields. Today's `CreateTaskResult` is **nested** (`{ task: {...} }`)
> while TASK-04 specifies a **flat** `{taskId,status,ttlMs,pollIntervalMs}` — so `ttl`→
> `ttlMs` and `pollInterval`→`pollIntervalMs` are renames, not just re-nesting. The v1
> 5-state enum is already **name-identical** to the v2 status enum
> (`working|input_required|completed|failed|cancelled`), so TASK-04's "maps
> deterministically" is likely a **locking tripwire**, not a translation table — planner
> should confirm rather than assume a mapping is needed.

- **D-15:** **On v2, `tasks/get` inlines the result; `tasks/result` and `tasks/list` answer
  `-32601`.** When the task is terminal and the store `supports_results()`, dispatch folds
  `store.get_result` into the Task payload's `result` (and `error`) field — the spec's own
  shape, one round trip. This extends the era gate 113-29 already opened on `tasks/result`,
  and `tasks/list` gets the same treatment via 112 D-10's mechanism (112 D-10 names this
  case explicitly: *"the same gate mechanism `tasks/list` will use in the opposite
  direction in Phase 114"*). **v1 keeps both methods exactly as today.** Rejected: keeping
  `tasks/result` alive on v2 as a pmcp extension (contradicts TASK-03 and is the divergence
  Phase 118's conformance run exists to surface); rejected: a result-opt-in request
  parameter (invents a parameter the unpublished schema does not define).

- **D-16:** **`tasks/update` is atomic: `InputRequired` → `Working` in one CAS write.**
  Delivering inputs to a task in `InputRequired` persists them and transitions to `Working`
  in a single compare-and-set, reusing the existing validated 5-state machine (46 transition
  tests) and the `put_if_version` every backend already implements — no new concurrency
  semantics appear. Any other source state is **refused** (a completed or cancelled task
  cannot be fed). Two concurrent updates: first writer wins, second sees the version
  conflict. Rejected: store-only delivery (leaves a window where a task sits in
  `InputRequired` with inputs delivered and a poller cannot distinguish "delivered" from
  "ignored") and a per-tool configurable knob.

- **D-17:** **`tasks/update` reuses Phase 113's MRTR input types AND its kind-directed
  decoding, with the kinds taken from the persisted Task record.** One public input model
  across MRTR and tasks (113 D-10's "one handler-facing type"), decoded via `decode_for`
  against the kinds the *server itself* recorded in the task's `inputRequests` — the task
  record is the analogue of 113's AEAD-sealed continuation. **This is a security decision,
  not a convenience one:** 113-27 closed **D-113-O**, where untagged decoding (Roots, then
  Sampling, then Elicitation — first that fits) mis-typed an elicitation answer as Sampling
  because `ElicitResult` and `CreateMessageResult` overlap, and the handler **re-elicited 16
  times** before dying on a misleading error. Any independent decoder that guesses at
  overlapping shapes reproduces that bug class. Reuse also inherits 113-02's five DoS
  bounds. Research corroborates the shape: the `inputs` map mirrors `inputResponses`.

- **D-18:** **The whole phase holds at `[~]` pending the final schema**, exactly as Phase
  113 is booked — implement structure-first, mark all six TASK requirements
  *implemented; pending final schema*, and flip them together when a versioned schema
  directory exists upstream. Inherits 113's recorded `hold` policy
  (`113-SPEC-RECHECK.md` § Third Outcome Policy, decided 2026-07-27) and its **condition**
  trigger — *"a versioned schema directory exists"*, not a date. **Known tradeoff, recorded
  so the planner sizes reviews accordingly and does not rediscover it:** this repeats the
  failure mode 113 named when it split HTTP-04 — a phase whose requirements cannot partially
  close, where each review reopens all six. The alternative (splitting TASK-01/03/05/06 as
  schema-independent and holding only the wire-exact TASK-02/04) was presented and **not**
  chosen; uniform consistency with 113's bookkeeping was preferred.

### Claude's Discretion

- Exact naming/placement of the era-projection site for capabilities (D-02) and of the
  flat-vs-nested `CreateTaskResult` projection relative to `inject_v2_result_envelope`.
- How era + `has_auth_provider` get threaded into `TaskDispatch::resolve_owner`, which today
  takes only `auth_context` (`src/server/task_dispatch.rs:398`).
- Method naming for the new defaulted trait methods and the capability probe (D-12).
- Whether TASK-04's "deterministic mapping" is satisfied by a locking tripwire over the
  already-name-identical enums or genuinely needs a conversion — confirm by measurement.
- Where the `result`/`error`/`inputRequests` fields live on `Task` (additive fields with
  `skip_serializing_if` vs a v2-only projection type), subject to D-02's v1-byte-identity lock.
- TTL/quota limit keying off the new v2 principal — not raised in discussion; use the
  existing `TaskSecurityConfig` conventions.

### Deferred Ideas (OUT OF SCOPE)

- **Unsolicited / server-directed task handles** — a v2 server minting `resultType:"task"`
  without a client `task` field. The spec marks it **MAY**, so declining is fully
  conformant, and none of TASK-01..06 requires it — but **PROJECT.md's v2.5 feature list
  names "server-directed creation"**, so it is recorded here rather than lost. Real
  motivation when it is picked up: a Lambda-style server returning a handle instead of
  holding a request open. Needs a scoping pass on the client-compatibility question (a v2
  client must then handle a task handle back from any task-capable tool).
- **A configurable proxy-header / claim-based identity source** for v2 owner binding —
  TASK-05's wording ("OAuth `sub` **or** a stable per-request identity") permits it, and the
  deployment shapes exist (Lambda `oauth_passthrough`, pmcp.run's proxy). Deferred because a
  header-derived identity is only as trustworthy as the proxy in front of it and needs its
  own opt-in + threat-model pass. D-07's table is the baseline it would extend.
- **Splitting TASK-02/04 (wire-exact) from TASK-01/03/05/06 (schema-independent)** so the
  latter can close on the merits without a publication event — presented and not chosen
  (D-18). Worth revisiting if the phase stalls in review for the reason 113's HTTP-04 split
  was created to fix.
- **Per-tool configurability of the `tasks/update` transition** (auto vs handler-driven) —
  rejected as an extra knob (D-16); revisit only if a real handler needs to batch several
  input deliveries before proceeding.
- **UNAS-01 (SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}`)** — still unassigned milestone-wide;
  **not** absorbed here. Do not fold it into this phase without an explicit scoping decision.

> ⚠ **Two deferrals need re-reading against research findings F5 and F18 before the planner
> treats them as settled.** See `## Open Questions` Q1 and Q2. F5 in particular shows that
> the first deferral above, read literally, makes v2 task *creation* unreachable.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TASK-01 | Tasks negotiated on v2 via the extensions map (`io.modelcontextprotocol/tasks`); v1 `experimental.tasks` negotiation continues to work | F1 (authoritative schema), F2 (tasks removed from core spec), F6 (**gap: `ClientCapabilities` has no `extensions` field** — the client-declares half is unimplementable today), `## Code Examples` §1 (exact wire bytes both directions), `ServerCapabilities.extensions` exists (capabilities.rs:109) |
| TASK-02 | A client can feed input into a running task via `tasks/update` | F4 (**param is `inputResponses`, NOT `inputs`**), F16 (`TasksUpdate` variant + the 5 match sites + the MRTR-eligibility compile tripwire), D-17 reuse targets measured (`decode_for` mrtr.rs:438, `InputResponses` mrtr.rs:523, 5 DoS bounds mrtr.rs:820-832), `## Code Examples` §3 |
| TASK-03 | `tasks/list` (and blocking `tasks/result` per final spec) era-gated off on v2, fully functional for v1 | F2 (core spec has zero task types), F9 (`tasks/list` + `tasks/result` absent from the extension schema — only get/update/cancel exist), `is_v1_task_era` (task_dispatch.rs:89) is the landed predicate to extend |
| TASK-04 | v2 task-augmented results use `resultType:"task"` with `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`; v1 5-state machine maps deterministically to the v2 status enum | F3 (**requirement text is incomplete — `createdAt`/`lastUpdatedAt` are REQUIRED, `pollIntervalMs` is optional**), F15 (**mapping confirmed name-identical → a locking tripwire, no conversion**), `ResponseDisposition::Task` already scaffolded (core.rs:1151) |
| TASK-05 | On v2, owner binding requires OAuth `sub` or a stable per-request identity, fails closed when absent; security test proves no cross-caller visibility | F8 (**task-not-found is `-32602` on v2, MUST for `tasks/get`** — pmcp currently emits `-32603`), F11 (**`ANONYMOUS_PRINCIPAL` = `""` is REJECTED by pmcp-tasks by default**), F7 (`-32003` triple-meaning collision affects D-08), spec § Security Considerations corroborates (`## Security Domain`), owner-prefixed storage key (generic.rs `make_key`) |
| TASK-06 | `TaskStore` trait, state machine, DynamoDB/Redis/in-memory backends survive unchanged — wire reshape behind the `TaskRouter` boundary, not a storage rewrite | F12 (**D-13's "two impls" is really THREE sites** — the pmcp-tasks `InMemoryTaskStore` delegating wrapper), F13 (pmcp-tasks unpublished → semver-checks does not cover it), F14 (`make test-feature-flags` already implements D-14 item 4), F19 (**no v1 golden byte fixtures exist — Wave 0**), 197 pmcp-tasks integration tests incl. 46 state-machine transitions |

</phase_requirements>

## Summary

**The single most important finding: the authoritative wire schema for this extension exists,
is machine-readable, and was read in full during this research.** `modelcontextprotocol/ext-tasks`
@ `main` carries `schema/draft/schema.ts` (374 lines), `schema/draft/schema.json` (46,903 bytes)
and `specification/draft/tasks.md` (910 lines). CONTEXT.md's grounding notes repeatedly hedge
wire questions as "confirm against the final schema"; almost all of them are now answered
definitively, and several answers **contradict** the v2.5 research pack that CONTEXT.md's
decisions were built on. The corrections are not cosmetic: `tasks/update`'s parameter is
`inputResponses` (not `inputs`), `CreateTaskResult` requires five fields (not the four TASK-04
names), `tasks/cancel` and `tasks/get` both reshape on v2 in ways CONTEXT.md never mentions,
and — most consequentially — **v2 task creation is server-directed by construction, with no
client `task` field anywhere in the extension.** CONTEXT.md defers "server-directed task
handles" as an optional MAY-extra; read literally, that deferral makes v2 task creation
unreachable, because `task_requested` (v1's `CallToolRequest.task`) is pmcp's only creation
trigger and v2 has no such field.

**The second-most important finding is a landmine inside Phase 113's landed code.**
`own_reserved_result_fields` (`src/server/core.rs:1299-1337`) unconditionally REMOVES the
top-level `inputRequests` key from any v2 result whose disposition is not `InputRequired`.
The v2 `tasks/get` response for an `input_required` task carries a top-level `inputRequests`
field that the schema marks **required**. As the code stands, that field would be silently
deleted with a `tracing::warn!`, producing a schema-invalid response — and because the
deletion is silent-by-design, an integration test asserting only "the request succeeded"
would pass. Phase 113's reserved-field registry must be extended, not worked around.

Beyond those, the phase is well-scoped and CONTEXT.md's architectural spine holds up under
measurement: the `TaskRouter` `serde_json::Value` seam really does isolate the reshape;
`ResponseDisposition::Task` really is pre-scaffolded; `resolve_mrtr_principal`'s three-row
table really is the right identity pattern; and D-17's MRTR reuse is *more* directly
applicable than CONTEXT.md knew (same key name, same type shape). Three smaller measured
corrections tighten the plan: `ANONYMOUS_PRINCIPAL` (`""`) is rejected by pmcp-tasks' default
`allow_anonymous: false`, so D-07's third row cannot create a task on a production backend
without configuration; D-13's "two impls" is really three sites because pmcp-tasks'
`InMemoryTaskStore` is a delegating wrapper that would silently inherit a not-supported
default; and `make test-feature-flags` already implements D-14's fourth evidence item, so it
should be reused rather than reinvented.

**Primary recommendation:** Vendor `ext-tasks` `schema/draft/schema.ts` + `schema.json` into
the repo at a pinned commit and drive every wire shape from it (the Phase-112
`error_codes.rs` PROVENANCE-comment discipline, applied to a whole extension). Then, before
any implementation plan is written, resolve the two scope questions research surfaced (Q1:
server-directed creation is mandatory, not deferrable; Q2: the `inputRequests`-stripping
collision) — both change task decomposition, not just task content.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Extension capability advertisement (server half, TASK-01) | Capability computation at build time (`apply_tasks_capability_rule`, `builder.rs:1051`) | Era-projected serialization (`discover_result_from_capabilities`, `core.rs:1104`) | The struct is computed once at build; the era projection is a serialization concern (D-02) |
| Client extension declaration (client half, TASK-01) | Client per-request `_meta` emission (`src/client/mod.rs`) | `ClientCapabilities` type (`src/types/capabilities.rs:25`) | Spec puts it in per-request `_meta.clientCapabilities`, not a handshake — matches v2's stateless model |
| Server reading the client's declaration | Request ingress → `ProtocolContext.client_capabilities` (`context.rs:115`) | `RequestHandlerExtra::client_capabilities()` (`cancellation.rs:406`) | Already-resolved-once-at-ingress is Phase 112's spine; do not re-parse |
| Era gating of `tasks/list`, `tasks/result` (TASK-03) | `TaskDispatch::route_tasks_endpoint` (`task_dispatch.rs:812`) | — | "every era gate belongs here" — the single store-vs-router precedence site |
| `resultType:"task"` discriminator (TASK-04) | Serialization envelope (`inject_v2_result_envelope`, `core.rs:1201`) | Create gate selects the disposition (`maybe_build_task_created`, `task_dispatch.rs:557`) | 112 D-08's model: dispatch selects, the shared helper emits |
| Flat-vs-nested result projection (TASK-04) | `task_dispatch.rs` (above the `Value` seam) | `src/types/tasks.rs` types | D-11: the reshape lives above the seam; types stay additive |
| Owner binding + fail-closed refusal (TASK-05) | `TaskDispatch::resolve_owner` (`task_dispatch.rs:398`) | Identity table shared with `resolve_mrtr_principal` (`core.rs:1579`) | One identity table per server for every v2 ingress path (D-07) |
| `tasks/update` input decoding (TASK-02) | `src/types/mrtr.rs` `decode_for` (`:438`) | Task record supplies the kinds | D-17: reuse the kind-directed decoder; never re-derive |
| Atomic `InputRequired`→`Working` CAS (TASK-02, D-16) | `GenericTaskStore<B>` (`crates/pmcp-tasks/src/store/generic.rs`) | `StorageBackend::put_if_version` (`backend.rs:217`) | Domain logic once; backends are dumb KV stores |
| Persistence of delivered inputs | `crates/pmcp-tasks` (Dynamo/Redis/memory) + in-crate `InMemoryTaskStore` | — | D-13, corrected to three sites by F12 |
| Header routing (`Mcp-Name` = taskId) | Client emitter (`logical_name_key`, `mrtr.rs:195`) | Server cross-check (`cross_check_name`, `streamable_http_server.rs:1012`) | F10: client-side MUST; server side already tolerant |

**Tier-assignment traps this map exists to prevent:**
- Putting the era gate in `server/mod.rs` or `core.rs` per-site instead of `task_dispatch.rs`
  — the module doc says the routing logic "lives HERE, once, never as a divergent second copy."
- Putting the flat/nested projection in `src/types/tasks.rs` as a serde change — that would
  alter **v1** bytes and break D-02's byte-identity lock.
- Reading the client's extension declaration off the raw body a second time instead of the
  already-resolved `ProtocolContext` (Phase 112's whole point).

## Standard Stack

### Core

**No new runtime dependencies.** This phase is a wire-API reshape over existing machinery.
Verified against `.planning/research/STACK.md`'s zero-new-runtime-deps constraint and by
measurement: every type, trait, codec and bound this phase needs already exists in-tree.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` / `serde_json` | 1.0 (in-tree) | Wire (de)serialization; the `TaskRouter` `Value` seam | Already the SDK's only serialization stack [VERIFIED: root Cargo.toml] |
| `uuid` | in-tree | Task ID minting (`Uuid::new_v4`) | Already used by both stores; 122 random bits satisfies the spec's unguessability MUST (F20) [VERIFIED: `src/server/task_store.rs:503`] |
| `async_trait` | in-tree | The `TaskStore` / `TaskRouter` / `StorageBackend` traits | House convention (CLAUDE.md) [VERIFIED: codebase] |
| `tracing` | in-tree | D-10's v1 migration warn; reserved-field override warns | Established pattern at every override site [VERIFIED: `core.rs:1311`] |

### Supporting (in-tree assets, not dependencies)

| Asset | Location | Purpose | When to Use |
|-------|----------|---------|-------------|
| `ResponseDisposition::Task` | `src/server/core.rs:1151` | The `resultType:"task"` discriminator | TASK-04 — already scaffolded *for this phase*; `as_wire_str()` already returns `"task"` |
| `resolve_mrtr_principal` | `src/server/core.rs:1579` | The three-row identity table | TASK-05 / D-07 — reuse verbatim, do not re-derive |
| `ANONYMOUS_PRINCIPAL` | `src/server/core.rs:1450` (`= ""`) | The no-auth-provider bucket | D-07 row 3 — **but see F11 before using it with a pmcp-tasks backend** |
| `is_v1_task_era` | `src/server/task_dispatch.rs:89` | The landed era predicate | TASK-03 — extend its *use*; the `-32002` behavior it guards is frozen |
| `decode_for` + `InputResponses` | `src/types/mrtr.rs:438`, `:523` | Kind-directed input decoding | TASK-02 / D-17 — the security-bearing reuse |
| The five MRTR DoS bounds | `src/types/mrtr.rs:820-832` | 64 entries / 64 KiB each / 256 KiB total / depth 32 | TASK-02 — inherited free by reusing the types |
| `StorageBackend::put_if_version` | `crates/pmcp-tasks/src/store/backend.rs:217` | The CAS primitive | D-16 — every backend already implements it |
| `TaskStatus::can_transition_to` | `src/types/tasks.rs:56` | The validated 5-state machine | D-16 — 46 transition tests already pin it |
| `make test-feature-flags` | `Makefile:301-336` | 4-row pmcp-tasks feature matrix | D-14 item 4 — already exists (F14) |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Vendoring `ext-tasks` schema at a pinned commit | Re-fetching at plan time | Vendoring is the Phase-112 `error_codes.rs` PROVENANCE discipline and survives upstream force-pushes; re-fetching cannot be reviewed offline. **Recommend vendoring.** |
| Additive `result`/`error`/`inputRequests` on `Task` | A v2-only `DetailedTask` projection type | Additive fields risk v1 byte drift (D-02 lock) if any `skip_serializing_if` is missed; a separate projection type costs a conversion but makes the v1 lock structural. **Planner's discretion per CONTEXT.md; research leans projection type** because the spec models these as five *distinct status-discriminated variants* with per-variant required fields (F3), which a single flat struct cannot express. |
| Reusing `-32003` for D-08's refusal | `-32021 MISSING_REQUIRED_CLIENT_CAPABILITY` for the *negotiation* refusal, keeping `-32003` for auth only | F7 shows `-32003` carries three meanings across pmcp + the two spec documents. Separating them is more truthful and both codes already exist. **See Q3.** |

**Installation:** none — no new packages.

**Version verification:** N/A (no new packages). Existing dependency versions verified in
the root `Cargo.toml`; `cargo semver-checks` baseline is `pmcp` 2.17.0 at 223/223.

## Package Legitimacy Audit

**NOT APPLICABLE — this phase installs zero external packages.**

Per `.planning/research/STACK.md`'s zero-new-runtime-deps constraint (v2.5 milestone-wide)
and confirmed by measurement: every type, trait, codec, bound and test harness this phase
requires already exists in-tree. No registry lookup, slopcheck run, or postinstall audit is
required because no package is added.

**If a plan later proposes any new dependency**, the Package Legitimacy Gate must be run
before that plan executes, and the addition must be justified against the milestone's
zero-new-deps constraint (which is a locked requirement, not a preference).

| Package | Registry | Disposition |
|---------|----------|-------------|
| *(none)* | — | No packages added by this phase |

**Packages removed due to slopcheck [SLOP] verdict:** none (no packages evaluated)
**Packages flagged as suspicious [SUS]:** none (no packages evaluated)

## Architecture Patterns

### System Architecture Diagram

```
                        ┌──────────────── v2 CLIENT (agent-shaped, D-05) ────────────────┐
                        │  server/discover ──► extensions map                            │
                        │       │                                                        │
                        │       ▼                                                        │
                        │  assert_capability("tasks")  ── era-aware (D-04) ──► fail fast  │
                        │       │                         reads extensions[…/tasks] on v2 │
                        │       ▼                                                        │
                        │  tools/call + _meta.clientCapabilities.extensions[…/tasks]={}  │
                        │       │            ▲ F6 GAP: ClientCapabilities has no          │
                        │       │              `extensions` field → key silently dropped   │
                        │       ▼                                                        │
                        │  poll loop: tasks/get ─► input_required? ─► tasks/update ─► …   │
                        │              Mcp-Name MUST = params.taskId (F10 client gap)     │
                        └───────────────────────────┬────────────────────────────────────┘
                                                    │ HTTP (streamable, stateless)
    ════════════════════════════════════════════════▼════════════════════════════════════
                              SERVER INGRESS (Phase 112 spine)
     classify_v2_request ──► require_three_headers ──► cross_check_method / cross_check_name
       │                       (Mcp-Method, Mcp-Name, MCP-Protocol-Version)
       │                       tasks/* are NOT name-bearing ⇒ value not cross-checked (SAFE)
       ▼
     ProtocolContext resolved ONCE  { era, protocol_version, client_info, client_capabilities }
       │
       ├──── era == V1 ─────────────────────────────────► v1 path: BYTE-IDENTICAL (D-02/D-10)
       │                                                  tasks/get|result|list|cancel unchanged
       ▼ era == V2
     ┌──────────────────── TaskDispatch (task_dispatch.rs) — ALL era gates live here ─────┐
     │                                                                                    │
     │  ① NEGOTIATION GATE   extension advertised?  ──no──► -32601 V2_TASKS_NOT_NEGOTIATED│
     │        │                                                                           │
     │  ②  ERA GATE          tasks/list, tasks/result on v2 ────────► -32601 (TASK-03)     │
     │        │                                                                           │
     │  ③  OWNER BINDING     three-row identity table (D-07, shared w/ resolve_mrtr_…)     │
     │        │              (None, has_auth_provider=true) ──► REFUSE before params parse │
     │        │                                                   -32003 @ HTTP 200 (D-08) │
     │        │              (None, false) ──► ANONYMOUS_PRINCIPAL ("")                    │
     │        │                                 ⚠ F11: rejected by pmcp-tasks by default   │
     │        ▼                                                                            │
     │  ④  PARAMS PARSE      tasks/update: inputResponses  (F4 — NOT `inputs`)             │
     │        │              kind-directed decode_for, kinds FROM the task record (D-17)   │
     │        ▼                                                                            │
     │  ⑤  ROUTE             store-first  ──NotFound/unsupported──►  router fall-through   │
     │        │                                                                            │
     └────────┼────────────────────────────── serde_json::Value seam ─────────────────────┘
              │                                    (the boundary earning its keep)
      ┌───────┴────────┐                                    │
      ▼                ▼                                    ▼
  pmcp in-crate    TaskRouterImpl ──► pmcp-tasks TaskStore ──► GenericTaskStore<B>
  InMemoryTaskStore                   │                          │  D-16 atomic CAS:
  (D-13 site 3)                       │                          │  InputRequired→Working
                                      ▼                          │  via put_if_version
                          InMemoryTaskStore (delegating           ▼
                          wrapper — D-13 site 2, F12)   StorageBackend: memory | DynamoDB | Redis
                                                        (D-13 site 1 covers Dynamo+Redis directly)
              │
    ══════════▼══════════════════════════════════════════════════════════════════════════
                       RESPONSE EGRESS (shared, twin-site)
     mrtr.finish() ──► disposition  { Complete | InputRequired | Task }
              │        ▲ TASK-04: the create path must select `Task` here
              ▼
     inject_v2_result_envelope (core.rs:1201) ── v2-only, object-results-only
              │
              ▼
     own_reserved_result_fields (core.rs:1299)
        • WRITES  resultType = disposition.as_wire_str()
        • WRITES  _meta["io.modelcontextprotocol/serverInfo"]
        • ⚠⚠ REMOVES top-level `inputRequests` unless disposition == InputRequired
              └─► COLLISION: v2 tasks/get on an input_required task MUST carry
                  top-level `inputRequests` (schema: required). It would be
                  SILENTLY DELETED. See Pitfall 1 / Q2 — highest-severity finding.
```

### Recommended Project Structure

No new modules. The reshape lands in existing files:

```
src/
├── server/
│   ├── task_dispatch.rs      # ALL era gates, owner binding, flat/nested projection,
│   │                         #   the new tasks/update route  ← the phase's center of mass
│   ├── task_store.rs         # + defaulted input-delivery method + supports_inputs() (D-12)
│   ├── tasks.rs              # + defaulted TaskRouter::handle_tasks_update (D-12)
│   ├── core.rs               # own_reserved_result_fields registry extension (Pitfall 1);
│   │                         #   TasksUpdate arm in client_request_mrtr_eligible (F16)
│   ├── mod.rs                # twin-site parity: TasksUpdate in the interception match
│   └── builder.rs            # apply_tasks_capability_rule call site (v2 arm, D-01)
├── types/
│   ├── capabilities.rs       # + ClientCapabilities.extensions (F6) + the typed
│   │                         #   TasksExtensionCapability struct (D-03)
│   ├── tasks.rs              # + TasksUpdateRequest; v2 result projection types
│   ├── protocol/mod.rs       # + ClientRequest::TasksUpdate variant
│   └── mrtr.rs               # logical_name_key: tasks/* → taskId? (F10, see Q4)
├── client/mod.rs             # tasks_update(); era-aware assert_capability (D-04);
│                             #   per-request extensions declaration
crates/pmcp-tasks/src/
├── store/generic.rs          # D-13 site 1: the ONE input-delivery impl (CAS)
├── store/mod.rs              # + defaulted trait method
└── store/memory.rs           # D-13 site 2: the delegating wrapper (F12 — do not forget)
schema/vendored/ext-tasks/    # NEW: pinned schema.ts + schema.json + PROVENANCE.md
examples/
├── s49_v2_tasks_server.rs    # D-05 server half
└── s50_v2_tasks_agent.rs     # D-05 client half — autonomous poll loop
tests/
├── v2_tasks.rs               # era gates, wire shapes, negotiation
├── v2_tasks_security.rs      # D-09 live-socket per-method cross-caller matrix
└── v1_tasks_golden.rs        # D-14 item 2 — byte-identity fixtures (F19: none exist)
```

### Pattern 1: Schema Provenance Comments (Phase 112's `error_codes.rs` discipline)

**What:** Every wire value taken from the unpublished draft carries a source comment naming
the file, the repo and the exact commit SHA, plus a pointer to the re-verification obligation.
**When to use:** Every field name, every enum string, every error code this phase writes.
**Why it matters here:** F1 found the schema in a `draft/` directory, not a versioned one, so
D-18's hold condition is unmet and every value is provisional. Provenance comments are what
make the eventual re-verification a mechanical diff rather than an archaeology exercise.

```rust
// Source: src/types/protocol/error_codes.rs:185-188 (the established in-repo pattern)
/// Header/body mismatch or a missing required v2 header (`-32020`).
/// ...
/// Provenance: `HEADER_MISMATCH = -32020` in `schema/draft/schema.ts` @
/// `71e3069`; see `113-SPEC-RECHECK.md` (verdict `PENDING` + recorded
/// exception).
pub const HEADER_MISMATCH: i32 = -32020;
```

### Pattern 2: Named Era Predicates Over Inline Checks

**What:** Each era gate is a named `const fn` with a truth-table rustdoc, not an inline
`matches!`. Each is proven load-bearing by a recorded removal run, and *orthogonally* —
disabling guard A fails only A's probe.
**When to use:** The negotiation gate, the `tasks/list` gate, the `tasks/result` gate, the
owner-binding gate. Four distinct gates, four named predicates, four independent controls.
**Established by:** `sessions_active_for`, `v1_initialize_gate_applies`, `is_v1_task_era`.

```rust
// Source: src/server/task_dispatch.rs:61-91 (verbatim, the shape to copy)
/// Does this request run under the v1 task lifecycle?
///
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | the v1 task lifecycle is untouched |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | the v2 task surface is not implemented and not negotiated |
pub(crate) const fn is_v1_task_era(era: Option<crate::types::protocol::Era>) -> bool {
    !matches!(era, Some(crate::types::protocol::Era::V2))
}
```

⚠ `is_v1_task_era`'s rustdoc currently states that it "gates ONLY the `-32002` emission" and
that "`tasks/get`, `tasks/list` and `tasks/cancel` are unchanged on every era". **That
rustdoc becomes false the moment this phase lands and must be updated in the same commit
that widens its use** — a stale "deliberately does NOT do X" comment is worse than none.

### Pattern 3: Source Tripwires With Justified Allowlists

**What:** A test that scans compiled `src/` text and fails on (a) a new unlisted site, (b) a
deleted guard, or (c) a stale allowlist entry. Entries carry written justifications with
enforced minimum length and pairwise distinctness.
**When to use:** Locking the v2 status-enum ↔ `TaskStatus` name identity (F15, TASK-04's
"deterministic mapping"); locking that no v2 tasks path can emit `-32603` for task-not-found
(F8); locking that every `tasks/*` route carries an era gate.
**Established by:** `tests/v2_prohibited_error_codes.rs` (`SHOULD_NOT_ALLOWLIST`, 18 tests).

### Pattern 4: Twin-Site Dispatch Parity

**What:** Every per-request wiring change lands at **both** native dispatch sites plus the
wasm mirror. `src/server/mod.rs` CALLS the shared helpers; it never defines its own.
**Measured sites a `ClientRequest::TasksUpdate` variant must reach (F16):**

| Site | What it is | Consequence of missing it |
|------|-----------|--------------------------|
| `src/server/core.rs:1607` `client_request_mrtr_eligible` | Exhaustive no-wildcard match | **COMPILE ERROR** — the designed tripwire. Correct answer is `false`; see Pitfall 4 |
| `src/server/core.rs:3157-3160` | `ServerCore` tasks interception | `tasks/update` falls to `-32601 "Method not supported"` |
| `src/server/core.rs:3322-3325` | Second core.rs enumeration | Silent misrouting |
| `src/server/mod.rs:1632-1637` | `Server::handle_client_request` adapter (a) | HTTP path never reaches TaskDispatch |
| `src/server/task_dispatch.rs:819-836` | `route_tasks_endpoint` match | Falls to the `_ =>` `-32601` arm |

### Anti-Patterns to Avoid

- **Changing `src/types/tasks.rs` serde to produce the v2 shape.** `Task`,
  `CreateTaskResult`, `GetTaskResult` and `CancelTaskResult` are all on the **v1** wire
  today. A serde-level rename (`ttl`→`ttlMs`) or un-nesting changes v1 bytes for every
  existing tasks server and breaks D-02's lock. The projection belongs above the seam.
- **Re-deriving the identity table or the input decoder.** Both exist because Phase 113 paid
  for the lesson (D-113-O cost a 16-round handler loop and a misleading error). An
  independent decoder that guesses at overlapping shapes reproduces that bug class exactly.
- **Making `tasks/update` MRTR-eligible.** See Pitfall 4 — `splice_mrtr_params` strips
  `inputResponses` unconditionally, which would delete the request's entire payload.
- **Asserting "the v1 suite still passes" as byte-identity evidence.** D-14 item 2 exists
  because a reshape is precisely the change that alters bytes while keeping assertions true.
  F19: no golden fixtures exist yet, so this is *creation* work, not *running* work.
- **Advertising `list: true` on v2.** `default_tasks_capability()` sets `list`/`cancel`/
  `requests.tools.call`; projecting it onto v2 would advertise a method that answers `-32601`.
  D-03 already forbids this; the trap is reaching for the existing helper out of convenience.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Typing the client's `inputResponses` entries | An independent decoder that tries each shape | `mrtr::decode_for(kind, value)` (`mrtr.rs:438`) with kinds from the task record | `ElicitResult` and `CreateMessageResult` structurally OVERLAP. Untagged "first that fits" decoding is defect D-113-O: 16 handler invocations, a misleading `-32602`. This is a security control, not convenience |
| Bounding `inputResponses` size/depth/count | Fresh limit constants | The five existing bounds (`mrtr.rs:820-832`): 64 entries, 64 KiB each, 256 KiB total, depth 32 | Inherited free by reusing the types; independently chosen bounds would diverge from the MRTR path on the same server |
| Atomic status transition + input persistence | A read-then-write with a mutex | `StorageBackend::put_if_version` (`backend.rs:217`) | Every backend already implements CAS ("CAS in trait from day one" — PROJECT.md). A read-then-write is a lost-update bug on DynamoDB/Redis |
| Validating the state transition | A new `InputRequired`→`Working` check | `TaskStatus::can_transition_to` (`tasks.rs:56`) | 46 transition tests already pin it (`crates/pmcp-tasks/tests/state_machine.rs`). A second copy is a divergence waiting to happen |
| Owner isolation / not-found semantics | An explicit owner comparison + an `OwnerMismatch` error | The owner-prefixed storage key (`make_key(owner_id, task_id)`) | Owner ID as **structural key** is a PROJECT.md key decision: a wrong owner yields a different key, so `NotFound` is structural rather than a code path that could be forgotten. Never mint an `OwnerMismatch` — it is the information leak |
| Task ID unguessability | A counter, a timestamp, or a hash | `uuid::Uuid::new_v4()` (already used, `task_store.rs:503`) | 122 random bits satisfies the spec's "MUST generate with sufficient entropy that a third party cannot enumerate or guess them" (F20). Already correct — lock it, don't change it |
| The `resultType` discriminator | A new field on the result structs | `ResponseDisposition::Task` + `inject_v2_result_envelope` | Already scaffolded *for this phase* (`core.rs:1139-1151`), with `as_wire_str() == "task"` unit-tested. Adding a public field would be semver churn for zero benefit |
| `Mcp-Name` header value encoding | A custom base64 wrapper | `mrtr::encode_header_value` / `decode_header_value` (`mrtr.rs:280`/`:301`) | One codec shared by the client emitter and the server cross-check; two copies is exactly how the two halves previously disagreed |
| pmcp-tasks feature-matrix evidence (D-14 #4) | A new CI job or script | `make test-feature-flags` (`Makefile:301-336`) | Already runs 4 rows (none / dynamodb / redis / both) × check + clippy `-D warnings` + test + doc. The `cargo check -p pmcp-tasks --features X` rows are dev-dep-free, which is the anti-false-green property D-14 wants (F14) |

**Key insight:** Nearly every primitive this phase needs was built by Phases 101/102/112/113
and is already load-bearing under test. The phase's real work is *wiring and gating*, not
construction — which is exactly why the two highest-risk findings (F5 server-directed
creation, and the `inputRequests` stripping collision) are both about **interaction with
existing code**, not about new code being hard to write.

## Runtime State Inventory

Included because this phase is a wire-API *migration* over a **durable** store: task records
persist in DynamoDB and Redis across the change, so a v1-shaped record must remain readable.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | **Persisted `TaskRecord`s** in DynamoDB / Redis / in-memory, serialized via `GenericTaskStore::serialize_record` with an owner-prefixed key (`make_key(owner_id, task_id)`) and a `version` for CAS. Records written by a pre-114 build carry **no delivered-inputs field** and no `inputRequests`. | **Code edit, no data migration** — the new input-delivery field must deserialize as absent-means-empty on an old record. Add a round-trip test that reads a **byte-fixture** of a pre-114 serialized record (the shape 113-27 used for its `Option<InputRequestKinds>` degrade test), not a hand-built struct |
| Stored data | **Owner-key namespace.** v1 unauthenticated → `"local"`; v2 no-auth-provider → `ANONYMOUS_PRINCIPAL` = `""`. Different key prefixes ⇒ **structurally disjoint**, which is what D-09's disjointness assertion is really asserting. | **None** — verified disjoint by construction (`make_key` prefixes with the owner). D-09's assertion is a lock, not a fix |
| Live service config | **None** — pmcp is a library. No externally-hosted workflow/dashboard config carries a tasks wire shape. Verified: the tasks surface is defined entirely in-repo plus the vendored spec | None |
| OS-registered state | **None** — verified: no OS-level registration (task scheduler, launchd, systemd, pm2) references a task method name or wire field. `grep` for tasks method names outside `src/`, `crates/`, `tests/`, `examples/` and `.planning/` returns nothing operational | None |
| Secrets/env vars | **None new.** The MRTR AEAD `requestState` key (Phase 113) is NOT used by `tasks/update` — the task record replaces the sealed continuation as the kinds source (D-17), so no key material is introduced or renamed | None |
| Build artifacts | **`ClientRequest` enum is `#[serde(tag="method")]`-tagged and public.** Adding `TasksUpdate` is additive for deserialization but a **new variant on a public enum** — check whether `ClientRequest` is `#[non_exhaustive]`; if not, adding a variant is a **semver-major** break and D-12/D-14's "223/223" claim fails for a reason unrelated to the traits | **Verify before planning** — see Q5. This is the one item in this table that could force a design change |

## Common Pitfalls

### Pitfall 1: `own_reserved_result_fields` silently deletes `inputRequests` from a v2 `tasks/get`

**Severity: HIGHEST.** This is the finding most likely to ship a schema-invalid response
while every test passes.

**What goes wrong:** The v2 `tasks/get` response for an `input_required` task is
`Result & InputRequiredTask`, whose `inputRequests` is a **top-level, required** field
(F3, verified in `schema.json`: `InputRequiredTask.required` includes `inputRequests`).
Phase 113's reserved-field registry removes exactly that key:

```rust
// Source: src/server/core.rs:1304, 1322-1336 (current tree, verbatim structure)
let mrtr_owned = disposition == ResponseDisposition::InputRequired;
...
if !mrtr_owned {
    for field in [
        crate::types::mrtr::REQUEST_STATE_KEY,
        crate::types::mrtr::INPUT_REQUESTS_KEY,   // "inputRequests"
    ] {
        if object.remove(field).is_some() {
            tracing::warn!(target: "mcp.v2", field,
                "removed a handler-supplied reserved result field from a result this \
                 egress did not mint");
        }
    }
}
```

A `tasks/get` response carries `disposition == Complete` (the spec requires
`resultType: "complete"` on `GetTaskResult`), so `mrtr_owned` is `false` and the field is
removed. The removal is **silent by design** — a `tracing::warn!`, not an error.

**Why it happens:** The registry's ownership model is "only the egress that MINTED these
fields may emit them", and it derives that from the MRTR disposition because
`InputRequired` was previously the only minter. Phase 114 introduces a **second legitimate
minter** — the tasks dispatch — that the model does not know about.

**How to avoid:** Extend the registry rather than bypassing it. The registry's own rustdoc
(`core.rs:1264-1282`) is "the authoritative reserved-field registry" and must be updated in
the same change. Options, in the phase's own idiom:
1. Pass an explicit "this egress owns the MRTR fields" flag instead of deriving it from the
   disposition — the rustdoc already flags the derivation as a convenience
   (`"mrtr_owned is derived from the disposition rather than passed separately"`).
2. Add a disposition/context signal for a tasks-dispatch-minted result.

Do **not** re-add the field after stripping, and do **not** special-case by method string —
both re-create the divergence the single-registry design prevents.

**Warning signs:** A `tasks/get` integration test that asserts only `status == "input_required"`
passes while the response is schema-invalid. Grep the test suite for
`tracing` warn capture; better, assert the **presence of the `inputRequests` key** in the
response bytes, and add a negative control that removes the registry fix and shows the key
absent (the 113-31 discipline: fire the negative case first).

### Pitfall 2: Deferring server-directed creation makes v2 tasks uncreatable

**What goes wrong:** pmcp's create gate requires `task_requested == true`, sourced from
`CallToolRequest.task: Option<Value>` (`src/types/tools.rs:484`) — v1's client-signals-task
field. The v2 extension has **no such field**. If the create gate keeps requiring it, no v2
`tools/call` can ever produce a `CreateTaskResult`, and TASK-04 cannot be demonstrated.

**Why it happens:** CONTEXT.md's Deferred list frames "server-directed task handles" as an
optional extra the spec marks `MAY`. The `MAY` in the spec is about whether a server
*elects* to materialize a task for a given request — not about whether creation can be
server-initiated. The spec is explicit: *"Task creation is server-directed… The server is
the sole decider; clients do not signal task preference on the request itself."*

**How to avoid:** On v2 the gate's `task_requested` input must be replaced by "the client
declared `io.modelcontextprotocol/tasks` in this request's `_meta.clientCapabilities.extensions`"
— which is also the spec's `MUST NOT return CreateTaskResult to a non-declaring client`.
That requires F6's `ClientCapabilities.extensions` field to exist first. v1 keeps the `task`
field unchanged. **Resolve as Q1 before decomposition** — it changes which tasks exist.

**Warning signs:** A plan that adds `resultType:"task"` emission but never touches the create
gate; a "TASK-04 complete" claim demonstrated only by a hand-built `CreateTaskResult` unit
test rather than a real v2 `tools/call` round trip.

### Pitfall 3: `ANONYMOUS_PRINCIPAL` is rejected by the production backends

**What goes wrong:** D-07's third row resolves the owner to `ANONYMOUS_PRINCIPAL`, which is
the **empty string** (`core.rs:1450`). `GenericTaskStore` rejects it:

```rust
// Source: crates/pmcp-tasks/src/store/generic.rs:129-142 (verbatim)
fn is_anonymous_owner(owner_id: &str) -> bool {
    owner_id.is_empty() || owner_id == DEFAULT_LOCAL_OWNER   // DEFAULT_LOCAL_OWNER == "local"
}
fn check_anonymous_access(&self, owner_id: &str) -> Result<(), TaskError> {
    if !self.security.allow_anonymous && Self::is_anonymous_owner(owner_id) {
        return Err(TaskError::StoreError(
            "anonymous access is not allowed; configure OAuth or enable allow_anonymous".into()));
    }
    Ok(())
}
```

`TaskSecurityConfig::allow_anonymous` defaults to **`false`** (`security.rs:89`). So on any
pmcp-tasks-backed server without explicit configuration, D-07 row 3 cannot create a task.

**Why it happens:** The two identity systems were designed independently. Note this is **not
a new regression** — v1's `"local"` fallback hits the identical check today — but it does
mean D-11's "DynamoDB/Redis-backed tasks work on v2 from day one" holds only for
authenticated callers, and D-05's paired example will fail if it pairs a no-auth server with
a pmcp-tasks backend.

**How to avoid:** Pick one deliberately and document it: (a) D-05's example uses the
in-crate `InMemoryTaskStore` (which has no such check — pmcp's `StoreConfig` has no
`allow_anonymous` field), or (b) the example sets `allow_anonymous: true` explicitly with a
comment naming the shared-bucket caveat D-07 already requires stating. Add a test asserting
the `(None, has_auth_provider=false)` row's behavior **on both backend paths** so the
asymmetry is recorded rather than discovered.

**Warning signs:** An example that works locally against the in-crate store and fails the
first time anyone points it at DynamoDB.

### Pitfall 4: Making `tasks/update` MRTR-eligible deletes its payload

**What goes wrong:** `splice_mrtr_params` "with the DEFAULT removes `inputResponses` and
`requestState` unconditionally" (`core.rs:1648-1649`). `tasks/update`'s entire payload *is*
`inputResponses`. Classifying it MRTR-eligible strips the request body.

**Why it happens:** The two surfaces share the key name and the value type (F4), which makes
"it's the same thing, route it the same way" a natural but wrong inference. They are the same
*data model* on two *different transports*: MRTR carries `inputResponses` on a **resent
original request** bound to an AEAD continuation; tasks carries it on a **dedicated method**
bound to a persisted task record.

**How to avoid:** `client_request_mrtr_eligible` (`core.rs:1607`) is an exhaustive
no-wildcard match, so adding `TasksUpdate` is a **compile error** that forces the decision —
and the correct answer is the `false` arm. Reuse the *types* and `decode_for`; never the MRTR
ingress pipeline. `enum_eligibility_agrees_with_the_method_table` already pins the other
direction (the `MRTR_METHODS` string table), so both halves stay consistent.

**Warning signs:** A `tasks/update` that returns a successful empty ack while the task never
leaves `input_required` — the payload was stripped before the handler saw it.

### Pitfall 5: `-32603` for task-not-found on v2

**What goes wrong:** `route_tasks_get` / `route_tasks_cancel` / `route_tasks_list` map every
`TaskStoreError` to `INTERNAL_ERROR` (`-32603`) — e.g. `task_dispatch.rs:685-688`. The spec
requires `-32602` for an invalid or nonexistent `taskId`: **MUST** for `tasks/get`, **SHOULD**
for `tasks/update` and `tasks/cancel`. Since owner-mismatch surfaces as `NotFound`
(deliberately), D-09's cross-caller probe currently reads `-32603` where a conformant client
expects `-32602`.

**Why it happens:** The v1 path never distinguished them, and `-32603` is the natural
catch-all for a `Result::Err`.

**How to avoid:** On v2, map `TaskStoreError::NotFound` → `-32602`, keeping every other error
on `-32603`. **This is not the frozen `-32002`→`-32602` question the phase must not
re-litigate** — that concerns *resource*-not-found and pmcp's task-pending squat. This is the
tasks extension's own independent allocation for *task*-not-found, and it does not touch
`V1_TASK_PENDING`. Keep v1 byte-identical.

**Warning signs:** A conformance run (Phase 118) flagging `tasks/get` error codes; a D-09
security test that asserts "an error occurred" rather than the specific code.

### Pitfall 6: `tasks/cancel` and `tasks/get` reshape too, and CONTEXT.md does not say so

**What goes wrong:** CONTEXT.md discusses gating `tasks/list` and `tasks/result` off, and
flattening `CreateTaskResult`. It does not mention that on v2:
- `CancelTaskResult = Result` — an **empty acknowledgement**. pmcp returns
  `CancelTaskResult::new(task)`, i.e. `{"task":{...}}` (`tasks.rs:519`).
- `GetTaskResult = Result & DetailedTask` — **flat**. pmcp returns `{"task":{...}}`
  (`tasks.rs:517`).
- `UpdateTaskResult = Result` — empty ack.

So **three** result shapes change on v2, not one.

**How to avoid:** Enumerate all five v2 tasks wire shapes explicitly in the plan
(`CreateTaskResult`, `GetTaskResult`, `UpdateTaskResult`, `CancelTaskResult`, plus the v1-only
`ListTasksResult`/`tasks/result` that are gated off) and assert each against the vendored
schema. Note the spec's semantics that come with the empty acks: cancellation is
*cooperative and eventually consistent* — the task **MAY** remain `working` after the ack and
**MAY** reach a terminal status other than `cancelled`.

### Pitfall 7: `failed` used for `isError: true` tool results

**What goes wrong:** The spec is explicit: *"The `failed` status **MUST NOT** be used to
represent non-JSON-RPC errors, such as a tool result that completed with `isError: true`."*
Those are `completed`, with the error detail inside `result`. `failed` is reserved for
JSON-RPC protocol errors and **MUST** carry the `error` field.

**How to avoid:** The mapping from a handler outcome to a terminal status must branch on
"was this a JSON-RPC error" vs "was this a `CallToolResult` with `isError`", not on "did
something go wrong". Assert both directions with fixtures — the two look identical from a
"the tool failed" mindset and are opposite on the wire.

### Pitfall 8: The stale `is_v1_task_era` rustdoc

**What goes wrong:** Its rustdoc says it "gates ONLY the `-32002` emission" and that
"`tasks/get`, `tasks/list` and `tasks/cancel` are unchanged on every era: the real v2 task
semantics are owned by Phase 114". Both sentences become false in this phase. A stale
"deliberately does NOT do X" comment actively misleads the next reader — 113-29 records
exactly this class of failure (two `-32002` sites "commented v1-scoped, neither ever traced").

**How to avoid:** Update the rustdoc in the same commit that widens the predicate's use, and
add it to the phase's review checklist. Same for the `V2_TASKS_NOT_NEGOTIATED` constant
doc, which currently states as fact that "pmcp advertises no `io.modelcontextprotocol/tasks`
entry" — true today, false after D-01.

## Code Examples

Verified patterns and wire shapes from the authoritative sources.

### 1. Extension capability negotiation — both directions (TASK-01)

```jsonc
// Source: modelcontextprotocol/ext-tasks @ main, specification/draft/tasks.md
//         § Capability Negotiation

// (a) CLIENT → SERVER, in PER-REQUEST capabilities (not a handshake — v2 is stateless)
{
  "params": {
    "_meta": {
      "io.modelcontextprotocol/clientCapabilities": {
        "extensions": { "io.modelcontextprotocol/tasks": {} }
      }
    }
  }
}

// (b) SERVER → CLIENT, in response to server/discover
{
  "result": {
    "capabilities": {
      "extensions": { "io.modelcontextprotocol/tasks": {} }
    }
  }
}
```

Corroborated independently by the **core** spec's own example file
(`schema/draft/examples/ServerCapabilities/extensions-tasks.json` @ main), which is
byte-for-byte `{"extensions":{"io.modelcontextprotocol/tasks":{}}}` — confirming D-03's
empty-object-on-the-wire decision exactly.

The capability type is `Record<string, never>` — an object that admits **no** properties:

```typescript
// Source: ext-tasks @ main, schema/draft/schema.ts (final declaration)
/**
 * The extension capability declaration for the tasks extension.
 * An empty object indicates support; no extension-specific settings are currently defined.
 */
export type TasksExtensionCapability = Record<string, never>;
```

⚠ `Record<string, never>` is stronger than "empty for now" — it declares that no settings
exist. D-03's "typed structure-ready in Rust with `skip_serializing_if` fields" remains a
safe forward-compatible choice, but the planner should not expect fields to arrive.

### 2. The `Task` wire shape and its required fields (TASK-04)

```typescript
// Source: ext-tasks @ main, schema/draft/schema.ts
export interface Task {
  taskId: string;
  status: TaskStatus;
  statusMessage?: string;
  createdAt: string;         // ISO 8601
  lastUpdatedAt: string;     // ISO 8601
  ttlMs: number | null;      // REQUIRED but nullable (null = unlimited)
  pollIntervalMs?: number;
}
export type TaskStatus =
  | "working" | "input_required" | "completed" | "failed" | "cancelled";
```

Required-field lists, read directly from the generated JSON Schema:

| Definition | `required` |
|-----------|-----------|
| `Task` | `taskId`, `status`, `createdAt`, `lastUpdatedAt`, `ttlMs` |
| `CompletedTask` | …the above + **`result`** |
| `InputRequiredTask` | …the above + **`inputRequests`** |
| `FailedTask` | …the above + **`error`** |
| `WorkingTask` / `CancelledTask` | …the above (no extra) |

```typescript
// Source: ext-tasks @ main, schema/draft/schema.ts — the discriminated union
export type DetailedTask =
  | WorkingTask | InputRequiredTask | CompletedTask | FailedTask | CancelledTask;

export type CreateTaskResult = Result & Task;          // FLAT; resultType MUST be "task"
export type GetTaskResult    = Result & DetailedTask;  // FLAT; resultType MUST be "complete"
export type UpdateTaskResult = Result;                 // empty ack; resultType "complete"
export type CancelTaskResult = Result;                 // empty ack; resultType "complete"
```

**Field-name mapping against pmcp's current `Task` (`src/types/tasks.rs:213-257`):**

| pmcp field (v1 wire) | v2 wire | Change |
|---|---|---|
| `task_id` → `taskId` | `taskId` | none |
| `status` | `status` | none — enum strings already identical (F15) |
| `ttl` → `ttl` | **`ttlMs`** | **rename**; both required-and-nullable, so `Option<u64>` without `skip_serializing_if` maps correctly |
| `created_at` → `createdAt` | `createdAt` | none |
| `last_updated_at` → `lastUpdatedAt` | `lastUpdatedAt` | none |
| `poll_interval` → `pollInterval` | **`pollIntervalMs`** | **rename**; optional both sides |
| `status_message` → `statusMessage` | `statusMessage` | none |
| `diagnostic_detail` → `diagnosticDetail` | *(absent)* | pmcp extension — the spec's `Task` has no `deny_unknown_fields`, so it travels harmlessly; the field's own rustdoc already anticipates migrating under a `_meta` slot |
| *(absent)* | `result` / `error` / `inputRequests` | **new, status-conditional and required per variant** |

A real `CreateTaskResult` on the wire — note it carries **all five** required `Task` fields,
which is why TASK-04's four-field enumeration is incomplete (F3):

```json
// Source: ext-tasks @ main, specification/draft/tasks.md § Task Creation
{
  "jsonrpc": "2.0", "id": 1,
  "result": {
    "resultType": "task",
    "taskId": "786512e2-9e0d-44bd-8f29-789f320fe840",
    "status": "working",
    "statusMessage": "The operation is now in progress.",
    "createdAt": "2025-11-25T10:30:00Z",
    "lastUpdatedAt": "2025-11-25T10:40:00Z",
    "ttlMs": 60000,
    "pollIntervalMs": 5000
  }
}
```

### 3. `tasks/update` — the parameter is `inputResponses` (TASK-02)

```typescript
// Source: ext-tasks @ main, schema/draft/schema.ts
export interface UpdateTaskRequest extends JSONRPCRequest {
  method: "tasks/update";
  params: {
    taskId: string;
    /** Responses to outstanding inputRequests previously surfaced by the server.
     *  Each key MUST correspond to a currently-outstanding inputRequest key. */
    inputResponses: InputResponses;
  };
}
export type InputResponse  = CreateMessageResult | ListRootsResult | ElicitResult;
export interface InputResponses { [key: string]: InputResponse; }
```

This is a **name-for-name and shape-for-shape match** with pmcp's existing MRTR type, which
makes D-17's reuse exact rather than analogous:

```rust
// Source: src/types/mrtr.rs:523 (current tree)
pub type InputResponses = BTreeMap<String, InputResponse>;
```

Spec rules that constrain the implementation (all from § Task Update Requests):
- A server **SHOULD ignore** `inputResponses` for a key that is not currently outstanding —
  never issued, already answered, or superseded.
- A server **MAY accept a partial set**; the task then **remains `input_required`** until the
  rest arrive. ⚠ **This qualifies D-16:** the `InputRequired`→`Working` transition is correct
  only when the delivered set *completes* the outstanding set. A partial delivery must persist
  the inputs and stay in `input_required`. D-16's "atomic in one CAS write" still holds — the
  atomic unit is (persist inputs [+ transition iff now complete]).
- Each `inputRequests` key **MUST be unique over the task's lifetime**; a server **MUST NOT**
  reuse a key after its response was delivered.
- The ack is **eventually consistent** — the server MAY ack before `tasks/get` reflects it.
- Servers **SHOULD** return a JSON-RPC error for an unknown `taskId` (`-32602`, per § Error
  Handling — SHOULD for `tasks/update`, MUST for `tasks/get`).

### 4. The identity table to reuse verbatim (TASK-05 / D-07)

```rust
// Source: src/server/core.rs:1570-1585 (current tree, verbatim)
/// Resolve the AAD principal, FAIL-CLOSED.
///
/// * an `AuthContext` is present → its `subject`;
/// * no `AuthContext` but an auth provider IS configured → `None`, i.e. refuse
///   MRTR entirely — a state-bearing continuation must not be mintable or
///   redeemable by an unauthenticated caller on a server that expects
///   authentication (T-113-22);
/// * no auth provider at all → [`ANONYMOUS_PRINCIPAL`].
fn resolve_mrtr_principal(principal: MrtrPrincipal<'_>) -> Option<&str> {
    match (principal.authenticated_subject, principal.has_auth_provider) {
        (Some(subject), _) => Some(subject),
        (None, true) => None,
        (None, false) => Some(ANONYMOUS_PRINCIPAL),
    }
}
```

Contrast with what `TaskDispatch::resolve_owner` does **today** — note the session-id
parameter the router chain still accepts (passed as `None`), and the `"local"` fallback that
D-10 freezes for v1:

```rust
// Source: src/server/task_dispatch.rs:398-419 (current tree, abridged)
pub(crate) fn resolve_owner(&self, auth_context: Option<&AuthContext>) -> Option<String> {
    if let Some(router) = self.task_router {
        return Some(match auth_context {
            Some(ctx) => router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None),
            None => router.resolve_owner(None, None, None),
        });
    }
    if self.task_store.is_some() {
        return Some(match auth_context {
            Some(ctx) => ctx.subject.clone(),
            None => "local".to_string(),
        });
    }
    None
}
```

⚠ The router path delegates to `pmcp_tasks::security::resolve_owner_id`, whose chain is
`subject → client_id → session_id → "local"` (`security.rs:174-198`). TASK-05 forbids a
session-id fallback on v2; the v2 arm must therefore **not** route through
`TaskRouter::resolve_owner` at all, even though the phase otherwise preserves the
store-first/router-fallthrough shape. `client_id` is also disqualified for v2 owner binding —
it is per-application (OAuth `azp`) and would collapse per-user isolation to per-app
isolation, as `resolve_owner`'s own comment already notes.

### 5. Streamable-HTTP routing header (F10)

> When `tasks/get`, `tasks/update`, or `tasks/cancel` is sent over the Streamable HTTP
> transport, the client **MUST** set the `Mcp-Name` header to the value of `params.taskId`.
> This allows transport intermediaries and load balancers to route subsequent requests for
> the same task to the server instance holding its state, which is typically required for
> correctness.
>
> — ext-tasks @ main, `specification/draft/tasks.md` § Streamable HTTP: Routing Headers

**Server side is already safe** (measured): `cross_check_name` returns `Ok(())` immediately
for a non-name-bearing method, and `logical_name_key` covers only `tools/call`, `prompts/get`,
`resources/read` — so a conformant client's `Mcp-Name: <taskId>` is accepted, not rejected.

```rust
// Source: src/server/streamable_http_server.rs:1012-1019 (current tree)
fn cross_check_name(mcp_name: &str, method: &str, body_name: Option<&str>)
    -> std::result::Result<(), &'static str> {
    if !is_name_bearing_method(method) {
        return Ok(());          // ← tasks/* land here today
    }
    ...
}
```

**Client side violates the MUST:** pmcp's emitter derives the header from the same
`logical_name_key` table and emits `Mcp-Name: ""` for a name-less method. Making tasks
methods name-bearing with `name_key = "taskId"` would fix the client *and* turn on the
server-side cross-check (a strictly stronger, still-conformant posture) — but it edits the
shared `MRTR_METHODS` table, which also drives MRTR eligibility. See Q4.

### 6. Era-gated `-32601` for a wrong-era method (TASK-03)

The mechanism is already in the tree and is what `tasks/list` should reuse:

```rust
// Source: src/server/task_dispatch.rs:644-665 (current tree, the tasks/result arm)
match (self.task_store.is_some(), is_v1_task_era(era)) {
    (true, true) => error_response(
        id,
        crate::types::protocol::error_codes::V1_TASK_PENDING,   // FROZEN -32002, v1 only
        "task result not available: task not completed".to_string(),
    ),
    (true, false) => error_response(
        id,
        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        V2_TASKS_NOT_NEGOTIATED.to_string(),
    ),
    (false, _) => error_response(
        id,
        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        "tasks/result not supported".to_string(),
    ),
}
```

⚠ Note the truthfulness problem this creates once D-01 lands: `V2_TASKS_NOT_NEGOTIATED`'s
message says the extension "is not negotiated", which will be **false** on a v2 server that
now advertises it. On such a server `tasks/result` is `-32601` because the method **does not
exist in v2**, not because negotiation failed. Two distinct conditions need two distinct
messages, and both are `-32601`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Tasks in the **core** MCP spec (2025-11-25), negotiated via `capabilities.tasks` / `experimental.tasks` | Tasks are a **separate extension repo** (`modelcontextprotocol/ext-tasks`), negotiated via the `extensions` map; core `schema/draft/schema.ts` @ main contains **zero** task type definitions (3 mentions, all in the `extensions` capability doc/example) | SEP-2663; ext-tasks repo created 2026-04-29, last pushed 2026-07-15 | Confirms TASK-01/03 direction definitively. Wire shapes now come from a second repo that versions independently of the core spec |
| `tasks/list` (paginated task enumeration) | **REMOVED.** The spec names its removal as a security improvement: *"Because there is no `tasks/list`, a server cannot inadvertently leak the existence of one caller's tasks to another. This is an improvement over the `2025-11-25` tasks specification, in which a poorly-scoped list could expose unrelated task IDs."* | SEP-2663 | TASK-03 + TASK-05 are the *same* improvement viewed from two angles |
| Blocking `tasks/result` (poll until terminal, `-32002` while pending) | **REMOVED.** `tasks/get` inlines `result` (or `error`) on the terminal `DetailedTask` variant — one round trip | SEP-2663 | Confirms D-15. Also removes the last v2-reachable `-32002` emission path |
| Client requests a task via a `task` field on `tools/call` | **Server-directed.** The client declares the extension capability per request; the server is *"the sole decider"*. No `task` request field exists | SEP-2663 | See Pitfall 2 / Q1 — the highest-impact scope consequence |
| `CreateTaskResult` / `GetTaskResult` / `CancelTaskResult` nest the task under `task` | **Flat intersections.** `Result & Task`, `Result & DetailedTask`; cancel and update are **empty acks** | SEP-2663 | Three result shapes change, not one (Pitfall 6) |
| `ttl` / `pollInterval` | **`ttlMs` / `pollIntervalMs`** | SEP-2663 | Explicit-unit renames; both confirmed against the JSON Schema |
| `notifications/tasks/status` (pmcp v1 constant, `crates/pmcp-tasks/src/constants.rs`) | **`notifications/tasks`**, delivered on a `subscriptions/listen` stream with `notifications.taskIds`; servers **MAY** push | SEP-2663 + SEP-2133 subscriptions | Adjacent to Phase 113's landed subscriptions work; optional. See Q2 |
| `MISSING_REQUIRED_CLIENT_CAPABILITY = -32003` (as written in the ext-tasks prose) | **`-32021`** in the core draft schema @ main | Post-RC renumbering | The ext-tasks prose is **stale** on this code. Directly affects D-08 (F7 / Q3) |

**Deprecated/outdated in this repo:**
- `is_v1_task_era`'s "gates ONLY the `-32002` emission" rustdoc — false after this phase.
- `V2_TASKS_NOT_NEGOTIATED`'s "pmcp advertises no `io.modelcontextprotocol/tasks` entry" —
  false after D-01.
- `pmcp_tasks::constants::METHOD_TASKS_LIST` / `METHOD_TASKS_RESULT` — v1-only after this
  phase; keep, but their rustdocs should say so.
- `MODEL_IMMEDIATE_RESPONSE_META_KEY` (`crates/pmcp-tasks/src/constants.rs`) — a SEP-1686
  v1 concept with no counterpart in the v2 extension schema.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `ext-tasks` `schema/draft/` is the authoritative source for the v2 tasks wire shape, and the final published extension schema will match it | Summary, Code Examples, State of the Art | MEDIUM-HIGH. It is a `draft/` directory in an **"Experimental"**-labelled repo (its own GitHub description). Every wire value in this research is provisional and must carry a PROVENANCE comment. Mitigated by D-18's hold, which this research **confirms is still correctly engaged** |
| A2 | The core spec repo's `cut-release.yml` `kind=final` `workflow_dispatch` (per 113-28's finding) governs the **core** schema only, and the extension versions independently | Open Questions Q6 | MEDIUM. If the extension needs its own release event, D-18's "a versioned schema directory exists" condition is ambiguous about *which* repo satisfies it. Phase 113's recorded policy predates the discovery that tasks moved to a separate repo |
| A3 | `ClientCapabilities` deserialization silently drops an unknown `extensions` key (no `deny_unknown_fields` anywhere in `src/types/capabilities.rs` — measured 0 occurrences) rather than erroring | F6 | LOW. Verified by grep; serde's default is to ignore unknown fields. If some outer wrapper is strict, the symptom becomes a hard parse error instead of silent loss — which would actually be *easier* to notice |
| A4 | Adding `TasksUpdate` to `ClientRequest` is semver-additive | Runtime State Inventory, Q5 | **HIGH if wrong.** Depends on whether `ClientRequest` is `#[non_exhaustive]` — **not verified in this session**. If it is not, this is a major break and D-12/D-14's "223/223" fails for an unrelated reason. **Verify first (Q5)** |
| A5 | `cargo check -p pmcp-tasks --features X` does not build dev-dependencies, making the existing `make test-feature-flags` rows dev-dep-free | F14, Validation Architecture | LOW-MEDIUM. This is standard cargo behavior (dev-deps build for `--all-targets`/`test`/`bench`), but the *specific* anti-false-green property D-14 wants should be re-measured once rather than assumed, since the Phase-109 false-green happened on this exact crate |
| A6 | The 46 tests in `crates/pmcp-tasks/tests/state_machine.rs` are the "46 transition tests" CONTEXT.md's D-16 cites | Standard Stack, Don't Hand-Roll | LOW. Count matches exactly; the file name matches the claim |
| A7 | `notifications/tasks` support is genuinely optional ("Servers **MAY** push"), so declining is conformant and Phase 118's conformance suite will not fail on its absence | Q2 | MEDIUM. The `MAY` is explicit in the spec text, but a conformance *suite* sometimes tests optional features when advertised. Since pmcp would not advertise `taskIds` in an acknowledgement, exposure should be nil |
| A8 | No golden byte fixtures for v1 `tasks/*` responses exist (grep for `golden`/`GOLDEN` across `tests/` matched only `v2_required_headers.rs`; no `tests/fixtures/` directory) | F19, Validation Architecture | LOW. Grep-verified two ways. Worst case some fixtures exist under a different name, which only *reduces* Wave-0 work |
| A9 | pmcp-tasks is not published to crates.io, so `cargo semver-checks` has no baseline for it | F13 | LOW. The crates.io API returned an error object for `pmcp-tasks`, and it is absent from CLAUDE.md's publish order while being a workspace member |

## Open Questions (RESOLVED)

**All six questions were resolved during planning on 2026-07-27 and each has an owning plan.** They are
retained in full below, unedited apart from the `RESOLVED:` marker on each, because the reasoning that led
to each answer is what a re-verifier at the D-18 gate needs — the answers are recorded, not the questions
erased. Q1 and Q4 additionally received **explicit user approval pre-execution** ("Approve DQ1 + DQ4",
2026-07-27) because they widened scope beyond a literal reading of CONTEXT.md's deferrals.

Resolution index: DQ1→114-12, DQ2→114-10, DQ3→114-09, DQ4→114-06, Q5 (answered by measurement in
`114-PATTERNS.md` Fact 1) → DQ5→114-13, DQ6→114-01, and DQ7 (the contract-YAML question raised by
PATTERNS Fact 3, not by this section) →114-18.

Q1 and Q2 changed **task decomposition**, not just task content, which is why they were resolved before
the planner wrote plans. Q3–Q6 were resolved inside planning.

1. **Is server-directed task creation in scope? (Blocks TASK-04 demonstrability)**
   - **RESOLVED → DQ1, owned by `114-12-PLAN.md`.** IN SCOPE. Research's reading was accepted: the spec's
     `MAY` governs whether a server *elects* to materialize a task, not whether creation may be
     server-initiated. The v2 create trigger is the client's per-request extension declaration; v1 keeps its
     `task` field. **Explicitly approved by the user pre-execution on 2026-07-27**, superseding CONTEXT.md's
     deferral for the create-trigger question. The narrower client-compatibility concern stays deferred.
   - **What we know:** The spec is unambiguous — creation is server-directed, no client `task`
     field exists on v2, and the server is "the sole decider". pmcp's create gate requires
     `task_requested` from v1's `CallToolRequest.task`.
   - **What's unclear:** CONTEXT.md's Deferred list defers "unsolicited / server-directed task
     handles" on the grounds that the spec marks it `MAY`. Research reads that `MAY` as
     *"whether the server elects to materialize a task for a given request"*, not
     *"whether creation may be server-initiated"* — under which reading the deferral, taken
     literally, makes v2 task creation unreachable and TASK-04 undemonstrable end-to-end.
   - **Recommendation:** Treat the v2 create trigger — "the client declared the extension on
     this request" — as **in scope and required**, and re-scope the deferred item to what it
     was plausibly protecting against: a v2 client having to handle a task handle back from
     *any* task-capable tool (a client-compatibility concern). Raise at the first planning
     checkpoint; this is a scope decision, not a research one.

2. **How is the `inputRequests`-stripping collision resolved? (Blocks D-15's v2 `tasks/get`)**
   - **RESOLVED → DQ2, owned by `114-10-PLAN.md`.** The recommended route was taken: an explicit ownership
     input replaces the disposition-derived `mrtr_owned` flag, removing the derivation the registry's own
     rustdoc flags as a convenience. The plan reproduces the silent strip at runtime BEFORE fixing it.
   - **What we know:** `own_reserved_result_fields` removes top-level `inputRequests` whenever
     `disposition != InputRequired`; a v2 `tasks/get` on an `input_required` task must carry it
     as a required top-level field with `resultType: "complete"`.
   - **What's unclear:** Whether to pass MRTR-ownership explicitly (the rustdoc already flags
     the derivation as a convenience) or to extend the disposition model. Both touch Phase
     113's registry, which is documented as authoritative and is guarded by tests.
   - **Recommendation:** Explicit ownership flag — smallest change to the registry's model,
     and it removes a derivation the registry's own docs call out. Requires a negative control
     showing the key **absent** without the fix (Pitfall 1). Needs an owner before any plan
     implements v2 `tasks/get`.

3. **Does D-08 keep `-32003`, given the triple meaning?**
   - **RESOLVED → DQ3, owned by `114-09-PLAN.md`.** Both codes, two meanings: `-32003` keeps the **auth**
     refusal (D-08 verbatim, the 113-23 shape) and the existing `-32021 MISSING_REQUIRED_CLIENT_CAPABILITY`
     carries the **non-declaring-client** refusal with an OBJECT-shaped `requiredCapabilities`. No new wire
     value is minted. The ext-tasks/core `-32003`-vs-`-32021` disagreement is recorded as its own row in
     `114-SPEC-RECHECK.md` for re-check at the gate.
   - **What we know:** pmcp `-32003` = `AUTHENTICATION_REQUIRED`; core draft `-32021` =
     `MISSING_REQUIRED_CLIENT_CAPABILITY`; ext-tasks prose uses `-32003` for
     missing-required-client-capability and requires it (**MUST**) for non-declaring clients
     issuing `tasks/get|update|cancel`.
   - **What's unclear:** On a tasks method, a `-32003` refusal is ambiguous between "you did
     not declare the extension" and "you are not authenticated" — the exact undiscoverability
     D-08 chose `-32003` to avoid. The ext-tasks prose appears stale (pre-renumbering).
   - **Recommendation:** Keep `-32003` for D-08's **auth** refusal (it is what the constant
     means in pmcp and what 113-23 already established for `subscriptions/listen`), and use
     the existing `-32021 MISSING_REQUIRED_CLIENT_CAPABILITY` for the **negotiation** refusal,
     with `error.data.requiredCapabilities` carrying
     `{"extensions":{"io.modelcontextprotocol/tasks":{}}}` — an object, never an array
     (`error_codes.rs:199-202` warns the conformance suite grades this). No new wire value is
     minted either way, so the schema hold is respected. Record the ext-tasks `-32003`
     discrepancy for the re-verification run.

4. **Do tasks methods become name-bearing (`Mcp-Name` = `taskId`)?**
   - **RESOLVED → DQ4, owned by `114-06-PLAN.md`.** Yes on the CLIENT emitter, via a **separate** name-key
     table — because `logical_name_key` and `mrtr_eligible` both derive from `MRTR_METHODS`, so a row there
     would make `tasks/update` MRTR-eligible and `splice_mrtr_params` would delete its entire payload
     (Pitfall 4). Server-side enforcement stays OFF this phase; the server is already tolerant.
     **Explicitly approved by the user pre-execution on 2026-07-27.**
   - **What we know:** The spec says clients **MUST** set it. pmcp's server already accepts it
     (non-name-bearing ⇒ no cross-check); pmcp's client emits `""`, violating the MUST.
   - **What's unclear:** `logical_name_key` derives from `MRTR_METHODS`, which *also* drives
     MRTR eligibility — so adding a row may couple two unrelated properties.
   - **Recommendation:** Fix the client half (it is a spec MUST and Phase 118 grades it).
     Inspect whether `MRTR_METHODS` rows can carry a name_key without implying eligibility; if
     they cannot be decoupled cheaply, add a separate name-key table and pin agreement between
     the two with a test — the same "one table, both ends" discipline, just a second table.

5. **Is `ClientRequest` `#[non_exhaustive]`?** *(Not verified this session — verify first)*
   - **RESOLVED by measurement → `114-PATTERNS.md` Fact 1; design consequence is DQ5, owned by
     `114-13-PLAN.md`.** It is **NOT** `#[non_exhaustive]` (`src/types/protocol/mod.rs:479-483`), so A4 was
     wrong and adding a variant would be semver-MAJOR. `tasks/update` therefore routes via
     `InternalClientRequest` + `classify_internal_method` + `parse_request_or_internal` (transport-agnostic,
     unlike the HTTP-only `HttpIngress` path `subscriptions/listen` uses). F16's 5-site table is the wrong
     table under this design, and the lost `client_request_mrtr_eligible` compile tripwire is replaced by
     three explicit guards.
   - **Why it matters:** If not, adding `TasksUpdate` is a semver-**major** break, and D-12's
     and D-14's "223/223 additive" evidence bar fails for a reason unrelated to the traits.
   - **Recommendation:** `rg '#\[non_exhaustive\]' -B2 src/types/protocol/mod.rs` and a
     `cargo semver-checks` dry run against a scratch variant, as the **first** task of Wave 0.
     If it is exhaustive, the design changes (an untagged catch-all, or routing `tasks/update`
     before typed deserialization as 113-29 showed the raw-body path already does).

6. **Which repo's versioned schema directory satisfies D-18's condition?**
   - **RESOLVED → DQ6, owned by `114-01-PLAN.md`.** BOTH. The recommendation was taken: the hold clears
     only when a versioned schema directory exists in the core spec repo **and** in ext-tasks. Six `[~]`
     requirements must not flip on a core-only publication event. Recorded in `114-SPEC-RECHECK.md`
     § Trigger Condition as a CONDITION, not a date.
   - **What we know (measured this session, 2026-07-28T02:18Z):** core spec `schema/` holds
     `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25`, `draft` — **no `2026-07-28`**.
     ext-tasks `schema/` holds only `draft`. So the condition is unmet in **both** repos and
     the `hold` remains correctly engaged, on the actual date the spec was due.
   - **What's unclear:** 113's policy was written before tasks moved to a separate repo, so
     "a versioned schema directory exists" does not say whose.
   - **Recommendation:** Amend the phase's own hold record to require **both** — the core
     schema for `resultType`/error codes and the ext-tasks schema for the tasks shapes. Six
     `[~]` requirements should not flip on a core-only publication event.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / `rustc` (stable) | All build/test work | ✓ | verify with `rustc --version`; CI uses `dtolnay/rust-toolchain@stable` | none — `rustup update stable` per CLAUDE.md pre-flight |
| `cargo-nextest` | `make test` / `make quality-gate` | ✓ | installed via `make dev-setup` | `cargo test` (weaker; no per-test LEAK detection) |
| `pmat` | CI cognitive-complexity gate (≤25) | ✓ | pinned **3.15.0** in CI | none — CI-only per Phase 75 D-07; run locally only to pre-check |
| `cargo-semver-checks` | D-14 item 3 (223/223) | ✓ | pinned **0.49.0** (Phase 112) | none — the additivity proof |
| `cargo-public-api` | zero-removed-items check | ✓ | pinned **0.52.0** (Phase 112) | none |
| `gh` CLI | Spec re-verification (D-18 arm) | ✓ | 2.64.0 (per 113-28) | `curl` + raw.githubusercontent (used successfully in this research) |
| Network access to `github.com` | Fetching/vendoring the ext-tasks schema | ✓ | — | **Vendoring the schema removes this dependency for all downstream plans** — a reason to vendor early |
| DynamoDB / Redis servers | D-11's "works on v2 from day one" claim | ✗ | — | `cargo check --features dynamodb,redis` proves compilation; **behavioral** verification uses `GenericTaskStore<InMemoryBackend>`, which shares 100% of the domain logic. Per the project's no-Docker-in-tests rule, do **not** add testcontainers |
| Node.js LTS 22.x | Official conformance suite | ✗ (not needed here) | — | CI-only, and it belongs to Phase 118 (CONF-01), not this phase |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:**
- Live DynamoDB/Redis — covered by the in-memory backend sharing the identical
  `GenericTaskStore<B>` domain logic (that is the type's stated purpose), plus the existing
  `make test-feature-flags` compile matrix. This is the established project posture, not a
  compromise.

## Validation Architecture

`workflow.nyquist_validation` is `true` in `.planning/config.json`.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo nextest` (Rust built-in test harness + nextest runner) |
| Config file | none dedicated; driven by `Makefile` targets and root `Cargo.toml` |
| Quick run command | `cargo nextest run --features full -E 'test(/v2_tasks/)'` |
| Full suite command | `make quality-gate` (fmt-check → lint → build → test-all → pmcp-package-gate → audit → unused-deps → check-todos → check-unwraps → validate-always → purity-check → comply) |

⚠ `make lint` has repeatedly caught lints that a bare `cargo clippy -- -D warnings` does not
(pedantic + nursery + cargo groups) — five consecutive plans in Phase 113's gap-closure wave.
Plans must run `make lint`, never a hand-rolled clippy invocation.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TASK-01 | v2 `server/discover` shows `extensions["io.modelcontextprotocol/tasks"] = {}` when a backend is configured | integration | `cargo nextest run --features full -E 'test(v2_tasks_extension_advertised)'` | ❌ Wave 0 |
| TASK-01 | v1 `initialize` stays byte-identical (no `extensions` key) | unit + golden | `cargo nextest run --features full -E 'test(default_serializes_without_extensions_key)'` | ✅ (`src/types/capabilities.rs:788`) — extend, keep green |
| TASK-01 | Client's per-request `_meta.clientCapabilities.extensions` survives the round trip to the server | integration (live socket) | `cargo nextest run --features full -E 'test(client_extension_declaration_reaches_the_server)'` | ❌ Wave 0 — **blocked on F6** |
| TASK-01 | Non-declaring client's `tasks/get` is refused with the missing-capability code + object-shaped `requiredCapabilities` | integration | `cargo nextest run --features full -E 'test(non_declaring_client_is_refused)'` | ❌ Wave 0 |
| TASK-02 | `tasks/update` with `inputResponses` moves a complete outstanding set `input_required → working` | integration | `cargo nextest run --features full -E 'test(tasks_update_completes_the_outstanding_set)'` | ❌ Wave 0 |
| TASK-02 | A **partial** `inputResponses` set persists and the task **stays** `input_required` | integration | `cargo nextest run --features full -E 'test(tasks_update_partial_set_stays_input_required)'` | ❌ Wave 0 — qualifies D-16 |
| TASK-02 | Unknown / already-answered / superseded keys are **ignored**, not errors | unit | `cargo nextest run --features full -E 'test(tasks_update_ignores_unoutstanding_keys)'` | ❌ Wave 0 |
| TASK-02 | Kind-directed decode: an elicitation answer under an elicitation key types as `Elicitation`, and a mismatched shape is refused (the D-113-O class) | unit + integration | `cargo nextest run --features full -E 'test(/tasks_update_kind_directed/)'` | ❌ Wave 0 |
| TASK-02 | The five MRTR DoS bounds fire on `tasks/update` (count, per-entry bytes, total bytes, depth) | property + unit | `cargo nextest run --features full -E 'test(/tasks_update_bounds/)'` | ❌ Wave 0 |
| TASK-02 | Two concurrent `tasks/update`s: first writer wins, second sees a version conflict | integration | `cargo nextest run --features full -E 'test(tasks_update_cas_first_writer_wins)'` | ❌ Wave 0 |
| TASK-03 | `tasks/list` and `tasks/result` answer `-32601` on v2 | integration | `cargo nextest run --features full -E 'test(/v2_tasks_list_and_result_are_gated/)'` | ❌ Wave 0 |
| TASK-03 | Both remain fully functional on v1 (negative control for the gate) | integration | `cargo nextest run --features full -E 'test(/v1_tasks_list_and_result_still_serve/)'` | ❌ Wave 0 |
| TASK-03 | Existing `-32002` v1 lock stays green | integration | `cargo nextest run --features full -E 'test(pending_tasks_result_preserves_minus_32002)'` | ✅ (`tests/v2_prohibited_error_codes.rs`) — must stay untouched |
| TASK-04 | v2 create returns flat `resultType:"task"` with **all five** required `Task` fields | integration | `cargo nextest run --features full -E 'test(v2_create_task_result_is_flat_and_complete)'` | ❌ Wave 0 |
| TASK-04 | v2 `tasks/get` on a **completed** task inlines `result`; on **failed** inlines `error`; on **input_required** inlines `inputRequests` | integration | `cargo nextest run --features full -E 'test(/v2_tasks_get_inlines/)'` | ❌ Wave 0 — the `inputRequests` case is Pitfall 1's regression test |
| TASK-04 | `isError: true` yields `completed` (not `failed`); a JSON-RPC error yields `failed` **with** `error` | unit | `cargo nextest run --features full -E 'test(/terminal_status_discipline/)'` | ❌ Wave 0 (Pitfall 7) |
| TASK-04 | Status-enum name identity is **locked** (5 spec strings ↔ 5 `TaskStatus` serde strings, exhaustive, no wildcard) | unit tripwire | `cargo nextest run --features full -E 'test(task_status_wire_strings_match_the_extension_schema)'` | ❌ Wave 0 — F15 says this is the whole of TASK-04's "deterministic mapping" |
| TASK-05 | Live-socket cross-caller matrix: B gets `NotFound` on A's taskId for **each** of `tasks/get`, `tasks/update`, `tasks/cancel` — never `OwnerMismatch` | integration (live socket) | `cargo nextest run --features full -E 'test(/v2_tasks_cross_caller/)'` | ❌ Wave 0 — D-09, per-method with per-method negative controls |
| TASK-05 | `(None, has_auth_provider=true)` refuses **before** the params parse, echoes the id, HTTP 200 | integration | `cargo nextest run --features full -E 'test(v2_tasks_unauthenticated_is_refused_before_params)'` | ❌ Wave 0 |
| TASK-05 | The refusal sits **after** the `-32601` era/negotiation gates (ordering asserted structurally) | integration | `cargo nextest run --features full -E 'test(the_auth_refusal_follows_the_method_gates)'` | ❌ Wave 0 |
| TASK-05 | v1 `"local"` and v2 `ANONYMOUS_PRINCIPAL` buckets are disjoint | integration | `cargo nextest run --features full -E 'test(v1_local_and_v2_anonymous_are_disjoint)'` | ❌ Wave 0 |
| TASK-05 | Task IDs are `Uuid::new_v4()` (entropy MUST) — locked, not changed | unit tripwire | `cargo nextest run --features full -E 'test(task_ids_are_v4_uuids)'` | ❌ Wave 0 |
| TASK-06 | pmcp-tasks suite green **unmodified** (197 integration tests incl. 46 state-machine) | integration | `cargo nextest run -p pmcp-tasks` | ✅ exists — must pass with a zero-byte diff to those files |
| TASK-06 | v1 `tasks/*` responses byte-identical to golden fixtures | golden | `cargo nextest run --features full -E 'test(/v1_tasks_golden/)'` | ❌ Wave 0 — **F19: no fixtures exist** |
| TASK-06 | `cargo semver-checks` 223/223, zero removed public items | build gate | `cargo semver-checks check-release` + `cargo public-api diff` | ✅ tooling pinned |
| TASK-06 | pmcp-tasks compiles + lints across `{none, dynamodb, redis, both}` | build matrix | `make test-feature-flags` | ✅ exists (`Makefile:301`) — reuse |
| TASK-06 | A pre-114 serialized `TaskRecord` byte-fixture still deserializes (absent-means-empty) | unit | `cargo nextest run -p pmcp-tasks -E 'test(a_pre_114_record_still_deserializes)'` | ❌ Wave 0 |
| ALWAYS | Fuzz target over the `tasks/update` raw-params boundary | fuzz | `cargo fuzz run fuzz_tasks_update` | ❌ Wave 0 (CLAUDE.md) |
| ALWAYS | Runnable paired example (server + agent-shaped client poll loop) | example | `cargo run --example s49_v2_tasks_server` / `s50_v2_tasks_agent` | ❌ Wave 0 (D-05 + CLAUDE.md) |
| ALWAYS | wasm build stays green (task code is `cfg(not(wasm32))`) | build | `make wasm-build` | ✅ exists — baseline must be measured at `HEAD~n`, not assumed |

### Sampling Rate

- **Per task commit:** `cargo nextest run --features full -E 'test(/v2_tasks/)'` plus
  `cargo nextest run -p pmcp-tasks` (the TASK-06 regression surface). Target < 30s.
- **Per wave merge:** `make quality-gate` + `cargo semver-checks check-release` +
  `make test-feature-flags` + `make wasm-build`.
- **Phase gate:** full suite green, semver 223/223, zero new PMAT cog-25 violations, all
  negative controls run and recorded RED-before / RED-under-control, then `/gsd:verify-work`.

### Wave 0 Gaps

- [ ] **`ClientRequest` `#[non_exhaustive]` verification** (Q5) — must be **first**; a
      negative answer changes the design
- [ ] `schema/vendored/ext-tasks/{schema.ts,schema.json,PROVENANCE.md}` — pinned commit +
      SHA256, so every later plan reviews against a fixed artifact offline
- [ ] `tests/v1_tasks_golden.rs` + fixture files — covers TASK-06 (**none exist today**)
- [ ] `tests/v2_tasks.rs` — negotiation, era gates, all four v2 wire shapes
- [ ] `tests/v2_tasks_security.rs` — the D-09 live-socket per-method matrix
- [ ] Shared test fixture module: a two-principal authenticated live-socket harness (extend
      the existing `tests/v2_mrtr.rs` / `tests/v2_subscriptions.rs` helpers rather than
      writing a third)
- [ ] `fuzz/fuzz_targets/fuzz_tasks_update.rs`
- [ ] `examples/s49_v2_tasks_server.rs` + `examples/s50_v2_tasks_agent.rs`
- [ ] Owner + fix for the `own_reserved_result_fields` collision (Q2) — blocks v2 `tasks/get`
- [ ] Framework install: **none needed** — nextest, semver-checks, public-api, pmat all present

## Security Domain

`security_enforcement` is not set to `false` in `.planning/config.json` ⇒ enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | **yes** | Existing OAuth `AuthContext`; `resolve_mrtr_principal`'s fail-closed table (D-07). No new auth mechanism — reuse only |
| V3 Session Management | **no (by design)** | v2 is session-free. TASK-05 explicitly forbids a session-id fallback; that prohibition *is* the control |
| V4 Access Control | **yes — the phase's central control** | Owner ID as **structural** storage key (`make_key(owner_id, task_id)`); `NotFound` never `OwnerMismatch`; owner derived from auth **only**, never client params (IDOR mitigation T-102-01) |
| V5 Input Validation | **yes** | `mrtr::decode_for` kind-directed decoding + the five DoS bounds (64 entries / 64 KiB each / 256 KiB total / depth 32). Reuse, do not re-derive (D-113-O) |
| V6 Cryptography | **partially — and notably NOT extended** | `uuid::Uuid::new_v4()` for unguessable task IDs (122 random bits). The Phase-113 AEAD `requestState` codec is **not** used by `tasks/update` — the persisted task record replaces the sealed continuation as the kinds source, so no key material is introduced. Never hand-roll |
| V7 Error Handling / Logging | **yes** | Refusal messages must not become oracles: task-not-found and owner-mismatch are **indistinguishable** by design. Follow `MrtrParseError`'s discipline — never render a client-chosen key or any value |
| V8 Data Protection | **yes** | TTL-bounded task records; `cleanup_expired` purges result payloads together with the task (`TaskRecord.result` lives on the internal record precisely so it cannot outlive it) |
| V13 API / Web Service | **yes** | The v2 method surface shrinks (`tasks/list`, `tasks/result` gated off) — a deliberate attack-surface reduction the spec itself frames as a security improvement |

### Known Threat Patterns for a dual-version MCP task server

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-caller task read (IDOR via a guessed/leaked `taskId`) | Information Disclosure | Owner-prefixed storage key ⇒ structural `NotFound`; v4-UUID unguessability; **`tasks/list` removed** so enumeration is impossible. D-09's live-socket per-method matrix is the proof |
| Owner-existence oracle (`NotFound` vs `OwnerMismatch`) | Information Disclosure | One indistinguishable `NotFound` for absent / wrong-owner / pending. A `-32602` message must not vary between the three (Pitfall 5's code change must not become an oracle) |
| Unauthenticated task creation on an auth-configured server | Spoofing / Elevation | D-07 row 2: **refuse**. D-08: `-32003` before the params parse, so a refused caller's body is never deserialized |
| Shared-bucket collapse on a no-auth server | Information Disclosure | Accepted and **documented**, not implied (D-07's stated caveat). Bounded in practice by F11: pmcp-tasks' `allow_anonymous: false` default already refuses the shared bucket |
| Mis-typed input injection via overlapping response shapes | Tampering | Kind-directed `decode_for` against server-recorded kinds (D-17). This is defect D-113-O; an independent decoder reproduces it |
| `inputResponses` resource exhaustion | Denial of Service | The five inherited MRTR bounds, asserted to fire **first**, before any decode |
| Replay of an answered `inputRequests` key | Tampering | Spec: keys **MUST** be unique over a task's lifetime and **MUST NOT** be reused after a response is delivered; servers ignore responses for non-outstanding keys |
| Lost-update on concurrent `tasks/update` | Tampering | `put_if_version` CAS — first writer wins, second sees a version conflict (D-16) |
| Unbounded task accumulation per owner | Denial of Service | `max_tasks_per_owner` (default 100, hard reject, no auto-eviction) + TTL + `cleanup_expired` |
| Trust elevation via a task channel | Spoofing | Spec: *"Hosts **MUST** apply the same trust model to these payloads as they would to standard elicitation/sampling requests. A task is not a higher-trust channel."* Client-side obligation; relevant to D-05's client half |
| Prohibited `-32002` on the v2 wire | (Conformance) | Already era-gated by 113-29 with a source tripwire; this phase must not add a new emission site — and D-15's removal of v2 `tasks/result` eliminates the last one |

## Sources

### Primary (HIGH confidence)

- **`modelcontextprotocol/ext-tasks` @ `main`** — the authoritative tasks extension:
  - `schema/draft/schema.ts` (374 lines) — read in full: `Task`, `TaskStatus`, `DetailedTask`
    and its five variants, `CreateTaskResult`, `GetTaskRequest`/`Result`,
    `UpdateTaskRequest`/`Result`, `CancelTaskRequest`/`Result`, `TaskStatusNotification`,
    `TaskSubscriptionNotifications`, `TasksExtensionCapability`
  - `schema/draft/schema.json` (46,903 bytes) — `required` arrays extracted programmatically
    for `Task`, `CompletedTask`, `InputRequiredTask`, `FailedTask`, `UpdateTaskRequest`
  - `specification/draft/tasks.md` (910 lines) — read: Extension Identifier, Capability
    Negotiation, Supported Methods, Polymorphic Results, Task Creation, Task Polling, Task
    Update Requests, Task Cancellation, Task Status Notifications, Streamable HTTP Routing
    Headers, Example Message Flow, Error Handling, Reservations, Security Considerations,
    Implementation Considerations
  - `seps/2663-tasks-extension.md` (53,250 bytes) and `seps/1686-tasks.md` (63,227 bytes) —
    present, enumerated, not read in full
  - Repo metadata: created 2026-04-29, pushed 2026-07-15, description *"Status: Experimental"*
- **`modelcontextprotocol/modelcontextprotocol` @ `main`** — the core spec:
  - `schema/` directory listing — `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25`,
    `draft`. **No `2026-07-28`** (measured 2026-07-28T02:18Z)
  - `schema/draft/schema.ts` (3,184 lines) — 3 task mentions, all in `extensions`; `ResultType
    = "complete" | "input_required" | string`; `HEADER_MISMATCH = -32020`,
    `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`, `UNSUPPORTED_PROTOCOL_VERSION = -32022`
  - `schema/draft/examples/ServerCapabilities/extensions-tasks.json` — the capability example
- **In-repo codebase (all line references measured this session):**
  `src/server/task_dispatch.rs`, `src/server/task_store.rs`, `src/server/tasks.rs`,
  `src/server/core.rs`, `src/server/mod.rs`, `src/server/streamable_http_server.rs`,
  `src/types/tasks.rs`, `src/types/capabilities.rs`, `src/types/mrtr.rs`,
  `src/types/protocol/mod.rs`, `src/types/protocol/error_codes.rs`,
  `src/types/protocol/context.rs`, `src/types/subscriptions.rs`, `src/types/tools.rs`,
  `src/server/cancellation.rs`, `src/client/mod.rs`,
  `crates/pmcp-tasks/src/{constants,security,router}.rs`,
  `crates/pmcp-tasks/src/store/{mod,generic,memory,backend,dynamodb}.rs`,
  `Makefile`, `Cargo.toml`, `.planning/config.json`
- **`.planning/` corpus:** `114-CONTEXT.md`, `REQUIREMENTS.md`, `STATE.md`, `ROADMAP.md`
  (phase table + execution order), `CLAUDE.md`

### Secondary (MEDIUM confidence)

- `crates.io` API for `pmcp-tasks` — returned an error object, i.e. **unpublished** (F13);
  corroborated by its absence from CLAUDE.md's publish order despite workspace membership
- Phase 113 findings quoted via `STATE.md`'s narrative entries (D-113-O's 16-round loop,
  113-23's `-32003`-at-HTTP-200 shape, 113-29's two `-32002` sites, 113-31's
  "the tests that would have failed did not exist"). The underlying `113-*-SUMMARY.md`
  files were **not** read directly this session — CONTEXT.md's canonical-refs list names
  them and the planner should read them before implementing D-08/D-09/D-17
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — read; scope is SEP-2640 Skills +
  the schema-server toolkit lift. One relevant line: *"`ServerCapabilities` must gain an
  `extensions` field"* (satisfied by Phase 112). No Phase-114-specific constraints

### Tertiary (LOW confidence — flagged for validation)

- The inference that ext-tasks' `-32003` for missing-capability is **stale** rather than an
  intentional extension-local allocation differing from core's `-32021` (Q3). Both documents
  are drafts; the renumbering direction is inferred from REQUIREMENTS.md's Out-of-Scope note
  (*"RC error-code allocation renumbered post-RC"*), not from a changelog entry
- The reading of the spec's `MAY` in Q1/Pitfall 2. Strongly supported by *"The server is the
  sole decider; clients do not signal task preference on the request itself"* and by the total
  absence of a request-side `task` field, but it reverses a CONTEXT.md deferral and is
  therefore a **scope decision for the user**, not a research conclusion

## Project Constraints (from CLAUDE.md)

Directives the planner must verify compliance against. These carry the same authority as
CONTEXT.md's locked decisions.

- **Zero tolerance for defects.** Clippy warnings are P0.
- **`make quality-gate` before any commit and before any push/PR.** Not individual cargo
  commands — `make lint` applies pedantic + nursery + cargo groups that bare
  `cargo clippy -- -D warnings` misses (measured: five consecutive Phase-113 plans broke on
  exactly this).
- **Cognitive complexity ≤ 25 per function**, enforced as a **PR-blocking** CI gate
  (`pmat quality-gate --fail-on-violation --checks complexity`, PMAT pinned 3.15.0).
  `task_dispatch.rs` is where this phase's complexity concentrates and it already hosts the
  full era/precedence matrix — plan the refactor techniques (P1–P6 in
  `75-RESEARCH.md`) up front rather than discovering a violation at PR time. ⚠ `STATE.md`
  records the gate currently at **3** violations (D-113-U: `write_canonical` cog 26,
  unowned) — this phase must not add a fourth, and D-113-U needs an owner before the branch
  merges.
- **Zero SATD comments.** No TODO/FIXME/HACK. `make check-todos` enforces it.
- **ALWAYS requirements for every new feature — no exceptions:** fuzz test, property test,
  unit tests (80%+ coverage), **and a runnable `cargo run --example`**. D-05's paired example
  satisfies the last; the fuzz + property items are separately required.
- **Doctests must pass**; all public APIs documented with working examples.
- **`--test-threads=1` in CI** (race-condition prevention) — long live-socket tests must not
  assume parallelism.
- **Contract-first development:** update/add the contract YAML in
  `../provable-contracts/contracts/<crate>/` and run `pmat comply check` before and after
  implementing. `make comply` is part of `quality-gate`.
- **Builder pattern, `async_trait`, `serde(rename_all = "camelCase")`** for protocol types;
  feature flags for optional functionality.
- **Examples numbered** (`s49_`, `s50_` continuing the Phase-113 `s47_`/`s48_` sequence).
- **Design docs in `docs/design/`**; user-facing features documented in **three shapes**
  (README + pmcp-book chapter + pmcp-course chapter) — though DOCS-05 assigns the v2
  migration guide to Phase 119, so this phase owes rustdoc + the examples, not the books.
- **Emergency `--no-verify` override requires justification and immediate follow-up.**

## Metadata

**Confidence breakdown:**

| Area | Level | Reason |
|------|-------|--------|
| v2 wire shapes (fields, enums, methods, results) | **HIGH** | Read from the authoritative `ext-tasks` `schema.ts`, cross-verified against the generated `schema.json` `required` arrays and against worked examples in the 910-line prose spec — three independent artifacts in agreement |
| Tasks removed from the core spec | **HIGH** | Measured: 3 "task" occurrences in a 3,184-line core `schema.ts`, all in the `extensions` capability docs; zero task type definitions |
| Codebase facts (line refs, types, trait shapes, match sites, test counts) | **HIGH** | Every claim read directly from the tree this session; no line reference is from memory |
| The `own_reserved_result_fields` / `inputRequests` collision | **HIGH** | Both halves measured: the removal loop (`core.rs:1322-1336`) and the schema's `InputRequiredTask.required` including `inputRequests`. Not yet reproduced at runtime — a plan should reproduce it before fixing (the 113-26/113-27 discipline) |
| `ANONYMOUS_PRINCIPAL` rejection by pmcp-tasks | **HIGH** | `is_anonymous_owner`, `check_anonymous_access`, `allow_anonymous: false` default and `ANONYMOUS_PRINCIPAL = ""` all read directly |
| Server-directed creation reading (Q1) | **MEDIUM-HIGH** | Spec text is unambiguous and the absence of a request-side `task` field is measured; but it reverses a CONTEXT.md deferral, making it a scope decision rather than a settled finding |
| `-32003` staleness in ext-tasks (Q3) | **MEDIUM** | The two drafts genuinely disagree; which is stale is inferred from REQUIREMENTS.md's renumbering note, not from a changelog |
| D-18 hold status | **HIGH for the measurement, MEDIUM for the policy** | Both repos measured to have no versioned schema directory at 2026-07-28T02:18Z. The policy is ambiguous about *which* repo satisfies the condition (Q6) |
| `ClientRequest` semver additivity (A4) | **LOW — unverified** | Explicitly not checked. Q5 makes it the first Wave-0 task because a negative answer changes the design |
| Pitfalls | **HIGH** | Seven of eight derive from measured code + measured schema; Pitfall 2 carries Q1's scope caveat |

**Research date:** 2026-07-28
**Valid until:** **7 days**, or immediately invalidated by either repo publishing a versioned
schema directory. The extension is labelled *Experimental* and lives in a `draft/` directory
in a repo whose last push was 13 days before this research — treat every wire value as
provisional and re-verify at the D-18 gate. Vendoring the schema at a pinned commit (the
primary recommendation) is what converts this from a decaying finding into a reviewable
artifact.
