//! AUTH-01 end to end: an authorization code that fails `state` or `iss`
//! validation can never reach the token endpoint, and the page the human sees
//! always matches what actually happened.
//!
//! # What makes these tests possible at all
//!
//! The interactive flow used to call `webbrowser::open()` directly, so no test
//! could see the generated `state`, stop a real browser window appearing on the
//! developer's machine, or deliver a legitimate callback. Plan 116-09 Task 1
//! added the `BrowserLauncher` seam; every test here drives the REAL
//! `OAuthHelper::authorize_with_details()` through it. The launcher captures the
//! authorization URL, lifts the `state` out of it, and performs a raw loopback
//! GET against the flow's own callback listener with whatever parameters the
//! case requires.
//!
//! # The three assertions every rejection row makes
//!
//! Cross-AI review (Codex HIGH #3) rejected an earlier design in which the
//! listener served the browser page and THEN handed the query to the parent for
//! validation. That ordering is self-contradictory: the task does not know the
//! outcome at the moment it must choose which page to write, so a callback later
//! rejected would already have rendered SUCCESS, and the failure page would be
//! unselectable. Validation therefore happens INSIDE the listener before any
//! response byte is committed, and each rejection row proves all three
//! consequences of that single `Result`:
//!
//! 1. the marker predicate (`is_iss_mismatch` / `is_state_mismatch`) — the
//!    stable programmatic discriminator, never a message substring;
//! 2. the BYTES the callback GET received are the FAILURE page, not the success
//!    page — the assertion the rejected design could not have satisfied;
//! 3. an `expect(0)` mock on `/token` with `assert_async()` — the code was never
//!    exchanged.

#![cfg(feature = "oauth")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mockito::{Mock, Server, ServerGuard};
use pmcp::client::oauth::{BrowserLauncher, OAuthConfig, OAuthHelper};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

/// Substituted with the `state` lifted out of the captured authorization URL.
const STATE_PLACEHOLDER: &str = "{STATE}";

/// The distinguishing fragment of the page served when validation PASSED.
const SUCCESS_MARKER: &str = "Authentication Successful!";
/// The distinguishing fragment of the page served when validation FAILED.
const FAILURE_MARKER: &str = "Authentication Failed";

/// What the driving launcher should put on the wire.
#[derive(Clone, Debug)]
enum CallbackRequest {
    /// A normal `GET /callback?<query> HTTP/1.1`, with `{STATE}` substituted.
    Query(String),
    /// A verbatim request line, for the malformed and oversize cases.
    RawLine(String),
}

/// A [`BrowserLauncher`] that completes the flow: it captures the authorization
/// URL and then delivers a callback of the test's choosing to the loopback
/// listener the flow has already bound.
#[derive(Debug)]
struct CallbackDrivingLauncher {
    port: u16,
    request: CallbackRequest,
    captured_url: Arc<Mutex<Option<String>>>,
    served_bytes: Arc<Mutex<Option<String>>>,
}

impl BrowserLauncher for CallbackDrivingLauncher {
    fn open(&self, url: &str) -> pmcp::Result<()> {
        *self.captured_url.lock().expect("captured_url") = Some(url.to_string());

        let state = Url::parse(url)
            .ok()
            .and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "state")
                    .map(|(_, v)| v.into_owned())
            })
            .unwrap_or_default();

        let line = match &self.request {
            CallbackRequest::Query(q) => {
                format!(
                    "GET /callback?{} HTTP/1.1",
                    q.replace(STATE_PLACEHOLDER, &state)
                )
            },
            CallbackRequest::RawLine(raw) => raw.clone(),
        };

        // The listener is already bound (the flow binds before it builds the
        // URL), so the connection completes from the accept backlog even though
        // the flow has not reached `accept()` yet. The GET runs on its own task
        // because `open` is synchronous and the flow must be free to accept.
        let port = self.port;
        let served = self.served_bytes.clone();
        tokio::spawn(async move {
            let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)).await else {
                return;
            };
            let request = format!("{line}\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
            if stream.write_all(request.as_bytes()).await.is_err() {
                return;
            }
            let _ = stream.flush().await;
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response).await;
            *served.lock().expect("served_bytes") =
                Some(String::from_utf8_lossy(&response).into_owned());
        });

        Ok(())
    }
}

