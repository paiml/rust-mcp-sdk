# Phase 114: Tasks Extension Migration - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-28
**Phase:** 114-tasks-extension-migration
**Areas discussed:** Extension negotiation surface, v2 owner binding & fail-closed, Which backend serves v2, v2 retrieval + tasks/update

---

## Extension negotiation surface

### Q1 — How does a server come to advertise `extensions: {"io.modelcontextprotocol/tasks": {}}` on v2?

| Option | Description | Selected |
|--------|-------------|----------|
| Auto — inherit the endpoint-backed rule | `apply_tasks_capability_rule` gains a v2 arm; a configured store/router auto-populates the entry, additive-only, explicit value preserved verbatim. One knob (112 D-09 reasoning). | ✓ |
| Explicit builder opt-in | A separate `.with_extension(...)` the author must call. More deliberate, but a v2 server with a working store silently serves nothing until the second knob is found. | |
| Auto, but only when v2 is opted in | Same as auto, computed only for accept-list-opted servers. | |

**User's choice:** Auto — inherit the endpoint-backed rule
**Notes:** Flagged the consequence immediately — `ServerCapabilities.extensions` is a plain struct field with a `default_serializes_without_extensions_key` test, so an unconditional rule would add an `extensions` key to v1 `initialize`. That fed directly into Q2.

### Q2 — How do `extensions[...tasks]`, `capabilities.tasks` and `experimental.tasks` coexist on the wire?

| Option | Description | Selected |
|--------|-------------|----------|
| Era-projected: v2 sees extensions, v1 sees tasks/experimental | Struct carries all; serialization projects per era. v1 `initialize` byte-identical (no `extensions` key); v2 `server/discover` shows extensions and drops v1 keys. Mirrors 112 D-07. | ✓ |
| Additive both eras — everyone sees everything | Simplest, no projection code. Changes v1 bytes for every existing tasks server; advertises a v2 extension to clients that cannot use it. | |
| Era-projected, and v2 drops `experimental` entirely | Cleaner v2 story, but suppresses experimental keys this phase does not own. | |

**User's choice:** Era-projected: v2 sees extensions, v1 sees tasks/experimental

### Q3 — What is the value of the `io.modelcontextprotocol/tasks` extension entry?

| Option | Description | Selected |
|--------|-------------|----------|
| Empty `{}`, but typed structure-ready | Serializes `{}`; Rust type is a struct with `skip_serializing_if` future fields so the final schema fills it without a public-API break. The Phase-112 error-code-table discipline applied to a capability. | ✓ |
| Empty `{}`, `serde_json::Value` typed | Zero new types, zero semver surface, no compile-time shape. | |
| Project the existing `ServerTasksCapability` | One definition for both eras — but would advertise `list: true` where `tasks/list` answers -32601, a capability lie. | |

**User's choice:** Empty `{}`, but typed structure-ready

### Q4 — Does the pmcp `Client` gate v2 `tasks/*` calls on the negotiated extensions map?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — extend the existing era-aware `assert_capability` | On v2 reads the extensions entry; un-negotiated call fails fast with a typed error before the round trip. Reuses 113-05; gives Phase 117 a clean precondition. | ✓ |
| No client-side gate — let the server answer | One source of truth; costs a round trip and yields a wire error instead of a typed one. | |
| Defer the client half to Phase 117 | CLNT-03 already owns the client story; leaves the v2 tasks surface untestable from a real client in this phase. | |

**User's choice:** Yes — extend the existing era-aware `assert_capability`

### Q5 — Owner steer (free text, in place of the more/next check)

**User's response (verbatim intent):** *"Part of the power of the SDK is that we use the same Rust SDK for building the servers and the clients (AI agents), and it should be easy to align them with the shift to v2 spec and tasks and benefit from the improved security and performance of the Rust SDK."*

**Notes:** Recorded as a locked steer, not a question. It settles Q4's answer as first-class rather than optional, and reframes the phase as dual-surface by construction. Reflected back and confirmed before continuing.

### Q6 — What proves "same SDK, both halves, easy to align"?

| Option | Description | Selected |
|--------|-------------|----------|
| Paired runnable example + live round-trip test | The 113-11 shape; satisfies CLAUDE.md's ALWAYS-an-example rule. | |
| Paired example + a one-line-diff migration proof | Adds a test asserting the v1→v2 delta really is one knob per side. | |
| Paired example + agent-shaped client, not just chat-shaped | Client half is an autonomous poll loop (the `pmcp-agent` shape), proving the surface for the ReAct client shape 113 D-07 made first-class and de-risking CLNT-03. Wider scope; some is CLNT-03's charter. | ✓ |

