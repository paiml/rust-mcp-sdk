//! Example: RFC 9207 `iss` validation and CSRF `state` validation, actually run.
//!
//! Run with:
//!   cargo run --example c11_oauth_iss_state_validation
//!
//! This example does NOT require network access — and it goes further than
//! that: it needs no browser, no loopback listener, no credentials, no
//! environment variable and no `oauth` feature. Every line below EXECUTES the
//! shipped validation logic rather than narrating what it would do, because
//! `validate_authorization_response` is a pure function of
//! `(callback query bytes, per-request record)`.
//!
//! # Why this example carries no `oauth` feature gate
//!
//! (Written without naming the attribute, so an audit grepping for a gate over
//! this file finds nothing at all rather than this paragraph.)
//!
//! Its sibling `c08_oauth_dcr.rs` is gated, because `OAuthHelper` needs an HTTP
//! client and a browser. This tier deliberately is not: the whole point of
//! decisions D-05 and D-06 is that a Cloudflare Workers or AWS Lambda redirect
//! handler — which cannot use `webbrowser`, `dirs` or a loopback `TcpListener`,
//! and may be compiled for `wasm32` — must be able to reach exactly this code.
//! An example that runs WITHOUT the feature is itself the demonstration that it
//! can. It also runs with `--features full,oauth`, unchanged.
//!
//! # What it shows
//!
//! 1. Accept — a well-formed callback carrying the matching `state` and `iss`.
//! 2. Reject — an `iss` that does not match the issuer recorded before the
//!    redirect, with the stable programmatic discriminators callers branch on.
//! 3. Reject — a `state` that does not match, which is the CSRF check.
//! 4. The trigger — how `IssPresence` is resolved from the environment, the
//!    builder and the authorization server's own metadata flag.
//!
//! The two rejections are EXPECTED outcomes that this example handles and
//! reports. It exits 0.

use pmcp::shared::oauth_validation::iss_presence_from;
use pmcp::{validate_authorization_response, AuthorizationRequestRecord, IssPresence};

/// The issuer read from the authorization server's validated metadata document
/// BEFORE the user-agent was redirected. RFC 9207's protection is only as good
/// as this value: it "provides no protection if the expected issuer was
/// obtained from an unvalidated source".
const RECORDED_ISSUER: &str = "https://as.example";

/// A different authorization server — the one a mix-up attack redirects to.
const ATTACKER_ISSUER: &str = "https://attacker.example";

/// The opaque, unguessable CSRF value generated for this one request.
const RECORDED_STATE: &str = "kR9x-2Qm7-Lz4T";

/// The PKCE verifier bound into the same record, per the specification's
/// "same per-request record" requirement. Not consulted by the validation — the
/// token exchange that follows needs it.
const CODE_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== PMCP SDK — RFC 9207 `iss` and CSRF `state` validation ===\n");
    println!("No network, no browser, no listener, no `oauth` feature.\n");

    // ONE record binds the issuer, the PKCE verifier, the state and the `iss`
    // policy together. Three separate locals would let one request's `state` be
    // validated against another request's record, and would make forgetting one
    // of them a silent mistake rather than a compile error.
    let record = AuthorizationRequestRecord::new(
        RECORDED_ISSUER,
        CODE_VERIFIER,
        RECORDED_STATE,
        IssPresence::Optional,
    );
    println!("Recorded before the redirect:");
    println!("  expected_issuer = {}", record.expected_issuer());
    println!("  state           = {}", record.state());
    println!("  iss_presence    = {:?}", record.iss_presence());
    // The Debug rendering is REDACTING: neither secret survives into a log line.
    println!("  record (Debug)  = {record:?}");
    println!();

    scenario_1_accept(&record);
    scenario_2_iss_mismatch(&record);
    scenario_3_state_mismatch(&record);
    scenario_4_precedence();

    println!("=== Done. Both refusals above are EXPECTED; this example exits 0. ===");
    Ok(())
}

/// Scenario 1 — the happy path.
fn scenario_1_accept(record: &AuthorizationRequestRecord) {
    println!("--- Scenario 1: ACCEPT — matching `state`, matching `iss` ---");
    let query = format!("code=SplxlOBeZQQYbYS6WxSbIA&state={RECORDED_STATE}&iss={RECORDED_ISSUER}");
    println!("  callback query : {query}");

    match validate_authorization_response(&query, record) {
        Ok(code) => {
            println!("  RESULT         : accepted");
            println!("  authorization code = {code}");
            println!("  -> this is the code that may now be redeemed at the token endpoint.");
        },
        Err(err) => println!("  UNEXPECTED     : {err}"),
    }
    println!();
}

