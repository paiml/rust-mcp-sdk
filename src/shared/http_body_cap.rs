//! Hardened HTTP plumbing for this crate's OAuth/OIDC surfaces.
//!
//! Two pieces, both consumed by every auth call site so neither policy can
//! diverge between them:
//!
//! 1. `collect_reqwest_body_within_cap` — the streaming, two-refusal bounded
//!    whole-body read. A discovery, token or registration response is read
//!    through it and nothing else, so no peer ever chooses this crate's
//!    allocation.
//! 2. `hardened_discovery_client` — a `reqwest` client whose redirect policy
//!    refuses any redirect that leaves the origin of the URL that issued it.
//!
//! # Why a helper and not a tripwire annotation
//!
//! `tests/v2_bounded_reads_tripwire.rs` treats `reqwest`'s four
//! response-consuming reads — its awaited text, bytes and json accessors,
//! including the turbofished json form — as unbounded whole-body reads, and its
//! `bound_in_scope` recognises **no** bounded form for any of them: those calls
//! take no limit argument, so there is nothing for a structural check to
//! recognise. A call site is therefore fixed by being REWRITTEN into a shape
//! that contains no needle at all, and routing it through this module is that
//! rewrite.
//!
//! The four needles are deliberately NOT spelled literally anywhere in this
//! file, so the same grep that audits a call site reports zero here rather than
//! reporting this explanation.
//!
//! Growing `WHOLE_BODY_ALLOWLIST` instead would contradict that list's own
//! written floor — "This list should shrink, never grow. It is now EMPTY" — and
//! would exempt the read rather than bound it.
//!
//! # Why it is gated, and `pub(crate)`
//!
//! Every item here takes or returns a `reqwest` type, so the module is gated on
//! `feature = "http-client"` and never reaches the `wasm32` build.
//!
//! `src/shared/mod.rs` declares the MODULE `pub(crate)`, and every item in it
//! is `pub(crate)` too: the auth files that consume them are all in-crate, and
//! this hardening adds no public API it does not need.
//!
//! # Why `redundant_pub_crate` is allowed here
//!
//! Two lints `make lint` runs CONTRADICT each other for an item inside a
//! `pub(crate)` module, and only one of them can be satisfied:
//!
//! - `clippy::redundant_pub_crate` (nursery) says a `pub(crate)` item inside an
//!   already-private module should be plain `pub`;
//! - `unreachable_pub`, which `src/lib.rs` turns on crate-wide with `#![warn]`,
//!   says a `pub` item that nothing outside the crate can reach should be
//!   `pub(crate)`.
//!
//! Measured, not reasoned: switching all seven items to `pub` traded 7
//! `redundant_pub_crate` errors for 7 `unreachable_pub` errors. `unreachable_pub`
//! is this crate's own deliberate, crate-wide style choice, so it wins, and the
//! nursery lint is allowed for this module ONLY, with the reason recorded here
//! so nobody re-runs the same experiment.
// Why: `clippy::redundant_pub_crate` (nursery) and the crate-wide `unreachable_pub`
// warning in `src/lib.rs` give opposite instructions for every item in this
// module, because the module itself is `pub(crate)`. See the module doc above —
// `unreachable_pub` wins, so the nursery lint is allowed here and nowhere else.
#![allow(clippy::redundant_pub_crate)]

use crate::error::{Error, Result};
use crate::shared::oauth_validation::same_origin;
use std::time::Duration;
use url::Url;

/// Ceiling on the bytes any single authorization-server response may occupy,
/// in bytes (1 `MiB`).
///
/// This is the cap the pre-existing Dynamic Client Registration read already
/// applied, so routing that site through
/// [`collect_reqwest_body_within_cap`] is a change of MECHANISM — streaming and
/// bounded DURING the read, rather than allocated whole and measured
/// afterwards — and not a change of POLICY.
///
/// A discovery document, a token response and a registration response are each
/// a few kilobytes in practice. Anything approaching this ceiling is either
/// hostile or broken.
pub(crate) const DEFAULT_AUTH_RESPONSE_BYTES: usize = 1_048_576;

/// How many redirects [`hardened_discovery_client`] will follow, all of them
/// within the issuer's own origin.
///
/// A bound is required independently of the origin rule: a server can redirect
/// to itself forever, and a same-origin loop satisfies the origin rule at every
/// hop.
pub(crate) const MAX_DISCOVERY_REDIRECTS: usize = 5;

