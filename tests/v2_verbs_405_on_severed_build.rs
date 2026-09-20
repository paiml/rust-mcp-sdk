#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(feature = "v1-compat"),
    not(target_arch = "wasm32")
))]
//! `GET /` and `DELETE /` answer `405` on a build with NO v1 — proven by RUNNING.
//!
//! # The defect this file closes
//!
//! Phase 117's `full-v2` severance was, until this file, proven entirely by
//! COMPILATION: `cargo build -p pmcp --no-default-features --features full-v2`
//! exits 0, so the v1 code is not in the binary. That is a strong claim about
//! what EXISTS. It is no claim at all about what the server ANSWERS.
//!
//! The gap is easy to paper over and worth naming. `tests/v2_required_headers.rs`
//! already asserts a `405` on GET and DELETE — but it runs under `--features full`,
//! where `v1-compat` is on, so what it exercises is
//! `v2_verb_rejection` short-circuiting the REAL v1 handler. It says nothing about
//! the build where that handler does not exist. A runtime claim needs a runtime
//! execution ON THE BUILD BEING CLAIMED ABOUT, which is what this file is.
//!
//! # Why the file-level `cfg` is `not(feature = "v1-compat")` and not a feature
//!
//! Phase 117 decision D-02 rejected a `v2-only` cargo feature because cargo
//! features must be ADDITIVE: enabling one may never remove behaviour, and a
//! feature whose job is to subtract violates that for every consumer who unions
//! feature sets across a dependency graph.
//!
//! `not(feature = "v1-compat")` here is a different thing entirely. It is a `cfg`
//! PREDICATE inside pmcp's own test compilation — it selects whether THIS test
//! binary contains any tests, and no dependency graph can observe it. Do not
//! "fix" this into a cargo feature, and do not add a `v2-only` feature to make it
//! read more symmetrically.
//!
//! The corollary is the failure mode to watch for: on a build that DOES carry
//! `v1-compat`, this file compiles to ZERO tests and `cargo test` still exits 0.
//! A run reporting `0 tests` is therefore a FAILURE of this file's purpose, not a
//! pass — the number of tests is part of the evidence. Plan 117-14 found exactly
//! that: a dev-dependency taking `pmcp`'s default features unified `v1-compat`
//! back on for every `cargo test`, so a severed-build test reported `0 tests`,
//! exit 0, while proving nothing. `cargo build -p pmcp` never sees dev-deps;
//! `cargo test` does.
//!
//! That criterion is ENFORCED by `scripts/run-severance-proofs.sh`, which the
//! `v1-severance` CI job runs: it greps the harness output for `running N tests`
//! with N >= 1 and fails the build otherwise. It cannot be enforced from inside
//! this file — a test in a `#![cfg]`-selected file can never observe its own
//! absence, which is why the earlier `assert!(!cfg!(feature = "v1-compat"))`
//! attempt in the sibling proof was a tautology and was deleted.
//!
//! # Run it with
//!
//! ```text
//! cargo test --test v2_verbs_405_on_severed_build --no-default-features --features full-v2
//! ```

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::v2::{
    build_v2_server, delete, get, header, post, spawn_with, teardown, v2_body, v2_headers, Resp,
    ALLOW, V1, V2,
};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::shared::http_constants::{MCP_PROTOCOL_VERSION, MCP_SESSION_ID};
use serde_json::json;
use tokio::task::JoinHandle;

/// Upper bound on any single request in this file.
///
/// A hung server must FAIL the test, not hang it. Without this, a severance
/// regression that deadlocks a verb (rather than answering it) would present as a
/// suite that never finishes, which reads as infrastructure flakiness rather than
/// as the defect it is. Generous enough that a loaded CI machine never trips it.
const VERB_TIMEOUT: Duration = Duration::from_secs(10);

/// The `405` every assertion in this file expects.
const METHOD_NOT_ALLOWED: u16 = 405;

/// The `404` every assertion in this file explicitly REJECTS.
///
/// A `404` here would not be a smaller version of the same answer; it would mean
/// `build_mcp_router` stopped routing the verb. See [`assert_refused_not_unrouted`].
const NOT_FOUND: u16 = 404;

/// Spawn the shared v2 fixture without naming a `v1-compat`-gated config field.
///
/// `..Default::default()` rather than the harness's own default-config spawn
/// helper: that helper builds `StreamableHttpServerConfig::default()` for itself,
/// but this file must be able to state its config positively, and the only fields
/// it may NAME are the ones present on both feature sets. `enable_json_response` is one of those SHARED
/// fields — deliberately exercised here so the file also demonstrates that a
/// `full-v2` consumer can still configure the transport.
async fn spawn() -> (SocketAddr, JoinHandle<()>) {
    let config = StreamableHttpServerConfig {
        enable_json_response: true,
        ..Default::default()
    };
    spawn_with(build_v2_server(), config).await
}

