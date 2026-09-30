//! The two Phase 128 escape hatches, driven end to end (E1 + E2 / SC-5).
//!
//! # What this binary covers
//!
//! Every row goes through a SYNTHESIZED `[[tools]]` handler against a wiremock
//! backend — never a connector called directly — because the synthesized handler is
//! the path a real `tools/call` takes and is where the two hooks have to be
//! reachable. The rows:
//!
//! - `request_policy_refusal_makes_zero_upstream_requests` — the change request's E1
//!   acceptance row: a policy refusing a path prefix, asserting an empty
//!   `received_requests()` and the policy's OWN fixed message.
//! - `request_policy_allow_control_reaches_the_backend_with_auth` — the passing
//!   control. A refusal suite with no accept row cannot distinguish "refused
//!   correctly" from "handler broken", and asserting the auth header arrived proves
//!   the allowed path is fully wired THROUGH the hook.
//! - `request_policy_sees_resolved_path_not_template` — the substituted path, not
//!   `/content/{version}/CUI`.
//! - `request_policy_sees_the_tool_name` — the row that fails if the synthesized
//!   handler stops calling `HttpConnector::execute_for_tool`. It exists because a
//!   mutation test found that reverting that one call kills NOTHING in the unit
//!   suite: no unit test drives a synthesized handler over a policy-carrying
//!   connector, so this is the only place the attribution is proven.
//! - `request_policy_never_sees_credential` — the policy scans EVERY field of its
//!   `OutboundRequest` for the configured secret and fails the test if it finds one.
//!   T-128-39's mitigation is the hook POSITION; this is the assertion that the
//!   position still holds.
//! - `request_policy_refusal_is_idempotent` — the same refused call twice: same
//!   message, zero requests both times. A refusal is side-effect-free.
//! - `request_policy_argument_validator_refuses_a_cross_field_combination` — E2
//!   refusing a combination the declared schema permits, with zero upstream
//!   requests.
//! - `request_policy_argument_validator_not_invoked_when_schema_refuses` — a
//!   counting validator plus schema-violating arguments, asserting the counter
//!   stayed at ZERO. This is T-128-41's assertion: inverting D1 and E2 fails here.
//! - `request_policy_argument_validator_survives_the_schema_opt_out` — with
//!   `enforce_input_schema = false` AND a registered validator, an undeclared
//!   argument is accepted while the validator's own rule still refuses. The joint
//!   test plan 03 carries from the other side, so the two cannot drift.
//!
//! # Why it is named and gated this way
//!
//! Named `request_policy` so `--test request_policy` resolves to a dedicated
//! binary, and every test fn carries the `request_policy_` prefix so a positional
//! name filter resolves too. Listed in the Makefile's `REQUIRED_TEST_BINARIES` so a
//! `#[cfg]` gate turning false is a gate FAILURE rather than a silent zero.
//!
//! The file gate is `#![cfg(all(feature = "http", feature = "input-validation"))]`
//! and deliberately NOT `openapi-code-mode`: both hooks are proven on the LIGHT,
//! JS-engine-free curated build, which is the toolkit's common case. The Code Mode
//! surface's own mirror of these assertions lives in
//! `src/code_mode.rs::request_policy_seam`, where the executor's private internals
//! are reachable.

#![cfg(all(feature = "http", feature = "input-validation"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use pmcp::RequestHandlerExtra;
use pmcp_server_toolkit::config::{
    ParamDecl, ServerConfig, ServerSection, ToolDecl, ValidationSection,
};
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::http::{HttpClient, HttpConnector};
use pmcp_server_toolkit::{
    async_trait, synthesize_from_config_with_http_connector_and_hooks, ArgumentRefusal,
    ArgumentValidator, OutboundRequest, PolicyRefusal, RequestPolicy, ToolkitHooks,
};
use serde_json::{json, Value};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The credential the connector adds SERVER-SIDE. Every field of every
/// `OutboundRequest` is scanned for it.
const SERVER_SIDE_TOKEN: &str = "server-side-bearer-9f2c";

/// The tool's `[[tools]]` path template — one whole-segment placeholder.
const TOOL_PATH: &str = "/content/{version}/CUI";

/// The tool's name, as the policy must see it in `OutboundRequest.tool`.
const TOOL_NAME: &str = "get_content";

/// The message the refusing policy returns. A FIXED string: it names the rule and
/// carries no byte of the request.
const REFUSAL: &str = "outbound endpoint is not on the allowlist";

/// One recorded `OutboundRequest`, flattened to owned values so the test can assert
/// on it after the borrow is gone.
#[derive(Debug, Clone)]
struct Recorded {
    tool: String,
    method: String,
    path: String,
    query: Vec<(String, String)>,
    body: Option<String>,
    call_id: String,
}