/// Names the rule in every refusal [`hardened_discovery_client`]'s policy
/// produces, so a caller (and a test) can recognise one without matching on a
/// whole sentence.
pub(crate) const REDIRECT_REFUSAL_MARKER: &str = "discovery redirect refused";

/// Read a `reqwest` response body into raw bytes, bounded by `max_bytes`.
///
/// # The two refusals
///
/// 1. A declared `Content-Length` over the cap is refused before a single body
///    byte is read. The header is a peer-controlled OPTIMISATION, never the
///    authority.
/// 2. The bytes actually delivered are accumulated through `Response::chunk`
///    with a running total checked BEFORE each append, so the read stops
///    mid-flight. A peer that understates or omits `Content-Length` therefore
///    gains nothing, and the allocation is bounded DURING the read rather than
///    measured after it.
///
/// A body of exactly `max_bytes` is ADMITTED; one byte over is refused. An
/// empty body returns an empty `Vec`, which is not an error.
///
/// `accumulated.len() <= max_bytes` holds at the top of every iteration, so
/// `max_bytes - accumulated.len()` cannot underflow and no unguarded `a + b` is
/// ever computed.
///
/// # The refusal message rule
///
/// A refusal names the LIMIT and the observed size, and echoes **no** body
/// content: the refusal must not become a channel for the very bytes it
/// refused. That rule is carried here from the SSE reader it is modelled on so
/// it survives a future edit; a test plants a marker in an oversized body and
/// requires its absence from the message.
///
/// # Why `chunk()` and not `bytes_stream()`
///
/// `Response::chunk` carries no `cfg`, while `bytes_stream` is behind
/// `#[cfg(feature = "stream")]`, which this crate does not enable — `Cargo.toml`
/// pins `reqwest` with `default-features = false` and features
/// `["json", "rustls", "form"]`. Accumulating through `chunk()` therefore costs
/// no dependency-surface change.
///
/// # Errors
///
/// Returns [`Error::Validation`] when the cap is exceeded and
/// [`Error::Internal`] when the read itself fails. That split is the contract
/// [`is_body_over_cap`] reads, so a caller can tell a hostile oversized body
/// (terminal) from a transient transport failure (retryable) without matching
/// on message text.
pub(crate) async fn collect_reqwest_body_within_cap(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    // Refusal 1 — advisory, and only ever an early exit.
    if let Some(declared) = response.content_length() {
        if declared > max_bytes as u64 {
            return Err(auth_body_over_cap(max_bytes, Some(declared)));
        }
    }

    // Refusal 2 — authoritative, over the bytes actually delivered.
    let mut accumulated: Vec<u8> = Vec::new();
    loop {
        let next = response.chunk().await;
        let Some(chunk) = next.map_err(|e| {
            Error::internal(format!(
                "authorization-server response body read failed: {e}"
            ))
        })?
        else {
            break;
        };
        // Overflow-safe by construction: `accumulated.len() <= max_bytes` is the
        // loop invariant, so `max_bytes - accumulated.len()` cannot underflow,
        // and no unguarded `a + b` is ever computed.
        if chunk.len() > max_bytes - accumulated.len() {
            return Err(auth_body_over_cap(max_bytes, None));
        }
        accumulated.extend_from_slice(&chunk);
    }

    Ok(accumulated)
}

/// Whether an error from [`collect_reqwest_body_within_cap`] is the CAP refusal
/// rather than a mid-read transport failure.
///
/// The distinction is load-bearing for discovery: an oversized body is a
/// TERMINAL failure that must abort the whole probe, while a failed read is an
/// availability failure that may be retried. Reading the variant rather than the
/// message keeps that classification from depending on wording.
pub(crate) fn is_body_over_cap(error: &Error) -> bool {
    matches!(error, Error::Validation(_))
}

