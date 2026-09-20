# Phase 114: Tasks Extension Migration - Context

**Gathered:** 2026-07-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Tasks become a **v2 extension** — a wire-API reshape that lives entirely in
`src/server/task_dispatch.rs`, **above** the proven `serde_json::Value` `TaskRouter`
boundary — while v1 Tasks stay fully functional, all backends survive, and stateless v2
owner-binding fails closed. Covers TASK-01..06.

On v2: Tasks negotiate through the `extensions` capability map
(`io.modelcontextprotocol/tasks`); `tasks/update` is added; `tasks/list` and blocking
`tasks/result` are era-gated off; task-augmented results carry `resultType:"task"` with a
flat `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`; owner binding requires an
OAuth `sub` or the Phase-113 stable per-request identity and refuses when absent. On v1:
byte-identical behavior, `experimental.tasks` / `capabilities.tasks` negotiation unchanged.

**Owner steer (locked):** one Rust SDK serves BOTH servers and clients (AI agents), and
aligning to v2 tasks must be an easy, symmetric change on each side that inherits the
SDK's security and performance properties. **The v2 tasks surface is therefore
dual-surface by construction — the server and client halves land together in this phase**,
not server-first-client-later.

**Explicit non-goals for this phase:**
- **Do NOT re-litigate `-32002`.** The `-32002`→`-32602` rename targets *resource-not-found*,
  not task-pending; pmcp's `V1_TASK_PENDING` squat stays frozen (ROADMAP, Phase 113 plans
  01/12: *"Phase 114 must not re-litigate this."*).
- **Do NOT rewrite `pmcp-tasks`** (REQUIREMENTS.md Out of Scope). The one exception agreed
  here is strictly additive — see D-13.
- Unsolicited / server-directed task handles are OUT (deferred, below).
- Full `pmcp-agent` wiring is CLNT-03 / Phase 117, not this phase (see D-06).

</domain>

<decisions>
## Implementation Decisions

### Extension negotiation surface (TASK-01)

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

### v2 owner binding, fail-closed (TASK-05)

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

### Which backend serves v2 (TASK-06)

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

### v2 retrieval + `tasks/update` (TASK-02, TASK-03, TASK-04)

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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone requirements & roadmap
- `.planning/ROADMAP.md` — v2.5 milestone section; **Phase 114 detail** (goal, TASK-01..06
  mapping, 5 success criteria); the **`-32002` RESOLVED note** stating verbatim that
  *"Phase 114 must not re-litigate this"*; the execution-order note sequencing 114 close
  after 113 for the shared stateless-identity pattern.
