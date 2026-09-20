---
phase: 106
reviewers: [codex, gemini]
reviewed_at: 2026-07-17T00:00:00Z
plans_reviewed: [106-01-PLAN.md, 106-02-PLAN.md, 106-03-PLAN.md]
---

# Cross-AI Plan Review — Phase 106

## Codex Review

# Cross-AI Plan Review — Phase 106

## Overall assessment

The phase is well researched and decomposed sensibly: core dispatch first, policy/capabilities second, documentation in parallel. The plans correctly identify the nested receive-loop architecture and the `sampling/createMessage` parse ambiguity. However, several working-tree mismatches would either prevent compilation or leave stated guarantees unfulfilled. The most important are the invalid legacy trait path, WASM-incompatible roots type, incomplete `Client` construction updates, unreachable handling of genuinely unknown wire methods, an ineffective approval-security model, and missing contract/version work required by repository policy.

Overall phase risk: **HIGH until the blocking issues below are corrected**.

---

# Plan 106-01 — Client Host Registry and Dispatch

## Summary

The plan has a strong core architecture and appropriately limits the implementation to nested server-to-client requests rather than introducing a background client pump. Its test-first emphasis is good. As written, though, it contains multiple concrete implementation blockers and overstates what the typed transport can handle.

## Strengths

- Correctly locates dispatch inside `Client::send_request`.
- Explicitly preserves the original request in `active_requests` while answering a nested request.
- Handles the important sampling parse ambiguity by recognizing `Request::Client(ClientRequest::CreateMessage)`.
- Uses distinct host-side names such as `HostSamplingHandler`.
- Keeps the registry immutable after construction, avoiding unnecessary locking.
- Includes unit, duplex integration, property, example, and WASM verification targets.
- Documents the idle-host limitation instead of expanding scope into a background receive loop.
- Uses JSON-RPC responses for unregistered known methods rather than terminating the connection.

## Concerns

- **HIGH — The documented server trait path does not exist.** `src/server/traits.rs` exists, but it is not declared as `pub mod traits`. The public trait is currently `crate::server::SamplingHandler` and re-exported as `crate::SamplingHandler`. Rustdoc links to `crate::server::traits::SamplingHandler` will fail the zero-warning rustdoc gate, and the book would document a nonexistent API.

- **HIGH — The proposed roots provider is not WASM-clean.** `ListRootsResult` lives in `crate::server::roots`, and that module is gated with `#[cfg(not(target_arch = "wasm32"))]`. A cfg-agnostic `client::host::RootsProvider` importing that type cannot compile for `wasm32-unknown-unknown`.

- **HIGH — Adding `host_registry` requires more constructor updates than the plan specifies.** It must be added to:

  - `Client::with_info`
  - `Client::with_options`
  - any other `Client` struct literals
  - `Clone for Client<T>`

  The plan only explicitly discusses the field, builder threading, and possibly `Debug`. Missing any constructor or `Clone` initializer causes compilation failure.

- **HIGH — Truly unknown wire methods cannot reach `dispatch_host_request`.** `parse_request` rejects unknown methods before producing a `TransportMessage::Request`. Thus the plan can return `-32601` for known-but-unhandled typed variants or missing handlers, but not for an arbitrary unknown JSON-RPC method. The must-have “unknown inbound request type returns -32601” is not achieved without changing parsing or retaining an unknown/raw request representation.

- **HIGH — The elicitation round-trip description is not executable through the stated high-level server path.** `PeerHandle` supports `sample`, `list_roots`, and progress, but not elicitation. `Server::run` also does not wire its `ElicitationManager` to the new generalized dispatcher. A tool can issue sampling and roots through `extra.peer()`, but not elicitation. HOST-02 can still be tested with a raw duplex server request pump, but the plan must specify that different harness.

- **MEDIUM — `RootsProvider` cannot report failure.** Its future returns `ListRootsResult` directly, unlike the sampling and elicitation handlers, which return `Result`. A roots provider backed by filesystem or workspace state may fail. This also makes the promised “handler failure → -32603” behavior inconsistent across the three host methods.

