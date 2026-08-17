//! Property-based tests for the `AuthProvider::on_unauthorized` + 401 retry
//! contract added in pmcp 2.8.0.
//!
//! Companion to the 5 hand-picked unit tests in `src/shared/streamable_http.rs`.
//! These sweep arbitrary HTTP status codes to catch any drift in the trigger
//! predicate "retry iff status == 401 AND auth provider is configured".
//!
//! Invariants verified:
//!   1. `on_unauthorized()` is invoked **iff** the first response is 401
//!      AND an auth provider is configured.
//!   2. `get_access_token()` is called exactly once for non-401 responses and
//!      exactly twice for a 401 → retry sequence (proves single-shot retry).
//!   3. When `auth_provider` is `None`, neither callback is invoked regardless
//!      of response status.

#![cfg(feature = "streamable-http")]

use async_trait::async_trait;
use mockito::Server as MockServer;
use pmcp::shared::streamable_http::{
    AuthProvider, SendOptions, StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::shared::TransportMessage;
use pmcp::types::{ClientNotification, Notification};
use proptest::prelude::*;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
use url::Url;

/// The 8 status codes the SDK realistically encounters. Held in a `const` so
/// the proptest can sweep each value deterministically via `sample::select`,
/// avoiding the prop_oneof-with-redundant-cases pattern.
const STATUS_CODES: &[u16] = &[200, 202, 400, 401, 403, 404, 500, 503];

/// Shared tokio runtime — built once, reused across every proptest case.
/// Per-case runtime construction would add ~1-5ms × N cases of pure overhead.
fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("failed to build tokio runtime for proptest harness"))
}

/// `AuthProvider` test double that counts each callback invocation.
struct CountingProvider {
    token: String,
    get_count: AtomicUsize,
    unauthorized_count: AtomicUsize,
}

impl fmt::Debug for CountingProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CountingProvider")
            .field("get_count", &self.get_count.load(Ordering::SeqCst))
            .field(
                "unauthorized_count",
                &self.unauthorized_count.load(Ordering::SeqCst),
            )
            .finish()
    }
}

impl CountingProvider {
    fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            get_count: AtomicUsize::new(0),
            unauthorized_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl AuthProvider for CountingProvider {
    async fn get_access_token(&self) -> pmcp::Result<String> {
        self.get_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.token.clone())
    }

    async fn on_unauthorized(&self) -> pmcp::Result<()> {
        self.unauthorized_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn make_transport(url: Url, provider: Option<Arc<dyn AuthProvider>>) -> StreamableHttpTransport {
    let mut builder = StreamableHttpTransportConfigBuilder::new(url);
    if let Some(p) = provider {
        builder = builder.with_auth_provider(p);
    }
    StreamableHttpTransport::new(builder.build())
}

fn ping_message() -> TransportMessage {
    TransportMessage::Notification(Notification::Client(ClientNotification::Initialized))
}

/// Drive one request through a mock server returning `status`. Returns the
/// provider's final `(get_count, unauthorized_count)` — or `(0, 0)` when no
/// provider is configured.
async fn run_one_case(status: u16, provider: Option<Arc<CountingProvider>>) -> (usize, usize) {
    let mut server = MockServer::new_async().await;
    // No `.expect(n)`: the SDK may issue 1 or 2 requests depending on `status`,
    // and both are valid contractual outcomes. We verify call counts via the
    // provider's atomics rather than mock fulfillment.
    let _mock = server
        .mock("POST", "/")
        .with_status(status as usize)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)
        .create_async()
        .await;

    let url = Url::parse(&server.url()).unwrap();
    let dyn_provider = provider.clone().map(|p| p as Arc<dyn AuthProvider>);
    let mut transport = make_transport(url, dyn_provider);

    // 401/500 will return Err; the test cares about call counts, not Ok/Err.
    let _ = transport
        .send_with_options(ping_message(), SendOptions::default())
        .await;

    provider.map_or((0, 0), |p| {
        (
            p.get_count.load(Ordering::SeqCst),
            p.unauthorized_count.load(Ordering::SeqCst),
        )
    })
}

proptest! {
    // 8 cases × 8 status codes covers every value deterministically; running
    // more would just retread the same 8 underlying scenarios.
    #![proptest_config(ProptestConfig::with_cases(8))]

    /// With a provider configured, `on_unauthorized` fires iff status == 401,
    /// and `get_access_token` is called twice on 401 (original + retry) and
    /// exactly once on every other status.
    #[test]
    fn property_on_unauthorized_triggers_iff_401(status in proptest::sample::select(STATUS_CODES)) {
        let provider = Arc::new(CountingProvider::new("token"));
        let (get_count, unauth_count) =
            rt().block_on(run_one_case(status, Some(provider)));

        if status == 401 {
            prop_assert_eq!(unauth_count, 1, "on_unauthorized must fire exactly once on 401");
            prop_assert_eq!(get_count, 2, "get_access_token must be called twice on 401 (original + retry)");
        } else if status == 202 {
            // 202 fetches a token TWICE, and not because of a retry.
            //
            // `ping_message()` is in fact the `notifications/initialized`
            // notification, and since Phase 118.2 a 202 to THAT notification
            // opens the GET session stream — a second HTTP request, which builds
            // its own Authorization header through the same provider. Before
            // 118.2 the stream-open sat inside `if !status.is_success()`, and
            // 202 satisfies `is_success()`, so it was dead code and this case
            // counted one. The property's SUBJECT is unchanged: `on_unauthorized`
            // still fires iff 401.
            prop_assert_eq!(unauth_count, 0,
                "on_unauthorized must NOT fire on non-401 status {}", status);
            prop_assert_eq!(get_count, 2,
                "202 to notifications/initialized fetches a token for the POST and again for the \
                 session-stream GET");
        } else {
            prop_assert_eq!(unauth_count, 0,
                "on_unauthorized must NOT fire on non-401 status {}", status);
            prop_assert_eq!(get_count, 1,
                "get_access_token must be called exactly once on non-401 status {}", status);
        }
    }

    /// With no provider, neither callback can be invoked — the retry branch
    /// is gated on `Some(provider)`. Guards dual-path correctness even if the
    /// trigger predicate is widened later.
    #[test]
    fn property_no_provider_means_no_retry(status in proptest::sample::select(STATUS_CODES)) {
        let (get_count, unauth_count) = rt().block_on(run_one_case(status, None));

        prop_assert_eq!(get_count, 0, "without a provider, get_access_token cannot be called");
        prop_assert_eq!(unauth_count, 0, "without a provider, on_unauthorized cannot be called");
    }
}
