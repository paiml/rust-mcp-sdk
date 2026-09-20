//! Example: an AUTONOMOUS 2026-07-28 (v2) AGENT that drives a task through a
//! pause and back out again.
//!
//! Start the paired SERVER first:
//! ```bash
//! cargo run --example s50_v2_tasks_server --features full
//! ```
//!
//! Then run this agent with:
//! ```bash
//! cargo run --example s51_v2_tasks_agent --features full
//! ```
//!
//! It takes the server address as `argv[1]` and defaults to `127.0.0.1:8150`,
//! which is where `s50` binds when it is given no address of its own. This is a
//! one-shot script: it exits 0 when every demonstration behaved as documented,
//! and NON-ZERO otherwise. Every `demo_*` below returns `Err` on a divergence and
//! `main` propagates it with `?`, so this file is an executable assertion rather
//! than a printout — a demonstration that printed "ok" regardless of what the
//! server did would teach the wrong contract.
//!
//! # What this demonstrates
//!
//! 1. **Explicit negotiation.** v2 has no `initialize`, so the agent asks
//!    `server/discover` EXPLICITLY and finds `io.modelcontextprotocol/tasks` in
//!    the server's `capabilities.extensions`. pmcp never probes for this on your
//!    behalf; once you have asked, the client enforces the capability locally on
//!    every later call.
//! 2. **The full autonomous round trip.** `tools/call` returns a task handle that
//!    is ALREADY paused; the agent polls, answers the server's `inputRequests`
//!    programmatically, delivers them with `tasks/update`, resumes polling, and
//!    reads the terminal `result` INLINE from the same `tasks/get` payload. That
//!    is this example's headline claim, executed rather than asserted.
//! 3. **The same exchange by hand.** An agent whose scheduler is not this
//!    process reaches for the pieces instead of the poller:
//!    `Client::tasks_get_detailed` for the paused task's `inputRequests` in one
//!    round trip, then `Client::tasks_update` to deliver the answers.
//! 4. **An undeclaring client gets no task handle.** On v2 the client's
//!    declaration IS the create trigger, so a client that did not declare
//!    receives an ordinary `CallToolResult` and is refused `-32021` if it tries
//!    `tasks/get` anyway.
//! 5. **The v2 retirements, from both sides.** `tasks/list` and `tasks/result`
//!    are gone. The pmcp client refuses them LOCALLY with zero bytes on the wire,
//!    and the server answers `-32601` to anyone who sends them anyway.
//!
//! # This is the AGENT shape, on purpose
//!
//! Every answer here is produced PROGRAMMATICALLY — nothing in this file reads
//! from standard input, and the assertion that it never will is a grep. That is
//! what makes it scriptable in CI, and it is the shape an autonomous agent uses:
//! an interactive host would prompt its user in the same callback and change
//! nothing else. Wiring this loop into the `pmcp-agent` crate proper is a
//! separate piece of work; what is de-risked here is the surface it will sit on.
//!
//! # A task is NOT a higher-trust channel
//!
//! The requests that arrive through `tasks/get`'s `inputRequests` are ordinary
//! elicitation / sampling / roots requests that happen to have travelled through
//! a task. The spec is explicit that hosts MUST apply the same trust model to
//! these payloads as they would to a standard elicitation or sampling request.
//! The responder below honours that: it reads the request's `message` for
//! display, answers from its OWN configuration, and executes nothing the server
//! supplied.
//!
//! # Everything here is a PRODUCTION client method
//!
//! There is no poll loop, no backoff and no wire decode written in this file.
//! `Client::wait_for_task_with_inputs` owns the loop, its poll-interval floor and
//! its bound on how many input rounds it will answer; `Client::tasks_update`
//! owns the delivery. An example that re-implemented any of them would drift
//! away from the SDK the moment the SDK changed, and would then teach a contract
//! the SDK no longer honours.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use pmcp::client::WaitForTaskOptions;
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use pmcp::shared::{StreamableHttpTransport, Transport, TransportMessage};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::content::Content;
use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::protocol::error_codes::{METHOD_NOT_FOUND, MISSING_REQUIRED_CLIENT_CAPABILITY};
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
use pmcp::types::tasks::{TaskDetailV2, TaskStatus, RELATED_TASK_META_KEY};
use pmcp::types::{InputRequest, InputRequests, InputResponse, InputResponses};
use pmcp::{Client, ClientBuilder, ToolCallResponse};
use serde_json::json;
use std::collections::HashMap;
use url::Url;

