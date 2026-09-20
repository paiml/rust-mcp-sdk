# pmcp.run upgrade note — task lifecycle now served over HTTP

**Audience:** pmcp.run platform (the Lambda/`StreamableHttpServer` hosts)
**SDK change:** Phase 102 "HTTP task dispatch" (pmcp core)
**Status:** on `gsd/phase-102-http-task-dispatch`; ships in the next pmcp release
(additive minor over the `2.10.0` task base). No wire/protocol break — drop-in.

---

## TL;DR

The `tasks/*` lifecycle is now served by the **high-level `Server`** (and therefore
by `StreamableHttpServer`). Until now Phase 101 put the whole lifecycle on
`ServerCore` only, and `Server::handle_request` **hard-rejected** `tasks/*` — so
pmcp.run had to shim every request through `ServerCore::handle_request`.

**You can now delete that shim.** A store-backed high-level `Server` serves the
full lifecycle over real HTTP with the same wire shapes you already proxy.

---

## Why it matters to pmcp.run

- **No more `ServerCore::handle_request` shim.** Build a normal high-level
  `Server`, attach a `TaskStore`, serve it via `StreamableHttpServer`. The
  `tasks/*` endpoints and the task-augmented `tools/call` create-path are handled
  for you.
- **Capability is auto-advertised.** A store-backed `Server` advertises the
  `tasks` capability at `build()` — your `initialize` response gains `tasks`
  automatically; nothing to hand-roll.
- **One shared implementation.** `Server` and `ServerCore` now route through a
  single `task_dispatch` unit, so there is no second copy that can drift from the
  Phase 101 behavior you already validated.

---

## The new API

```rust
use std::sync::Arc;
use pmcp::Server;

let store: Arc<dyn pmcp::TaskStore> = /* your store (DynamoDB-backed in prod) */;

let server = Server::builder()
    .name("my-task-server")
    .version("1.0.0")
    .tool("long_running_tool", MyTaskTool)   // declares TaskSupport::Required/Optional
    .task_store(store)                        // <-- new: attach the backend
    .build()?;                                // <-- auto-advertises `tasks`

// Serve over HTTP exactly as before — StreamableHttpServer calls Server::handle_request.
```

- `ServerBuilder::task_store(Arc<dyn TaskStore>)` — the standard polling backend.
- `ServerBuilder::with_task_store(Arc<dyn TaskRouter>)` — legacy experimental
  router path (kept for back-compat; prefer `task_store`).
- A tool declaring `TaskSupport::Required` with **no** backend is now a build-time
  error (fail-closed), so you can't ship a hollow `tasks` capability.

### Before / after

```rust
// BEFORE (Phase 101): tasks/* rejected by Server, so pmcp.run shimmed:
//   let core: ServerCore = ...;
//   let resp = core.handle_request(id, request, auth_context).await;  // <-- delete this

// AFTER (Phase 102): the high-level Server serves it directly:
let resp = server.handle_request(id, request, auth_context).await;
```

A complete worked HTTP server lives in `examples/s46_http_tool_as_task.rs`
(`cargo run --example s46_http_tool_as_task --features full`) — built **only** via
`Server::builder()…task_store()`, no `ServerCore` reference.

---

## Per-user task isolation maps to your proxy header

Task ownership is derived from the authenticated principal, **never** from client
params (IDOR mitigation). Over HTTP the SDK reads pmcp.run's proxy headers:
`extract_auth_from_proxy_headers` sets `AuthContext.subject = x-pmcp-user-id`
(`client_id` stays `None`), and the owner key **is** that subject.

- **A task created under `x-pmcp-user-id: alice` is owned by `alice`.** Owner B
  with a different `x-pmcp-user-id` cannot `get` / `result` / `cancel` it —
  proven by a live-HTTP cross-owner isolation test.
- **Subject-first, not app-first** (Phase 102 fix): ownership keys on the OAuth
  subject, so a user keeps their tasks across reconnects/sessions. (A prior
  pre-release revision keyed on `client_id` first, which would have collapsed
  per-user isolation to per-application for any auth path that populated it — that
  is fixed; with your proxy `client_id` is `None` anyway, so behavior is the
  intended per-user isolation.)

### ⚠ Transports without an auth context have no per-user isolation
If you ever run a `task_store`-backed server over **stdio** (or any transport that
doesn't resolve a per-request `AuthContext`), every task lands under a single
`"local"` owner — no per-user isolation. That's fine for single-user/CLI use; for
multi-tenant hosting keep using the authenticating HTTP path (which you do). This
is now documented on `Server::run` / `run_stdio`.

---

## Wire shapes are frozen — your proxy & WASM client need no changes

The over-the-wire contract is unchanged from what Phase 101 produced and what you
already replay through the proxy:

- **Store-minted task id** appears on both `CreateTaskResult.task.taskId` **and**
  `_meta.relatedTask.taskId` (the tool's fabricated id is never trusted on the
  wire — the three ids are consistent).
- **`tasks/result` on a still-pending task** returns the frozen `-32002` error
  (precedence preserved).
- `tasks/get | list | cancel` return the same typed result shapes.

No changes required to the mcp-proxy id-rewriting or the WASM agent client.

---

## Migration checklist for pmcp.run

1. Bump the `pmcp` dependency to the release carrying Phase 102.
2. Replace `ServerCore::handle_request(...)` task plumbing with a high-level
   `Server::builder()…tool(...).task_store(store).build()` served by
   `StreamableHttpServer` (your existing serve path).
3. Drop the `ServerCore` shim and any `tasks/*`-reject workaround.
4. Confirm tools that must always create tasks use `TaskSupport::Required` (now a
   build-time guarantee that the backend exists).
5. Keep emitting `x-pmcp-user-id` from the proxy — it is the task owner key. No
   other header is required for isolation.

---

## Verification reference (already green in the SDK)

- `tests/tool_as_task_lifecycle_http.rs` — live HTTP loopback round-trip
  (`initialize → call(task) → tasks/get → tasks/result`) over real
  `StreamableHttpServer` + `StreamableHttpTransport`, plus live-HTTP cross-owner
  isolation. Ephemeral port, clean server shutdown.
- `examples/s46_http_tool_as_task.rs` — the pmcp.run-shaped reference server.
- Phase 101 `ServerCore` task tests still green (zero behavior change to the path
  you already shipped); full lib suite + `make quality-gate` + `make doc-check`
  pass.
