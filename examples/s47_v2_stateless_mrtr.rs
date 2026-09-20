//! Example: STATELESS MCP 2026-07-28 server doing MULTI-ROUND-TRIP ELICITATION
//!
//! Run this server with:
//! ```bash
//! cargo run --example s47_v2_stateless_mrtr --features full
//! ```
//!
//! Optionally pass a bind address: `cargo run --example s47_v2_stateless_mrtr
//! --features full -- 127.0.0.1:9000`. Then, in another terminal, run the paired
//! CLIENT which fulfils the elicitation automatically:
//! ```bash
//! cargo run --example s48_v2_mrtr_client --features full
//! ```
//!
//! # What this demonstrates
//!
//! - **No `initialize` handshake.** A 2026-07-28 request declares its era in
//!   `params._meta`, so the very first byte a client sends can be a `tools/call`.
//! - **No `Mcp-Session-Id`.** The server below is built with the STATEFUL default
//!   HTTP config (it still mints sessions for 2025-11-25 clients), yet no v2
//!   response carries a session id. That is the PER-REQUEST era gate, not a
//!   build-time stateless switch.
//! - **A handler that asks for more input.** `weather` cannot answer without a
//!   city, so it returns `resultType: "input_required"` with an
//!   `elicitation/create` entry instead of guessing or failing.
//! - **An AEAD-protected `requestState`.** The handler's continuation is sealed
//!   into an opaque token bound to the caller's principal, the method, and a
//!   digest of the request's salient parameters. A client must echo it verbatim;
//!   it cannot read or forge it.
//! - **Resumption across the retry.** Round two arrives as an independent HTTP
//!   request with a different JSON-RPC id and no session, and the handler still
//!   resumes exactly where it left off.
//! - **Dual version.** The same binary keeps serving 2025-11-25 clients, which is
//!   the whole point of per-request negotiation.
//!
//! # Deployment contract: `PMCP_REQUEST_STATE_KEY`
//!
//! The `requestState` token is sealed with a 32-byte key. How that key is
//! resolved is a SECURITY-RELEVANT deployment decision:
//!
//! - **Unset** — the server generates a fresh PER-PROCESS key and logs a startup
//!   WARNING. Single-instance development is fine; a horizontally-scaled
//!   deployment is NOT, because instance B cannot open instance A's token and
//!   every load-balanced retry is re-elicited from scratch. This example
//!   deliberately leaves the variable unset so you SEE that warning on startup.
//! - **Set and valid** — hex- or base64-encoded 32 bytes, IDENTICAL on every
//!   instance. This is what a multi-instance deployment must do.
//! - **Set and MALFORMED** — the server BUILD fails. A silently degraded
//!   crypto key is worse than a refusal to start.
//! - `PMCP_REQUEST_STATE_KEY_PREVIOUS` joins the ACCEPTING set only, for
//!   zero-downtime rotation, and `PMCP_REQUEST_STATE_TTL_SECS` overrides the
//!   continuation lifetime.
//!
//! A programmatic alternative exists and BEATS the environment:
//! `Server::builder().with_request_state_key([u8; 32])` (plus
//! `.with_request_state_ttl(..)`). Use it when your key comes from a secrets
//! manager rather than the process environment. No key is hardcoded here.

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::shared::http_constants::{ACCEPT_STREAMABLE, MCP_METHOD, MCP_NAME, MCP_PROTOCOL_VERSION};
use pmcp::testing::{META_CLIENT_CAPABILITIES, META_PROTOCOL_VERSION};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::types::elicitation::ElicitRequestParams;
use pmcp::types::mrtr::{InputRequest, InputRequests, InputResponse, MrtrSignal};
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::{RequestHandlerExtra, Server, ToolHandler};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// The tool this example exposes.
const TOOL_NAME: &str = "weather";

/// The argument the handler needs, and the `inputRequests` key it asks under.
///
/// Reusing ONE key for both is not required by the spec — the key is
/// server-assigned and opaque to the client — but it keeps the example readable.
const CITY_KEY: &str = "city";

