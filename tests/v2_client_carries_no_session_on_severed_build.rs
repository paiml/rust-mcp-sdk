//! SMPL-02 at RUNTIME, on the severed build: the `full-v2` client stores no
//! session id and sends no DELETE.
//!
//! # Why a runtime test at all
//!
//! `tests/v1_severability_tripwire.rs` proves the client transport's v1 surface
//! is GATED, and `cargo build --no-default-features --features full-v2` proves
//! the gated crate compiles. Neither proves what the compiled client actually
//! DOES. A transport that kept a second, ungated session field, or that emitted
//! a DELETE from some other path, would satisfy both and still falsify
//! SMPL-02's "the v2 code path carries no session/SSE-resumability baggage".
//!
//! So this file runs the severed client against a real socket and asserts the
//! behaviour from the SERVER side.
//!
//! # Why every absence assertion is made from the SERVER
//!
//! A client-side assertion cannot prove absence: on this build the accessor that
//! would report a stored session id does not exist, and "the method is missing"
//! is a compile fact the tripwire already covers. What matters is the WIRE. The
//! stub server here records every request's method and headers, so "no
//! `Mcp-Session-Id` was echoed" and "no DELETE was sent" are observations of
//! bytes that did or did not arrive.
//!
//! # The `cfg` predicate is NOT a negative cargo feature
//!
//! `not(feature = "v1-compat")` below is a `cfg` predicate inside THIS crate's
//! own test compilation. It selects whether this test target's source compiles
//! at all; it does not ask cargo to subtract a feature from anyone's dependency
//! graph. D-02 rejected an inverted `v2-only` FEATURE precisely because cargo
//! features are additive and unification would strip v1 for unrelated
//! consumers — a `cfg` in a test file has no such reach.
//!
//! # Running it
//!
//! ```text
//! cargo test --test v2_client_carries_no_session_on_severed_build \
//!     --no-default-features --features full-v2
//! ```
//!
//! A run that reports `0 tests` is a FAILURE, not a pass: it means the whole
//! file was compiled out, which is exactly what happens under the default
//! feature set.
//!
//! # Where that criterion is ENFORCED — and where it cannot be
//!
//! In `scripts/run-severance-proofs.sh`, which CI runs from the `v1-severance`
//! job: it greps the harness output for `running N tests` with N >= 1 and fails
//! the build otherwise.
//!
//! It is deliberately NOT enforced here. This file used to carry a test whose
//! whole body was `assert!(!cfg!(feature = "v1-compat"))` — asserted from INSIDE
//! a file whose own `#![cfg]` already guarantees it. `cfg!` expands to a bool
//! literal, so the assertion was `!false`: it could not fail on any input, and on
//! the build where it would be false the test did not exist to run. A test inside
//! a conditionally-compiled file can never police whether that file was compiled;
//! the observer has to be outside the compilation unit. It was deleted in Phase
//! 117's fix pass and replaced by the script's guard, whose failure path is
//! executed as its own negative control.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(feature = "v1-compat"),
    not(target_arch = "wasm32")
))]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::{ClientRequest, ListToolsRequest, Request, RequestId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use url::Url;

/// Ceiling on every await in this file.
///
/// A hung server must FAIL the test rather than hang it: an integration test
/// with no timeout turns a transport regression into a CI job that runs until
/// the runner's own wall clock kills it, with no failure message to read.
const STEP_TIMEOUT: Duration = Duration::from_secs(10);

/// The session id the stub server PLANTS on every response.
///
/// The severed client must ignore it. A v1 client would store it and echo it on
/// the next request, which is exactly what the assertions below look for.
const PLANTED_SESSION: &str = "planted-session-id-that-must-not-be-echoed";

/// One request the stub server saw.
#[derive(Clone, Debug)]
struct SeenRequest {
    method: String,
    headers: HashMap<String, String>,
}

/// Everything the stub server observed, shared with the test body.
type Observed = Arc<Mutex<Vec<SeenRequest>>>;

/// A stub HTTP server: minimal on purpose.
///
/// Hand-rolled over `TcpListener` rather than built on `pmcp`'s own server so
/// the observation is independent of the code under test, and so per-method
/// request counting needs no hook into a framework. It answers every POST with a
/// JSON-RPC result AND a `Mcp-Session-Id` header, and answers anything else
/// `200 OK` so a stray DELETE would succeed rather than error — the test must
/// fail because a DELETE was SENT, not because it was refused.
struct StubServer {
    addr: SocketAddr,
    observed: Observed,
    handle: tokio::task::JoinHandle<()>,
}

impl StubServer {
    async fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        let observed: Observed = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&observed);

        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let sink = Arc::clone(&sink);
                tokio::spawn(async move {
                    let mut buffer = Vec::new();
                    let mut chunk = [0u8; 4096];
                    // Read until the headers are complete, then read exactly the
                    // advertised body. A stub, not a parser: every request this
                    // test provokes is small and well-formed.
                    loop {
                        let Ok(read) = stream.read(&mut chunk).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        buffer.extend_from_slice(&chunk[..read]);
                        let Some(head_end) = find_head_end(&buffer) else {
                            continue;
                        };
                        let head = String::from_utf8_lossy(&buffer[..head_end]).to_string();
                        let (method, headers) = parse_head(&head);
                        let want = headers
                            .get("content-length")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(0);
                        if buffer.len() < head_end + 4 + want {
                            continue;
                        }
                        sink.lock().await.push(SeenRequest {
                            method: method.clone(),
                            headers,
                        });
                        let body = br#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\n\
                             Content-Type: application/json\r\n\
                             Mcp-Session-Id: {PLANTED_SESSION}\r\n\
                             Content-Length: {}\r\n\
                             Connection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.write_all(body).await;
                        let _ = stream.flush().await;
                        return;
                    }
                });
            }
        });

        Self {
            addr,
            observed,
            handle,
        }
    }

    fn url(&self) -> Url {
        Url::parse(&format!("http://{}/", self.addr)).expect("stub url parses")
    }

    async fn seen(&self) -> Vec<SeenRequest> {
        self.observed.lock().await.clone()
    }
}

