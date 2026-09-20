//! Phase 113-06 (HTTP-02 / HTTP-03): live-HTTP acceptance for the `requestState`
//! verdict table.
//!
//! These tests drive a REAL `StreamableHttpServer` over a loopback TCP socket with
//! a raw `reqwest` client (NOT the in-memory transport — RESEARCH Pitfall 11), so
//! every status code, JSON-RPC envelope and MRTR field crosses the actual axum HTTP
//! boundary.
//!
//! # Keys come from the BUILDER, never from the environment
//!
//! Every server here is built with
//! [`ServerBuilder::with_request_state_key`](pmcp::ServerBuilder::with_request_state_key)
//! and every token is minted with the SAME bytes through the
//! [`pmcp::testing`] seam, which wraps the PRODUCTION codec. Plan 03 made the
//! codec server-instance-owned precisely so tests never have to mutate the
//! `requestState` key ENVIRONMENT VARIABLE — a process-global whose value is
//! order-dependent under `cargo test`'s in-process parallel threads. The
//! variable's name is deliberately not spelled anywhere in this file.
//!
//! # "Expired" is expressed as a zero TTL, not as a sleep
//!
//! `mint_request_state(.., Duration::ZERO, ..)` seals `exp == now`, which the
//! codec classifies as expired on the very next `verify`. That is deterministic
//! and instant; a `with_request_state_ttl(1s)` server plus a sleep would be
//! neither, and would also risk the server's own freshly minted reply token
//! expiring before the test could open it.
//!
//! Test reliability doctrine (carried from `tests/v2_required_headers.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after each
//! round-trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use common::v2::{post, spawn_default_config, v2_body, v2_headers, V1, V2};
use pmcp::server::auth::{AuthContext, AuthProvider};
use pmcp::server::{PromptHandler, Server};
use pmcp::testing::{mint_request_state, open_request_state, ANONYMOUS_PRINCIPAL};
use pmcp::types::elicitation::ElicitRequestParams;
use pmcp::types::mrtr::{InputRequest, InputRequests, MrtrSignal, MRTR_SIGNAL_META_KEY};
use pmcp::types::protocol::error_codes::INVALID_PARAMS;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::GetPromptResult;
use pmcp::{RequestHandlerExtra, ServerCapabilities, ToolHandler};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

/// The shared `requestState` key every server in this file is BUILT with.
const KEY: [u8; 32] = [0x37; 32];

/// A key NO server here holds — the D-04 "another instance minted it" case.
const FOREIGN_KEY: [u8; 32] = [0x99; 32];

/// The subject the auth-configured servers report for a credentialed request.
const BOB: &str = "bob";

/// A principal no server in this file ever authenticates as.
const ALICE: &str = "alice";

/// A comfortably-live continuation lifetime for the tokens these tests mint.
///
/// The expiry verdict is exercised with `Duration::ZERO` instead (see the module
/// docs), so nothing here ever has to race a clock.
const LIVE_TTL: Duration = Duration::from_mins(5);

/// The salient params dispatch derives from a `tools/call` for `search`.
///
/// The AEAD binds to what dispatch will EXECUTE, so a test token must be minted
/// against exactly this shape (see [`mint_request_state`]).
fn search_params(arguments: &Value) -> Value {
    json!({ "name": "search", "arguments": arguments })
}

// ===========================================================================
// Handlers that participate in MRTR.
// ===========================================================================

/// What one handler invocation observed.
#[derive(Debug, Clone)]
struct Observed {
    continuation: Option<Value>,
    round: Option<u8>,
    had_input_responses: bool,
}

/// A recording tool that resumes when it has a verified continuation and
/// otherwise signals `input_required`.
#[derive(Clone, Default)]
struct MrtrSearchTool {
    calls: Arc<AtomicUsize>,
    observed: Arc<Mutex<Vec<Observed>>>,
}

#[async_trait]
impl ToolHandler for MrtrSearchTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let observed = Observed {
            continuation: extra.mrtr_continuation().cloned(),
            round: extra.mrtr_round(),
            had_input_responses: extra.input_responses().is_some(),
        };
        let resuming = observed.continuation.is_some();
        self.observed
            .lock()
            .expect("observation log is not poisoned")
            .push(observed);
        if resuming {
            return Ok(json!({ "answer": "resumed" }));
        }
        // First call (or a strip-and-re-run): ask for input.
        let mut input_requests = InputRequests::new();
        input_requests.insert(
            "user_name".to_string(),
            InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
                message: "What is your name?".to_string(),
                requested_schema: json!({ "type": "object" }),
            })),
        );
        let signal = MrtrSignal {
            input_requests,
            continuation: json!({ "step": 1 }),
        };
        let mut meta = serde_json::Map::new();
        meta.insert(
            MRTR_SIGNAL_META_KEY.to_string(),
            serde_json::to_value(&signal).expect("the MRTR signal serializes"),
        );
        extra.set_result_meta(meta);
        Ok(json!({ "answer": "need-input" }))
    }
}

