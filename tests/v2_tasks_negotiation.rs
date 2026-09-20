//! Phase 114-05 (TASK-01, D-01/D-02/D-03): **the tasks-extension negotiation
//! matrix, driven over a REAL loopback socket in both eras.**
//!
//! Six rows, one `#[tokio::test]` each, every test named for the row it proves:
//!
//! | # | row | era |
//! |---|-----|-----|
//! | 1 | a tasks-backed server advertises `extensions["io.modelcontextprotocol/tasks"] = {}` | v2 |
//! | 2 | the same response carries NEITHER v1 tasks spelling | v2 |
//! | 3 | an unrelated `experimental` key survives the projection | v2 |
//! | 4 | a server with no task backend advertises no extension entry | v2 |
//! | 5 | `initialize` is byte-identical to today, against a golden literal | v1 |
//! | 6 | an explicitly configured extension value is served verbatim | v2 |
//!
//! # Assertion discipline carried from `tests/v2_subscriptions.rs`
//!
//! * Every assertion carries a reason string NAMING the decision it protects, so
//!   a failure says which rule broke rather than which value differed.
//! * Absence is asserted as key ABSENCE (`.get(..).is_none()` / a raw-string
//!   check), never against `null`. Both `ServerCapabilities::tasks` and
//!   `::extensions` carry `skip_serializing_if = "Option::is_none"`, so a
//!   value-based assertion would accept the exact falsy shape a future change
//!   would emit.
//! * Presence of the extension is asserted as equality with `{}`, never
//!   `is_some()`. A presence-only assertion would pass on a change that started
//!   projecting `default_tasks_capability()`'s `list`/`cancel`/`requests` flags
//!   into the extension value — the capability lie D-03 forbids.
//! * Where a test has both halves, the non-vacuity half fires FIRST: an
//!   "`experimental.tasks` is absent" assertion is worthless if `experimental`
//!   itself is missing, so the map's existence is established before its
//!   contents are denied.
//! * Teardown is drop-sockets → `abort()` → `await`, through the shared
//!   [`teardown`] helper (D-113-T).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    long_task_tool, post, spawn_default_config, spawn_tasks_server, teardown, v1_body,
    v2_discover_body, v2_headers, AuthPosture, SearchTool, DISCOVER_EXTENSION_KEY, TASKS_TOOL_NAME,
    V1, V2,
};
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::Server;
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::ServerCapabilities;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;

// ===========================================================================
// Fixtures.
// ===========================================================================

/// A v2-opted-in, tasks-BACKED server with caller-chosen capabilities.
///
/// [`spawn_tasks_server`] is the shared harness fixture and is used wherever it
/// fits (tests 1 and 6's control). It pre-seeds an `extensions` map and no
/// `experimental` map, so the rows that need a specific pre-existing capability
/// shape build one here instead of forking the harness.
fn tasks_backed_server(capabilities: ServerCapabilities) -> Server {
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    Server::builder()
        .name("v2-tasks-negotiation")
        .version("1.0.0")
        .capabilities(capabilities)
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool(TASKS_TOOL_NAME, long_task_tool())
        .task_store(store)
        .build()
        .expect("tasks-backed server builds")
}

/// A v2-opted-in server with NO task backend at all.
///
/// It registers the plain `search` tool rather than [`long_task_tool`]: the
/// latter declares `TaskSupport::Required`, which with no backend is a
/// BUILD-TIME validation error — the endpoint-backed rule's other half.
fn backendless_server(capabilities: ServerCapabilities) -> Server {
    Server::builder()
        .name("v2-no-tasks-negotiation")
        .version("1.0.0")
        .capabilities(capabilities)
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool("search", SearchTool)
        .build()
        .expect("backend-less server builds")
}

/// `ServerCapabilities` carrying an `experimental` map with the given entries.
fn experimental(entries: &[(&str, Value)]) -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    let mut map = HashMap::new();
    for (key, value) in entries {
        map.insert((*key).to_string(), value.clone());
    }
    caps.experimental = Some(map);
    caps
}

/// `ServerCapabilities` carrying an `extensions` map with the given entries.
fn extensions(entries: &[(&str, Value)]) -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    let mut map = HashMap::new();
    for (key, value) in entries {
        map.insert((*key).to_string(), value.clone());
    }
    caps.extensions = Some(map);
    caps
}

/// Drive a real v2 `server/discover` and return `result.capabilities`.
///
/// `server/discover` carries no logical name, so `Mcp-Name` is the EMPTY string
/// — the locked cross-plan header rule, emitted by the shared harness.
async fn discover_capabilities(addr: SocketAddr, id: i64) -> Value {
    let response = post(
        addr,
        &v2_headers("server/discover", ""),
        &v2_discover_body(json!(id)),
    )
    .await;
    assert_eq!(
        response.status, 200,
        "a v2 server/discover must be served: {}",
        response.raw
    );
    response
        .body
        .get("result")
        .and_then(|result| result.get("capabilities"))
        .cloned()
        .unwrap_or_else(|| panic!("discover must carry result.capabilities: {}", response.raw))
}

