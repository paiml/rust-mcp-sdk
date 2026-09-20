# Architecture Research

**Domain:** Dual-version (2025-11-25 + 2026-07-28) MCP protocol support inside the pmcp Rust SDK
**Researched:** 2026-07-22
**Confidence:** HIGH (integration points read directly from source; v2 wire semantics confirmed against the official RC changelog)

## Scope

How the 2026-07-28 (v2) spec features integrate with pmcp's **existing** architecture, running both versions concurrently via per-request negotiation (no hard cutover). Every integration point below is grounded in the current code, not guessed. File/line references are to the tree at `pmcp 2.17.0`.

Authoritative v2 wire semantics (RC changelog, https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/):
- `initialize`/`initialized` handshake **removed** (SEP-2575); protocolVersion + clientInfo + capabilities move to per-request `_meta` (`io.modelcontextprotocol/clientInfo`). New `server/discover` fetches capabilities on demand.
- `Mcp-Session-Id` **removed** (SEP-2567); state is application-minted handles passed as ordinary tool args.
- Required Streamable-HTTP headers (SEP-2243): `MCP-Protocol-Version`, `Mcp-Method`, `Mcp-Name` — for body-free LB/gateway routing.
- Tasks → extension: `tasks/list` **removed** (unscopable without sessions), `tasks/update` **added**, creation **server-directed**.
- Elicitation: `InputRequiredResult` + `requestState` multi-round-trip replaces held-open SSE.
- JSON Schema 2020-12; `structuredContent` may be **any JSON value** (not only object).
- Error `-32002` → `-32602` (SEP-2164).

## Standard Architecture

### System Overview — where a version signal enters and flows

```
┌──────────────────────────────────────────────────────────────────────┐
│  TRANSPORT INGRESS  (per-request era detection lives HERE)            │
│  ┌────────────────────────┐   ┌──────────────────────────────────┐   │
│  │ streamable_http_server │   │ stdio.rs / wasm_http.rs          │   │
│  │  - MCP-Protocol-Version │   │  - v1: initialize handshake      │   │
│  │  - Mcp-Method/Mcp-Name  │   │  - v2: _meta clientInfo+version  │   │
│  │  - Mcp-Session-Id (v1)  │   │                                  │   │
│  └──────────┬─────────────┘   └───────────────┬──────────────────┘   │
│             │  builds ProtocolContext{era, version, clientInfo, caps} │
├─────────────┼──────────────────────────────────┼─────────────────────┤
│             ▼   (threaded next to auth_context) ▼                     │
│  DISPATCH  ServerCore::handle_request_internal (core.rs:1118)         │
│   ┌──────────────────────────────────────────────────────────────┐   │
│   │ era-gate:  V1 → initialize/initialized + tasks/list allowed   │   │
│   │            V2 → server/discover + tasks/update, no initialize  │   │
│   │  error codes: V1 -32002  |  V2 -32602                          │   │
│   └───────────────────────────┬──────────────────────────────────┘   │
├───────────────────────────────┼──────────────────────────────────────┤
│  HANDLER SURFACE                ▼                                      │
│   RequestHandlerExtra{ request_meta, + client_info(), version() }     │
│   TaskRouter (serde_json::Value seam) ── unchanged boundary           │
├───────────────────────────────┼──────────────────────────────────────┤
│  STORAGE (unchanged)            ▼                                      │
│   GenericTaskStore ─ InMemory / DynamoDB / Redis                      │
└──────────────────────────────────────────────────────────────────────┘
```

The one architectural idea that makes dual-version tractable: **detect era at ingress, carry it as an explicit `ProtocolContext` alongside `auth_context`, and era-gate a small number of decision points** — rather than forking the transport or the dispatcher.

### Component Responsibilities (new vs modified)