/// A trivial prompt, so `prompts/get` is a real cross-method replay target.
struct GreetingPrompt;

#[async_trait]
impl PromptHandler for GreetingPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(vec![], Some("greeting".to_string())))
    }
}

/// An auth provider that reports a FIXED subject when an `Authorization` header
/// is present, and NO context at all when it is absent.
///
/// The absent case is what makes `unauthenticated_on_auth_server_refused`
/// meaningful: the server HAS a provider (so MRTR must fail closed) yet the
/// request produced no `AuthContext`.
struct FixedSubjectAuth(&'static str);

#[async_trait]
impl AuthProvider for FixedSubjectAuth {
    async fn validate_request(&self, header: Option<&str>) -> pmcp::Result<Option<AuthContext>> {
        Ok(header.map(|_| AuthContext::new(self.0)))
    }
}

// ===========================================================================
// Spawning.
// ===========================================================================

/// A v2-opted-in server holding [`KEY`], optionally behind a fixed-subject auth
/// provider.
fn build_server(tool: MrtrSearchTool, auth_subject: Option<&'static str>) -> Server {
    let builder = Server::builder()
        .name("v2-mrtr-harness")
        .version("1.0.0")
        .capabilities(ServerCapabilities::default())
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .with_request_state_key(KEY)
        .tool("search", tool)
        .prompt("greeting", GreetingPrompt);
    let builder = match auth_subject {
        Some(subject) => builder.auth_provider(FixedSubjectAuth(subject)),
        None => builder,
    };
    builder.build().expect("server builds")
}

/// Spawn a server with no auth provider (the [`ANONYMOUS_PRINCIPAL`] case).
async fn spawn_anon() -> (SocketAddr, JoinHandle<()>, MrtrSearchTool) {
    let tool = MrtrSearchTool::default();
    let (addr, handle) = spawn_default_config(build_server(tool.clone(), None)).await;
    (addr, handle, tool)
}

/// Spawn a server behind a fixed-subject auth provider.
async fn spawn_auth(subject: &'static str) -> (SocketAddr, JoinHandle<()>, MrtrSearchTool) {
    let tool = MrtrSearchTool::default();
    let (addr, handle) = spawn_default_config(build_server(tool.clone(), Some(subject))).await;
    (addr, handle, tool)
}

/// A `tools/call` retry body carrying `requestState` (and optionally the
/// symmetric `inputResponses`) as TOP-LEVEL `params` siblings.
fn retry_body(id: i64, arguments: &Value, request_state: Value, with_responses: bool) -> String {
    let mut params = search_params(arguments);
    let object = params.as_object_mut().expect("params is an object");
    object.insert("requestState".to_string(), request_state);
    if with_responses {
        object.insert(
            "inputResponses".to_string(),
            json!({ "user_name": { "action": "accept", "content": { "user_name": "Alice" } } }),
        );
    }
    v2_body("tools/call", json!(id), params)
}

/// The `Authorization` header a credentialed request sends.
fn bearer() -> Vec<(String, String)> {
    let mut headers = v2_headers("tools/call", "search");
    headers.push(("authorization".to_string(), "Bearer test-token".to_string()));
    headers
}

// ===========================================================================
// Verdict: Ok — the retry resumes.
// ===========================================================================

#[tokio::test]
async fn valid_state_resumes() {
    let (addr, handle, tool) = spawn_anon().await;
    let token = mint_request_state(
        &KEY,
        LIVE_TTL,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&json!({})),
        &json!({ "step": 1 }),
        0,
    )
    .expect("token mints");

    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(2, &json!({}), json!(token), true),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body was {}", response.raw);
    let result = response
        .body
        .get("result")
        .expect("a verified retry produces a result");
    assert_ne!(
        result.get("resultType").and_then(Value::as_str),
        Some("input_required"),
        "a live token must RESUME, not re-prompt: {result}"
    );

    // The handler saw both the decrypted continuation and the input responses.
    let observed = tool.observed.lock().expect("not poisoned").clone();
    assert_eq!(observed.len(), 1, "the handler ran exactly once");
    assert_eq!(observed[0].continuation, Some(json!({ "step": 1 })));
    assert_eq!(observed[0].round, Some(0));
    assert!(observed[0].had_input_responses);
}

