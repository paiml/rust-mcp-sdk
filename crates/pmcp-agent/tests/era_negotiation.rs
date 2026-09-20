#![cfg(all(feature = "url-connector", not(target_arch = "wasm32")))]
//! CLNT-03 supporting evidence: the era classification in
//! `UrlConnectorClientFactory::client_for` is NOT vacuous.
//!
//! # Why this file is separate from `agent_v2_e2e.rs`
//!
//! `agent_v2_e2e.rs` is plan 117-04's executable contract, written RED before
//! the implementation existed. It is left byte-for-byte as written. This file
//! carries the extra evidence plan 117-07 owes for its own design choice — the
//! host-layer reachability probe — against a server the pmcp-specific
//! `Error::Protocol` branch does NOT cover.
//!
//! # The case under test
//!
//! `crates/pmcp-agent/src/invoker/factory.rs` documents the measurement that
//! forces a host-layer probe: a connect failure and a non-2xx HTTP status BOTH
//! arrive as `Error::Transport(TransportError::Request(String))`, so neither the
//! error variant nor its prose can classify them. Only a pmcp server hitting the
//! v2 JSON-RPC-envelope branch surfaces a distinguishable `Error::Protocol` —
//! which covers pmcp's OWN servers and nothing else.
//!
//! A third-party endpoint that accepts TCP and answers a plain `404` with a
//! non-JSON body is exactly the uncovered case. It must classify as ANSWERED
//! and therefore trigger the v1 attempt. If the classifier were vacuous — if it
//! treated any v2 failure as unreachable — the stub would observe exactly one
//! request and this test would fail.
//!
//! Nothing here inspects the TEXT of an error to classify it: the stub reads the
//! JSON-RPC `method` field of the requests it received and asserts on THOSE.

#[path = "common/v2_server.rs"]
mod v2_server;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

use pmcp::types::protocol::{protocol_era, Era, PROTOCOL_VERSION_2026_07_28};
use pmcp_agent::invoker::{ConnectorClientFactory, InvokerError, UrlConnectorClientFactory};
use pmcp_agent::EffectTrace;

use v2_server::{spawn_v1_only, spawn_v2, teardown, BOUNDED_WAIT};

/// The JSON-RPC method a v2 era probe sends.
const V2_PROBE_METHOD: &str = "server/discover";

/// The JSON-RPC method that exists ONLY on v1.
const V1_HANDSHAKE_METHOD: &str = "initialize";

/// A raw-socket endpoint that ANSWERS every request with a plain `404` and a
/// non-JSON body, recording the JSON-RPC method of each request it received.
///
/// Deliberately NOT an MCP server and not even a JSON endpoint: this is the
/// "third-party server that is simply not an MCP endpoint" case.
///
/// It separates two observations that a naive request log would conflate:
/// a BARE accepted connection carrying no request (the host-layer reachability
/// probe, which connects and drops) from a real JSON-RPC request.
struct FourOhFourStub {
    endpoint: String,
    methods: Arc<StdMutex<Vec<String>>>,
    bare_connections: Arc<AtomicUsize>,
    handle: tokio::task::JoinHandle<()>,
}

impl FourOhFourStub {
    /// The JSON-RPC methods observed, in arrival order.
    ///
    /// TOTAL: a poisoned lock yields the recovered contents rather than
    /// panicking, so a fixture fault never aborts the test process.
    fn observed(&self) -> Vec<String> {
        self.methods
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// How many connections were accepted that carried NO request at all —
    /// i.e. the host-layer reachability probe's footprint.
    ///
    /// An atomic rather than a mutex: a poisoned counter lock would have made
    /// the recording side silently DROP an increment, which weakens the
    /// `>= 1` assertion this exists to support.
    fn bare_connections(&self) -> usize {
        self.bare_connections.load(Ordering::Relaxed)
    }
}

/// Spawn the stub on an ephemeral loopback port.
async fn spawn_404_stub() -> FourOhFourStub {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind an ephemeral loopback port");
    let addr = listener
        .local_addr()
        .expect("the bound address is readable");
    let methods: Arc<StdMutex<Vec<String>>> = Arc::new(StdMutex::new(Vec::new()));
    let bare_connections: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    let recorder = Arc::clone(&methods);
    let bare_recorder = Arc::clone(&bare_connections);

    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _peer)) = listener.accept().await else {
                return;
            };
            let body = read_request_body(&mut stream).await;
            if body.is_empty() {
                // A connection that carried no request: the reachability probe.
                bare_recorder.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let method = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|value| {
                    value
                        .get("method")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "<unparsed>".to_string());
            if let Ok(mut observed) = recorder.lock() {
                observed.push(method);
            }
            // A plain 404 with a NON-JSON body: the server ANSWERED, and its
            // answer carries no MCP or JSON-RPC signal whatsoever.
            const BODY: &str = "not an mcp endpoint";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{BODY}",
                BODY.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    });

    FourOhFourStub {
        endpoint: format!("http://{addr}/"),
        methods,
        bare_connections,
        handle,
    }
}

