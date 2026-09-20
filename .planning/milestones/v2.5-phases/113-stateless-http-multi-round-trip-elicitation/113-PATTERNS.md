# Phase 113: Stateless HTTP + Multi-Round-Trip Elicitation - Pattern Map

**Mapped:** 2026-07-24
**Files analyzed:** 24 (9 new, 15 modified)
**Analogs found:** 24 / 24

> **Read this before planning.** Every new file in this phase has a concrete
> in-repo analog. Phase 112 built the era-gate spine *in the same files this phase
> edits*, so most "analogs" are **in-file precedents** — the strongest possible
> match (same file, same author intent, same review pass). Copy those verbatim
> rather than inventing a parallel mechanism.

---

## File Classification

### New files

| New File | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `src/server/request_state.rs` | utility (crypto) | transform (mint/verify token) | `src/shared/pkce.rs` | exact |
| `src/types/mrtr.rs` | model (protocol types) | transform (wire adapter) | `src/types/elicitation.rs` | exact |
| `tests/common/v2.rs` (shared helper) | test (harness) | request-response | `tests/v2_required_headers.rs:236-320` | exact |
| `tests/v2_stateless_http.rs` | test (integration, live HTTP) | request-response | `tests/v2_required_headers.rs` | exact |
| `tests/v2_mrtr.rs` | test (integration, live HTTP) | request-response | `tests/v2_required_headers.rs` | exact |
| `tests/v2_subscriptions.rs` | test (integration, SSE) | streaming | `tests/v2_required_headers.rs` + `tests/server_subscriptions.rs` | role-match |
| `tests/v2_client.rs` | test (integration, client) | request-response | `tests/v2_required_headers.rs` | exact |
| `fuzz/fuzz_targets/fuzz_request_state.rs` | test (fuzz) | transform | `fuzz/fuzz_targets/pkce_helper.rs` | exact |
| `examples/s47_v2_stateless_mrtr.rs` | example (runnable server) | request-response | `examples/t05_streamable_http_stateless.rs` | exact |

### Modified files

| Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------|------|-----------|----------------|---------------|
| `src/server/streamable_http_server.rs` | transport/router | request-response + streaming | itself, `:348-629` (Phase 112 gate) | in-file precedent |
| `src/server/core.rs` | controller (dispatch) | request-response | itself, `:1087-1224` | in-file precedent |
| `src/server/mod.rs` | controller (twin dispatch) + builder | request-response | itself, `:1414-1423` and `:2544-2558` | in-file precedent |
| `src/server/builder.rs` | builder (config) | — | `src/server/mod.rs:2544-2558` | exact |
| `src/server/subscriptions.rs` | service (registry) | pub-sub | itself + `streamable_http_server.rs:2342-2394` | role-match |
| `src/types/tools.rs` | model | — | itself, `:464-471` (`task` field) | in-file precedent |
| `src/types/prompts.rs` | model | — | `src/types/tools.rs:464-471` | exact |
| `src/types/resources.rs` | model | — | `src/types/tools.rs:464-471` | exact |
| `src/types/protocol/error_codes.rs` | config (constants table) | — | itself, `:65-162` | in-file precedent |
| `src/types/elicitation.rs` | model (serde) | transform | `src/types/resources.rs:356` + `protocol/mod.rs:561` | role-match |
| `src/types/mod.rs` | config (module registry) | — | itself, `:6-19` | in-file precedent |
| `src/shared/streamable_http.rs` | transport (client) | request-response | itself `:574-583` + `streamable_http_server.rs:477-588` | in-file precedent |
| `src/client/mod.rs` | controller (client protocol) | request-response (bounded loop) | itself `:1066-1140` + `:2526-2548` | in-file precedent |
| `src/client/host/mod.rs` | service (registry/classifier) | event-driven | itself, `:79-131` | in-file precedent |
| `src/error/mod.rs` | model (error enum) | — | itself, `:94-105` (`ToolRejected`) | in-file precedent |
| `Cargo.toml` | config | — | itself, `rustls`/`jsonwebtoken` optional deps | in-file precedent |

---

## Pattern Assignments

### `src/server/request_state.rs` (NEW — utility/crypto, transform)

**Analog:** `src/shared/pkce.rs` (the repo's only other "small crypto primitive
module": CSPRNG + base64 + digest, `Result`-typed, zero `unwrap`, doctested,
known-answer unit tests). Copy its *shape* exactly; swap SHA-256 for `ring::aead`.

**Module-doc + why-this-exists pattern** (`src/shared/pkce.rs:1-21`):

```rust
//! Target-agnostic PKCE (RFC 7636) crypto helper for OAuth 2.0 Authorization
//! Code flows.
//!
//! This module provides the pure cryptographic primitives needed to drive an
//! OAuth Authorization Code + PKCE flow ...
//!
//! # Why a shared helper
//!
//! Browser PKCE and the existing native loopback flow both need RFC 7636
//! verifier/challenge/state primitives. ...
```

**Imports pattern** (`src/shared/pkce.rs:50-52`) — note `crate::error::{Error, Result}`,
`URL_SAFE_NO_PAD`, and that **no new crate is imported**:

```rust
use crate::error::{Error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
```

**CSPRNG pattern — single centralized `getrandom` call, no `unwrap`/`expect`**
(`src/shared/pkce.rs:54-70`). This is the pattern for BOTH the 12-byte AEAD nonce
AND the D-04 per-process fallback key:

```rust
/// Number of CSPRNG bytes used to build a code verifier / state value.
const PKCE_RANDOM_BYTES: usize = 32;

/// Fill a fixed-size buffer with cryptographically secure random bytes.
///
/// Centralises the single `getrandom::fill` call so both the verifier and the
/// state generators share one CSPRNG source, and so a `getrandom::Error` is
/// mapped to [`Error::internal`] in exactly one place (no `unwrap`/`expect`).
fn random_bytes() -> Result<[u8; PKCE_RANDOM_BYTES]> {
    let mut buf = [0u8; PKCE_RANDOM_BYTES];
    getrandom::fill(&mut buf)
        .map_err(|e| Error::internal(format!("CSPRNG (getrandom) failed: {e}")))?;
    Ok(buf)
}
```

> RESEARCH's `resolve_key()` pseudocode uses `.expect("CSPRNG")` — **do not copy
> that**; use the `map_err(Error::internal)` form above. `make check-unwraps` is
> part of `quality-gate`.