// ===========================================================================
// Verdict: `AuthFailed` — a JSON-RPC error, never a re-prompt.
// ===========================================================================

/// The exact conformance mutation of `sep-2322-reject-tampered-state`.
///
/// A complete result OR a re-prompt is a conformance FAILURE, so the absence of
/// `result` is asserted explicitly rather than inferred from the error's
/// presence.
#[tokio::test]
async fn tampered_state_errors() {
    let (addr, handle, tool) = spawn_anon().await;
    let token = mint_request_state(
        &KEY,
        LIVE_TTL,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&json!({})),
        &json!({ "step": 1 }),
        0,
    )
    .expect("token mints");

    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(2, &json!({}), json!(format!("{token}-TAMPERED")), true),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_none(),
        "a tampered requestState must NEVER produce a result — neither a complete \
         one nor a re-prompt: {}",
        response.raw
    );
    assert_eq!(
        tool.calls.load(Ordering::SeqCst),
        0,
        "the handler must never be invoked for a tampered token"
    );
}

/// T-113-02: the binding principal is `AuthContext::subject` and nothing else,
/// so a token minted for `alice` cannot be redeemed by `bob`.
#[tokio::test]
async fn principal_mismatch_errors() {
    // The server authenticates every credentialed caller as `bob`; the token was
    // minted for `alice`. Both hold the SAME key, so this isolates the principal.
    let (addr, handle, tool) = spawn_auth(BOB).await;
    let token = mint_request_state(
        &KEY,
        LIVE_TTL,
        ALICE,
        "tools/call",
        &search_params(&json!({})),
        &json!({ "step": 1 }),
        0,
    )
    .expect("token mints");

    let response = post(
        addr,
        &bearer(),
        &retry_body(2, &json!({}), json!(token), true),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        response.raw
    );
    assert!(response.body.get("result").is_none());
    assert_eq!(tool.calls.load(Ordering::SeqCst), 0);
}

