---
phase: 70
reviewers: [gemini, codex]
reviewed_at: "2026-04-17T06:05:00Z"
plans_reviewed: [70-01-PLAN.md, 70-02-PLAN.md, 70-03-PLAN.md]
---

# Cross-AI Plan Review — Phase 70

## Gemini Review

# Phase 70 Plan Review: Extensions and Peer Back-channel

This review covers the three-plan implementation for **Phase 70: Add Extensions typemap and peer back-channel to RequestHandlerExtra**, aimed at achieving parity with the official Rust MCP SDK (`rmcp`).

### 1. Summary
The plan set for Phase 70 is exceptionally well-researched and surgically designed. By leveraging the existing `http::Extensions` dependency and the established `ElicitationManager` pattern, the plans add significant ergonomic value to the SDK with minimal architectural risk. The strategy for maintaining backwards compatibility—using `#[non_exhaustive]` combined with a mechanical refactor of internal test sites—is robust and follows idiomatic Rust evolution patterns. The inclusion of comprehensive property, integration, and fuzz testing ensures the new capabilities meet the "Toyota Way" zero-defect standards mandated in the project context.

### 2. Strengths
- **Surgical Dependency Management:** Correctly identifies that `http::Extensions` is already a transitive dependency, avoiding the "dependency bloat" of adding crates like `anymap3`.
- **Robust Backwards Compatibility:** The audit of 28 `::new()` sites and 16 `::default()` sites confirms the API remains stable. The decision to mark `RequestHandlerExtra` as `#[non_exhaustive]` is a high-value long-term posture that aligns with `rmcp`.
- **Pattern-Matched Architecture:** The `PeerHandle` implementation (mpsc + oneshot + timeout) directly adapts the proven `ElicitationManager` logic, ensuring consistency with existing async patterns in the codebase.
- **Security-First Design:**
  - `Extensions` are correctly identified as needing `Clone + Send + Sync + 'static`.
  - `PeerHandle` isolation is verified via a dedicated integration test for session routing (`T-70-02` mitigation).
  - `Debug` redaction for `Extensions` (type-names only) and `PeerHandle` (placeholder) prevents credential leakage in logs.
- **WASM Sensitivity:** Clearly distinguishes between the wasm-safe `Extensions` and the non-wasm `PeerHandle`.
- **Compliance with "ALWAYS" Requirements:** Explicitly schedules proptests, integration tests, fuzz targets, and examples across the three waves.

### 3. Concerns
- **Positional Struct Literal Breakage (LOW):** While the plans refactor all 12 internal sites, any external users of the SDK who used positional struct literals (instead of `::new()` or `::default()`) will face a breaking change. *Mitigation:* This is standard for `#[non_exhaustive]` migrations and is correctly addressed in the migration documentation task.
- **`session_id` Plumbing Risk (MEDIUM):** Research identifies a risk that `session_id` might not be readily available at all 9 dispatch sites. *Mitigation:* Plan 02, Task 1 is a dedicated "decision task" to classify the plumbing path (A/B/C) before execution.
- **`Extensions` Clone Cost (LOW):** `http::Extensions` performs a deep clone of values. Users might unknowingly insert large structures. *Mitigation:* The plan includes a specific documentation task to warn users to prefer `Arc<T>`.

### 4. Suggestions
- **Trait Re-exports:** In Plan 02, Task 2, ensure `PeerHandle` is re-exported at the crate root (`src/lib.rs`) alongside `RequestHandlerExtra`.
- **Performance Benchmarking:** Since `RequestHandlerExtra` is cloned frequently in middleware chains, consider adding a micro-benchmark in a future phase.
- **Explicit Mock for Examples:** In `examples/s43`, ensure the mock's `sample` implementation is documented as "for demonstration only".

### 5. Risk Assessment: LOW
The overall risk is **LOW**. Architectural fit is additive; stability analysis is exhaustive; verification covers unit, property, integration, and fuzz testing; cognitive complexity is kept low by using existing patterns (`ElicitationManager`).

**Verdict:** **APPROVED.** The planner should proceed with Wave 1.

---

## Codex Review

# Plan Review

### 70-01-PLAN.md

**1. Summary.** This is the strongest of the three plans. The scope is tight, the dependency story is clean, and it maps well to the actual codebase shape: two `RequestHandlerExtra` structs, 12 positional literals in `src/server/workflow/prompt_handler.rs`, and an already-available `http::Extensions`. The main weakness is that it overstates backward compatibility: `#[non_exhaustive]` is a public API break for downstream struct-literal users even if all internal `::new()` and `::default()` call sites survive unchanged.