| Component | File | Status | v2 Responsibility |
|-----------|------|--------|-------------------|
| `version.rs` negotiation | `src/types/protocol/version.rs` | **MODIFIED** | Add `"2026-07-28"` to `SUPPORTED_PROTOCOL_VERSIONS`; add `protocol_era(version) -> Era` classifier. `negotiate_protocol_version` unchanged in shape. |
| `ProtocolContext` | new (`src/shared/` or `src/server/`) | **NEW** | Value object `{ era, negotiated_version, client_info, client_capabilities }` built at ingress, threaded through dispatch. |
| `http_constants.rs` | `src/shared/http_constants.rs` | **MODIFIED** | Add `MCP_METHOD = "mcp-method"`, `MCP_NAME = "mcp-name"`. `MCP_SESSION_ID`/`MCP_PROTOCOL_VERSION` already present. |
| streamable-HTTP server | `src/server/streamable_http_server.rs` | **MODIFIED** | Era-gate session resolution; enforce inbound v2 required headers; suppress `Mcp-Session-Id` on v2 responses. |
| streamable-HTTP client | `src/shared/streamable_http.rs`, `wasm_http.rs` | **MODIFIED** | Emit `MCP-Protocol-Version`/`Mcp-Method`/`Mcp-Name` on v2 requests; stop expecting `Mcp-Session-Id`. |
| `ServerCore` dispatch | `src/server/core.rs:1118` | **MODIFIED** | Era-gate the `initialize` arm, the not-initialized guard (`-32002`→`-32602`), and method-not-found; add `server/discover` + `tasks/update` arms. |
| `RequestHandlerExtra` | `src/server/cancellation.rs:179`, `src/shared/cancellation.rs:50` | **MODIFIED** | Add typed `client_info()` / `protocol_version()` accessors over the existing `request_meta` field. |
| `RequestMeta` | `src/types/protocol/mod.rs:315` | **REUSED as-is** | The Phase-109 `#[serde(flatten)] other` map already round-trips `io.modelcontextprotocol/clientInfo`. No type change needed. |
| `ClientRequest` enum | `src/types/protocol/mod.rs:478` | **MODIFIED** | Add `ServerDiscover` and `TasksUpdate` variants (serde-tagged by method name). |
| `TaskRouter` trait | `src/server/tasks.rs:24` | **MODIFIED (additive)** | Add `handle_tasks_update`; keep `handle_tasks_list` (v1-only routing). serde_json::Value seam preserved. |
| `pmcp-tasks` router/store | `crates/pmcp-tasks/` | **MODIFIED (API reshape)** | Implement `tasks/update`, server-directed creation; storage backends untouched. |
| structured-output bridge | `src/server/core.rs:689-703` | **MINOR** | Already emits `CallToolResult::structured(value)` for any `Value`. Add scalar branch to `summarize_structured_output`. |
| schema validation | `src/server/output_validation.rs`, `schema_utils.rs` | **MODIFIED** | Accept JSON Schema 2020-12 keywords; validate non-object `structuredContent`. |
| OAuth | `src/server/auth/` | **MODIFIED** | RFC 9207 `iss` validation, DCR `application_type` (the 6 auth SEPs). |
| conformance harness | `crates/pmcp-team-servers` (Phase 109) | **EXTENDED** | Add v2 fixtures; run both eras through `ConformanceTarget`. |

## Data Flow

### v1 request (2025-11-25) — unchanged path

```
POST (Mcp-Session-Id, MCP-Protocol-Version: 2025-11-25)
  → extract_session_and_protocol_headers()          [streamable_http_server.rs:895]
  → is_initialize_request()? yes → process_init_session() mints session   [:468]
  → ServerCore::handle_request_internal: Initialize arm → handle_initialize()  [core.rs:451]
  → response emits Mcp-Session-Id + MCP-Protocol-Version                   [:1093/:1095]
```

### v2 request (2026-07-28) — reuses the existing stateless branch

```
POST (MCP-Protocol-Version: 2026-07-28, Mcp-Method, Mcp-Name; NO Mcp-Session-Id)
  → validate_headers(): v2 requires the three headers (400 if missing)
  → protocol_era(header) = V2  → BUILD ProtocolContext
  → era-gate: SKIP process_init_session (treat as stateless regardless of session_id_generator)
  → ProtocolContext threaded next to auth_context into handle_request()
  → dispatch: tools/call reads _meta clientInfo → RequestHandlerExtra.client_info()
  → response: echo MCP-Protocol-Version: 2026-07-28, emit NO Mcp-Session-Id
```