**Known-answer + determinism + distinctness unit-test triad**
(`src/shared/pkce.rs:148-198`) — mirror this for mint/verify (KAT vector,
mint→verify identity, two mints differ because the nonce differs):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 7636 Appendix B vector pins S256 correctness.
    #[test]
    fn pkce_rfc7636_appendix_b_vector() {
        let challenge = code_challenge_s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    /// Distinct verifiers are produced across calls (entropy sanity check).
    #[test]
    fn pkce_verifiers_are_distinct() {
        let a = generate_code_verifier().expect("CSPRNG available on host");
        let b = generate_code_verifier().expect("CSPRNG available on host");
        assert_ne!(a, b, "two CSPRNG draws must not collide");
    }
}
```

**Env-var resolution pattern (D-03/D-04)** — analog `src/server/observability/config.rs:128-152`.
The house style is `if let Ok(v) = std::env::var("PMCP_*")` with a *silent* fall
back to the default. Phase 113 differs on ONE point (locked by D-04): the
fall-through branch MUST `tracing::warn!`. The `PMCP_` prefix and read-once-at-
build placement come from this analog:

```rust
    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        // Master switch
        if let Ok(enabled) = std::env::var("PMCP_OBSERVABILITY_ENABLED") {
            if let Ok(v) = enabled.parse() {
                self.enabled = v;
            }
        }
        // Backend selection
        if let Ok(backend) = std::env::var("PMCP_OBSERVABILITY_BACKEND") {
            self.backend = backend;
        }
```

**Principal source for the AAD** — `src/server/auth/traits.rs:61-88`. Bind to
`AuthContext.subject` (the `sub` claim), **never** `clientInfo` (see Shared
Patterns → Identity anchoring):

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthContext {
    /// Subject identifier (user ID from the `sub` claim).
    pub subject: String,
    /// Granted scopes/permissions.
    pub scopes: Vec<String>,
    /// Additional claims from the token.
    pub claims: HashMap<String, serde_json::Value>,
    ...
}
```

**Verdict-enum decomposition (PMAT cog ≤ 25)** — model the verifier on the
`V2Classification` / `V2GateOutcome` split in `streamable_http_server.rs:379-397`
(small enums + tiny single-responsibility fns + one thin composer). See the
"Era gate" excerpt below.

---

### `src/types/mrtr.rs` (NEW — model/protocol types, transform)