/// The task-capable tool `s50` exposes.
const TOOL_NAME: &str = "research";

/// The `inputRequests` key `s50` asks under, and the field its schema wants.
///
/// The key is SERVER-ASSIGNED; the agent reads it back off the request map
/// rather than assuming it, and this constant exists only to build the answer's
/// `content` object and to check the result reflects what was supplied.
const TOPIC_KEY: &str = "topic";

/// What this agent answers with. A real agent would compute it; the point is
/// that it is produced without asking a human, and without evaluating anything
/// the server sent.
const TOPIC_ANSWER: &str = "post-quantum key exchange";

/// Where `s50` binds when it is given no address.
const DEFAULT_ADDR: &str = "127.0.0.1:8150";

/// How long to poll a task before giving up, in seconds.
///
/// Generous enough to survive a slow machine, short enough that a genuinely
/// stuck server fails the run instead of hanging CI.
const POLL_BUDGET_SECS: u64 = 30;

/// How often to poll, in milliseconds. The SDK clamps this to a small floor, so
/// a zero here could not hot-spin the loop even if it were passed.
const POLL_INTERVAL_MS: u64 = 50;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string());
    let url = Url::parse(&format!("http://{addr}/"))?;

    println!();
    println!("=============================================================");
    println!("  v2 (2026-07-28) AUTONOMOUS TASKS AGENT  ->  http://{addr}");
    println!("=============================================================");

    demo_negotiation(&url).await?;
    demo_autonomous_task_round_trip(&url).await?;
    demo_manual_update_for_your_own_scheduler(&url).await?;
    demo_undeclared_client_is_refused(&url).await?;
    demo_retired_methods_are_gone_on_v2(&url).await?;

    println!();
    println!("=============================================================");
    println!("  All five demonstrations behaved as documented.");
    println!("=============================================================");
    Ok(())
}

/// A `Client` opted into 2026-07-28, optionally DECLARING the tasks extension.
///
/// A fresh transport per client keeps the five demonstrations independent.
///
/// `with_tasks_extension()` is the whole client-side opt-in. On v2 it is not a
/// preference: it is the server's create TRIGGER, so a client that omits it can
/// call the same tool all day and never be handed a task.
fn v2_client(url: &Url, declare_tasks: bool) -> pmcp::Result<Client<StreamableHttpTransport>> {
    let transport = StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(url.clone()).build(),
    );
    let builder = ClientBuilder::new(transport)
        .with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))?;
    Ok(if declare_tasks {
        builder.with_tasks_extension().build()
    } else {
        builder.build()
    })
}

/// 1. v2 has no handshake, so capabilities are learned by ASKING.
async fn demo_negotiation(url: &Url) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[1] Negotiation — explicit server/discover");
    println!("-------------------------------------------------------------");
    let mut client = v2_client(url, true)?;

    // EXPLICIT, and pmcp will never do it for you. Phase 113 forbids an implicit
    // capability probe outright, because a client that probes to decide which
    // protocol to speak is a client that can be steered into the wrong one.
    // Calling it STORES the projection, after which `assert_capability` enforces
    // on v2 exactly as strictly as it does on v1.
    let discovered = client.server_discover().await?;

    println!("    server        : {}", discovered.server_info.name);
    let Some(extensions) = discovered.capabilities.extensions.as_ref() else {
        return Err("a tasks-capable v2 server must publish capabilities.extensions".into());
    };
    if !extensions.contains_key(TASKS_EXTENSION_KEY) {
        return Err(format!(
            "server/discover carries no {TASKS_EXTENSION_KEY} entry; it advertises {:?}",
            extensions.keys().collect::<Vec<_>>()
        )
        .into());
    }
    println!(
        "    extensions    : {:?}",
        extensions.keys().collect::<Vec<_>>()
    );
    println!("    tasks negotiated — configuring a TaskStore is what advertises it.");
    Ok(())
}