- **MEDIUM — Example registration is omitted.** The repository states every top-level example is registered in `Cargo.toml`, but `Cargo.toml` is not listed or edited. The example should also be added to the examples index if that index is expected to remain complete.

- **MEDIUM — HOST-06 traceability is incomplete.** The plan implements HOST-06 rustdoc, but its frontmatter only lists `[HOST-01, HOST-02, HOST-03]`.

- **MEDIUM — Handler error messages may disclose sensitive internal details.** Returning the full SDK `Error` text to an untrusted server as `-32603` may expose paths, provider details, or credentials. Internal errors should normally be logged locally and returned with a sanitized message.

- **LOW — Sequential host callbacks can stall the original request indefinitely.** This is compatible with the nested architecture, but the API documentation should state that handlers own timeout/cancellation policy.

## Suggestions

- Replace every `server::traits::SamplingHandler` reference with the actual public path, preferably `pmcp::SamplingHandler` or `pmcp::server::SamplingHandler`.

- Move `Root` and `ListRootsResult` into a target-agnostic protocol/type module and re-export them from `server::roots`, or explicitly cfg-gate roots hosting on WASM. Do not claim a cfg-agnostic host module until this is resolved.

- Add an exhaustive checklist for every `Client` constructor and clone implementation.

- Narrow the `-32601` claim to “known but unhandled or unregistered request,” or add a typed `Unknown { method, params }` request representation so arbitrary inbound methods can be answered.

- Define two integration harness shapes:

  - High-level `Server` tool for sampling and roots.
  - Raw duplex `ServerRequest::ElicitationCreate` pump for elicitation.

  Alternatively, intentionally extend `PeerHandle` with elicitation, but that is additional public scope and should be approved explicitly.

- Change roots to something like:

  ```rust
  Fn() -> BoxFuture<'static, Result<ListRootsResult>>
  ```

- Add `Cargo.toml` and `examples/README.md` to `files_modified`.

- Add HOST-06 to the plan frontmatter requirements.

- Sanitize internal handler failures while preserving a locally logged diagnostic.

- Add a test showing a known unregistered request receives `-32601` and a subsequent normal response still completes the original client call.

## Risk assessment

**HIGH.** The main design is sound, but the invalid API path, WASM type gating, missing constructor/clone work, and infeasible elicitation test path are implementation blockers. The unknown-method guarantee also cannot be met by the currently proposed dispatch layer.

---

# Plan 106-02 — Approval, Capabilities, and Fuzzing

## Summary

Separating approval and capability derivation from basic dispatch is a good sequencing choice. Making the registry authoritative is aligned with HOST-05. The approval design, however, is internally inconsistent: it runs after the model handler but cannot inspect the completion, so it neither prevents an unapproved LLM call nor delivers the stated post-completion review semantics. The fuzz target is also much shallower than its name and threat claims suggest.

## Strengths

- Clear dependency on 106-01.
- Registry-authoritative host capability fields prevent obvious capability drift.
- Preserves unrelated capability fields.
- Covers both directions of capability override in tests.
- Returns approval denial as a JSON-RPC response rather than a transport error.
- Includes default-allow, explicit allow, and deny tests.
- Adds adversarial serde input coverage without introducing dependencies.
- Keeps callback types free from Tokio spawning.

## Concerns

- **HIGH — Approval occurs after the potentially expensive or sensitive action.** The sampling handler is the component that calls the LLM. Running approval after it returns does not prevent a server from coercing the client into an unapproved LLM call. Therefore threat T-106-05 is not actually mitigated.

- **HIGH — The callback cannot inspect the completion despite the stated rationale.** `ApprovalCallback` accepts only `&CreateMessageParams`, not `&CreateMessageResult`. Calling it after the handler does not “let the approver see the actual completion.”

- **HIGH — Moving params into the handler conflicts with borrowing them afterward.** The handler takes `CreateMessageParams` by value, while approval later needs `&params`. The plan needs to require cloning before handler invocation or revise the signatures.