**Analog:** `src/types/elicitation.rs` — same size class, same job (one spec
section's types + serde attrs + a `mod tests` doing serialize/roundtrip asserts).

**Module-doc + spec-version header** (`src/types/elicitation.rs:1-17`):

```rust
//! Spec-compliant MCP elicitation types (2025-11-25).
//!
//! Replaces the PMCP-proprietary elicitation format with the official
//! MCP specification. Two modes: form (JSON Schema-based) and URL.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
```

**Struct + camelCase + skip-if-none pattern** (`src/types/elicitation.rs:48-57`) —
this is exactly the shape `InputRequiredResult` / `InputResponseRequestParams` need:

```rust
/// Elicitation result returned by the client (MCP 2025-11-25).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitResult {
    /// User's action on the elicitation
    pub action: ElicitAction,
    /// Form content (present when action is Accept, absent otherwise)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HashMap<String, Value>>,
}
```

**Serialize + roundtrip test pattern** (`src/types/elicitation.rs:107-130`) — assert
the *wire key spelling* explicitly (`json["requestedSchema"]`), not just roundtrip:

```rust
    #[test]
    fn elicit_request_form_mode_serialization() {
        let params = ElicitRequestParams::Form { /* ... */ };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["mode"], "form");
        assert!(json["requestedSchema"]["properties"]["name"].is_object());

        let roundtrip: ElicitRequestParams = serde_json::from_value(json).unwrap();
        ...
    }
```

**Method-tagged enum pattern for `InputRequest`** — `src/types/protocol/mod.rs:544-557`.
`InputRequests` values are `{ "method": ..., "params": ... }`, which is EXACTLY
`ServerRequest`'s existing internally-tagged shape. Reuse it rather than
hand-rolling:

```rust
/// Server request types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum ServerRequest {
    #[serde(rename = "sampling/createMessage")]
    CreateMessage(Box<super::sampling::CreateMessageParams>),
    #[serde(rename = "roots/list")]
    ListRoots,
    #[serde(rename = "elicitation/create")]
    ElicitationCreate(Box<crate::types::elicitation::ElicitRequestParams>),
}
```

**Module registration** — `src/types/mod.rs:6-19` (alphabetical `pub mod` list) and
the flat re-export block at `:46-48`:

```rust
pub mod auth;
pub mod capabilities;
...
pub mod elicitation;
...
pub use elicitation::{
    ElicitAction, ElicitRequestParams, ElicitResult, ElicitationCompleteNotification,
};
```

---

### `src/server/streamable_http_server.rs` (MODIFIED — transport/router)

**Analog: itself.** Phase 112 built the whole v2 gate here as a decomposed,
cog-safe, property-tested block. Phase 113's session gate, 405 rule, and
`subscriptions/listen` route go **inside the same block, using the same shapes**.

**Section-banner + "consume, never re-resolve" doctrine** (`:348-361`) — the new
session gate belongs under this banner:

```rust
// ===========================================================================
// v2 required-header gate (Plan 112-06, VERS-05 / D-05 / D-06 / D-11).
//
// The v2 verdict is Plan 04's RESOLVED `ProtocolContext.era`, CONSUMED here —
// this layer never runs a second independent era resolver (Pitfall 2). The
// streamable-HTTP inbound handler resolves the context ONCE (for this gate) and
// threads that SAME value into `Server::handle_request_with_context`, so
// dispatch is a pass-through, not a re-resolve.
//
// The classifier is decomposed into small single-responsibility helpers, each
// well under cognitive-complexity 25 (PMAT CI gate — WARNING 4), composed by a
// thin top-level `classify_v2_request`. Every new header-violation error sources
// its JSON-RPC code from `error_codes::` (VERS-06); no new bare -326xx literal.
// ===========================================================================
```

**Verdict-enum decomposition to copy** (`:379-397`) — the `sessions_active` /
GET-DELETE-405 / `requestState`-verdict logic should be small enums + tiny fns
composed by one thin top-level fn, NOT one big `match`:

```rust
/// The classification of an opted-in request over the header/`_meta` matrix.
enum V2Classification {
    /// v1 / both signals non-v2 → run the legacy path with zero enforcement.
    Legacy,
    /// v2 on BOTH the header and the resolved `_meta` era → enforce headers.
    Enforce,
    /// A conflict cell (v2-header/non-v2-`_meta` or vice-versa) → fail closed.
    Reject(i32, &'static str),
}

/// Outcome of the whole v2 gate for one request.
enum V2GateOutcome {
    Passthrough,
    EnforceOk { method: String, name: String },
    Reject(i32, String),
}
```

**Thin composer pattern** (`:506-536`) — the exact template for a
`sessions_active(state, era)` predicate routing the four session sites:

```rust
/// The thin top-level classifier over the full matrix (cog-safe composition).
///
/// Inputs: decoded header signals + Plan-04 resolved `meta_is_v2` + the untrusted
/// body `method`/`params.name`. Output: accept (with echo headers) | reject(code)
/// | passthrough. Pure and non-panicking — property-tested.
fn classify_v2_request(
    headers: &HeaderMap,
    meta_is_v2: bool,
    body_method: Option<&str>,
    body_name: Option<&str>,
) -> V2GateOutcome {
    use crate::types::protocol::error_codes::INVALID_REQUEST;
    let header = decode_version_header(headers);
    match classify_era_cell(header, meta_is_v2) {
        V2Classification::Legacy => V2GateOutcome::Passthrough,
        V2Classification::Reject(code, msg) => V2GateOutcome::Reject(code, msg.to_string()),
        V2Classification::Enforce => { /* ... */ }
    }
}
```

**Era read (the `sessions_active` input)** (`:620-627`) — `Ok(None)` == not
opted in == zero enforcement (D-04). The new predicate must preserve this:

```rust
    // `Ok(None)` == not opted in → zero enforcement (D-04).
    let Some(ref pc) = context else {
        return (context.clone(), V2GateOutcome::Passthrough);
    };
    let meta_is_v2 = pc.era == crate::types::protocol::Era::V2;
```

**The four session sites to gate** — verified line numbers:

| Site | Line | Current predicate |
|------|------|-------------------|
| `process_init_session` | `:867` | `if let Some(generator) = &state.config.session_id_generator {` |
| `validate_non_init_session` | `:910` | `if state.config.session_id_generator.is_some() {` |
| `validate_protocol_version_matches_session` | `:1088` | `if state.config.session_id_generator.is_none() { return Ok(()) }` |
| response header emission | `:1494-1496` | see below |

```rust
    // src/server/streamable_http_server.rs:1492-1497
    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, APPLICATION_JSON.parse().unwrap());
    if let Some(sid) = response_session_id {
        response_headers.insert(MCP_SESSION_ID, sid.parse().unwrap());
    }
    response_headers.insert(MCP_PROTOCOL_VERSION, version_to_send.parse().unwrap());
```

**Router (GET/DELETE → 405 on v2)** — `:289-291`:

```rust
        .route("/", post(handle_post_request))
        .route("/", get(handle_get_sse))
        .route("/", delete(handle_delete_session))
```

Era signal on GET/DELETE is the header only — reuse `decode_version_header`
(`:399-414`), which already handles absent/malformed/oversized without panicking:

```rust
/// Decode the `MCP-Protocol-Version` header without panicking (T-112-13).
fn decode_version_header(headers: &HeaderMap) -> HeaderProtocolVersion {
    let Some(raw) = headers.get(MCP_PROTOCOL_VERSION) else {
        return HeaderProtocolVersion::Absent;
    };
    if raw.as_bytes().len() > MAX_V2_HEADER_VALUE_LEN {
        return HeaderProtocolVersion::Malformed;
    }
    match raw.to_str() {
        Err(_) => HeaderProtocolVersion::Malformed,
        Ok(s) if s == crate::types::protocol::PROTOCOL_VERSION_2026_07_28 => {
            HeaderProtocolVersion::V2
        },
        Ok(_) => HeaderProtocolVersion::Other,
    }
}
```

**Error-response pattern (status + JSON-RPC code together)** — the shape for
`-32020`/`-32021`/`-32022`@400 and `-32601`@404; note the code always comes from
`error_codes::`, never a bare literal (`:874-878`, `:925-929`, `:2413-2417`):

```rust
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        crate::types::protocol::error_codes::INVALID_REQUEST,
                        "Session already initialized",
                    ));
```

**SSE stream pattern for `subscriptions/listen`** — `handle_get_sse` at `:2347-2394`
is the long-lived-stream analog (channel → `UnboundedReceiverStream` → `Sse::new`
→ attach headers). Copy the lifecycle; replace session-keying with
subscription-id keying and add the ack-first frame + keep-alive:

```rust
async fn handle_get_sse(State(state): State<ServerState>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(error_response) = validate_headers(&headers, "GET") {
        return error_response;
    }
    ...
    let (tx, rx) = mpsc::unbounded_channel();
    state.sse_streams.write().insert(session_id.clone(), tx.clone());

    replay_sse_events_from_header(&headers, &tx, state.config.event_store.as_ref()).await;

    let stream = UnboundedReceiverStream::new(rx);
    let session_id_for_header = session_id.clone();
    let session_id_for_stream = session_id.clone();
    let event_store = state.config.event_store.clone();

    let sse = Sse::new(stream.map(move |msg| {
        Ok::<_, Infallible>(sse_event_for_message(
            &msg,
            &session_id_for_stream,
            event_store.as_ref(),
        ))
    }));

    let mut response = sse.into_response();
    attach_sse_response_headers(&mut response, &session_id_for_header);
    response
}
```

> **HTTP-05 lever:** `replay_sse_events_from_header(...)` at `:2376` is the single
> call the v2 path must NOT reach. Simplest conformant edit is to skip that one
> line on v2 — but `handle_get_sse` is 405 on v2 anyway, so the real work is the
> `replay_events_after` call sites at `:48` / `:94`.

**Frame-building pattern (for the ack frame + notification frames)** — `:1017-1037`:

```rust
fn build_sse_response_from_single_message(response: TransportMessage) -> Response {
    let (tx, rx) = mpsc::unbounded_channel();
    tx.send(response).unwrap();
    let stream = UnboundedReceiverStream::new(rx);
    let sse = Sse::new(stream.map(|msg| {
        let event_id = Uuid::new_v4().to_string();
        let json_bytes = crate::shared::StdioTransport::serialize_message(&msg).unwrap_or_else(|e| {
            tracing::error!(target: "mcp.sse", error = %e, "Failed to serialize SSE message");
            Vec::new()
        });
        let json_str = String::from_utf8(json_bytes).unwrap_or_else(|_| "{}".to_string());
        Ok::<_, Infallible>(Event::default().id(event_id).event("message").data(json_str))
    }));
    sse.into_response()
}
```

**Property test pattern (required for the new predicates)** — `:2693-2729`:

```rust
    proptest::proptest! {
        /// The classifier NEVER panics over arbitrary header bytes + signal
        /// combinations, and holds the accept/reject invariants (T-112-13).
        #[test]
        fn v2_header_gate_proptest(
            header_kind in 0u8..4,
            meta_is_v2 in proptest::bool::ANY,
            have_method in proptest::bool::ANY,
            ...
        ) {
            ...
            // Must not panic.
            let out = classify_v2_request(&h, meta_is_v2, body_method.as_deref(), body_name.as_deref());
```

---

### `src/server/core.rs` (MODIFIED — controller/dispatch)

**Analog: itself, `:1087-1224`.** Phase 112 scaffolded `InputRequired`
*specifically for this phase*. Wire it; do not add a parallel mechanism.

**The disposition to select** (`:1097-1123`) — the `#[allow(dead_code)]` on
`InputRequired` **must be deleted** when wired (CLAUDE.md zero-SATD):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseDisposition {
    /// The result is a final, complete result (the default; absent-means-complete).
    Complete,
    /// The result requests further input before it can complete (Phase 113).
    #[allow(dead_code)]
    InputRequired,
    /// The result is a task handle rather than a terminal result (Phase 114).
    #[allow(dead_code)]
    Task,
}

impl ResponseDisposition {
    /// The wire `resultType` discriminator string.
    pub(crate) fn as_wire_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::InputRequired => "input_required",
            Self::Task => "task",
        }
    }
}
```

**The injection point** (`:1141-1171`) — v2-only, object-results-only,
collision-safe. The `serverInfo` placement fix (RESEARCH Pitfall 6) is the last
two lines:

```rust
pub(crate) fn inject_v2_result_envelope(
    response: &mut JSONRPCResponse,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    server_info: &Implementation,
    disposition: ResponseDisposition,
) {
    // v2-only: a v1 (or non-opted-in) response is left byte-identical.
    if !matches!(
        protocol_context.map(|c| c.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        return;
    }
    // Only success results carry the envelope; errors / notifications do not.
    let crate::types::jsonrpc::ResponsePayload::Result(ref mut value) = response.payload else {
        return;
    };
    // A non-object result (scalar/array/null) cannot carry a key — leave it.
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    // Collision-safe: respect a handler-set `resultType` (the 113/114 path).
    obj.entry("resultType".to_string())
        .or_insert_with(|| Value::String(disposition.as_wire_str().to_string()));
    // Attach serverInfo on the v2 object result; never overwrite a handler value.
    obj.entry("serverInfo".to_string())
        .or_insert_with(|| serde_json::to_value(server_info).unwrap_or(Value::Null));
}
```

**Era-gated method arm (the `subscriptions/listen` -32601 branch, and the
"input_required only on 3 methods" gate)** — `build_discover_response` at
`:1189-1224` is the template for "v2-only method, v1 gets `-32601`":

```rust
pub(crate) fn build_discover_response(
    id: RequestId,
    capabilities: &ServerCapabilities,
    info: &Implementation,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
) -> JSONRPCResponse {
    // Era gate (D-10): v2 only. A v1 / non-opted-in request is method-not-found.
    if !matches!(
        protocol_context.map(|c| c.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        return ServerCore::error_response(
            id,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND,
            "Method not found: server/discover".to_string(),
        );
    }
    ...
    let mut response = ServerCore::success_response(id, serde_json::to_value(result).unwrap());
    // Parity: the v2 object result carries resultType + serverInfo via the SAME
    // shared envelope helper every other v2 result uses.
    inject_v2_result_envelope(&mut response, protocol_context, info, ResponseDisposition::Complete);
    response
}
```

**Isolated wire-shape conversion fn** (`:1066-1085`) — the pattern for the
`inputRequests`↔`ElicitRequestParams` adapter (D-10: "one place the wire shape is
assembled, so a final-spec change is localized"):

```rust
/// Isolated conversion fn producing the [`ServerDiscoverResult`] wire shape
/// (Phase 112, VERS-04).
///
/// This is the SINGLE place the discover wire shape is assembled: ...
/// Keeping the shape behind one fn means a final-spec change is localized
/// (Codex MEDIUM — "server/discover wire shape is provisional").
pub(crate) fn discover_result_from_capabilities(
    capabilities: &ServerCapabilities,
    info: &Implementation,
    negotiated_version: String,
) -> ServerDiscoverResult { ... }
```

---

### `src/server/mod.rs` (MODIFIED — twin dispatch site + builder)

**Analog: itself.** Two separate in-file precedents.

**Twin-site envelope injection** (`:1414-1423`) — whatever `core.rs` does with
`InputRequired`, this site must do identically (the Phase 109/112 parity rule):

```rust
        // Twin-site v2 envelope injection (VERS-07 / D-07 / D-08): the ONE shared
        // helper in `core.rs` — v2-only, object-results-only, collision-safe;
        // v1 / non-opted-in responses stay byte-identical.
        crate::server::core::inject_v2_result_envelope(
            &mut response,
            protocol_context.as_ref(),
            &self.info,
            crate::server::core::ResponseDisposition::Complete,
        );
        response
```

**Builder opt-in pattern** (`:2544-2558`) — the template for BOTH the
`requestState` key/TTL config AND the D-13 `subscriptions/listen` capability
opt-in. Note: `#[must_use]`, "with no call, behaves exactly as today", and an
explicit never-all-reject fallback:

```rust
    /// Opt into a protocol-version accept-list (Phase 112, VERS-01/02; D-02/D-04).
    ///
    /// The high-level `Server` twin of
    /// [`ServerCoreBuilder::with_supported_protocol_versions`](crate::server::builder::ServerCoreBuilder::with_supported_protocol_versions).
    /// With no call, the server is v1-only and behaves exactly as today. An empty
    /// accept-list falls back to the v1-only default (never all-reject).
    #[must_use]
    pub fn with_supported_protocol_versions(
        mut self,
        versions: impl IntoIterator<Item = ProtocolVersion>,
    ) -> Self {
        self.supported_protocol_versions =
            crate::types::protocol::context::normalize_accept_list(versions);
        self
    }
```

**Default-field wiring** (`:2540`) — every new builder field needs its `Default`
entry in the same struct-literal block:

```rust
            supported_protocol_versions: crate::types::protocol::context::default_accept_list(),
```

`src/server/builder.rs` gets the `ServerCoreBuilder` twin of each new method —
same doc, same `#[must_use]`, cross-linked both ways (the existing pair is
`core.rs:442` ↔ `mod.rs:2551`).

---

### `src/types/tools.rs` / `prompts.rs` / `resources.rs` (MODIFIED — model)

**Analog: `src/types/tools.rs:450-484`** — the `task` field is the *exact*
additive-Option precedent RESEARCH Pitfall 10 names. Copy the four attributes
(`#[non_exhaustive]` on the struct, `skip_serializing_if`, `default`, doc-comment
explaining why it is `Option`) and update the `new()` constructor:

```rust
/// Tool call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CallToolRequest {
    /// Tool name to invoke
    pub name: String,
    /// Tool arguments (must match input schema)
    #[serde(default)]
    pub arguments: Value,
    /// Request metadata (e.g., progress token)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)] // _meta is part of MCP protocol spec
    pub _meta: Option<RequestMeta>,
    /// Task augmentation parameters (experimental MCP Tasks).
    ///
    /// When present, the server creates a task and returns `CreateTaskResult`
    /// instead of `CallToolResult`. Uses `serde_json::Value` to avoid circular
    /// crate dependency (`pmcp-tasks` depends on `pmcp`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub task: Option<Value>,
}

impl CallToolRequest {
    /// Create a tool call request.
    ///
    /// `_meta` and `task` default to `None`.
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self { name: name.into(), arguments, _meta: None, task: None }
    }
```

> `GetPromptRequest` (`src/types/prompts.rs`) is built with a **struct literal**
> in tests (`tests/v2_required_headers.rs:325-329`), so adding a field there is a
> source break for any struct-literal construction — check `#[non_exhaustive]`
> parity across all three before adding.

---

### `src/types/protocol/error_codes.rs` (MODIFIED — config/constants)

**Analog: itself.** The module is already structured to receive exactly these
three values ("v2 semantic error-code values ... are structurally omitted here").

**Constant + doc pattern** (`:65-94`):

```rust
/// Request timeout — the server-side operation exceeded its deadline.
pub const REQUEST_TIMEOUT: i32 = -32001;

/// Unsupported capability (`-32002`).
///
/// The capability-unsupported semantic carried by
/// [`crate::error::ErrorCode::UNSUPPORTED_CAPABILITY`]. This intentionally
/// shares the number `-32002` with [`V1_TASK_PENDING`] but is a DIFFERENT
/// meaning — the two are kept distinct by name and are NOT reconciled.
pub const UNSUPPORTED_CAPABILITY: i32 = -32002;
```

**Locking-test discipline** (`:104-121`) — each new constant needs a test that
pins its value and its relationship to the rest of the table:

```rust
    /// Both distinct meanings of `-32002` are present, by their own names, with
    /// the same numeric value. This collision is intentional and preserved.
    #[test]
    fn both_minus_32002_meanings_coexist() {
        assert_eq!(V1_TASK_PENDING, -32002);
        assert_eq!(UNSUPPORTED_CAPABILITY, -32002);
        assert_eq!(V1_TASK_PENDING, UNSUPPORTED_CAPABILITY);
    }
```

Also update the module doc at `:10-21` — it currently says the v2 values are
"structurally omitted ... finalized only when the 2026-07-28 `schema.json`
publishes". Phase 113 lands them; the doc must stop claiming they are absent.

---

### `src/types/elicitation.rs` (MODIFIED — serde, RESEARCH Pitfall 4)

**Problem:** `#[serde(tag = "mode")]` at `:25` rejects a spec-shaped form
elicitation that omits `mode`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum ElicitRequestParams {
    #[serde(rename = "form", rename_all = "camelCase")]
    Form { message: String, requested_schema: Value },
    #[serde(rename = "url", rename_all = "camelCase")]
    Url { message: String, elicitation_id: String, url: String },
}
```

**Analog for the fix — `deserialize_with` shim:** `src/types/resources.rs:353-357`
is the in-repo precedent for a custom deserializer module attached to one field
without changing the public type:

```rust
        deserialize_with = "crate::types::content::resource_contents_serde::deserialize"
```

**Analog for the fix — untagged fallback:** `src/types/protocol/mod.rs:559-567` and
`src/types/jsonrpc.rs:29` show the house `#[serde(untagged)]` usage:

```rust
/// Combined request types (client or server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Request {
    Client(Box<ClientRequest>),
    Server(Box<ServerRequest>),
}
```

Constraint from D-10: **serialization must keep emitting `"mode":"form"`** (v1
byte-compat), so a custom `Deserialize` + derived `Serialize` split is preferable
to flipping the whole enum to untagged. Pair it with the existing round-trip test
(`:107-130`) plus a new "no `mode` key" deserialization test.

---

### `src/shared/streamable_http.rs` (MODIFIED — client transport, CLNT-01)

**Analog: itself `:557-583` for the emission site; the server-side Phase-112
helpers for the *values*.**

**Header emission site to extend** (`:557-583`) — note `Mcp-Method`/`Mcp-Name` are
absent today and `MCP_SESSION_ID` is emitted unconditionally (must be suppressed
on v2):

```rust
        // Start building request with hyper
        let mut request_builder = Request::builder().method(method.clone()).uri(url);

        // Add extra headers from config
        for (key, value) in &extra_headers {
            request_builder = request_builder.header(key.as_str(), value.as_str());
        }
        ...
        // Add session ID header if we have one
        if let Some(session_id) = &session_id {
            request_builder = request_builder.header(MCP_SESSION_ID, session_id.as_str());
        }

        // Add protocol version header if we have one
        if let Some(protocol_version) = self.protocol_version.read().as_ref() {
            request_builder =
                request_builder.header(MCP_PROTOCOL_VERSION, protocol_version.as_str());
        }
```

**Where `Mcp-Name` comes from — reuse the server's single source of truth**
(`streamable_http_server.rs:469-488`). Do NOT write a second table; lift or
re-export this one so client and server can never drift:

```rust
/// Single source of truth for a name-bearing method's logical-name location.
///
/// `tools/call` and `prompts/get` carry it in `params.name`; `resources/read` in
/// `params.uri` ... Both [`is_name_bearing_method`] and the body-name extraction
/// in [`extract_body_method_and_name`] derive from this one table so the
/// "which methods are name-bearing" set and the "where the name lives" map can
/// never drift ...
fn logical_name_key(method: &str) -> Option<&'static str> {
    match method {
        "tools/call" | "prompts/get" => Some("name"),
        "resources/read" => Some("uri"),
        _ => None,
    }
}
```

**Non-panicking header insertion** (`streamable_http_server.rs:572-588`) — the
client emitter must be equally panic-free (the base64 sentinel path plugs in here):

```rust
/// Emit the three required v2 headers outbound WITHOUT panicking (T-112-13).
fn apply_v2_outbound_headers(headers: &mut HeaderMap, method: &str, name: &str) {
    if let Ok(v) = HeaderValue::from_str(method) {
        headers.insert(MCP_METHOD, v);
    }
    if let Ok(v) = HeaderValue::from_str(name) {
        headers.insert(MCP_NAME, v);
    }
    if let Ok(v) = HeaderValue::from_str(crate::types::protocol::PROTOCOL_VERSION_2026_07_28) {
        headers.insert(MCP_PROTOCOL_VERSION, v);
    }
}
```

**Constants** — `src/shared/http_constants.rs` already has everything needed
(`MCP_METHOD:16`, `MCP_NAME:24`, `ACCEPT_STREAMABLE:43`); no new constant module.

---

### `src/client/mod.rs` (MODIFIED — client protocol, CLNT-01 / CLNT-02)

**Analog A — the builder opt-in (D-08).** `src/server/mod.rs:2544-2558`
(`with_supported_protocol_versions`) is the symmetric server twin; `with_protocol_version`
must read the same way. The local builder shape to extend is `:2907-2928`:

```rust
impl<T: Transport> ClientBuilder<T> {
    /// Create a new client builder.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            options: ProtocolOptions::default(),
            middleware_chain: EnhancedMiddlewareChain::new(),
            host_registry: crate::client::host::ClientHostRegistry::default(),
        }
    }

    /// Set whether to enforce strict capabilities.
    pub fn enforce_strict_capabilities(mut self, enforce: bool) -> Self {
        self.options.enforce_strict_capabilities = enforce;
        self
    }
```

**Analog B — the bounded retry loop (D-09).** `call_tool_and_poll` at `:1066-1140`
is the in-file precedent for "bounded client-side loop with a typed give-up":

```rust
    pub async fn call_tool_and_poll(
        &self,
        name: String,
        arguments: serde_json::Value,
        max_polls: usize,
    ) -> Result<CallToolResult> {
        /// Default polling interval when the server doesn't specify one.
        const DEFAULT_POLL_INTERVAL_MS: u64 = 5000;
        ...
                let mut polls = 0;
                loop {
                    polls += 1;
                    let task = self.tasks_get(&task_id).await?;
                    if task.status.is_terminal() { ... }
                    if max_polls > 0 && polls >= max_polls {
                        return Err(Error::internal(format!(
                            "Task {} did not complete after {} polls",
                            task_id, max_polls
                        )));
                    }
                    ...
                }
```

> **Deviation required:** this analog gives up with `Error::internal(...)`. D-09
> mandates a **typed** `MrtrRoundLimitExceeded`. Copy the loop/counter structure,
> not the untyped error.

**Analog C — the three-way handler dispatch (CLNT-02).** `dispatch_host_request`
at `:2526-2548` already routes exactly the three MRTR kinds. The MRTR fold is a
sibling that returns `InputResponse` values instead of `JSONRPCResponse`:

```rust
    async fn dispatch_host_request(
        &self,
        id: RequestId,
        request: Request,
    ) -> crate::types::JSONRPCResponse {
        use crate::client::host::{classify_host_request, HostRequestKind};
        match classify_host_request(&request) {
            HostRequestKind::Sampling => self.dispatch_host_sampling(id, request).await,
            HostRequestKind::Elicitation => self.dispatch_host_elicitation(id, request).await,
            HostRequestKind::Roots => self.dispatch_host_roots(id).await,
            HostRequestKind::Ping => {
                crate::types::JSONRPCResponse::success(id, serde_json::json!({}))
            },
            HostRequestKind::Unhandled => Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            ),
        }
    }
```

**Missing-handler guard pattern** (`:2579-2594`) — D-06's "no handler → do NOT
resend, return the `input_required` result" is the same early-return shape:

```rust
        // At least one sampling handler (legacy or WithTools) must be registered.
        if self.host_registry.sampling.is_none() && self.host_registry.sampling_with_tools.is_none()
        {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        }
```

---

### `src/client/host/mod.rs` (MODIFIED — classifier)

**Analog: itself, `:79-131`.** Add an `inputRequests`-shaped sibling to
`classify_host_request` (the request objects arrive as `{method, params}` JSON,
not as a parsed `Request`), and derive `_meta.clientCapabilities` from the
registry rather than from caller-supplied values.

```rust
/// Classification of an inbound request at a client into a host-handler kind.
///
/// Pure, synchronous, and side-effect free so it can be property/fuzz tested
/// independently of the async dispatch path.
///
/// Exposed as `#[doc(hidden)] pub` (not part of the public API surface) solely
/// so the routing fuzz target can drive [`classify_host_request`] directly.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRequestKind {
    Sampling,
    Elicitation,
    Roots,
    Ping,
    Unhandled,
}

#[doc(hidden)]
pub fn classify_host_request(request: &Request) -> HostRequestKind {
    match request {
        Request::Client(client) => match client.as_ref() { ... },
        Request::Server(server) => match server.as_ref() {
            ServerRequest::CreateMessage(_) => HostRequestKind::Sampling,
            ServerRequest::ElicitationCreate(_) => HostRequestKind::Elicitation,
            ServerRequest::ListRoots => HostRequestKind::Roots,
        },
    }
}
```

**Capability-honesty source** — the registry fields at `:51-61` are the ground
truth for `clientCapabilities` (spec obligation 7 / conformance `capability-check`):

```rust
#[derive(Clone, Default)]
pub struct ClientHostRegistry {
    pub(crate) sampling: Option<Arc<dyn HostSamplingHandler>>,
    pub(crate) sampling_with_tools: Option<Arc<dyn HostSamplingHandlerWithTools>>,
    pub(crate) elicitation: Option<Arc<dyn HostElicitationHandler>>,
    pub(crate) roots: Option<RootsProvider>,
    pub(crate) approval: Option<PreflightApproval>,
    pub(crate) result_review: Option<SamplingResultReview>,
}
```

**Handler signature that must not change (D-06/D-10)** — `src/client/host/elicitation.rs:17-27`:

```rust
#[async_trait]
pub trait HostElicitationHandler: Send + Sync {
    /// Collect user input for the given inbound elicitation request.
    async fn handle_elicitation(&self, params: ElicitRequestParams) -> Result<ElicitResult>;
}
```

---

### `src/error/mod.rs` (MODIFIED — typed error, D-09)

**Analog: itself, `:94-105`** (`ToolRejected` — the most recent structured
variant, with named fields and a `#[error("...")]` display):

```rust
    #[error("{message}")]
    ToolRejected {
        ...
    },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
```

Adding a variant to the public `Error` enum is semver-relevant — check whether
`Error` is `#[non_exhaustive]` before adding, and re-run
`cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp`.

---

### `Cargo.toml` (MODIFIED — config)

**Analog: itself.** The optional-dep + feature-list pattern is already there:

```toml
# Transport dependencies (optional, feature-gated)
hyper-rustls = { version = "0.27", default-features = false, features = ["http1", "http2", "tls12", "ring", "native-tokio"], optional = true }
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"], optional = true }
axum = { version = "0.8.5", optional = true }

[features]
streamable-http = ["dep:hyper", "dep:hyper-util", "dep:hyper-rustls", "dep:rustls", "dep:futures-util", "dep:bytes", "dep:axum", "dep:tower", "dep:tower-http"]
```

Also note the **normal (non-optional) all-target** dep block at `:86-93`, and its
comment explaining *why* it is normal — the same reasoning applies if any part of
`request_state.rs` must link on wasm:

```toml
# OAuth dependencies
sha2 = "0.11"
base64 = "0.22"

# Cross-target CSPRNG for the wasm-safe PKCE helper (src/shared/pkce.rs).
# HIGH-1: must be a NORMAL [dependencies] entry so the ungated pkce module links
# on the HOST target too ...
getrandom = "0.4"
```

---

### `tests/v2_*.rs` (NEW — integration tests)

**Analog: `tests/v2_required_headers.rs`** (exact — same phase lineage, same
transport, same reliability discipline).

**File header: cfg gate + reliability doctrine** (`:1-19`):

```rust
//! Phase 112-06 (VERS-05 / D-05 / D-06 / D-11): live-HTTP acceptance gate for the
//! required v2 headers on the streamable-HTTP path.
//!
//! These tests drive a REAL `StreamableHttpServer` over a loopback TCP socket
//! with a raw `reqwest` client (NOT the in-memory transport — RESEARCH Pitfall
//! 11) so every header/`_meta` combination crosses the actual axum HTTP boundary.
//! ...
//! Test reliability (carried from the Phase 102/104 HTTP harness): EPHEMERAL
//! PORT (`127.0.0.1:0`, address read back from `start()`), READINESS (`start()`
//! binds before returning), SHUTDOWN (`JoinHandle::abort()` after each round-trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]
```

**Server spawn helper** (`:238-244`) — **HTTP-01 deviation required:** RESEARCH
Pitfall 1 says the new stateless tests must build with `Default::default()`, NOT
`::stateless()`, or the era gate is never exercised:

```rust
async fn spawn(opt_in_v2: bool) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let server = Arc::new(Mutex::new(build_server(opt_in_v2)));
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http =
        StreamableHttpServer::with_config(addr, server, StreamableHttpServerConfig::stateless());
    http.start().await.expect("server starts")
}
```

**v2 opt-in server builder** (`:125-143`):

```rust
fn build_server(opt_in_v2: bool) -> Server {
    let mut builder = Server::builder().name("v2-required-headers").version("1.0.0");
    if opt_in_v2 {
        builder = builder
            .capabilities(extensions_capabilities())
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(V2.to_string()),
            ]);
    }
    builder
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource)
        .build()
        .expect("server builds")
}
```

**Raw-response view + POST helper** (`:246-293`) — lift this into `tests/common/`
(RESEARCH Wave-0 gap) verbatim:

```rust
/// Raw response view: HTTP status + the three v2 headers + the JSON body + the
/// RAW response text (kept for byte-identity assertions ...).
struct Resp {
    status: u16,
    mcp_method: Option<String>,
    mcp_name: Option<String>,
    mcp_version: Option<String>,
    body: serde_json::Value,
    raw: String,
}

async fn post(addr: SocketAddr, extra: &[(&str, &str)], body: &str) -> Resp {
    let client = reqwest::Client::new();
    let mut req = client
        .post(format!("http://{addr}"))
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(body.to_string());
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let resp = req.send().await.expect("request sent");
    let status = resp.status().as_u16();
    let hget = |name: &str| {
        resp.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
    };
    ...
}
```

**v2 request-body builder — build through the TYPED struct** (`:295-319`). This is
load-bearing: it is why the `_meta` camelCase spelling can't drift. The MRTR
retry body builder must extend this fn (adding `inputResponses`/`requestState` as
**top-level `params` siblings**, per RESEARCH):

```rust
/// A raw `tools/call` body. `meta_version` (when `Some`) is carried in
/// `params._meta` under the reserved protocol-version key so the SHARED Plan-04
/// resolver classifies the era from `_meta` (the authoritative signal).
///
/// Built via pmcp's OWN serialization so the wire `_meta` field name round-trips
/// exactly what the server deserializes ...
fn call_body(tool: &str, meta_version: Option<&str>) -> String {
    let mut req = CallToolRequest::new(tool, serde_json::json!({}));
    if let Some(v) = meta_version {
        req._meta = Some(RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            serde_json::json!(v),
        ));
    }
    let params = serde_json::to_value(&req).expect("params serialize");
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": params,
    })
    .to_string()
}
```

> **HTTP-03 note:** `call_body` hard-codes `"id": 1`. The MRTR retry MUST use a
> **different** JSON-RPC id — parameterize the id when lifting to `tests/common/`.

**Trivial dispatch targets** (`:44-98`) — `SearchTool` / `GreetingPrompt` /
`GreetingResource` give all three MRTR-eligible methods a real handler; the MRTR
test file needs input-requiring variants of the same three.

---

### `fuzz/fuzz_targets/fuzz_request_state.rs` (NEW — fuzz)

**Analog: `fuzz/fuzz_targets/pkce_helper.rs`** (exact — same crypto-helper target
class, complete file):

```rust
//! Fuzz target for `pmcp::shared::pkce` — the wasm-safe PKCE crypto helper.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run pkce_helper` (plain form,
//! no `+nightly` — matches the repo Makefile `test-fuzz` target, LOW-7).
//!
//! Invariant: the verifier → S256 challenge → base64url-decode roundtrip must
//! NEVER panic on arbitrary input bytes. Error paths are acceptable; panics are
//! not. ...

#![no_main]

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use libfuzzer_sys::fuzz_target;
use pmcp::shared::pkce::code_challenge_s256;

fuzz_target!(|data: &[u8]| {
    let verifier = URL_SAFE_NO_PAD.encode(data);
    let challenge = code_challenge_s256(&verifier);
    let _ = URL_SAFE_NO_PAD.decode(challenge.as_bytes());
});
```

Also see `fuzz/fuzz_targets/client_host_routing.rs` for the "drive a
`#[doc(hidden)] pub` classifier from a fuzz target" precedent — the same trick is
needed to reach `request_state::verify` if it is `pub(crate)`.

---

### `examples/s47_v2_stateless_mrtr.rs` (NEW — runnable example)

**Analog: `examples/t05_streamable_http_stateless.rs`** (exact).

**Header + run instructions + imports** (`:1-25`):

```rust
//! Example: Stateless Streamable HTTP Server
//!
//! This example demonstrates:
//! - Running an MCP server over HTTP without session management
//! - Simplified stateless operation
//! - Perfect for serverless deployments (AWS Lambda, etc.)
//! - No session overhead or tracking
//!
//! Run this server with:
//! ```bash
//! cargo run --example 23_streamable_http_server_stateless
//! ```

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::{Server, ToolHandler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
```

> The stale `cargo run --example 23_...` line in that header does not match the
> file name — **do not copy that defect**; the new example's doc must name its own
> file. Also see `examples/s13_elicit_input.rs` for the elicitation-handler side
> and `examples/t06_streamable_http_client.rs` for a client-side companion.

---

### `src/server/subscriptions.rs` (MODIFIED — pub-sub registry, D-13/HTTP-04)

**Analog: itself, `:14-56`** — an `Arc<RwLock<HashMap<..>>>` registry with a
manual `Debug` impl and an optional notification-sender callback. The v2 listen
registry (subscriptionId → sender + agreed filter) is the same shape:

```rust
/// Manages resource subscriptions for the server.
#[derive(Clone)]
pub struct SubscriptionManager {
    /// Map of resource URI to set of subscriber IDs
    subscriptions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Optional callback for sending notifications
    notification_sender: Option<Arc<dyn Fn(ServerNotification) + Send + Sync>>,
}

impl std::fmt::Debug for SubscriptionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionManager")
            .field("subscriptions", &self.subscriptions.try_read().map_or(0, |s| s.len()))
            .finish()
    }
}
```

The stream lifecycle itself belongs in `streamable_http_server.rs` — see the
`handle_get_sse` excerpt above.

---

## Shared Patterns

### 1. Era gate: consume `ProtocolContext`, never re-resolve
**Source:** `src/server/streamable_http_server.rs:348-361, 620-627`; `src/server/core.rs:1147-1153`
**Apply to:** every file with a v2 branch — `streamable_http_server.rs`, `core.rs`, `mod.rs`

```rust
    if !matches!(
        protocol_context.map(|c| c.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        return;   // or: legacy path, byte-identical
    }
```

`Ok(None)` from the resolver means *not opted in* → zero era code runs (112 D-04).
Handler-side read is `RequestHandlerExtra::era()` (`src/server/cancellation.rs:366-369`):

```rust
    #[must_use]
    pub fn era(&self) -> Option<crate::types::protocol::Era> {
        self.protocol_context.as_ref().map(|ctx| ctx.era)
    }
```

### 2. Twin-site parity (native `core.rs` + `mod.rs`, plus wasm mirror)
**Source:** `src/server/mod.rs:1414-1423` calling `src/server/core.rs:1141`
**Apply to:** `input_required` emission, `inputResponses` ingestion, `requestState` verify

One shared helper in `core.rs`; both dispatch sites call it. Never a per-site copy.
The wasm mirror (`src/server/wasm_core.rs`) must stay compiling — `core.rs` is
`#[cfg(not(target_arch = "wasm32"))]`, so MRTR server code must not leak into it
(D-14: no wasm MRTR this phase).

### 3. Error codes come from the table, never a bare literal
**Source:** `src/types/protocol/error_codes.rs`; call sites e.g.
`streamable_http_server.rs:874-878`, `core.rs:1200-1204`
**Apply to:** all new `-32020` / `-32021` / `-32022` / `-32601`@404 paths

```rust
                    return Err(create_error_response(
                        StatusCode::BAD_REQUEST,
                        crate::types::protocol::error_codes::INVALID_REQUEST,
                        "Session already initialized",
                    ));
```

Every constant gets a locking test (`error_codes.rs:104-121`).

### 4. Identity anchoring — `AuthContext.subject`, never `clientInfo`
**Source:** `src/server/auth/traits.rs:61-64`; the explicit warning at
`src/server/cancellation.rs:382-410`
**Apply to:** `request_state.rs` AAD composition, any v2 authorization decision

```rust
    /// Returns the client's SELF-REPORTED implementation info, or `None` when absent.
    ///
    /// **Security — self-reported, not for authorization:** this value is the
    /// client-supplied `clientInfo` surfaced verbatim from initialization. It is
    /// informational ONLY (telemetry, feature-hints, logging) and MUST NOT be
    /// used as an authorization anchor or trusted identity. Real identity binds
    /// to the OAuth token ...
    pub fn client_info(&self) -> Option<&crate::types::Implementation> { ... }
```

### 5. Single-source-of-truth tables (no parallel maps)
**Source:** `src/server/streamable_http_server.rs:469-488` (`logical_name_key`),
`src/server/core.rs:1066-1085` (`discover_result_from_capabilities`)
**Apply to:** the MRTR-eligible-method set, the `Mcp-Name` extraction (client +
server), the `inputRequests`↔`ElicitRequestParams` adapter

The doc comment states the rule explicitly: "Both ... derive from this one table
so the ... set and the ... map can never drift."

### 6. Non-panicking, bounded parsing of attacker-controlled input
**Source:** `src/server/streamable_http_server.rs:363-423`
**Apply to:** `requestState` decode, `inputResponses` parsing, `Mcp-Name` decode

```rust
/// Upper bound on a decoded header value we will consider (`DoS` guard, T-112-13).
const MAX_V2_HEADER_VALUE_LEN: usize = 8192;

/// Read a header as a bounded UTF-8 string, or `None` if absent/malformed.
fn bounded_header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(name)?;
    if raw.as_bytes().len() > MAX_V2_HEADER_VALUE_LEN {
        return None;
    }
    raw.to_str().ok().map(str::to_string)
}
```

### 7. Exhaustive-variant tripwire tests
**Source:** `src/server/streamable_http_server.rs:2684-2691` and the
`all_meta_bearing_client_requests_are_extracted` pattern named in CONTEXT.md
**Apply to:** "`input_required` only on the 3 MRTR-eligible methods" (Pitfall 3),
"advertised subscription capability ⇒ `subscriptions/listen` served" (D-13)

```rust
    #[test]
    fn apply_v2_outbound_headers_sets_all_three_without_panic() {
        let mut h = HeaderMap::new();
        apply_v2_outbound_headers(&mut h, "tools/call", "search");
        assert_eq!(h.get(MCP_METHOD).unwrap(), "tools/call");
        assert_eq!(h.get(MCP_NAME).unwrap(), "search");
        assert_eq!(h.get(MCP_PROTOCOL_VERSION).unwrap(), V2);
    }
```

### 8. Builder opt-in doc contract
**Source:** `src/server/mod.rs:2544-2558`
**Apply to:** `Client::with_protocol_version`, `mrtr_round_limit`, the
`requestState` key/TTL config, the `subscriptions/listen` capability opt-in

Every new builder method: `#[must_use]`, a doc line stating *"with no call,
behaves exactly as today"*, a cross-link to its twin on the other builder, and a
`Default` entry in the same struct-literal block (`mod.rs:2540`).

---

## No Analog Found

None. Every file in this phase has an in-repo analog.

Two items are **partial-analog** and need planner attention because the analog
must be *deviated from*:

| File | Analog | Required deviation |
|------|--------|--------------------|
| `src/client/mod.rs` (MRTR loop) | `call_tool_and_poll` `:1066-1140` | analog gives up with `Error::internal`; D-09 requires a **typed** `MrtrRoundLimitExceeded` |
| `tests/v2_stateless_http.rs` | `tests/v2_required_headers.rs:238-244` | analog builds with `StreamableHttpServerConfig::stateless()`; HTTP-01 tests MUST build with `Default::default()` (RESEARCH Pitfall 1) or the era gate is never exercised |

Two items have **no in-repo precedent for one sub-part**:

| Sub-part | Status | Guidance |
|----------|--------|----------|
| `ring::aead` seal/open | no in-repo AEAD usage exists (`ring` is only a transitive `rustls` dep today) | use RESEARCH "Code Examples → Verify a `requestState` token" verbatim; wrap it in the `pkce.rs` module shape above |
| `=?base64?…?=` header sentinel encoder | no in-repo implementation (`grep MCP_NAME src/shared/streamable_http.rs` → 0 matches) | new code; encoder lives beside `logical_name_key`'s consumer, unit-tested per the `pkce.rs` known-answer style |

---

## Metadata

**Analog search scope:** `src/server/`, `src/client/`, `src/shared/`, `src/types/`,
`src/error/`, `tests/`, `fuzz/fuzz_targets/`, `examples/`, `Cargo.toml`

**Files read for excerpt extraction (15):**
`src/shared/pkce.rs`, `src/shared/http_constants.rs`, `src/shared/streamable_http.rs`,
`src/types/elicitation.rs`, `src/types/tools.rs`, `src/types/mod.rs`,
`src/types/protocol/error_codes.rs`, `src/types/protocol/mod.rs`,
`src/server/core.rs`, `src/server/mod.rs`, `src/server/streamable_http_server.rs`,
`src/server/subscriptions.rs`, `src/server/cancellation.rs`, `src/server/auth/traits.rs`,
`src/server/observability/config.rs`, `src/client/mod.rs`, `src/client/host/mod.rs`,
`src/client/host/elicitation.rs`, `tests/v2_required_headers.rs`,
`fuzz/fuzz_targets/pkce_helper.rs`, `examples/t05_streamable_http_stateless.rs`

**Project skills checked:** `.claude/skills/` and `.agents/skills/` exist
(`spike-findings-rust-mcp-sdk`). RESEARCH already assessed them as not directly
relevant to Phase 113's protocol work; no pattern conflicts found.

**Pattern extraction date:** 2026-07-24