/// 2. The headline claim: create -> `input_required` -> update -> terminal, with
///    no human in the loop.
async fn demo_autonomous_task_round_trip(
    url: &Url,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[2] Autonomous round trip — create, pause, update, terminal");
    println!("-------------------------------------------------------------");
    let mut client = v2_client(url, true)?;
    client.server_discover().await?;

    // ---- Create. An ordinary tools/call; the DECLARATION is what makes it a
    // ---- task, and the handle comes back on `resultType: "task"`.
    let task = match client
        .call_tool_with_task(TOOL_NAME.to_string(), json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => task,
        ToolCallResponse::Result(_) => {
            return Err(
                "a declaring v2 client must receive a task handle, not an ordinary result".into(),
            )
        },
    };
    println!("    created       : {} ({})", task.task_id, task.status);

    // The handle is ALREADY paused: the server recorded its input requests
    // against the store-minted id inside the very call that minted it, so there
    // is no window in which the task looks runnable and is not.
    if task.status != TaskStatus::InputRequired {
        return Err(format!(
            "the handle must arrive already paused, got status {}",
            task.status
        )
        .into());
    }

    // ---- Poll THROUGH the pause. One production call owns the whole loop. ----
    //
    // The counter is the load-bearing part of this demonstration: without it a
    // server that never paused, and a client that never answered, would pass
    // every other assertion below.
    let rounds = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&rounds);
    let result = client
        .wait_for_task_with_inputs(
            &task.task_id,
            poll_options(),
            move |requests: InputRequests| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    answer_all(&requests)
                }
            },
        )
        .await?;

    let rounds = rounds.load(Ordering::Relaxed);
    if rounds == 0 {
        return Err(
            "the task completed without ever asking for input — the pause never happened, \
                    so this run proves nothing about tasks/update"
                .into(),
        );
    }
    println!("    input rounds  : {rounds}");

    // ---- The terminal result arrived INLINE. ----
    //
    // On v2 there is no `tasks/result` to call: the payload the poll loop was
    // already fetching carries the result the moment the task completes.
    if result.is_error {
        return Err(format!(
            "the task completed with an error result: {:?}",
            result.content
        )
        .into());
    }
    let text = first_text(&result).ok_or("the terminal result must carry text content")?;
    println!("    result        : {text}");
    if !text.contains(TOPIC_ANSWER) {
        return Err(format!(
            "the terminal result must reflect the topic this agent supplied ({TOPIC_ANSWER:?}); \
             got {text:?}"
        )
        .into());
    }

    // A last read for the record, through the same production method the loop
    // used. On v2 this returns a `Task` with `ttl` / `pollInterval` already
    // mapped off the flat `ttlMs` / `pollIntervalMs` wire spelling.
    let settled = client.tasks_get(&task.task_id).await?;
    if settled.status != TaskStatus::Completed {
        return Err(format!("expected a completed task, got {}", settled.status).into());
    }
    println!(
        "    final status  : {} (ttl {:?} ms)",
        settled.status, settled.ttl
    );
    Ok(())
}

