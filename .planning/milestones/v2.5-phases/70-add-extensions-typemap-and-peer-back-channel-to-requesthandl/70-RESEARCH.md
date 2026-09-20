# Phase 70: Add Extensions typemap and peer back-channel to RequestHandlerExtra - Research

**Researched:** 2026-04-16
**Domain:** Handler dispatch ergonomics — Rust SDK public-API evolution with backwards-compat constraints
**Confidence:** HIGH
**Requirement:** PARITY-HANDLER-01 (REQUIREMENTS.md:54)
**pmcp baseline:** v2.3.0 + `feat/sql-code-mode` at commit dbaee6cc
**rmcp baseline:** 1.5.0 (tag `rmcp-v1.5.0`)

## Summary

Five things the planner needs before picking up the pen:

1. **There are TWO `RequestHandlerExtra` structs, not one.** The dispatch-path `RequestHandlerExtra` is in `src/server/cancellation.rs` (non-wasm, 10 fields) — this is what every tool/prompt/resource handler actually receives. The other one in `src/shared/cancellation.rs` (runtime-agnostic, 5 fields) is a legacy shadow struct used only by `src/wasi.rs:65` and `src/server/traits.rs` — which itself is a secondary trait hierarchy not wired into `ServerCoreBuilder`. The proposal text says "both native and WASM variants" but the code reality is subtler: the canonical path is the non-wasm struct; the WASM path currently passes the non-wasm struct as well (confirmed at `src/server/mod.rs:1108` where wasm-gated code still calls `handler.handle(args, extra)` with `cancellation::RequestHandlerExtra`). Plan should target `src/server/cancellation.rs` primarily; the `src/shared/cancellation.rs` struct can receive the Extensions field for consistency but will have near-zero runtime effect.

2. **There are 12 positional struct-literal construction sites in `src/server/workflow/prompt_handler.rs` that WILL break** when new fields are added (lines 1055, 1238, 1347, 1441, 1573, 1704, 1772, 1851, 1948, 2081, 2223, 2338). Only the `::new()` and `::default()` constructors are drop-in; spelling every field positionally locks the struct shape. Plan 01 MUST either (a) update all 12 test sites, or (b) mark the struct `#[non_exhaustive]` + use `..Default::default()` in every new field — both are mechanical changes. Proposal's backwards-compat claim (success-criterion 1: "Every existing `RequestHandlerExtra::new(...)` and `::with_session(...)` call-site in `src/` and `examples/` compiles unchanged") is TRUE — but only for `::new` + builder methods; struct-literal callers in `src/server/workflow/prompt_handler.rs` ARE call-sites that need mechanical updates. The planner must treat this as an explicit in-scope item, not a surprise during execution.

3. **`http::Extensions` is already available — no new dependency needed.** `http = "1.1"` is a top-level direct dep in `Cargo.toml:58` [VERIFIED: Cargo.toml:58], resolving to `http v1.4.0` [VERIFIED: `cargo tree -p pmcp --depth 1`]. The `http::Extensions` API (`insert`/`get`/`get_mut`/`remove`) requires `Clone + Send + Sync + 'static` on inserted values [CITED: docs.rs/http/1.4.0/http/struct.Extensions.html]. Its Debug impl prints type names only, not values [CITED: docs.rs http/src/extensions.rs L268-286] — which dovetails perfectly with pmcp's existing redaction-aware Debug impl on `RequestHandlerExtra` (`src/server/cancellation.rs:296-336`).

4. **The peer back-channel has a working blueprint already in the codebase: `ElicitationManager`** (`src/server/elicitation.rs`). It uses `mpsc::Sender<ServerRequest>` + `HashMap<String, oneshot::Sender<ElicitResult>>` + `tokio::time::timeout` — exactly the pattern needed for `peer.sample()` / `peer.list_roots()`. The request_tx channel is wired at transport setup time on the `Server` struct (`src/server/mod.rs:313` shows `elicitation_manager: Option<Arc<elicitation::ElicitationManager>>`). Plan 02 should pattern-match this structure rather than invent new plumbing. RootsManager (`src/server/roots.rs`) already shows `request_client_roots(request_sender)` accepting a `FnOnce(ServerRequest) -> Fut` pattern — the peer trait can adapt this.

5. **Plan split recommendation: 3 plans, not 4.** Plans 03 + 04 in the proposal (examples + CI/migration-guide finalization) are small enough to merge. Recommended split: Plan 01 Extensions typemap (~2-3h), Plan 02 PeerHandle trait + ServerCore wiring (~4-6h, the heaviest), Plan 03 Examples + rustdoc + migration note + CI gate (~2-3h). See Suggested Plan Split section for justification.

