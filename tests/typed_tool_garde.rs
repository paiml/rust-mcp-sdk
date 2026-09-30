//! E3 — `garde` field validation on hand-written typed tools (Phase 128, plan 04).
//!
//! Run:
//!
//! ```text
//! RUSTFLAGS="" cargo test -p pmcp --features "full" --test typed_tool_garde -- --test-threads=1
//! RUSTFLAGS="" PROPTEST_CASES=64 cargo test -p pmcp --features "full" --test typed_tool_garde -- --ignored property_
//! ```
//!
//! Naming rule: every test fn is prefixed `typed_tool_garde_` so a positional
//! filter (`cargo test typed_tool_garde`) resolves this binary's contents, and the
//! single property arm is prefixed `property_` so `make test-property`'s
//! `--ignored property_` selector reaches it. This file lives in ROOT `tests/`
//! precisely because that selector only scans root targets.
//!
//! What every refusal row asserts: that the rendered message does NOT contain the
//! rejected value. A refusal that names the declared bound is the goal; a refusal
//! that quotes the caller's argument is the leak this plan closes (T-128-17,
//! T-128-17a, T-128-17b).
#![cfg(feature = "validation")]

use garde::Validate;
use pmcp::{RequestHandlerExtra, ToolHandler, TypedSyncTool, TypedTool};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// The token the D1 path (`schema_validation::safe_pointer`) redacts a
/// caller-chosen pointer segment with. Duplicated here as a STRING LITERAL on
/// purpose: the const is crate-private, and an integration test asserting the
/// literal is what pins the two paths to the same wire text.
const REDACTED_SEGMENT: &str = "<redacted>";

fn extra(id: &str) -> RequestHandlerExtra {
    RequestHandlerExtra::new(id.to_string(), Default::default())
}

/// Unwrap a refusal's message, failing loudly on any other error variant.
///
/// A refusal is a TOOL-LEVEL rejection (`isError: true`), not a protocol error:
/// the model reads a protocol error as a server fault and has nothing to correct.
fn refusal(err: pmcp::Error) -> String {
    match err {
        pmcp::Error::ToolRejected { message, .. } => message,
        other => panic!("expected Error::ToolRejected, got {other:?}"),
    }
}

fn object_schema() -> Value {
    json!({"type": "object"})
}