### Key insight per sub-question

**(a) Where negotiation lives.** Today it is *split and implicit*: the pure `negotiate_protocol_version()` fn is called at three handler sites (core.rs:457, mod.rs:1262, wasm_server.rs:127), and the transport separately validates the `MCP-Protocol-Version` header against `SUPPORTED_PROTOCOL_VERSIONS` (streamable_http_server.rs:671). For dual-version this must become *explicit and single-point*: **resolve era at transport ingress** (the header/handshake signal only exists there) and **carry the result into dispatch** as `ProtocolContext`. ServerCore keeps its `initialize` arm for v1 clients; v2 clients never send `initialize`, so the arm is simply never hit for them — no removal, pure era-gating. This is why "initialize removed in v2 but still works for v1" needs zero conditional inside `handle_initialize`; the branch just isn't reached under V2.

**(b) Session bifurcation without forking the transport.** The transport *already* bifurcates: `StreamableHttpServerConfig::stateless()` (streamable_http_server.rs:249) sets `session_id_generator: None`, and `process_init_session`/`validate_non_init_session` (:468/:512) branch on `session_id_generator.is_some()`. **v2 maps directly onto the existing stateless branch.** The change is one era gate *before* session resolution: `if ctx.era == V2 { skip session entirely } else { existing stateful/stateless config path }`. `SessionInfo{ initialized, protocol_version }` (:266) stays for v1; v2 creates no `SessionInfo`. `compute_outbound_protocol_version` (:944) already returns the right value; the only new rule is "do not insert `Mcp-Session-Id` when era == V2". No fork — both eras share `handle_post_fast_path`/`handle_post_with_middleware`.

**(c) `_meta` clientInfo → `RequestHandlerExtra`.** The plumbing already exists end-to-end: `req._meta` (a `RequestMeta` whose Phase-109 `#[serde(flatten)] other` map catches `io.modelcontextprotocol/*` keys, mod.rs:333-345) is serialized to `Value` and passed via `.with_request_meta()` into `RequestHandlerExtra.request_meta` (core.rs:514-518). So v2's `io.modelcontextprotocol/clientInfo` *already round-trips into handlers today* for `tools/call`. Three gaps: (1) it is a raw buried `Value` — add typed `extra.client_info()` / `extra.protocol_version()` accessors; (2) only `tools/call` threads `_meta` — `get_prompt`/`read_resource`/etc. need the same `.with_request_meta()` wiring for v2 (they receive per-request clientInfo too); (3) capability-gated code reads `client_capabilities` cached at `initialize` (core.rs:454) — v2 has no initialize, so a v2 path must populate/read capabilities from per-request `_meta`. The `ProtocolContext` carries `client_capabilities` to close (3).

**(d) pmcp-tasks migration.** The `serde_json::Value` seam of `TaskRouter` (tasks.rs:24) is the load-bearing insulation — it lets the API reshape without touching `pmcp` core types or the storage backends (`GenericTaskStore` + DynamoDB/Redis are domain-object stores, untouched). Reshape: (i) add `handle_tasks_update` to the trait + `METHOD_TASKS_UPDATE` (constants.rs) + `ClientRequest::TasksUpdate`; (ii) **keep** `handle_tasks_list` on the trait but era-gate routing so v2 never dispatches `tasks/list` (task_dispatch.rs `route_tasks_endpoint`); (iii) server-directed creation reuses the existing `tool_requires_task` hook (tasks.rs:78) — extend it so the router can elect task creation without a client-supplied `task` field. **Owner-binding pitfall:** `resolve_owner(subject, client_id, session_id)` (tasks.rs:67) prefers subject then client_id then session_id. v2 has no session_id → unauthenticated v2 servers lose per-caller task isolation. Flag: v2 task servers should require OAuth (subject) or an app-minted handle.