**2. Strengths.**
- Uses `http::Extensions` instead of inventing a typemap or adding a new dependency.
- Correctly treats both `src/server/cancellation.rs` and `src/shared/cancellation.rs` as in scope.
- Explicitly handles the 12 known struct-literal sites instead of discovering them mid-implementation.
- Keeps the wave focused on additive context storage, not peer plumbing.
- Includes useful verification for clone semantics, overwrite semantics, and debug redaction behavior.

**3. Concerns.**
- **HIGH:** `#[non_exhaustive]` is not actually "drop-in backwards-compatible" for any downstream crate constructing `RequestHandlerExtra { ... }`. Internal compatibility is fine; public compatibility is not.
- **MEDIUM:** The shared `RequestHandlerExtra` gets the new field but not the same accessor surface. That asymmetry may be acceptable internally, but it weakens the "both structs gain the same extensions field" story.
- **LOW:** The proptests are valid but narrow. They mostly exercise one or two types and don't prove much about mixed-type occupancy or remove semantics.
- **LOW:** The plan assumes all 12 literals are trivially reducible to `::new(...)`. It says to verify per-site, which is good, but that should be elevated from a note to an explicit acceptance requirement.

**4. Suggestions.**
- Reframe the compatibility claim: "non-breaking for constructor/builder users; breaking for downstream struct-literal users."
- Decide deliberately whether `#[non_exhaustive]` is acceptable in this phase. If yes, call out the semver impact explicitly. If no, keep the field addition and convert only internal tests to builders.
- Add `extensions()` / `extensions_mut()` to the shared struct too, or explicitly document why that surface is intentionally smaller.
- Add one unit/property test covering `remove::<T>()` and one covering two different stored types simultaneously.
- Make "all 12 literals verified to preserve non-default fields if any appear" part of acceptance, not just task prose.

**5. Risk Assessment: MEDIUM.** Implementation work is straightforward, but the public API compatibility claim is materially overstated because of `#[non_exhaustive]`.

---

### 70-02-PLAN.md

**1. Summary.** This plan is directionally right but architecturally under-specified in ways that matter. The `PeerHandle` trait itself is reasonable, but the concrete wiring is not grounded in the current transport/runtime design. The biggest issue is that the plan assumes an outbound `mpsc::Sender<ServerRequest>` and corresponding response-correlation path are available at the dispatch layer. In the current code, that is true for `ElicitationManager`, but not for general handler dispatch, and the legacy `Server` path only has `notification_tx: Sender<Notification>`, which is the wrong channel type.

**2. Strengths.**
- Splits the hardest work into a dedicated wave and explicitly calls out the session/threading risk.
- Correctly cfg-gates peer functionality to non-wasm.
- Fresh-per-request peer construction is the right direction for session isolation.
- Reuses the `ElicitationManager` pattern instead of proposing new concurrency primitives.
- Treats progress reporting as a convenience wrapper rather than new protocol work.

**3. Concerns.**
- **HIGH:** The plan does not solve the actual outbound request/response plumbing problem. `sample()` and `list_roots()` need both request dispatch and response fulfillment. A local `pending: HashMap<correlation_id, oneshot>` in `DispatchPeerHandle` is useless unless something routes client responses back into it. The current plan never specifies that integration.
- **HIGH:** The legacy `Server` path does not expose `Sender<ServerRequest>` at the dispatch sites. It has `notification_tx: Option<mpsc::Sender<Notification>>` in `src/server/mod.rs`, which cannot carry `ServerRequest`.
- **HIGH:** The session gate in Task 1 is necessary but not sufficient. Even if a session id were discoverable, the plan still lacks the transport-level mechanism to send server requests and receive correlated responses.
- **HIGH:** `AuthContext` does not contain `session_id` in the current code (`src/server/auth/traits.rs`). So Path A in Task 1 is currently false.
- **HIGH:** The proposed tests use APIs that do not exist: `CreateMessageParams::default()`, `CreateMessageResult::default()`, and `ProgressToken::from(String)`.
- **MEDIUM:** Integration tests cannot import `pub(crate)` items like `server::peer_impl::DispatchPeerHandle` from an external test crate without adding explicit test support exports.
- **MEDIUM:** The plan says "9 dispatch sites" but the core architectural value is on the active dispatch path. Forcing parity onto the legacy path before the transport abstraction exists increases risk and scope.
- **LOW:** Wiring `peer` into `handle_create_message` is not obviously wrong, but it is not central to the phase goal and adds conceptual noise.