/// `{"buckets": {<key>: {"label": "too-long-label"}}}`, built explicitly because
/// the map KEY is the caller-controlled part under test.
fn buckets_payload(key: &str) -> Value {
    let mut buckets = serde_json::Map::new();
    buckets.insert(key.to_string(), json!({"label": "too-long-label"}));
    json!({"buckets": Value::Object(buckets)})
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A declared `length` rule: the canonical E3 shape.
#[derive(Debug, Deserialize, Validate)]
struct TitleArgs {
    #[garde(length(chars, max = 10))]
    title: String,
}

/// No `garde` derive at all — the "existing constructor, no validation" row.
#[derive(Debug, Deserialize)]
struct PlainArgs {
    title: String,
}

/// A `#[garde(dive)]` into a map: garde 0.23 extends the `Path` with the map's
/// KEYS (`garde-0.23.0/src/validate.rs:293`), which are caller-chosen.
#[derive(Debug, Deserialize, Validate)]
struct BucketsArgs {
    #[garde(dive)]
    buckets: HashMap<String, Bucket>,
}

#[derive(Debug, Deserialize, Validate)]
struct Bucket {
    #[garde(length(chars, max = 4))]
    label: String,
}

/// A wrong-type deserialization failure: `serde_json`'s `Display` quotes the value.
#[derive(Debug, Deserialize, Validate)]
struct CountArgs {
    #[garde(range(max = 100))]
    count: u32,
}

/// An unknown-enum-variant deserialization failure.
#[derive(Debug, Deserialize, Validate)]
struct ModeArgs {
    #[garde(skip)]
    #[allow(dead_code)]
    mode: Mode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
enum Mode {
    Fast,
    Slow,
}

/// An invalid-string deserialization failure (`char` wants exactly one character).
#[derive(Debug, Deserialize, Validate)]
struct InitialArgs {
    #[garde(skip)]
    #[allow(dead_code)]
    initial: char,
}

/// The PHI-shaped payload every refusal row asserts the absence of.
const SECRET: &str = "Jane Doe DOB 1970-01-01";

// ---------------------------------------------------------------------------
// TypedTool — the async type
// ---------------------------------------------------------------------------

#[tokio::test]
async fn typed_tool_garde_violation_refuses_and_never_echoes_the_value() {
    let tool = TypedTool::new_validated_with_schema(
        "validated_title",
        object_schema(),
        |args: TitleArgs, _extra| Box::pin(async move { Ok(json!({"title": args.title})) }),
    );

    let err = tool
        .handle(json!({"title": SECRET}), extra("r-1"))
        .await
        .expect_err("an over-long title must be refused");
    let message = refusal(err);

    assert!(
        message.contains("title"),
        "the refusal must name the DECLARED field: {message}"
    );
    assert!(
        message.contains("10"),
        "the refusal must carry the declared bound: {message}"
    );
    assert!(
        !message.contains(SECRET),
        "the refusal leaked the rejected value: {message}"
    );
}

#[tokio::test]
async fn typed_tool_garde_conforming_value_runs_the_handler() {
    let tool = TypedTool::new_validated_with_schema(
        "validated_title",
        object_schema(),
        |args: TitleArgs, _extra| Box::pin(async move { Ok(json!({"title": args.title})) }),
    );

    let out = tool
        .handle(json!({"title": "0123456789"}), extra("r-2"))
        .await
        .expect("a 10-character title is exactly at the declared maximum");
    assert_eq!(out, json!({"title": "0123456789"}));
}

#[tokio::test]
async fn typed_tool_garde_existing_constructor_runs_no_validation() {
    // `TitleArgs` DOES implement `garde::Validate`, and the plain constructor must
    // still not validate: the stored validator is `None`.
    let tool = TypedTool::new_with_schema(
        "unvalidated_title",
        object_schema(),
        |args: TitleArgs, _extra| Box::pin(async move { Ok(json!({"title": args.title})) }),
    );

    let out = tool
        .handle(json!({"title": SECRET}), extra("r-3"))
        .await
        .expect("the plain constructor must not validate");
    assert_eq!(out, json!({"title": SECRET}));
}

#[tokio::test]
async fn typed_tool_garde_type_without_derive_is_unaffected() {
    let tool =
        TypedTool::new_with_schema("plain_title", object_schema(), |args: PlainArgs, _extra| {
            Box::pin(async move { Ok(json!({"len": args.title.len()})) })
        });

    let out = tool
        .handle(json!({"title": SECRET}), extra("r-4"))
        .await
        .expect("a type with no garde derive behaves exactly as before");
    assert_eq!(out, json!({"len": SECRET.len()}));
}

#[tokio::test]
async fn typed_tool_garde_map_dive_key_is_redacted_not_echoed() {
    let tool = TypedTool::new_validated_with_schema(
        "validated_buckets",
        object_schema(),
        |_args: BucketsArgs, _extra| Box::pin(async move { Ok(json!({"ok": true})) }),
    );

    let err = tool
        .handle(buckets_payload(SECRET), extra("r-5"))
        .await
        .expect_err("the inner label violates its declared maximum");
    let message = refusal(err);

    assert!(
        !message.contains(SECRET),
        "the caller-chosen map key reached the refusal: {message}"
    );
    assert!(
        message.contains(REDACTED_SEGMENT),
        "the caller-chosen path segment must carry the fixed redaction token: {message}"
    );
    assert!(
        message.contains("buckets") && message.contains("label"),
        "the DECLARED segments must survive the projection: {message}"
    );
}

#[tokio::test]
async fn typed_tool_garde_dotted_map_key_is_redacted_as_one_segment() {
    // `garde::Path`'s `Display` joins components with `.`, so a key CONTAINING a
    // dot is indistinguishable from nesting once rendered. The projection walks the
    // components instead, so the whole key is redacted as a single segment rather
    // than split into halves that each look like an identifier.
    let dotted = "ssn.value";
    let tool = TypedTool::new_validated_with_schema(
        "validated_buckets",
        object_schema(),
        |_args: BucketsArgs, _extra| Box::pin(async move { Ok(json!({"ok": true})) }),
    );

    let err = tool
        .handle(buckets_payload(dotted), extra("r-6"))
        .await
        .expect_err("the inner label violates its declared maximum");
    let message = refusal(err);

    assert!(
        !message.contains(dotted),
        "a dotted caller key must not survive as text: {message}"
    );
    assert_eq!(
        message.matches(REDACTED_SEGMENT).count(),
        1,
        "the dotted key must be ONE redacted segment, not two: {message}"
    );
}

// ---------------------------------------------------------------------------
// The deserialization route in front of garde (T-128-17a)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn typed_tool_garde_unvalidated_deserialize_message_is_byte_identical() {
    let payload = json!({"count": SECRET});
    let expected_inner = serde_json::from_value::<CountArgs>(payload.clone())
        .expect_err("a string is not a u32")
        .to_string();

    let tool = TypedTool::new_with_schema(
        "legacy_count",
        object_schema(),
        |_args: CountArgs, _extra| Box::pin(async move { Ok(json!({})) }),
    );

    let message = refusal(
        tool.handle(payload, extra("r-7"))
            .await
            .expect_err("deserialization must fail"),
    );

    assert_eq!(
        message,
        format!("Invalid arguments for tool 'legacy_count': {expected_inner}"),
        "the UNVALIDATED path's message must stay byte-identical to today's"
    );
    assert!(
        message.contains(SECRET),
        "today's message does echo the value — that is the asymmetry being pinned"
    );
}

#[tokio::test]
async fn typed_tool_garde_validated_deserialize_redacts_a_wrong_type() {
    let tool = TypedTool::new_validated_with_schema(
        "validated_count",
        object_schema(),
        |_args: CountArgs, _extra| Box::pin(async move { Ok(json!({})) }),
    );

    let message = refusal(
        tool.handle(json!({"count": SECRET}), extra("r-8"))
            .await
            .expect_err("a string is not a u32"),
    );

    assert!(
        !message.contains(SECRET),
        "the validated path leaked the rejected value: {message}"
    );
    assert!(
        message.contains("validated_count"),
        "the refusal must still name the tool: {message}"
    );
}

#[tokio::test]
async fn typed_tool_garde_validated_deserialize_redacts_an_unknown_enum_variant() {
    let tool = TypedTool::new_validated_with_schema(
        "validated_mode",
        object_schema(),
        |_args: ModeArgs, _extra| Box::pin(async move { Ok(json!({})) }),
    );

    let message = refusal(
        tool.handle(json!({"mode": SECRET}), extra("r-9"))
            .await
            .expect_err("an unknown variant must fail"),
    );

    assert!(
        !message.contains(SECRET),
        "the validated path leaked the unknown variant text: {message}"
    );
}

#[tokio::test]
async fn typed_tool_garde_validated_deserialize_redacts_an_invalid_string() {
    let tool = TypedTool::new_validated_with_schema(
        "validated_initial",
        object_schema(),
        |_args: InitialArgs, _extra| Box::pin(async move { Ok(json!({})) }),
    );

    let message = refusal(
        tool.handle(json!({"initial": SECRET}), extra("r-10"))
            .await
            .expect_err("a multi-character string is not a char"),
    );

    assert!(
        !message.contains(SECRET),
        "the validated path leaked the invalid string: {message}"
    );
}

// ---------------------------------------------------------------------------
// TypedSyncTool — the same behaviours on the sync type
// ---------------------------------------------------------------------------

#[tokio::test]
async fn typed_tool_garde_sync_violation_refuses_and_never_echoes_the_value() {
    let tool = TypedSyncTool::new_validated_with_schema(
        "validated_sync_title",
        object_schema(),
        |args: TitleArgs, _extra| Ok(json!({"title": args.title})),
    );

    let message = refusal(
        tool.handle(json!({"title": SECRET}), extra("s-1"))
            .await
            .expect_err("an over-long title must be refused"),
    );

    assert!(message.contains("title"), "{message}");
    assert!(
        !message.contains(SECRET),
        "the sync refusal leaked the rejected value: {message}"
    );
}

#[tokio::test]
async fn typed_tool_garde_sync_conforming_value_runs_the_handler() {
    let tool = TypedSyncTool::new_validated_with_schema(
        "validated_sync_title",
        object_schema(),
        |args: TitleArgs, _extra| Ok(json!({"title": args.title})),
    );

    let out = tool
        .handle(json!({"title": "0123456789"}), extra("s-2"))
        .await
        .expect("a 10-character title is exactly at the declared maximum");
    assert_eq!(out, json!({"title": "0123456789"}));
}

#[tokio::test]
async fn typed_tool_garde_sync_existing_constructor_runs_no_validation() {
    let tool = TypedSyncTool::new_with_schema(
        "unvalidated_sync_title",
        object_schema(),
        |args: TitleArgs, _extra| Ok(json!({"title": args.title})),
    );

    let out = tool
        .handle(json!({"title": SECRET}), extra("s-3"))
        .await
        .expect("the plain constructor must not validate");
    assert_eq!(out, json!({"title": SECRET}));
}

#[tokio::test]
async fn typed_tool_garde_sync_map_dive_key_is_redacted_not_echoed() {
    let tool = TypedSyncTool::new_validated_with_schema(
        "validated_sync_buckets",
        object_schema(),
        |_args: BucketsArgs, _extra| Ok(json!({"ok": true})),
    );

    let message = refusal(
        tool.handle(buckets_payload(SECRET), extra("s-4"))
            .await
            .expect_err("the inner label violates its declared maximum"),
    );

    assert!(!message.contains(SECRET), "{message}");
    assert!(message.contains(REDACTED_SEGMENT), "{message}");
}

#[tokio::test]
async fn typed_tool_garde_sync_validated_deserialize_is_redacted() {
    let tool = TypedSyncTool::new_validated_with_schema(
        "validated_sync_count",
        object_schema(),
        |_args: CountArgs, _extra| Ok(json!({})),
    );

    let message = refusal(
        tool.handle(json!({"count": SECRET}), extra("s-5"))
            .await
            .expect_err("a string is not a u32"),
    );
    assert!(!message.contains(SECRET), "{message}");
}

#[tokio::test]
async fn typed_tool_garde_sync_unvalidated_deserialize_message_is_byte_identical() {
    let payload = json!({"count": SECRET});
    let expected_inner = serde_json::from_value::<CountArgs>(payload.clone())
        .expect_err("a string is not a u32")
        .to_string();

    let tool = TypedSyncTool::new_with_schema(
        "legacy_sync_count",
        object_schema(),
        |_args: CountArgs, _extra| Ok(json!({})),
    );

    let message = refusal(
        tool.handle(payload, extra("s-6"))
            .await
            .expect_err("deserialization must fail"),
    );
    assert_eq!(
        message,
        format!("Invalid arguments for tool 'legacy_sync_count': {expected_inner}")
    );
}

// ---------------------------------------------------------------------------
// `new_validated` — the schema-generating constructor
// ---------------------------------------------------------------------------

#[cfg(feature = "schema-generation")]
mod schema_generating {
    use super::{extra, refusal, SECRET};
    use garde::Validate;
    use pmcp::{ToolHandler, TypedSyncTool, TypedTool};
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize, JsonSchema, Validate)]
    struct TitleArgs {
        #[garde(length(chars, max = 10))]
        title: String,
    }

    #[tokio::test]
    async fn typed_tool_garde_new_validated_generates_a_schema_and_validates() {
        let tool = TypedTool::new_validated("generated", |args: TitleArgs, _extra| {
            Box::pin(async move { Ok(json!({"title": args.title})) })
        });

        assert_eq!(
            tool.metadata().expect("metadata").input_schema["type"],
            json!("object"),
            "new_validated must still generate the schema `new` would"
        );

        let message = refusal(
            tool.handle(json!({"title": SECRET}), extra("g-1"))
                .await
                .expect_err("an over-long title must be refused"),
        );
        assert!(!message.contains(SECRET), "{message}");
    }

    #[tokio::test]
    async fn typed_tool_garde_sync_new_validated_generates_a_schema_and_validates() {
        let tool = TypedSyncTool::new_validated("generated_sync", |args: TitleArgs, _extra| {
            Ok(json!({"title": args.title}))
        });

        assert!(tool.metadata().expect("metadata").input_schema.is_object());

        let message = refusal(
            tool.handle(json!({"title": SECRET}), extra("g-2"))
                .await
                .expect_err("an over-long title must be refused"),
        );
        assert!(!message.contains(SECRET), "{message}");
    }
}