/// T-113-03: the digest of the salient params (and the method name) is in the
/// AAD, so a token cannot be replayed onto different arguments or onto a
/// different method.
#[tokio::test]
async fn originating_request_mismatch_errors() {
    let (addr, handle, tool) = spawn_anon().await;
    let token = mint_request_state(
        &KEY,
        LIVE_TTL,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&json!({ "q": "a" })),
        &json!({ "step": 1 }),
        0,
    )
    .expect("token mints");

    // (a) same method + same tool, DIFFERENT arguments.
    let different_args = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(2, &json!({ "q": "b" }), json!(token.clone()), false),
    )
    .await;
    assert_eq!(
        different_args.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        different_args.raw
    );
    assert!(different_args.body.get("result").is_none());

    // (b) the SAME token replayed onto a different method.
    let cross_method = post(
        addr,
        &v2_headers("prompts/get", "greeting"),
        &v2_body(
            "prompts/get",
            json!(3),
            json!({ "name": "greeting", "arguments": {}, "requestState": token }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(
        cross_method.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        cross_method.raw
    );
    assert!(cross_method.body.get("result").is_none());
    assert_eq!(
        tool.calls.load(Ordering::SeqCst),
        0,
        "neither replay may reach a handler"
    );
}

// ===========================================================================
// Verdict: `UnknownKey` / `Expired` — strip and RE-RUN.
// ===========================================================================

/// D-04's multi-instance degradation. The non-empty `inputRequests` assertion is
/// the whole point of the consensus fix: a state-only re-elicit would tell the
/// client to retry into the same failure forever.
#[tokio::test]
async fn unknown_key_reelicits_with_input_requests() {
    let (addr, handle, tool) = spawn_anon().await;
    let token = mint_request_state(
        &FOREIGN_KEY,
        LIVE_TTL,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&json!({})),
        &json!({ "step": 9 }),
        3,
    )
    .expect("token mints");

    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(2, &json!({}), json!(token), true),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body was {}", response.raw);
    let result = response
        .body
        .get("result")
        .expect("an unknown key RE-ELICITS, it does not error");
    assert_eq!(result["resultType"], "input_required", "got {result}");
    assert!(
        result["requestState"].is_string(),
        "the re-elicitation must carry a fresh requestState: {result}"
    );
    let input_requests = result["inputRequests"]
        .as_object()
        .expect("inputRequests is an object");
    assert!(
        !input_requests.is_empty(),
        "the re-elicitation must carry REAL inputRequests the client can answer, \
         not a state-only result: {result}"
    );

    // The re-run handler saw a PRISTINE first call.
    let observed = tool.observed.lock().expect("not poisoned").clone();
    assert_eq!(observed.len(), 1, "the original handler ran exactly once");
    assert_eq!(observed[0].continuation, None);
    assert_eq!(observed[0].round, None);
    assert!(
        !observed[0].had_input_responses,
        "a re-run must behave exactly as a first call"
    );
}

/// T-113-49: an authentic but expired token re-elicits the SAME way while
/// PRESERVING the round, so a server cannot reset a client's D-09 bound by
/// letting tokens expire.
#[tokio::test]
async fn expired_state_reelicits_preserving_round() {
    let (addr, handle, tool) = spawn_anon().await;
    // A zero TTL seals `exp == now` — already expired on the next verify.
    let token = mint_request_state(
        &KEY,
        Duration::ZERO,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&json!({})),
        &json!({ "step": 9 }),
        3,
    )
    .expect("token mints");

    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(2, &json!({}), json!(token), true),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body was {}", response.raw);
    let result = response
        .body
        .get("result")
        .expect("an expired token RE-ELICITS, it does not error");
    assert_eq!(result["resultType"], "input_required", "got {result}");
    let input_requests = result["inputRequests"]
        .as_object()
        .expect("inputRequests is an object");
    assert!(!input_requests.is_empty(), "got {result}");

    // Decrypt the returned token with the shared key: the round SURVIVED.
    let fresh = result["requestState"].as_str().expect("a fresh token");
    let (state, round) = open_request_state(
        &KEY,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&json!({})),
        fresh,
    )
    .expect("the server's own token verifies against the shared key");
    assert!(
        round > 0,
        "the expired token's round must be carried into the fresh one, got {round}"
    );
    assert_eq!(round, 4, "round 3 preserved, minted at round + 1");
    assert_eq!(state, json!({ "step": 1 }));

    // The re-run handler still saw a pristine first call.
    let observed = tool.observed.lock().expect("not poisoned").clone();
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].continuation, None);
    assert_eq!(observed[0].round, None);
    assert!(!observed[0].had_input_responses);
}

// ===========================================================================
// Malformed input is REJECTED, never silently absent (T-113-44).
// ===========================================================================

#[tokio::test]
async fn malformed_state_is_rejected_not_ignored() {
    let (addr, handle, tool) = spawn_anon().await;

    // (a) `requestState` is a JSON NUMBER.
    let not_a_string = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(2, &json!({}), json!(42), false),
    )
    .await;
    assert_eq!(not_a_string.status, 400, "body was {}", not_a_string.raw);
    assert_eq!(not_a_string.body["error"]["code"], INVALID_PARAMS);
    assert!(not_a_string.body.get("result").is_none());

    // (b) `requestState` is a 9000-character string (over MAX_REQUEST_STATE_LEN).
    let oversized = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(3, &json!({}), json!("x".repeat(9000)), false),
    )
    .await;
    handle.abort();

    assert_eq!(oversized.status, 400, "body was {}", oversized.raw);
    assert_eq!(oversized.body["error"]["code"], INVALID_PARAMS);
    assert!(oversized.body.get("result").is_none());

    assert_eq!(
        tool.calls.load(Ordering::SeqCst),
        0,
        "a malformed MRTR field must be rejected BEFORE any handler runs"
    );
}

#[tokio::test]
async fn oversized_input_responses_rejected() {
    let (addr, handle, tool) = spawn_anon().await;
    // 65 entries — one over MAX_INPUT_RESPONSES.
    let mut entries = serde_json::Map::new();
    for index in 0..65 {
        entries.insert(format!("k{index}"), json!({ "action": "accept" }));
    }
    let mut params = search_params(&json!({}));
    params
        .as_object_mut()
        .expect("params is an object")
        .insert("inputResponses".to_string(), Value::Object(entries));

    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_body("tools/call", json!(2), params),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 400, "body was {}", response.raw);
    assert_eq!(response.body["error"]["code"], INVALID_PARAMS);
    assert!(response.body.get("result").is_none());
    assert_eq!(tool.calls.load(Ordering::SeqCst), 0);
}