- **HIGH — The fuzz target does not exercise dispatch.** It only calls `serde_json::from_value` for two parameter types. It does not cover:

  - `parse_request`
  - sampling alias normalization
  - handler routing
  - JSON-RPC error construction
  - connection continuation
  - hangs or callback behavior

  Calling it `client_host_dispatch` and claiming it protects “the dispatch path” is misleading.

- **HIGH — Verification pipelines can mask failure.** Commands such as:

  ```bash
  cargo test ... 2>&1 | tail -20
  ```

  report the exit status of `tail` unless `pipefail` is enabled. The same defect appears throughout all three plans. A failing Cargo or mdBook command can therefore be reported as passing.

- **MEDIUM — `-32603` is a weak semantic choice for policy refusal.** It labels an intentional approval denial as an internal server error. A documented MCP/application error in the server-error range would communicate refusal more accurately. If `-32603` is locked, the docs should at least explain the taxonomy.

- **MEDIUM — Capability presence does not describe supported sampling or elicitation modes.** `SamplingCapabilities::default()` and `ElicitationCapabilities::default()` serialize as empty objects. The SDK has fields for sampling tools and elicitation form/URL support. Presence-only derivation may satisfy the locked requirement, but it cannot truthfully express which modes a particular handler supports.

- **MEDIUM — Connection survival is not proven by inspecting a response object.** A unit test of `dispatch_host_request` only proves it returns a response. It does not prove that the receive loop sends it, continues waiting, and accepts a subsequent call.

- **LOW — Denial reasons may disclose local policy information.** Sending the callback’s complete reason to the remote server should be an explicit API decision.

## Suggestions

- Choose one coherent approval model:

  - **Preflight approval:** callback sees params and runs before the handler, genuinely preventing unapproved LLM calls.
  - **Postflight review:** callback sees both params and result, allowing output review before release.
  - **Two-stage model:** preflight authorization plus optional postflight output review.

- If retaining postflight approval, change the callback to receive a result-aware context and clone the params explicitly before passing ownership to the handler.

- Rename the fuzz target to `host_params_deserialization`, or make dispatch normalization a pure, accessible function and fuzz that actual function.

- Add corpus cases for:

  - both sampling parse variants
  - missing required params
  - extreme nesting and large arrays
  - unknown typed requests
  - handler/provider errors

- Replace piped verification with unpiped commands, or prefix scripts with `set -o pipefail`.

- Add a duplex denial test that:

  1. triggers sampling,
  2. receives a denial,
  3. performs a subsequent client request successfully.

- Consider allowing builder methods to accept capability detail alongside handlers, such as elicitation modes or sampling tool support, while keeping handler presence authoritative.

## Risk assessment

**HIGH.** Capability derivation is straightforward, but the approval mechanism does not provide the security property claimed, and the fuzz work does not test the advertised surface. Masked verification failures further undermine execution confidence.

---

# Plan 106-03 — Sampling and Hosting Documentation

## Summary

The documentation plan is appropriately small and clearly explains the two sampling directions and nested-flow limitation. It should proceed after correcting the invalid server trait path and tightening its dependency/verification details.

## Strengths

- Keeps documentation scope limited to the required disambiguation seed.
- Explicitly states that the legacy path is retained and not deprecated.
- Includes a compact direction-comparison table.
- Documents the idle-host limitation, which is essential to avoid overpromising.
- Adds the page to `SUMMARY.md` without restructuring the book.
- References the runnable example.
- Can execute in parallel with plan 02 once plan 01’s API is stable.

## Concerns

- **HIGH — It instructs authors to document a nonexistent public path.** `pmcp::server::traits::SamplingHandler` is not currently a public module path. The actual public path is `pmcp::server::SamplingHandler` or `pmcp::SamplingHandler`.

- **MEDIUM — It documents `on_sampling_approval` without depending on plan 02.** The builder method exists after plan 01, but its approval behavior is not functional until plan 02. This is harmless at final phase merge, but unsafe if plan 03 is independently shipped or reviewed before plan 02.