**4. Suggestions.**
- Make the first task an architecture task, not just a classifier. It should answer two separate questions: (a) Where does outbound `ServerRequest` transport live? (b) Where do correlated responses get routed back?
- Reduce scope if needed: land `PeerHandle` surface and `RequestHandlerExtra.peer` first, but wire only where a real request/response path exists.
- Consider introducing a shared server-to-client request dispatcher abstraction rather than constructing peer plumbing directly from ad hoc channels at each site.
- Prove the end-to-end response path before touching all 9 sites. One working dispatch path is worth more than nine call sites with no fulfillment mechanism.
- Fix the test plan to use real constructors/builders for `CreateMessageParams`, `CreateMessageResult`, and `ProgressToken`.
- Decide whether legacy `Server` support is actually in scope for this phase. If it is, the transport/runtime work needs to be planned explicitly, not treated as a small follow-on.

**5. Risk Assessment: HIGH.** The trait shape is fine, but the concrete implementation plan is not yet executable against the current architecture. The missing outbound request/response plumbing is a phase-level blocker, not a minor risk.

---

### 70-03-PLAN.md

**1. Summary.** This plan has the right cleanup/compliance goals, and the `s42`/`s43` numbering is correct, but the examples are weaker than the plan claims and the fuzz target is too shallow. As written, the examples mostly demonstrate manually-constructed `RequestHandlerExtra`, not actual handler-path behavior, and the sample code again assumes nonexistent `Default` impls on sampling types.

**2. Strengths.**
- Correctly avoids the `s22`/`s23` collision and uses `s42`/`s43`.
- Registers examples and fuzz targets explicitly, which makes CI/discovery better.
- Includes migration prose and ties documentation work to `cargo doc -D warnings`.
- Keeps the fuzz target lightweight and compatible with the existing fuzz crate layout.

**3. Concerns.**
- **HIGH:** `s43` does not actually show `extra.peer()` "inside a tool handler" as claimed. It constructs `RequestHandlerExtra` in `main` and calls the peer directly.
- **HIGH:** The example code assumes `CreateMessageResult::default()` and likely `CreateMessageParams::default()`, which do not exist.
- **MEDIUM:** `s42` also does not really demonstrate cross-middleware transfer; it demonstrates manual insert/get on a local extra.
- **MEDIUM:** The fuzz target does not exercise `DispatchPeerHandle` or response-correlation behavior. It is mostly serde smoke over existing types.
- **MEDIUM:** `cargo run --example ...` examples that intentionally avoid starting the server are fine, but then they should be framed as API demos, not runtime/server examples.
- **LOW:** Depending on how Plan 02 lands, import paths for `PeerHandle` and peer-related types may drift.

**4. Suggestions.**
- Recast `s42` and `s43` as "API demonstrations" unless they truly run through a handler path.
- If you want a real handler example, implement a tiny `ToolHandler` and invoke it in-process with a prepared `RequestHandlerExtra`, or run a minimal server/client loopback.
- Replace all `Default` assumptions on sampling types with real constructors.
- Strengthen the fuzz target to cover the actual peer boundary.
- Keep the docs work, but let the example/fuzz details wait until the actual peer surface from Plan 02 is stable.

**5. Risk Assessment: MEDIUM-HIGH.** The registration/docs work is solid, but the example and fuzz content currently do not prove what the plan says they prove.

---

### Cross-Plan Assessment (Codex)

**What's solid:** wave split, `http::Extensions` choice, `s42`/`s43` numbering, 12-site refactor.

**Biggest phase risks:**
- The peer wave is not yet end-to-end sound. The current codebase does not expose a generic handler-level outbound `ServerRequest` channel plus correlated response path.
- The "backwards-compatible" story is only true for constructor/builder users, not for public struct-literal users if `#[non_exhaustive]` lands.
- Several planned tests/examples rely on nonexistent APIs (`Default` and `From` impl assumptions).

**Overall recommendation:**
- **Approve Plan 01 with revisions.**
- **Do not execute Plan 02 as written.** Rework it around a real outbound request/response architecture first.
- **Defer locking Plan 03 until Plan 02's actual public surface exists.**

**Overall Risk: HIGH.**

---

## Consensus Summary