/// An ephemeral loopback port for one test's callback listener.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("a loopback port")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// A discovery document, parameterised on the RFC 9207 flag and on the `issuer`
/// the authorization server declares.
///
/// `iss_flag: None` OMITS `authorization_response_iss_parameter_supported`
/// entirely, which is the D-01 floor case — the server said nothing.
fn discovery_body(base: &str, iss_flag: Option<bool>) -> String {
    let mut doc = json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code"],
        "scopes_supported": ["openid"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
    });
    if let Some(flag) = iss_flag {
        doc["authorization_response_iss_parameter_supported"] = json!(flag);
    }
    doc.to_string()
}

/// The outcome of one driven flow.
struct DrivenFlow {
    outcome: pmcp::Result<pmcp::client::oauth::AuthorizationResult>,
    served: String,
    elapsed: Duration,
    _server: ServerGuard,
    token_mock: Mock,
}

impl DrivenFlow {
    fn error(&self) -> &pmcp::Error {
        self.outcome
            .as_ref()
            .expect_err("this row must be a refusal")
    }

    /// Assert the two page-level halves of a rejection, then the zero-call mock.
    async fn assert_refused_with_failure_page(&self) {
        assert!(
            self.served.contains(FAILURE_MARKER),
            "the browser must have received the FAILURE page; got: {}",
            self.served
        );
        assert!(
            !self.served.contains(SUCCESS_MARKER),
            "a rejected callback must NEVER see the success page; got: {}",
            self.served
        );
        self.token_mock.assert_async().await;
    }
}

/// Run one complete flow: mock discovery, mock `/token`, drive the callback.
///
/// `token_calls_expected` is the `expect(N)` on `/token`. Every rejection row
/// passes 0 — that mock IS the proof that an unvalidated code is never
/// exchanged.
async fn drive_flow(
    iss_flag: Option<bool>,
    request: CallbackRequest,
    token_calls_expected: usize,
) -> DrivenFlow {
    let mut server = Server::new_async().await;
    let base = server.url();

    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, iss_flag))
        .create_async()
        .await;

    let token_mock = server
        .mock("POST", "/token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "access_token": "granted-access-token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "refresh_token": "granted-refresh-token",
                "scope": "openid",
            })
            .to_string(),
        )
        .expect(token_calls_expected)
        .create_async()
        .await;

    let port = free_port();
    let captured_url = Arc::new(Mutex::new(None));
    let served_bytes = Arc::new(Mutex::new(None));

    let launcher = Arc::new(CallbackDrivingLauncher {
        port,
        request,
        captured_url,
        served_bytes: served_bytes.clone(),
    });

    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("iss-integration".into()),
        dcr_enabled: false,
        scopes: vec!["openid".into()],
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher);

    let started = Instant::now();
    let outcome = helper.authorize_with_details().await;
    let elapsed = started.elapsed();

    // The driving task reads the response to EOF; give it a moment to land.
    let mut served = String::new();
    for _ in 0..200 {
        if let Some(bytes) = served_bytes.lock().expect("served_bytes").clone() {
            served = bytes;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // The flow's own callback timeout is 5 minutes; every row here must resolve
    // in seconds. This is the "does not hang" assertion for the malformed rows.
    assert!(
        elapsed < Duration::from_secs(60),
        "the flow took {elapsed:?} — it must not hang waiting on a callback it already handled"
    );

    DrivenFlow {
        outcome,
        served,
        elapsed,
        _server: server,
        token_mock,
    }
}

// ---------------------------------------------------------------------------
// Group 1 — the four RFC 9207 `iss` table rows, end to end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_matching_state_with_a_different_iss_is_refused_and_never_redeems() {
    // Row 1 failing: the authorization server ADVERTISED `iss` and the callback
    // carries one naming a different authorization server. This is the
    // mix-up attack RFC 9207 exists to stop, and it must not reach `/token`.
    let flow = drive_flow(
        Some(true),
        CallbackRequest::Query(format!(
            "code=good-code&state={STATE_PLACEHOLDER}&iss={}",
            urlencoding("https://attacker-authorization-server.example")
        )),
        0,
    )
    .await;
    assert!(
        flow.error().is_iss_mismatch(),
        "an iss naming a different authorization server must be refused: {}",
        flow.error()
    );
    assert_eq!(
        flow.error().iss_actual(),
        Some("https://attacker-authorization-server.example"),
        "the typed accessor must report what was actually received"
    );
    flow.assert_refused_with_failure_page().await;
}