**User's choice:** Paired example + agent-shaped client, not just chat-shaped
**Notes:** Boundary recorded to avoid collision with Phase 117 — this phase's client half is agent-*shaped*; wiring into the real `pmcp-agent` crate stays CLNT-03.

---

## v2 owner binding & fail-closed

Context surfaced before questioning: `TaskDispatch::resolve_owner` (`src/server/task_dispatch.rs:398`) returns the literal `"local"` for every unauthenticated caller on any server — the shared bucket TASK-05 exists to kill. The Phase-113 precedent (`resolve_mrtr_principal`, `src/server/core.rs:1579`) was quoted verbatim so the options were concrete.

### Q1 — What is the v2 owner-binding rule, especially on a server with NO auth provider?

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse the MRTR three-row table verbatim | sub wins; auth-configured + no sub → refuse; no auth provider → `ANONYMOUS_PRINCIPAL`. One identity table for every v2 ingress path. Shared-bucket-on-no-auth caveat must be documented, not implied. | ✓ |
| Hard fail-closed on v2 — no sub, no tasks | Strictest reading; no shared bucket can ever exist on v2. Costs the stdio/dev loop and makes the paired example much harder. | |
| Three-row table + a configurable identity source | Literally TASK-05's "sub OR a stable per-request identity"; covers Lambda `oauth_passthrough` / pmcp.run proxy. Header-derived identity is only as trustworthy as the proxy. | |

**User's choice:** Reuse the MRTR three-row table verbatim
**Notes:** The configurable identity source was moved to Deferred Ideas rather than dropped.

### Q2 — What does the `(None, auth-configured)` row return, and where does it fire?

| Option | Description | Selected |
|--------|-------------|----------|
| `AUTHENTICATION_REQUIRED` (-32003), before the params parse | 113-23's `subscriptions/listen` shape: -32003 at HTTP 200, id echoed, after the -32601 gates and before deserializing params. Existing constant — no new wire value under the schema hold. | ✓ |
| `INVALID_PARAMS` (-32602), matching MRTR's reject | Both table-sharing paths would share the refusal too; slightly off semantically. | |
| `METHOD_NOT_FOUND` (-32601), reusing `V2_TASKS_NOT_NEGOTIATED` | Leaks least, but untruthful, collides with a different condition, and hides the fix. | |

**User's choice:** `AUTHENTICATION_REQUIRED` (-32003), before the params parse

### Q3 — What must the TASK-05 security test cover?

| Option | Description | Selected |
|--------|-------------|----------|
| Live socket, every v2 tasks method, + cross-era isolation | Two authenticated principals; `tasks/get`/`update`/`cancel` each return NotFound (never OwnerMismatch); plus v1-`"local"` vs v2-`ANONYMOUS_PRINCIPAL` disjointness; per-method negative controls. | ✓ |
| Live socket, one representative method + negative control | Cheaper; a future method resolving its owner differently would slip through. | |
| Unit matrix over the identity table only | Fastest; explicitly the shape 113-31 caught as insufficient. | |

**User's choice:** Live socket, every v2 tasks method, + cross-era isolation

### Q4 — Does the v1 owner-binding path change at all?

| Option | Description | Selected |
|--------|-------------|----------|
| Byte-identical, plus a warn pointing at v2 | v1 wire behavior and `"local"` untouched; `tracing::warn!` names the shared bucket and the v2 fix. Zero wire/fixture change; makes the security win a reason to migrate. | ✓ |
| Byte-identical, zero change, no warn | Minimal blast radius, cleanest severability for Phase 117; operator gets no signal. | |
| Tighten v1 too | More secure everywhere; breaks the v1-untouched promise in a minor release. | |

**User's choice:** Byte-identical, plus a warn pointing at v2

---

## Which backend serves v2

Correction surfaced mid-area and stated plainly: the two paths are not peers. `pmcp::server::task_store::TaskStore` has one production impl (`InMemoryTaskStore`); **DynamoDB and Redis live only in `crates/pmcp-tasks/src/store/`** behind that crate's own trait, reaching pmcp solely via `TaskRouterImpl: TaskRouter`. So TASK-06's "DynamoDB/Redis/in-memory backends" refers to the pmcp-tasks side, and serving v2 from the in-crate path alone would make v2 tasks in-memory-only.

### Q1 — Which backend paths serve v2 tasks?