### Agreed Strengths (both reviewers)
- `http::Extensions` is the correct typemap choice (zero new deps).
- `s42`/`s43` example numbering avoids the `s22`/`s23` collision.
- 12-site positional struct-literal refactor in `prompt_handler.rs` is the right mechanical cleanup.
- Wave split (extensions → peer → examples/docs/fuzz) is sensible.
- Pattern-matching `ElicitationManager` for the peer implementation is idiomatic.
- `Send + Sync + 'static + Clone` requirement on extensions is correctly identified.
- WASM cfg-gating of the peer field is correctly scoped.

### Agreed Concerns (both reviewers raised these)
- **`session_id` threading is uncertain.** Gemini ranks MEDIUM with Plan 02 Task 1 as mitigation; Codex ranks HIGH and argues the classifier is necessary but not sufficient (even if session_id is discoverable, the transport-level request/response mechanism is missing).
- **`#[non_exhaustive]` backwards-compat caveat.** Gemini: LOW (standard for migrations, covered in docs). Codex: HIGH (claim is materially overstated for downstream struct-literal users).

### Divergent Views (worth investigating)

| Dimension | Gemini | Codex |
|-----------|--------|-------|
| **Overall Risk** | LOW | HIGH |
| **Plan 02 executable as written?** | Yes, with Path A/B/C gate sufficient | No — missing outbound `ServerRequest` transport + response-correlation path |
| **`AuthContext.session_id`** | Assumed present | NOT present in current code (`src/server/auth/traits.rs`) — Path A in Task 1 is false |
| **Sampling/CreateMessage API shape** | Assumed stable | `CreateMessageParams::default()`, `CreateMessageResult::default()`, `ProgressToken::from(String)` may not exist |
| **Legacy `Server` path coverage** | Not flagged | Legacy path has `Sender<Notification>`, not `Sender<ServerRequest>` — wiring peer there is architecturally incompatible without new transport work |
| **s43 demonstrates handler path?** | Yes (with MockPeer) | No — constructs `RequestHandlerExtra` in `main()`, not inside an actual `ToolHandler` |
| **Fuzz target coverage** | Adequate | Too shallow — serde smoke over existing types, doesn't exercise `DispatchPeerHandle` or correlation-id parsing |

### Highest-Priority Concerns to Address (Codex-specific, unverified — need codebase check)

The following Codex findings are **specific, actionable, and not noted by Gemini**. Each should be verified against the live codebase before executing Plan 02 / 03:

1. **[BLOCKER candidate]** Does `AuthContext` actually contain `session_id`? Codex claims no (`src/server/auth/traits.rs`). Plan 02 Task 1 Path A assumes yes. **Verify** before Plan 02 execution.
2. **[BLOCKER candidate]** Is there an outbound `mpsc::Sender<ServerRequest>` available at the 9 dispatch sites, OR only `Sender<Notification>`? If only notification channel exists, `sample()` / `list_roots()` cannot be implemented without new transport plumbing.
3. **[BLOCKER candidate]** Is there any existing mechanism to correlate a server-issued request's ID to its client response, OR does the request/response fulfillment layer need to be built from scratch?
4. **[MEDIUM]** Do `CreateMessageParams`, `CreateMessageResult`, and `ProgressToken` actually have `Default` / `From<String>` impls? Tests and examples depend on it.
5. **[MEDIUM]** Does `s43` currently construct `RequestHandlerExtra` in `main()` rather than inside a `ToolHandler` impl? If so, the example doesn't demonstrate the intended handler pattern.
6. **[MEDIUM]** Does the shared `RequestHandlerExtra` in `src/shared/cancellation.rs` get the full `extensions()` / `extensions_mut()` accessor surface, or only the field? Codex flagged asymmetry.

### Recommended Next Action

The divergence between the two reviewers is significant enough that **running `/gsd-plan-phase 70 --reviews` to let the planner incorporate Codex's architectural concerns** is warranted. The planner should:

1. **Verify Codex's HIGH findings against the live codebase** — specifically `AuthContext`'s fields, the channel type at dispatch sites, and the existence of response-correlation infrastructure.
2. **If findings are confirmed:** rework Plan 02 to either (a) scope down to wire peer only where a real request/response path exists, or (b) add a new plan that builds the outbound `ServerRequest` dispatcher before touching the 9 call sites.
3. **Address the `#[non_exhaustive]` semver framing** in all three plans — reframe the backwards-compat claim as "non-breaking for constructor users; breaking for struct-literal users" and explicitly call out the semver impact.
4. **Fix the `Default` / `From` assumptions** in test scaffolding and s43 example, OR add a pre-implementation verification task that confirms those APIs exist.
5. **Clarify s43 scope** — either implement it inside a real `ToolHandler` or recast its description as an "API demonstration" rather than a handler-path demo.