/// Answer every request the server asked, programmatically.
///
/// This is the entire responder contract: keys come from the server's map and
/// are echoed back UNCHANGED (they are how the server matches an answer to the
/// question it asked), and each value must be the shape the originating request
/// demanded — the server decodes it against the kind IT recorded and refuses a
/// mismatch, so a sampling-shaped answer to an elicitation cannot be smuggled
/// past it.
///
/// Nothing from the server is executed or evaluated here; the request's
/// `message` is read for display only.
fn answer_all(requests: &InputRequests) -> pmcp::Result<InputResponses> {
    let mut answers = InputResponses::new();
    for (key, request) in requests {
        let InputRequest::Elicitation(params) = request else {
            return Err(pmcp::Error::validation(format!(
                "this agent only knows how to answer elicitations, and {key:?} is not one"
            )));
        };
        if let ElicitRequestParams::Form { message, .. } = params.as_ref() {
            println!("    server asks   : {message}");
        }
        println!("    agent answers : {TOPIC_ANSWER}");

        let mut content = HashMap::new();
        content.insert(TOPIC_KEY.to_string(), json!(TOPIC_ANSWER));
        answers.insert(
            key.clone(),
            InputResponse::Elicitation(Box::new(ElicitResult {
                action: ElicitAction::Accept,
                content: Some(content),
            })),
        );
    }
    Ok(answers)
}

/// The poll budget both pollers run under.
///
/// One set of options for both demonstrations, because two independently-tuned
/// answers to "how long do I wait" is how they drift apart. The SDK clamps the
/// interval to its own floor, so this cannot hot-spin the loop.
fn poll_options() -> WaitForTaskOptions {
    WaitForTaskOptions {
        poll_interval: Some(POLL_INTERVAL_MS),
        max_poll_duration_secs: Some(POLL_BUDGET_SECS),
    }
}

/// The first text item of a result, if it has one.
fn first_text(result: &pmcp::types::CallToolResult) -> Option<&str> {
    result.content.iter().find_map(|content| match content {
        Content::Text { text } => Some(text.as_str()),
        _ => None,
    })
}

/// 3. The same exchange for an agent that owns its OWN scheduling.
///
/// [`Client::wait_for_task_with_inputs`] is the right answer when the agent can
/// afford to sit on the task. An agent that cannot — a Lambda woken by a queue
/// message, a long-lived worker multiplexing thousands of tasks, anything whose
/// scheduler is not this process — calls the same two production methods
/// directly instead:
///
/// - [`Client::tasks_get_detailed`] reads the task AND its status-conditional
///   detail in ONE round trip, so the `inputRequests` arrive with the status
///   that justifies them rather than from a second, possibly-disagreeing read.
/// - [`Client::tasks_update`] delivers the answers. Its acknowledgement is an
///   EMPTY object and claims nothing about the task's status — the task may
///   still be moving when the call returns.
///
/// There is still no loop written here. [`Client::wait_for_task`], the
/// responder-less sibling, takes over once the pause is answered — which is also
/// what makes this demonstration load-bearing: without the `tasks_update` above,
/// that call would return the input-required error instead of a result.
async fn demo_manual_update_for_your_own_scheduler(
    url: &Url,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[3] Manual update — tasks_get_detailed + tasks_update by hand");
    println!("-------------------------------------------------------------");
    let mut client = v2_client(url, true)?;
    client.server_discover().await?;

    let task = match client
        .call_tool_with_task(TOOL_NAME.to_string(), json!({}))
        .await?
    {
        ToolCallResponse::Task(task) => task,
        ToolCallResponse::Result(_) => {
            return Err("a declaring v2 client must receive a task handle".into())
        },
    };
    println!("    created       : {}", task.task_id);

    let detailed = client.tasks_get_detailed(&task.task_id).await?;
    let TaskDetailV2::InputRequired { input_requests } = detailed.detail() else {
        return Err(format!(
            "expected a paused task inlining its inputRequests, detail was {:?}",
            detailed.detail()
        )
        .into());
    };
    println!(
        "    outstanding   : {:?}",
        input_requests.keys().collect::<Vec<_>>()
    );

    client
        .tasks_update(&task.task_id, answer_all(input_requests)?)
        .await?;
    println!("    tasks/update  : delivered (the ack is an EMPTY object)");

    // No responder needed now — the pause has already been answered, so the
    // plain poller reaches the terminal result.
    let result = client
        .wait_for_task(&task.task_id, poll_options())
        .await
        .map_err(|error| {
            format!("wait_for_task must reach a terminal result after the update: {error}")
        })?;
    let text = first_text(&result).ok_or("the terminal result must carry text content")?;
    if !text.contains(TOPIC_ANSWER) {
        return Err(format!("the result must reflect the supplied topic; got {text:?}").into());
    }
    println!("    result        : {text}");
    Ok(())
}