/// Build the over-cap refusal for [`collect_reqwest_body_within_cap`].
///
/// Names the LIMIT and the observed size, and deliberately echoes no body
/// content. `declared` is `Some` only when the peer's own `Content-Length` was
/// over the cap; when the peer understated or omitted it, the read was stopped
/// mid-flight and no total is knowable, so the message says so rather than
/// inventing one.
fn auth_body_over_cap(max_bytes: usize, declared: Option<u64>) -> Error {
    let observed = match declared {
        Some(bytes) => format!("declares Content-Length {bytes}"),
        None => "delivered more than the cap (Content-Length absent or understated)".to_string(),
    };
    Error::validation(format!(
        "authorization-server response body {observed}, over the {max_bytes}-byte cap \
         (DEFAULT_AUTH_RESPONSE_BYTES); refusing to read it. No byte of the refused body is \
         reproduced here"
    ))
}

/// A `reqwest` client for authorization-server metadata discovery whose redirect
/// policy cannot be steered off the issuer's origin.
///
/// # The rule, and why it exists
///
/// A redirect is followed ONLY when its target shares the origin — scheme, host
/// and effective port, judged by
/// [`same_origin`](crate::shared::oauth_validation::same_origin) — of the URL
/// that issued it. Everything upstream of the fetch validates the ISSUER, so a
/// `https://issuer.example/.well-known/...` that answers `302` with
/// `Location: http://attacker.example/...` would defeat every one of those
/// checks: the document that arrives is authored by a host the issuer check
/// never saw. The `https` to `http` case is the same defect in miniature, which
/// is why the scheme is part of the origin and not merely a preference.
///
/// The redirect count is bounded at [`MAX_DISCOVERY_REDIRECTS`] independently,
/// because a server can redirect to ITSELF forever and every hop of such a loop
/// satisfies the origin rule.
///
/// Both discovery call sites in this crate build their client here, so the
/// policy cannot diverge between them.
///
/// A refusal names only the two ORIGINS involved, never the full URLs: an origin
/// carries no path, and a discovery path is the part an operator is least
/// likely to want in a log.
///
/// # Errors
///
/// Returns [`Error::Internal`] when `reqwest` cannot build a client at all
/// (a TLS backend or proxy-configuration failure). Callers must surface that
/// rather than silently falling back to a default client, which would discard
/// the policy this function exists to install.
pub(crate) fn hardened_discovery_client(timeout: Duration) -> Result<reqwest::Client> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        // The borrows of `attempt` end with this block, so the decision is owned
        // by the time `attempt` is consumed below.
        let decision = {
            let previous = attempt.previous();
            if previous.len() > MAX_DISCOVERY_REDIRECTS {
                Err(redirect_limit_refusal())
            } else {
                match previous.last() {
                    Some(from) if discovery_redirect_permitted(from, attempt.url()) => Ok(()),
                    Some(from) => Err(cross_origin_redirect_refusal(from, attempt.url())),
                    // Unreachable with today's `reqwest`, which pushes the
                    // redirecting URL onto `previous` before consulting the
                    // policy. Fail CLOSED if that ever changes: with nothing to
                    // compare against, no target can be judged same-origin.
                    None => Err(unjudgeable_redirect_refusal()),
                }
            }
        };
        match decision {
            Ok(()) => attempt.follow(),
            Err(message) => attempt.error(message),
        }
    });

    reqwest::Client::builder()
        .timeout(timeout)
        .redirect(policy)
        .build()
        .map_err(|e| {
            Error::internal(format!(
                "failed to build the hardened discovery HTTP client, so discovery cannot run \
                 without dropping its redirect policy: {e}"
            ))
        })
}

/// Whether a redirect from `previous` to `next` stays inside one origin.
///
/// This is the whole of the policy's decision, extracted so every row of the
/// rule — including the `https` to `http` downgrade, which no plaintext mock
/// server can drive over the wire — is assertable directly.
fn discovery_redirect_permitted(previous: &Url, next: &Url) -> bool {
    same_origin(previous, next)
}

/// Whether a `reqwest` error is a redirect refusal rather than a transport
/// failure.
///
/// A caller uses this to classify: a refused redirect is a statement about the
/// document's AUTHORSHIP, so retrying it returns the same answer and falling
/// through to another candidate would be the silent downgrade the policy exists
/// to prevent. A connect or timeout failure is an availability problem and is
/// retryable.
pub(crate) fn is_redirect_refusal(error: &reqwest::Error) -> bool {
    error.is_redirect()
}