/// Await `future` under [`VERB_TIMEOUT`], failing the test if it does not settle.
async fn within<F: std::future::Future<Output = Resp>>(what: &str, future: F) -> Resp {
    tokio::time::timeout(VERB_TIMEOUT, future)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "FAILURE MODE: {what} did not answer within {VERB_TIMEOUT:?}.\n\
                 CONSEQUENCE: a verb that HANGS is not a verb that answers 405, and a suite that \
                 hangs reads as infrastructure noise rather than as the severance regression it \
                 is.\n\
                 WHAT TO DO: check that the `full-v2` half of the verb returns the shared 405 \
                 rather than opening a stream."
            )
        })
}

/// The core assertion: the verb is REFUSED (`405`), not UNROUTED (`404`).
///
/// Both directions are asserted separately and on purpose. `405` alone would also
/// be satisfied by a lucky coincidence in a later refactor; the explicit
/// not-`404` check names the specific regression that Task 1 of plan 117-13 was
/// written to prevent — deleting the `get`/`delete` routes from
/// `build_mcp_router` on the severed build. That would look like "v1 is gone" and
/// would in fact change the v2 wire answer, because `404` says "no such endpoint"
/// while `405` says "this endpoint does not take this verb".
fn assert_refused_not_unrouted(response: &Resp, verb: &str) {
    assert_ne!(
        response.status,
        NOT_FOUND,
        "FAILURE MODE: {verb} / answered {NOT_FOUND}, not {METHOD_NOT_ALLOWED}.\n\
         CONSEQUENCE: a {NOT_FOUND} means `build_mcp_router` stopped ROUTING {verb} on the \
         `full-v2` build. That is a DIFFERENT wire answer — \"no such endpoint\" rather than \
         \"this endpoint does not take this verb\" — and it regresses plan 117-13 Task 1's \
         \"the router is unchanged\" requirement.\n\
         WHAT TO DO: restore the `.route(\"/\", {})` line in `build_mcp_router`; the verb must \
         stay routed on BOTH feature sets and be refused by its handler.",
        verb.to_lowercase()
    );
    assert_eq!(
        response.allow.as_deref(),
        Some(ALLOW),
        "FAILURE MODE: the {verb} / refusal carried `Allow: {:?}`, not `{ALLOW}`.\n\
         CONSEQUENCE: RFC 9110 §15.5.6 is a MUST — \"the origin server MUST generate an `Allow` \
         header field in a 405 response containing a list of the target resource's currently \
         supported methods\". Without it the refusal tells an intermediary or a generic HTTP \
         client only that it was wrong, never what to do instead.\n\
         WHAT TO DO: `method_not_allowed_for_verb` in `src/server/streamable_http_server.rs` is \
         THE single 405 constructor for both the v2 rejection head and the severed-build twin \
         bodies. Fix it there, once.",
        response.allow
    );
    assert_eq!(
        response.status, METHOD_NOT_ALLOWED,
        "FAILURE MODE: {verb} / answered {}, not {METHOD_NOT_ALLOWED}.\n\
         CONSEQUENCE: the 2026-07-28 transport spec is verbatim that a GET or DELETE to the MCP \
         endpoint is answered `405 Method Not Allowed`. On a build with no v1 at all there is no \
         other answer available.\n\
         BODY: {}\n\
         WHAT TO DO: check the `full-v2` half of the verb in \
         `src/server/streamable_http_server/v1_session_off.rs`.",
        response.status, response.raw
    );
}

/// A bare `GET /` on the severed build is `405`, and specifically not `404`.
///
/// "Bare" is load-bearing: no `MCP-Protocol-Version` header, so
/// `v2_verb_rejection` declines to fire and the answer comes from the twin body
/// rather than from the era guard. On a `--features full` build this exact
/// request reaches the real v1 SSE handler and opens a stream.
#[tokio::test]
async fn a_bare_get_on_the_severed_build_is_405_and_not_404() {
    let (addr, handle) = spawn().await;

    let response = within("a bare GET /", get(addr, &[])).await;
    assert_refused_not_unrouted(&response, "GET");

    teardown(handle, ()).await;
}

/// A bare `DELETE /` on the severed build is `405`, and specifically not `404`.
///
/// On a `--features full` build this request reaches the real v1 teardown handler
/// and is answered `404 No session ID provided` — a `404` that means something
/// entirely different from an unrouted verb, which is why this file asserts the
/// status rather than any body text.
#[tokio::test]
async fn a_bare_delete_on_the_severed_build_is_405_and_not_404() {
    let (addr, handle) = spawn().await;

    let response = within("a bare DELETE /", delete(addr, &[])).await;
    assert_refused_not_unrouted(&response, "DELETE");

    teardown(handle, ()).await;
}