#[tokio::test]
async fn the_happy_path_serves_the_success_page_and_calls_the_token_endpoint() {
    // Driving the true happy path needs the callback to echo the flow's OWN
    // issuer, which is only known after the mock server is created — so this row
    // builds its server inline rather than through `drive_flow`.
    let mut server = Server::new_async().await;
    let base = server.url();

    let _disc = server
        .mock("GET", "/.well-known/openid-configuration")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(discovery_body(&base, Some(true)))
        .create_async()
        .await;

    let token_mock = server
        .mock("POST", "/token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "access_token": "granted-access-token",
                "token_type": "Bearer",
                "expires_in": 3600,
                "refresh_token": "granted-refresh-token",
                "scope": "openid profile",
            })
            .to_string(),
        )
        .expect(1)
        .create_async()
        .await;

    let port = free_port();
    let served_bytes = Arc::new(Mutex::new(None));
    let launcher = Arc::new(CallbackDrivingLauncher {
        port,
        request: CallbackRequest::Query(format!(
            "code=good-code&state={STATE_PLACEHOLDER}&iss={}",
            urlencoding(&base)
        )),
        captured_url: Arc::new(Mutex::new(None)),
        served_bytes: served_bytes.clone(),
    });

    let helper = OAuthHelper::new(OAuthConfig {
        mcp_server_url: Some(base.clone()),
        client_id: Some("happy-path".into()),
        dcr_enabled: false,
        scopes: vec!["openid".into()],
        redirect_port: port,
        ..OAuthConfig::default()
    })
    .expect("helper")
    .with_browser_launcher(launcher);

    let result = helper
        .authorize_with_details()
        .await
        .expect("a matching state and a matching iss must authorize");

    assert_eq!(result.access_token, "granted-access-token");
    assert_eq!(
        result.refresh_token.as_deref(),
        Some("granted-refresh-token")
    );
    assert_eq!(result.scopes, vec!["openid", "profile"]);
    token_mock.assert_async().await;

    let mut served = String::new();
    for _ in 0..200 {
        if let Some(bytes) = served_bytes.lock().expect("served").clone() {
            served = bytes;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // The SUCCESS page, asserted by observation — so the two branches are
    // distinguished by what the browser received, not by assumption.
    assert!(
        served.contains(SUCCESS_MARKER),
        "the browser must have received the SUCCESS page; got: {served}"
    );
    assert!(
        served.contains("HTTP/1.1 200 OK"),
        "the success response must be a 200; got: {served}"
    );
    assert!(
        !served.contains(FAILURE_MARKER),
        "the success branch must not serve the failure page; got: {served}"
    );
}

#[tokio::test]
async fn an_absent_iss_against_an_advertising_server_is_refused() {
    // Row 2: the document said `authorization_response_iss_parameter_supported:
    // true` and the callback carries no `iss` at all.
    let flow = drive_flow(
        Some(true),
        CallbackRequest::Query(format!("code=good-code&state={STATE_PLACEHOLDER}")),
        0,
    )
    .await;
    assert!(
        flow.error().is_iss_mismatch(),
        "advertised-but-absent must be an iss mismatch: {}",
        flow.error()
    );
    assert_eq!(
        flow.error().iss_actual(),
        None,
        "absence is expressed as iss_actual() == None, not as a second marker"
    );
    flow.assert_refused_with_failure_page().await;
}

#[tokio::test]
async fn an_absent_iss_against_a_silent_server_proceeds() {
    // Row 4 — D-01's floor: lenient ONLY when the authorization server says
    // nothing and sends nothing. This is the row that keeps existing v1
    // deployments working, so it must reach the token endpoint.
    let flow = drive_flow(
        None,
        CallbackRequest::Query(format!("code=good-code&state={STATE_PLACEHOLDER}")),
        1,
    )
    .await;
    assert!(
        flow.outcome.is_ok(),
        "a silent server plus a silent callback must proceed: {:?}",
        flow.outcome.as_ref().err().map(ToString::to_string)
    );
    assert!(
        flow.served.contains(SUCCESS_MARKER),
        "the floor case must serve the SUCCESS page; got: {}",
        flow.served
    );
    flow.token_mock.assert_async().await;
}

#[tokio::test]
async fn a_present_but_different_iss_is_refused_even_when_nothing_was_advertised() {
    // Row 3: the floor has teeth. A PRESENT `iss` is always compared, whatever
    // the metadata said.
    let flow = drive_flow(
        None,
        CallbackRequest::Query(format!(
            "code=good-code&state={STATE_PLACEHOLDER}&iss=https%3A%2F%2Fevil.example"
        )),
        0,
    )
    .await;
    assert!(
        flow.error().is_iss_mismatch(),
        "a present iss is compared even under IssPresence::Optional: {}",
        flow.error()
    );
    assert_eq!(flow.error().iss_actual(), Some("https://evil.example"));
    flow.assert_refused_with_failure_page().await;
}

// ---------------------------------------------------------------------------
// Group 2 — CSRF `state`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_mismatched_state_is_refused_before_the_iss_is_even_considered() {
    let flow = drive_flow(
        None,
        CallbackRequest::Query("code=good-code&state=forged-by-an-attacker".to_string()),
        0,
    )
    .await;
    assert!(
        flow.error().is_state_mismatch(),
        "a forged state must be a state mismatch: {}",
        flow.error()
    );
    assert!(
        !flow.error().to_string().contains("forged-by-an-attacker"),
        "the refusal must not echo the state it was given: {}",
        flow.error()
    );
    flow.assert_refused_with_failure_page().await;
}