/// Where the server binds when `argv[1]` is absent.
const DEFAULT_ADDR: &str = "127.0.0.1:8147";

/// A tool that cannot answer without a city, and ASKS for it.
struct WeatherTool;

#[async_trait]
impl ToolHandler for WeatherTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // ---- Round 2: resume from the VERIFIED continuation. ----
        //
        // `mrtr_continuation()` is `Some` only after the server-owned AEAD codec
        // authenticated the token the client echoed back, so this value is
        // SERVER-MINTED and trusted. `input_responses()` is the opposite: it came
        // off the wire and must be treated as untrusted input.
        if let Some(continuation) = extra.mrtr_continuation() {
            let units = continuation
                .get("units")
                .and_then(Value::as_str)
                .unwrap_or("metric");
            let Some(city) = answered_city(&extra) else {
                return Err(pmcp::Error::validation(
                    "the retry carried a valid requestState but no usable city answer",
                ));
            };
            return Ok(json!({
                "city": city,
                "units": units,
                "forecast": "sunny, 21 degrees",
                "resumedAtRound": extra.mrtr_round(),
            }));
        }

        // ---- Round 1a: the caller supplied the city up front. ----
        if let Some(city) = args.get(CITY_KEY).and_then(Value::as_str) {
            return Ok(json!({
                "city": city,
                "units": "metric",
                "forecast": "sunny, 21 degrees",
                "resumedAtRound": Value::Null,
            }));
        }

        // ---- Round 1b: ASK for it. ----
        //
        // This is the ENTIRE server-side MRTR authoring surface: build the
        // requests you need answered, attach whatever continuation state lets you
        // resume, and put the returned pair on the result's `_meta`. The dispatch
        // layer seals `continuation` into the opaque `requestState`, emits
        // `resultType: "input_required"` alongside your `inputRequests`, and
        // removes the internal key before serialization.
        let mut input_requests = InputRequests::new();
        input_requests.insert(
            CITY_KEY.to_string(),
            InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
                message: "Which city should I check the weather for?".to_string(),
                requested_schema: json!({
                    "type": "object",
                    "properties": { CITY_KEY: { "type": "string" } },
                    "required": [CITY_KEY],
                }),
            })),
        );
        let signal = MrtrSignal {
            input_requests,
            // Handler-owned state. It is sealed, never published — the client
            // sees only the opaque token.
            continuation: json!({ "units": "metric" }),
        };
        let (key, value) = signal
            .into_meta_entry()
            .map_err(|error| pmcp::Error::internal(error.to_string()))?;
        let mut meta = serde_json::Map::new();
        meta.insert(key, value);
        extra.set_result_meta(meta);

        Ok(json!({ "status": "I need to know which city first." }))
    }
}

