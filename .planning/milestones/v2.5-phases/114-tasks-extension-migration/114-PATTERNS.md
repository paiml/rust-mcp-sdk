# Phase 114: Tasks Extension Migration - Pattern Map

**Mapped:** 2026-07-28
**Files analyzed:** 24 (15 modified, 9 created)
**Analogs found:** 23 / 24

Every excerpt below was read from the tree this session. Line numbers are measured,
not remembered. Nothing in this phase needs a new architectural shape — 23 of 24
files have an in-repo analog, several of them scaffolded *for this phase* by Phases
112/113.

---

## File Classification

### Modified — core SDK (`src/`)

| File | Role | Data Flow | Closest Analog | Match |
|------|------|-----------|----------------|-------|
| `src/server/task_dispatch.rs` | dispatch/router | request-response | itself — `handle_tasks_result` era arm (`:644-665`) | exact (in-file) |
| `src/server/task_store.rs` | trait + model | CRUD | itself — `set_result`/`get_result`/`supports_results` (`:320-390`, `:637-680`) | exact (in-file) |
| `src/server/tasks.rs` | trait (Value seam) | request-response | itself — `create_workflow_task` defaulted (`:103-112`) | exact (in-file) |
| `src/server/core.rs` | dispatcher + egress envelope | request-response | itself — `own_reserved_result_fields` (`:1299-1337`), `client_request_mrtr_eligible` (`:1607-1633`) | exact (in-file) |
| `src/server/mod.rs` | dispatcher (twin site) | request-response | itself — tasks interception (`:1631-1652`) | exact (in-file) |
| `src/server/builder.rs` | config/builder | build-time | itself — `apply_tasks_capability_rule` (`:1051-1060`, called `:1224`) | exact (in-file) |
| **`src/server/streamable_http_server.rs`** | transport ingress | request-response | itself — `classify_http_ingress` + `assemble_subscriptions_listen` (`:1429`, `:3025`) | exact (in-file) — **see Fact 1** |
| `src/types/capabilities.rs` | model (config types) | config | itself — `ServerCapabilities.extensions` (`:84-109`) + its 4 serde tests (`:788-884`) | exact (in-file) |
| `src/types/tasks.rs` | model (wire types) | CRUD | itself — `Task` (`:210-230`), `CreateTaskResult` (`:481-495`) | exact (in-file) |
| `src/types/protocol/mod.rs` | model (protocol enums) | request-response | `InternalClientRequest` (`:647`) — **not** `ClientRequest` (`:483`) | role-match — **see Fact 1** |
| `src/types/mrtr.rs` | utility (codec + bounds) | transform | itself — `logical_name_key` (`:195`), `MRTR_METHODS` table | exact (in-file) |
| `src/client/mod.rs` | client API | request-response + polling | itself — `assert_capability` (`:2830`), `tasks_get` (`:1223`), `wait_for_task` (`:1323`) | exact (in-file) |

### Modified — `crates/pmcp-tasks/` (additive only, D-13)

| File | Role | Data Flow | Closest Analog | Match |
|------|------|-----------|----------------|-------|
| `crates/pmcp-tasks/src/store/generic.rs` | service (domain logic) | CRUD + CAS | itself — `update_status` (`:240-300`) | exact (in-file) |
| `crates/pmcp-tasks/src/store/mod.rs` | trait | CRUD | itself — `set_result` (`:300-320`) | exact (in-file) |
| `crates/pmcp-tasks/src/store/memory.rs` | delegating adapter | CRUD | itself — the whole `impl TaskStore` (`:348-425`) | exact (in-file) |

### Created

| File | Role | Data Flow | Closest Analog | Match |
|------|------|-----------|----------------|-------|
| `schema/vendored/ext-tasks/{schema.ts,schema.json,PROVENANCE.md}` | vendored artifact | n/a | `src/types/protocol/error_codes.rs:160-213` PROVENANCE block | role-match (idiom, not a file) |
| `examples/s50_v2_tasks_server.rs` | example (server) | request-response | `examples/s47_v2_stateless_mrtr.rs` | exact — **see Fact 2 (renumbered)** |
| `examples/s51_v2_tasks_agent.rs` | example (agent client) | polling loop | `examples/s48_v2_mrtr_client.rs` + `Client::wait_for_task` (`src/client/mod.rs:1323`) | exact |
| `tests/v2_tasks.rs` | integration test | request-response | `tests/v2_subscriptions.rs` + `tests/common/v2.rs` | exact |
| `tests/v2_tasks_security.rs` | integration test (live socket) | request-response | `tests/v2_subscriptions.rs:1288-1413` (the D-113-N pair) | exact |
| `tests/v1_tasks_golden.rs` | golden fixture test | request-response | `tests/v2_required_headers.rs:763-836` (`assert_v1_byte_identical`) | exact |
| `tests/v2_tasks_tripwires.rs` | source tripwire | n/a | `tests/v2_prohibited_error_codes.rs:682-748` (`SiteKind`/`SiteEntry`) | exact |
| `fuzz/fuzz_targets/fuzz_tasks_update.rs` (+ `fuzz/Cargo.toml` `[[bin]]`) | fuzz target | transform | `fuzz/fuzz_targets/fuzz_request_state.rs` (48 lines, whole file) | exact |
| property test for the `inputResponses` bounds | property test | transform | `src/types/mrtr.rs:2523-2561` (in-module `proptest!`) | exact |
| contract YAML in `../provable-contracts/contracts/pmcp/` | contract | n/a | — | **NO ANALOG — directory does not exist** |

---

## Measured Facts That Change Pattern Choice

Three things the planner must not re-derive. Each was measured this session and each
selects a *different* analog than the one RESEARCH.md's file list implies.

### Fact 1 — `ClientRequest` is NOT `#[non_exhaustive]`. Q5 is answered: do not add a variant.

`src/types/protocol/mod.rs:479-483` (verbatim, no `#[non_exhaustive]`):

```rust
/// Client request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum ClientRequest {
```

RESEARCH.md's A4 flagged this as **unverified with HIGH risk if wrong**. It is wrong:
adding `ClientRequest::TasksUpdate` is a semver-**major** break and would fail D-14's
223/223 bar for a reason unrelated to the traits. The repository already documented the
rule and the workaround — `src/server/streamable_http_server.rs:1389-1395`, verbatim:

```rust
/// Classified here rather than added as a public `ClientRequest` variant:
/// Phase 112 established that discipline precisely to keep semver MINOR
/// (`enum_variant_added` on a public exhaustive enum is a MAJOR break), and
/// `cargo semver-checks` catches a regression. The params stay RAW because
/// this classifier must never reject a body — a malformed `params` becomes a
/// structured `-32602` in the served branch, after the header gate and auth
/// have run, not a parse error before them.
```