- **MEDIUM — “Links the two trait entry points” is not enforced.** Acceptance only greps for text. It does not require actual rustdoc/docs.rs links or validate their targets.

- **MEDIUM — mdBook verification can mask failure.** `mdbook build ... | tail -5` lacks `pipefail`.

- **LOW — “Full chapter arrives later” may age poorly.** A durable link to the roadmap or wording such as “This page focuses on direction and hosting semantics” would avoid stale future-tense prose.

## Suggestions

- Use the real public paths:

  - `pmcp::client::host::HostSamplingHandler`
  - `pmcp::SamplingHandler` or `pmcp::server::SamplingHandler`

- Add actual links to generated API documentation, or use explicit code-path references consistently without claiming they are links.

- Either add `106-02` as a dependency or phrase approval as part of the completed Phase 106 surface rather than assuming plan-level independent shippability.

- Run `mdbook build pmcp-book` without a masking pipeline.

- Add a lightweight link checker or at least verify the referenced example is registered and builds.

## Risk assessment

**MEDIUM.** The prose structure is good, but the documented legacy API path is wrong and would confuse readers. Once corrected, this plan is low risk.

---

# Cross-plan gaps

- **HIGH — No contract-first work is planned.** Repository instructions require every feature or bug fix to update the relevant YAML under `../provable-contracts/contracts/<crate>/` and run `pmat comply check` before and after implementation. None of the plans includes this.

- **HIGH — The promised pmcp minor version bump is absent.** Plan 106-01 says `2.15.0 -> 2.16.0`, but no plan modifies `Cargo.toml`, `Cargo.lock`, or release-facing documentation. Either add a dedicated version/release task or remove the claim and defer the bump explicitly.

- **HIGH — Verification commands repeatedly mask failures through `| tail`.** Fix this in every task before execution.

- **MEDIUM — No explicit PMAT complexity verification is included.** `make quality-gate` does not run the CI PMAT complexity gate. The new `dispatch_host_request` should be checked against cognitive complexity ≤25, especially if it combines routing, handler invocation, approval, serialization, and error mapping.

- **MEDIUM — No explicit 80% coverage measurement is planned.** The plans add substantial tests, but the repository’s stated feature policy requires an 80%+ target. Add a scoped coverage check or document how the phase demonstrates the threshold.

- **MEDIUM — Example metadata is incomplete.** Add `s49_sampling_host` to `Cargo.toml` and the maintained examples index.

## Recommended sequencing

1. Correct the public API paths and resolve the WASM roots-type location.
2. Add contract YAML and pre-implementation `pmat comply check`.
3. Revise 106-01 for all `Client` constructors/clone, realistic unknown-method semantics, and a separate elicitation harness.
4. Redesign approval as genuine preflight authorization or result-aware postflight review.
5. Strengthen the fuzz target and denial integration test.
6. Add version, example registration, coverage, PMAT, and non-masking verification tasks.
7. Execute 106-01, then run 106-02 and 106-03 in parallel.
8. Finish with `pmat comply check`, WASM check, mdBook build, PMAT complexity check, and `make quality-gate`.

---

## Gemini Review

# Cross-AI Plan Review: Phase 106 — Client Host Surface

I have conducted a thorough review of the three execution plans (`106-01-PLAN.md`, `106-02-PLAN.md`, and `106-03-PLAN.md`) against the PMCP codebase located at `/Users/guy/Development/mcp/sdk/rust-mcp-sdk`. Here is the structured feedback on plan quality, completeness, and risks.

---

