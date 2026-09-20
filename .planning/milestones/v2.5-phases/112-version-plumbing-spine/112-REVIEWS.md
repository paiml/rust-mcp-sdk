---
phase: 112
reviewers: [gemini, codex]
reviewed_at: 2026-07-23T01:06:31Z
plans_reviewed: [112-09-PLAN.md, 112-10-PLAN.md]
scope: gap_closure
---

# Cross-AI Plan Review — Phase 112 (Gap Closure)

Two external models independently reviewed the gap-closure plans `112-09` (Gap B+C: `_meta`/`ProtocolContext` threading) and `112-10` (Gap A: `server/discover` reachability). Both plans had already passed the internal `gsd-plan-checker` (after one revision round). This external pass is deliberately adversarial.

**Headline:** Gemini rated both plans LOW risk and execution-ready. Codex rated both HIGH risk / **not** execution-ready, citing concrete correctness and security gaps. Where the two diverged, the orchestrator verified the disputed claims directly against the source tree — **Codex's two most severe findings are CONFIRMED real** (see Verification Notes). Recommendation: revise via `/gsd:plan-phase 112 --reviews` before executing.

## Gemini Review

# Cross-AI Plan Review: Phase 112 Gap-Closure Plans (112-09 & 112-10)

An independent, adversarial review has been performed on implementation plans **112-09** and **112-10** for the `pmcp` Rust SDK. Below is the detailed evaluation, scoping judgment, risk analysis, and recommendations.

---

## 1. Judgment on Scoping Decision (`server/discover` HTTP-Only)

**Verdict: HONEST AND ACCEPTABLE.**

* **Spec & Architecture Realities:** In MCP 2026-07-28 (v2), version negotiation headers (`MCP-Protocol-Version`, `Mcp-Method`, `Mcp-Name`) operate exclusively on HTTP transport ingress. Stdio transports communicate via raw line-delimited JSON-RPC frames without HTTP headers.
* **Criterion Satisfaction:** Success Criterion #3 explicitly requires `server/discover` to be reachable *"over at least one real transport."* Delivering this end-to-end on Streamable HTTP satisfies SC#3 directly.
* **Semver Guarantee:** Modifying `TransportMessage` to introduce an internal message variant would require matching updates across ~30 call sites and public/internal adapter boundaries, introducing a high risk of breaking public API contracts. Deferring stdio interception guarantees that `cargo-semver-checks` continues to classify the release as **MINOR (additive 2.x)**.
* **Fallback Behavior:** Stdio clients sending `server/discover` continue to receive standard JSON-RPC `-32601` (Method Not Found) via `parse_request()`, maintaining backward compatibility and byte-identity.

---

## 2. Review of Plan 112-09 (Gap B + C: `_meta`/`ProtocolContext` Threading)

### Summary
Executing Plan 112-09 will successfully make requirements **VERS-01, VERS-03, VERS-05, VERS-07, and VERS-09** `TRUE`. By extending `extract_request_meta_value` to match `ClientRequest::GetPrompt` and `ClientRequest::ReadResource`, the plan fixes the root cause preventing the HTTP header gate from recognizing v2 requests for these methods (Gap C). Threading `protocol_context` and `request_meta` into `RequestHandlerExtra` across both `ServerCore` (`core.rs`) and `Server` (`mod.rs`) ensures that prompt and resource handlers receive resolved `ProtocolContext` and `RequestMeta` (Gap B).

### Strengths
* **Root-Cause Alignment:** Instead of adding ad-hoc patches in the HTTP layer, the plan fixes the underlying ingress metadata extraction (`extract_request_meta_value`) and dispatch context construction.
* **Twin-Site Parity:** Explicitly updates both native dispatch entry points (`src/server/core.rs` and `src/server/mod.rs`), preventing divergence between low-level core dispatch and high-level server dispatch.
* **Non-Mutating Envelope Reuse:** Reuses the existing generic `inject_v2_result_envelope` without modifying response serialization for v1 clients.
* **Discriminating Acceptance Criteria:** Requires exact, scoped `grep` assertions and includes property tests for `RequestMeta` round-tripping and live HTTP integration tests for both accepted v2 calls and golden v1 byte-identity.

### Concerns

#### [MEDIUM] Lack of Compiler Enforcement for Future `ClientRequest` Variants
* **Issue:** `extract_request_meta_value` uses a wildcard match arm `_ => None`. If a future MCP method is added to `ClientRequest` that carries `_meta`, the compiler will not force `extract_request_meta_value` to be updated.
* **Impact:** Future name-bearing or meta-carrying methods could silently suffer from the same gap.
* **Mitigation:** While documented in the function's doc comments, an explicit match on all non-meta `ClientRequest` variants (or a unit test verifying all `ClientRequest` variants that contain `_meta` are handled) would provide compile-time or test-time protection.

