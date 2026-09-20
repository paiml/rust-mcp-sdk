# Phase 70 Review-Claim Verification

Verified at: 2026-04-17T06:20:00Z
Branch: feat/sql-code-mode
Commit: 68e1371885bf56460094608919e52d50ef5cbac9

## Codex HIGH Findings — Verification Results

### Finding 1: AuthContext lacks session_id

- **Status: CONFIRMED**
- **Evidence:** `src/server/auth/traits.rs:61-88` — the `AuthContext` struct body has fields `subject`, `scopes`, `claims`, `token`, `client_id`, `expires_at`, `authenticated` — **no `session_id` field**. The only `session_id` occurrences in the file (lines 511, 514, 517) are parameters on `SessionStore::get_session/update_session/invalidate_session` trait methods (session-store keying, not a field on AuthContext).
- **Impact on plans:**
  - **Plan 02 Task 1 Path A is materially false.** Cannot extract `session_id` via `auth_context.session_id`.
  - **Alternative found:** Both `RequestHandlerExtra` structs ALREADY carry `session_id: Option<String>` directly (`src/server/cancellation.rs` interfaces block line 92 — already documented in Plan 01's interfaces; `src/shared/cancellation.rs:45` confirmed in this verification). So session_id access expression is simply `extra.session_id.clone()` once a DispatchPeerHandle is being built from `extra` — this is Path B, not Path A.
  - **However,** at the dispatch sites in `src/server/core.rs:411-418` and `src/server/mod.rs:1055-1060`, the code constructs `RequestHandlerExtra::new(...)` WITHOUT setting `session_id` (no `.with_session_id(...)` call in scope). `grep session_id src/server/core.rs src/server/mod.rs` returns **zero matches** in either file — meaning `session_id` is never populated at dispatch time in the current code. So even though the FIELD exists on RequestHandlerExtra, it is always `None` at the 9 dispatch sites.
  - **Net:** session_id is discoverable on the struct but never populated at runtime. This is a separate plumbing gap from Codex's claim, but it still breaks Plan 02 Path A exactly as Codex predicted.

### Finding 2: No outbound Sender<ServerRequest> at dispatch sites

- **Status: CONFIRMED (HIGH severity)**
- **Evidence:**
  - `src/server/mod.rs:305` — legacy `Server` has `notification_tx: Option<mpsc::Sender<Notification>>`. That channel carries `Notification`, NOT `ServerRequest`.
  - `src/server/mod.rs:313` — legacy `Server` has `elicitation_manager: Option<Arc<elicitation::ElicitationManager>>`. The ElicitationManager OWNS its own `request_tx: Option<mpsc::Sender<ServerRequest>>` (`src/server/elicitation.rs:25`) but that sender is never wired to the transport layer — `grep set_request_channel src/server/mod.rs` returns zero matches, and `spawn_message_handler` (line 741) never drains or forwards any ServerRequest.
  - `src/server/core.rs` — `ServerCore` struct (lines 199-277) has NO `notification_tx`, NO `request_tx`, NO `elicitation_manager` field. Zero outbound-to-client channels.
  - `grep ServerRequest src/server/core.rs` returns no matches (the file doesn't even reference the ServerRequest enum).
  - `RootsManager::request_client_roots` (`src/server/roots.rs:155-166`) takes a FnOnce sender argument — it has no owned channel; the caller must supply one. This matches Codex's observation that outbound plumbing is ad-hoc per subsystem.
- **Impact:**
  - **Plan 02 Task 4 cannot wire `.with_peer(Arc::new(DispatchPeerHandle::new(request_tx, ...)))` because `request_tx` does not exist at any of the 9 dispatch sites.**
  - The legacy Server could theoretically clone `elicitation_manager.request_tx` IF the code set up a channel to drain into the transport — but that code does not exist yet. In the current code, `set_request_channel` is never called, so elicitation requests built by handlers would go nowhere even today.
  - ServerCore has no way at all to dispatch a ServerRequest to a client today. The "active dispatch path" cannot support a peer handle until that transport is built.

### Finding 3: No client-response correlation layer

- **Status: CONFIRMED (HIGH severity)**
- **Evidence:**
  - `src/server/elicitation.rs:23` — ElicitationManager has a `pending: Arc<RwLock<HashMap<String, oneshot::Sender<ElicitResult>>>>`. This IS a correlation layer, but it is **elicitation-specific** (keyed by `elicitation_id`, fulfilled by `ElicitationManager::handle_response`) and only fulfills `ElicitResult`, not `CreateMessageResult` or `ListRootsResult`.
  - `grep handle_response src/server/mod.rs` returns no matches. The legacy Server's `spawn_message_handler` (line 741) observes `TransportMessage::Response(_)` (line 791) and emits `"Server received unexpected response message"` — **it drops the response entirely** rather than routing it to any pending correlation map.
  - There is no generic pending-request HashMap on ServerCore or Server. `grep pending src/server/mod.rs src/server/core.rs` shows only unrelated hits (TokenBucket, config fields).
- **Impact:**
  - Even if Plan 02 adds a `pending: HashMap<correlation_id, oneshot::Sender<Value>>` to DispatchPeerHandle, nothing in the current code routes a client's CreateMessageResult JSON-RPC response back into that map. `DispatchPeerHandle::sample()` would insert a pending oneshot, send the ServerRequest out (if there were a channel — see Finding 2), and then `timeout()` would fire because no response ever arrives.
  - Building the correlation/fulfillment layer is PHASE-LEVEL NEW WORK — not in the current plans.

### Finding 4: Missing Default / From impls on sampling types

- **Status: CONFIRMED**
- **Evidence:**
  - `grep "impl Default for CreateMessageParams\|impl Default for CreateMessageResult" src/` returns no matches.
  - `src/types/sampling.rs:197-230` — `CreateMessageParams` has `#[derive(Debug, Clone, Serialize, Deserialize)]` (NO `Default`) plus `#[non_exhaustive]`. It has a `new(messages: Vec<SamplingMessage>)` constructor (line 237). Callers must provide messages.
  - `src/types/sampling.rs:290-304` — `CreateMessageResult` has `#[derive(Debug, Clone, Serialize, Deserialize)]` (NO `Default`) plus `#[non_exhaustive]`. It has a `new(content, model)` constructor.
  - `grep "impl From<String> for ProgressToken\|impl From<&str> for ProgressToken" src/` returns no matches.
  - `src/types/notifications.rs:54-61` — `ProgressToken` is an enum with variants `String(String)` and `Number(i64)`. The canonical construction is `ProgressToken::String("...".to_string())`, not `ProgressToken::from("...")`.
- **Impact:**
  - **Plan 02 Task 3 test `test_peer_sample_respects_timeout`** uses `CreateMessageParams::default()` — this WILL NOT COMPILE.
  - **Plan 02 Task 3 test `test_peer_progress_notify_noop_without_reporter`** uses `ProgressToken::from("prog-1".to_string())` — this WILL NOT COMPILE.
  - **Plan 02 Task 5 integration test `test_sample_session_routing`** uses `CreateMessageParams::default()` — WILL NOT COMPILE.
  - **Plan 03 Task 1 example `s43_handler_peer_sample`** uses `CreateMessageParams::default()` AND `CreateMessageResult::default()` — WILL NOT COMPILE.
  - Fixes: use `CreateMessageParams::new(Vec::new())`, `CreateMessageResult::new(content, model)`, `ProgressToken::String("prog-1".to_string())` — all already exist per the struct constructors above.

### Finding 5: s43 doesn't run through a ToolHandler

- **Status: CONFIRMED**
- **Evidence:** Plan 03 Task 1 Step 2 (70-03-PLAN.md lines 267-352) shows the s43 body: `#[tokio::main] async fn main() { let mut extra = RequestHandlerExtra::default().with_peer(Arc::new(MockPeer)); ... extra.peer().expect(..).sample(CreateMessageParams::default()).await?; }`. The example has `impl PeerHandle for MockPeer` but does NOT have `impl ToolHandler for ...`. The peer call is driven from `main()` directly.
- **Impact:**
  - The example demonstrates the PeerHandle trait API surface but does not demonstrate "inside-a-tool-handler usage" as the plan narrative claims.
  - If kept, recast narrative as "API demonstration" (per Codex suggestion). If a true handler-path demo is wanted, wrap the peer call inside an `impl ToolHandler` and invoke `handler.handle(args, extra).await` in-process.

### Also Verified (lower stakes):

- **Shared RequestHandlerExtra accessor parity:** `src/shared/cancellation.rs` (full file read) — existing accessors are `auth_context()` (line 90) and `is_cancelled()` (line 95). There is NO existing `extensions()` / `extensions_mut()` surface. Plan 01 Task 1 steps 7-8 add only the FIELD, not the accessors, to the shared struct. **Confirmed: Codex's MEDIUM asymmetry note is accurate.** Plan 01 should also add `extensions()` / `extensions_mut()` methods to the shared struct for parity, or explicitly document why the shared struct has a smaller surface.

- **session_id availability at dispatch sites:** `grep session_id src/server/core.rs src/server/mod.rs` returns zero matches. The struct field on RequestHandlerExtra exists, but the dispatch sites never populate it. Plan 02 must either (a) plumb session_id through `ProtocolHandler::handle_request` (scope expansion), (b) leave session_id as `None` at construction time (accept that peer session isolation can't be asserted), or (c) defer peer wiring.

## Conclusion: Revision Scope

**Revision scope: HEAVY (Tier C).**

Justification:
- Finding 1 CONFIRMED (LOW impact; Path B via `extra.session_id` works but population is absent)
- Finding 2 CONFIRMED (HIGH impact — no outbound ServerRequest transport at any dispatch site)
- Finding 3 CONFIRMED (HIGH impact — no response correlation infrastructure that peer can reuse)
- Finding 4 CONFIRMED (MEDIUM impact — tests and examples won't compile as written)
- Finding 5 CONFIRMED (MEDIUM impact — example narrative is misleading but fixable)

Findings 2 AND 3 are confirmed (Tier C trigger). The current Plan 02 is not executable against the live codebase — wiring `.with_peer(Arc::new(DispatchPeerHandle::new(request_tx, ...)))` at 9 dispatch sites fails because `request_tx` does not exist and no response path could fulfill a pending oneshot even if it did. A foundational transport+correlation layer must be built before peer can be wired.

## Recommendation

Restructure Phase 70 as FOUR plans:

- **70-01:** Extensions typemap (light revision of current 70-01)
- **70-02:** Outbound server-to-client request dispatcher + response correlation (NEW — was not in the original plan set)
- **70-03:** PeerHandle trait + DispatchPeerHandle + wire .with_peer(...) at dispatch sites (rescoped from old 70-02)
- **70-04:** Examples + fuzz + docs + migration prose + make quality-gate (rescoped from old 70-03)

The ROADMAP and STATE must be updated to reflect "4 plans" instead of "3 plans."