| Option | Description | Selected |
|--------|-------------|----------|
| Both — reshape lives entirely in `task_dispatch`, above the `Value` boundary | Same store-first → router-fallthrough dispatch; `crates/pmcp-tasks` needs no change to serve v2, so DynamoDB/Redis work on v2 from day one. Literal reading of "reshape behind the TaskRouter boundary". | ✓ |
| In-crate `TaskStore` path only; `TaskRouter` stays v1-only | Smallest surface, cleanly severable; v2 tasks become in-memory-only and the "investment survives" claim fails for v2. | |
| Both, but `tasks/update` lands on the store path first | Lower risk on the new method; leaves DynamoDB/Redis unable to use the one feature v2 adds. | |

**User's choice:** Both — reshape lives entirely in `task_dispatch`, above the `Value` boundary

### Q2 — How does `tasks/update` reach the backends without a breaking trait change?

| Option | Description | Selected |
|--------|-------------|----------|
| Defaulted methods + capability probe on both traits | `TaskRouter::handle_tasks_update` defaulted to not-supported (`create_workflow_task` shape); `TaskStore` gains a defaulted input-delivery method + `supports_inputs()` (`set_result`/`supports_results` shape). Purely additive; semver stays green. | ✓ |
| A separate opt-in trait backends implement | Keeps the big traits from growing; costs a second registration knob and a runtime lookup. | |
| No trait change — express update via existing methods | Zero semver surface; constrains what update can mean and pushes typed inputs into a scratchpad channel. | |

**User's choice:** Defaulted methods + capability probe on both traits

### Q3 — Which backends actually implement input delivery in this phase?

| Option | Description | Selected |
|--------|-------------|----------|
| In-crate `InMemoryTaskStore` + pmcp-tasks `GenericTaskStore` | One impl in `GenericTaskStore<B>` covers memory/DynamoDB/Redis with zero divergence, plus the in-crate store so the core path and the example work standalone. Touches `pmcp-tasks` additively. | ✓ |
| In-crate `InMemoryTaskStore` only | Keeps `pmcp-tasks` byte-unchanged; the one new v2 feature would not work on production backends. | |
| Seam only — no backend implements it yet | Smallest phase; TASK-02 could not be demonstrated end to end. | |

**User's choice:** In-crate `InMemoryTaskStore` + pmcp-tasks `GenericTaskStore`
**Notes:** Recorded explicitly in CONTEXT.md that this touches `pmcp-tasks` additively, since TASK-06's text says "unchanged" — so it is not discovered in review.

### Q4 — What is the evidence bar for TASK-06?

| Option | Description | Selected |
|--------|-------------|----------|
| v1 suite green + v1 wire byte-identity + semver + dev-dep-free build | All four, including a dev-dependency-free build across dynamodb/redis so the Phase-109 feature-unification false-green cannot recur on the crate where it already occurred. | ✓ |
| v1 suite green + semver-checks | The two gates already run every phase; leaves wire drift and false-greens uncovered. | |
| v1 suite green only | A passing suite cannot distinguish "unchanged" from "changed where nothing asserts". | |

**User's choice:** v1 suite green + v1 wire byte-identity + semver + dev-dep-free build

---

## v2 retrieval + tasks/update

Grounding surfaced before questioning: today's `Task` has no `result`/`error`/`inputRequests` fields; `CreateTaskResult` is nested `{ task: {...} }` while TASK-04 specifies flat `{taskId,status,ttlMs,pollIntervalMs}` (so `ttl`→`ttlMs`, `pollInterval`→`pollIntervalMs` are renames); and the v1 5-state enum is already name-identical to the v2 status enum.

### Q1 — How does a v2 client retrieve a completed task's result?

| Option | Description | Selected |
|--------|-------------|----------|
| v2 `tasks/get` inlines it; `tasks/result` → -32601 on v2 | Folds `store.get_result` into the Task payload's `result`/`error` when terminal; extends the era gate 113-29 opened; `tasks/list` gets the same via 112 D-10. v1 keeps both methods. | ✓ |
| Keep `tasks/result` alive on v2 as a pmcp extension | Preserves the Phase 101/102 DX unchanged; answers a method the spec removed — the divergence Phase 118 exists to surface. | |
| Inline only when the client asks for it | Real efficiency argument; invents a request parameter the unpublished schema does not define. | |

**User's choice:** v2 `tasks/get` inlines it; `tasks/result` → -32601 on v2

### Q2 — What does `tasks/update` do to the task's state?