/// 4. On v2 the client's DECLARATION is the create trigger, so an undeclaring
///    client is never handed a task handle.
async fn demo_undeclared_client_is_refused(
    url: &Url,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[4] Undeclared client — no declaration, no task");
    println!("-------------------------------------------------------------");
    let client = v2_client(url, false)?;

    // The SAME tools/call the declaring client made in [2].
    let response = client
        .call_tool_with_task(TOOL_NAME.to_string(), json!({}))
        .await?;
    match response {
        ToolCallResponse::Task(task) => {
            return Err(format!(
                "a non-declaring v2 client must NOT receive a task handle, but got {}",
                task.task_id
            )
            .into())
        },
        ToolCallResponse::Result(result) => {
            // The handle channel is `_meta.relatedTask`, and it must be absent.
            //
            // The tool's own value gets text-wrapped into this result, so its
            // FABRICATED `taskId` string does appear in the content. That string
            // is worthless by construction — no store ever minted it — which is
            // exactly the point `s50` makes at `DISCARDED_TASK_ID`. The
            // structural assertions are the meaningful ones.
            let related = result
                ._meta
                .as_ref()
                .and_then(|meta| meta.get(RELATED_TASK_META_KEY));
            if related.is_some() {
                return Err(format!(
                    "_meta.{RELATED_TASK_META_KEY} leaked a task handle to a \
                     non-declaring client: {related:?}"
                )
                .into());
            }
            println!("    ordinary CallToolResult, no _meta.{RELATED_TASK_META_KEY}");
        },
    }

    // And a direct tasks/* call is refused by the SERVER, naming the capability
    // that was missing. The id below was never minted by anything; the refusal
    // fires on the declaration check, before ownership is ever consulted, so no
    // id is being probed for existence here.
    let error = match client.tasks_get("never-minted").await {
        Ok(task) => {
            return Err(format!(
                "tasks/get must be refused for an undeclaring client, got task {}",
                task.task_id
            )
            .into())
        },
        Err(error) => error,
    };
    let code = protocol_code(&error).ok_or_else(|| {
        format!("expected a JSON-RPC protocol error from the server, got: {error}")
    })?;
    if code != MISSING_REQUIRED_CLIENT_CAPABILITY {
        return Err(format!(
            "expected {MISSING_REQUIRED_CLIENT_CAPABILITY} \
             (missing required client capability), got {code}: {error}"
        )
        .into());
    }
    println!("    tasks/get     : {code} — {error}");
    Ok(())
}