async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(server).await
}

// ===========================================================================
// Row 1 — the advertisement.
// ===========================================================================

/// A v2 `server/discover` against a tasks-backed server advertises the tasks
/// extension, and its value is EXACTLY the empty object.
///
/// This is TASK-01's server half observed end to end: one knob (`has_backend`)
/// drives the entry through the same shared `apply_tasks_capability_rule` that
/// already drives `capabilities.tasks`, so an existing tasks server needs no
/// code change to be discoverable by a v2 client.
#[tokio::test]
async fn v2_tasks_extension_advertised() {
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;
    let capabilities = discover_capabilities(addr, 1).await;
    teardown(handle, ()).await;

    assert_eq!(
        capabilities["extensions"][TASKS_EXTENSION_KEY],
        json!({}),
        "the tasks extension must be advertised as the EMPTY OBJECT. Equality \
         rather than presence is the point: D-03 forbids projecting \
         default_tasks_capability()'s list/cancel/requests flags in here, \
         because advertising list:true on an era where tasks/list answers \
         -32601 is exactly the capability lie the endpoint-backed rule \
         prevents: {capabilities}"
    );
    assert_eq!(
        capabilities["extensions"][DISCOVER_EXTENSION_KEY]["enabled"],
        json!(true),
        "and it lands ALONGSIDE the server's pre-existing extensions key, never \
         replacing the map: {capabilities}"
    );
}

// ===========================================================================
// Row 2 — the suppression.
// ===========================================================================

/// The same v2 response carries NEITHER v1 tasks spelling: no
/// `capabilities.tasks`, no `capabilities.experimental.tasks`.
///
/// The server is built with an `experimental.tasks` flag ON PURPOSE. The shared
/// harness fixture has no `experimental` map at all, so asserting the key's
/// absence against it would be vacuous — it would pass against a projection that
/// suppressed nothing.
#[tokio::test]
async fn v2_discover_omits_the_v1_tasks_keys() {
    let (addr, handle) = spawn(tasks_backed_server(experimental(&[
        ("tasks", json!({ "legacy": true })),
        ("io.example/flag", json!(true)),
    ])))
    .await;
    let capabilities = discover_capabilities(addr, 2).await;
    teardown(handle, ()).await;

    // Non-vacuity first: the map this test denies a key inside must exist.
    assert!(
        capabilities["experimental"].is_object(),
        "the experimental map must survive the projection, or the absence \
         assertion below proves nothing: {capabilities}"
    );
    assert!(
        capabilities.get("tasks").is_none(),
        "the v1 `tasks` capability must be ABSENT from a v2 discover — a v2 \
         client negotiates tasks through extensions, and a v1 spelling on a v2 \
         wire points at a negotiation home that does not exist there. Absence, \
         not null: `skip_serializing_if` keeps None off the wire, so a \
         value-based check would accept an explicit null: {capabilities}"
    );
    assert!(
        capabilities["experimental"].get("tasks").is_none(),
        "the v1 `experimental.tasks` flag must be ABSENT from a v2 discover for \
         the same reason: {capabilities}"
    );
}

// ===========================================================================
// Row 3 — the scope of the suppression.
// ===========================================================================

/// A NON-tasks `experimental` key survives the v2 projection untouched.
///
/// D-02 explicitly rejected suppressing the whole `experimental` block: this
/// phase owns exactly one key inside it. Without this row, "remove
/// `experimental.tasks`" and "remove `experimental`" are indistinguishable.
#[tokio::test]
async fn v2_discover_preserves_an_unrelated_experimental_key() {
    let (addr, handle) = spawn(tasks_backed_server(experimental(&[(
        "io.example/flag",
        json!({ "enabled": true }),
    )])))
    .await;
    let capabilities = discover_capabilities(addr, 3).await;
    teardown(handle, ()).await;

    assert_eq!(
        capabilities["experimental"]["io.example/flag"],
        json!({ "enabled": true }),
        "an experimental key this phase does not own must survive the v2 \
         projection verbatim: {capabilities}"
    );
}

// ===========================================================================
// Row 4 — the endpoint-backed half.
// ===========================================================================

/// A v2 server with NEITHER a store NOR a router advertises no tasks extension.
///
/// Presence of the key is a PROMISE that `tasks/*` works. A server that cannot
/// serve them must make no such promise (T-114-18), which is why the rule is
/// endpoint-backed rather than driven by tool metadata.
#[tokio::test]
async fn v2_no_backend_advertises_no_tasks_extension() {
    let (addr, handle) = spawn(backendless_server(extensions(&[(
        DISCOVER_EXTENSION_KEY,
        json!({ "enabled": true }),
    )])))
    .await;
    let capabilities = discover_capabilities(addr, 4).await;
    teardown(handle, ()).await;

    // Non-vacuity first: this server DOES have an extensions map, so the
    // absence below is about the tasks key specifically and not about a map
    // that was never emitted.
    assert_eq!(
        capabilities["extensions"][DISCOVER_EXTENSION_KEY]["enabled"],
        json!(true),
        "the server's own extensions map must be projected: {capabilities}"
    );
    assert!(
        capabilities["extensions"]
            .get(TASKS_EXTENSION_KEY)
            .is_none(),
        "a server with no task backend must advertise NO tasks extension entry: \
         the entry is a promise that tasks/* works: {capabilities}"
    );
}