/// The refusal for a redirect that leaves the issuer's origin.
fn cross_origin_redirect_refusal(previous: &Url, next: &Url) -> String {
    format!(
        "{REDIRECT_REFUSAL_MARKER}: {} redirected to {}, a DIFFERENT origin. A discovery redirect \
         that leaves the issuer's origin hands document authorship to another host and defeats \
         every issuer check upstream of it. Only the origins are named here; the paths are not \
         reproduced",
        origin_of(previous),
        origin_of(next),
    )
}

/// The refusal for a redirect chain longer than [`MAX_DISCOVERY_REDIRECTS`].
fn redirect_limit_refusal() -> String {
    format!(
        "{REDIRECT_REFUSAL_MARKER}: more than {MAX_DISCOVERY_REDIRECTS} redirects were offered. \
         Every hop of a server redirecting to itself satisfies the same-origin rule, so the count \
         is bounded separately rather than followed forever"
    )
}

/// The fail-closed refusal for a redirect with no previous URL to judge against.
fn unjudgeable_redirect_refusal() -> String {
    format!(
        "{REDIRECT_REFUSAL_MARKER}: the redirect chain carried no previous URL, so the target's \
         origin cannot be compared against anything and the redirect fails closed"
    )
}

/// Render a URL's origin — scheme, host and effective port — and nothing else.
fn origin_of(url: &Url) -> String {
    let host = url.host_str().unwrap_or("<no host>");
    match url.port_or_known_default() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use std::error::Error as StdError;
    use std::time::Duration;
    use url::Url;

    /// A cap small enough that a fixture crosses it in bytes rather than
    /// megabytes. Production passes `DEFAULT_AUTH_RESPONSE_BYTES`.
    const TEST_CAP: usize = 32;

    /// Planted in every oversized fixture body. A refusal that names it would be
    /// echoing the very bytes it refused.
    const CANARY: &str = "MARKER-DO-NOT-ECHO-b3f1";

    fn test_timeout() -> Duration {
        Duration::from_secs(10)
    }

    /// Render an error together with its whole `source` chain.
    ///
    /// `reqwest::Error`'s own `Display` deliberately omits the source, so the
    /// custom redirect refusal is only visible by walking the chain.
    fn rendered_chain(error: &dyn StdError) -> String {
        let mut rendered = error.to_string();
        let mut current = error.source();
        while let Some(cause) = current {
            rendered.push_str(" <- ");
            rendered.push_str(&cause.to_string());
            current = cause.source();
        }
        rendered
    }

    #[tokio::test]
    async fn within_cap_refuses_a_declared_content_length_over_the_cap() {
        let mut server = Server::new_async().await;
        let body = format!("{}{}", CANARY, "x".repeat(200));
        let _m = server
            .mock("GET", "/big")
            .with_status(200)
            .with_body(&body)
            .create_async()
            .await;

        let response = reqwest::Client::new()
            .get(format!("{}/big", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.content_length(),
            Some(body.len() as u64),
            "the fixture must DECLARE a Content-Length or this row exercises refusal 2, not refusal 1"
        );

        let error = collect_reqwest_body_within_cap(response, TEST_CAP)
            .await
            .unwrap_err();
        let message = error.to_string();
        assert!(
            is_body_over_cap(&error),
            "a cap refusal must be distinguishable from a mid-read failure: {message}"
        );
        assert!(
            message.contains(&TEST_CAP.to_string()),
            "the refusal must name the LIMIT: {message}"
        );
        assert!(
            message.contains(&body.len().to_string()),
            "the Content-Length variant must name the DECLARED size: {message}"
        );
        assert!(
            !message.contains(CANARY),
            "a refusal must never echo a byte of the body it refused: {message}"
        );
    }

    #[tokio::test]
    async fn within_cap_refuses_a_chunked_body_that_exceeds_the_cap_mid_flight() {
        let mut server = Server::new_async().await;
        let _m = server
            .mock("GET", "/stream")
            .with_status(200)
            .with_chunked_body(|writer| {
                writer.write_all(CANARY.as_bytes())?;
                writer.write_all(&[b'x'; 500])
            })
            .create_async()
            .await;

        let response = reqwest::Client::new()
            .get(format!("{}/stream", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.content_length(),
            None,
            "the fixture must OMIT Content-Length or refusal 1 short-circuits this row"
        );

        let error = collect_reqwest_body_within_cap(response, TEST_CAP)
            .await
            .unwrap_err();
        let message = error.to_string();
        assert!(
            is_body_over_cap(&error),
            "a mid-flight cap refusal is still a cap refusal: {message}"
        );
        assert!(
            message.contains(&TEST_CAP.to_string()),
            "the refusal must name the LIMIT: {message}"
        );
        assert!(
            message.contains("Content-Length absent or understated"),
            "the mid-flight variant must state that no total is knowable rather than invent one: \
             {message}"
        );
        assert!(
            !message.contains(CANARY),
            "a refusal must never echo a byte of the body it refused: {message}"
        );
    }

    #[tokio::test]
    async fn within_cap_admits_a_body_exactly_at_the_cap() {
        let mut server = Server::new_async().await;
        let body = "y".repeat(TEST_CAP);
        let _m = server
            .mock("GET", "/exact")
            .with_status(200)
            .with_body(&body)
            .create_async()
            .await;

        let response = reqwest::Client::new()
            .get(format!("{}/exact", server.url()))
            .send()
            .await
            .unwrap();
        let bytes = collect_reqwest_body_within_cap(response, TEST_CAP)
            .await
            .unwrap();
        assert_eq!(bytes.len(), TEST_CAP);
        assert_eq!(bytes, body.as_bytes());
    }

    #[tokio::test]
    async fn within_cap_returns_byte_identical_content_under_the_cap() {
        let mut server = Server::new_async().await;
        // Non-ASCII on purpose: the helper returns RAW bytes, so nothing here
        // may be normalised or lossily decoded on the way through.
        let body = r#"{"issuer":"https://as.example","note":"üπ"}"#;
        let _m = server
            .mock("GET", "/small")
            .with_status(200)
            .with_body(body)
            .create_async()
            .await;

        let response = reqwest::Client::new()
            .get(format!("{}/small", server.url()))
            .send()
            .await
            .unwrap();
        let bytes = collect_reqwest_body_within_cap(response, DEFAULT_AUTH_RESPONSE_BYTES)
            .await
            .unwrap();
        assert_eq!(bytes, body.as_bytes());
    }

    #[tokio::test]
    async fn within_cap_returns_an_empty_vec_for_an_empty_body() {
        let mut server = Server::new_async().await;
        let _m = server
            .mock("GET", "/empty")
            .with_status(204)
            .create_async()
            .await;

        let response = reqwest::Client::new()
            .get(format!("{}/empty", server.url()))
            .send()
            .await
            .unwrap();
        let bytes = collect_reqwest_body_within_cap(response, TEST_CAP)
            .await
            .unwrap();
        assert!(bytes.is_empty(), "an empty body is not an error");
    }

    #[test]
    fn hardened_discovery_client_permits_only_same_origin_redirect_targets() {
        let from = Url::parse("https://as.example/.well-known/openid-configuration").unwrap();

        assert!(discovery_redirect_permitted(
            &from,
            &Url::parse("https://as.example/elsewhere").unwrap()
        ));
        assert!(
            discovery_redirect_permitted(
                &from,
                &Url::parse("https://as.example:443/elsewhere").unwrap()
            ),
            "the EFFECTIVE port is what makes the explicit 443 the same origin"
        );

        assert!(
            !discovery_redirect_permitted(&from, &Url::parse("https://cdn.example/x").unwrap()),
            "a different host is a different origin"
        );
        // mockito cannot serve TLS, so the https -> http downgrade row is
        // asserted against the very function the redirect policy calls. The
        // wire-level sibling below drives the same rule in the http -> https
        // direction through a live redirect.
        assert!(
            !discovery_redirect_permitted(&from, &Url::parse("http://as.example/x").unwrap()),
            "an https -> http downgrade on the SAME host is still a different origin"
        );
        // The row above is NOT on its own a scheme detector: dropping the scheme
        // from the comparison still refuses it, because the two DEFAULT ports
        // (443 and 80) differ. Measured — under a host+port-only break that row
        // still passed. This one pins the same effective port on both sides, so
        // only the scheme can decide it.
        let explicit = Url::parse("https://as.example:8443/.well-known/openid-configuration")
            .expect("fixture URL parses");
        assert!(
            !discovery_redirect_permitted(
                &explicit,
                &Url::parse("http://as.example:8443/x").unwrap()
            ),
            "an https -> http downgrade on the same host AND the same port is a different origin"
        );
        assert!(
            !discovery_redirect_permitted(&from, &Url::parse("https://as.example:8443/x").unwrap()),
            "a different port is a different origin"
        );
    }

    #[tokio::test]
    async fn hardened_discovery_client_follows_a_same_origin_redirect() {
        let mut server = Server::new_async().await;
        let _target = server
            .mock("GET", "/target")
            .with_status(200)
            .with_body("arrived")
            .create_async()
            .await;
        let _redirect = server
            .mock("GET", "/redirect")
            .with_status(302)
            .with_header("location", "/target")
            .create_async()
            .await;

        let client = hardened_discovery_client(test_timeout()).unwrap();
        let response = client
            .get(format!("{}/redirect", server.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        let bytes = collect_reqwest_body_within_cap(response, TEST_CAP)
            .await
            .unwrap();
        assert_eq!(bytes, b"arrived");
    }

    #[tokio::test]
    async fn hardened_discovery_client_refuses_a_cross_origin_redirect() {
        let mut origin = Server::new_async().await;
        let mut elsewhere = Server::new_async().await;

        // A SECOND mockito server is a genuinely different origin (its port
        // differs), so `expect(0)` here proves the redirect was not followed
        // rather than merely that it failed.
        let never = elsewhere
            .mock("GET", "/target")
            .with_status(200)
            .with_body("SHOULD NOT BE FETCHED")
            .expect(0)
            .create_async()
            .await;
        let _redirect = origin
            .mock("GET", "/offsite")
            .with_status(302)
            .with_header("location", &format!("{}/target", elsewhere.url()))
            .create_async()
            .await;

        let client = hardened_discovery_client(test_timeout()).unwrap();
        let error = client
            .get(format!("{}/offsite", origin.url()))
            .send()
            .await
            .unwrap_err();

        assert!(
            is_redirect_refusal(&error),
            "a refused redirect must be distinguishable from a transport failure: {error}"
        );
        assert!(
            !error.is_connect(),
            "the refusal must happen BEFORE any connection to the other origin: {error}"
        );
        let chain = rendered_chain(&error);
        assert!(
            chain.contains(REDIRECT_REFUSAL_MARKER),
            "the refusal must name the rule it enforced: {chain}"
        );
        never.assert_async().await;
    }

    #[tokio::test]
    async fn hardened_discovery_client_refuses_a_scheme_change_on_the_same_host() {
        let mut server = Server::new_async().await;
        let never = server
            .mock("GET", "/target")
            .with_status(200)
            .expect(0)
            .create_async()
            .await;
        let _redirect = server
            .mock("GET", "/downgrade")
            .with_status(302)
            .with_header(
                "location",
                &format!("https://{}/target", server.host_with_port()),
            )
            .create_async()
            .await;

        let client = hardened_discovery_client(test_timeout()).unwrap();
        let error = client
            .get(format!("{}/downgrade", server.url()))
            .send()
            .await
            .unwrap_err();

        assert!(
            is_redirect_refusal(&error),
            "a scheme change is an origin change: {error}"
        );
        assert!(
            !error.is_connect(),
            "the refusal must happen BEFORE any TLS attempt against a plaintext port: {error}"
        );
        never.assert_async().await;
    }

    #[tokio::test]
    async fn hardened_discovery_client_bounds_a_redirect_loop_within_one_origin() {
        let mut server = Server::new_async().await;
        let _loop_mock = server
            .mock("GET", "/loop")
            .with_status(302)
            .with_header("location", "/loop")
            .expect_at_least(1)
            .create_async()
            .await;

        let client = hardened_discovery_client(test_timeout()).unwrap();
        let error = client
            .get(format!("{}/loop", server.url()))
            .send()
            .await
            .unwrap_err();

        assert!(
            is_redirect_refusal(&error),
            "a same-origin redirect loop must TERMINATE rather than hang: {error}"
        );
        let chain = rendered_chain(&error);
        assert!(
            chain.contains(REDIRECT_REFUSAL_MARKER),
            "the refusal must name the rule it enforced: {chain}"
        );
        assert!(
            chain.contains(&MAX_DISCOVERY_REDIRECTS.to_string()),
            "the refusal must name the redirect LIMIT: {chain}"
        );
    }
}
