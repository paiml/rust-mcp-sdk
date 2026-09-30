//! Integration tests for standalone `#[mcp_tool]` functions.
//!
//! These tests verify full macro expansion: compilation, schema generation,
//! tool handler implementation, and builder registration.

use pmcp::ToolHandler;
use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// === Shared argument types ===

#[derive(Debug, Deserialize, JsonSchema)]
struct EchoArgs {
    message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct EchoResult {
    echoed: String,
}

// === Test 0: a malformed-arguments call is a tool-level rejection ===

/// Arguments that do not deserialize are the caller's mistake to correct, so the
/// generated handler returns `Error::ToolRejected` (an `isError: true` tool
/// result), not a JSON-RPC protocol error a model reads as a server fault.
/// (pmcp-macros 0.6.2.)
#[tokio::test]
async fn test_malformed_arguments_are_a_tool_level_rejection() {
    let tool = echo();
    let err = tool
        .handle(
            serde_json::json!({"message": 42}),
            pmcp::RequestHandlerExtra::default(),
        )
        .await
        .expect_err("a number is not a string");
    assert!(
        matches!(err, pmcp::Error::ToolRejected { .. }),
        "expected ToolRejected, got {err:?}"
    );
    assert!(
        err.to_string()
            .contains("Invalid arguments for tool 'echo'"),
        "{err}"
    );
}

// === Test 1: Minimal async tool with typed output (D-14) ===

#[mcp_tool(description = "Echo a message")]
async fn echo(args: EchoArgs) -> pmcp::Result<EchoResult> {
    Ok(EchoResult {
        echoed: args.message,
    })
}

#[tokio::test]
async fn test_echo_tool_handle() {
    let tool = echo();
    let args = serde_json::json!({"message": "hello"});
    let extra = pmcp::RequestHandlerExtra::default();
    let result = tool.handle(args, extra).await.unwrap();
    assert_eq!(result["echoed"], "hello");
}

#[test]
fn test_echo_tool_metadata() {
    let tool = echo();
    let meta = tool.metadata().expect("metadata should exist");
    assert_eq!(meta.name, "echo");
    assert_eq!(meta.description.as_deref(), Some("Echo a message"));
    // Should have outputSchema since EchoResult: JsonSchema
    assert!(
        meta.output_schema.is_some(),
        "typed output should generate outputSchema"
    );
}

// === Test 2: Tool with Value return (D-15 -- no outputSchema) ===

#[mcp_tool(description = "Returns untyped JSON")]
async fn untyped(args: EchoArgs) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"msg": args.message}))
}

#[test]
fn test_untyped_no_output_schema() {
    let tool = untyped();
    let meta = tool.metadata().unwrap();
    assert!(
        meta.output_schema.is_none(),
        "Value return should NOT produce outputSchema"
    );
}

// === Test 3: No-arg tool (D-12) ===

#[mcp_tool(description = "Get server version")]
async fn version() -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"version": "1.0"}))
}

#[tokio::test]
async fn test_no_arg_tool() {
    let tool = version();
    let extra = pmcp::RequestHandlerExtra::default();
    let result = tool.handle(serde_json::json!({}), extra).await.unwrap();
    assert_eq!(result["version"], "1.0");
}

// === Test 4: Sync tool auto-detection (D-26) ===

#[mcp_tool(description = "Sync version check")]
fn sync_version() -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"version": "sync"}))
}

#[tokio::test]
async fn test_sync_tool() {
    let tool = sync_version();
    let extra = pmcp::RequestHandlerExtra::default();
    let result = tool.handle(serde_json::json!({}), extra).await.unwrap();
    assert_eq!(result["version"], "sync");
}

// === Test 5: Tool with State<T> (D-08, STATE-INJECTION) ===

struct MyDb {
    prefix: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct QueryArgs {
    sql: String,
}

#[mcp_tool(description = "Query with state")]
async fn query(args: QueryArgs, db: pmcp::State<MyDb>) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"query": format!("{}: {}", db.prefix, args.sql)}))
}

