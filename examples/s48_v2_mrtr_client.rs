//! Example: a 2026-07-28 CLIENT that fulfils multi-round-trip elicitation
//! automatically.
//!
//! Start the paired SERVER first:
//! ```bash
//! cargo run --example s47_v2_stateless_mrtr --features full
//! ```
//!
//! Then run this client with:
//! ```bash
//! cargo run --example s48_v2_mrtr_client --features full
//! ```
//!
//! It takes the server address as `argv[1]` and defaults to `127.0.0.1:8147`,
//! which is where `s47` binds when it is given no address of its own. This is a
//! one-shot script: it exits 0 when every demonstration behaved as documented,
//! and non-zero otherwise.
//!
//! # What this demonstrates
//!
//! 1. **Automatic fulfilment.** With a `HostElicitationHandler` registered, the
//!    plain `call_tool` you already use returns the COMPLETED result. The
//!    gather -> resend loop — a second HTTP request, a fresh JSON-RPC id, the
//!    verbatim `requestState`, the symmetric `inputResponses` map — happens
//!    inside the SDK and is invisible to the caller.
//! 2. **The unfulfillable case is not a silent empty success.** When the handler
//!    DECLINES, the client refuses to answer on the user's behalf and the caller
//!    receives `Error::input_required_unfulfilled` carrying the full result.
//!    `CallToolResult::content` is `#[serde(default)]`, so without this an
//!    `input_required` result would deserialize into an EMPTY `CallToolResult`
//!    and look like success.
//! 3. **Capability honesty, both directions.** A client's declared v2
//!    capabilities are derived from its REGISTERED handlers — it cannot
//!    advertise `elicitation` it could not service. A server may only ask for a
//!    capability the client declared, so a handler-less client is answered
//!    `-32021` rather than handed a question it could never answer. That is why
//!    demonstration 2 uses a DECLINING handler rather than no handler at all.
//!
//! The elicitation handler here answers PROGRAMMATICALLY rather than reading
//! stdin, so the example stays scriptable — and it is the shape an autonomous
//! agent uses. An interactive host would prompt its user in the same callback.

use async_trait::async_trait;
use pmcp::client::host::HostElicitationHandler;
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use pmcp::shared::StreamableHttpTransport;
use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
use pmcp::{Client, ClientBuilder};
use serde_json::json;
use std::collections::HashMap;
use url::Url;

/// The tool `s47` exposes.
const TOOL_NAME: &str = "weather";

/// The `inputRequests` key `s47` asks under, and the field its schema wants.
const CITY_KEY: &str = "city";

/// Where `s47` binds when it is given no address.
const DEFAULT_ADDR: &str = "127.0.0.1:8147";

/// An elicitation handler that answers from configuration instead of prompting.
struct ScriptedElicitation {
    /// What to answer with. `Accept` fulfils the request; anything else means
    /// "the user said no", and the SDK will then NOT resend.
    action: ElicitAction,
    /// The city to report when accepting.
    city: &'static str,
}

#[async_trait]
impl HostElicitationHandler for ScriptedElicitation {
    async fn handle_elicitation(&self, params: ElicitRequestParams) -> pmcp::Result<ElicitResult> {
        if let ElicitRequestParams::Form { message, .. } = &params {
            println!("    server asked  : {message}");
        }
        let accepted = matches!(self.action, ElicitAction::Accept);
        println!(
            "    client answers: {}",
            if accepted { self.city } else { "(declined)" }
        );
        let mut content = HashMap::new();
        content.insert(CITY_KEY.to_string(), json!(self.city));
        Ok(ElicitResult {
            action: self.action,
            content: accepted.then_some(content),
        })
    }
}