**Consequence for the plan:** `tasks/update` follows the `subscriptions/listen`
pattern, not the `tasks/get` pattern — an `HttpIngress` arm + an
`InternalClientRequest` variant, routed on the raw body **before** typed
deserialization. RESEARCH.md's F16 "5 match sites a `ClientRequest::TasksUpdate` must
reach" therefore becomes **the wrong table**: the `client_request_mrtr_eligible`
compile tripwire (`core.rs:1607`) will not fire (nothing was added to the enum), so
Pitfall 4's protection is *lost* and must be replaced with an explicit assertion that
`tasks/update` never enters the MRTR ingress. The two files this shifts work into are
`src/server/streamable_http_server.rs` (absent from RESEARCH.md's structure list) and
`src/types/protocol/mod.rs:647` (`InternalClientRequest`).

### Fact 2 — `s49` is already taken twice. The examples are `s50`/`s51`.

`examples/` contains **both** `s49_sampling_host.rs` and `s49_v2_subscriptions_client.rs`
(52 `sNN_` examples total). RESEARCH.md proposes `s49_v2_tasks_server.rs` — a third
collision. Next free pair: **`s50_v2_tasks_server.rs`** + **`s51_v2_tasks_agent.rs`**.

### Fact 3 — `../provable-contracts/` does not exist on this machine.

`ls -d ../provable-contracts` → `No such file or directory`. CLAUDE.md's
contract-first directive names `../provable-contracts/contracts/<crate>/` and
`make comply` is part of `quality-gate`. There is no in-repo analog to copy a contract
YAML from. Plans must either (a) confirm `make comply` no-ops without the sibling repo,
or (b) raise it — do not silently skip a documented gate.

---

## Pattern Assignments

### `src/server/task_dispatch.rs` (dispatch/router, request-response) — the phase's center of mass

**Analog:** itself. Every pattern this file needs already exists inside it.

**Era-gate pattern** (`:644-665`) — the shape TASK-03's `tasks/list` + `tasks/result`
gates copy, and the shape the negotiation gate copies:

```rust
match (self.task_store.is_some(), is_v1_task_era(era)) {
    (true, true) => error_response(
        id,
        // FROZEN wire value -32002 (byte-identical) ...
        crate::types::protocol::error_codes::V1_TASK_PENDING,
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

**Named-era-predicate pattern** (`:61-91`) — copy the truth-table rustdoc verbatim in
shape for each new gate (four gates, four predicates, four independent negative
controls):

```rust
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | the v1 task lifecycle is untouched |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | the v2 task surface is not implemented and not negotiated |
pub(crate) const fn is_v1_task_era(era: Option<crate::types::protocol::Era>) -> bool {
    !matches!(era, Some(crate::types::protocol::Era::V2))
}
```

⚠ This rustdoc's `# What this predicate deliberately does NOT do` block (`:83-88`) says
it "gates ONLY the `-32002` emission" and that "`tasks/get`, `tasks/list` and
`tasks/cancel` are unchanged on every era". **Both sentences become false in this phase
and must be rewritten in the same commit that widens the use** (Pitfall 8). Same for
`V2_TASKS_NOT_NEGOTIATED` (`:49-59`), whose doc asserts as fact that "pmcp advertises
no `io.modelcontextprotocol/tasks` entry" — false after D-01.

**Owner-resolution pattern to REPLACE** (`:398-419`, the site D-07 rewires). Note the
router branch delegates to `TaskRouter::resolve_owner` with a session-id slot, which
TASK-05 forbids on v2:

```rust
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
            None => "local".to_string(),   // D-10 FREEZES this for v1
        });
    }
    None
}
```

**Per-endpoint route pattern** (`:669-709`, `route_tasks_get`) — the store-first /
router-fallthrough / no-backend triple every new route copies, and the site Pitfall 5's
`NotFound → -32602` v2 mapping edits:

```rust
if let Some(store) = self.task_store {
    match store.get(&params.task_id, &owner_id).await {
        Ok(task) => {
            let result = crate::types::tasks::GetTaskResult::new(task);
            success_response(id, serde_json::to_value(result).unwrap_or_default())
        },
        Err(e) => error_response(
            id,
            crate::types::protocol::error_codes::INTERNAL_ERROR,   // ← -32603 today
            e.to_string(),
        ),
    }
} else if let Some(task_router) = self.task_router {
    // ... router fall-through
} else {
    error_response(id, METHOD_NOT_FOUND, "Tasks not enabled".to_string())
}
```

**Capability-rule pattern** (`:136-160`) — D-01's v2 arm goes here, additive-only, same
"explicit value preserved verbatim" discipline:

```rust
if capabilities.tasks.is_none() && has_backend {
    capabilities.tasks = Some(default_tasks_capability());
}
```

**Self-enforcing-gate pattern** (`:557-582`, `maybe_build_task_created`) — the create
gate whose `task_requested` input Q1/Pitfall 2 replaces on v2. Copy the "caller passes
raw facts, the helper enforces the COMPLETE gate internally" contract:

```rust
let gate_open = task_requested
    && self.task_store.is_some()
    && task_support
        .is_some_and(|ts| matches!(ts, TaskSupport::Required | TaskSupport::Optional));
if !gate_open {
    return None;
}
```

**Truth-table unit-test pattern** (`:846-1005`, `mod gate_tests`) — one `#[tokio::test]`
per gate row, each named for the row it proves (`gate_rejects_when_no_backend`,
`gate_accepts_optional_task_shaped`, …), plus a shared assertion helper
(`assert_store_minted`, `:948`). Copy this for the era-gate and owner-binding matrices.

---

### `src/server/streamable_http_server.rs` (transport ingress, request-response)

**Analog:** itself — the `subscriptions/listen` route landed by 113-10/113-23.

**Internally-routed-method pattern** (`:1429-1440`) — how `tasks/update` reaches
dispatch without touching `ClientRequest` (Fact 1):

```rust
fn classify_http_ingress(body: &[u8]) -> Option<HttpIngress> {
    let req: crate::types::JSONRPCRequest<serde_json::Value> = serde_json::from_slice(body).ok()?;
    if req.method == crate::types::subscriptions::SUBSCRIPTIONS_LISTEN_METHOD {
        return Some(HttpIngress::SubscriptionsListen {
            id: req.id,
            params: req.params,
        });
    }
    ...
}
```

**Gate-ordering pattern — this is D-08 verbatim** (`:3037-3092`). The order is the
requirement, not an implementation detail: two `-32601` gates, *then* the auth refusal,
*then* the params parse:

```rust
let era = protocol_context.map(|pc| pc.era);
if !matches!(era, Some(crate::types::protocol::Era::V2)) {
    return listen_rejection_response(era, id, METHOD_NOT_FOUND,
        format!("Method not found: {SUBSCRIPTIONS_LISTEN_METHOD}"));
}
// ... capability gate → also METHOD_NOT_FOUND ...
let Some(principal) = resolve_listen_principal(auth_context, view.has_auth_provider) else {
    return listen_rejection_response(era, id, AUTHENTICATION_REQUIRED,
        format!("{SUBSCRIPTIONS_LISTEN_METHOD} requires an authenticated caller on this server"));
};
let agreed = match resolve_agreed_filter(params, &view) { ... };   // ← params parse LAST
```

Its rustdoc (`:2979-3004`) enumerates the rejection cases in order and explains *why*
each sits where it does — copy that documentation shape for the tasks route.

**Identity-table-at-a-second-ingress pattern** (`:2960-2969`) — the precedent for
threading `has_auth_provider` into a route that only had `auth_context`, which is
exactly CONTEXT.md's Discretion item for `TaskDispatch::resolve_owner`:

```rust
fn resolve_listen_principal(
    auth_context: Option<&crate::server::auth::AuthContext>,
    has_auth_provider: bool,
) -> Option<String> {
    match (auth_context, has_auth_provider) {
        (Some(context), _) => Some(context.subject.clone()),
        (None, true) => None,
        (None, false) => Some(crate::server::subscriptions::anonymous_principal()),
    }
}
```

**HTTP-200-refusal pattern** (`:2783-2806`, `listen_rejection_response`) — how a
`-32003` answers at 200 with a JSON-RPC error body and the original id, without
touching `v2_status_for_code`:

```rust
let response = envelope_for_live_request(
    ResponsePayload::Error(JSONRPCError { code, message, data: None }), id);
let status = v2_dispatch_response_status(era, &response);
let mut http = build_json_response(&TransportMessage::Response(response), "subscriptions/listen gate");
if let Some(status) = status { *http.status_mut() = status; }
```

**Non-name-bearing method check** (`:1012-1019`, `cross_check_name`) — already returns
`Ok(())` for `tasks/*`, so a conformant client's `Mcp-Name: <taskId>` is accepted today
(F10). If Q4 makes tasks name-bearing, this is the site that starts enforcing.

---

### `src/server/core.rs` (dispatcher + egress envelope, request-response)

**Analog:** itself.

**Reserved-field-registry pattern** (`:1299-1337`) — the Pitfall-1 collision site.
Extend the registry; do not bypass it. The `mrtr_owned` derivation is the exact line to
replace with an explicit ownership input:

```rust
let mrtr_owned = disposition == ResponseDisposition::InputRequired;
let wire_result_type = disposition.as_wire_str();
if let Some(object) = result.as_object_mut() {
    ...
    if !mrtr_owned {
        for field in [
            crate::types::mrtr::REQUEST_STATE_KEY,
            crate::types::mrtr::INPUT_REQUESTS_KEY,   // ← v2 tasks/get MUST carry this
        ] {
            if object.remove(field).is_some() {
                tracing::warn!(target: "mcp.v2", field,
                    "removed a handler-supplied reserved result field from a result this \
                     egress did not mint");
            }
        }
    }
}
```

Its rustdoc (`:1261-1298`) is "the authoritative reserved-field registry" and carries a
markdown table of every reserved key + the server's behavior. **That table must gain a
row (or a qualified `inputRequests` row) in the same change.** The rustdoc also
already flags the derivation as a convenience (`:1296-1298`), which is the sentence
Q2's recommended fix removes.

**Era-gated envelope-injection pattern** (`:1201-1226`) — the v2-only, object-results-only
egress the flat `CreateTaskResult` projection and `resultType:"task"` ride on:

```rust
if !matches!(protocol_context.map(|c| c.era), Some(crate::types::protocol::Era::V2)) {
    return;   // v1 byte-identical
}
let ResponsePayload::Result(ref mut value) = response.payload else { return; };
if !value.is_object() { return; }
own_reserved_result_fields(value, server_info, disposition);
```

**Pre-scaffolded discriminator** (`:1126-1163`) — `ResponseDisposition::Task` exists
*for this phase*, with `as_wire_str() == "task"` already unit-tested. Note the
`#[cfg_attr(not(test), allow(dead_code))]` on the `Task` variant (`:1150`): **remove
that allow** when the phase wires it, or the gate stops linting a live variant.

**Compile-time eligibility tripwire** (`:1607-1633`) — the exhaustive no-wildcard match.
Under Fact 1 nothing new lands here, so its protection does **not** extend to
`tasks/update`; the plan needs an explicit substitute (an assertion that the
`tasks/update` ingress never reaches `splice_mrtr_params`).

**Fail-closed identity table** (`:1559-1585`) — D-07's source, to reuse *verbatim*, not
re-derive:

```rust
pub(crate) struct MrtrPrincipal<'a> {
    pub authenticated_subject: Option<&'a str>,
    pub has_auth_provider: bool,
}

fn resolve_mrtr_principal(principal: MrtrPrincipal<'_>) -> Option<&str> {
    match (principal.authenticated_subject, principal.has_auth_provider) {
        (Some(subject), _) => Some(subject),
        (None, true) => None,
        (None, false) => Some(ANONYMOUS_PRINCIPAL),
    }
}
```

`ANONYMOUS_PRINCIPAL` is at `:1450` (`= ""`) and is `#[cfg(all(feature = "streamable-http",
not(target_arch = "wasm32")))]` — the tasks path is `#[cfg(not(wasm32))]` but **not**
`streamable-http`-gated, so a shared use needs a cfg decision.

**Era-projection site** (`:1104-1114`, `discover_result_from_capabilities`) — D-02's
projection belongs here: "the SINGLE place the discover wire shape is assembled … projects
the already-computed `capabilities` (including `extensions`) … read-only".

---

### `src/server/task_store.rs` (trait + model, CRUD)

**Analog:** itself — the `set_result`/`get_result`/`supports_results` triple D-12 copies.

**Additive-defaulted-method pattern** (`:281-330`). Copy the *whole* shape: the
"**additive** trait method with a default implementation, so existing out-of-tree
implementations keep compiling" rustdoc, the explicit-never-silent error, the
owner-scoping MUST, the `# Errors` section, and a runnable doctest:

```rust
/// This is an **additive** trait method with a default implementation, so
/// existing out-of-tree [`TaskStore`] implementations keep compiling. The
/// default returns [`TaskStoreError::Internal`] to signal — explicitly,
/// never silently — that the store does not persist terminal results.
///
/// Implementations MUST scope the write by `owner_id` (mirroring
/// [`TaskStore::get`] / [`TaskStore::cancel`]) so one owner cannot set a
/// result on another owner's task.
async fn set_result(&self, task_id: &str, _owner_id: &str, _result: CallToolResult)
    -> Result<(), TaskStoreError>
{
    let _ = task_id;
    Err(TaskStoreError::Internal {
        message: "store does not support terminal results".to_string(),
    })
}
```

**Capability-probe pattern** (`:381-390`) — the model for `supports_inputs()`:

```rust
/// Whether this store persists terminal results ...
/// Defaults to `false`. The dispatch layer consults this before serving the
/// store-result path, so a store that cannot persist results falls through
/// to the [`TaskRouter`] instead of silently dropping or serving empty results.
fn supports_results(&self) -> bool { false }
```

**Override pattern for `InMemoryTaskStore`** (`:637-680`) — D-13's third site. Note
`Self::validate_access` is what turns owner mismatch into `NotFound`:

```rust
async fn get_result(&self, task_id: &str, owner_id: &str) -> Result<CallToolResult, TaskStoreError> {
    let entry = self.records.get(task_id)
        .ok_or_else(|| TaskStoreError::NotFound { task_id: task_id.to_string() })?;
    Self::validate_access(entry.value(), task_id, owner_id)?;
    entry.value().result.clone()
        .ok_or_else(|| TaskStoreError::NotFound { task_id: task_id.to_string() })
}

fn supports_results(&self) -> bool { true }
```

**Internal-record pattern** (`:397-409`) — where delivered inputs belong (off the wire
`Task`, so they are purged with the task by `cleanup_expired`):

```rust
/// The `result` field holds the terminal [`CallToolResult`] for a completed
/// task. It lives on this INTERNAL record (never on the wire [`Task`], whose
/// shape is locked) so it is purged together with the task by
/// [`InMemoryTaskStore::cleanup_expired`] — no separate unexpiring map.
struct TaskRecord { task: Task, owner_id: String, expires_at: Option<Instant>, result: Option<CallToolResult> }
```

---

### `src/server/tasks.rs` (trait over the `Value` seam, request-response)

**Analog:** itself — `create_workflow_task` (`:103-112`), the exact shape D-12 names for
`handle_tasks_update`:

```rust
/// # Default
///
/// Returns an error indicating workflow tasks are not supported.
async fn create_workflow_task(
    &self,
    _workflow_name: &str,
    _owner_id: &str,
    _progress: Value,
) -> Result<Value> {
    Err(crate::error::Error::internal(
        "workflow tasks not supported by this router",
    ))
}
```

All params and returns are `serde_json::Value` by design (`:20-22`) — the seam D-11 says
the reshape stays *above*. `handle_tasks_update` takes `(params: Value, owner_id: &str)`
to match its five siblings (`:45-60`).

---

### `src/server/mod.rs` + `src/server/builder.rs` (twin-site parity, request-response / build-time)

**Twin-site interception pattern** (`mod.rs:1631-1652`) — the match to widen, with the
comment block that already explains the auth/era threading:

```rust
#[cfg(not(target_arch = "wasm32"))]
if matches!(
    request,
    ClientRequest::TasksGet(_) | ClientRequest::TasksResult(_)
        | ClientRequest::TasksList(_) | ClientRequest::TasksCancel(_)
) {
    return self.task_dispatch()
        .route_tasks_endpoint(id, &request, auth_context.as_ref(),
            protocol_context.as_ref().map(|ctx| ctx.era))
        .await;
}
```

**Shared-rule call sites** (`builder.rs:1051-1060` + `mod.rs:4770`) — both delegate to
the one free fn, "never a re-derived second copy (HTASK-01)", so D-01 has exactly one
place to change:

```rust
let has_backend = self.task_store.is_some() || self.task_router.is_some();
crate::server::task_dispatch::apply_tasks_capability_rule(
    &mut self.capabilities, &self.tool_infos, has_backend,
)
```

Invoked from `builder.rs:1224` inside `build()`.

---

### `src/types/capabilities.rs` (model, config)

**Analog:** itself — `ServerCapabilities.extensions` (`:84-109`), added by Phase 112 with
the exact doc shape D-03's typed capability copies:

```rust
/// Extension capabilities — reverse-domain-keyed protocol extensions.
///
/// This is the wire-correct home for declarations from the Extensions
/// Track of MCP ... Use `experimental` only for pre-SEP, pre-namespaced flags.
#[serde(skip_serializing_if = "Option::is_none")]
pub extensions: Option<HashMap<String, serde_json::Value>>,
```

Both `ClientCapabilities` (`:22-25`) and `ServerCapabilities` (`:48-51`) are
`#[derive(Default)] #[non_exhaustive] #[serde(rename_all = "camelCase")]` with every
field `Option<_> + skip_serializing_if` — so F6's `ClientCapabilities.extensions` is a
**field on a `#[non_exhaustive]` struct**, i.e. semver-additive (unlike Fact 1's enum).

**Serde-lock test pattern** (`:788-884`) — four tests already pin the extensions serde
behavior and D-02 must keep the first one true:
`default_serializes_without_extensions_key`, `extensions_round_trip_byte_equal`,
`extensions_and_experimental_coexist`, `extensions_camelcase_serde`. Copy this
four-test shape for `ClientCapabilities.extensions` and for
`TasksExtensionCapability` serializing as `{}`.

---

### `src/types/tasks.rs` (model, CRUD)

**Analog:** itself. The v1 wire shapes to leave byte-identical (D-02 lock,
anti-pattern: do not serde-rename these):

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub task_id: String,
    pub status: TaskStatus,
    /// Time-to-live in milliseconds. Required but nullable per MCP spec:
    /// `None` serializes as `null` (unlimited TTL), `Some(ms)` as a number.
    pub ttl: Option<u64>,                                   // ← v2 wants `ttlMs`
    pub created_at: String,
    pub last_updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,                         // ← v2 wants `pollIntervalMs`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    ...
}
```

```rust
pub struct CreateTaskResult { pub task: Task }      // :485 — NESTED; v2 is flat
pub struct GetTaskResult    { pub task: Task }      // :517 — NESTED; v2 is flat
pub struct CancelTaskResult { pub task: Task }      // :596 — v2 is an empty ack
```

`Task`'s `diagnostic_detail` rustdoc (`:231-256`) is the model for documenting a
pmcp-extension field that travels harmlessly because `Task` has no
`deny_unknown_fields` — the same argument the v2 projection needs for
`diagnosticDetail`, and the same "future migration under a `_meta` slot" note.

---

### `src/types/mrtr.rs` (utility: kind-directed codec + DoS bounds, transform)

**Analog:** itself — D-17's reuse target. **Do not write a second decoder.**

**Kind-directed decode** (`:432-450`):

```rust
/// This is the CORRECT decoding path: the kind comes from the originating
/// [`InputRequest`], so a `CreateMessageResult`-shaped value presented for an
/// `elicitation/create` entry is REJECTED rather than silently reclassified.
pub fn decode_for(kind: InputRequestKind, value: Value) -> Result<Self, serde_json::Error> {
    match kind {
        InputRequestKind::Elicitation => serde_json::from_value(value).map(|r| Self::Elicitation(Box::new(r))),
        InputRequestKind::Sampling    => serde_json::from_value(value).map(|r| Self::Sampling(Box::new(r))),
        InputRequestKind::Roots       => serde_json::from_value(value).map(|r| Self::Roots(Box::new(r))),
    }
}
```

`try_from_value_untagged` (`:480-496`) is the **anti-pattern**: its own rustdoc records
D-113-O ("silently RECLASSIFIED as Sampling; the handler's `Elicitation` arm never
matched, it re-elicited, and the operation looped with no error raised anywhere"). It
survives for exactly two cases, neither of which is `tasks/update` — the persisted task
record supplies the kinds.

**Types to reuse as-is** (`:514-532`): `InputRequests`/`InputResponses` are
`BTreeMap` "so the wire key order is deterministic for tests and digests, and duplicate
keys are impossible"; `InputRequestKinds` is `pub(crate)` today and would need a
visibility decision if the task record stores it.

**The five bounds, inherited free** (`:816-832`): `MAX_REQUEST_STATE_LEN` 8192,
`MAX_INPUT_RESPONSES` 64, `MAX_INPUT_RESPONSE_BYTES` 65_536,
`MAX_INPUT_RESPONSES_TOTAL_BYTES` 262_144, `MAX_INPUT_RESPONSE_DEPTH` 32. Enforced by
`check_input_response_bounds` (`:1032-1084`). Do not mint new constants.

**Name-key table** (`:195`, `logical_name_key`) with its lock test
`logical_name_key_table` (`:1970`) and `mrtr_eligible_is_exactly_three_methods`
(`:1923`) — the two tests that constrain Q4's "make tasks name-bearing" option.

---

### `src/client/mod.rs` (client API, request-response + polling)

**Analog:** itself.

**Era-aware capability assertion** (`:2812-2858`) — D-04 adds a `"tasks"`-on-v2 arm that
reads the extensions map. The existing arm and the v2 escape hatch:

```rust
fn assert_capability(&self, capability: &str, method: &str) -> Result<()> {
    if self.is_v2() && self.server_capabilities.is_none() {
        return Ok(());          // v2 has no handshake; the SERVER is the authority
    }
    let has_capability = match capability {
        ...
        "tasks" => self.server_capabilities.as_ref().is_some_and(|c| c.tasks.is_some()),
        ...
        _ => { tracing::error!(...); debug_assert!(false, ...); false },
    };
```

The `_ =>` arm's `tracing::error!` + `debug_assert!` is the house pattern for "a new
capability string was wired without updating this match" — reuse it, do not add a
silent `false`.

**Client task-method pattern** (`:1223-1235`, `tasks_get`) — the four-line shape
`tasks_update` copies (`ensure_initialized` → `assert_capability` → typed request →
`parse_task_payload`). Under Fact 1, `tasks_update` cannot build a `ClientRequest`
variant, so it uses `send_untyped_request` instead — the shape at `:798-804`:

```rust
let response = self
    .send_untyped_request(request_id, crate::types::protocol::SERVER_DISCOVER_METHOD,
        serde_json::json!({}))
    .await?;
```

**Agent poll-loop pattern** (`:1323-1386`, `wait_for_task`) — D-05's client half is this
loop plus a `tasks/update` step. Copy: `web_time::Instant` (wasm-safe), the
`poll_decision()` single-source classifier matched exhaustively with no `_` arm, the
`MIN_POLL_MS` floor, and the remaining-budget clamp. Note the current
`InputRequired` arm **errors out** — the agent example is exactly the consumer that
should instead call `tasks/update`:

```rust
TaskPollDecision::InputRequired => {
    return Err(Error::validation(format!(
        "task {task_id} is input_required; wait_for_task cannot provide \
         input — handle the elicitation, then resume polling"
    )));
},
```

**Explicit-discover pattern** (`:775-819`, `server_discover`) — takes `&mut self` and
STORES the capabilities so `assert_capability` then enforces on v2. Its rustdoc warning
("never uses it to CHOOSE an era … do not 'restore' the latter") is a constraint on the
agent example: it must call `server_discover` explicitly.

---

### `crates/pmcp-tasks/src/store/generic.rs` (service, CRUD + CAS) — D-13 site 1

**Analog:** itself — `update_status` (`:240-300`) is D-16's atomic write, step for step:

```rust
let key = make_key(owner_id, task_id);
let versioned = self.backend.get(&key).await.map_err(|e| Self::map_storage_error(e, task_id))?;
let mut record = Self::deserialize_record(&versioned.data)?;
record.version = versioned.version;

// Owner isolation
if record.owner_id != owner_id {
    tracing::warn!(task_id, expected_owner = owner_id, actual_owner = record.owner_id,
        "owner mismatch on task update_status (returning NotFound)");
    return Err(TaskError::NotFound { task_id: task_id.to_string() });
}
if record.is_expired() { return Err(TaskError::Expired { .. }); }

// Validate state machine transition
record.task.status.validate_transition(task_id, &new_status)?;

// Apply transition
record.task.status = new_status;
record.task.last_updated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

// CAS write
let bytes = Self::serialize_record(&record)?;
let new_version = self.backend.put_if_version(&key, &bytes, versioned.version).await
    .map_err(|e| Self::map_storage_error(e, task_id))?;
```

`complete_with_result` (`:464-520`) is the same pattern doing **two** mutations in one
CAS — the closer analog for "persist inputs [+ transition iff the outstanding set is now
complete]" (the `Code Examples §3` qualification of D-16). Its rustdoc line "CAS write
via `put_if_version`, guaranteeing atomicity" is the claim to restate.

**Anonymous-owner check** (`:129-142`) — Pitfall 3, verbatim. `ANONYMOUS_PRINCIPAL`
(`""`) hits `is_anonymous_owner` and `allow_anonymous` defaults to `false`, so D-07 row 3
cannot create a task here without configuration:

```rust
fn is_anonymous_owner(owner_id: &str) -> bool {
    owner_id.is_empty() || owner_id == DEFAULT_LOCAL_OWNER   // "local"
}
fn check_anonymous_access(&self, owner_id: &str) -> Result<(), TaskError> {
    if !self.security.allow_anonymous && Self::is_anonymous_owner(owner_id) {
        return Err(TaskError::StoreError(
            "anonymous access is not allowed; configure OAuth or enable allow_anonymous".into()));
    }
    Ok(())
}
```

⚠ `is_anonymous_owner` treats `""` and `"local"` **identically**. D-09's "v1 `local` and
v2 `ANONYMOUS_PRINCIPAL` are disjoint" claim holds at the `make_key` prefix level (two
different keys) but **not** at this predicate — worth an explicit test so the two
statements are not confused.

**CAS-conflict test harness** (`:913-940`, `CasConflictBackend`) — a backend wrapper whose
`put_if_version` always returns `VersionConflict`. This already exists; reuse it for
D-16's concurrent-update test rather than racing two real writers.

---

### `crates/pmcp-tasks/src/store/mod.rs` + `memory.rs` (trait + delegating adapter, CRUD)

**Trait-method pattern** (`mod.rs:300-320`) — note this crate's trait methods are
**required** (no defaults) with a `# Errors` bullet list naming each `TaskError` variant.
D-12's additive method needs a default here to keep out-of-tree impls compiling:

```rust
/// # Errors
///
/// - [`TaskError::NotFound`] if no task with the given ID exists.
/// - [`TaskError::Expired`] if the task's TTL has elapsed.
/// - [`TaskError::OwnerMismatch`] if the task belongs to a different owner.
/// - [`TaskError::StoreError`] on backend failures.
async fn set_result(&self, task_id: &str, owner_id: &str, result: Value) -> Result<(), TaskError>;
```

**Delegating-wrapper pattern** (`memory.rs:345-425`) — D-13 site 2 (F12: the site most
likely to be forgotten, because omitting it silently inherits the not-supported
default). Every method is one line:

```rust
// ---- TaskStore delegation impl ----
#[async_trait]
impl TaskStore for InMemoryTaskStore {
    async fn set_result(&self, task_id: &str, owner_id: &str, result: Value) -> Result<(), TaskError> {
        self.inner.set_result(task_id, owner_id, result).await
    }
    ...
}
```

A tripwire that every `GenericTaskStore` method has a matching delegation line would
make F12 structural rather than a review item.

---

### `examples/s50_v2_tasks_server.rs` + `examples/s51_v2_tasks_agent.rs`

**Analogs:** `examples/s47_v2_stateless_mrtr.rs` (296 lines) and
`examples/s48_v2_mrtr_client.rs` (236 lines) — the 113-11 pair D-05 names.

**Server-half shape** (`s47`, `:1-88`): a module rustdoc that opens with both `cargo run`
commands, a `# What this demonstrates` bullet list, a deployment-contract section for any
env var involved, then named consts (`TOOL_NAME`, `CITY_KEY`, `DEFAULT_ADDR =
"127.0.0.1:8147"`), then one `ToolHandler` whose `handle` is organized by labelled
rounds (`// ---- Round 2: resume from the VERIFIED continuation. ----`), then
`argv[1]`-or-default binding. For Pitfall 3, the server half must either use the
in-crate `InMemoryTaskStore` or set `allow_anonymous: true` **with the D-07 shared-bucket
caveat spelled out in a comment** — pick one deliberately and say why.

**Agent-half shape** (`s48`): three `demo_*` async fns called from `main`, each printing
a numbered banner and **returning `Err` when the demonstration did not behave as
documented** — the example is an executable assertion, not a printout:

```rust
demo_automatic_fulfilment(&url).await?;
demo_unfulfilled_is_returned(&url).await?;
demo_undeclared_capability(&url).await?;
```

```rust
fn v2_client(url: &Url, handler: Option<ScriptedElicitation>) -> pmcp::Result<Client<StreamableHttpTransport>> {
    let transport = StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(url.clone()).build());
    let builder = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?;
    ...
}
```

`s48`'s closing rustdoc line is the phrase D-05 is asking for: the handler "answers
PROGRAMMATICALLY rather than reading stdin, so the example stays scriptable — and it is
the shape an autonomous agent uses."

---

### `tests/v2_tasks.rs` + `tests/v2_tasks_security.rs`

**Analogs:** `tests/common/v2.rs` (the shared harness — **extend, do not fork**) and
`tests/v2_subscriptions.rs`.

**Two-principal auth harness** (`common/v2.rs:188-210`) — exactly what D-09 needs:

```rust
/// An auth provider mapping `Bearer <name>` onto the subject `<name>`.
///
/// Lets a test choose its principals: two requests with different bearers arrive
/// as two DIFFERENT principals ...
pub struct BearerSubjects;

#[async_trait]
impl pmcp::server::auth::AuthProvider for BearerSubjects {
    async fn validate_request(&self, authorization_header: Option<&str>)
        -> pmcp::Result<Option<pmcp::server::auth::AuthContext>>
    {
        match authorization_header.and_then(|h| h.strip_prefix("Bearer ")) {
            Some(subject) if !subject.is_empty() => Ok(Some(AuthContext::new(subject))),
            _ => Err(pmcp::Error::authentication("missing or invalid token")),
        }
    }
}
```

**The `Ok(None)` provider D-08's precondition actually needs**
(`v2_subscriptions.rs:144-170`, `OptionalBearer`). Read its rustdoc before writing the
test — `BearerSubjects` returns `Err` for a missing token, so the transport answers 401
long before dispatch and the auth-refusal branch is never reached:

```rust
/// D-113-N's precondition is a server that HAS an auth provider AND lets an
/// unauthenticated request through to dispatch. The shared harness's
/// [`BearerSubjects`] cannot produce it ... So this suite constructs the
/// precondition EXPLICITLY rather than hoping the shared fixture happens to
/// have that shape.
struct OptionalBearer;
```

**Live-socket spawn helpers** (`common/v2.rs:216-265`) — `spawn_default_config` is the
default choice and its rustdoc says why `stateless()` would invalidate a per-request era
test. Also available: `v2_body`, `v2_body_with_caps` (`:307`), `v2_headers` (`:380`),
`post`/`post_raw` (`:493`/`:522`), `Resp` (`:409`), `FRAME_TIMEOUT` (`:270`),
`extensions_capabilities` (`:144`) + `DISCOVER_EXTENSION_KEY`.

**The refusal test to copy line-for-line** (`v2_subscriptions.rs:1299-1344`) — four
assertions, each with a reason string that names the decision it protects:

```rust
assert_eq!(stream.status, 200,
    "-32003 is DELIBERATELY unremapped: it is not in v2_status_for_code's 400 arm ...");
let refusal = stream.expect_json().await;
assert_eq!(refusal["error"]["code"], json!(AUTHENTICATION_REQUIRED),
    "the refusal is -32003, the same fail-closed answer the MRTR ingress gives ...: {refusal}");
assert_eq!(refusal["id"], json!(41), "the ORIGINAL request id is echoed: {refusal}");
assert!(refusal["result"].is_null(), "a refusal carries no result: {refusal}");
```

**The paired negative control** (`:1346-1413`) — `unauthenticated_..._still_serves_on_a_
server_with_no_auth_provider`, whose rustdoc states what a future "cleanup" would break.
D-09 needs one of these per method (`tasks/get`, `tasks/update`, `tasks/cancel`), each
proving *its own* guard load-bearing.

Teardown discipline (`:1407-1412`): drop sockets, then `handle.abort()`, then
`handle.await` — otherwise nextest's 100 ms leak timeout fires as noise.

---

### `tests/v1_tasks_golden.rs`

**Analog:** `tests/v2_required_headers.rs:763-836`. F19 is confirmed — this is the only
byte-identity helper in the suite, and there is no `tests/fixtures/` directory:

```rust
/// Parse the raw v1 response text and assert full structural equality against a
/// pinned golden JSON-RPC shape, plus assert the raw string carries no v2 keys.
fn assert_v1_byte_identical(raw: &str, expected_result: &serde_json::Value) {
    let parsed: serde_json::Value = serde_json::from_str(raw).expect("v1 response must be valid JSON");
    let expected = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": expected_result });
    assert_eq!(parsed, expected,
        "v1 wire must be structurally identical to the golden fixture; got raw: {raw}");
    assert!(!raw.contains("resultType"), "v1 raw must not contain resultType: {raw}");
    assert!(!raw.contains("serverInfo"), "v1 raw must not contain serverInfo: {raw}");
    assert!(!raw.contains("_meta"), "v1 raw must not contain _meta: {raw}");
}
```

Call-site shape (`:795-811`): one `#[tokio::test]` per method, golden inline as a
`json!` literal, `spawn(false)` for a NOT-opted-in server. Note the `_meta` assertion
must be relaxed for `tasks/*`: today's `CreateTaskResult` envelope deliberately carries
`_meta.relatedTask` (`task_dispatch.rs:527-532`), so a copy-pasted third assertion would
fail for the wrong reason.

---

### `tests/v2_tasks_tripwires.rs`

**Analog:** `tests/v2_prohibited_error_codes.rs:676-748`. Copy the two-kind entry model
(a DEFINITION site carries no guard; an EMISSION site must name one), the
per-entry `why` with an enforced minimum length (`MIN_JUSTIFICATION_CHARS = 40`, `:111`),
and the anti-vacuity fixture (`TEST_ONLY_MENTION`, `:748`):

```rust
enum SiteKind {
    /// The `pub const` declaration itself. Carries no era guard by definition.
    Definition,
    /// A site that writes the code onto a response, or reads it for comparison.
    /// `guard` is a substring that MUST appear in the same file — the era
    /// predicate that keeps this site off the v2 path.
    Emission { guard: &'static str },
}
struct SiteEntry { path: &'static str, kind: SiteKind, why: &'static str }
```

Three rot-checks the existing suite enforces and this one must too: an unlisted file
fails; a listed file that no longer mentions the token fails; an emission site whose
named guard disappeared fails. The scanner primitives (`strip`, `src_files`,
`line_of`, …) are `:750-1030` and are **deliberately duplicated per test crate** — the
comment at `:676-680` explains that a Rust integration test is its own crate, so restate
the idiom rather than trying to share it.

Three locks this phase needs: the 5 spec status strings ↔ 5 `TaskStatus` serde strings
(F15 — TASK-04's whole "deterministic mapping"); no v2 tasks path emitting `-32603` for
not-found (Pitfall 5); every `tasks/*` route carrying an era gate.

---

### `fuzz/fuzz_targets/fuzz_tasks_update.rs`

**Analog:** `fuzz/fuzz_targets/fuzz_request_state.rs` (48 lines — read it whole). Copy
the four-part module rustdoc (the `cargo fuzz run` command in *plain* form with no
`+nightly`; why this boundary is attacker-controlled; a numbered **Invariants** list; a
**Corpus cases worth seeding** list), the `feature = "fuzzing"`-gated public seam so the
target can reach `pub(crate)` code, and the deterministic-replay note:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use pmcp::server::request_state::fuzz_support::{verify_bytes, VERDICT_OK};

fuzz_target!(|data: &[u8]| {
    let verdict = verify_bytes(data);
    assert_ne!(verdict, VERDICT_OK,
        "verify() accepted a token the fuzzer produced without the key — \
         this is an AEAD authentication break, not a flake");
});
```

Registration (`fuzz/Cargo.toml`, tail) — a `[[bin]]` block per target, optionally
preceded by a comment naming the plan and requirement:

```toml
# Phase 113-13 (HTTP-04): the `subscriptions/listen` CLIENT frame decoder runs on
# bytes supplied by a remote server, so it is fuzzed at that boundary.
[[bin]]
name = "subscription_listen_frames"
path = "fuzz_targets/subscription_listen_frames.rs"
test = false
doc = false
bench = false
```

---

### Property test for the `inputResponses` bounds

**Analog:** `src/types/mrtr.rs:2523-2561` — an in-module `proptest::proptest! { }` block
sitting next to the deterministic unit tests it generalizes, with a custom strategy
whose recursion depth is chosen to stay *inside* the bound under test:

```rust
proptest::proptest! {
    /// Within the cap, the digest is (a) stable under key REORDERING and
    /// (b) different for structurally different values.
    #[test]
    fn distinct_params_within_the_cap_digest_distinctly(
        left in arb_shallow_json(), right in arb_shallow_json(),
    ) {
        ...
        proptest::prop_assert_eq!(...);
    }
}

/// Nested JSON comfortably inside [`MAX_CANONICAL_DEPTH`].
fn arb_shallow_json() -> impl proptest::strategy::Strategy<Value = Value> { ... }
```

Also note the compile-time bound-ordering assertion at `:2487`:
`const _: () = assert!(MAX_INPUT_RESPONSE_DEPTH < MAX_CANONICAL_DEPTH);` — a free lock
for any new bound relationship.

Standalone alternative if a separate file is preferred:
`tests/state_machine_properties.rs` (`prop_compose!` + `proptest!`).

---

### `schema/vendored/ext-tasks/`

**Analog (idiom, not a file):** `src/types/protocol/error_codes.rs:160-213`. There is no
`schema/` directory in the repo today and no prior vendored-artifact precedent, so the
PROVENANCE discipline is what transfers. The block-level provenance header:

```rust
// PROVENANCE: the numeric values and identifiers were read from
// `schema/draft/schema.ts` @ commit 71e306956a4959c9655e5036be215d41986596e6
// (2026-07-16) under the `PENDING` verdict + `## Recorded Exception` in
// `.planning/phases/113-.../113-SPEC-RECHECK.md`,
// because the final `schema/2026-07-28` had not yet published. That record
// obliges a re-verification against the published schema before any Phase-113
// requirement is flipped complete.
```

…and the per-item form (`:185-188`):

```rust
/// Provenance: `HEADER_MISMATCH = -32020` in `schema/draft/schema.ts` @
/// `71e3069`; see `113-SPEC-RECHECK.md` (verdict `PENDING` + recorded
/// exception).
```

`MISSING_REQUIRED_CLIENT_CAPABILITY = -32021` already exists (`:213`) with a rustdoc
that resolves Q3 in the repo's own voice — including the sentence D-08 needs
("The accompanying `error.data.requiredCapabilities` payload is a `ClientCapabilities`
**OBJECT** … never an array. Emitting an array here is a wire-contract violation that
the official conformance suite grades.") and an explicit
"This is a **DIFFERENT** constant from `UNSUPPORTED_CAPABILITY` (-32002) … They are not
interchangeable and must not be reconciled."

Add `PROVENANCE.md` with: source repo, commit SHA, fetch date, SHA256 of each vendored
file, the re-verification obligation, and a pointer to the phase's hold record.

---

## Shared Patterns

### Fail-closed identity (apply to every v2 tasks ingress)
**Source:** `src/server/core.rs:1559-1585` (`MrtrPrincipal` + `resolve_mrtr_principal`);
second-ingress precedent at `src/server/streamable_http_server.rs:2960-2969`.
**Apply to:** `TaskDispatch::resolve_owner`, the `tasks/update` route, the create gate.
One table per server; never re-derive; never add a session-id or `client_id` row.

### Auth refusal at HTTP 200, after the method gates, before the params parse
**Source:** `src/server/streamable_http_server.rs:3037-3092` + `:2783-2806` +
the ordered rejection-case rustdoc at `:2979-3004`.
**Apply to:** every v2 `tasks/*` method. `AUTHENTICATION_REQUIRED` is deliberately absent
from `v2_status_for_code`'s 400 arm — do not "fix" that mapping.

### Era-projected serialization (v1 bytes frozen, v2 projected)
**Source:** `src/server/core.rs:1201-1226` (`inject_v2_result_envelope`) +
`:1104-1114` (`discover_result_from_capabilities`).
**Apply to:** the capability projection (D-02) and the flat/nested result projection
(TASK-04). Anti-pattern: doing either as a serde change in `src/types/tasks.rs` or
`src/types/capabilities.rs`.

### Additive trait extension (defaulted method + capability probe)
**Source:** `src/server/task_store.rs:281-390`; `src/server/tasks.rs:103-112`.
**Apply to:** `TaskStore` input delivery + `supports_inputs()`,
`TaskRouter::handle_tasks_update`, and the `pmcp-tasks` `TaskStore` mirror. Keeps
`cargo semver-checks` at 223/223 (contrast Fact 1, where an enum variant would not).

### One shared rule, two dispatch sites
**Source:** `src/server/builder.rs:1051-1060` and `src/server/mod.rs:4770` both calling
`task_dispatch::apply_tasks_capability_rule`; `src/server/mod.rs:1631-1652` calling
`route_tasks_endpoint`.
**Apply to:** every new gate. `task_dispatch.rs`'s module doc (`:376-380`) is the rule:
"the task-lifecycle logic lives HERE, once, never as a divergent second copy."

### Negative-control-per-guard
**Source:** `tests/v2_subscriptions.rs:1299-1344` (the refusal) paired with `:1346-1413`
(the deliberate divergence that must keep working).
**Apply to:** all four new era gates and all three owner-binding methods. Disabling guard
A must fail only A's probe.

### Source tripwire with a justified, self-rotting allowlist
**Source:** `tests/v2_prohibited_error_codes.rs:682-748`.
**Apply to:** the status-enum name-identity lock, the `-32603`-for-not-found ban, the
"every `tasks/*` route has an era gate" check.

### Wire values carry provenance
**Source:** `src/types/protocol/error_codes.rs:160-213`.
**Apply to:** every field name, enum string and error code this phase writes, plus the
vendored schema's `PROVENANCE.md`.

### `pmcp-tasks` feature matrix (D-14 item 4 — already built)
**Source:** `Makefile:301-336`, `make test-feature-flags`. Four rows
(`--no-default-features`, `dynamodb`, `redis`, `dynamodb,redis`) × `check` + `clippy -D
warnings` + `test --no-run` + `test --doc` + `doc` with `RUSTDOCFLAGS="-D warnings"`.
The `cargo check -p pmcp-tasks --features X` rows are the dev-dep-free ones. Reuse; do
not write a new script.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `../provable-contracts/contracts/pmcp/*.yaml` | contract | n/a | The sibling repo does not exist on this machine (`ls -d ../provable-contracts` → No such file or directory). CLAUDE.md requires contract-first + `make comply` inside `quality-gate`; there is nothing in-repo to copy a contract shape from. Needs an explicit planning decision, not a silent skip. |

Partial-analog note: `schema/vendored/ext-tasks/` has no file-level precedent (no
`schema/` directory exists, and no vendored third-party artifact was found under
`src/`, `crates/` or `tests/`). Use the `error_codes.rs` PROVENANCE *idiom* above.

---

## Metadata

**Analog search scope:** `src/server/`, `src/types/`, `src/client/`,
`crates/pmcp-tasks/src/store/`, `tests/`, `tests/common/`, `examples/`,
`fuzz/fuzz_targets/`, `fuzz/Cargo.toml`, `Makefile`, `.claude/skills/`

**Files read this session (24):** `src/server/task_dispatch.rs` (full),
`src/server/tasks.rs` (full), `src/server/task_store.rs` (`:240-440`, `:630-684`),
`src/server/core.rs` (`:1060-1365`, `:1440-1670`), `src/server/mod.rs` (`:1620-1690`),
`src/server/builder.rs` (`:1040-1060`),
`src/server/streamable_http_server.rs` (`:1373-1465`, `:2783-2833`, `:2960-3099`),
`src/types/capabilities.rs` (`:18-122`), `src/types/tasks.rs` (`:200-255`, `:470-530`),
`src/types/protocol/mod.rs` (`:468-547`), `src/types/mrtr.rs` (`:400-540`, `:2505-2580`),
`src/types/protocol/error_codes.rs` (`:160-220`), `src/client/mod.rs` (`:760-840`,
`:1210-1440`, `:2770-2890`), `crates/pmcp-tasks/src/store/generic.rs` (`:228-308`),
`crates/pmcp-tasks/src/store/mod.rs` (`:288-348`),
`crates/pmcp-tasks/src/store/memory.rs` (`:336-406`), `tests/common/v2.rs` (`:130-270`),
`tests/v2_subscriptions.rs` (`:116-236`, `:1288-1428`),
`tests/v2_prohibited_error_codes.rs` (`:676-756`),
`tests/v2_required_headers.rs` (`:750-840`), `examples/s47_v2_stateless_mrtr.rs`
(`:1-130`), `examples/s48_v2_mrtr_client.rs` (full),
`fuzz/fuzz_targets/fuzz_request_state.rs` (full), `Makefile` (`:295-340`)

**Project skills checked:** `.claude/skills/spike-findings-rust-mcp-sdk/` — `SKILL.md`
plus `references/` and `sources/`; **no `rules/` directory**, so no rule files were
loaded. RESEARCH.md already records its one relevant line (`ServerCapabilities` must
gain an `extensions` field — satisfied by Phase 112).

**Pattern extraction date:** 2026-07-28