| Option | Description | Selected |
|--------|-------------|----------|
| Atomic: `InputRequired` → `Working` in the same CAS write | Reuses the validated 5-state machine and `put_if_version` all backends implement; other source states refused; first writer wins. | ✓ |
| Store-only — inputs persisted, handler transitions | Flexible; leaves a window where a poller cannot tell "delivered" from "ignored". | |
| Configurable per tool | Covers both; adds a third task-related tool knob, cutting against the symmetry steer. | |

**User's choice:** Atomic: `InputRequired` → `Working` in the same CAS write

### Q3 — Does `tasks/update`'s `inputs` map reuse Phase 113's MRTR types and decoding?

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse both — types and kind-directed decoding, kinds from the Task record | One public input model (113 D-10); `decode_for` against server-recorded `inputRequests`; inherits 113-27's D-113-O fix and 113-02's five DoS bounds. | ✓ |
| Reuse the wire types, write task-specific decoding | Honest about different provenance; re-opens D-113-O — any independent decoder guessing at overlapping shapes reproduces that bug. | |
| Separate task-input types entirely | Decouples the lifecycles; two public input models to learn and the security discipline must be re-derived. | |

**User's choice:** Reuse both — types and kind-directed decoding, kinds from the Task record
**Notes:** D-113-O was quoted concretely during the question (untagged decode mis-typed an elicitation answer as Sampling; the handler re-elicited 16 times) so the choice was made against the actual failure, not an abstraction.

### Q4 — How does Phase 114 handle the still-unpublished 2026-07-28 schema?

| Option | Description | Selected |
|--------|-------------|----------|
| Split by schema-dependence up front | TASK-01/03/05/06 close `[x]` on the merits; TASK-02/04 hold `[~]`. Applies 113's HTTP-04 lesson prospectively. | |
| Whole phase holds at `[~]` like 113 | Uniform bookkeeping, exactly consistent with how 113 is recorded. Repeats the failure mode 113 named — a phase that cannot partially close. | ✓ |
| Block until the schema publishes | Zero risk; stalls the milestone on an event with no committed date, and Phase 117 depends on 114. | |

**User's choice:** Whole phase holds at `[~]` like 113
**Notes:** The partial-close tradeoff was stated in the option text before the choice and is recorded alongside D-18 so the planner sizes reviews accordingly. The rejected split is preserved in Deferred Ideas as a revisit trigger. Not re-litigated after the decision.

### Q5 — Are unsolicited / server-directed task handles in scope?

| Option | Description | Selected |
|--------|-------------|----------|
| Out of scope — record as a deferred idea | Spec says MAY; no TASK requirement needs it; the phase already carries six requirements plus a dual-surface example. | ✓ |
| In scope — a v2 server may mint unsolicited | Real Lambda/enterprise win; widens the phase and adds client-compatibility questions. | |
| In scope, opt-in per tool | Middle ground; another tool-declaration knob for a MAY capability. | |

**User's choice:** Out of scope — record as a deferred idea
**Notes:** Raised because the sources disagree — PROJECT.md's v2.5 feature list names "server-directed creation" while none of TASK-01..06 mentions it. Captured in Deferred Ideas with that provenance so it is not lost.

---

## Claude's Discretion

- Naming/placement of the era-projection site for capabilities, and of the flat-vs-nested `CreateTaskResult` projection relative to `inject_v2_result_envelope`.
- How era + `has_auth_provider` get threaded into `TaskDispatch::resolve_owner`, which today takes only `auth_context`.
- Method naming for the new defaulted trait methods and the capability probe.
- Whether TASK-04's "deterministic mapping" is a locking tripwire over already-name-identical enums or a genuine conversion — confirm by measurement.
- Where `result`/`error`/`inputRequests` live on `Task` (additive `skip_serializing_if` fields vs a v2-only projection type), subject to the v1-byte-identity lock.
- TTL/quota limits keying off the new v2 principal — not raised; use existing `TaskSecurityConfig` conventions.

## Deferred Ideas

- Unsolicited / server-directed task handles (spec MAY; named in PROJECT.md's feature list but in no TASK requirement).
- A configurable proxy-header / claim-based identity source for v2 owner binding (permitted by TASK-05's wording; needs its own opt-in + threat-model pass).
- Splitting TASK-02/04 from TASK-01/03/05/06 by schema-dependence — presented and not chosen; revisit trigger recorded.
- Per-tool configurability of the `tasks/update` transition.
- UNAS-01 (SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}`) — still unassigned milestone-wide; deliberately not absorbed here.