### 1. Summary
The implementation plans provide a solid, additive, and backward-compatible roadmap for enabling the spec-compliant host-side surface on the [Client](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs#L107). By registering client-side handlers for sampling, elicitation, and roots listing, the client shifts from a purely request-driven consumer to a functional host. The plans correctly identify and address the `sampling/createMessage` parse ambiguity (resolving to both `Request::Client` and `Request::Server` variants in [protocol_helpers.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/protocol_helpers.rs#L21)), protect connection state by returning standard JSON-RPC `-32601` errors on unhandled methods, and cleanly segregate host traits into the new `client::host` namespace to avoid naming collisions. However, the plans introduce critical Rust compile-time async lifetime errors in the approval callback definition, contradict the design goal of allowing the callback to inspect generated completions, and risk breaking customized developer capability fields during initialization.

---

### 2. Strengths
*   **Excellent Naming and Segregation**: Introducing a separate module path `pmcp::client::host::*` and naming the traits [HostSamplingHandler](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs) and `HostElicitationHandler` completely sidesteps any compile-time or developer naming confusion with the pre-existing server-side `SamplingHandler` (the "LLM-server pattern").
*   **Parse Ambiguity Workaround**: Recognizing that inbound `sampling/createMessage` requests deserializing through [protocol_helpers.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/protocol_helpers.rs#L16) default to `Request::Client` (due to the order in `parse_request`), and explicitly matching both `Request::Client` and `Request::Server` shapes in the dispatcher is a crucial, high-quality catch.
*   **Non-destructive JSON-RPC Error Taxonomy**: Returning `-32601` (method not found) or `-32603` (internal failure) error responses rather than returning a connection-level `Err` keeps the transport loop alive and preserves the connection when unhandled or denied requests occur.
*   **WASM Compatibility**: The proposed traits and types are entirely `cfg`-agnostic, avoiding any tokio-specific runtime spawns. The use of `async_trait` and target-agnostic locks guarantees compilation compatibility with the `wasm32-unknown-unknown` target.
*   **Documentation Disambiguation**: Conceptually dividing the same-named features into "Spec host sampling" (server asks client) and the legacy "LLM-server pattern" (client asks server) inside the book and rustdoc prevents developer confusion about protocol flow direction.

---

### 3. Concerns
*   **Concern 1 (HIGH): Async Approval Callback Lifetime Mismatch (`'static` vs reference)**
    *   *Detail*: The plan defines `ApprovalCallback` as:
        ```rust
        pub type ApprovalCallback = Arc<dyn Fn(&CreateMessageParams) -> futures::future::BoxFuture<'static, ApprovalDecision> + Send + Sync>;
        ```
        Because the returned future is `'static`, it is legally forbidden from capturing or referencing the borrowed input parameter `&CreateMessageParams` (since it does not live for `'static`). Any developer trying to implement an async approval callback that inspects the request parameters (e.g., printing or matching on the prompt messages) will hit a compile-time lifetime violation.
    *   *Location*: `src/client/host/sampling.rs` (defined in Plan 01)
*   **Concern 2 (HIGH): Missing Generated Result in Approval Callback**
    *   *Detail*: Plan 02 states that the approval callback is invoked *after* the handler produces the `CreateMessageResult` to "let the approver see the actual completion." However, the proposed `ApprovalCallback` signature (`Fn(&CreateMessageParams)`) only accepts request parameters and lacks an argument for the generated `CreateMessageResult`. The callback is therefore unable to inspect the text it is supposed to approve.
    *   *Location*: `src/client/host/sampling.rs` (defined in Plan 01) and [src/client/mod.rs](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs) (invoked in Plan 02)
*   **Concern 3 (HIGH): Destructive Capability Overwrite in `initialize`**
    *   *Detail*: In `initialize`, the plan overrides capability fields directly:
        ```rust
        capabilities.sampling = self.host_registry.sampling.is_some().then(SamplingCapabilities::default);
        ```
        This completely wipes out any sub-capability fields a developer configured in their [ClientCapabilities](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/capabilities.rs#L25) (such as the list of supported models in `SamplingCapabilities::models` or list-change notifications in `RootsCapabilities::list_changed`). 
    *   *Location*: `src/client/mod.rs` (initialize path)
*   **Concern 4 (MEDIUM): Missing Error Channel in `RootsProvider`**
    *   *Detail*: The proposed signature is:
        ```rust
        pub type RootsProvider = Arc<dyn Fn() -> futures::future::BoxFuture<'static, ListRootsResult> + Send + Sync>;
        ```
        Listing roots is an I/O operation (querying directories or external storage) and can fail. Because the closure returns `ListRootsResult` directly instead of a `Result<ListRootsResult, Error>`, the client has no way to return a JSON-RPC error response to the server if a roots listing operation fails.
    *   *Location*: `src/client/host/roots.rs` (defined in Plan 01)
*   **Concern 5 (MEDIUM): Fuzz Target does not exercise Dispatch Logic**
    *   *Detail*: The fuzzer `client_host_dispatch` is planned to only call `from_value` on fuzzed parameters. This only fuzzes serde deserialization (already covered by unit tests). It does not test the new `dispatch_host_request` routing, matching, or handler invocation.
    *   *Location*: `fuzz/fuzz_targets/client_host_dispatch.rs` (Plan 02)
*   **Concern 6 (MEDIUM): Denial of Wallet / Token Exhaustion Vulnerability**
    *   *Detail*: Because the approval callback runs *after* the [HostSamplingHandler](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/client/mod.rs) generates the completion, a malicious server can repeatedly call `sampling/createMessage` to make LLM calls. Even if the callback denies the completion, the tokens have already been billed to the user's LLM account.
    *   *Location*: `src/client/mod.rs` (dispatch path)

---

### 4. Suggestions
*   **Fix Approval Callback Lifetimes and Signature**: Redefine the signature to pass arguments by value so they can be easily moved into the `'static` future, and include the generated `CreateMessageResult`:
    ```rust
    pub type ApprovalCallback = Arc<
        dyn Fn(CreateMessageParams, CreateMessageResult) -> futures::future::BoxFuture<'static, ApprovalDecision> + Send + Sync
    >;
    ```
*   **Use Conditional Override for Capabilities**: In `initialize`, only overwrite the capability fields if the handler is absent, or merge them to keep developer-configured sub-capabilities intact:
    ```rust
    if self.host_registry.sampling.is_none() {
        capabilities.sampling = None;
    } else if capabilities.sampling.is_none() {
        capabilities.sampling = Some(SamplingCapabilities::default());
    }
    // Repeat for elicitation and roots fields
    ```
*   **Wrap `RootsProvider` in a Result**: Update the provider's signature to return a `Result` to allow robust error handling:
    ```rust
    pub type RootsProvider = Arc<
        dyn Fn() -> futures::future::BoxFuture<'static, Result<ListRootsResult, crate::Error>> + Send + Sync
    >;
    ```
*   **Improve Fuzzing Depth**: Have the `client_host_dispatch` fuzzer parse bytes using [parse_request](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/shared/protocol_helpers.rs#L16) and feed the result into `Client::dispatch_host_request`. This tests the actual parsing logic and the match branches for boxing.
*   **Dual-Stage Approval**: Consider adding support for running the approval callback *before* the handler is called (to approve parameters and protect the LLM budget), or running it twice (pre-generation for budget authorization and post-generation for output safety).
*   **Improve Builder Ergonomics**: Instead of making the builder methods accept the exact `Arc` type aliases, make them generic:
    ```rust
    pub fn on_sampling_approval<F>(mut self, cb: F) -> Self
    where
        F: Fn(CreateMessageParams, CreateMessageResult) -> futures::future::BoxFuture<'static, ApprovalDecision> + Send + Sync + 'static
    {
        self.host_registry.approval = Some(Arc::new(cb));
        self
    }
    ```
*   **Use Existing Response Constructors**: Use the pre-existing [JSONRPCResponse::success](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/jsonrpc.rs#L226) and [JSONRPCResponse::error](file:///Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/jsonrpc.rs#L234) constructors in `dispatch_host_request` instead of instantiating the struct fields manually.

---

### 5. Risk Assessment

*   **Overall Risk Level: MEDIUM**

**Justification:** 
The feature additions are entirely additive (minor version bump) and do not break the legacy path. The routing logic and duplex tests are soundly designed and modeled on working paradigms. However, implementing the plans exactly as written will guarantee a compile-time lifetime failure on custom async approval callbacks (Concern 1) and will strip out custom developer-configured capabilities on initialization (Concern 3). Applying the suggestions outlined above will reduce the risk level to **LOW**.

---

## Consensus Summary

Orchestrator note: the load-bearing factual claims below were verified directly against the tree on 2026-07-17 before this synthesis (marked ✓ where checked).

### Agreed Strengths
- Core architecture is right: dispatch inside the existing `send_request` receive loop (nested-only, no background pump), parse-ambiguity handling for both `Request::Client`/`Request::Server` sampling variants, JSON-RPC error responses instead of connection teardown, immutable registry, distinct `client::host` naming, additive-only surface.
- Sequencing (dispatch → policy/capabilities → docs) and the duplex-test-first emphasis.

### Agreed Concerns (2 reviewers — highest priority)
1. **Approval hook is postflight and signature-broken (Codex HIGH, Gemini HIGH×2 + MEDIUM).** Running approval after the handler cannot prevent an unapproved/billed LLM call (T-106-05 not actually mitigated; denial-of-wallet), the callback signature `Fn(&CreateMessageParams) -> BoxFuture<'static, _>` can neither inspect the completion it supposedly reviews nor legally borrow the params into a `'static` future (compile-time lifetime violation), and by-value handler params conflict with borrowing afterward. Fix: preflight approval (owned/cloned params, before handler) as the mandatory gate; optional result-review stage with a result-aware signature.
2. **Fuzz target does not exercise dispatch (Codex HIGH, Gemini MEDIUM).** It only fuzzes `serde_json::from_value` on two param types. Either rename to match reality or fuzz `parse_request` → dispatch routing as a pure function.
3. **`RootsProvider` lacks an error channel (both MEDIUM).** Return `Result<ListRootsResult>` so provider failure can map to -32603 consistently with the other two handlers.

### Verified Single-Reviewer Blockers (Codex, orchestrator-confirmed ✓)
4. **✓ `server::traits` is not a compiled module** — `src/server/mod.rs` has no `mod traits;`; the file is dead code and the live trait is `server/mod.rs:353`. All plan/doc references to `pmcp::server::traits::SamplingHandler` would break the zero-warning rustdoc gate; use `pmcp::SamplingHandler`.
5. **✓ `pub mod roots` and `pub mod elicitation` are `#[cfg(not(target_arch = "wasm32"))]`** (`server/mod.rs:170-179`) — a "cfg-agnostic" `client::host` importing `ListRootsResult`/server elicitation types cannot compile on wasm32. Move/re-export the wire types target-agnostically or scope the wasm claim.
6. **✓ `PeerHandle` has no elicitation method** (`peer_impl.rs`: only `sample`/`list_roots`/`progress_notify`) — the planned high-level elicitation round-trip harness is not executable as written; needs a raw duplex pump or an explicit `PeerHandle` extension.
7. **Constructor coverage**: adding `host_registry` requires updating every `Client` constructor/`Clone`; plan lists only field+builder.
8. **Version-bump claim without a task**: no plan touches `Cargo.toml` for the promised 2.15.0→2.16.0; add a task or defer explicitly.
9. **`| tail` masks failures in verify commands across all three plans** — remove pipes or use `set -o pipefail`.

### Divergent Views
- **Capability derivation (HOST-05):** Gemini calls the registry-authoritative override "destructive" and proposes a merge — but the merge is exactly what the plan-checker rejected as violating the locked CONTEXT decision (no capability without a handler). Resolution that honors both: handler absent ⇒ field forced to `None` (locked, anti-capability-lie); handler present ⇒ preserve the caller's configured sub-capability detail, only inserting `default()` when the caller set none. Divergence goes to the locked decision; Gemini's sub-capability-preservation nuance is adopted within it.
- **Overall risk:** Codex HIGH (until blockers fixed) vs Gemini MEDIUM (LOW after fixes). Both agree the architecture is sound and the issues are plan-text-level, fixable before execution.