impl Recorded {
    /// Every field rendered into one string, for the credential scan. A scan that
    /// looked at only the fields the author thought of is the scan that misses.
    fn all_fields(&self) -> String {
        let query = self
            .query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.tool,
            self.method,
            self.path,
            query,
            self.body.clone().unwrap_or_default(),
            // The credential scan must cover EVERY field. `call_id` is minted here
            // and cannot carry a secret, but "cannot" is what this row asserts.
            self.call_id
        )
    }
}

/// Records every request it is shown, then allows or refuses.
struct Recorder {
    seen: Arc<Mutex<Vec<Recorded>>>,
    refuse: bool,
}

#[async_trait]
impl RequestPolicy for Recorder {
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal> {
        self.seen.lock().expect("lock").push(Recorded {
            tool: req.tool.to_string(),
            method: req.method.to_string(),
            path: req.path.to_string(),
            query: req.query.to_vec(),
            body: req.body.map(ToString::to_string),
            call_id: req.call_id.to_string(),
        });
        if self.refuse {
            Err(PolicyRefusal::new(REFUSAL))
        } else {
            Ok(())
        }
    }
}

/// A policy plus the log it writes.
fn recorder(refuse: bool) -> (Arc<Recorder>, Arc<Mutex<Vec<Recorded>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(Recorder {
            seen: Arc::clone(&seen),
            refuse,
        }),
        seen,
    )
}

/// Counts invocations, then refuses when `end` precedes `start` — a rule the
/// declared schema (two unconstrained integers) permits.
struct EndAfterStart {
    calls: Arc<AtomicUsize>,
}

impl ArgumentValidator for EndAfterStart {
    fn validate(&self, args: &Value) -> Result<(), ArgumentRefusal> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let start = args.get("start").and_then(Value::as_i64);
        let end = args.get("end").and_then(Value::as_i64);
        match (start, end) {
            (Some(s), Some(e)) if e < s => {
                Err(ArgumentRefusal::new("`end` must not precede `start`"))
            },
            _ => Ok(()),
        }
    }
}

/// A required parameter of `param_type`, with NO declared narrowing.
fn param(name: &str, param_type: &str) -> ParamDecl {
    ParamDecl {
        name: name.to_string(),
        param_type: Some(param_type.to_string()),
        required: true,
        ..Default::default()
    }
}

