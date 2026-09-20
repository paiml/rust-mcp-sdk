//! Structured tool output bridge acceptance gate.
//!
//! Per the MCP spec, a tool that declares an `outputSchema` SHOULD return
//! `structuredContent` conforming to it. Before this gate, the SDK had both
//! halves of the vocabulary and no bridge: `ToolInfo::output_schema` was
//! published in `tools/list`, but the Payload-path dispatchers stringified
//! every result into `content[0].text` and only set `structured_content` for
//! widget tools.
//!
//! These tests prove:
//! 1. `CallToolResult::structured` / `structured_with_text` — the success-side
//!    counterparts of `CallToolResult::rejected`: one value, one call, both
//!    voices (text for text-only clients, `structuredContent` for
//!    structured-aware clients).
//! 2. Both native dispatchers (high-level `Server` over an in-process duplex
//!    transport, and `ServerCore` via a server pump) auto-emit
//!    `structuredContent` for Payload-path tools whose cached `ToolInfo`
//!    declares an `output_schema` (e.g. `TypedToolWithOutput`), round-tripping
//!    the exact handler value in BOTH voices.
//! 3. Tools without a declared `output_schema` keep today's text-only
//!    envelope on both dispatchers (no behavior change for existing code).
//!
//! Phase 115 plan 04 (SCHM-02) added a fourth claim, in the `era_aware` module
//! at the bottom:
//!
//! 4. A scalar, an array or `null` survives as `structuredContent` through BOTH
//!    dispatchers on `Era::V2` — with the era MEASURED in-band via the
//!    `resultType` witness rather than assumed — and the v1 path is proven
//!    unchanged by contrast. There is no `is_object()` guard anywhere on this
//!    path to remove: 115-RESEARCH § Finding 6 measured that the "object-only
//!    bridge" lives in v1's spec TEXT, never in pmcp's code.

#![cfg(all(not(target_arch = "wasm32"), feature = "schema-generation"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::sync::Arc;