/// Scenario 2 — a mix-up attack: the right `state`, the wrong issuer.
fn scenario_2_iss_mismatch(record: &AuthorizationRequestRecord) {
    println!("--- Scenario 2: REJECT — `iss` mismatch (RFC 9207 §2.4) ---");
    let query = format!("code=SplxlOBeZQQYbYS6WxSbIA&state={RECORDED_STATE}&iss={ATTACKER_ISSUER}");
    println!("  callback query : {query}");

    match validate_authorization_response(&query, record) {
        Ok(code) => println!("  UNEXPECTED     : accepted, code = {code}"),
        Err(err) => {
            println!("  RESULT         : refused");
            // The STABLE programmatic discriminators. Callers branch on these,
            // never on the message text, which is free to change.
            println!("  err.is_iss_mismatch()   = {}", err.is_iss_mismatch());
            println!("  err.iss_expected()      = {:?}", err.iss_expected());
            println!("  err.iss_actual()        = {:?}", err.iss_actual());
            println!("  message                 : {err}");
            println!(
                "  -> the authorization code was NOT redeemed. Sending it to the recorded\n     \
                 issuer's token endpoint is exactly the mix-up attack this check prevents."
            );
        },
    }
    println!();
}

/// Scenario 3 — CSRF: a `state` that was never issued by this client.
fn scenario_3_state_mismatch(record: &AuthorizationRequestRecord) {
    println!("--- Scenario 3: REJECT — `state` mismatch (CSRF) ---");
    let query =
        format!("code=SplxlOBeZQQYbYS6WxSbIA&state=not-the-state-we-sent&iss={RECORDED_ISSUER}");
    println!("  callback query : {query}");

    match validate_authorization_response(&query, record) {
        Ok(code) => println!("  UNEXPECTED     : accepted, code = {code}"),
        Err(err) => {
            let message = err.to_string();
            println!("  RESULT         : refused");
            println!("  err.is_state_mismatch() = {}", err.is_state_mismatch());
            println!("  message                 : {message}");
            println!(
                "  -> the message names NEITHER state value, deliberately: recorded present = \
                 {}, received present = {}.",
                message.contains(RECORDED_STATE),
                message.contains("not-the-state-we-sent")
            );
            println!(
                "     An error string reaches logs, terminals and crash reports, and the \
                 recorded\n     `state` is a live CSRF token for as long as the request is open."
            );
            println!(
                "  -> `state` is also evaluated BEFORE `iss`, so a forged callback cannot use \
                 the\n     refusal to learn anything about the issuer comparison."
            );
        },
    }
    println!();
}

/// Scenario 4 — how the policy that governs an ABSENT `iss` is resolved.
fn scenario_4_precedence() {
    println!("--- Scenario 4: the trigger — D-04 precedence (env > builder > discovery) ---");
    println!("  A PRESENT `iss` is ALWAYS compared, whatever any of these say. The only");
    println!("  configurable question is whether an ABSENT `iss` is fatal.\n");

    // Row 1: the operator's environment override wins over both lower tiers.
    let env_wins = iss_presence_from(
        Some(IssPresence::Required),
        Some(IssPresence::Optional),
        Some(false),
    );
    println!("  env=strict, builder=lenient, discovery=false  -> {env_wins:?}");

    // Row 2: with no override, the builder wins over the discovery flag.
    let builder_wins = iss_presence_from(None, Some(IssPresence::Required), Some(false));
    println!("  env=unset,  builder=strict,  discovery=false  -> {builder_wins:?}");

    // Row 3: with neither, an authorization server that ADVERTISES
    // `authorization_response_iss_parameter_supported: true` makes an absent
    // `iss` fatal all by itself.
    let discovery_wins = iss_presence_from(None, None, Some(true));
    println!("  env=unset,  builder=unset,   discovery=TRUE   -> {discovery_wins:?}");
    println!();

    // And that last row has teeth: with Required, a callback carrying no `iss`
    // at all is refused even though its `state` matches perfectly.
    let strict = AuthorizationRequestRecord::new(
        RECORDED_ISSUER,
        CODE_VERIFIER,
        RECORDED_STATE,
        discovery_wins,
    );
    let query = format!("code=SplxlOBeZQQYbYS6WxSbIA&state={RECORDED_STATE}");
    println!("  With that {discovery_wins:?} record, a callback with NO `iss`:");
    println!("    callback query : {query}");
    match validate_authorization_response(&query, &strict) {
        Ok(code) => println!("    UNEXPECTED   : accepted, code = {code}"),
        Err(err) => {
            println!("    RESULT       : refused");
            println!("    err.is_iss_mismatch()   = {}", err.is_iss_mismatch());
            println!("    err.iss_expected()      = {:?}", err.iss_expected());
            println!(
                "    err.iss_actual()        = {:?}   (None = advertised but ABSENT)",
                err.iss_actual()
            );
        },
    }
    println!();
}