/// The one-tool config every row synthesizes from.
fn cfg(validation: ValidationSection) -> ServerConfig {
    ServerConfig {
        server: ServerSection {
            name: "umls-demo".to_string(),
            version: "0.1.0".to_string(),
            validation,
            ..Default::default()
        },
        tools: vec![ToolDecl {
            name: TOOL_NAME.to_string(),
            description: Some("Fetch content for a CUI".to_string()),
            path: Some(TOOL_PATH.to_string()),
            method: Some("GET".to_string()),
            parameters: vec![
                param("version", "string"),
                param("start", "integer"),
                param("end", "integer"),
            ],
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// An `HttpClient` pointed at `server` adding the static bearer server-side,
/// carrying `policy` when one is supplied.
fn connector_for(
    server: &MockServer,
    policy: Option<Arc<dyn RequestPolicy>>,
) -> Arc<dyn HttpConnector> {
    let auth = create_auth_provider(&AuthConfig::Bearer {
        token: SERVER_SIDE_TOKEN.to_string(),
        required: true,
    })
    .expect("bearer auth provider");
    let client =
        HttpClient::new(reqwest::Client::new(), server.uri(), auth).expect("build http client");
    let client = match policy {
        Some(p) => client.with_request_policy(p),
        None => client,
    };
    Arc::new(client)
}

/// Mount a CATCH-ALL mock and synthesize the tool's handler.
///
/// The catch-all is deliberate: with it mounted, a `received_requests()` of zero
/// can only mean the request was never made — never that it was made and went
/// unmatched.
async fn arrange(
    server: &MockServer,
    policy: Option<Arc<dyn RequestPolicy>>,
    hooks: &ToolkitHooks,
    validation: ValidationSection,
) -> Arc<dyn pmcp::server::ToolHandler> {
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(server)
        .await;
    let mut out = synthesize_from_config_with_http_connector_and_hooks(
        &cfg(validation),
        connector_for(server, policy),
        hooks,
    )
    .expect("synthesize one tool");
    assert_eq!(out.len(), 1, "exactly one synthesized tool expected");
    out.remove(0).2
}

/// Every request the mock actually observed.
async fn observed(server: &MockServer) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .expect("wiremock records requests")
}

/// A fully conforming argument set.
fn good_args() -> Value {
    json!({ "version": "current", "start": 1, "end": 9 })
}

// -----------------------------------------------------------------------------
// E1 — RequestPolicy
// -----------------------------------------------------------------------------

#[tokio::test]
async fn request_policy_refusal_makes_zero_upstream_requests() {
    let server = MockServer::start().await;
    let (policy, _seen) = recorder(true);
    let handler = arrange(
        &server,
        Some(policy),
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;

    let err = handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect_err("the policy refuses");

    assert!(
        err.to_string().contains(REFUSAL),
        "the refusal must carry the policy's OWN fixed message, got: {err}"
    );
    assert!(
        observed(&server).await.is_empty(),
        "a policy refusal must record ZERO upstream requests; got {}",
        observed(&server).await.len()
    );
}

/// Each curated `tools/call` is one request with its OWN non-empty `call_id`.
///
/// A `call_id` shared between two calls would let a policy that budgets per call
/// charge one caller for another's traffic; an empty one would silently disable
/// grouping. Refs #386.
#[tokio::test]
async fn request_policy_call_id_is_unique_per_curated_call() {
    let server = MockServer::start().await;
    let (policy, seen) = recorder(false);
    let handler = arrange(
        &server,
        Some(policy),
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;
    for _ in 0..2 {
        handler
            .handle(good_args(), RequestHandlerExtra::default())
            .await
            .expect("allowed");
    }
    let seen = seen.lock().expect("lock").clone();
    assert_eq!(seen.len(), 2, "one policy check per call");
    assert!(
        seen.iter().all(|r| !r.call_id.is_empty()),
        "a synthesized handler always attributes its call: {seen:?}"
    );
    assert_ne!(
        seen[0].call_id, seen[1].call_id,
        "two tools/call invocations must not share a call id"
    );
}

#[tokio::test]
async fn request_policy_allow_control_reaches_the_backend_with_auth() {
    let server = MockServer::start().await;
    let (policy, _seen) = recorder(false);
    let handler = arrange(
        &server,
        Some(policy),
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;

    let out = handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect("an allowing policy lets the call through");
    assert_eq!(out["ok"], true);

    let requests = observed(&server).await;
    assert_eq!(requests.len(), 1, "exactly one upstream request");
    let auth = requests[0]
        .headers
        .get("authorization")
        .expect("the auth header was applied AFTER the hook")
        .to_str()
        .expect("ascii");
    assert_eq!(auth, format!("Bearer {SERVER_SIDE_TOKEN}"));
}

#[tokio::test]
async fn request_policy_sees_resolved_path_not_template() {
    let server = MockServer::start().await;
    let (policy, seen) = recorder(false);
    let handler = arrange(
        &server,
        Some(policy),
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;
    handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect("allowed");

    let seen = seen.lock().expect("lock");
    assert_eq!(seen.len(), 1, "ONE invocation per logical outbound request");
    assert!(
        seen[0].path.ends_with("/content/current/CUI"),
        "the policy must see the SUBSTITUTED path, got {}",
        seen[0].path
    );
    assert!(
        !seen[0].path.contains('{'),
        "the policy must never see the template, got {}",
        seen[0].path
    );
    assert_eq!(seen[0].method, "GET");
}

/// The row that dies if `HttpToolHandler` stops calling `execute_for_tool`.
///
/// A mutation test proved that reverting that one call kills NOTHING in the unit
/// suite — no unit test drives a synthesized handler over a policy-carrying
/// connector. This is the only assertion that keeps the attribution honest.
#[tokio::test]
async fn request_policy_sees_the_tool_name() {
    let server = MockServer::start().await;
    let (policy, seen) = recorder(false);
    let handler = arrange(
        &server,
        Some(policy),
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;
    handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect("allowed");

    assert_eq!(
        seen.lock().expect("lock")[0].tool,
        TOOL_NAME,
        "the synthesized handler must attribute the request to its own tool"
    );
}

/// T-128-39: the mitigation is the hook POSITION (before `auth.apply`); this is the
/// assertion that the position still holds.
#[tokio::test]
async fn request_policy_never_sees_credential() {
    let server = MockServer::start().await;
    let (policy, seen) = recorder(false);
    let handler = arrange(
        &server,
        Some(policy),
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;
    handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect("allowed");

    // Control: the credential DID reach the wire, so a clean scan means the policy
    // could not see it — not that no credential existed.
    let requests = observed(&server).await;
    assert!(
        requests[0].headers.contains_key("authorization"),
        "the credential must actually be in play for this scan to mean anything"
    );

    for recorded in seen.lock().expect("lock").iter() {
        let rendered = recorded.all_fields();
        assert!(
            !rendered.contains(SERVER_SIDE_TOKEN),
            "the configured secret reached the policy: {rendered}"
        );
        assert!(
            !rendered.to_lowercase().contains("bearer"),
            "a credential scheme reached the policy: {rendered}"
        );
    }
}

#[tokio::test]
async fn request_policy_refusal_is_idempotent() {
    let server = MockServer::start().await;
    let (policy, _seen) = recorder(true);
    let handler = arrange(
        &server,
        Some(policy),
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;

    let first = handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect_err("refused")
        .to_string();
    assert!(observed(&server).await.is_empty());
    let second = handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect_err("refused again")
        .to_string();

    assert_eq!(first, second, "a refusal must be deterministic");
    assert!(
        observed(&server).await.is_empty(),
        "a refusal must be side-effect-free: still ZERO upstream requests"
    );
}

/// A server that registers nothing behaves exactly as it did before this phase.
#[tokio::test]
async fn request_policy_absent_leaves_the_request_path_unchanged() {
    let server = MockServer::start().await;
    let handler = arrange(
        &server,
        None,
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;
    let out = handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect("succeeds");
    assert_eq!(out["ok"], true);
    assert_eq!(observed(&server).await.len(), 1);
}

// -----------------------------------------------------------------------------
// E2 — ArgumentValidator
// -----------------------------------------------------------------------------

/// Hooks registering `EndAfterStart` for this tool, plus its call counter.
fn validator_hooks() -> (ToolkitHooks, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let hooks = ToolkitHooks::default().with_argument_validator(
        TOOL_NAME,
        Arc::new(EndAfterStart {
            calls: Arc::clone(&calls),
        }),
    );
    (hooks, calls)
}

#[tokio::test]
async fn request_policy_argument_validator_refuses_a_cross_field_combination() {
    let server = MockServer::start().await;
    let (hooks, calls) = validator_hooks();
    let handler = arrange(&server, None, &hooks, ValidationSection::default()).await;

    // Both integers satisfy the declared schema; only E2 can express the relation.
    let err = handler
        .handle(
            json!({ "version": "current", "start": 9, "end": 1 }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("the validator refuses");

    assert!(
        err.to_string().contains("`end` must not precede `start`"),
        "the refusal must carry the validator's own message, got: {err}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        observed(&server).await.is_empty(),
        "an E2 refusal must record ZERO upstream requests"
    );
}

/// T-128-41: inverting D1 and E2 fails HERE.
#[tokio::test]
async fn request_policy_argument_validator_not_invoked_when_schema_refuses() {
    let server = MockServer::start().await;
    let (hooks, calls) = validator_hooks();
    let handler = arrange(&server, None, &hooks, ValidationSection::default()).await;

    // `start` is a declared integer; a string violates the declared schema.
    let err = handler
        .handle(
            json!({ "version": "current", "start": "nine", "end": 1 }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("D1 refuses");

    assert!(
        !err.to_string().contains("must not precede"),
        "D1 must be the refuser, not E2, got: {err}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "the validator saw arguments the declared schema already refused"
    );
    assert!(observed(&server).await.is_empty());
}

/// The joint test plan 03 carries from the other side: turning off one enforcement
/// must never silently turn off another.
#[tokio::test]
async fn request_policy_argument_validator_survives_the_schema_opt_out() {
    let server = MockServer::start().await;
    let (hooks, calls) = validator_hooks();
    let validation = ValidationSection {
        enforce_input_schema: false,
        ..ValidationSection::default()
    };
    let handler = arrange(&server, None, &hooks, validation).await;

    // Half one: the SCHEMA check is off, so an undeclared key is accepted.
    let out = handler
        .handle(
            json!({ "version": "current", "start": 1, "end": 9, "undeclared": true }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect("with enforce_input_schema=false an undeclared key is accepted");
    assert_eq!(out["ok"], true);

    // Half two: the VALIDATOR still refuses.
    let err = handler
        .handle(
            json!({ "version": "current", "start": 9, "end": 1 }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("the validator still refuses");
    assert!(
        err.to_string().contains("`end` must not precede `start`"),
        "a registered validator must survive the schema opt-out, got: {err}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        observed(&server).await.len(),
        1,
        "exactly the accepted call reached the backend"
    );
}

/// With NEITHER hook registered, both surfaces are present and inert (SC-5 empty).
#[tokio::test]
async fn request_policy_neither_hook_registered_is_present_but_inert() {
    let server = MockServer::start().await;
    let handler = arrange(
        &server,
        None,
        &ToolkitHooks::default(),
        ValidationSection::default(),
    )
    .await;
    // D1 still enforces — SC-5's surfaces being inert must not disable D1.
    handler
        .handle(
            json!({ "version": "current", "start": "nine", "end": 1 }),
            RequestHandlerExtra::default(),
        )
        .await
        .expect_err("D1 still refuses a schema violation");
    assert!(observed(&server).await.is_empty());
    // And a conforming call goes through untouched.
    handler
        .handle(good_args(), RequestHandlerExtra::default())
        .await
        .expect("succeeds");
    assert_eq!(observed(&server).await.len(), 1);
}
