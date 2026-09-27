//! `garde` field validation on a hand-written typed tool (Phase 128, E3).
//!
//! Run it:
//!
//! ```text
//! cargo run --example s57_typed_tool_garde_validation --features "full"
//! ```
//!
//! **This example is meant to be RUN, not merely built.** `make test-examples`
//! compiles every example and runs none, so building it proves only that the API
//! type-checks — it cannot show that a refusal is value-free, which is the whole
//! point. Running it is therefore the manual leg of the CLAUDE.md ALWAYS-example
//! requirement and is listed in `128-VALIDATION.md` § Manual-Only Verifications.
//!
//! What it demonstrates, in order:
//!
//! 1. A request type carrying a `garde` `length` rule and a `range` rule, registered
//!    through [`TypedTool::new_validated`] — the opt-in constructor. The plain
//!    `TypedTool::new` stores no validator and would run neither rule.
//! 2. A conforming call: the handler body runs and returns its result.
//! 3. A refused call: the handler body never runs, and the refusal names the
//!    DECLARED bound while containing no byte of the rejected value. The example
//!    asserts that absence and prints the verdict, so a future regression is visible
//!    in the output rather than only in a test log.
//! 4. The same payload through the PLAIN constructor, which is accepted — the
//!    evidence that E3 is opt-in and changes nothing for an existing tool.
//!
//! A refusal is expected output here, not a failure: the process exits 0 after it.
//!
//! Sibling examples: `s16_typed_tools` (typed tools + schema generation),
//! `s20_typed_tool_v2` (the typed-tool builder surface). `s19_wasm_typed_tools` is
//! deliberately NOT related — its body is `wasm32`-only and it teaches a different
//! validation surface (`server::wasm_typed_tool::validation`).

use garde::Validate;
use pmcp::{RequestHandlerExtra, ToolHandler, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

/// Arguments for the `create_report` tool.
///
/// The `garde` attributes are the declaration; `TypedTool::new_validated` is what
/// makes them run.
#[derive(Debug, Deserialize, JsonSchema, Validate)]
struct CreateReportArgs {
    /// Report title. At most 16 characters.
    #[garde(length(chars, min = 1, max = 16))]
    title: String,
    /// How many rows to include. 1..=100.
    #[garde(range(min = 1, max = 100))]
    rows: u32,
}

/// A PHI-shaped value, so a leak would be obvious in the printed output.
const SENSITIVE_TITLE: &str = "Jane Doe DOB 1970-01-01 SSN 123-45-6789";

fn extra(request_id: &str) -> RequestHandlerExtra {
    RequestHandlerExtra::new(request_id.to_string(), Default::default())
}

fn report_tool_validated() -> impl ToolHandler {
    TypedTool::new_validated("create_report", |args: CreateReportArgs, _extra| {
        Box::pin(async move {
            Ok(json!({
                "title": args.title,
                "rows": args.rows,
                "status": "created",
            }))
        })
    })
}

fn report_tool_plain() -> impl ToolHandler {
    // Same type, same rules declared — but the plain constructor stores no
    // validator, so nothing runs them.
    TypedTool::new(
        "create_report_unvalidated",
        |args: CreateReportArgs, _extra| {
            Box::pin(async move { Ok(json!({"title": args.title, "rows": args.rows})) })
        },
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let validated = report_tool_validated();

    println!("=== 1. The declared schema `new_validated` still generates ===");
    let schema = validated
        .metadata()
        .and_then(|info| serde_json::to_value(info.input_schema).ok())
        .unwrap_or(Value::Null);
    println!(
        "    properties: {}\n",
        schema
            .get("properties")
            .map_or_else(|| "<none>".to_string(), ToString::to_string)
    );

    println!("=== 2. ACCEPTED — a conforming call ===");
    let accepted = validated
        .handle(
            json!({"title": "Q3 revenue", "rows": 25}),
            extra("req-accept"),
        )
        .await?;
    println!("    handler result: {accepted}\n");

    println!("=== 3. REFUSED — the title violates its declared maximum ===");
    let refusal = validated
        .handle(
            json!({"title": SENSITIVE_TITLE, "rows": 25}),
            extra("req-refuse"),
        )
        .await
        .expect_err("a 39-character title must be refused");
    let message = refusal.to_string();
    println!("    refusal: {message}");
    println!("    rejected value was: {SENSITIVE_TITLE:?}");
    assert!(
        !message.contains(SENSITIVE_TITLE),
        "REGRESSION: the refusal echoed the rejected value"
    );
    println!("    -> the rejected value does NOT appear in the refusal above: value-free ✓");
    println!("    -> the refusal names the declared field and bound instead\n");

    println!("=== 4. REFUSED — the row count is out of its declared range ===");
    let range_refusal = validated
        .handle(
            json!({"title": "Q3 revenue", "rows": 5000}),
            extra("req-range"),
        )
        .await
        .expect_err("5000 rows exceeds the declared maximum");
    let range_message = range_refusal.to_string();
    println!("    refusal: {range_message}");
    assert!(
        !range_message.contains("5000"),
        "REGRESSION: the refusal echoed the rejected number"
    );
    println!("    -> \"5000\" does NOT appear in the refusal above: value-free ✓\n");

    println!("=== 5. E3 is OPT-IN — the plain constructor accepts the same payload ===");
    let unvalidated = report_tool_plain()
        .handle(
            json!({"title": SENSITIVE_TITLE, "rows": 5000}),
            extra("req-plain"),
        )
        .await?;
    println!("    handler result: {unvalidated}");
    println!("    -> no rule ran, so no existing `TypedTool` user's behavior changed\n");

    println!("Done. A refusal is expected output here, so this process exits 0.");
    Ok(())
}