/// Read one HTTP/1.1 request off `stream` and return its body.
async fn read_request_body(stream: &mut tokio::net::TcpStream) -> String {
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 1024];
    while let Ok(read) = stream.read(&mut chunk).await {
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&raw).into_owned();
        let Some(header_end) = text.find("\r\n\r\n") else {
            continue;
        };
        let content_length = text
            .lines()
            .take_while(|line| !line.is_empty())
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        if text.len() - (header_end + 4) >= content_length {
            return text[header_end + 4..].to_string();
        }
    }
    String::from_utf8_lossy(&raw).into_owned()
}

/// The uncovered third-party case: TCP accepted, plain `404`, non-JSON body.
///
/// It must classify as ANSWERED — so the v1 attempt runs — and the whole call
/// must still FAIL (era V1 is reported only when a real `initialize` SUCCEEDS).
#[tokio::test]
async fn a_third_party_404_endpoint_is_classified_answered_and_triggers_the_v1_attempt() {
    let stub = spawn_404_stub().await;
    let factory = UrlConnectorClientFactory::new();

    let outcome = timeout(BOUNDED_WAIT, factory.client_for(&stub.endpoint))
        .await
        .expect("client_for against a 404 endpoint must FAIL fast, not hang");

    let observed = stub.observed();

    // The host-layer probe ran: it accepted a connection that carried no
    // request at all. That connection IS the typed reachability fact.
    assert!(
        stub.bare_connections() >= 1,
        "the host-layer reachability probe must connect before attempt 1; the stub saw \
         {} bare connections",
        stub.bare_connections()
    );
    // The classification is not vacuous: the v1 attempt was REACHED, which can
    // only happen through the `Answered` arm. An `Unreachable` classification
    // would have produced `server/discover` alone.
    assert_eq!(
        observed,
        vec![V2_PROBE_METHOD.to_string(), V1_HANDSHAKE_METHOD.to_string()],
        "an endpoint that ACCEPTS TCP and answers `404` must be classified as having ANSWERED, so \
         the v2 era is attempted FIRST and the v1 fallback attempt then runs. Stub observed: \
         {observed:?}"
    );

    // The STRUCTURAL invariant: era V1 is reported only on a real `initialize`
    // success, so a 404-answering endpoint yields an error and no connector.
    let err = outcome.err().unwrap_or(InvokerError::Config(
        "no error was produced at all".to_string(),
    ));
    assert!(
        matches!(err, InvokerError::Transport(_)),
        "a `404`-answering endpoint fails at the TRANSPORT layer, classified by REACHABILITY and \
         never by the text of the message; got {err:?}"
    );

    stub.handle.abort();
}

// ===========================================================================
// The RECORDING end of the era plumbing.
//
// A field nobody populates closes no hole. These two cases drive a LIVE
// connector through `client_for` and record its negotiated version into an
// `EffectTrace`, so the accessor -> builder -> replay-guard chain is proven end
// to end rather than only at its endpoints.
// ===========================================================================

/// Against a live v2 server, the connector reports `2026-07-28` and that is what
/// lands in the recorded trace.
#[tokio::test]
async fn a_live_v2_connector_populates_the_recorded_negotiated_version() {
    let live = spawn_v2().await;
    let factory = UrlConnectorClientFactory::new();

    let connector = timeout(BOUNDED_WAIT, factory.client_for(&live.endpoint()))
        .await
        .expect("client_for completed within BOUNDED_WAIT")
        .expect("a connector is established against a v2-accepting server");

    let recorded = connector
        .negotiated_protocol_version()
        .map(str::to_string)
        .unwrap_or_default();
    let trace = EffectTrace::new(vec![], vec![]).with_negotiated_version(recorded);

    assert!(
        trace.negotiated_version.is_some(),
        "a trace recorded through the LIVE path must carry the negotiated version; the recording \
         end of D-08's plumbing is what makes the replay guard reachable"
    );
    assert_eq!(
        trace.negotiated_version.as_deref(),
        Some(PROTOCOL_VERSION_2026_07_28),
        "a v2 connection records the 2026-07-28 wire string verbatim"
    );
    assert_eq!(
        trace.negotiated_version.as_deref().map(protocol_era),
        Some(Era::V2)
    );

    teardown(live.handle, connector).await;
}

/// Against a live v1-only server, the connector reports the version the server
/// ECHOED in its `initialize` result — not a hardcoded guess — and it classifies
/// as V1.
#[tokio::test]
async fn a_live_v1_fallback_records_the_server_echoed_version() {
    let live = spawn_v1_only().await;
    let factory = UrlConnectorClientFactory::new();

    let connector = timeout(BOUNDED_WAIT, factory.client_for(&live.endpoint()))
        .await
        .expect("client_for completed within BOUNDED_WAIT")
        .expect("a connector is established over the v1 fallback");

    let recorded = connector
        .negotiated_protocol_version()
        .map(str::to_string)
        .unwrap_or_default();
    let trace = EffectTrace::new(vec![], vec![]).with_negotiated_version(recorded);

    assert!(
        trace.negotiated_version.is_some(),
        "the v1 fallback must record a version too, or a v1 trace replays as era-less"
    );
    assert_ne!(
        trace.negotiated_version.as_deref(),
        Some(PROTOCOL_VERSION_2026_07_28),
        "the v1 fallback must NOT record the v2 wire string"
    );
    assert_eq!(
        trace.negotiated_version.as_deref().map(protocol_era),
        Some(Era::V1),
        "the v1 fallback's recorded version classifies as V1 via protocol_era"
    );

    teardown(live.handle, connector).await;
}