// ===========================================================================
// Fail-closed identity and method confinement.
// ===========================================================================

/// T-113-22: on a server that HAS an auth provider, an unauthenticated request
/// cannot redeem a state-bearing continuation — verification is not even
/// attempted.
#[tokio::test]
async fn unauthenticated_on_auth_server_refused() {
    let (addr, handle, tool) = spawn_auth(BOB).await;
    let token = mint_request_state(
        &KEY,
        LIVE_TTL,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&json!({})),
        &json!({ "step": 1 }),
        0,
    )
    .expect("token mints");

    // No `Authorization` header: the provider yields no AuthContext at all.
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(2, &json!({}), json!(token), true),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        response.raw
    );
    assert!(response.body.get("result").is_none());
    assert_eq!(
        tool.calls.load(Ordering::SeqCst),
        0,
        "an unauthenticated caller must not reach the handler on an auth-configured server"
    );
}

// ===========================================================================
// D-113-M: a request that cannot be canonicalized cannot be bound, so it can
// neither mint nor present a continuation (T-113-122/123/125/126).
// ===========================================================================

/// Mirror of the crate-internal `MAX_CANONICAL_DEPTH`, which is `pub(crate)` and
/// therefore unnameable from an integration test.
///
/// NOT a comment-pinned duplicate:
/// [`the_mirrored_depth_cap_matches_the_live_one`] MEASURES the live bound by
/// walking `mint_request_state` until it refuses, and asserts the measurement
/// equals this value. Both halves cannot drift silently.
const CANONICAL_DEPTH_CAP: usize = 64;

/// `arguments` nested `levels` objects deep around `leaf`.
///
/// The salient whitelist wrapper costs one canonical level, so the deepest
/// `arguments` that still binds is `CANONICAL_DEPTH_CAP - 1` levels.
fn nested_arguments(levels: usize, leaf: &str) -> Value {
    let mut value = json!(leaf);
    for _ in 0..levels {
        value = json!({ "n": value });
    }
    value
}

/// Walk the depth ladder until minting refuses, and check the mirror.
///
/// This is what makes [`CANONICAL_DEPTH_CAP`] a measurement rather than a
/// comment. `mint_request_state` wraps the PRODUCTION codec and the PRODUCTION
/// binding, so `None` here is exactly the condition the server refuses on.
#[test]
fn the_mirrored_depth_cap_matches_the_live_one() {
    let mints_at = |levels: usize| {
        mint_request_state(
            &KEY,
            LIVE_TTL,
            ANONYMOUS_PRINCIPAL,
            "tools/call",
            &search_params(&nested_arguments(levels, "leaf")),
            &json!({ "step": 1 }),
            0,
        )
        .is_some()
    };

    // A hard scan bound sized as a multiple of the mirror, so a regression that
    // removed the cap fails FAST instead of hanging the suite.
    let observed = (0..CANONICAL_DEPTH_CAP * 3)
        .find(|&levels| !mints_at(levels))
        .expect("the depth cap must refuse SOMEWHERE below 3x the mirrored value");

    assert_eq!(
        observed, CANONICAL_DEPTH_CAP,
        "the live canonicalization cap moved; update CANONICAL_DEPTH_CAP"
    );
    assert!(
        mints_at(observed - 1),
        "the boundary must be exact: one level shallower still mints"
    );
}