/// A `Client` opted into 2026-07-28, optionally carrying an elicitation handler.
///
/// A fresh transport per client keeps the three demonstrations independent.
fn v2_client(
    url: &Url,
    handler: Option<ScriptedElicitation>,
) -> pmcp::Result<Client<StreamableHttpTransport>> {
    let transport = StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(url.clone()).build(),
    );
    let builder = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?;
    Ok(match handler {
        Some(handler) => builder.on_elicitation(handler).build(),
        None => builder.build(),
    })
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string());
    let url = Url::parse(&format!("http://{addr}/"))?;

    println!();
    println!("=============================================================");
    println!("  v2 (2026-07-28) MRTR CLIENT  ->  http://{addr}");
    println!("=============================================================");

    demo_automatic_fulfilment(&url).await?;
    demo_unfulfilled_is_returned(&url).await?;
    demo_undeclared_capability(&url).await?;

    println!();
    println!("=============================================================");
    println!("  All three demonstrations behaved as documented.");
    println!("=============================================================");
    Ok(())
}

/// 1. A registered handler makes the whole exchange invisible to the caller.
async fn demo_automatic_fulfilment(
    url: &Url,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[1] Automatic fulfilment — plain call_tool, handler ACCEPTS");
    println!("-------------------------------------------------------------");
    let client = v2_client(
        url,
        Some(ScriptedElicitation {
            action: ElicitAction::Accept,
            city: "Berlin",
        }),
    )?;

    let result = client.call_tool(TOOL_NAME.to_string(), json!({})).await?;

    println!("    caller receives a COMPLETE result:");
    for content in &result.content {
        println!("      {}", serde_json::to_string(content)?);
    }
    if result.content.is_empty() {
        return Err("expected a completed result with content".into());
    }
    println!("    (two HTTP requests, no initialize, no Mcp-Session-Id)");
    Ok(())
}

/// 2. A DECLINED elicitation reaches the caller as the typed error, never as an
///    empty success.
async fn demo_unfulfilled_is_returned(
    url: &Url,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[2] Unfulfilled — plain call_tool, handler DECLINES");
    println!("-------------------------------------------------------------");
    let client = v2_client(
        url,
        Some(ScriptedElicitation {
            action: ElicitAction::Decline,
            city: "Berlin",
        }),
    )?;

    let error = match client.call_tool(TOOL_NAME.to_string(), json!({})).await {
        Ok(result) => {
            return Err(format!(
                "an input_required result must NOT surface as a CallToolResult with \
                 {} content items — that would be a silently empty success",
                result.content.len()
            )
            .into())
        },
        Err(error) => error,
    };
    if !error.is_input_required_unfulfilled() {
        return Err(format!("expected an input_required_unfulfilled error, got: {error}").into());
    }
    let Some(result) = error.input_required_result() else {
        return Err("the full input_required result must be recoverable".into());
    };
    println!("    caller receives Error::input_required_unfulfilled, carrying:");
    println!("      resultType    : {}", result.result_type);
    println!(
        "      inputRequests : {:?}",
        result
            .input_requests
            .as_ref()
            .map(|requests| requests.keys().collect::<Vec<_>>())
            .unwrap_or_default()
    );
    println!(
        "      requestState  : {} (opaque — never parsed by the client)",
        if result.request_state.is_some() {
            "present"
        } else {
            "absent"
        }
    );
    println!("    the user said no, so the client did NOT resend on their behalf.");
    Ok(())
}

/// 3. A handler-less client declares no `elicitation`, so the server refuses
///    rather than asking a question it could never answer.
async fn demo_undeclared_capability(
    url: &Url,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[3] Capability honesty — NO handler registered");
    println!("-------------------------------------------------------------");
    let client = v2_client(url, None)?;

    match client.call_tool(TOOL_NAME.to_string(), json!({})).await {
        Ok(_) => Err("a server must not fulfil what it had to elicit".into()),
        Err(error) => {
            println!("    server refuses instead of eliciting:");
            println!("      {error}");
            println!("    the client advertises only the capabilities it registered");
            println!("    handlers for, so this is caught in ONE round trip.");
            Ok(())
        },
    }
}