#[tokio::test]
async fn an_absent_state_is_a_mismatch_not_a_skip() {
    let flow = drive_flow(
        None,
        CallbackRequest::Query("code=good-code".to_string()),
        0,
    )
    .await;
    assert!(
        flow.error().is_state_mismatch(),
        "an omitted state must not be treated as one that matched: {}",
        flow.error()
    );
    flow.assert_refused_with_failure_page().await;
}

#[tokio::test]
async fn a_duplicated_state_parameter_is_refused() {
    // A proxy taking the LAST occurrence and a client taking the FIRST disagree
    // about what was validated, so a repeat is a smuggling primitive.
    let flow = drive_flow(
        None,
        CallbackRequest::Query(format!(
            "code=good-code&state={STATE_PLACEHOLDER}&state=attacker-second-value"
        )),
        0,
    )
    .await;
    assert!(
        flow.outcome.is_err(),
        "a duplicated security parameter must be refused"
    );
    flow.assert_refused_with_failure_page().await;
}

// ---------------------------------------------------------------------------
// Group 3 — non-disclosure of authorization-server text
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_error_description_behind_a_wrong_iss_reaches_neither_the_error_nor_the_browser() {
    const MARKER: &str = "CANARY-1f4a9c-DO-NOT-DISCLOSE";

    let flow = drive_flow(
        None,
        CallbackRequest::Query(format!(
            "error=access_denied&error_description={MARKER}&state={STATE_PLACEHOLDER}\
             &iss=https%3A%2F%2Fevil.example"
        )),
        0,
    )
    .await;

    // The evaluation order is what implements the MUST NOT: `iss` is decided
    // before an authorization-server `error` may be surfaced at all.
    assert!(
        flow.error().is_iss_mismatch(),
        "the iss mismatch must win over the AS-supplied error: {}",
        flow.error()
    );
    assert!(
        !flow.error().to_string().contains(MARKER),
        "an error_description behind a failing iss must not be surfaced: {}",
        flow.error()
    );
    assert!(
        !flow.served.contains(MARKER),
        "the failure page must carry no authorization-server-supplied text: {}",
        flow.served
    );
    flow.assert_refused_with_failure_page().await;
}