/// Read the city out of the client's `inputResponses`, if it answered usefully.
///
/// Returns `None` for a missing, declined or wrong-shaped answer — every value
/// here is CLIENT-SUPPLIED and must be validated exactly like tool arguments.
fn answered_city(extra: &RequestHandlerExtra) -> Option<String> {
    let InputResponse::Elicitation(result) = extra.input_responses()?.get(CITY_KEY)? else {
        return None;
    };
    result
        .content
        .as_ref()?
        .get(CITY_KEY)?
        .as_str()
        .map(str::to_string)
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // --- Example setup: logging at INFO so the pmcp startup WARNING about an
    // --- unset PMCP_REQUEST_STATE_KEY is visible rather than swallowed.
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info,s47_v2_stateless_mrtr=info")
        .init();

    let requested: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string())
        .parse()?;

    // The accept-list is what opts this server into 2026-07-28. Listing the
    // 2025-11-25 version alongside it is what keeps v1 clients working: the era
    // is negotiated PER REQUEST, so one binary serves both.
    let server = Server::builder()
        .name("s47-v2-stateless-mrtr")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .with_supported_protocol_versions([
            ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        // No `.with_request_state_key(..)` on purpose: see the header. The
        // per-process fallback and its startup WARNING are part of the demo.
        .tool(TOOL_NAME, WeatherTool)
        .build()?;

    // The STATEFUL default config — a live session-id generator. v2 requests are
    // still session-free, because the era gate decides that per request.
    let http = StreamableHttpServer::with_config(
        requested,
        Arc::new(Mutex::new(server)),
        StreamableHttpServerConfig::default(),
    );
    let (addr, server_handle) = http.start().await?;

    print_instructions(addr);

    // A server, not a one-shot script: run until signalled.
    server_handle.await?;
    Ok(())
}

/// Print the bound address and a copy-pasteable round-1 request.
fn print_instructions(addr: SocketAddr) {
    let body = round_one_body();
    println!();
    println!("=============================================================");
    println!("  STATELESS v2 (2026-07-28) MRTR SERVER");
    println!("=============================================================");
    println!("  Listening on : {addr}");
    println!("  Endpoint     : http://{addr}");
    println!(
        "  Versions     : {LATEST_PROTOCOL_VERSION} (v1) and {PROTOCOL_VERSION_2026_07_28} (v2)"
    );
    println!("  Tool         : {TOOL_NAME}");
    println!();
    println!("  If a 'PMCP_REQUEST_STATE_KEY is not set' WARNING appeared above,");
    println!("  that is deliberate — see this example's header for the contract.");
    println!("-------------------------------------------------------------");
    println!("  ROUND 1 — call the tool with NO city. The server answers");
    println!("  resultType: \"input_required\" with an elicitation entry and an");
    println!("  opaque requestState:");
    println!();
    println!("    curl -sS http://{addr} \\");
    println!("      -H 'content-type: application/json' \\");
    println!("      -H 'accept: {ACCEPT_STREAMABLE}' \\");
    println!("      -H '{MCP_PROTOCOL_VERSION}: {PROTOCOL_VERSION_2026_07_28}' \\");
    println!("      -H '{MCP_METHOD}: tools/call' \\");
    println!("      -H '{MCP_NAME}: {TOOL_NAME}' \\");
    println!("      -d '{body}'");
    println!();
    println!("-------------------------------------------------------------");
    println!("  ROUND 2 — copy the requestState string from the round-1");
    println!("  response VERBATIM (it is opaque; do not parse it) and resend the");
    println!("  SAME request with a DIFFERENT JSON-RPC id, adding two TOP-LEVEL");
    println!("  params siblings of name/arguments — never inside _meta:");
    println!();
    println!("      \"requestState\": \"<the token from round 1>\",");
    println!("      \"inputResponses\": {{");
    println!("        \"{CITY_KEY}\": {{ \"action\": \"accept\",");
    println!("                   \"content\": {{ \"{CITY_KEY}\": \"Berlin\" }} }}");
    println!("      }}");
    println!();
    println!("  Keep name and arguments IDENTICAL: the token is bound to a digest");
    println!("  of them, so a token minted for one call cannot be replayed onto");
    println!("  another. The response completes — no session, no handshake.");
    println!("-------------------------------------------------------------");
    println!("  Or let the paired client do all of it for you:");
    println!("    cargo run --example s48_v2_mrtr_client --features full -- {addr}");
    println!("=============================================================");
    println!();
    println!("Press Ctrl+C to stop the server");
}

/// The round-1 JSON-RPC body, built through `serde_json` so the reserved `_meta`
/// key spellings come from pmcp's own constants rather than being retyped.
fn round_one_body() -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": TOOL_NAME,
            "arguments": {},
            "_meta": {
                META_PROTOCOL_VERSION: PROTOCOL_VERSION_2026_07_28,
                // A server may only ask for a capability the client DECLARED, so
                // an under-declaring request is answered -32021 instead.
                META_CLIENT_CAPABILITIES: { "elicitation": {} },
            },
        },
    })
    .to_string()
}