// ===========================================================================
// Row 5 — the v1 lock.
// ===========================================================================

/// A v1 `initialize` against a tasks-backed server is byte-identical to today,
/// compared against a full inline golden literal.
///
/// # Read this before changing the literal
///
/// A diff here is a **v1 WIRE BREAK**, not a fixture that drifted — the same
/// rule `tests/v1_tasks_golden.rs` states for the `tasks/*` bodies. D-02's
/// promise is that the capability STRUCT may carry everything both eras want
/// while the SERIALIZATION boundary decides what each era sees; this row is that
/// promise measured from the v1 side.
///
/// The fixture deliberately configures NO `extensions` map, so the raw-string
/// check below is meaningful: any `extensions` key in the response text can only
/// have come from this phase's auto-advertisement leaking onto the v1 wire.
#[tokio::test]
async fn v1_initialize_stays_byte_identical() {
    let (addr, handle) = spawn(tasks_backed_server(ServerCapabilities::default())).await;
    let response = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(5),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "v1-client", "version": "0.0.0" }
            }),
        ),
    )
    .await;
    teardown(handle, ()).await;

    assert_eq!(
        response.status, 200,
        "a v1 initialize must be served: {}",
        response.raw
    );
    assert!(
        !response.raw.contains("extensions"),
        "a v1 initialize response must carry NO extensions key. The tasks \
         extension is the v2 negotiation home; a v1 client negotiates through \
         capabilities.tasks, and adding a key here would move the initialize \
         bytes of every tasks server that exists today (D-02, T-114-16): {}",
        response.raw
    );

    let expected = json!({
        "protocolVersion": V1,
        "capabilities": {
            "tools": { "listChanged": false },
            "tasks": {
                "list": {},
                "cancel": {},
                "requests": { "tools": { "call": {} } }
            }
        },
        "serverInfo": { "name": "v2-tasks-negotiation", "version": "1.0.0" }
    });
    assert_eq!(
        response.body["result"], expected,
        "the v1 initialize result drifted from its golden literal; \
         capabilities.tasks must still be present exactly as before: {}",
        response.raw
    );
}

// ===========================================================================
// Row 6 — the escape hatch, named as such.
// ===========================================================================

/// An explicitly configured tasks-extension value is served VERBATIM on v2.
///
/// # This is a NONCONFORMANT escape hatch, not a supported extension shape
///
/// The vendored draft schema types this capability as `Record<string, never>`
/// (D-03) — a value admitting no properties at all — so the non-empty value this
/// test configures is **not conformant**, and a client is entitled to ignore or
/// reject it. The additive-only rule preserves it deliberately anyway: an
/// explicitly configured `extensions` value is the OPERATOR's, and silently
/// rewriting an operator's configuration is worse than serving something the
/// operator chose. Classify this row as *a nonconformant deployment opt-in*, not
/// as evidence that the extension has a settings shape.
///
/// The second half is what keeps the first half from being read as permission:
/// pmcp itself NEVER auto-populates a non-empty value. The auto-advertise path
/// (D-01) always writes `{}`, so the only route to a non-empty value on the wire
/// is an explicit operator call.
#[tokio::test]
async fn an_explicitly_configured_tasks_extension_value_is_served_verbatim_as_a_nonconformant_escape_hatch(
) {
    let configured = json!({ "io.example/nonconformant": true });
    let (addr, handle) = spawn(tasks_backed_server(extensions(&[(
        TASKS_EXTENSION_KEY,
        configured.clone(),
    )])))
    .await;
    let capabilities = discover_capabilities(addr, 6).await;
    teardown(handle, ()).await;

    assert_eq!(
        capabilities["extensions"][TASKS_EXTENSION_KEY], configured,
        "an explicitly configured extension value must reach the wire \
         unchanged: the additive-only rule never overwrites an operator's own \
         value: {capabilities}"
    );

    // The control: the SAME code path with nothing configured writes `{}`.
    let (addr, handle) = spawn_tasks_server(AuthPosture::None).await;
    let auto = discover_capabilities(addr, 7).await;
    teardown(handle, ()).await;

    assert_eq!(
        auto["extensions"][TASKS_EXTENSION_KEY],
        json!({}),
        "pmcp's OWN auto-advertisement is always the empty object, so a \
         non-empty value on a pmcp wire is always an operator opt-in and never \
         something the SDK minted: {auto}"
    );
}