// ---------------------------------------------------------------------------
// Group 4 — bounded, fail-closed handling of peer-controlled callback bytes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_request_line_over_the_transport_cap_is_refused_without_hanging() {
    // MAX_CALLBACK_REQUEST_LINE_BYTES is 16 KiB; 64 KiB of path is well over it.
    let huge = "A".repeat(64 * 1024);
    let flow = drive_flow(
        None,
        CallbackRequest::RawLine(format!("GET /callback?code=x&pad={huge} HTTP/1.1")),
        0,
    )
    .await;
    assert!(
        flow.outcome.is_err(),
        "an oversized request line must be refused"
    );
    assert!(
        !flow.error().to_string().contains("AAAA"),
        "the refusal must reproduce none of the refused bytes: {}",
        flow.error()
    );
    flow.assert_refused_with_failure_page().await;
}

#[tokio::test]
async fn a_query_over_the_validator_cap_is_refused_and_echoes_nothing() {
    // Between MAX_CALLBACK_QUERY_BYTES (8 KiB) and
    // MAX_CALLBACK_REQUEST_LINE_BYTES (16 KiB): the transport cap admits it and
    // the pure validator refuses it, which is the seam being exercised.
    let padding = "B".repeat(10 * 1024);
    let flow = drive_flow(
        None,
        CallbackRequest::Query(format!(
            "code=good-code&state={STATE_PLACEHOLDER}&pad={padding}"
        )),
        0,
    )
    .await;
    assert!(
        flow.outcome.is_err(),
        "a query over the validator cap must be refused"
    );
    assert!(
        !flow.error().to_string().contains("BBBB"),
        "the refusal must echo none of the query: {}",
        flow.error()
    );
    flow.assert_refused_with_failure_page().await;
}

#[tokio::test]
async fn a_request_line_with_no_request_target_does_not_hang_the_flow() {
    let flow = drive_flow(
        None,
        CallbackRequest::RawLine("THIS-IS-NOT-AN-HTTP-REQUEST-LINE".to_string()),
        0,
    )
    .await;
    assert!(
        flow.outcome.is_err(),
        "an unparseable callback must be refused"
    );
    assert!(
        flow.elapsed < Duration::from_secs(60),
        "an unparseable callback must not push the flow to its 5-minute timeout"
    );
    flow.assert_refused_with_failure_page().await;
}

// ---------------------------------------------------------------------------
// Group 5 — the served pages are the SAME bytes this module has always served
// ---------------------------------------------------------------------------

#[tokio::test]
async fn both_pages_are_byte_identical_to_the_pages_this_module_has_always_served() {
    // The exact bytes, copied from the pre-116-09 implementation. Only the two
    // status lines and these literals are ever written; neither page gains any
    // authorization-server-supplied text.
    const SUCCESS: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
         <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
         <h1 style='color: green;'>Authentication Successful!</h1>\
         <p>You can close this window and return to the terminal.</p>\
         </body></html>";
    const FAILURE: &str = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
         <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
         <h1 style='color: red;'>Authentication Failed</h1>\
         <p>No authorization code received. Please try again.</p>\
         </body></html>";

    let refused = drive_flow(
        None,
        CallbackRequest::Query("code=x&state=wrong".to_string()),
        0,
    )
    .await;
    assert_eq!(
        refused.served, FAILURE,
        "the failure page must be byte-identical to the page this module has always served"
    );
    refused.token_mock.assert_async().await;

    let accepted = drive_flow(
        None,
        CallbackRequest::Query(format!("code=x&state={STATE_PLACEHOLDER}")),
        1,
    )
    .await;
    assert_eq!(
        accepted.served, SUCCESS,
        "the success page must be byte-identical to the page this module has always served"
    );
    accepted.token_mock.assert_async().await;
}

/// Minimal percent-encoding for the `iss` value, which is a full URL.
fn urlencoding(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
