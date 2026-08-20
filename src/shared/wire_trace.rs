//! Wire-level request/response tracing for the Streamable HTTP client.
//!
//! # Why this exists
//!
//! A server can only reject what a client actually sent, so the first question
//! in any conformance dispute is "what went on the wire?". Before this module
//! that question could not be answered from inside the SDK at all: diagnosing a
//! single deployed server's `400` needed a hand-written TCP proxy and a local
//! fixture, because nothing logged the outgoing headers. For a project whose
//! tooling story is "best-in-class MCP testing", that is the wrong shape.
//!
//! # It is `tracing`, not a bespoke logger
//!
//! Everything here emits on the [`WIRE_TARGET`](crate::shared::wire_trace::WIRE_TARGET) tracing target, so it composes
//! with the ecosystem instead of competing with it:
//!
//! - **debugging**: `RUST_LOG=pmcp::wire=debug cargo run --example …`
//! - **CI**: the same target through `tracing_subscriber`'s JSON layer, so a run
//!   emits machine-readable frames a job can archive as an artifact.
//! - **filtering**: `pmcp::wire=debug` alone, without turning on every other
//!   `pmcp` log, because it is its own target rather than a level on a shared one.
//!
//! # Zero cost when it is off
//!
//! Every entry point is guarded by [`enabled`](crate::shared::wire_trace::enabled) before it builds a single
//! `String`. Redaction, header formatting and body previewing are all
//! allocation-heavy relative to sending a request, so doing them unconditionally
//! and letting the subscriber discard the event would tax every production
//! request for a debugging feature nobody enabled. The guard is what makes it
//! honest to leave the instrumentation permanently compiled in.
//!
//! # Secrets are redacted BY DEFAULT
//!
//! [`REDACTED_HEADERS`](crate::shared::wire_trace::REDACTED_HEADERS) is deny-by-default for the headers that carry
//! credentials or session identity. A wire dump is exactly the artifact someone
//! pastes into a bug report, so the safe default is the only responsible one —
//! and an opt-out is deliberately NOT offered here: a caller that truly needs a
//! raw credential can read it from its own config, not from our logs.

use tracing::Level;

/// The tracing target every wire event is emitted on.
///
/// Its own target rather than a level on `pmcp`, so `pmcp::wire=debug` turns on
/// wire dumps WITHOUT turning on the rest of the SDK's debug logging.
pub const WIRE_TARGET: &str = "pmcp::wire";

/// Headers whose VALUES are never rendered, matched case-insensitively.
///
/// Deny-by-default: the name is still shown (knowing a request carried
/// `Authorization` is diagnostically useful; knowing the bearer token is not).
pub const REDACTED_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "mcp-session-id",
    "x-api-key",
    "api-key",
];

/// Bytes of a body rendered before truncation.
///
/// A `resources/read` of an MCP-App UI resource returns tens of kilobytes of
/// HTML; dumping it whole turns a diagnostic into a haystack and can push
/// secrets from an unrelated field into a log. The preview keeps the JSON-RPC
/// envelope — `method`, `id`, and the head of `params` — which is what a wire
/// dispute is actually about.
pub const BODY_PREVIEW_BYTES: usize = 2048;

/// Whether wire tracing is on.
///
/// Call this BEFORE building any diagnostic string.
#[must_use]
pub fn enabled() -> bool {
    tracing::enabled!(target: WIRE_TARGET, Level::DEBUG)
}

/// Render one header value, redacting it when its name is credential-bearing.
#[must_use]
pub fn render_header_value(name: &str, value: &str) -> String {
    if REDACTED_HEADERS
        .iter()
        .any(|redacted| name.eq_ignore_ascii_case(redacted))
    {
        // The LENGTH is kept: "present but empty" and "present with a value" are
        // different bugs, and distinguishing them is the whole reason a reader
        // is looking at a wire dump.
        format!("<redacted {} bytes>", value.len())
    } else {
        value.to_string()
    }
}