#### [LOW] Extra Clone of `Option<ProtocolContext>` in Dispatch Arms
* **Issue:** `handle_request_internal` clones `protocol_context` into `handle_get_prompt` and `handle_read_resource`.
* **Impact:** `ProtocolContext` is a lightweight struct containing small heap allocations (strings for implementation info). The performance impact is negligible, but unnecessary cloning could be avoided by passing references `Option<&ProtocolContext>` where owned values are not required.

### Suggestions for Plan 112-09
1. **Exhaustive Variant Test:** Add an explicit unit test `test_all_client_requests_with_meta_are_extracted()` that uses a dummy instance of every `ClientRequest` variant to assert that any variant possessing a `pub _meta` field yields `Some` when passed to `extract_request_meta_value`.

### Risk Assessment
* **Overall Risk:** **LOW**
* **Justification:** Edits are additive, well-bounded, and mirror the existing, proven `handle_call_tool` pattern. V1 byte-identity is strictly preserved via era-gating.

---

## 3. Review of Plan 112-10 (Gap A: Production Reachability of `server/discover`)

### Summary
Executing Plan 112-10 will successfully make requirement **VERS-04 (SC#3)** `TRUE`. By intercepting incoming HTTP POST bodies prior to `parse_transport_message_fast`/`parse_transport_message_with_middleware`, the plan routes `server/discover` requests through `parse_request_or_internal` to `Server::handle_discover` and the shared `build_discover_response` projection. Deleting `ServerCore::handle_discover` and `ServerCore::dispatch_internal_client_request` removes all `#[allow(dead_code)]` attributes, satisfying the Toyota Way / zero-SATD quality bar.

### Strengths
* **Clean Architectural Refactoring:** Consolidating discovery logic into `build_discover_response` eliminates duplicated capability projection code between high-level `Server` and internal core.
* **Elimination of Dead Code Annotations:** Completely deletes the unreachable `ServerCore` wrapper methods and migrates unit tests, removing `#[allow(dead_code)]` attributes and avoiding compiler warning suppression.
* **Non-Panicking Body Inspection:** Uses the established `from_slice` pattern (modeled after `extract_body_method_and_name`) so malformed or non-JSON payloads safely bypass interception and fall back to standard 400 error handling.
* **Read-Only Capability Projection:** Maintains strict state immutability during discovery (no mutation of `is_initialized` state or recomputation of capabilities).

### Concerns

#### [MEDIUM] HTTP Header Gate Bypassing on Intercepted `server/discover` Path
* **Location:** `src/server/streamable_http_server.rs` (`try_route_internal_request`)
* **Issue:** `try_route_internal_request` resolves the era directly from raw `_meta` via `resolve_protocol_context`. However, for v2 requests, it does not explicitly validate incoming HTTP headers (`MCP-Protocol-Version` and `Mcp-Method: server/discover`) against `run_v2_header_gate`.
* **Impact:** A client sending a v2 `_meta` payload over HTTP with invalid or conflicting HTTP headers (e.g. `MCP-Protocol-Version: 2025-11-25`) might be accepted by `try_route_internal_request`, whereas a `tools/call` request with the same header mismatch would be rejected with HTTP 400 by the header gate.
* **Mitigation:** Ensure `try_route_internal_request` checks `run_v2_header_gate` or verifies that `MCP-Protocol-Version` in the HTTP header matches the era resolved from `_meta`.

#### [LOW] Locking & Deadlock Verification
* **Location:** `src/server/streamable_http_server.rs`
* **Analysis:** `try_route_internal_request` locks `state.server` to read `supported_protocol_versions`, `capabilities`, and `info`. Because `try_route_internal_request` is invoked at the very entry of the POST handler *before* any other locks are acquired, and returns `Some(response)` immediately upon match, the lock is released before the function returns. If it returns `None`, the lock is released before the standard dispatch path runs.
* **Verdict:** **Zero deadlock or double-lock risk.**

#### [LOW] Cognitive Complexity Budget (PMAT cog ≤25)
* **Location:** `src/server/streamable_http_server.rs`
* **Issue:** Modifying the HTTP POST handler entry point risks exceeding the cognitive complexity limit if logic is inline.
* **Mitigation:** The plan explicitly mandates breaking down `try_route_internal_request` into small helper functions (raw body re-parse, era resolution, and discover response assembly). This keeps cognitive complexity well under 25 per function.

### Suggestions for Plan 112-10
1. **Header Gate Validation in `try_route_internal_request`:** Include an explicit check inside `try_route_internal_request` to verify that if `_meta` resolves to `Era::V2`, the HTTP request header `MCP-Protocol-Version` is also present and valid (`2026-07-28`), returning HTTP 400 rejection otherwise.
2. **Batch Request Guard:** Add an explicit inline comment and test confirming that array payloads (`[ { "method": "server/discover" } ]`) fail single-request deserialization in `try_route_internal_request`, returning `None` to let standard HTTP request validation reject JSON-RPC batching.

### Risk Assessment
* **Overall Risk:** **LOW to MEDIUM**
* **Justification:** Early POST path interception is clean and isolates discovery from standard request parsing. Deleting dead code improves quality gate compliance. Addressing the HTTP header gate consistency concern reduces the risk to **LOW**.

---

## 4. Combined Synthesis & Verification Matrix

Executing both plans sequentially will close all 3 identified gaps:

| Requirement | Description | Plan | Status Post-Execution | Verification Method |
| :--- | :--- | :--- | :--- | :--- |
| **VERS-01** | Thread `ProtocolContext` through dispatch | 112-09 | **SATISFIED** | Unit & Handler visibility tests in `core.rs` / `mod.rs` |
| **VERS-03** | Thread `request_meta` to `RequestHandlerExtra` | 112-09 | **SATISFIED** | Unit & Handler visibility tests |
| **VERS-04** | `server/discover` capability discovery | 112-10 | **SATISFIED** | E2E HTTP integration tests in `v2_required_headers.rs` |
| **VERS-05** | Enforce v2 HTTP headers on name-bearing methods | 112-09 | **SATISFIED** | Live HTTP matrix tests (`prompts/get`, `resources/read`) |
| **VERS-07** | Inject v2 envelope (`resultType`, `serverInfo`) | 112-09 | **SATISFIED** | Assertions on HTTP JSON responses for prompt & resource calls |
| **VERS-09** | Surface era & client info inside handlers | 112-09 | **SATISFIED** | Extra inspector assertions in handler tests |

### Quality Gate & Invariant Checklist
* **`make quality-gate`:** Passing (zero clippy warnings under pedantic+nursery, cognitive complexity ≤25 per fn, zero SATD).
* **v1 Wire Byte-Identity:** Guaranteed and tested via golden fixtures in `tests/v2_required_headers.rs`.
* **Semver Invariant:** `cargo semver-checks` will classify the release as **MINOR** (no breaking changes or public enum modifications).

---

## Work Summary
* Conducted an adversarial review of implementation plans **112-09** and **112-10**.
* Confirmed that HTTP-only scoping for `server/discover` is honest, acceptable, and preserves semver MINOR guarantees.
* Verified that Plan 112-09 closes Gap B and Gap C without introducing v1 wire identity regressions.
* Verified that Plan 112-10 closes Gap A and cleans up dead code without deadlock risk.
* Highlighted key recommendations for HTTP header gate consistency in `server/discover` and test exhaustiveness for `ClientRequest` variants.

---

## Codex Review

# Independent adversarial review

## Executive conclusion

Neither plan is execution-ready as written.

- **112-09 correctly identifies and fixes the missing `ProtocolContext` threading**, but it does not fully close Gap C for `resources/read`: the live header gate cross-checks `Mcp-Name` against `params.name`, while a standard `ReadResourceRequest` has `params.uri`, not `params.name`. Its “golden byte-identity” tests also do not compare bytes.
- **112-10 chooses an acceptable HTTP-only scope**, but the proposed early interception bypasses important parts of both HTTP pipelines and does not apply the existing v2 header-classification matrix. It also conflicts with the actual current HTTP behavior for v1 `server/discover`, so its byte-identity claim is presently false.

The underlying design is recoverable without adding a public `TransportMessage` variant, but both plans need revision before execution.

---

# Plan 112-09

## Summary

The production edits to `extract_request_meta_value` and the four prompt/resource handler sites are fundamentally correct and should make `extra.era()`, `extra.client_info()`, and `extra.trace_context()` available when requests reach those handlers. They should also enable v2 result-envelope injection for prompt and resource results. However, the plan will not make the stated live `resources/read` acceptance criterion true for a standards-shaped request because the existing HTTP gate extracts only `params.name`. The plan also overstates its v1 byte-identity proof and contains an incorrect expectation for opted-in v1 handlers. Therefore, it only partially re-satisfies VERS-01/03/07/09 and does not reliably re-satisfy VERS-05.

## Strengths

- The root cause for handler context loss is accurately located:

  - `ServerCore` dispatch drops context at [src/server/core.rs:1582](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/core.rs:1582) and the corresponding resource arm.
  - High-level `Server` drops it at [src/server/mod.rs:1438](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs:1438) and [src/server/mod.rs:1445](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/mod.rs:1445).
  - Both handler implementations construct incomplete `RequestHandlerExtra` values.

- Mirroring the existing `tools/call` builder chain is low-risk and preserves the established architecture.

- Extending `extract_request_meta_value` is the right ingress fix. The relevant request types all use `Option<RequestMeta>`, and `RequestMeta`’s flattened map preserves reserved v2 identity and tracing keys.

- The envelope claim is sound once context is threaded: both dispatch surfaces already call the shared era-gated injector, and it is a no-op unless `Era::V2`.

- Requiring tests at both dispatch surfaces is valuable. The twin implementations are a demonstrated drift risk.

- The documented go-forward rule for future `_meta`-bearing variants is useful and currently exhaustive: only call-tool, get-prompt, and read-resource carry `RequestMeta`.

## Concerns

### HIGH — `resources/read` still fails the real header gate

The gate extracts only `params.name` at [src/server/streamable_http_server.rs:527](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:527), and `cross_check_name` requires it for `resources/read`.

But `ReadResourceRequest` contains:

```rust
pub uri: String,
pub _meta: Option<RequestMeta>,
```

There is no `name` field at [src/types/resources.rs:160](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/types/resources.rs:160).

Consequences:

- A typed, standards-shaped `resources/read` request will produce `body_name == None`.
- A full v2 request will still be rejected by `cross_check_name`.
- A test can accidentally conceal this defect by inserting a nonstandard extra `params.name`; serde will likely ignore it during typed parsing while the raw-body gate consumes it.
- `src/server/streamable_http_server.rs` is not even listed under Plan 112-09’s modified files.

This means the plan does not actually close Gap C for compliant resource requests.

### HIGH — the proposed v1 tests do not prove byte identity

The tests described as “golden byte-identity fixtures” merely assert that `resultType` and `serverInfo` are absent.

That proves only two keys were not added. It does not catch changes in:

- field ordering;
- omitted/default fields;
- error/result shape;
- response ID;
- whitespace or serializer behavior;
- content headers;
- other newly emitted metadata.

The existing `Resp` test helper parses JSON and discards the raw response text, making byte comparison impossible.

### MEDIUM — the opted-in v1 handler expectation is wrong

The plan says:

> on a v1 / non-opted-in server, the same handlers see `extra.era()==None`

Those are different cases:

- Non-opted-in server: `resolve_ingress_protocol_context` returns `None`.
- Opted-in server receiving no v2 signal: the resolver selects a v1 version and returns `Some(ProtocolContext { era: Era::V1, ... })`.

The test matrix must distinguish `None` from `Some(Era::V1)`.

### MEDIUM — handler tests may not prove the dispatch arms were repaired

The acceptance language permits a test that directly calls `handle_get_prompt` or `handle_read_resource` with a context. Such a test would pass even if `process_client_request` or `handle_request_internal` still dropped the context.

The tests must enter above the affected dispatch match arms.

### MEDIUM — trace propagation is promised but not required by acceptance

The plan’s behavior and success criteria claim `trace_context()` is live, but the named acceptance criterion explicitly requires only `era` and `client_info`.

A regression that omits `.with_request_meta(...)` but retains `.with_protocol_context(...)` could therefore pass the stated handler acceptance check. That would leave trace propagation broken.

### MEDIUM — the automated test command is malformed

This command supplies two positional libtest filters:

```bash
cargo test --lib --features full -- extract_request_meta_value prompt_resource_protocol_context
```

Standard Rust’s test harness accepts one filter; the second positional value is generally rejected. Use separate commands or a single common substring.

### MEDIUM — project-mandated fuzz coverage is absent

The plan adds a proptest but no fuzz target or explicit fuzz verification, despite the stated phase quality requirement of unit + property/fuzz + integration coverage for changed behavior.

### LOW — unnecessary cloning instructions obscure ownership

The dispatch match arms are mutually exclusive, so some prescribed `protocol_context.clone()` calls may be unnecessary. This is not a correctness problem, but execution should let the compiler guide the minimum cloning.

## Suggestions

1. Add `src/server/streamable_http_server.rs` to Plan 112-09 and make logical-name extraction method-aware:

   - `tools/call` → `params.name`
   - `prompts/get` → `params.name`
   - `resources/read` → `params.uri`
   - name-less methods → presence-only

2. Build the resource body strictly from `ReadResourceRequest` and do not add a synthetic `name` property. Set `Mcp-Name` to the URI and prove the gate accepts it.

3. Extend `Resp` with the exact raw response bytes or string. Pin baseline fixtures captured from the current v1 implementation and compare exact bytes.

4. Split handler expectations into:

   - non-opted-in → `era() == None`;
   - opted-in v1 fallback → `era() == Some(Era::V1)`;
   - opted-in v2 → `era() == Some(Era::V2)`.

5. Enter through the real dispatch entrypoints, not the private leaf handlers.

6. Put protocol version, `clientInfo`, `traceparent`, `tracestate`, and baggage into `_meta`; assert all relevant accessors inside both prompt and resource handlers.

7. Replace the verification command with separate invocations, for example:

```bash
cargo test --lib --features full extract_request_meta_value
cargo test --lib --features full prompt_resource_protocol_context
```

8. Add the required fuzz/property coverage for raw metadata extraction and malformed reserved values.

## Risk Assessment

**HIGH**

The context-threading implementation itself is low-to-medium risk, but the plan’s live resource acceptance test is built on a false assumption about `params.name`. As written, execution can either fail the acceptance test or make it pass using a nonstandard request that conceals the remaining production bug. The byte-identity claim is also not actually tested.

---

# Plan 112-10

## Summary

HTTP-only delivery is an honest and acceptable interpretation of SC#3: it provides one real production transport while avoiding a public exhaustive-enum change that could violate the MINOR requirement. Deleting the crate-private wrappers and consolidating the projection is also semver-safe. The proposed HTTP implementation, however, is under-specified at the most critical boundary. Intercepting immediately before transport parsing is necessary, but returning a response there bypasses the normal header gate, session validation, authentication, event storage, and portions of middleware response processing. It also changes actual current v1 HTTP behavior while claiming byte identity. Consequently, Plan 112-10 does not safely re-satisfy VERS-04 as written.

## Strengths

- The plan correctly recognizes that interception must happen before `StdioTransport::parse_message`. That parser calls public `parse_request`, which collapses the internal request before dispatch.

- HTTP-only scope is reasonable:

  - SC#3 says “at least one real transport.”
  - The v2 header contract is HTTP-specific.
  - Avoiding a public `TransportMessage` variant substantially reduces semver risk.

- `build_discover_response` is a good consolidation boundary:

  - one era gate;
  - one capability projection;
  - one envelope injection;
  - no capability recomputation;
  - no initialization mutation.

- Deleting the two `pub(crate)` `ServerCore` methods does not remove public API and should not create a semver break.

- Matching the crate-private `InternalClientRequest` directly in the HTTP layer is acceptable; that enum can grow without public exhaustive-match impact.

- The plan explicitly calls for decomposing raw parsing, era resolution, and response building, which is appropriate for the cognitive-complexity limit.

- A short-lived `state.server` lock followed by synchronous `Server::handle_discover` is not inherently deadlocking.

## Concerns

### HIGH — the internal path bypasses the v2 header gate

The proposed helper resolves the era from raw `_meta`, but it does not explicitly call `classify_v2_request` or enforce the existing matrix at [src/server/streamable_http_server.rs:490](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:490).

As written, it may accept:

- v2 `_meta` with no `MCP-Protocol-Version` header;
- v2 header with non-v2 or absent `_meta`;
- a mismatched `Mcp-Method`;
- missing `Mcp-Name`;
- malformed or oversized v2 headers.

Simply echoing three headers on the response is not equivalent to enforcing them on ingress.

### HIGH — early return bypasses authentication

On the fast path, authentication occurs only after parsing, session resolution, and header gating at [src/server/streamable_http_server.rs:1583](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:1583).

On the middleware path, authentication occurs at [src/server/streamable_http_server.rs:1890](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:1890).

An interception immediately before the parse calls returns before both. Even a read-only capability projection may be protected by an installed auth provider. The threat model’s assertion that no auth-gated data is added does not justify bypassing the server’s configured request boundary.

### HIGH — the proposed placement bypasses session and protocol validation

The helper is to run before:

- `resolve_session_for_request`;
- legacy protocol-version validation;
- response session-ID handling;
- event storage.

That can make `server/discover` behave differently from every other HTTP method in stateful mode, including accepting invalid or unknown session IDs.

### HIGH — middleware behavior is not preserved

In the middleware path, interception before `parse_transport_message_with_middleware` occurs after request middleware but before normal response middleware assembly and error hooks.

The plan says to use “existing success-response assembly,” but the available middleware response builder requires:

- a `TransportMessage`;
- resolved response session ID;
- computed outbound version;
- middleware/context arguments.

Those values are not all available at the proposed interception point. A single generic early-return helper cannot transparently preserve both fast and middleware response semantics without more explicit factoring.

### HIGH — the v1 byte-identity assertion conflicts with current HTTP behavior

Current fast-path parsing does this:

1. `StdioTransport::parse_message`;
2. public `parse_request`;
3. internal method becomes method-not-found error;
4. `parse_transport_message_fast` maps that parse failure to an HTTP 400 `PARSE_ERROR` response with `id: null`.

See [src/server/streamable_http_server.rs:1412](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/streamable_http_server.rs:1412).

The middleware path has a different parse-error assembly again.

The proposed interception would instead return a normal serialized `JSONRPCResponse` containing `-32601`, probably HTTP 200 and with the original request ID. That may be a better semantic result, but it is not byte-identical to the existing HTTP path.

The plan must resolve the conflict between:

- “standard `-32601`,” and
- “actual existing HTTP bytes remain unchanged.”

It cannot merely assert both.

### MEDIUM — non-opted-in short-circuiting is ambiguous

Calling `resolve_protocol_context` directly on a non-opted-in v1 accept list with v2 `_meta` yields an unsupported-version error. Existing ingress behavior first checks `is_v2_opted_in` and returns `Ok(None)` without inspecting v2 metadata.

The helper must preserve that exact order:

```text
if not opted in:
    no era detection; legacy handling
else:
    resolve raw metadata
```

The non-opted-in test may catch this, but the implementation instruction should be explicit.

### MEDIUM — tests do not exercise the dangerous pathways

All proposed end-to-end tests use stateless HTTP without middleware or authentication. They will not detect:

- authentication bypass;
- invalid session acceptance;
- response-middleware bypass;
- event-store omission;
- fast/middleware parity;
- header-matrix bypass.

### MEDIUM — rejection tests are too weak

The discover tests cover a happy v2 request and two v1/non-opted-in cases. They do not test:

- v2 `_meta` without v2 headers;
- v2 header without v2 `_meta`;
- missing `Mcp-Method`;
- missing `Mcp-Name`;
- mismatched `Mcp-Method`;
- response header echo, including empty `Mcp-Name`;
- preservation of the JSON-RPC request ID.

### MEDIUM — response assembly is not concrete enough

The plan should specify that `JSONRPCResponse` becomes `TransportMessage::Response` and then passes through the same per-path response pipeline. Otherwise, an executor could serialize the inner response differently, omit event storage, or miss SSE/session behavior.

### LOW — stale documentation remains outside the listed files

Comments in `src/types/protocol/mod.rs` and `src/shared/protocol_helpers.rs` still describe `dispatch_internal_client_request` as the consumer. Deleting that function without updating these comments leaves architectural documentation inaccurate.

### LOW — locking is manageable, but lock scope should be explicit

There is no necessary double-lock if the helper:

1. acquires `state.server`;
2. resolves context and synchronously builds the response;
3. releases the guard;
4. runs event storage and response middleware afterward.

It should not hold the server mutex while awaiting middleware, event storage, or other transport work.

## Suggestions

1. Do not implement discover as an immediate early response. Introduce a private HTTP-local ingress classification, such as:

```rust
enum HttpIngress {
    Public(TransportMessage),
    Discover {
        id: RequestId,
        raw_meta: Option<Value>,
    },
}
```

This is crate-private/local and therefore avoids the public `TransportMessage` semver problem.

2. Let both POST pipelines continue through their normal stages after classification:

```text
body limit
request middleware
header validation
raw ingress classification
session resolution
v2 header matrix
legacy version validation
authentication
dispatch
event storage
normal fast/middleware response assembly
```

3. Factor a raw-meta counterpart to `run_v2_header_gate` that:

- short-circuits non-opted-in servers;
- calls the shared resolver once;
- invokes the same `classify_v2_request`;
- returns the same `V2GateOutcome`.

4. For `server/discover`, treat `Mcp-Name` as presence-only, consistent with `cross_check_name` for name-less methods. Explicitly test an empty value if that is the intended canonical value.

5. Decide the v1 compatibility rule using a captured pre-change HTTP fixture:

   - If literal byte identity is mandatory, fall through to the old parse path for non-v2/non-opted-in requests.
   - If standardizing HTTP on `-32601` is intentional, document it as a deliberate v1 behavior change and reconcile that with the milestone invariant before implementation.

6. Add tests for both fast and middleware paths, including a response middleware that visibly modifies or records the discover response.

7. Add an auth provider test proving unauthorized discover does not bypass authentication.

8. Add a stateful/session test for invalid session IDs.

9. Add the full discover header matrix and assert status, error code, original ID, and all outbound headers.

10. Update stale documentation referencing the deleted wrapper functions.

11. Add property/fuzz coverage for raw-body internal classification and `_meta` extraction, particularly malformed JSON, non-object params, oversized metadata, and unusual IDs.

## Risk Assessment

**HIGH**

The internal method can be made reachable over HTTP without a public enum change, and the lock itself is not the main risk. The high risk comes from inserting a parallel request pipeline that skips established security and transport invariants. The current tests would not detect most of those regressions.

---

# Cross-plan assessment

## Will both plans together make all blocked requirements true?

Not yet.

- **VERS-01/03/07/09:** likely true after the core 112-09 edits, provided tests enter through the real dispatch arms and assert tracing as well as era/client identity.
- **VERS-05:** still false for a standard `resources/read` request until the HTTP gate derives its logical name from `params.uri`.
- **VERS-04:** discover becomes reachable only if 112-10’s interception is integrated into—not returned ahead of—the existing HTTP pipeline.

## HTTP-only discover scope

The scoping is honest and acceptable. SC#3 explicitly requires at least one real transport, and avoiding a public `TransportMessage` variant is a sensible semver tradeoff.

It should be documented as a transport support limitation:

- HTTP v2: supported.
- stdio/adapters/WASI: method remains unavailable.
- Future extension requires a separate semver-reviewed design.

That is not a hidden gap unless broader transport parity is stated elsewhere as a requirement.

## Required plan changes before execution

At minimum:

1. Add method-aware URI extraction for `resources/read`.
2. Replace “absence of two keys” with actual raw-byte v1 fixtures.
3. Correct `None` versus `Some(Era::V1)` expectations.
4. Refactor HTTP parsing into a private ingress classification that preserves the rest of both POST pipelines.
5. Apply the complete v2 header matrix to discover.
6. Preserve auth, sessions, middleware, event storage, response ID, and response assembly.
7. Resolve the contradiction between actual current HTTP v1 behavior and the desired `-32601` behavior.
8. Add discriminating fast/middleware, auth, session, malformed-input, and header-matrix tests.

**Overall combined risk: HIGH until revised; MEDIUM after these corrections.**

---

## Consensus Summary

### Verification Notes (orchestrator, checked against live source)

The two reviewers disagreed sharply on severity, so the disputed claims were checked against the code rather than taken on faith:

| Claim | Source check | Verdict |
|-------|--------------|---------|
| **112-09: a standards-shaped v2 `resources/read` is still rejected** — the HTTP gate cross-checks `Mcp-Name` against `params.name`, but `ReadResourceRequest` has `uri`, not `name` | `src/types/resources.rs:160` (`pub uri: String`, no `name`); `cross_check_name` + `is_name_bearing_method` include `resources/read` (`streamable_http_server.rs:470,486`); `extract_body_method_and_name` reads `params.get("name")` (`:537`) | **CONFIRMED.** `body_name == None` → `cross_check_name` returns `Err` → REJECT. `prompts/get` is unaffected (`GetPromptRequest.name` exists, `prompts.rs:276`). 112-09 does not list `streamable_http_server.rs` in `files_modified`. Gap C is not closed for `resources/read` as written. |
| **112-10: v1 `server/discover` byte-identity claim is self-contradictory** — current HTTP path returns a `PARSE_ERROR` 400 (`id: null`), not a clean `-32601` | `PARSE_ERROR` + `StatusCode::BAD_REQUEST` + `"id": null` present (`streamable_http_server.rs:342,655,1412`) | **CONFIRMED plausible.** Plan cannot claim both "returns `-32601`" and "byte-identical to existing behavior." One must be chosen and documented as a deliberate (benign) v1 HTTP change. |
| **112-10: early-return interception bypasses auth/session/middleware** | Architectural: `try_route_internal_request` returns `Some(response)` at the top of the POST handler, before the dispatch pipeline where auth/session/header-gate/event-store run | **SOUND.** An early return before the pipeline necessarily precedes those stages. Security-relevant if an auth provider is installed. |
| **112-09: `cargo test -- filter1 filter2` is malformed (harness accepts one filter)** | — | **LOW CONFIDENCE / likely incorrect.** Modern libtest accepts multiple positional filters (OR semantics). Harmless to split the command anyway, but not a real blocker. |

### Agreed Strengths (both reviewers)

- **112-09 fixes the correct root cause.** Extending `extract_request_meta_value` + mirroring the proven `handle_call_tool` builder chain at all four prompt/resource handler sites is low-risk and architecturally consistent; the era-gated envelope injector means v1 output is untouched in principle.
- **112-09's twin-site discipline** (repairing both `core.rs` and `mod.rs`) correctly targets a demonstrated drift risk.
- **112-10's HTTP-only `server/discover` scope is honest and acceptable.** Both agree SC#3 ("at least one real transport") is satisfied by HTTP, and that avoiding a public `TransportMessage` variant is the right call to keep the release semver-MINOR. Should be documented as an explicit transport-support limitation (HTTP: yes; stdio/adapters/WASI: not yet).
- **112-10's `build_discover_response` consolidation + wrapper deletion** is a clean, semver-safe way to remove the `#[allow(dead_code)]` (deleting `pub(crate)` methods is not a public-API break).
- **No deadlock risk** in 112-10's short-lived `state.server` lock (both agree; Gemini explicitly, Codex with the caveat to not hold the guard across await).

### Agreed Concerns (raised by both — highest priority)

1. **[HIGH] The intercepted `server/discover` path does not apply the v2 header-classification matrix.** Both reviewers flag this (Gemini MEDIUM, Codex HIGH): resolving era from raw `_meta` alone means a v2-`_meta`/conflicting-header (or missing-header) request could be accepted on the discover branch where `tools/call` would be rejected 400. Fix: run discover through the same `classify_v2_request`/header gate (a raw-`_meta` counterpart to `run_v2_header_gate`).

### Divergent Views (Codex-only, verified real → must address)

2. **[HIGH — 112-09] `resources/read` name cross-check uses `params.name`, which the request doesn't have.** CONFIRMED above. Fix: make logical-name extraction method-aware (`resources/read` → `params.uri`), add `streamable_http_server.rs` to 112-09's scope, and build the test body from a real `ReadResourceRequest` (no synthetic `params.name` that would hide the bug).
3. **[HIGH — 112-10] Early-return interception bypasses auth / session / legacy-version validation / response middleware / event storage, and its "success-response assembly" needs values not available pre-parse.** Fix (Codex's design): don't early-return — introduce a crate-*local* `HttpIngress::{Public(TransportMessage), Discover{id, raw_meta}}` classification and let both POST pipelines continue through their normal stages (header gate → session → auth → dispatch → event store → per-path response assembly).
4. **[HIGH — 112-10] v1 byte-identity claim contradicts current HTTP behavior** (PARSE_ERROR 400 vs `-32601`). CONFIRMED above. Resolve explicitly: either fall through to the old parse path for v1/non-opted-in, or document the `-32601` HTTP 200 as a deliberate v1 behavior change reconciled against the milestone's byte-identity invariant.
5. **[MEDIUM — 112-09] Test rigor:** "golden byte-identity" tests only assert two keys are absent (the `Resp` helper discards raw bytes — no real byte comparison); `era()==None` conflates non-opted-in (`None`) with opted-in-v1 (`Some(Era::V1)`); handler-visibility tests must enter through the real dispatch arms (not call leaf handlers directly) or they won't prove the dispatch fix; acceptance requires only `era`+`client_info` so a dropped `.with_request_meta` (trace regression) could pass; project-mandated **fuzz** coverage is absent (only proptest).
6. **[MEDIUM — 112-10] Test rigor:** all e2e tests are stateless/no-middleware/no-auth, so they can't detect the bypass regressions; the rejection matrix (v2-`_meta`/no-header, header/no-`_meta`, missing/mismatched `Mcp-Method`/`Mcp-Name`, request-id preservation) is untested; stale doc comments referencing the deleted wrappers remain in `types/protocol/mod.rs` + `shared/protocol_helpers.rs`.

### Minor (single-reviewer, low severity)

- [MED — Gemini] `extract_request_meta_value`'s `_ => None` wildcard gives no compile-time tripwire for a future `_meta`-bearing `ClientRequest` variant — add an exhaustive-variant test. (Codex notes the go-forward doc rule is currently exhaustive: only call-tool/get-prompt/read-resource carry `RequestMeta`.)
- [LOW — both] Unnecessary `protocol_context.clone()` in mutually-exclusive dispatch arms; let the compiler guide minimum cloning.

### Bottom line

Both plans close the *conceptual* root causes, but **112-09 leaves `resources/read` (VERS-05) actually broken** and **112-10's interception design has real security/behavioral gaps and a contradictory v1 claim**. Neither is execution-ready as written. Recommended: `/gsd:plan-phase 112 --reviews` to fold findings #1–#6 into the plans, then re-check and execute.