/// 5. `tasks/list` and `tasks/result` are RETIRED on 2026-07-28 — proven from
///    both ends.
///
/// The two halves answer different questions and neither substitutes for the
/// other. A pmcp client refuses LOCALLY, so it never spends a round trip on a
/// method it knows is gone; the SERVER still has to answer correctly, because
/// clients from other SDKs will send them. Asserting only the local half would
/// leave the server's answer untested by this example; asserting only the wire
/// half would leave a reader thinking they must handle a `-32601` they will
/// never see.
async fn demo_retired_methods_are_gone_on_v2(
    url: &Url,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("[5] Retirements — tasks/list and tasks/result are gone on v2");
    println!("-------------------------------------------------------------");
    let client = v2_client(url, true)?;

    // ---- Half one: the client refuses locally, with ZERO bytes sent. ----
    let listed = client.tasks_list(None).await;
    let error = listed
        .err()
        .ok_or("tasks/list must not succeed on a v2 connection")?;
    if !error.is_retired_on_v2() {
        return Err(format!("tasks/list must fail as retired-on-v2, got: {error}").into());
    }
    println!(
        "    tasks/list    : refused locally -> use {:?}",
        error.retired_replacement().unwrap_or("(unnamed)")
    );

    let resulted = client.tasks_result("never-minted").await;
    let error = resulted
        .err()
        .ok_or("tasks/result must not succeed on a v2 connection")?;
    if !error.is_retired_on_v2() {
        return Err(format!("tasks/result must fail as retired-on-v2, got: {error}").into());
    }
    println!(
        "    tasks/result  : refused locally -> use {:?}",
        error.retired_replacement().unwrap_or("(unnamed)")
    );

    // ---- Half two: the SERVER answers -32601 to anyone who sends them. ----
    for (method, params) in [
        ("tasks/list", json!({})),
        ("tasks/result", json!({ "taskId": "never-minted" })),
    ] {
        let code = server_answer_code(url, method, params).await?;
        if code != METHOD_NOT_FOUND {
            return Err(
                format!("{method} must answer {METHOD_NOT_FOUND} on v2, got {code}").into(),
            );
        }
        println!("    wire {method:<13}: {code} from the server");
    }
    Ok(())
}

/// Send ONE raw JSON-RPC frame at a v2 server and report the `error.code` it
/// answers with.
///
/// This deliberately bypasses `Client`, because `Client` is the thing being
/// documented: on v2 it refuses the two retired methods before a byte leaves the
/// process, so there is no client method that can observe the server's answer.
/// The transport still does all the v2 work — it derives the `Mcp-Method` /
/// `Mcp-Name` routing headers from the frame and surfaces the JSON-RPC error
/// envelope that rides the 404 — so this is a raw FRAME, not a hand-rolled
/// client.
async fn server_answer_code(
    url: &Url,
    method: &str,
    params: serde_json::Value,
) -> std::result::Result<i32, Box<dyn std::error::Error>> {
    let mut transport = StreamableHttpTransport::new(
        StreamableHttpTransportConfigBuilder::new(url.clone()).build(),
    );
    transport.set_negotiated_protocol_version(Some(PROTOCOL_VERSION_2026_07_28.to_string()));

    let mut params = params;
    // BOTH reserved keys: since Phase 118.1 (gap G-6) a v2 request whose
    // `_meta` omits `io.modelcontextprotocol/clientCapabilities` is refused with
    // `-32602` before dispatch, which would mask the `-32601` this probe exists
    // to observe. `clientInfo` stays out — it is a SHOULD.
    params["_meta"] = json!({
        pmcp::testing::META_PROTOCOL_VERSION: PROTOCOL_VERSION_2026_07_28,
        pmcp::testing::META_CLIENT_CAPABILITIES: {
            "elicitation": {}, "sampling": {}, "roots": {},
        },
    });
    let frame = json!({
        "jsonrpc": "2.0",
        "id": format!("retired-{method}"),
        "method": method,
        "params": params,
    });

    transport.send_raw(serde_json::to_vec(&frame)?).await?;
    match transport.receive().await? {
        TransportMessage::Response(response) => match response.payload {
            ResponsePayload::Error(error) => Ok(error.code),
            ResponsePayload::Result(_) => {
                Err(format!("{method} answered a SUCCESS on a 2026-07-28 connection").into())
            },
        },
        other => Err(format!("expected a response frame for {method}, got {other:?}").into()),
    }
}

/// The JSON-RPC `error.code` behind a [`pmcp::Error`], when it carries one.
fn protocol_code(error: &pmcp::Error) -> Option<i32> {
    match error {
        pmcp::Error::Protocol { code, .. } => Some(code.0),
        _ => None,
    }
}