impl Drop for StubServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Index just past the request head, i.e. the offset of the `\r\n\r\n`.
fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// The request method plus a lowercase-keyed header map.
fn parse_head(head: &str) -> (String, HashMap<String, String>) {
    let mut lines = head.split("\r\n");
    let method = lines
        .next()
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_default()
        .to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    (method, headers)
}

/// A `tools/list` request, the smallest exchange that proves the client works.
fn tools_list(id: i64) -> TransportMessage {
    TransportMessage::Request {
        id: RequestId::Number(id),
        request: Request::Client(Box::new(ClientRequest::ListTools(
            ListToolsRequest::default(),
        ))),
    }
}

/// Await `future`, failing the test rather than hanging when it does not settle.
async fn within<F: std::future::Future>(label: &str, future: F) -> F::Output {
    match tokio::time::timeout(STEP_TIMEOUT, future).await {
        Ok(value) => value,
        Err(_) => panic!(
            "FAILURE MODE: `{label}` did not settle within {STEP_TIMEOUT:?}.\n\
             CONSEQUENCE: without this bound the test would hang the whole CI job instead of \
             reporting a transport regression.\n\
             WHAT TO DO: investigate the transport step named above — do not raise the timeout to \
             make it pass."
        ),
    }
}

/// The severed client completes a POST exchange, stores no session id, and
/// sends no DELETE on close.
///
/// The three assertions are ordered deliberately. The exchange must SUCCEED
/// first: a transport that failed to connect would trivially send no
/// `Mcp-Session-Id` and no DELETE, so a dead client could otherwise masquerade
/// as a severed one.
#[tokio::test]
async fn the_severed_client_stores_no_session_and_sends_no_delete() {
    let server = StubServer::start().await;
    let config = StreamableHttpTransportConfigBuilder::new(server.url())
        .enable_json_response()
        .build();
    let mut transport = StreamableHttpTransport::new(config);

    // 1. The exchange SUCCEEDS.
    within("first POST", transport.send(tools_list(1)))
        .await
        .expect("the severed client must complete a POST exchange");
    let reply = within("first receive", transport.receive())
        .await
        .expect("the severed client must deliver the server's reply");
    assert!(
        matches!(reply, TransportMessage::Response(_)),
        "FAILURE MODE: the severed client did not deliver a JSON-RPC response; got {reply:?}.\n\
         CONSEQUENCE: every absence assertion below would pass vacuously, because a client that \
         cannot talk sends no session header and no DELETE either.\n\
         WHAT TO DO: fix the transport, not the assertions."
    );

    // 2. A SECOND request carries NO `Mcp-Session-Id`, even though the server
    //    planted one on the first response.
    within("second POST", transport.send(tools_list(2)))
        .await
        .expect("the severed client must complete a second POST exchange");
    let _ = within("second receive", transport.receive()).await;

    // 3. Closing sends NO DELETE.
    within("close", transport.close())
        .await
        .expect("close must succeed");

    let seen = server.seen().await;
    assert!(
        seen.len() >= 2,
        "FAILURE MODE: the stub server observed {} request(s), expected at least 2.\n\
         CONSEQUENCE: the per-method and per-header assertions below would be checking an empty \
         or partial record.\n\
         WHAT TO DO: fix the stub server or the transport — never relax this floor.\n\
         Observed: {seen:?}",
        seen.len()
    );

    let echoed: Vec<&SeenRequest> = seen
        .iter()
        .filter(|request| request.headers.contains_key(MCP_SESSION_ID))
        .collect();
    let echoed_count = echoed.len();
    assert!(
        echoed.is_empty(),
        "FAILURE MODE: the severed client sent `Mcp-Session-Id` on {echoed_count} request(s) after the server \
         planted `{PLANTED_SESSION}`.\n\
         CONSEQUENCE: a `full-v2` build is still storing and echoing a server-controlled session \
         identity (T-117-52), so SMPL-02's 'no session baggage' is false at runtime whatever the \
         source scan says.\n\
         WHAT TO DO: gate the capture site in `StreamableHttpTransport::capture_session_header`. \
         Do NOT weaken this assertion.\n\
         Offending requests: {echoed:?}"
    );

    let deletes: Vec<&SeenRequest> = seen
        .iter()
        .filter(|request| request.method.eq_ignore_ascii_case("DELETE"))
        .collect();
    assert!(
        deletes.is_empty(),
        "FAILURE MODE: closing the severed transport sent {} DELETE request(s).\n\
         CONSEQUENCE: the `full-v2` client is emitting a teardown for a session that never \
         existed (T-117-55). The severed build is supposed to have no DELETE construction site at \
         all, not a runtime branch that happens to be false.\n\
         WHAT TO DO: keep the whole DELETE construction inside \
         `StreamableHttpTransport::terminate_session`'s `v1-compat` half.\n\
         Observed methods: {:?}",
        deletes.len(),
        seen.iter().map(|r| r.method.as_str()).collect::<Vec<_>>()
    );
}