/// Render a body as UTF-8 with a bounded preview.
///
/// Lossy on purpose: a malformed body is exactly the case worth seeing, so
/// invalid UTF-8 becomes U+FFFD rather than suppressing the whole dump.
#[must_use]
pub fn render_body(body: &[u8]) -> String {
    if body.len() <= BODY_PREVIEW_BYTES {
        return String::from_utf8_lossy(body).into_owned();
    }
    format!(
        "{}… <truncated, {} bytes total>",
        String::from_utf8_lossy(&body[..BODY_PREVIEW_BYTES]),
        body.len()
    )
}

/// Format a header list into one `name: value` line per header, redacted.
#[must_use]
pub fn render_headers<'a, I>(headers: I) -> String
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    headers
        .into_iter()
        .map(|(name, value)| format!("{name}: {}", render_header_value(name, value)))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A credential's VALUE never reaches the log; its NAME and length do.
    #[test]
    fn credential_headers_are_redacted_by_default() {
        let rendered = render_header_value("Authorization", "Bearer super-secret-token");
        assert!(
            !rendered.contains("super-secret-token"),
            "the token must never be rendered: {rendered}"
        );
        assert!(
            rendered.contains("25"),
            "the LENGTH is kept so present-but-empty stays distinguishable: {rendered}"
        );
    }

    /// Matching is case-insensitive — a server may echo any casing.
    #[test]
    fn redaction_is_case_insensitive() {
        for name in ["authorization", "AUTHORIZATION", "Mcp-Session-Id", "COOKIE"] {
            let rendered = render_header_value(name, "secret-value");
            assert!(
                !rendered.contains("secret-value"),
                "{name} must be redacted regardless of casing: {rendered}"
            );
        }
    }

    /// The headers a wire dispute is ABOUT are shown verbatim.
    #[test]
    fn protocol_headers_are_shown_verbatim() {
        for (name, value) in [
            ("MCP-Protocol-Version", "2026-07-28"),
            ("Mcp-Method", "resources/read"),
            ("Mcp-Name", "ui://app/keypad"),
            ("Content-Type", "application/json"),
        ] {
            assert_eq!(
                render_header_value(name, value),
                value,
                "{name} is the diagnostic payload and must not be redacted"
            );
        }
    }

    /// A large body is truncated and SAYS it was truncated.
    #[test]
    fn a_large_body_is_truncated_and_says_so() {
        let body = vec![b'x'; BODY_PREVIEW_BYTES * 3];
        let rendered = render_body(&body);
        assert!(
            rendered.contains("truncated"),
            "a silent truncation would be a lie about what was sent"
        );
        assert!(
            rendered.contains(&(BODY_PREVIEW_BYTES * 3).to_string()),
            "the TRUE size must survive truncation: {}",
            &rendered[rendered.len().saturating_sub(80)..]
        );
        assert!(rendered.len() < BODY_PREVIEW_BYTES * 2);
    }

    /// A small body is rendered whole, with no truncation marker.
    #[test]
    fn a_small_body_is_rendered_whole() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"resources/read"}"#;
        let rendered = render_body(body);
        assert_eq!(rendered, String::from_utf8_lossy(body));
        assert!(!rendered.contains("truncated"));
    }

    /// Invalid UTF-8 is shown lossily rather than suppressing the dump.
    #[test]
    fn invalid_utf8_is_lossy_not_fatal() {
        let rendered = render_body(&[0xff, 0xfe, b'o', b'k']);
        assert!(rendered.contains("ok"), "the readable part must survive");
    }

    /// The whole point: a rendered header block carries the v2 routing headers
    /// and hides the credential.
    #[test]
    fn a_rendered_block_shows_routing_and_hides_secrets() {
        let block = render_headers([
            ("MCP-Protocol-Version", "2026-07-28"),
            ("Mcp-Method", "resources/read"),
            ("Authorization", "Bearer nope"),
        ]);
        assert!(block.contains("MCP-Protocol-Version: 2026-07-28"));
        assert!(block.contains("Mcp-Method: resources/read"));
        assert!(block.contains("Authorization: <redacted"));
        assert!(!block.contains("nope"));
    }
}