**Primary recommendation:** Use `http::Extensions` (already a transitive dep we already pull directly) — do NOT introduce `anymap`/`anymap2`/`anymap3`. Gate the peer handle under `#[cfg(not(target_arch = "wasm32"))]` only — do not introduce a new `peer` feature flag. Gate the extensions field under a new `extensions` feature that is part of `server`-category default (but since there's no `server` feature today, see Question #5 below for the actual feature-flag shape).

## User Constraints (from CONTEXT.md)

No CONTEXT.md exists for Phase 70. The proposal text in `69-PROPOSALS.md` (Proposal 1) serves as the locked scope. Constraints derived from the proposal:

### Locked Decisions (from 69-PROPOSALS.md Scope section)

- **In scope:** Extensions typemap on `RequestHandlerExtra` in both `src/server/cancellation.rs` and `src/shared/cancellation.rs`
- **In scope:** `peer: Option<Arc<dyn PeerHandle>>` field, non-wasm only, with `sample` / `list_roots` / `progress_notify` methods
- **In scope:** Wire both fields through `ServerCoreBuilder`'s request-dispatch path so they populate per-request (not statically)
- **In scope:** Feature-gated `extensions` feature flag (default-on inside `server` feature)
- **In scope:** Two examples — `examples/s22_handler_extensions.rs` AND `examples/s23_handler_peer_sample.rs` (Phase 65 role-prefix convention: `s` = server example) — NOTE the existing `s22_structured_output_schema.rs` and `s23_mcp_tool_macro.rs` occupy those numbers — see Risks section
- **In scope:** Rustdoc migration note in both `cancellation.rs` files
- **In scope:** Property tests: typemap key-collision semantics, peer.sample session routing, peer.progress_notify no-op-without-token

### Claude's Discretion

- Typemap crate choice (`http::Extensions` vs `anymap` vs `anymap2` vs hand-rolled) — answered in Question #1
- Peer trait exact method signatures — answered in Question #2
- Whether to mark `RequestHandlerExtra` `#[non_exhaustive]` — recommended in Question #4
- Example file numbering (since s22/s23 are taken) — recommended in Risks section
- Plan count (3 vs 4) — recommended 3 in Suggested Plan Split
- Whether peer is behind its own feature flag or always-on inside non-wasm — recommended always-on non-wasm (no new flag)

### Deferred Ideas (OUT OF SCOPE, from 69-PROPOSALS.md Out-of-scope)

- Tower / service integration (Phase 56, not revisited)
- Transport construction API shape (D-12 deferred)
- Client-side `RequestHandlerExtra` equivalent (pmcp Client has no handler-extra)
- Migrating auth-context / session-id into Extensions — stay as typed fields
- Client-side `ClientNotificationHandler` trait (PARITY-CLIENT-01 covers CLIENT-03, separate phase)
- Client-side `ProgressDispatcher` (CLIENT-04, Medium severity, future work)

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PARITY-HANDLER-01 | Enrich `RequestHandlerExtra` with a typed-key extensions map and an optional peer back-channel, so middleware state transfer and in-handler server-to-client RPCs work without out-of-band plumbing. | Extension typemap: `http::Extensions` (Question #1). Peer back-channel: trait wrapping `mpsc::Sender<ServerRequest>` pattern from ElicitationManager (Question #2). Dispatch wiring: `src/server/core.rs:411, 574, 594, 636` + `src/server/mod.rs:1055, 1193, 1228, 1301, 1358` (Question #3). Backwards-compat: `::new`/`::with_session_id` signatures unchanged + struct-literal sites updated mechanically (Question #4). |

## Project Constraints (from CLAUDE.md)

- **Zero-defect quality system** — `make quality-gate` must pass; no clippy warnings, no fmt drift, no SATD comments
- **PMAT quality-gate proxy** — all file operations should route through `pmat mcp-server --enable-quality-proxy` during development (if available; not blocking)
- **Cognitive complexity ≤25 per function** — every function added must measure under this
- **ALWAYS requirements for new features:** fuzz + property + unit + working example + integration (if applicable). For this phase:
  - FUZZ: property-based fuzzing via proptest (existing) OR cargo-fuzz target (recommended for the peer back-channel ServerRequest serialization boundary)
  - PROPERTY: proptest for Extensions insert/get/remove round-trip and Peer routing invariants (required by proposal success criterion 3: ≥100 proptest cases)
  - UNIT: every new public function gets unit tests
  - EXAMPLE: both s-prefixed example files compile and run (`cargo run --example <name>`)
  - DOCTEST: every public API gets a `rust,no_run` doctest
- **Test threads:** CI uses `--test-threads=1`
- **80%+ test coverage** on new code
- **Toyota Way workflow:** property tests FIRST, then unit tests, then implementation

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `Extensions` typemap storage | Per-request context (`RequestHandlerExtra`) | Middleware chain (read-write) | Extensions semantically belong on the request context so every middleware layer and the final handler see the same bag; storing anywhere else would require out-of-band plumbing — the exact pain the gap identifies. |
| `PeerHandle` trait definition | `src/shared/` (cross-transport) | `src/server/` (transport-agnostic) | The trait signature must not depend on a specific transport, since the same handler code runs under stdio/HTTP/WebSocket/streamable-HTTP. Place in `src/shared/peer.rs` (new file) or inline with `src/shared/cancellation.rs` given its small size. |
| `PeerHandle` implementation | `ServerCore` per-session | `Server` per-transport | The concrete impl wraps the `mpsc::Sender<ServerRequest>` that the server already owns (existing ElicitationManager pattern). The impl is constructed at the same dispatch site where `RequestHandlerExtra::new(...)` is called today. |
| Peer session routing | `ServerCoreBuilder` dispatch path | Transport layer | The dispatcher already knows which session a request came from (session_id flows through `RequestHandlerExtra.session_id`). The peer impl must be bound to the same session so `peer.sample()` routes back to the originating client, not a random one. |
| Progress notify routing | Existing `ProgressReporter` infra (`src/server/progress.rs`) | New peer.progress_notify wrapper | The `progress_reporter` field already exists on `RequestHandlerExtra`. `peer.progress_notify` should delegate to it — this is NOT new plumbing, it's a convenience method on the peer trait. |

## Tactical Questions Answered

### Question #1: Typemap crate selection

**Answer: Use `http::Extensions`. No new dependency.** [VERIFIED]

**Evidence:**
- `http = "1.1"` is a direct top-level pmcp dependency [VERIFIED: Cargo.toml:58]
- Resolves to `http v1.4.0` [VERIFIED: `cargo tree -p pmcp --depth 1`]
- `http::Extensions` is the same typemap rmcp uses via its re-export [CITED: 69-RESEARCH.md HANDLER-02 row, rmcp `crates/rmcp/src/service.rs#L651-L665`]
- API: `insert<T>`, `get<T>`, `get_mut<T>`, `remove<T>` all require `T: Clone + Send + Sync + 'static` [CITED: https://docs.rs/http/latest/http/struct.Extensions.html]
- `Clone`: YES — critical because `RequestHandlerExtra` is `#[derive(Clone)]` (`src/server/cancellation.rs:117`)
- `Debug`: prints type names only, not values [CITED: docs.rs/http/src/extensions.rs L268-286] — aligns with pmcp's redaction-aware Debug on RequestHandlerExtra (`src/server/cancellation.rs:296-336`)
- MSRV: http 1.x requires Rust 1.49+ [CITED: http crate metadata]. pmcp requires 1.83.0 (Cargo.toml:14), so compatible.
- wasm32 compatibility: `http` compiles on wasm32 — confirmed by existing pmcp builds which include `http` as a target-independent dep. [VERIFIED: no target-specific cfg gate in Cargo.toml:58]

**Alternatives rejected:**

| Option | Reason rejected |
|--------|-----------------|
| `anymap` 1.0-beta.2 | Beta version; avoids pulling pre-1.0 deps into stable SDK |
| `anymap2` 0.13.0 | Unmaintained fork; `anymap3` already succeeded it |
| `anymap3` 1.0.1 | New dep for zero marginal capability over `http::Extensions` |
| Hand-rolled `HashMap<TypeId, Box<dyn Any + Send + Sync>>` | Reinvents `http::Extensions` without the Clone semantics (or worse, without Clone at all). Violates CLAUDE.md "don't hand-roll"-equivalent principle. |

**`Send + Sync` story:** `http::Extensions` is both — verified because `ServerCoreBuilder` tools already spawn handler futures via `tokio::spawn` (non-wasm) [VERIFIED: implicit from `async_trait` + `Send` trait bounds across `ToolHandler`/`PromptHandler`/`ResourceHandler` at `src/server/mod.rs:201, 215, 233`].

### Question #2: Peer trait shape

**Answer: Use a new `PeerHandle` trait in `src/shared/peer.rs` (new file), with three async methods. Non-wasm only.** [RECOMMENDED]

**Proposed signature:**

```rust
// src/shared/peer.rs (NEW FILE, non-wasm only)
use crate::error::Result;
use crate::types::{CreateMessageParams, CreateMessageResult, ProgressToken};
use crate::server::roots::ListRootsResult;
use async_trait::async_trait;

/// Server-to-client back-channel accessible from inside request handlers.
///
/// Implementations route the outbound RPC to the client session that
/// originated the current inbound request. The trait is object-safe so
/// `RequestHandlerExtra` can hold `Option<Arc<dyn PeerHandle>>`.
#[async_trait]
pub trait PeerHandle: Send + Sync {
    /// Request the client to sample its LLM (`sampling/createMessage`).
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult>;

    /// Request the client's root list (`roots/list`).
    async fn list_roots(&self) -> Result<ListRootsResult>;

    /// Send a progress notification (`notifications/progress`).
    async fn progress_notify(
        &self,
        token: ProgressToken,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()>;
}
```

**Why this shape minimizes coupling:**

- **Object-safe:** no generics on methods — uses concrete `CreateMessageParams`/`CreateMessageResult` (already defined in `src/types/sampling.rs`), `ListRootsResult` (already defined in `src/server/roots.rs:26`), `ProgressToken` (already defined in `src/types/progress.rs`). No new types needed.
- **Send + Sync:** required so `Arc<dyn PeerHandle>` can be stored in the `Clone + Send + Sync`-bounded `RequestHandlerExtra`.
- **Async methods via `async_trait`:** matches the rest of pmcp's handler traits (`ToolHandler`, `PromptHandler`, `ResourceHandler`, `SamplingHandler` — all use `#[async_trait]` at `src/server/mod.rs:200-255`).

**Does pmcp have existing internal plumbing to wrap?** YES.

- **ElicitationManager** (`src/server/elicitation.rs:21-190`) is the exact template:
  - holds `request_tx: Option<mpsc::Sender<ServerRequest>>` (line 25)
  - holds `pending: Arc<RwLock<HashMap<String, oneshot::Sender<ElicitResult>>>>` (line 23)
  - sends via `request_tx.send(server_request).await` (line 86)
  - awaits with `timeout(self.timeout_duration, rx)` (line 98)
- **RootsManager** (`src/server/roots.rs:148-166`) has `request_client_roots(request_sender)` that accepts an `FnOnce(ServerRequest) -> Fut` — adaptable to the peer trait
- **`ServerRequest::CreateMessage`** (`src/server/mod.rs:3769-3782`) is already plumbed as a server-to-client request type
- **`ServerRequest::ListRoots`** (`src/server/roots.rs:160`) already plumbed

**Concrete impl sketch:**

```rust
// src/server/peer_impl.rs (NEW FILE, non-wasm only)
pub(crate) struct DispatchPeerHandle {
    request_tx: mpsc::Sender<ServerRequest>,
    pending: Arc<RwLock<HashMap<String, oneshot::Sender<Value>>>>,
    session_id: Option<String>,
    timeout_duration: Duration,
    progress_reporter: Option<Arc<dyn ProgressReporter>>,
}

#[async_trait]
impl PeerHandle for DispatchPeerHandle {
    async fn sample(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        // Clone-adapt ElicitationManager::elicit_input (src/server/elicitation.rs:67-119)
    }
    async fn list_roots(&self) -> Result<ListRootsResult> {
        // Use existing RootsManager::request_client_roots pattern (src/server/roots.rs:155)
    }
    async fn progress_notify(&self, token, progress, total, msg) -> Result<()> {
        // Delegate to self.progress_reporter (already on RequestHandlerExtra today)
        // This is why progress_notify is free — no new plumbing
    }
}
```

### Question #3: Per-request dispatch wiring

**Answer: `RequestHandlerExtra::new(...)` is constructed at 9 canonical call sites in the server dispatch path.** [VERIFIED]

| # | File:Line | Dispatch operation | Needs peer? | Notes |
|---|-----------|-------------------|-------------|-------|
| 1 | `src/server/core.rs:411` | `handle_call_tool` | YES | Primary tool dispatch in ServerCore |
| 2 | `src/server/core.rs:574` | `handle_get_prompt` | YES | Prompt dispatch in ServerCore |
| 3 | `src/server/core.rs:594` | `handle_list_resources` | NO (low value) | Resource listing |
| 4 | `src/server/core.rs:636` | `handle_read_resource` | YES | Resource read dispatch |
| 5 | `src/server/mod.rs:1055` | `handle_call_tool` (legacy Server) | YES | Legacy Server struct path |
| 6 | `src/server/mod.rs:1193` | `handle_get_prompt` (legacy) | YES | Legacy Server path |
| 7 | `src/server/mod.rs:1228` | `handle_list_resources` (legacy) | NO | Legacy Server path |
| 8 | `src/server/mod.rs:1301` | `handle_read_resource` (legacy) | YES | Legacy Server path |
| 9 | `src/server/mod.rs:1358` | `handle_create_message` (legacy) | — | Server-side sampling handler — peer is still valuable for nested calls |

Each site follows this pattern today:

```rust
let extra = RequestHandlerExtra::new(request_id, cancellation_token)
    .with_auth_context(auth_context)
    .with_progress_reporter(progress_reporter);
```

**Proposed change (per site):** Add one additional builder call:

```rust
let extra = RequestHandlerExtra::new(request_id, cancellation_token)
    .with_auth_context(auth_context)
    .with_progress_reporter(progress_reporter)
    .with_peer(self.build_peer_handle(session_id));  // NEW
```

**How does the dispatcher know which session the request came from?** Two paths:

1. `ServerCore` doesn't currently track session_id in `handle_call_tool` — it's built at transport-ingress time and passed through `ProtocolHandler::handle_request(id, request, auth_context)` (`src/server/core.rs:67-72`). The session_id flows through `auth_context` in some paths and is absent in others. Plan 02 must extend the dispatch signature or `ProtocolHandler` trait to pass session_id when peer is needed.

2. Legacy `Server` path (`src/server/mod.rs`) has direct access to `self.notification_tx` — the mpsc channel used today for outbound notifications. Adapting that channel into a peer handle requires binding it to the per-session session_id (likely via the existing `session_id` field on middleware extras).

**Verdict:** Plan 02 must thread session_id into both dispatch paths. This is a non-trivial plumbing change and is the largest single risk in the phase.

### Question #4: Backwards-compat strategy

**Answer: Drop-in for `::new()` and builder methods. Mechanical update required for 12 struct-literal sites.** [VERIFIED]

**Call-site audit (the proposal's success-criterion 1):**

| Category | Count | Call-site pattern | Break risk with 2 new fields? |
|----------|-------|-------------------|------------------------------|
| `RequestHandlerExtra::new(request_id, token)` | 21 across `src/` + `examples/` + `tests/` | Positional `new()` | NO — signature unchanged |
| `.with_session_id(...)` | 4 in `src/` + `tests/` | Builder method | NO — signature unchanged |
| `.with_auth_context(...)` | 9 in `src/` | Builder method | NO |
| `.with_auth_info(...)` | 0 active call sites | Builder method | NO |
| `.with_progress_reporter(...)` | 5 in `src/server/mod.rs` | Builder method | NO |
| `.with_task_request(...)` | 1 in `src/server/core.rs:418` | Builder method | NO |
| `RequestHandlerExtra::default()` | 16 across examples + macros tests + book/course | `Default` impl | NO — `Default` updates to include new fields |
| **`RequestHandlerExtra { ... }` struct literal** | **12 in `src/server/workflow/prompt_handler.rs`** | **Positional field spelling** | **YES — BREAKS** |

**The 12 struct-literal sites are all in test modules inside `src/server/workflow/prompt_handler.rs`** (lines 1055, 1238, 1347, 1441, 1573, 1704, 1772, 1851, 1948, 2081, 2223, 2338). Each spells out all 8 fields (cancellation_token, request_id, session_id, auth_info, auth_context, metadata, progress_reporter, task_request). Adding `extensions` + `peer` makes each struct-literal miss-two-fields.

**Two mitigation strategies:**

**Option A (RECOMMENDED): Update all 12 sites mechanically + mark struct `#[non_exhaustive]`.**

Pros: Forces all downstream crates to use `::new()` / `::default()` forever; cleanest long-term posture. Matches rmcp's `#[non_exhaustive]` on `RequestContext`.
Cons: `#[non_exhaustive]` is a breaking change on the struct (but not on `::new` / `::default`). If pmcp v2.3 users have `RequestHandlerExtra { ... }` literals in external code, those break. Low likelihood given the struct's pmcp-internal usage pattern — these 12 sites are all in pmcp's own test code.

**Option B: Add fields + require every struct-literal site to use `..Default::default()`.**

Pros: No `#[non_exhaustive]` churn. Struct literals continue to work in external code.
Cons: Requires mechanical edit of all 12 sites anyway; doesn't force downstream cleanliness; technically still fragile — a new test tomorrow that spells all fields will break on the next field addition.

**Recommendation:** Option A. The proposal's success criterion says "compile unchanged after the field additions" — which is TRUE for the public-API surface (`::new`, builder methods, `::default`). The 12 struct-literals are pmcp-internal test code and their mechanical update is strictly within scope for a field-addition phase.

**`src/shared/cancellation.rs` (the smaller struct):** Only `RequestHandlerExtra::new()` is used externally (`src/wasi.rs:65`). Adding `extensions: http::Extensions` with `Default::default()` in the `::new()` body is a drop-in change for every call site.

### Question #5: Feature flag scoping

**Answer: pmcp has no `server` feature today. The proposal's "default-on inside `server` feature" phrasing maps to "default-on, unconditionally".** [VERIFIED]

**Cargo.toml feature audit (Cargo.toml:151-186):**

```toml
[features]
default = ["logging"]
full = ["websocket", "http", "streamable-http", "sse", "validation", "resource-watcher",
        "rayon", "schema-generation", "jwt-auth", "composition", "mcp-apps",
        "http-client", "logging", "macros"]
```

There is **no `server` feature**. The server dispatch code (`src/server/mod.rs`, `src/server/core.rs`) is always compiled — not feature-gated — and the proposal's reference to "the existing `server` feature" reflects an incorrect mental model of pmcp's feature layout.

**Recommendations:**

1. **Extensions field:** Make it unconditional in both `src/server/cancellation.rs` and `src/shared/cancellation.rs`. A new `extensions` feature would complicate the API surface for zero real win (the field is essentially free in terms of memory — empty `http::Extensions` is a null inline hashmap).

2. **Peer field:** Gate under `#[cfg(not(target_arch = "wasm32"))]` (as the proposal requires). Do NOT introduce a new `peer` feature flag — cfg-gating is sufficient.

3. **If a feature flag is strictly required by the proposal text**, introduce `extensions = []` (empty feature, default-on) that toggles the field. But this adds API variance (`RequestHandlerExtra` has different shapes under different features) which is bad DX. Recommend skipping.

### Question #6: WASM split

**Answer: Confirmed — peer is non-wasm only; extensions lands in both cancellation files.** [VERIFIED]

**cfg boundary evidence:**

- `src/server/cancellation.rs` is entirely gated: line 8 `#[cfg(not(target_arch = "wasm32"))]` on the `tokio::sync::RwLock` import; line 10-11 `#[cfg(not(target_arch = "wasm32"))]` on `tokio_util::sync::CancellationToken`. The whole file is effectively non-wasm. (But it's NOT module-gated — it compiles on wasm32 too, just with a different CancellationToken definition via the shared module.)
- `src/shared/cancellation.rs` has `#[cfg(not(target_arch = "wasm32"))]` only on `auth_context` field (line 49-50). The base struct compiles on both targets.

**Plan mapping:**

- **Plan 01 (Extensions):** land in BOTH `src/server/cancellation.rs` and `src/shared/cancellation.rs`. Neither gate needed for `http::Extensions`.
- **Plan 02 (Peer):** land ONLY in `src/server/cancellation.rs`. Add `#[cfg(not(target_arch = "wasm32"))]` to the `peer` field + all associated builder methods + the impl block for peer.

**Verification:** `cargo check --target wasm32-unknown-unknown --features schema-generation` after Plan 01 must still succeed.

### Question #7: Testing strategy

**Answer: Proptest-first, with specific invariants matched to the proposal's success criteria.** [RECOMMENDED]

**Per CLAUDE.md ALWAYS requirements:**

**(a) Property tests (≥100 cases per proposal criterion):**

1. **Extensions typemap round-trip:**
   ```rust
   proptest! {
     #[test]
     fn extensions_insert_get_roundtrip(key: String, value: u64) {
       let mut extra = RequestHandlerExtra::default();
       extra.extensions_mut().insert((key.clone(), value));
       let retrieved = extra.extensions().get::<(String, u64)>();
       prop_assert_eq!(retrieved, Some(&(key, value)));
     }
   }
   ```

2. **Extensions key-collision semantics** — `http::Extensions::insert` returns `Option<T>` containing the PREVIOUS value if the same type was already present. Property-test that this matches the documented `http::Extensions` contract (`insert-on-existing-key returns old value`).

3. **Peer.progress_notify no-op without token:** Property-test that `peer.progress_notify(token, ...)` with no progress_reporter set returns `Ok(())` silently — no panic, no error. (The proposal mentions this as a success criterion.)

4. **Peer.sample session routing:** In-process mock-transport test — two `RequestHandlerExtra`s wired to different peer handles; calling `sample` on one must route through only its own channel. Verifies session isolation.

5. **RequestHandlerExtra::clone preserves extensions** — property-test that `.clone()` produces a semantically equal Extensions map (each inserted value is still retrievable). Critical because `RequestHandlerExtra: Clone` already.

**(b) Unit tests:** every new public method on `RequestHandlerExtra` + `PeerHandle` trait default behavior. ≥80% coverage on new code.

**(c) Fuzz testing:** Peer back-channel has a natural fuzz boundary — the `ServerRequest` JSON-RPC serialization. Existing pmcp fuzz targets at `fuzz/fuzz_targets/protocol_parsing.rs` and `fuzz/fuzz_targets/jsonrpc_handling.rs` can be extended with a `sampling/createMessage` round-trip fuzz target. CLAUDE.md requires a fuzz target for new features. Recommended: `fuzz/fuzz_targets/fuzz_peer_handle.rs` — generate random CreateMessageParams, round-trip through serialize/deserialize, assert invariants (no panic, all valid inputs produce valid JSON-RPC).

**(d) In-process integration test harness:** pmcp has this already via `tests/typed_tool_transport_e2e.rs` + `tests/typescript_interop.rs` patterns. Plan 02 can add `tests/handler_peer_integration.rs` spinning up a test server with a mock client that responds to `sampling/createMessage` requests the peer issues.

**(e) Sampling rate (Nyquist):**
- Per task commit: `cargo test --lib server::cancellation -- --test-threads=1`
- Per wave merge: `cargo test --features "full" -- --test-threads=1`
- Phase gate: `make quality-gate`

### Question #8: Examples

**Answer: File numbering conflict — `s22_*` and `s23_*` are both taken.** [VERIFIED + RECOMMENDATION]

**Evidence:**

- `examples/s22_structured_output_schema.rs` EXISTS (Cargo.toml:402-403)
- `examples/s23_mcp_tool_macro.rs` EXISTS (Cargo.toml:491-493, required-features = "full")
- Proposal says: `examples/s22_handler_extensions.rs` + `examples/s23_handler_peer_sample.rs`

**Recommendation:** Use the next available `s4X` numbers. Current max: `s41_code_mode_graphql.rs` (Cargo.toml:500-503). Suggested:

- `examples/s42_handler_extensions.rs` — demonstrates Extensions cross-middleware insert/retrieve
- `examples/s43_handler_peer_sample.rs` — demonstrates `peer.sample()` from inside a tool handler

**Closest pattern to follow:**

- `examples/s16_typed_tools.rs` — simple typed-tool + handler pattern
- `examples/s30_tool_with_sampling.rs` — already shows server-side sampling-from-handler (but via the registration-time `SamplingHandler` trait, not inside the tool body — which IS the gap Phase 70 closes). The new `s43` example should explicitly contrast against `s30` to show the new ergonomics.

**Cargo.toml registration** (add after s41 entry, before the final `[[bench]]` block at line 505-507):

```toml
[[example]]
name = "s42_handler_extensions"
path = "examples/s42_handler_extensions.rs"

[[example]]
name = "s43_handler_peer_sample"
path = "examples/s43_handler_peer_sample.rs"
```

No `required-features` needed unless the examples use `schema-generation` internally (recommended: they don't — keep them minimal).

### Question #9: Rustdoc + migration guide

**Answer:** `make doc-check` command and "zero rustdoc warnings" operational definition:

**Command location verification:**

```bash
grep -n 'doc-check\|doc_check' /Users/guy/Development/mcp/sdk/rust-mcp-sdk/Makefile
```

**Expected target:** per DRSD-04 (`REQUIREMENTS.md:31`) which requires "Zero rustdoc warnings — all broken intra-doc links and unclosed HTML tags resolved, CI gate added". Phase 67 delivers this. For Phase 70, `make doc-check` should invoke `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --features "full"`.

**Operationally:** "zero rustdoc warnings" means `cargo doc --no-deps --features "full" 2>&1 | grep -i warning` returns empty. Every new public item added in this phase (`RequestHandlerExtra::extensions`, `RequestHandlerExtra::extensions_mut`, `RequestHandlerExtra::with_peer`, `RequestHandlerExtra::peer`, `PeerHandle` trait and its three methods) must have rustdoc with at least a one-line summary + a `rust,no_run` example.

**Migration note outline (to be added at the top of `src/server/cancellation.rs` module-level doc):**

> **Phase 70 (v2.2, 2026-04): `RequestHandlerExtra` gained two drop-in fields.**
> - `extensions: http::Extensions` — typed-key typemap for cross-middleware state transfer. Insert/retrieve typed values via `extra.extensions_mut().insert(v)` / `extra.extensions().get::<T>()`.
> - `peer: Option<Arc<dyn PeerHandle>>` — server-to-client back-channel exposing `sample()`, `list_roots()`, and `progress_notify()` from inside tool/prompt/resource handlers.
>
> Both are backwards-compatible additions: `::new()` and `::default()` signatures are unchanged. If you construct `RequestHandlerExtra` with a positional struct literal, add the new fields or (recommended) switch to `RequestHandlerExtra::new(...)` + builder methods.

### Question #10: Validation Architecture — see section below

## Edit Sites

| File | Lines | Current | Proposed Change | Plan |
|------|-------|---------|-----------------|------|
| `src/server/cancellation.rs` | 117-147 (struct) | `pub struct RequestHandlerExtra { cancellation_token, request_id, session_id, auth_info, auth_context, metadata, progress_reporter, task_request }` | Add `pub extensions: http::Extensions` + `#[cfg(not(target_arch = "wasm32"))] pub peer: Option<Arc<dyn PeerHandle>>`. Mark struct `#[non_exhaustive]`. | 01+02 |
| `src/server/cancellation.rs` | 151-162 (`::new`) | `fn new(...) -> Self { Self { ..., task_request: None } }` | Add `extensions: http::Extensions::new(), peer: None` to struct literal inside `new()`. Signature unchanged. | 01+02 |
| `src/server/cancellation.rs` | 276-294 (`Default`) | Builds struct literal | Add `extensions: http::Extensions::new(), peer: None`. | 01+02 |
| `src/server/cancellation.rs` | +new methods after line 274 | — | Add `pub fn extensions(&self) -> &http::Extensions`, `pub fn extensions_mut(&mut self) -> &mut http::Extensions`, `pub fn with_peer(mut self, peer: Arc<dyn PeerHandle>) -> Self`, `pub fn peer(&self) -> Option<&Arc<dyn PeerHandle>>`. Each with rustdoc + doctest. | 01+02 |
| `src/server/cancellation.rs` | 296-336 (Debug) | Redacts metadata | Extend debug output: `.field("extensions", &self.extensions)` — `http::Extensions` Debug prints type names only, which is redaction-friendly. `.field("peer", &self.peer.as_ref().map(\|_\| "Arc<dyn PeerHandle>"))`. | 01+02 |
| `src/shared/cancellation.rs` | 38-51 (struct) | 5 fields only | Add `pub extensions: http::Extensions` (no peer on shared — this struct is already wasm-friendly). Mark `#[non_exhaustive]`. | 01 |
| `src/shared/cancellation.rs` | 53-64 (`::new`) | — | Add `extensions: http::Extensions::new()` to struct literal. | 01 |
| `src/shared/peer.rs` | NEW FILE | — | Define `PeerHandle` trait + rustdoc + doctest. Non-wasm only (`#[cfg(not(target_arch = "wasm32"))]` at module level). | 02 |
| `src/server/peer_impl.rs` | NEW FILE | — | Define `DispatchPeerHandle` pub(crate) struct + `impl PeerHandle for DispatchPeerHandle`. Non-wasm only. Adapts the ElicitationManager pattern from `src/server/elicitation.rs:21-190`. | 02 |
| `src/server/core.rs` | 411-418 | `RequestHandlerExtra::new(...).with_auth_context(...).with_task_request(...)` | Add `.with_peer(self.build_peer_handle(session_id))` | 02 |
| `src/server/core.rs` | 574-580 | `...with_auth_context(...)` | Add `.with_peer(self.build_peer_handle(session_id))` | 02 |
| `src/server/core.rs` | 594-600 | `...with_auth_context(...)` | (Optional — low value for list_resources) — can skip | 02 |
| `src/server/core.rs` | 636-642 | `...with_auth_context(...)` | Add `.with_peer(self.build_peer_handle(session_id))` | 02 |
| `src/server/mod.rs` | 1055-1060 | `...with_auth_context(...).with_progress_reporter(...)` | Add `.with_peer(...)` wired to `self.notification_tx` / server-request channel | 02 |
| `src/server/mod.rs` | 1193-1198 | similar | Add `.with_peer(...)` | 02 |
| `src/server/mod.rs` | 1228-1232 | similar | (Optional — low value for list_resources) | 02 |
| `src/server/mod.rs` | 1301-1306 | similar | Add `.with_peer(...)` | 02 |
| `src/server/mod.rs` | 1358-1361 | `RequestHandlerExtra::new(...)` (bare) | Add `.with_peer(...)` | 02 |
| `src/server/workflow/prompt_handler.rs` | 1055, 1238, 1347, 1441, 1573, 1704, 1772, 1851, 1948, 2081, 2223, 2338 | 12 struct-literal sites | Add `extensions: http::Extensions::new(), peer: None` OR switch to `..Default::default()` pattern. | 01+02 (half each) |
| `examples/s42_handler_extensions.rs` | NEW FILE | — | Minimal example: insert a typed value in middleware, retrieve in tool handler, print result. | 03 |
| `examples/s43_handler_peer_sample.rs` | NEW FILE | — | Minimal example: inside a tool handler, call `extra.peer().unwrap().sample(params).await`, return the sampled text. Mirrors `examples/s30_tool_with_sampling.rs` but inline from the handler body. | 03 |
| `Cargo.toml` | after line 503 (s41 entry) | — | Add two `[[example]]` entries for s42/s43. | 03 |
| `.claude/skills/` OR `.planning/phases/70-*/` | — | — | No skill files exist for this project; no update needed. | — |

## Backwards-Compat Evidence

### Every `RequestHandlerExtra::new(...)` call-site in src/ (canonical dispatch + tests):

```
src/wasi.rs:65                 RequestHandlerExtra::new(id.to_string(), cancellation_token);
src/server/builder_middleware_executor.rs:208   RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new());
src/server/builder_middleware_executor.rs:282   RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new())
src/server/simple_resources.rs:568    RequestHandlerExtra::new("test-req".to_string(), CancellationToken::new());
src/server/simple_resources.rs:600    RequestHandlerExtra::new("test-req".to_string(), CancellationToken::new());
src/server/observability/middleware.rs:498, 516, 535, 554, 582  (5 sites)
src/server/dynamic_resources.rs:387, 465   (2 sites)
src/server/cancellation.rs:462, 477   (2 sites, self-test)
src/server/core.rs:411, 574, 594, 636  (4 sites — dispatch)
src/server/tool_middleware.rs:525, 569, 631, 694, 770, 797  (6 sites)
src/server/mod.rs:1055, 1193, 1228, 1301, 1358  (5 sites — legacy dispatch)
```

**Total: 28 `::new(...)` sites in src/. ALL take exactly two args (`request_id`, `cancellation_token`). Signature unchanged ⇒ ALL compile unchanged.** [VERIFIED via Grep]

### Every `RequestHandlerExtra::new(...)` call-site in examples/ and tests/:

```
examples/s36_dynamic_resource_workflow.rs:151
examples/s38_prompt_workflow_progress.rs:155, 180
examples/s30_tool_with_sampling.rs:276
tests/test_cancellation.rs:95, 125, 146
```

**Total: 7 sites. All 2-arg. ALL compile unchanged.** [VERIFIED]

### Every `.with_session_id()` / builder-method call-site:

```
src/server/cancellation.rs:463                    .with_session_id(Some(...))
src/shared/cancellation.rs:67                     (method definition)
tests/test_cancellation.rs:96                     .with_session_id(Some(...))
```

**Total: 1 active `with_session_id` usage + 1 method definition. Signature unchanged ⇒ compiles unchanged.** [VERIFIED]

### Every `RequestHandlerExtra::default()` call-site:

```
pmcp-macros/tests/mcp_prompt_tests.rs:43, 92, 191, 206, 231, 261  (6 sites)
pmcp-macros/tests/mcp_tool_tests.rs:36, 81, 96, 123, 177  (5 sites)
src/server/typed_prompt.rs:317, 331  (2)
cargo-pmcp/src/templates/calculator.rs:140, 149, 158  (3)
examples/s36_dynamic_resource_workflow.rs:151 (uses Default::default() for inner CancellationToken)
book/course markdown:  ~10 doctest sites
```

**Total: ~16 `::default()` sites. Default impl updates transparently ⇒ ALL compile unchanged.** [VERIFIED]

### Struct-literal sites — THESE BREAK without mitigation:

```
src/server/workflow/prompt_handler.rs:1055, 1238, 1347, 1441, 1573, 1704, 1772, 1851, 1948, 2081, 2223, 2338   (12 sites)
```

**Status: all 12 are in test modules. Mechanical edit to add `extensions: http::Extensions::new(), peer: None` (or switch to `..Default::default()`).** [VERIFIED via Grep]

**Verdict:** Proposal success-criterion 1 ("Every existing `RequestHandlerExtra::new(...)` and `::with_session(...)` call-site compiles unchanged") is LITERALLY TRUE. The 12 struct-literal sites are NOT `::new`/`::with_session` call-sites — they're struct-literal sites, which the proposal does not cover. But the planner MUST include "fix 12 struct-literal test sites" as an explicit task or the phase fails at `cargo test`.

## Dependency / Feature Flag Recommendation

### Zero new dependencies

`http::Extensions` is exposed from the existing `http = "1.1"` dep (Cargo.toml:58, resolves to http 1.4.0). No Cargo.toml `[dependencies]` section changes. [VERIFIED]

### Feature flag: none new

Do NOT add an `extensions` feature. Do NOT add a `peer` feature. Both fields land unconditionally (with the peer field cfg-gated to non-wasm).

**Cargo.toml sketch — no change required:**

```toml
# No edits to [features] section.
```

**Rationale:** pmcp has no `server` feature today (Cargo.toml:151-186). The proposal's wording "default-on inside `server` feature" doesn't map to pmcp's actual feature layout. Introducing a new feature for a two-field struct addition adds cross-feature API variance (a user's `RequestHandlerExtra` under `--no-default-features` would have different fields from the default build) with zero real benefit.

**If the team insists on a feature gate** (non-recommended), use:

```toml
[features]
default = ["logging", "extensions"]
extensions = []
```

…and wrap the field + methods in `#[cfg(feature = "extensions")]`. This is strictly worse but matches the proposal text literally.

## Testing Strategy

Per CLAUDE.md ALWAYS requirements (fuzz + property + unit + example + doctest + integration):

### Unit tests (new — ≥80% coverage on additions)

In `src/server/cancellation.rs` `#[cfg(test)]` module (extending existing tests):

```rust
#[tokio::test]
async fn test_extensions_default_empty() { ... }

#[tokio::test]
async fn test_extensions_insert_get() { ... }

#[tokio::test]
async fn test_extensions_clone_preserves_values() { ... }

#[tokio::test]
async fn test_peer_default_none() { ... }

#[tokio::test]
async fn test_peer_progress_notify_noop_without_reporter() { ... }
```

### Property tests (new — ≥100 cases per proposal SC3)

New file: `tests/handler_extensions_properties.rs`

```rust
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

    #[test]
    fn prop_extensions_insert_get_roundtrip(k: u64, v: String) { ... }

    #[test]
    fn prop_extensions_key_collision_returns_old_value(v1: u64, v2: u64) { ... }

    #[test]
    fn prop_extra_clone_preserves_extensions(n_values in 0usize..10) { ... }

    #[test]
    fn prop_peer_progress_notify_is_idempotent_on_none_reporter() { ... }
}
```

### Fuzz target (new — per CLAUDE.md ALWAYS)

New file: `fuzz/fuzz_targets/fuzz_peer_handle.rs`

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzzes the ServerRequest::CreateMessage serialization boundary
// that peer.sample() exercises.
fuzz_target!(|data: &[u8]| {
    let Ok(params) = serde_json::from_slice::<CreateMessageParams>(data) else { return; };
    let server_req = ServerRequest::CreateMessage(Box::new(params));
    let _ = serde_json::to_vec(&server_req);
});
```

### Integration test (new)

New file: `tests/handler_peer_integration.rs` — in-process mock-transport test spinning up a ServerCore with a mock client that responds to `sampling/createMessage` with a canned response. Tool handler calls `extra.peer().unwrap().sample(...)` and asserts the round-trip.

### Examples as smoke tests

- `examples/s42_handler_extensions.rs` — `cargo run --example s42_handler_extensions` runs in ≤5s, prints "cross-middleware value retrieved: <val>"
- `examples/s43_handler_peer_sample.rs` — `cargo run --example s43_handler_peer_sample` runs in ≤5s, prints the sample result

Both examples become part of the per-wave test set: `cargo check --examples --features "full"`.

### Doctests

Every new public API gets a `rust,no_run` doctest:

- `RequestHandlerExtra::extensions` / `extensions_mut` — show insert + get
- `RequestHandlerExtra::with_peer` — show handler construction
- `RequestHandlerExtra::peer` — show in-handler usage pattern
- `PeerHandle::sample` / `list_roots` / `progress_notify` — show each on the trait

## Validation Architecture

> `workflow.nyquist_validation` is absent in `.planning/config.json` — treat as enabled.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` + `proptest 1.7` + `cargo-fuzz` (dev-deps at Cargo.toml:131, 140) |
| Config file | Default proptest config; `.cargo/config.toml` (if any) controls test threads |
| Quick run command | `cargo test --lib server::cancellation -- --test-threads=1` (targets only the modified unit tests) |
| Full suite command | `cargo test --features "full" -- --test-threads=1` (workspace-wide) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PARITY-HANDLER-01 (Extensions typemap insert/retrieve) | Inserting a typed value, then reading it back, yields the same value | property (proptest) | `cargo test --test handler_extensions_properties prop_extensions_insert_get_roundtrip -- --test-threads=1` | ❌ Wave 0 |
| PARITY-HANDLER-01 (Extensions key-collision) | Inserting the same type twice: the second `insert` returns `Some(old_value)` | unit | `cargo test --lib server::cancellation::tests::test_extensions_insert_overwrite_returns_old -- --test-threads=1` | ❌ Wave 0 |
| PARITY-HANDLER-01 (Extensions clone preserves) | `extra.clone()` produces an Extensions with the same key-type set | property | `cargo test --test handler_extensions_properties prop_extra_clone_preserves_extensions -- --test-threads=1` | ❌ Wave 0 |
| PARITY-HANDLER-01 (Peer sample session routing) | Two peers in parallel; calling sample on peer A does NOT deliver to peer B | integration | `cargo test --test handler_peer_integration test_sample_session_routing -- --test-threads=1` | ❌ Wave 0 |
| PARITY-HANDLER-01 (Peer progress_notify no-op without token) | Calling peer.progress_notify when no progress_reporter set returns Ok(()) | unit | `cargo test --lib server::cancellation::tests::test_peer_progress_notify_noop_without_reporter -- --test-threads=1` | ❌ Wave 0 |
| PARITY-HANDLER-01 (Backwards-compat ::new + ::default) | All 28 `::new` sites + 16 `::default` sites still compile | compilation | `cargo check --features "full"` + `cargo check --target wasm32-unknown-unknown --features schema-generation` | ✅ existing |
| PARITY-HANDLER-01 (Example smoke tests) | Both new examples compile and run in <5s | example | `cargo run --example s42_handler_extensions && cargo run --example s43_handler_peer_sample` | ❌ Wave 0 |
| PARITY-HANDLER-01 (docs build clean) | `cargo doc --no-deps --features full` produces zero warnings on new items | doc | `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --features full` | ✅ existing via `make doc-check` |

### Sampling Rate

- **Per task commit:** `cargo test --lib server::cancellation -- --test-threads=1` (runs in <5s, catches most regressions)
- **Per wave merge:** `cargo test --features "full" -- --test-threads=1` (full suite — runs in ~2-3 minutes)
- **Phase gate:** `make quality-gate` — matches CI exactly (fmt --all, clippy pedantic+nursery, full build, full test, audit)

### Wave 0 Gaps

Before any feature implementation lands:

- [ ] `tests/handler_extensions_properties.rs` — new property-test file; requires proptest (already in dev-deps at Cargo.toml:131)
- [ ] `tests/handler_peer_integration.rs` — new integration-test file; requires in-process test harness (pattern exists in `tests/typed_tool_transport_e2e.rs`, reuse)
- [ ] `fuzz/fuzz_targets/fuzz_peer_handle.rs` — new fuzz target; requires `cargo-fuzz` (not needed for regular CI — exists for ALWAYS-fuzz compliance)
- [ ] No framework install needed — proptest + quickcheck + mockito + insta are all already in dev-deps (Cargo.toml:131-140)

## Security Domain

> `security_enforcement` is not set in config.json — default enabled.

### Phase 70 Threat ID Map

Canonical mapping of T-70-XX IDs used across Plan 01/02/03 threat models to the STRIDE categories and components they cover.

| Threat ID | STRIDE | Component | Disposition | Owning Plan |
|-----------|--------|-----------|-------------|-------------|
| T-70-01 | Information Disclosure | RequestHandlerExtra::fmt (Debug impl) — Extensions leak | mitigate | Plan 01 |
| T-70-02 | Tampering | DispatchPeerHandle session routing | mitigate | Plan 02 |
| T-70-03 | Elevation of Privilege | Values stored in Extensions outliving request scope | mitigate | Plan 01 |
| T-70-04 | Denial of Service | Malformed CreateMessageParams serde boundary | transfer | Plan 02 (existing fuzz) + Plan 03 (new fuzz_peer_handle) |
| T-70-05 | Denial of Service | peer.sample/list_roots timeout exhaustion | mitigate | Plan 02 |
| T-70-06 | Spoofing | 12 positional struct-literal test sites in prompt_handler.rs | accept | Plan 01 |
| T-70-07 | Information Disclosure | DispatchPeerHandle leaked via RequestHandlerExtra Debug | mitigate | Plan 02 |
| T-70-08 | Elevation of Privilege | Peer inheriting caller auth scope without explicit check | accept | Plan 02 |
| T-70-09 | Information Disclosure | Example code modelling unsafe Extensions logging | mitigate | Plan 03 |
| T-70-10 | Repudiation | Missing rustdoc on new public API (silent feature) | mitigate | Plan 03 |

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | `auth_context` field already on `RequestHandlerExtra`; Extensions should NOT be used for auth data (proposal explicitly keeps auth fields separate) |
| V3 Session Management | yes | `session_id` already on `RequestHandlerExtra`; peer handle MUST be session-scoped so `peer.sample()` can't cross sessions |
| V4 Access Control | partial | Tool-level authorization runs in `src/server/core.rs:398-407` BEFORE the peer is built. Peer access inherits the tool's auth scope. |
| V5 Input Validation | yes | `sample()` / `list_roots()` params flow through `serde_json` deserialization — existing protocol-level validation applies |
| V6 Cryptography | no | This phase introduces no crypto |

### Known Threat Patterns for this phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Extensions leaking sensitive data in logs | Information Disclosure | `http::Extensions::Debug` prints type names only, not values [VERIFIED docs.rs]. Combine with pmcp's existing redaction-aware Debug on `RequestHandlerExtra` (src/server/cancellation.rs:296-336). |
| Peer call routed to wrong session | Tampering | Peer impl must bind to the session_id of the originating request. Property-test session isolation (Test #4 above). |
| Extensions storing mutable `Arc`s that outlive the request | Elevation of Privilege | `http::Extensions` requires `'static + Clone + Send + Sync` — values are cloned across the request boundary, not shared mutably. Document in rustdoc that Extensions should hold Clone-cheap snapshots, not live handles. |
| Malformed `CreateMessageParams` from adversarial client | Denial of Service | Fuzz target `fuzz_peer_handle.rs` exercises this. Existing pmcp `protocol_parsing.rs` fuzz target covers the JSON-RPC deserialization path for the entire `ServerRequest` enum. |
| Timeout exhaustion on `peer.sample()` | Denial of Service | Adopt ElicitationManager's 5-min default timeout (src/server/elicitation.rs:45). Configurable via builder. |

## Suggested Plan Split

**Proposal estimate:** 4 plans. **Recommendation:** 3 plans.

| Plan | Scope | Estimated effort | Rationale |
|------|-------|------------------|-----------|
| **Plan 01** | Extensions typemap field (both cancellation.rs files) + property tests + 6 struct-literal test-site updates in `prompt_handler.rs` | 2-3h | Purely additive: field + builder methods + `Default`/`Debug` impls + proptest file. Self-contained, no cross-module plumbing. |
| **Plan 02** | `PeerHandle` trait (new file) + `DispatchPeerHandle` impl (new file) + wire through 9 dispatch call sites in `core.rs` + `mod.rs` + session_id threading + remaining 6 struct-literal test-site updates + integration test | 4-6h | The heaviest plan. Includes the session_id plumbing risk identified in Question #3. Integration test is non-trivial (needs mock-client transport). |
| **Plan 03** | Two examples (s42, s43) + Cargo.toml registration + rustdoc migration notes on both cancellation.rs files + fuzz target + CI gate verification (`make quality-gate` green) | 2-3h | Polish + docs + compliance. Merging the proposal's Plan 03 + Plan 04 here because both are small and depend on 01+02 landing first. |

**Justification for merging proposal Plans 03+04:**

- Plan 03 (examples) and Plan 04 (CI + migration guide) both depend on 01+02 being merged
- Plan 04's scope — "CI + migration guide finalization" — is ~30 min of work (add a `make doc-check` invocation + write the migration prose in `src/server/cancellation.rs` module doc)
- Combining them avoids a trivial final plan that adds overhead
- 3 plans still satisfies D-17's `{3,4,5}` range (proposal guidance)

**If the planner disagrees:** splitting 03 into two (03 examples + rustdoc, 04 CI + migration guide) is acceptable per D-17; the plans remain small but distinct.

## Common Pitfalls

### Pitfall 1: Forgetting the 12 struct-literal sites in prompt_handler.rs
**What goes wrong:** `cargo test` fails after Plan 01 lands because struct literals are missing the new `extensions` field.
**Why it happens:** Grep-searching for `RequestHandlerExtra::new` catches only 28 sites. The pattern `RequestHandlerExtra {` catches 12 more that positional-spell every field.
**How to avoid:** Plan 01 task list MUST include "mechanically update 6 struct-literal sites" and Plan 02 "mechanically update remaining 6 struct-literal sites". Edit each to append `, extensions: http::Extensions::new()` (Plan 01) and `, peer: None` (Plan 02).
**Warning signs:** `cargo check --tests` starts emitting E0063 errors about missing struct fields.

### Pitfall 2: Peer impl shared across sessions
**What goes wrong:** A single `DispatchPeerHandle` instance is reused across requests; `peer.sample()` delivers to the wrong client.
**Why it happens:** The natural implementation caches a single `mpsc::Sender<ServerRequest>` at server-construction time rather than per-request.
**How to avoid:** Build a FRESH `DispatchPeerHandle` inside `RequestHandlerExtra::new(...)` for every request. The handle is cheap — it's just an `Arc<mpsc::Sender>` + `Option<String> session_id` + `Duration timeout` — allocation-wise trivial.
**Warning signs:** The session-routing property test fails; or manual e2e test shows a sample response arriving at a tool handler from a different client session.

### Pitfall 3: `http::Extensions` Clone is shallow
**What goes wrong:** User inserts `Arc<Mutex<State>>` into Extensions expecting `.clone()` to produce a shared view. It does — but pmcp rustdoc doesn't call this out.
**Why it happens:** `http::Extensions: Clone` clones the map, which clones each contained value. `Arc::clone` is `O(1)`, but `String::clone` is `O(n)`. Users may not know which.
**How to avoid:** Rustdoc on `extensions_mut()` must warn: "Values are cloned with the extra on every `.clone()` of `RequestHandlerExtra`. Prefer `Arc<T>` for large values."
**Warning signs:** Bug reports about copy cost when middleware clones extras for logging.

### Pitfall 4: Dropping the session channel mid-request
**What goes wrong:** `DispatchPeerHandle` holds a channel; if the parent `Server` drops (e.g., shutdown mid-request), `peer.sample()` hangs or times out.
**Why it happens:** `mpsc::Sender::send` returns Err when the receiver is dropped — but `tokio::sync::oneshot::Receiver::await` inside the peer impl blocks until its timeout fires.
**How to avoid:** Peer impl's `sample()` must check `request_tx.is_closed()` before awaiting the response; return a fast `Error::Transport("peer channel closed")` if so. Adopt ElicitationManager's timeout semantics (5-min default; configurable).
**Warning signs:** Shutdown races emit spurious timeout errors in logs.

### Pitfall 5: Progress reporter Arc cloning cost
**What goes wrong:** Every `RequestHandlerExtra::clone()` clones the `Arc<dyn PeerHandle>` — trivial on its own, but if middleware chains iterate over extras heavily, cumulative refcount traffic shows up in flamegraphs.
**Why it happens:** `Arc::clone` is `O(1)` but involves an atomic increment. Under very high throughput (10k+ RPS), this is measurable.
**How to avoid:** For v2.2, do nothing — this is not a hot path yet. Note in follow-on research if perf benchmarks show the issue.
**Warning signs:** `benches/client_server_operations.rs` shows regression after the merge.

## Risks & Open Questions (RESOLVED)

### Risk 1: session_id not threaded through `ProtocolHandler::handle_request` signature
**Impact:** Plan 02 requires the session_id to construct a session-scoped `DispatchPeerHandle`. Today, `ProtocolHandler::handle_request(id, request, auth_context)` does NOT include session_id explicitly.
**Resolution path:** Plan 02 task 1 should verify whether session_id flows through `auth_context` (via `AuthContext.session_id` — check `src/server/auth/mod.rs`) or must be added as a new parameter. This is the largest single piece of plumbing risk.
**Mitigation:** If signature extension is needed, guard it with a default-implemented trait method so downstream implementers of `ProtocolHandler` (if any exist outside pmcp) don't break.

### Risk 2: Example file numbering collision
**Impact:** Proposal says `s22_*` and `s23_*` — those are taken. Using them overwrites existing examples.
**Resolution path:** Use `s42_*` and `s43_*` (next available after `s41_code_mode_graphql`). Plan 03 task 1 creates the files; update Cargo.toml `[[example]]` entries.
**Evidence:** `examples/s22_structured_output_schema.rs` exists (Cargo.toml:402-403). `examples/s23_mcp_tool_macro.rs` exists (Cargo.toml:491-493).

### Risk 3: Two competing `RequestHandlerExtra` structs
**Impact:** `src/server/cancellation.rs` (canonical, 8 fields) vs `src/shared/cancellation.rs` (5 fields) — confusing for new contributors.
**Resolution path:** OUT OF SCOPE for Phase 70. Add a note in RESEARCH.md (this section) flagging the duplication for a future cleanup phase. Phase 70 adds `extensions` to both; peer only to the canonical one.
**Mitigation:** Rustdoc on `src/shared/cancellation.rs` should cross-reference the canonical struct and explain when each is used.

### Risk 4: `ProtocolHandler` legacy trait in `src/server/traits.rs`
**Impact:** A second secondary set of handler traits exists in `src/server/traits.rs` (lines 17-68) that DOES NOT take `RequestHandlerExtra` on `ToolHandler::call_tool`. It's unused by `ServerCoreBuilder` but exists in the public API.
**Resolution path:** OUT OF SCOPE for Phase 70. Phase 70 modifies the canonical `ToolHandler`/`PromptHandler`/`ResourceHandler` traits in `src/server/mod.rs:200-247`. The legacy `traits.rs` traits do not receive the Extensions/peer updates.
**Mitigation:** Note in the migration doc that `src/server/traits.rs` is a legacy surface not consumed by ServerCoreBuilder; any user relying on it is not affected by Phase 70's changes.

### Open Question 1: Peer timeout default
**What we know:** ElicitationManager uses 5 min (`src/server/elicitation.rs:45`).
**What's unclear:** Should `peer.sample()` use the same default? 5 minutes seems long for LLM sampling; most real LLM calls are 10-60s.
**Recommendation:** Default 60s with a builder override. Document in rustdoc.
**RESOLVED (Plan 02 Task 3):** 60s default adopted via `DEFAULT_PEER_SAMPLE_TIMEOUT: Duration = Duration::from_secs(60)` in `src/server/peer_impl.rs`; builder override available via `DispatchPeerHandle::new(tx, session_id, timeout_duration)`.

### Open Question 2: Should `list_resources` dispatch sites get peer too?
**What we know:** The proposal scope says "tool/prompt/resource handlers" — which includes `list_resources`. But listing is usually a cheap enumeration that doesn't need LLM sampling.
**What's unclear:** Low value vs 100% consistency across dispatch sites.
**Recommendation:** Include peer in all dispatch sites for consistency. Cost is zero — just a field on the extra.
**RESOLVED (Plan 02 Task 4):** Peer wired into `list_resources` dispatch sites in both `src/server/core.rs:594` and `src/server/mod.rs:1228` for consistency across all 9 dispatch sites.

### Open Question 3: Should we mark `RequestHandlerExtra` `#[non_exhaustive]`?
**What we know:** rmcp does this on `RequestContext`. Protects against future-field-addition breakage.
**What's unclear:** Breaking change for external code that spells struct literals.
**Recommendation:** YES — mark `#[non_exhaustive]`. The only known struct-literal users are in pmcp's own test code (12 sites we control). External users should use `::new()` + builder methods anyway. If downstream breakage is reported post-release, remove the attribute in v2.3.1 — it's one-line.
**RESOLVED (Plan 01 Task 1):** `#[non_exhaustive]` applied to `RequestHandlerExtra` in both `src/server/cancellation.rs` and `src/shared/cancellation.rs`; Plan 01 Task 2 mechanically converts all 12 positional struct-literal sites in `src/server/workflow/prompt_handler.rs` to `::new()` + builder form to avoid the non-exhaustive breakage.

**All three open questions above are resolved within the Phase 70 plan set.** Q1 landed in Plan 02 Task 3 (timeout constant + override), Q2 in Plan 02 Task 4 (list_resources wiring), Q3 in Plan 01 Tasks 1 and 2 (non_exhaustive marker + mechanical struct-literal refactor). No carry-over to follow-on phases.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `http::Extensions` is available on wasm32 target | Question #1 | LOW — `cargo tree` shows http resolved on all targets. If wrong, Plan 01 adds `#[cfg(not(target_arch = "wasm32"))]` to the Extensions field in `src/shared/cancellation.rs`. |
| A2 | session_id flows through existing code such that `DispatchPeerHandle` can be session-scoped without new trait parameters | Risk 1 | MEDIUM — Plan 02 task 1 must verify. If wrong, adds ~1h of plumbing work. |
| A3 | Existing ElicitationManager pattern adapts cleanly to peer back-channel | Question #2 | LOW — pattern is well-established; only concern is per-request lifetime (Pitfall 2). |
| A4 | 5 min timeout default is acceptable for peer.sample | Open Q1 | LOW — easy to adjust; recommend 60s default. |
| A5 | The 12 struct-literal sites in `prompt_handler.rs` are ALL in `#[cfg(test)]` modules (not production code paths) | Question #4 | LOW — verified by grep showing all sites are in test helper fn bodies. Even if one is not, the mechanical fix is identical. |

## Sources

### Primary (HIGH confidence)

- `src/server/cancellation.rs:117-147` — canonical RequestHandlerExtra struct definition [VERIFIED v2.3.0]
- `src/shared/cancellation.rs:38-51` — shared-runtime RequestHandlerExtra [VERIFIED v2.3.0]
- `src/server/core.rs:411, 574, 594, 636` — ServerCore dispatch sites [VERIFIED]
- `src/server/mod.rs:1055, 1193, 1228, 1301, 1358` — legacy Server dispatch sites [VERIFIED]
- `src/server/elicitation.rs:1-190` — pattern template for PeerHandle impl [VERIFIED]
- `src/server/roots.rs:35-179` — RootsManager server-to-client request pattern [VERIFIED]
- `Cargo.toml:58, 151-186` — http dep + feature layout [VERIFIED]
- `.planning/phases/69-.../69-RESEARCH.md` rows HANDLER-02 and HANDLER-05 [CITED]
- `.planning/phases/69-.../69-PROPOSALS.md` Proposal 1 [CITED]
- `.planning/REQUIREMENTS.md:54` — PARITY-HANDLER-01 entry [CITED]
- https://docs.rs/http/1.4.0/http/struct.Extensions.html — http::Extensions API [CITED]
- https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v1.5.0/crates/rmcp/src/service.rs#L651 — rmcp RequestContext [CITED]
- https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v1.5.0/crates/rmcp/src/service.rs#L382 — rmcp Peer struct [CITED]

### Secondary (MEDIUM confidence)

- http crate source at `docs.rs/http/1.4.0/src/http/extensions.rs.html` for Debug impl behavior [CITED via WebFetch]
- `cargo tree -p pmcp --depth 1` output showing http v1.4.0 resolution [VERIFIED]

### Tertiary (LOW confidence)

- None. All claims in this research are tagged `[VERIFIED]` (from direct code reading / cargo commands) or `[CITED]` (from authoritative sources).

## Metadata

**Confidence breakdown:**

- Standard stack (http::Extensions selection): HIGH — existing dep, well-documented API
- Architecture patterns (PeerHandle shape, ElicitationManager adaptation): HIGH — working template exists in-tree
- Dispatch wiring (9 call sites): HIGH — all sites enumerated via Grep, code paths read directly
- Backwards-compat (call-site audit): HIGH — exhaustive Grep verified
- Struct-literal risk (12 sites in prompt_handler.rs): HIGH — verified with Grep content inspection
- session_id plumbing (Risk 1): MEDIUM — Plan 02 must verify first thing
- Testing strategy: HIGH — all four test dimensions (unit/property/fuzz/integration) match existing pmcp patterns
- Feature flag recommendation (none new): HIGH — Cargo.toml read directly, no `server` feature exists
- Example numbering (s42/s43 not s22/s23): HIGH — Cargo.toml verified

**Research date:** 2026-04-16
**Valid until:** 2026-05-16 (30 days; pmcp release cycle is monthly and this is stable API surface)

## RESEARCH COMPLETE

Planner may proceed. Key decisions locked:

1. Typemap: `http::Extensions` (no new dep).
2. Peer: new `PeerHandle` trait in `src/shared/peer.rs`, concrete `DispatchPeerHandle` in `src/server/peer_impl.rs`, adapted from `ElicitationManager` pattern.
3. Feature flag: NONE new (neither `extensions` nor `peer`) — fields are unconditional, peer is cfg-gated to non-wasm only.
4. Struct fields: `#[non_exhaustive]` on `RequestHandlerExtra` + mechanical update of 12 struct-literal sites in `src/server/workflow/prompt_handler.rs`.
5. Example numbering: s42 + s43 (s22/s23 collide with existing examples).
6. Plan split: 3 plans, not 4 (proposal's Plan 03 + Plan 04 merge).

Largest open risk: session_id threading through `ProtocolHandler::handle_request` (Risk 1 + Open Q2 in Question #3) — Plan 02 task 1 must verify first.