/// THE D-113-M END-TO-END PROOF, in two halves.
///
/// **Half 1 (the fix).** Requests A and B whose `arguments` are identical down to
/// the canonicalization cap and differ BELOW it used to digest identically, so a
/// `requestState` minted for A verified against B. A can no longer mint at all:
/// the server answers `INVALID_PARAMS` and issues no token, so there is nothing
/// to cross-verify.
///
/// **Half 2 (the control).** Requests A' and B' differing ABOVE the cap mint and
/// cross-reject exactly as they always did. Without this half, half 1 would be
/// consistent with the server having simply stopped minting — this shows normal
/// cross-request rejection is untouched, and that the recovered token is the real
/// minted continuation, opened with the server's own key (the 113-11 pattern)
/// rather than trusted because it round-tripped.
#[tokio::test]
async fn a_token_minted_for_one_request_is_refused_on_a_request_differing_below_the_depth_cap() {
    // ---- Half 1: A cannot mint, so it can never be cross-verified. ----
    let (addr, handle, tool) = spawn_anon().await;
    let deep_a = nested_arguments(CANONICAL_DEPTH_CAP, "SECRET-A");
    let deep_b = nested_arguments(CANONICAL_DEPTH_CAP, "SECRET-B");
    assert_ne!(deep_a, deep_b, "the fixtures must genuinely differ");

    // The handler signals `input_required`, which is what would mint.
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_body("tools/call", json!(2), search_params(&deep_a)),
    )
    .await;

    assert_eq!(
        response.body["error"]["code"], INVALID_PARAMS,
        "an over-deep request must be refused at the mint path, body was {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_none(),
        "no result may be emitted for an unbindable request: {}",
        response.raw
    );
    assert!(
        !response.raw.contains("requestState"),
        "no continuation may be minted for a request that cannot be identified: {}",
        response.raw
    );
    // The mint refusal is at EGRESS, so the handler did run — that is the
    // documented shape, and it is why nothing reaches the wire instead.
    assert_eq!(tool.calls.load(Ordering::SeqCst), 1);

    // And a token PRESENTED on the deep request is refused before verification.
    let presented = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(3, &deep_b, json!("any-token-at-all"), false),
    )
    .await;
    handle.abort();
    assert_eq!(
        presented.body["error"]["code"], INVALID_PARAMS,
        "body was {}",
        presented.raw
    );
    assert!(presented.body.get("result").is_none());
    assert_eq!(
        tool.calls.load(Ordering::SeqCst),
        1,
        "a state-bearing over-deep request must never reach the handler"
    );

    // ---- Half 2: the shallow control, unchanged. ----
    let (addr, handle, tool) = spawn_anon().await;
    let shallow_a = nested_arguments(CANONICAL_DEPTH_CAP - 2, "SECRET-A");
    let shallow_b = nested_arguments(CANONICAL_DEPTH_CAP - 2, "SECRET-B");

    let minted = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_body("tools/call", json!(2), search_params(&shallow_a)),
    )
    .await;
    let result = minted
        .body
        .get("result")
        .unwrap_or_else(|| panic!("A' must still mint, body was {}", minted.raw));
    assert_eq!(result["resultType"], "input_required", "got {result}");
    let token = result["requestState"]
        .as_str()
        .expect("A' mints a continuation");

    // Prove it is the REAL minted continuation by opening it with the server's
    // own key, rather than inferring that from a successful round-trip.
    let (state, round) = open_request_state(
        &KEY,
        ANONYMOUS_PRINCIPAL,
        "tools/call",
        &search_params(&shallow_a),
        token,
    )
    .expect("the server's own token opens against A' and the shared key");
    assert_eq!(state, json!({ "step": 1 }));
    assert_eq!(round, 1);

    // The same token does NOT open against B' — the digests differ.
    assert!(
        open_request_state(
            &KEY,
            ANONYMOUS_PRINCIPAL,
            "tools/call",
            &search_params(&shallow_b),
            token,
        )
        .is_none(),
        "a token minted for A' must not open against B'"
    );

    // ...and the server agrees, over real HTTP.
    let replayed = post(
        addr,
        &v2_headers("tools/call", "search"),
        &retry_body(3, &shallow_b, json!(token), true),
    )
    .await;
    handle.abort();

    assert_eq!(
        replayed.body["error"]["code"], INVALID_PARAMS,
        "the cross-request replay must still be rejected, body was {}",
        replayed.raw
    );
    assert!(replayed.body.get("result").is_none());
    assert_eq!(
        tool.calls.load(Ordering::SeqCst),
        1,
        "only the minting call reached the handler; the replay did not"
    );
}

/// T-113-23: MRTR is confined to `tools/call`, `prompts/get` and
/// `resources/read`. A `requestState` on any other method is IGNORED — not
/// verified, not errored.
#[tokio::test]
async fn request_state_on_non_mrtr_method_is_ignored() {
    let (addr, handle, _tool) = spawn_anon().await;
    let response = post(
        addr,
        &v2_headers("tools/list", ""),
        &v2_body(
            "tools/list",
            json!(2),
            json!({ "requestState": "not-even-a-real-token" }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body was {}", response.raw);
    let result = response
        .body
        .get("result")
        .expect("tools/list still succeeds");
    assert!(
        result["tools"].is_array(),
        "a requestState on a non-MRTR method must be inert: {result}"
    );
    assert!(
        result.get("requestState").is_none(),
        "no MRTR field may appear on a non-eligible method's result: {result}"
    );
}