**(e) Required headers.** Split by direction. **Server-inbound** (streamable_http_server.rs `validate_headers`): for V2, require `MCP-Protocol-Version=2026-07-28`, `Mcp-Method`, `Mcp-Name`; 400 on absence; optionally cross-check header method/name vs body. **Client-outbound** (src/shared/streamable_http.rs + wasm_http.rs): emit the three headers on every v2 request. Add `MCP_METHOD`/`MCP_NAME` to http_constants.rs. These headers duplicate body fields deliberately (body-free routing), so they are derivable at send time from the JSON-RPC method + tool name.

**(f) `server/discover`.** Maps onto the existing registry-authoritative capability derivation (Phase 106). `ServerCore.capabilities` (core.rs:210) is already computed; `handle_initialize` returns `InitializeResult{ capabilities, server_info, ... }` (core.rs:459). Add a `handle_discover()` that returns the *same* capability projection **without** the handshake side effects (does not set `initialized`, does not cache client caps). Low complexity — a read-only view of state that already exists.

**(g) Structured-output bridge + JSON Schema 2020-12.** The emit path is *already* value-agnostic: core.rs:703 calls `CallToolResult::structured(value)` where `value: serde_json::Value`, so non-object `structuredContent` already flows to the wire. Real work is in validation: `output_validation.rs`/`schema_utils.rs` must accept 2020-12 keywords (`$ref`/`$defs`, `oneOf`/`anyOf`/`allOf`, conditionals) and validate top-level non-object schemas. Also `summarize_structured_output` (core.rs:1329) matches only `Array`/`Object` — add a scalar branch. `ttlMs`/`cacheScope` are additive optional fields on the result envelope.

**(h) Build order** — see dependency ordering below.

## Suggested Build Order (dependency-driven)

```
(1) Version plumbing spine  ──►  (2) Stateless HTTP + elicitation
      │  │  │                          │
      │  │  └──►  (3) server/discover ──┤ (parallel w/ 2)
      │  └─────►  (4) Tasks extension  ─┤ (needs era + owner-from-_meta)
      └────────►  (5) JSON Schema 2020-12 + structured bridge (mostly independent)
                  (6) Auth SEPs (fully independent)
                          └──────────►  (7) Conformance (validates all, last)
```

1. **Version plumbing spine (foundation, blocks 2/3/4).** Extend `version.rs` (add `2026-07-28`, `protocol_era`), add `MCP_METHOD`/`MCP_NAME` constants, define `ProtocolContext`, thread era+clientInfo through `handle_request` and into `RequestHandlerExtra` (typed accessors), add `ServerDiscover`+`TasksUpdate` enum variants (types only), era-gate the `-32002`→`-32602` error path. Nothing else can be era-aware until this lands. **Depends on:** nothing.
2. **Stateless streamable-HTTP.** Era-gate session resolution (v2 → existing stateless branch), inbound v2 header enforcement + outbound emission (no session id for v2), client-side header emission, multi-round-trip elicitation (`InputRequiredResult`/`requestState`). **Depends on:** (1).
3. **`server/discover` handler + v2 capability capture from `_meta`.** **Depends on:** (1). Parallelizable with (2).
4. **Tasks extension migration.** Era-gated routing (drop `tasks/list` for v2, add `tasks/update`, server-directed creation), owner-binding without session. **Depends on:** (1); loosely on (2) for statelessness semantics.
5. **JSON Schema 2020-12 + structured bridge + caching hints.** Largely independent; only touches (1) for era-gating validation strictness. Parallelizable with (2)-(4).
6. **Auth-hardening SEPs** (RFC 9207 `iss`, DCR `application_type`). Fully independent; parallelizable.
7. **Conformance** against the official suite; extend the Phase-109 harness to run both eras. **Depends on:** all above; runs last.

## Architectural Patterns

### Pattern 1: Era detection at ingress, era-gating at decision points
**What:** Resolve `ProtocolEra` once where the version signal exists (HTTP header / stdio handshake / `_meta`) and thread it as an explicit value; gate the handful of divergent decisions (session, initialize vs discover, tasks/list vs tasks/update, error code) on it.
**When:** Any dual-protocol coexistence.
**Trade-offs:** One new value threaded through call sites vs. avoiding a full transport/dispatcher fork. Strongly favors gating — the divergent surface is small (~6 gates).