// ---------------------------------------------------------------------------
// Property arm — the CLAUDE.md ALWAYS-property leg for E3
// ---------------------------------------------------------------------------

/// Over arbitrary strings longer than the declared maximum, the rendered refusal
/// never contains the input.
///
/// Selected by `make test-property` (`cargo test --features "full" -- --ignored
/// property_`), which only scans ROOT test targets — hence this file's location.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_typed_tool_garde_refusal_never_contains_the_input() {
    use proptest::prelude::*;

    const TOOL: &str = "validated_property_tool";

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("current-thread runtime");

    proptest!(|(chars in proptest::collection::vec(any::<char>(), 11..40))| {
        let input: String = chars.into_iter().collect();

        // The refusal's fixed text is known WITHOUT the input, so a coincidental
        // substring collision is discarded rather than reported as a leak.
        let template = format!("Invalid arguments for tool '{TOOL}': /title: ");
        prop_assume!(!template.contains(input.as_str()));

        let tool = TypedTool::new_validated_with_schema(
            TOOL,
            object_schema(),
            |args: TitleArgs, _extra| Box::pin(async move { Ok(json!({"title": args.title})) }),
        );

        let message = refusal(
            runtime
                .block_on(tool.handle(json!({"title": input.clone()}), extra("p-1")))
                .expect_err("an over-long title must be refused"),
        );

        prop_assert!(
            !message.contains(input.as_str()),
            "refusal leaked the input: {}",
            message
        );
        prop_assert!(message.contains("title"), "{}", message);
    });
}