#[tokio::test]
async fn test_tool_with_state() {
    let tool = query().with_state(MyDb {
        prefix: "DB".into(),
    });
    let args = serde_json::json!({"sql": "SELECT 1"});
    let extra = pmcp::RequestHandlerExtra::default();
    let result = tool.handle(args, extra).await.unwrap();
    assert_eq!(result["query"], "DB: SELECT 1");
}

// === Test 6: Tool with custom name (D-06) ===

#[mcp_tool(description = "Custom named", name = "my_custom_tool")]
async fn internal_name(args: EchoArgs) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"msg": args.message}))
}

#[test]
fn test_custom_name() {
    let tool = internal_name();
    let meta = tool.metadata().unwrap();
    assert_eq!(meta.name, "my_custom_tool");
}

// === Test 7: Tool with annotations (D-23) ===

#[mcp_tool(
    description = "Destructive op",
    annotations(destructive = true, idempotent = false)
)]
async fn delete(args: EchoArgs) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"deleted": args.message}))
}

#[test]
fn test_annotations() {
    let tool = delete();
    let meta = tool.metadata().unwrap();
    let ann = meta.annotations.expect("should have annotations");
    assert_eq!(ann.destructive_hint, Some(true));
    assert_eq!(ann.idempotent_hint, Some(false));
}

// === Test 8: Tool with RequestHandlerExtra (D-07) ===

#[mcp_tool(description = "Tool with extra")]
async fn with_extra(
    args: EchoArgs,
    extra: pmcp::RequestHandlerExtra,
) -> pmcp::Result<serde_json::Value> {
    // Verify extra is accessible and not cancelled
    assert!(!extra.is_cancelled());
    Ok(serde_json::json!({"msg": args.message}))
}

#[tokio::test]
async fn test_tool_with_extra() {
    let tool = with_extra();
    let args = serde_json::json!({"message": "with_extra"});
    let extra = pmcp::RequestHandlerExtra::default();
    let result = tool.handle(args, extra).await.unwrap();
    assert_eq!(result["msg"], "with_extra");
}

// === Test 9: Registration on ServerBuilder ===

#[test]
fn test_builder_registration() {
    let builder = pmcp::ServerBuilder::new()
        .name("test")
        .version("1.0.0")
        .tool("echo", echo());
    // This compiles = ToolHandler impl is correct
    drop(builder);
}

// ==========================================================================
// Rustdoc fallback integration tests
// ==========================================================================

/// Add two numbers together.
#[mcp_tool]
async fn rustdoc_only_tool(args: EchoArgs) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"echoed": args.message}))
}

#[test]
fn test_rustdoc_only_description() {
    let tool = rustdoc_only_tool();
    let meta = tool.metadata().expect("metadata should exist");
    assert_eq!(
        meta.description.as_deref(),
        Some("Add two numbers together."),
        "rustdoc-only tool should harvest `/// Add two numbers together.`"
    );
}

/// IGNORED (rustdoc must not win).
#[mcp_tool(description = "WINS")]
async fn precedence_tool(args: EchoArgs) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"echoed": args.message}))
}

#[test]
fn test_attribute_wins_over_rustdoc() {
    let tool = precedence_tool();
    let meta = tool.metadata().expect("metadata should exist");
    assert_eq!(
        meta.description.as_deref(),
        Some("WINS"),
        "attribute description MUST win over rustdoc silently"
    );
}

/// First line of the description.
/// Second line with more detail.
///
/// Third line after a blank middle.
#[mcp_tool]
async fn multiline_rustdoc_tool(args: EchoArgs) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({"echoed": args.message}))
}

#[test]
fn test_multiline_rustdoc_normalization() {
    let tool = multiline_rustdoc_tool();
    let meta = tool.metadata().expect("metadata should exist");
    assert_eq!(
        meta.description.as_deref(),
        Some(
            "First line of the description.\nSecond line with more detail.\nThird line after a blank middle."
        ),
        "multi-line rustdoc must be trim-joined with blank lines dropped"
    );
}

// === Compile-fail tests (trybuild) ===

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/mcp_tool_missing_description.rs");
    t.compile_fail("tests/ui/mcp_tool_multiple_args.rs");
    t.compile_fail("tests/ui/mcp_tool_missing_description_and_rustdoc.rs");
    t.compile_fail("tests/ui/mcp_tool_nonempty_args_missing_description_and_rustdoc.rs");
}