### Pattern 2: `serde_json::Value` trait seam as a version firewall
**What:** `TaskRouter`'s `Value` boundary lets the Tasks *wire API* reshape (list→update, server-directed creation) while `pmcp` core types and storage backends stay put.
**When:** Extension whose spec is still moving. Already proven by the v1.0→v2.x Tasks evolution.
**Trade-offs:** Loses compile-time typing at the seam; buys total decoupling and lets the DynamoDB/Redis investment survive a breaking wire change untouched.

### Pattern 3: Flattened namespaced `_meta` as the v2 transport for out-of-band context
**What:** The Phase-109 `RequestMeta.other` flatten map is exactly the shape v2 uses for `io.modelcontextprotocol/clientInfo`, capabilities, and W3C trace keys.
**When:** Already the mechanism; v2 just adds reserved keys. Add typed accessors, not new types.
**Trade-offs:** Stringly-typed keys; but zero new protocol types and byte-identical v1 serialization (empty map emits nothing).

## Anti-Patterns

### Anti-Pattern 1: Forking the transport into v1 and v2 servers
**What people do:** Stand up a parallel `streamable_http_server_v2.rs`.
**Why it's wrong:** Doubles the DNS-rebinding/auth/middleware/SSE surface and guarantees drift; the two eras differ only in session handling and headers.
**Instead:** Era-gate `process_init_session` and header emission inside the *existing* handlers. v2 == the existing stateless branch.

### Anti-Pattern 2: Deleting the `initialize` arm / `tasks/list` to "modernize"
**What people do:** Remove v1 code paths because v2 dropped them.
**Why it's wrong:** Breaks the dual-version contract — v1 clients (Claude Code, Cursor on older versions) still handshake and still call `tasks/list`.
**Instead:** Keep them; era-gate so V2 requests never reach them.

### Anti-Pattern 3: Assuming v2 statelessness gives task owner isolation for free
**What people do:** Ship v2 task servers without auth, relying on old session-based owner binding.
**Why it's wrong:** No `Mcp-Session-Id` in v2 → `resolve_owner` falls through to a shared/`"local"` owner → cross-caller task leakage.
**Instead:** Require OAuth subject or an app-minted handle for v2 task servers; document the precedence.

## Integration Points

### Internal Boundaries

| Boundary | Communication | v2 consideration |
|----------|---------------|------------------|
| transport ↔ ServerCore | `handle_request(id, request, auth_context)` (core.rs:1172/1391) | Add `ProtocolContext` param (or fold into extra) — the single new thread through the seam. |
| ServerCore ↔ handlers | `RequestHandlerExtra` | New `client_info()`/`protocol_version()` accessors; ensure all methods thread `_meta`, not just `tools/call`. |
| pmcp ↔ pmcp-tasks | `TaskRouter` (`Value`) | Additive `handle_tasks_update`; era-gated `tasks/list`. |
| pmcp-tasks ↔ storage | `GenericTaskStore`/`StorageBackend` | Unchanged. |
| server ↔ client transports | HTTP headers | New `Mcp-Method`/`Mcp-Name`; conditional `Mcp-Session-Id`. |

## Sources

- Existing code (read directly): `src/types/protocol/version.rs`, `src/types/protocol/mod.rs` (RequestMeta:315, ClientRequest:478), `src/server/core.rs` (dispatch:1118, initialize:451, structured bridge:686-703), `src/server/streamable_http_server.rs` (session/header handling:249,468,895,944,1222), `src/server/tasks.rs` (TaskRouter:24), `src/server/task_dispatch.rs`, `src/shared/http_constants.rs`, `src/server/cancellation.rs:179`, `crates/pmcp-tasks/src/constants.rs` — HIGH
- MCP 2026-07-28 Release Candidate changelog: https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/ — HIGH
- Project memory `project_mcp_spec_2026_07_28_impact.md` (repo-grepped impact map) — HIGH

---
*Architecture research for: dual-version MCP protocol support in pmcp*
*Researched: 2026-07-22*