/// A `GET` carrying v1 credentials is still `405`: there is no v1 body left.
///
/// This is the assertion that can ONLY pass on the severed build. The request
/// declares `MCP-Protocol-Version: 2025-11-25` and presents an `Mcp-Session-Id`,
/// so on a `v1-compat` build it is a well-formed v1 SSE resume attempt against a
/// server that accepts v1. Here it is refused, because the code that would have
/// served it is not compiled.
#[tokio::test]
async fn a_v1_flavoured_get_is_405_because_no_v1_body_is_compiled() {
    let (addr, handle) = spawn().await;

    let response = within(
        "a v1-flavoured GET /",
        get(
            addr,
            &[
                header(MCP_PROTOCOL_VERSION, V1),
                header(MCP_SESSION_ID, "a-session-that-never-existed"),
            ],
        ),
    )
    .await;
    assert_refused_not_unrouted(&response, "GET");
    assert!(
        response.mcp_session_id.is_none(),
        "FAILURE MODE: the refused GET echoed an `{MCP_SESSION_ID}` header back.\n\
         CONSEQUENCE: the severed build would be reflecting an attacker-supplied session id as a \
         stream identity — the exact passthrough the `resolve_sse_session` twin was written to \
         remove.\n\
         WHAT TO DO: check that no SSE hardening headers are attached on this build."
    );

    teardown(handle, ()).await;
}

/// A `DELETE` naming a session is still `405`, and leaks no existence oracle.
///
/// On a `v1-compat` build this request is answered `404 Unknown session ID` for an
/// absent session and `200` for a present one — a difference an attacker can use
/// to probe which session ids exist. A build with no session map answers neither:
/// it answers `405` for every id, which is the same answer for all inputs.
#[tokio::test]
async fn a_v1_flavoured_delete_is_405_and_leaks_no_session_oracle() {
    let (addr, handle) = spawn().await;

    let first = within(
        "a v1-flavoured DELETE /",
        delete(
            addr,
            &[
                header(MCP_PROTOCOL_VERSION, V1),
                header(MCP_SESSION_ID, "a-session-that-never-existed"),
            ],
        ),
    )
    .await;
    assert_refused_not_unrouted(&first, "DELETE");

    let second = within(
        "a second v1-flavoured DELETE /",
        delete(
            addr,
            &[
                header(MCP_PROTOCOL_VERSION, V1),
                header(MCP_SESSION_ID, "a-completely-different-id"),
            ],
        ),
    )
    .await;
    assert_refused_not_unrouted(&second, "DELETE");

    assert_eq!(
        first.status, second.status,
        "FAILURE MODE: two different session ids produced different statuses ({} vs {}).\n\
         CONSEQUENCE: that difference IS a session-existence oracle, on a build that holds no \
         sessions to disclose.\n\
         WHAT TO DO: the `full-v2` DELETE answer must not depend on the id it is given.",
        first.status, second.status
    );

    teardown(handle, ()).await;
}

/// The control: a `POST /` on the SAME server still succeeds.
///
/// Without this, every assertion above would also pass against a server that
/// failed to start, was listening on the wrong port, or was refusing everything —
/// a `405` is not obviously distinguishable from "broken" unless something on the
/// same socket demonstrably works. This POST is a real v2 `tools/list` round trip
/// through the required-header gate, so it exercises the machinery the severed
/// build is supposed to KEEP.
#[tokio::test]
async fn a_post_on_the_same_server_still_succeeds() {
    let (addr, handle) = spawn().await;

    let mut headers = v2_headers("tools/list", "");
    headers.push(header(MCP_PROTOCOL_VERSION, V2));
    let response = within(
        "a v2 POST /",
        post(addr, &headers, &v2_body("tools/list", json!(1), json!({}))),
    )
    .await;

    assert_eq!(
        response.status, 200,
        "FAILURE MODE: the control POST answered {}, so the server on this socket is not \
         serving anything.\n\
         CONSEQUENCE: every 405 assertion in this file would pass against a dead or misconfigured \
         server, which is the classic way a severance proof becomes vacuous.\n\
         BODY: {}\n\
         WHAT TO DO: fix the fixture before trusting any other test in this file.",
        response.status, response.raw
    );
    assert!(
        response.body.get("result").is_some(),
        "FAILURE MODE: the control POST returned 200 with no `result` member.\n\
         BODY: {}\n\
         WHAT TO DO: the control must be a REAL round trip, not merely a 200.",
        response.raw
    );

    teardown(handle, ()).await;
}