- `.planning/REQUIREMENTS.md` — TASK-01..06 full text; the traceability table; **Out of
  Scope** (esp. "Rewriting `pmcp-tasks` for the extension" and "Hard-coding new `-3202x`/
  `-32602` error codes before the final schema").

### Phase 112 spine (direct dependency — READ FIRST)
- `.planning/phases/112-version-plumbing-spine/112-CONTEXT.md` — 112 D-07/D-08
  (`resultType` on v2 only, injected at dispatch; the internal typed enum this phase's
  `"task"` discriminator uses), **112 D-09/D-10** (one-knob auto-enable; `-32601` for
  wrong-era methods — D-10 names `tasks/list` in Phase 114 explicitly), D-01/D-02 (builder
  accept-list opt-in, no feature flag), D-11 (transport-agnostic era).
- `.planning/phases/112-version-plumbing-spine/112-VERIFICATION.md` — proof the spine is live.

### Phase 113 (direct dependency — the identity + input-decoding patterns this phase reuses)
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-CONTEXT.md` —
  D-07 (agent clients are first-class, never assume a human responder), D-10 (one
  handler-facing input type), **D-11** (polling-over-Tasks is pmcp's recommended enterprise
  mechanism — this phase is that substrate).
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md`
  § Third Outcome Policy — the recorded `hold` and its **condition** trigger, which D-18
  inherits verbatim.
- `.planning/phases/113-stateless-http-multi-round-trip-elicitation/deferred-items.md` —
  open/unowned items (D-113-S, D-113-T, D-113-V…) so this phase does not silently adopt or
  duplicate them.
- `113-27-SUMMARY.md` (same directory) — **D-113-O**, the untagged-decoding defect D-17
  exists to avoid reproducing.
- `113-23-SUMMARY.md` (same directory) — the `-32003`-at-HTTP-200 refusal shape D-08 copies.
- `113-31-SUMMARY.md` (same directory) — why unit-only coverage was ruled insufficient (D-09).

### v2.5 research pack (2026-07-22, HIGH confidence)
- `.planning/research/FEATURES.md` — **the Tasks-as-extension row** (wire shapes for
  `Task`/`CreateTaskResult`, status enum, `tasks/update{taskId,inputs}`, `tasks/list`
  REMOVED, blocking `tasks/result` REMOVED, unsolicited handles as MAY, capability key) and
  the anti-feature row "Rewriting `pmcp-tasks` for the extension".
- `.planning/research/SUMMARY.md` — final-spec checkpoint discipline (wire-exact values only
  from the published schema).
- `.planning/research/PITFALLS.md` — the accidental-3.0 pitfall; `cargo semver-checks` /
  `cargo public-api` gate this phase (D-12/D-14 depend on it).
- `.planning/research/STACK.md` — zero-new-runtime-deps constraint.

### Project context & house rules
- `.planning/PROJECT.md` — v2.5 framing; the enterprise-remote-StreamableHTTP focus; the
  **Key Decisions** table rows this phase leans on: *serde_json::Value for TaskRouter*,
  *Owner ID as structural key (NotFound, never OwnerMismatch)*, *KV StorageBackend with
  GenericTaskStore*, *CAS in trait from day one*, *Polling-only for tasks/result*.
- `CLAUDE.md` — ALWAYS requirements (fuzz + property + unit + runnable example), `make
  quality-gate` before commit, the PMAT cognitive-complexity CI gate (≤ 25).
- `docs/design/tasks-feature-design.md` — the original Tasks design doc.

### External spec sources (verify against the final schema before fixing any wire value)
- MCP Tasks Extension site — `https://tasks.extensions.modelcontextprotocol.io/`
- `ext-tasks` reference repo — `https://github.com/modelcontextprotocol/ext-tasks`
- SEP-2663 (tasks) — `https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2663`
- SEP-2133 (extensions framework) —
  `https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2133`

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`ResponseDisposition::Task`** (`src/server/core.rs:1127`, `as_wire_str() == "task"`) —
  already scaffolded by Phase 112 *specifically for this phase*; `inject_v2_result_envelope`
  is the injection point. TASK-04's discriminator is wiring, not new machinery.
- **`resolve_mrtr_principal`** (`src/server/core.rs:1579`) — the exact three-row identity
  table D-07 reuses; `ANONYMOUS_PRINCIPAL` is the existing constant.
- **`is_v1_task_era`** (`src/server/task_dispatch.rs:89`) — the era predicate 113-29 landed.
  Its rustdoc says explicitly it gates **only** the `-32002` emission and that
  "`tasks/get`, `tasks/list` and `tasks/cancel` are unchanged on every era: the real v2 task
  semantics are owned by Phase 114 (requirement TASK-03)." Extending its use is this
  phase's job; the `-32002` behavior it guards is frozen.
- **`V2_TASKS_NOT_NEGOTIATED`** (`src/server/task_dispatch.rs`) — the existing
  method-not-found message for a v2 tasks call on a server that never negotiated the
  extension. Do not reuse it for the auth refusal (D-08).
- **`apply_tasks_capability_rule`** (`src/server/task_dispatch.rs`, called from
  `src/server/builder.rs:1051`) + **`default_tasks_capability()`** — the single shared
  endpoint-backed capability rule D-01 extends. Already additive-only and shared between
  `ServerCoreBuilder` and `ServerBuilder` (HTASK-01), so there is one place to change.
- **`ServerCapabilities.extensions`** (`src/types/capabilities.rs:109`,
  `Option<HashMap<String, serde_json::Value>>`) — the reverse-DNS map Phase 112 added, with
  `default_serializes_without_extensions_key` / `extensions_and_experimental_coexist`
  already pinning its serde behavior. D-02 must keep the first of those true.
- **`TaskDispatch`** (`src/server/task_dispatch.rs:381`) — the one place store-vs-router
  precedence lives ("never as a divergent second copy"); every era gate belongs here.
- **`TaskStore` defaulted-method precedents** — `set_result`/`get_result` +
  `supports_results()` (`src/server/task_store.rs:320/371/388`); and on `TaskRouter`,
  `create_workflow_task` defaulted to an error (`src/server/tasks.rs:88`). D-12 copies both.
- **`GenericTaskStore<B>` + `StorageBackend`** (`crates/pmcp-tasks/src/store/`) — one domain
  impl covering memory/DynamoDB/Redis; D-13's single input-delivery impl lands here.
- **MRTR input types + `decode_for` + `InputRequestKinds`** (`src/types/mrtr.rs`) — D-17's
  reuse target, including the five DoS bounds and the kind-directed decode.
- **Client era-aware `assert_capability`** (landed 113-05) — D-04 extends it rather than
  adding a parallel mechanism.

### Established Patterns
- **Per-request wiring at BOTH dispatch sites** (`src/server/core.rs` **and**
  `src/server/mod.rs`) + wasm mirror parity — the Phase 109/112/113 precedent. Note
  `src/server/mod.rs:1648` already carries an `is_v1_task_era` comment, so both sites are
  in play.
- **Era-gated dispatch arms** (112) — the mechanism for `-32601` on wrong-era methods.
- **Envelope injection at serialization** (`inject_v2_result_envelope`) — where the
  `resultType:"task"` discriminator and the flat `CreateTaskResult` projection belong.
- **Named era predicates over inline checks** — `sessions_active_for` /
  `v1_initialize_gate_applies` / `is_v1_task_era`; each guard proven load-bearing by a
  recorded removal run, and orthogonally (disabling guard A fails only A's probe).
- **Source tripwires with justified allowlists** — e.g.
  `tests/v2_prohibited_error_codes.rs`'s allowlist naming the era guard each
  `V1_TASK_PENDING` site carries; an unlisted site, a deleted guard or a stale entry all
  fail. The same discipline suits the new era gates.
- **`cargo semver-checks` (223/223) + `cargo public-api` zero-removed-items** every phase —
  the milestone's additivity proof; D-12/D-13 are shaped to keep it green.
- **Negative controls are the evidence** — a fix is credited only when a named test is shown
  RED before and RED again under a post-fix control.

### Integration Points
- v2 HTTP ingress (112 era gate) → `stateless()` branch (113) → `TaskDispatch` era arms →
  store-first / router-fallthrough → envelope injection with `resultType:"task"`.
- Capability computation → **era-projected** capability output: v1 `initialize` (unchanged
  bytes) vs v2 `server/discover` (`extensions` entry).
- `tasks/update` ingress → owner binding (D-07/D-08) → kind-directed input decode (D-17) →
  atomic CAS transition (D-16) → backend (`GenericTaskStore` or in-crate store).
- Client: `with_protocol_version(v2)` → `server/discover` → extensions-map capability
  assertion (D-04) → agent poll loop (D-05).

</code_context>

<specifics>
## Specific Ideas

- **"One SDK, both halves" is the phase's organizing principle**, not a nice-to-have. The
  v2 tasks migration must be a small, symmetric change on the server side and the client
  (agent) side, and the phase's example pair is what makes that claim checkable rather than
  asserted.
- **The client half is agent-shaped on purpose.** An autonomous poll loop — not an
  interactive chat client — is the shape that matters for pmcp's enterprise/Lambda focus and
  the one 113 D-07 made first-class.
- **Reuse over re-derivation, specifically for security-bearing code.** Two of this phase's
  decisions (D-07's identity table, D-17's kind-directed decoding) exist because Phase 113
  already paid for the lesson; independently re-deriving either reintroduces a defect that
  was found, reproduced and closed.
- **"Unchanged" claims need byte-level evidence.** D-14's insistence on golden-fixture
  byte-identity over "the suite passes" comes from repeated findings in this milestone that
  a green suite can hide both wire drift and missing coverage.

</specifics>

<deferred>
## Deferred Ideas

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

</deferred>

---

*Phase: 114-Tasks Extension Migration*
*Context gathered: 2026-07-28*