use duplex::{call_via_core, call_via_server};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::typed_tool::{TypedTool, TypedToolWithOutput};
use pmcp::types::{CallToolResult, Content};
use pmcp::{Server, ToolHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Extract the single text block from a result's content.
fn text_of(result: &CallToolResult) -> &str {
    match result
        .content
        .first()
        .expect("result carries at least one content block")
    {
        Content::Text { text } => text,
        other => panic!("expected text content, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Typed fixture tool: declares an outputSchema via TypedToolWithOutput.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct ProposeArgs {
    /// Corpus to propose a schema for.
    corpus: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ProposedSchema {
    /// Entity type names discovered in the corpus.
    entities: Vec<String>,
    /// Total entity count.
    count: u32,
}

fn propose_schema_tool() -> impl ToolHandler {
    TypedToolWithOutput::new("propose_schema", |args: ProposeArgs, _extra| {
        Box::pin(async move {
            let _ = args.corpus;
            Ok(ProposedSchema {
                entities: vec!["Person".to_string(), "Company".to_string()],
                count: 2,
            })
        })
    })
    .with_description("Propose a graph schema for a corpus")
}

/// The exact JSON value `propose_schema_tool` emits, for round-trip asserts.
fn expected_proposed_schema() -> Value {
    json!({ "entities": ["Person", "Company"], "count": 2 })
}

/// A schema-less tool (input typing only) — the regression control.
fn text_only_tool() -> impl ToolHandler {
    TypedTool::new("text_only", |args: ProposeArgs, _extra| {
        Box::pin(async move {
            let _ = args.corpus;
            Ok(json!({ "plain": true }))
        })
    })
}

// ---------------------------------------------------------------------------
// 1. Constructors: structured / structured_with_text.
// ---------------------------------------------------------------------------

#[test]
fn structured_dual_emits_one_value_in_both_voices() {
    let value = expected_proposed_schema();
    let result = CallToolResult::structured(value.clone());

    assert!(!result.is_error, "structured() is a success result");
    assert_eq!(
        result.structured_content,
        Some(value.clone()),
        "structuredContent carries the value verbatim"
    );
    let parsed: Value =
        serde_json::from_str(text_of(&result)).expect("text voice is valid JSON of the value");
    assert_eq!(parsed, value, "text voice round-trips to the same value");
}

/// The D-06 widening sibling: a non-object payload is a deliberate, greppable
/// choice at the call site, and both voices still carry the same value.
#[test]
fn structured_value_accepts_a_scalar_and_serializes_the_text_voice() {
    let result = CallToolResult::structured_value(json!(42));

    assert!(!result.is_error, "structured_value() is a success result");
    assert_eq!(
        result.structured_content,
        Some(json!(42)),
        "a scalar reaches structuredContent verbatim"
    );
    assert_eq!(
        text_of(&result),
        "42",
        "the text voice is the canonical serialization of the same scalar"
    );
}

/// MEASURED FINDING (Phase 115 plan 04) — an emitted `structuredContent: null`
/// is NOT recoverable through `CallToolResult`'s own `Deserialize` impl.
///
/// The SERVER side is correct and v2-conformant: `skip_serializing_if =
/// "Option::is_none"` omits the key for `None` and emits an explicit `null` for
/// `Some(Value::Null)`, which both dispatcher tests below assert on the raw wire.
/// The collapse happens on the way BACK IN: serde's default `Option<T>`
/// deserializer maps a JSON `null` to `None`, so a client re-reading the result
/// into `CallToolResult` cannot distinguish "structured content is null" from
/// "there is no structured content".
///
/// Left as-is deliberately. It is PRE-EXISTING (the field has always been
/// `Option<Value>` with default serde semantics), it is not a wire defect, and
/// changing it would alter the client-side meaning of every `CallToolResult` on
/// BOTH eras — a decision for the phase, not for an execution plan. Booked as a
/// deferred item for 115-10.
///
/// This test is the tripwire: if the deserialization is ever made
/// null-preserving, it fails and forces the change to be acknowledged here.
#[test]
fn present_null_structured_content_does_not_survive_a_typed_reread() {
    let result = CallToolResult::structured_value(json!(null));
    assert_eq!(
        result.structured_content,
        Some(Value::Null),
        "the constructed value carries a present null"
    );

    let wire = serde_json::to_string(&result).expect("result serializes");
    assert!(
        wire.contains(r#""structuredContent":null"#),
        "the wire keeps the present null: {wire}"
    );

    let reread: CallToolResult = serde_json::from_str(&wire).expect("result deserializes");
    assert_eq!(
        reread.structured_content, None,
        "FINDING: serde maps a JSON null onto Option::None, so the present-null is lost on a \
         typed re-read even though the wire carried it"
    );
}

/// The regression fence for D-06's "the signature stays" promise:
/// `CallToolResult::structured` keeps its object-shaped intent and its exact
/// behaviour. If this fails, the widening landed on `structured` instead of on
/// its sibling and every existing call site's compile-time signal was lost.
#[test]
fn structured_keeps_its_object_shaped_intent() {
    let value = json!({ "a": 1 });
    let result = CallToolResult::structured(value.clone());

    assert!(!result.is_error);
    assert_eq!(result.structured_content, Some(value.clone()));
    let parsed: Value = serde_json::from_str(text_of(&result)).expect("text voice is JSON");
    assert_eq!(parsed, value, "text voice round-trips to the same object");
}

#[test]
fn structured_with_text_separates_the_two_voices() {
    let value = expected_proposed_schema();
    let result = CallToolResult::structured_with_text(value.clone(), "Proposed 2 entity types.");

    assert!(!result.is_error);
    assert_eq!(result.structured_content, Some(value));
    assert_eq!(
        text_of(&result),
        "Proposed 2 entity types.",
        "human voice differs from the raw serialization"
    );
}

// ---------------------------------------------------------------------------
// 2. Dispatcher auto-emit: declared outputSchema => structuredContent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_auto_emits_structured_content_for_declared_output_schema() {
    let server = Server::builder()
        .name("structured-output-server")
        .version("1.0.0")
        .tool("propose_schema", propose_schema_tool())
        .build()
        .expect("server builds");

    let result = call_via_server(server, "propose_schema", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content,
        Some(expected_proposed_schema()),
        "high-level Server bridges declared outputSchema to structuredContent"
    );
    let parsed: Value = serde_json::from_str(text_of(&result)).expect("text voice is JSON");
    assert_eq!(
        parsed,
        expected_proposed_schema(),
        "text voice still round-trips for text-only clients"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_core_auto_emits_structured_content_for_declared_output_schema() {
    let core: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("structured-output-core")
            .version("1.0.0")
            .tool("propose_schema", propose_schema_tool())
            .build()
            .expect("core builds"),
    );

    let result = call_via_core(core, "propose_schema", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content,
        Some(expected_proposed_schema()),
        "ServerCore bridges declared outputSchema to structuredContent"
    );
    let parsed: Value = serde_json::from_str(text_of(&result)).expect("text voice is JSON");
    assert_eq!(parsed, expected_proposed_schema());
}

// ---------------------------------------------------------------------------
// 3. Regression: no declared outputSchema => text-only envelope, both paths.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_keeps_text_only_envelope_without_output_schema() {
    let server = Server::builder()
        .name("text-only-server")
        .version("1.0.0")
        .tool("text_only", text_only_tool())
        .build()
        .expect("server builds");

    let result = call_via_server(server, "text_only", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content, None,
        "no declared outputSchema: high-level Server emits text only"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_core_keeps_text_only_envelope_without_output_schema() {
    let core: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("text-only-core")
            .version("1.0.0")
            .tool("text_only", text_only_tool())
            .build()
            .expect("core builds"),
    );

    let result = call_via_core(core, "text_only", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content, None,
        "no declared outputSchema: ServerCore emits text only"
    );
}

// ---------------------------------------------------------------------------
// 4. Non-object structuredContent, BOTH dispatchers, BOTH eras (SCHM-02).
// ---------------------------------------------------------------------------

/// Era-aware dispatcher coverage for SCHM-02.
///
/// # What makes these tests non-vacuous
///
/// Every v2 test here does three things the pre-review design did not:
/// 1. builds its server WITH `.with_supported_protocol_versions(v2_accept_list())`
///    — without the opt-in, `resolve_ingress_protocol_context` returns `Ok(None)`
///    before it ever reads `_meta` (D-04) and the request is served as v1;
/// 2. sends a request whose `params._meta` carries the reserved
///    protocol-version key, built through pmcp's own `RequestMeta`;
/// 3. asserts `assert_v2_witness` FIRST — the `resultType` key that
///    `inject_v2_result_envelope` adds ONLY on `Era::V2` — so the era is
///    measured in-band instead of assumed.
///
/// `structured_output_the_v2_witness_is_load_bearing` proves step 3 actually
/// discriminates.
///
/// Gated on `testing` because the reserved `_meta` key comes from
/// `pmcp::testing::META_PROTOCOL_VERSION` rather than a hardcoded string;
/// `testing` is folded into `full`, which every Phase 115 test command uses.
#[cfg(feature = "testing")]
mod era_aware {
    use super::duplex::{
        assert_no_v2_witness, assert_v2_witness, call_tool_request, call_tool_result_of,
        initialize_via_core, raw_via_core, raw_via_server, result_object, v2_accept_list,
    };
    use super::{json, Arc, ProtocolHandler, Server, ServerCoreBuilder, ToolHandler, Value};
    use pmcp::server::typed_tool::TypedToolWithOutput;
    use pmcp::types::protocol::Era;

    // -----------------------------------------------------------------------
    // Fixtures: tools whose declared outputSchema DESCRIBES a non-object value.
    // -----------------------------------------------------------------------

    /// Every fixture registers under one name, so the request builders below
    /// need no per-test plumbing.
    const TOOL: &str = "non_object_output";

    /// A tool that returns `value` verbatim and declares `output_schema`.
    fn value_tool(name: &str, output_schema: Value, value: Value) -> impl ToolHandler {
        TypedToolWithOutput::new_with_schemas(
            name.to_string(),
            json!({ "type": "object" }),
            Some(output_schema),
            move |_args: Value, _extra| {
                let value = value.clone();
                Box::pin(async move { Ok(value) })
            },
        )
    }

    /// Returns `42`, declares `{"type": "integer"}` — the schema DESCRIBES the
    /// scalar, which is what D-04 requires of a non-object payload.
    fn scalar_int_tool() -> impl ToolHandler {
        value_tool(TOOL, json!({ "type": "integer" }), json!(42))
    }

    /// Returns `["a", "b"]`, declares a matching array schema.
    fn array_tool() -> impl ToolHandler {
        value_tool(
            TOOL,
            json!({ "type": "array", "items": { "type": "string" } }),
            json!(["a", "b"]),
        )
    }

    /// Returns `null`, declares `{"type": "null"}`.
    fn null_tool() -> impl ToolHandler {
        value_tool(TOOL, json!({ "type": "null" }), json!(null))
    }

    /// Returns `42` but declares an OBJECT schema — the D-04 mismatch case.
    fn mismatched_object_schema_tool() -> impl ToolHandler {
        value_tool(
            TOOL,
            json!({
                "type": "object",
                "properties": { "n": { "type": "integer" } },
                "required": ["n"],
            }),
            json!(42),
        )
    }

    /// The empty arguments object every fixture accepts.
    fn no_args() -> Value {
        json!({})
    }

    // -----------------------------------------------------------------------
    // Fixture servers. The v2 pair opts in; the v1 pair deliberately does not,
    // so the two eras differ by CONFIGURATION as well as by request shape.
    // -----------------------------------------------------------------------

    fn v2_server(tool: impl ToolHandler + 'static) -> Server {
        Server::builder()
            .name("structured-output-v2-server")
            .version("1.0.0")
            .tool(TOOL, tool)
            .with_supported_protocol_versions(v2_accept_list())
            .build()
            .expect("v2 server builds")
    }

    fn v1_server(tool: impl ToolHandler + 'static) -> Server {
        Server::builder()
            .name("structured-output-v1-server")
            .version("1.0.0")
            .tool(TOOL, tool)
            .build()
            .expect("v1 server builds")
    }

    fn v2_core(tool: impl ToolHandler + 'static) -> Arc<dyn ProtocolHandler> {
        Arc::new(
            ServerCoreBuilder::new()
                .name("structured-output-v2-core")
                .version("1.0.0")
                .tool(TOOL, tool)
                .with_supported_protocol_versions(v2_accept_list())
                .build()
                .expect("v2 core builds"),
        )
    }

    fn v1_core(tool: impl ToolHandler + 'static) -> Arc<dyn ProtocolHandler> {
        Arc::new(
            ServerCoreBuilder::new()
                .name("structured-output-v1-core")
                .version("1.0.0")
                .tool(TOOL, tool)
                .build()
                .expect("v1 core builds"),
        )
    }

    // -----------------------------------------------------------------------
    // Scalar.
    // -----------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_v2_scalar_structured_content_survives_round_trip() {
        let response = raw_via_server(
            v2_server(scalar_int_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "Server / v2 scalar structuredContent");
        assert_eq!(
            call_tool_result_of(&response).structured_content,
            Some(json!(42)),
            "the high-level Server hands a scalar through to structuredContent"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_core_v2_scalar_structured_content_survives_round_trip() {
        let response = raw_via_core(
            v2_core(scalar_int_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "ServerCore / v2 scalar structuredContent");
        assert_eq!(
            call_tool_result_of(&response).structured_content,
            Some(json!(42)),
            "ServerCore hands a scalar through to structuredContent"
        );
    }

    // -----------------------------------------------------------------------
    // Array.
    // -----------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_v2_array_structured_content_survives_round_trip() {
        let response = raw_via_server(
            v2_server(array_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "Server / v2 array structuredContent");
        assert_eq!(
            call_tool_result_of(&response).structured_content,
            Some(json!(["a", "b"])),
            "the high-level Server hands an array through to structuredContent"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_core_v2_array_structured_content_survives_round_trip() {
        let response = raw_via_core(
            v2_core(array_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "ServerCore / v2 array structuredContent");
        assert_eq!(
            call_tool_result_of(&response).structured_content,
            Some(json!(["a", "b"])),
            "ServerCore hands an array through to structuredContent"
        );
    }

    // -----------------------------------------------------------------------
    // Null — a PRESENT null is not an omitted field. This is the distinction
    // `skip_serializing_if = "Option::is_none"` makes observable, and the one a
    // shape check would most easily get wrong.
    //
    // The assertion is on the RAW result object rather than on a re-read
    // `CallToolResult`, because `Map::get` is where "present-null vs absent" is
    // actually expressible: a skipped key gives `None`, an emitted null gives
    // `Some(&Value::Null)`. A typed re-read cannot express it — see
    // `present_null_structured_content_does_not_survive_a_typed_reread` at the
    // top of this file for that measured finding.
    // -----------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_v2_null_structured_content_is_present_not_omitted() {
        let response = raw_via_server(
            v2_server(null_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "Server / v2 null structuredContent");
        assert_eq!(
            result_object(&response).get("structuredContent"),
            Some(&Value::Null),
            "a null payload is PRESENT as an explicit null, NOT an omitted key"
        );
        let wire = serde_json::to_string(&response).expect("response serializes");
        assert!(
            wire.contains(r#""structuredContent":null"#),
            "the key must reach the wire with an explicit null, not be skipped: {wire}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_core_v2_null_structured_content_is_present_not_omitted() {
        let response = raw_via_core(
            v2_core(null_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "ServerCore / v2 null structuredContent");
        assert_eq!(
            result_object(&response).get("structuredContent"),
            Some(&Value::Null),
            "a null payload is PRESENT as an explicit null, NOT an omitted key"
        );
        let wire = serde_json::to_string(&response).expect("response serializes");
        assert!(
            wire.contains(r#""structuredContent":null"#),
            "the key must reach the wire with an explicit null, not be skipped: {wire}"
        );
    }

    // -----------------------------------------------------------------------
    // D-04 at the dispatcher level: an object-shaped outputSchema against a
    // scalar payload is a MISMATCH, and a mismatch is warn-only.
    // -----------------------------------------------------------------------

    /// The schema mismatch is reported as a `tracing` warning by
    /// `src/server/output_validation.rs` and deliberately does NOT fail the
    /// call — the module's house style is "never an error result without adding
    /// a production failure mode" (T-115-15). This test proves the DISPATCHER
    /// still returns a result; `output_validation.rs`'s own unit tests
    /// (115-03 Task 3) carry the verdict-level assertions.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_v2_object_schema_with_scalar_payload_still_returns_a_result() {
        let response = raw_via_server(
            v2_server(mismatched_object_schema_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "Server / v2 schema mismatch");
        let result = call_tool_result_of(&response);
        assert!(
            !result.is_error,
            "a schema mismatch is warn-only, not an error result"
        );
        assert_eq!(
            result.structured_content,
            Some(json!(42)),
            "the value still reaches the wire verbatim"
        );
    }

    /// The `ServerCore` twin of
    /// [`server_v2_object_schema_with_scalar_payload_still_returns_a_result`].
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_core_v2_object_schema_with_scalar_payload_still_returns_a_result() {
        let response = raw_via_core(
            v2_core(mismatched_object_schema_tool()),
            call_tool_request(TOOL, no_args(), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "ServerCore / v2 schema mismatch");
        let result = call_tool_result_of(&response);
        assert!(
            !result.is_error,
            "a schema mismatch is warn-only, not an error result"
        );
        assert_eq!(
            result.structured_content,
            Some(json!(42)),
            "the value still reaches the wire verbatim"
        );
    }

    // -----------------------------------------------------------------------
    // The FROZEN half (D-05): v1 is proven unchanged BY CONTRAST, which is what
    // makes the v2 assertions above mean anything.
    // -----------------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_v1_scalar_structured_content_is_unchanged() {
        let response = raw_via_server(
            v1_server(scalar_int_tool()),
            call_tool_request(TOOL, no_args(), Era::V1),
        )
        .await;

        assert_no_v2_witness(&response, "Server / v1 scalar structuredContent");
        assert_eq!(
            call_tool_result_of(&response).structured_content,
            Some(json!(42)),
            "pmcp ALREADY emits a scalar on v1 — more permissive than v1's spec text, and D-05 \
             FREEZES that rather than correcting it"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_core_v1_scalar_structured_content_is_unchanged() {
        let core = v1_core(scalar_int_tool());
        // MEASURED: ServerCore gates a v1 request behind the `initialize`
        // handshake (`v1_initialize_gate_applies`), unlike the v2 requests
        // above, which need none. See `duplex::initialize_via_core`.
        initialize_via_core(&core).await;
        let response = raw_via_core(core, call_tool_request(TOOL, no_args(), Era::V1)).await;

        assert_no_v2_witness(&response, "ServerCore / v1 scalar structuredContent");
        assert_eq!(
            call_tool_result_of(&response).structured_content,
            Some(json!(42)),
            "the v1 ServerCore path is unchanged, scalar included"
        );
    }

    // -----------------------------------------------------------------------
    // Anti-vacuity: prove the witness itself discriminates.
    // -----------------------------------------------------------------------

    /// Run the SAME `Era::V2` request through two cores that differ ONLY in
    /// whether they opted into the v2 accept-list.
    ///
    /// Without this, every "v2" assertion in this file could be silently running
    /// as v1 and nothing would say so — which is exactly the defect the cross-AI
    /// review found in the pre-review plan.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn structured_output_the_v2_witness_is_load_bearing() {
        let opted_in = v2_core(scalar_int_tool());
        let response = raw_via_core(opted_in, call_tool_request(TOOL, no_args(), Era::V2)).await;
        assert_v2_witness(&response, "opted-in core, Era::V2 request");

        // The SAME request against a core that never opted in:
        // `resolve_ingress_protocol_context` short-circuits to `Ok(None)` before
        // it reads `_meta` at all (D-04), so this is served as v1 — and being
        // v1, it needs the handshake first.
        let not_opted_in = v1_core(scalar_int_tool());
        initialize_via_core(&not_opted_in).await;
        let response =
            raw_via_core(not_opted_in, call_tool_request(TOOL, no_args(), Era::V2)).await;
        assert_no_v2_witness(&response, "non-opted-in core, identical Era::V2 request");
        assert_eq!(
            call_tool_result_of(&response).structured_content,
            Some(json!(42)),
            "the payload still round-trips — only the ERA differs between the two halves"
        );
    }
}
