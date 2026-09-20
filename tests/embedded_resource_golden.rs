//! Phase 118.1-02 (CONF-04, D-04 / D-11): **spec-derived byte-literal goldens for
//! the `EmbeddedResource` wire shape, written BEFORE the fix that makes them pass.**
//!
//! # These goldens are RED on an unfixed tree, and that is the point
//!
//! Plan 118.1-03 changes `Content::Resource`'s emitter from the flat legacy shape
//! pmcp has always produced to the spec's nested `EmbeddedResource`. This file is
//! the fence that change has to clear, and D-04 requires the fence to be
//! demonstrated failing FIRST. On the tree this file was written against, EIGHT
//! of its ten tests fail — the four goldens on each of the two eras: six on a
//! flat-versus-nested diff and two on a missing `blob`. The two that pass are
//! the controls, and they are required to pass.
//!
//! **Do not make a literal pass.** If a golden here is red and you are not
//! landing 118.1-03, the correct response is to leave it red — exactly as
//! `tests/v1_lists_golden.rs` says a diff in ITS literals is a wire break rather
//! than a stale fixture. Weakening a literal, or marking a test `#[ignore]`,
//! converts the only evidence that the fence measures anything into evidence that
//! it does not.
//!
//! # Every literal is hand-derived from the vendored schema, never from pmcp
//!
//! Each of the four byte literals below carries a `schema.ts:` provenance comment
//! naming the interface and line range it was typed from, in
//! `schema/vendored/core-2026-07-28/schema.ts`. That derivation is the whole
//! mechanism: a golden captured from pmcp's own serializer would be GREEN on the
//! unfixed tree, which is precisely how a fence comes to restate the defect it was
//! supposed to catch. If any literal here passes before 118.1-03 lands, it was
//! derived from the wrong oracle and must be rewritten against the schema.
//!
//! # Why raw strings, not parsed JSON
//!
//! This crate builds `serde_json` with `preserve_order` (`Cargo.toml:55`), so key
//! ORDER is observable on the wire, and the flat-to-nested reshape is exactly the
//! class of change that alters bytes while leaving every structural assertion
//! true. The v1 leg therefore compares the raw response text; the v2 leg asserts
//! the content item appears in the raw bytes verbatim.
//!
//! # Which spawn each leg uses, and why
//!
//! Following the spawn-choice doctrine at `tests/v2_stateless_http.rs:9-23`:
//!
//! * **v1 leg — [`spawn_stateless_config`]**, i.e. `StreamableHttpServerConfig::stateless()`.
//!   That config carries `enable_json_response: true`, so [`Resp::raw`] IS the
//!   JSON-RPC frame rather than an SSE-framed copy of it, which is what lets the
//!   v1 leg pin a FULL frame byte for byte. This is the same choice
//!   `tests/v1_lists_golden.rs` makes for the same reason.
//! * **v2 leg — [`spawn_default_config`]**, i.e. `StreamableHttpServerConfig::default()`,
//!   which keeps a LIVE `session_id_generator`. `stateless()` is a BUILD-TIME
//!   config that removes the session machinery before a request is ever seen, so a
//!   v2 leg spawned that way could never prove the PER-REQUEST era gate ran. A
//!   real dual-version deployment is built with `Default::default()`, so the
//!   stateful config is the realistic case.
//!
//! **One fixture server serves both legs and is v2-opted-in on both.** The era is
//! then selected by the REQUEST — a bare body for v1, a reserved `_meta` plus the
//! three v2 headers for v2 — which is a strictly stronger claim than pointing a v1
//! body at a server that could not have answered v2 anyway. Each leg asserts its
//! own era witness so "this response was v1" is a checked property rather than an
//! assumption.
//!
//! # Why the two legs pin different amounts
//!
//! The v1 leg pins the FULL frame. The v2 leg pins only the content ITEM, as a
//! byte-exact substring, plus the presence of the v2 result envelope. The v2
//! envelope (`resultType` / `serverInfo`, and on `resources/read` also `ttlMs` /
//! `cacheScope`) is Phase 112's and Phase 115's surface and is pinned by their own
//! suites; re-pinning it here would make CONF-04's fence fail for envelope reasons
//! and would put two files in charge of the same bytes.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use async_trait::async_trait;
use common::v2::{
    post, spawn_default_config, spawn_stateless_config, teardown, v1_body, v2_body, v2_headers_for,
    Resp, V1, V2,
};
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::{
    CallToolResult, Content, GetPromptResult, ListResourcesResult, PromptInfo, PromptMessage,
    ReadResourceResult,
};
use pmcp::{RequestHandlerExtra, ToolHandler, ToolOutput};
use serde_json::{json, Value};
use std::collections::HashMap;

// ===========================================================================
// Dynamic-value normalization.
//
// Restated from `tests/v1_lists_golden.rs:87-186` rather than shared: a Rust
// integration test is its own crate, so the two files cannot import each other.
// ===========================================================================

/// A response value that cannot be pinned because it is minted per run.
///
/// `token` replaces the value in the CANONICAL normalization (the one compared
/// against the golden literal). In the SAME-WIDTH normalization the token is
/// padded with `#` to the value's own byte width, so the normalized string is
/// exactly as long as the raw one — the check that proves the substitution
/// neither adds nor removes bytes and, in particular, never deletes a key.
struct DynamicField {
    /// The JSON object key whose STRING value is dynamic.
    key: &'static str,
    /// The canonical placeholder written into the golden literal.
    token: &'static str,
    /// Shape predicate the raw value must satisfy — a normalization that
    /// accepted any string would let a reshaped value through unnoticed.
    shape: fn(&str) -> bool,
    /// Human-readable form of `shape`, for the failure message.
    shape_description: &'static str,
}

/// Every value on the wire in this file is one the fixture server chose: no id,
/// no timestamp, no port. So NOTHING is normalized and the four literals are
/// pinned verbatim, byte for byte.
///
/// The machinery is kept anyway, and still runs on every call, so the width
/// invariant is an executed no-op rather than an untested claim — and so a future
/// fixture that DOES need normalization has the instrument in place instead of
/// reaching for a relaxed comparison.
const NO_DYNAMICS: &[DynamicField] = &[];

/// `token`, padded with `#` to exactly `width` bytes.
fn width_preserving(token: &str, width: usize) -> String {
    assert!(
        token.len() <= width,
        "placeholder `{token}` is wider than the {width}-byte value it replaces; \
         pick a shorter token rather than shortening the value"
    );
    let mut padded = String::with_capacity(width);
    padded.push_str(token);
    padded.push_str(&"#".repeat(width - token.len()));
    padded
}

/// Replace every dynamic value in `raw`.
///
/// With `same_width`, each value becomes a padded placeholder of its own width;
/// otherwise it becomes the bare canonical token. Both passes are pure string
/// operations, so key order, spacing and null-versus-absent all survive into the
/// comparison.
fn substitute(raw: &str, fields: &[DynamicField], same_width: bool) -> String {
    let mut out = raw.to_string();
    for field in fields {
        out = substitute_one(&out, field, same_width);
    }
    out
}

fn substitute_one(raw: &str, field: &DynamicField, same_width: bool) -> String {
    let needle = format!("\"{}\":\"", field.key);
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut hits = 0_usize;
    while let Some(position) = rest.find(needle.as_str()) {
        let value_start = position + needle.len();
        out.push_str(&rest[..value_start]);
        let tail = &rest[value_start..];
        let end = tail
            .find('"')
            .unwrap_or_else(|| panic!("unterminated `{}` string value in: {raw}", field.key));
        let value = &tail[..end];
        assert!(
            (field.shape)(value),
            "`{}` carried `{value}`, which is not {} — either the value shape \
             changed or this fixture is normalizing the wrong key",
            field.key,
            field.shape_description
        );
        if same_width {
            out.push_str(&width_preserving(field.token, value.len()));
        } else {
            out.push_str(field.token);
        }
        rest = &tail[end..];
        hits += 1;
    }
    assert!(
        hits > 0,
        "declared dynamic key `{}` does not appear in the response — a golden \
         that normalizes an absent key proves nothing: {raw}",
        field.key
    );
    out.push_str(rest);
    out
}

fn key_occurrences(text: &str, key: &str) -> usize {
    text.matches(format!("\"{key}\":").as_str()).count()
}

// ===========================================================================
// The flat-shape leak guard.
// ===========================================================================

/// Read a JSON-RPC frame out of a response body that may be bare JSON or SSE.
///
/// The v2 leg is spawned with `enable_json_response: false`, so its reply arrives
/// as `event: message\ndata: {…}`. Handling both framings here keeps the guard
/// usable on BOTH legs; an unparseable frame is an `Err` rather than a silent
/// `Ok`, because a guard that accepts what it cannot read is worse than no guard.
fn parse_frame(raw: &str) -> Result<Value, String> {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        return Ok(value);
    }
    for line in raw.lines() {
        if let Some(payload) = line.strip_prefix("data:") {
            if let Ok(value) = serde_json::from_str::<Value>(payload.trim()) {
                return Ok(value);
            }
        }
    }
    Err(format!(
        "the flat-resource guard could not parse this frame as JSON or as an SSE \
         `data:` frame, so it inspected NOTHING. Treat that as a failure, not as a \
         pass. Raw response was: {raw}"
    ))
}

/// Reject any content-level `{"type":"resource"}` object that is not the spec's
/// nested `EmbeddedResource` (a schema interface, not a Rust item — kept as a
/// code span rather than an intra-doc link, per the Phase-67 demote-to-backticks
/// pattern that `make doc-check` enforces).
///
/// # Why this is a function returning `Result`, not an inline `assert!`
///
/// Until 118.1-03 lands, this guard FIRES on every real response the fixture
/// server produces, and after it lands it should fire on none — so on neither
/// side of the fix does a real round trip prove the guard DISCRIMINATES. A
/// guard that rejected everything, or that was wired to the wrong key and
/// accepted everything, would be indistinguishable from a working one at exactly
/// the moment it was supposed to catch a regression. Returning a `Result` lets
/// [`embedded_resource_golden_flat_guard_is_load_bearing`] drive synthetic frames
/// through the predicate directly — no `catch_unwind`, no test-only duplicate of
/// the logic — so both directions are proven today.
///
/// `ReadResourceResult.contents` elements are deliberately NOT flagged: the D-01
/// boundary keeps that position FLAT, and the custom projection strips the `type`
/// tag there, so those objects never match this guard's `"type":"resource"`
/// precondition.
fn flat_embedded_resource_guard(raw: &str) -> Result<(), String> {
    let frame = parse_frame(raw)?;
    let mut stack = vec![frame];
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(map) => {
                inspect_tagged_resource(&map)?;
                stack.extend(map.into_iter().map(|(_, nested)| nested));
            },
            Value::Array(items) => stack.extend(items),
            _ => {},
        }
    }
    Ok(())
}

/// The per-object half of [`flat_embedded_resource_guard`].
///
/// Split out so the walk stays a plain traversal and this stays a plain
/// predicate; the three rejections are each exercised by the negative control.
fn inspect_tagged_resource(map: &serde_json::Map<String, Value>) -> Result<(), String> {
    if map.get("type").and_then(Value::as_str) != Some("resource") {
        return Ok(());
    }
    if map.contains_key("uri") {
        return Err(format!(
            "a content-level `{{\"type\":\"resource\"}}` object carries a TOP-LEVEL `uri`. \
             That is pmcp's legacy FLAT shape and it is the G-1 defect: the spec's \
             `EmbeddedResource` (schema.ts:1734-1744) nests the contents under a \
             `resource` key. Do NOT relax this guard to accept the flat shape — emit \
             the nested one. Object was: {}",
            Value::Object(map.clone())
        ));
    }
    let Some(resource) = map.get("resource") else {
        return Err(format!(
            "a content-level `{{\"type\":\"resource\"}}` object carries NEITHER a nested \
             `resource` nor a flat `uri`, so it is not a valid `EmbeddedResource` in \
             either shape (schema.ts:1734-1744). Object was: {}",
            Value::Object(map.clone())
        ));
    };
    let Some(nested) = resource.as_object() else {
        return Err(format!(
            "`resource` must be a `TextResourceContents` or `BlobResourceContents` OBJECT \
             (schema.ts:1535-1540, schema.ts:1548-1555), got: {resource}"
        ));
    };
    if nested.contains_key("annotations") {
        return Err(format!(
            "`annotations` is declared on `EmbeddedResource` itself (schema.ts:1741), \
             OUTSIDE the nested `resource` — the MCP Apps widget path reads it at content \
             level. Found it inside `resource`: {resource}"
        ));
    }
    Ok(())
}

// ===========================================================================
// Era witnesses.
// ===========================================================================

/// The Phase-112 v2 response-envelope keys. `inject_v2_result_envelope`
/// (`src/server/core.rs:1644`) returns early on any era that is not `V2`, so
/// either of these on a v1 wire means that early return was bypassed — and their
/// ABSENCE is what proves the v1 leg really was served as v1.
const V2_ENVELOPE_KEYS: [&str; 2] = ["resultType", "serverInfo"];

fn assert_v1_era(raw: &str, label: &str) {
    for key in V2_ENVELOPE_KEYS {
        assert!(
            !raw.contains(key),
            "{label}: this leg sends a bare v1 body against a v2-CAPABLE server, so its \
             answer carrying the v2 envelope key `{key}` means the per-request era gate \
             misfired and the literal below is pinning the wrong era's bytes: {raw}"
        );
    }
}

fn assert_v2_era(response: &Resp, label: &str) {
    assert_eq!(
        response.status, 200,
        "{label}: the v2 leg must be served, got HTTP {} with body {}",
        response.status, response.raw
    );
    assert!(
        response.body["result"].get("resultType").is_some(),
        "{label}: no `resultType` in the result, so this request was NOT served as v2 and \
         the item assertion below would be a v1 assertion wearing a v2 label: {}",
        response.raw
    );
}

// ===========================================================================
// The assertion helpers.
// ===========================================================================

/// One pinned embedded-resource response.
struct EmbeddedGolden<'a> {
    /// The JSON-RPC request id the frame must echo.
    id: i64,
    /// The full v1 frame, byte for byte, after canonical normalization.
    frame: &'a str,
    /// The content item ALONE — the substring the v2 leg pins. It must appear
    /// verbatim inside `frame`, which [`assert_v1_frame_bytes`] and
    /// [`embedded_resource_golden_frames_embed_their_items`] both check, so the
    /// two literals cannot drift apart.
    item: &'a str,
    /// The same frame's `result` payload, for a readable structural message.
    result: Value,
    /// Values normalized before comparison (see [`DynamicField`]).
    dynamics: &'a [DynamicField],
}

/// The failure text the raw-byte comparison carries.
///
/// Factored out so the `assert_eq!` invocation stays on one line: this is the
/// assertion a reviewer greps for when asking "does this file compare bytes, or
/// only parsed JSON?".
fn wire_break_message(raw: &str) -> String {
    format!(
        "CONF-04 embedded-resource wire bytes do not match the SPEC-DERIVED golden. \
         On an unfixed tree this failure is EXPECTED and is the D-04 evidence — plan \
         118.1-03 lands the emitter change that turns it green. Do NOT re-record the \
         golden and do NOT weaken it: the literal was hand-typed from \
         schema/vendored/core-2026-07-28/schema.ts, and a literal captured from pmcp's \
         own serializer could not be red here at all. Raw response was: {raw}"
    )
}

/// Assert the v1 leg's raw frame is byte-identical to `golden.frame`.
fn assert_v1_frame_bytes(raw: &str, golden: &EmbeddedGolden<'_>) {
    assert!(
        golden.frame.contains(golden.item),
        "the frame literal must EMBED the item literal verbatim, or the two legs are \
         pinning different bytes under one name"
    );

    let same_width = substitute(raw, golden.dynamics, true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the placeholder substitution changed the response length; it must be \
         width-preserving so it cannot mask an added or removed byte: {raw}"
    );
    for field in golden.dynamics {
        assert_eq!(
            key_occurrences(&same_width, field.key),
            key_occurrences(raw, field.key),
            "the substitution changed how often `{}` appears; it must replace VALUES \
             only and never delete a key: {raw}",
            field.key
        );
    }

    let normalized = substitute(raw, golden.dynamics, false);
    assert_eq!(normalized, golden.frame, "{}", wire_break_message(raw));

    let parsed: Value = serde_json::from_str(&normalized).expect("v1 response must be valid JSON");
    let expected = json!({ "jsonrpc": "2.0", "id": golden.id, "result": golden.result });
    assert_eq!(
        parsed, expected,
        "the full JSON-RPC frame (jsonrpc + id + result) must match the golden"
    );

    flat_embedded_resource_guard(raw).unwrap_or_else(|leak| panic!("{leak}"));
}

/// Assert the v2 leg's raw bytes carry `golden.item` verbatim.
fn assert_v2_carries_item(response: &Resp, golden: &EmbeddedGolden<'_>, label: &str) {
    assert_v2_era(response, label);
    assert!(
        response.raw.contains(golden.item),
        "{label}: the v2 response does not carry the spec-derived content item \
         verbatim.\n  expected item: {}\n  {}",
        golden.item,
        wire_break_message(&response.raw)
    );
    flat_embedded_resource_guard(&response.raw).unwrap_or_else(|leak| panic!("{label}: {leak}"));
}

// ===========================================================================
// Fixture values.
// ===========================================================================

/// The tool the fixture server registers. ONE tool, not two: `tools/list` is
/// served from a `HashMap`, whose iteration order `std` randomizes per process,
/// so a second entry would make any list golden non-deterministic. The two
/// tool-result fixtures are selected by an ARGUMENT instead.
const TOOL_NAME: &str = "emb_lookup";

/// `arguments.variant` selecting the plain embedded-resource result.
const VARIANT_PLAIN: &str = "plain";

/// `arguments.variant` selecting the annotated embedded-resource result.
const VARIANT_ANNOTATED: &str = "annotated";

/// The one prompt the fixture server registers, for the same reason.
const PROMPT_NAME: &str = "emb_summarize";

/// Pinned in [`PROMPT_MESSAGE_EMBEDDED`], so it is part of the golden bytes.
const PROMPT_DESCRIPTION: &str = "an embedded resource in a prompt message";

const EMB_URI: &str = "emb://fixture/one.txt";
const EMB_MIME: &str = "text/plain";
const EMB_TEXT: &str = "embedded resource body";

const BLOB_URI: &str = "emb://fixture/pixel.png";
const BLOB_MIME: &str = "image/png";
const BLOB_BASE64: &str = "iVBORw0KGgo=";

const ANN_URI: &str = "emb://fixture/annotated.txt";
const ANN_MIME: &str = "text/plain";
const ANN_TEXT: &str = "annotated embedded body";

// ===========================================================================
// Golden literals — each hand-typed from the vendored schema.
// ===========================================================================

// Derived from schema.ts:1734-1744 (`EmbeddedResource { type: "resource";
// resource: TextResourceContents | BlobResourceContents; annotations?; _meta? }`)
// composed with schema.ts:1514-1527 (`ResourceContents { uri; mimeType?; _meta? }`)
// and schema.ts:1535-1540 (`TextResourceContents extends ResourceContents { text }`).
// Key order follows schema declaration order, which is what `preserve_order`
// makes observable: `type`, then `resource` holding `uri`, `mimeType`, `text`.
const ITEM_EMBEDDED_TEXT: &str = r#"{"type":"resource","resource":{"uri":"emb://fixture/one.txt","mimeType":"text/plain","text":"embedded resource body"}}"#;

/// Golden 1 — `tools/call` whose single content item is an embedded TEXT resource.
///
/// The envelope around the item is `CallToolResult` (`src/types/tools.rs:614`):
/// `content`, then `isError`, which is a plain `bool` with no `skip_serializing_if`
/// and is therefore ALWAYS on the wire. Derived from schema.ts:1734-1744 plus
/// schema.ts:1514-1527 and schema.ts:1535-1540.
const TOOL_RESULT_EMBEDDED: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"resource","resource":{"uri":"emb://fixture/one.txt","mimeType":"text/plain","text":"embedded resource body"}}],"isError":false}}"#;

/// Golden 2 — `prompts/get` whose single message content is the SAME embedded
/// resource, with the SAME nesting.
///
/// `PromptMessage` is `{ role, content: ContentBlock }` (schema.ts:1704-1707) and
/// `ContentBlock` includes `EmbeddedResource` (schema.ts:2305-2306), so the item
/// bytes are identical to golden 1's — which is the property this fixture exists
/// to pin, since the two positions are served by two different dispatch paths.
/// Derived from schema.ts:1734-1744 plus schema.ts:1514-1527 and
/// schema.ts:1535-1540.
const PROMPT_MESSAGE_EMBEDDED: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"description":"an embedded resource in a prompt message","messages":[{"role":"user","content":{"type":"resource","resource":{"uri":"emb://fixture/one.txt","mimeType":"text/plain","text":"embedded resource body"}}}]}}"#;

// Derived from schema.ts:1514-1527 (`ResourceContents`) plus schema.ts:1548-1555
// (`BlobResourceContents extends ResourceContents { blob: string }`).
// **This one is FLAT and stays flat.** `ReadResourceResult.contents` is
// `ResourceContents[]`, which carries NO `type` discriminator — that is the D-01
// boundary: the custom `resource_contents_serde` projection
// (`src/types/content.rs:325`) is already spec-correct here and 118.1-03 must not
// touch it. What is missing today is `blob`, which G-2 adds.
const ITEM_RESOURCES_READ_BINARY: &str =
    r#"{"uri":"emb://fixture/pixel.png","mimeType":"image/png","blob":"iVBORw0KGgo="}"#;

/// Golden 3 — `resources/read` on a BINARY resource.
///
/// `ReadResourceResult._meta` is `skip_serializing_if`, and its absence is part of
/// what this literal pins. Derived from schema.ts:1514-1527 plus schema.ts:1548-1555.
const RESOURCES_READ_BINARY: &str = r#"{"jsonrpc":"2.0","id":3,"result":{"contents":[{"uri":"emb://fixture/pixel.png","mimeType":"image/png","blob":"iVBORw0KGgo="}]}}"#;

// Derived from schema.ts:1734-1744, and specifically from schema.ts:1741 —
// `annotations?: Annotations` is declared on `EmbeddedResource` ITSELF, a sibling
// of `resource`, NOT inside it. `Annotations` (schema.ts:2270-2300) declares
// `audience`, `priority`, `lastModified` in that order; `lastModified` is unset
// here and `skip_serializing_if` keeps it off the wire.
const ITEM_TOOL_RESULT_EMBEDDED_ANNOTATED: &str = r#"{"type":"resource","resource":{"uri":"emb://fixture/annotated.txt","mimeType":"text/plain","text":"annotated embedded body"},"annotations":{"audience":["user"],"priority":0.5}}"#;

/// Golden 4 — the tool-result shape with a content-level `annotations` object.
///
/// The load-bearing claim is placement: `annotations` sits OUTSIDE `resource`.
/// The MCP Apps widget path destructures `{ uri, meta, .. }` at content level
/// (`src/server/core.rs:1000`, `src/server/mod.rs:2533`), so getting this nesting
/// wrong would silently relocate a field two consumers already read. Derived from
/// schema.ts:1734-1744 (`annotations` at schema.ts:1741).
const TOOL_RESULT_EMBEDDED_ANNOTATED: &str = r#"{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"resource","resource":{"uri":"emb://fixture/annotated.txt","mimeType":"text/plain","text":"annotated embedded body"},"annotations":{"audience":["user"],"priority":0.5}}],"isError":false}}"#;

// ===========================================================================
// Fixture content construction.
// ===========================================================================

/// Build a [`Content`] from pmcp's LEGACY FLAT resource JSON.
///
/// # Why the fixtures go through `Deserialize` instead of a constructor
///
/// On the unfixed tree `Content::Resource` has no `blob` field and no
/// `annotations` field, so there is NO typed way to author goldens 3 and 4 at
/// all — `Content::resource_with_text` is the only resource constructor and it
/// carries neither. Deserializing the flat legacy object is the only authoring
/// route that COMPILES today, and it has a second virtue: it exercises D-03's
/// tolerant reader from the flat direction, which is the compatibility affordance
/// 118.1-03 must ship alongside the emitter change.
///
/// Note what this does TODAY: `Content` has no `deny_unknown_fields`, so `blob`
/// and `annotations` are parsed and silently DROPPED. That is precisely why
/// goldens 3 and 4 are red before the fix — the fields never reach the wire
/// because they never reach the type.
///
/// Once 118.1-03 adds the fields plus `Content::resource_with_blob` and a
/// `with_annotations` builder, these helpers may be rewritten to use them, but
/// they must keep producing the same expected bytes.
fn content_from_legacy_flat_json(value: Value) -> Content {
    serde_json::from_value(value)
        .expect("the flat legacy resource shape must deserialize into a Content")
}

fn embedded_text_content() -> Content {
    Content::resource_with_text(EMB_URI, EMB_TEXT, EMB_MIME)
}

fn annotated_content() -> Content {
    content_from_legacy_flat_json(json!({
        "type": "resource",
        "uri": ANN_URI,
        "mimeType": ANN_MIME,
        "text": ANN_TEXT,
        "annotations": { "audience": ["user"], "priority": 0.5 }
    }))
}

fn binary_content() -> Content {
    content_from_legacy_flat_json(json!({
        "type": "resource",
        "uri": BLOB_URI,
        "mimeType": BLOB_MIME,
        "blob": BLOB_BASE64
    }))
}

// ===========================================================================
// Fixtures: the pinned server.
// ===========================================================================

/// The one tool, serving both tool-result fixtures via `arguments.variant`.
///
/// It returns [`ToolOutput::Result`], so the `CallToolResult` it builds reaches
/// the wire VERBATIM — no response middleware, no text-wrap, no widget
/// enrichment. That is deliberate: this file pins `Content`'s serialization, and
/// a fixture whose envelope was synthesized by the dispatch tail would be pinning
/// the tail as well.
struct EmbeddedTool;

impl EmbeddedTool {
    fn envelope(variant: &str) -> pmcp::Result<CallToolResult> {
        match variant {
            VARIANT_PLAIN => Ok(CallToolResult::new(vec![embedded_text_content()])),
            VARIANT_ANNOTATED => Ok(CallToolResult::new(vec![annotated_content()])),
            other => Err(pmcp::Error::invalid_params(format!(
                "unknown variant `{other}`; the fixture serves only `{VARIANT_PLAIN}` and \
                 `{VARIANT_ANNOTATED}`"
            ))),
        }
    }

    fn variant_of(args: &Value) -> String {
        args.get("variant")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
}

#[async_trait]
impl ToolHandler for EmbeddedTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Serialization fallback for non-dispatch callers; the dispatch path uses
        // `handle_output` below.
        Ok(serde_json::to_value(Self::envelope(&Self::variant_of(
            &args,
        ))?)?)
    }

    async fn handle_output(
        &self,
        args: Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ToolOutput> {
        Ok(ToolOutput::Result(Self::envelope(&Self::variant_of(
            &args,
        ))?))
    }
}

/// The one prompt, whose single user message carries the same embedded resource.
struct EmbeddedPrompt;

#[async_trait]
impl PromptHandler for EmbeddedPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(
            vec![PromptMessage::user(embedded_text_content())],
            Some(PROMPT_DESCRIPTION.to_string()),
        ))
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo::new(PROMPT_NAME).with_description(PROMPT_DESCRIPTION))
    }
}

/// A resource handler serving exactly one BINARY resource.
struct BinaryResources;

#[async_trait]
impl ResourceHandler for BinaryResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        assert_eq!(
            uri, BLOB_URI,
            "the fixture only ever reads its one pinned binary URI"
        );
        Ok(ReadResourceResult::new(vec![binary_content()]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![]))
    }
}

/// The fixture server, v2-OPTED-IN so the REQUEST selects the era on both legs.
fn fixture_server() -> Server {
    Server::builder()
        .name("embedded-resource-golden")
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool(TOOL_NAME, EmbeddedTool)
        .prompt(PROMPT_NAME, EmbeddedPrompt)
        .resources(BinaryResources)
        .build()
        .expect("the embedded-resource fixture server builds")
}

// ===========================================================================
// Round trips.
// ===========================================================================

/// One v1 request: bare body, no v2 headers, no reserved `_meta`.
async fn v1_round_trip(id: i64, method: &str, params: Value) -> Resp {
    let (addr, handle) = spawn_stateless_config(fixture_server()).await;
    let response = post(addr, &[], &v1_body(method, json!(id), params)).await;
    teardown(handle, ()).await;
    response
}

/// One v2 request: the three routing headers derived from the body through the
/// PRODUCTION table, plus a well-formed reserved `_meta`.
async fn v2_round_trip(id: i64, method: &str, params: Value) -> Resp {
    let (addr, handle) = spawn_default_config(fixture_server()).await;
    let headers = v2_headers_for(method, &params);
    let response = post(addr, &headers, &v2_body(method, json!(id), params)).await;
    teardown(handle, ()).await;
    response
}

fn tool_call_params(variant: &str) -> Value {
    json!({ "name": TOOL_NAME, "arguments": { "variant": variant } })
}

// ===========================================================================
// The four goldens, each as its own value.
//
// # Why each fixture is TWO tests rather than one with two legs
//
// A single test running v1 then v2 makes the v2 leg UNREACHABLE while the v1 leg
// is red — which is its state for the whole of this plan. The v2 half would then
// have zero executed evidence until 118.1-03 landed, and a v2 leg that never
// reached the server at all (a bad header, a malformed `_meta`, a 400) would look
// exactly like a v2 leg that was simply not run yet. Splitting them makes both
// eras produce their own failure, today.
// ===========================================================================

fn golden_tool_result_embedded() -> EmbeddedGolden<'static> {
    EmbeddedGolden {
        id: 1,
        frame: TOOL_RESULT_EMBEDDED,
        item: ITEM_EMBEDDED_TEXT,
        result: json!({
            "content": [
                {
                    "type": "resource",
                    "resource": {
                        "uri": EMB_URI,
                        "mimeType": EMB_MIME,
                        "text": EMB_TEXT
                    }
                }
            ],
            "isError": false
        }),
        dynamics: NO_DYNAMICS,
    }
}

fn golden_prompt_message_embedded() -> EmbeddedGolden<'static> {
    EmbeddedGolden {
        id: 2,
        frame: PROMPT_MESSAGE_EMBEDDED,
        item: ITEM_EMBEDDED_TEXT,
        result: json!({
            "description": PROMPT_DESCRIPTION,
            "messages": [
                {
                    "role": "user",
                    "content": {
                        "type": "resource",
                        "resource": {
                            "uri": EMB_URI,
                            "mimeType": EMB_MIME,
                            "text": EMB_TEXT
                        }
                    }
                }
            ]
        }),
        dynamics: NO_DYNAMICS,
    }
}

fn golden_resources_read_binary() -> EmbeddedGolden<'static> {
    EmbeddedGolden {
        id: 3,
        frame: RESOURCES_READ_BINARY,
        item: ITEM_RESOURCES_READ_BINARY,
        result: json!({
            "contents": [
                {
                    "uri": BLOB_URI,
                    "mimeType": BLOB_MIME,
                    "blob": BLOB_BASE64
                }
            ]
        }),
        dynamics: NO_DYNAMICS,
    }
}

fn golden_tool_result_embedded_annotated() -> EmbeddedGolden<'static> {
    EmbeddedGolden {
        id: 4,
        frame: TOOL_RESULT_EMBEDDED_ANNOTATED,
        item: ITEM_TOOL_RESULT_EMBEDDED_ANNOTATED,
        result: json!({
            "content": [
                {
                    "type": "resource",
                    "resource": {
                        "uri": ANN_URI,
                        "mimeType": ANN_MIME,
                        "text": ANN_TEXT
                    },
                    "annotations": { "audience": ["user"], "priority": 0.5 }
                }
            ],
            "isError": false
        }),
        dynamics: NO_DYNAMICS,
    }
}

/// Drive one v1 leg: round trip, assert the era, then pin the full frame.
async fn run_v1_leg(golden: &EmbeddedGolden<'_>, method: &str, params: Value, label: &str) {
    let response = v1_round_trip(golden.id, method, params).await;
    assert_eq!(
        response.status, 200,
        "{label} must be served, got HTTP {}: {}",
        response.status, response.raw
    );
    assert_v1_era(&response.raw, label);
    assert_v1_frame_bytes(&response.raw, golden);
}

/// Drive one v2 leg: round trip, assert the era, then pin the content item.
async fn run_v2_leg(golden: &EmbeddedGolden<'_>, method: &str, params: Value, label: &str) {
    let response = v2_round_trip(golden.id, method, params).await;
    assert_v2_carries_item(&response, golden, label);
}

// ===========================================================================
// Golden 1 — tools/call with an embedded text resource.
// ===========================================================================

#[tokio::test]
async fn embedded_resource_golden_tool_result_embedded_v1() {
    run_v1_leg(
        &golden_tool_result_embedded(),
        "tools/call",
        tool_call_params(VARIANT_PLAIN),
        "v1 tools/call (embedded resource)",
    )
    .await;
}

#[tokio::test]
async fn embedded_resource_golden_tool_result_embedded_v2() {
    run_v2_leg(
        &golden_tool_result_embedded(),
        "tools/call",
        tool_call_params(VARIANT_PLAIN),
        "v2 tools/call (embedded resource)",
    )
    .await;
}

// ===========================================================================
// Golden 2 — prompts/get with the same embedded resource.
// ===========================================================================

#[tokio::test]
async fn embedded_resource_golden_prompt_message_embedded_v1() {
    run_v1_leg(
        &golden_prompt_message_embedded(),
        "prompts/get",
        json!({ "name": PROMPT_NAME }),
        "v1 prompts/get (embedded resource)",
    )
    .await;
}

#[tokio::test]
async fn embedded_resource_golden_prompt_message_embedded_v2() {
    run_v2_leg(
        &golden_prompt_message_embedded(),
        "prompts/get",
        json!({ "name": PROMPT_NAME }),
        "v2 prompts/get (embedded resource)",
    )
    .await;
}

// ===========================================================================
// Golden 3 — resources/read on a binary resource. FLAT, and it stays flat: this
// is the D-01 boundary.
// ===========================================================================

#[tokio::test]
async fn embedded_resource_golden_resources_read_binary_v1() {
    run_v1_leg(
        &golden_resources_read_binary(),
        "resources/read",
        json!({ "uri": BLOB_URI }),
        "v1 resources/read (binary)",
    )
    .await;
}

#[tokio::test]
async fn embedded_resource_golden_resources_read_binary_v2() {
    run_v2_leg(
        &golden_resources_read_binary(),
        "resources/read",
        json!({ "uri": BLOB_URI }),
        "v2 resources/read (binary)",
    )
    .await;
}

// ===========================================================================
// Golden 4 — annotations at CONTENT level, outside `resource`.
// ===========================================================================

#[tokio::test]
async fn embedded_resource_golden_tool_result_embedded_annotated_v1() {
    run_v1_leg(
        &golden_tool_result_embedded_annotated(),
        "tools/call",
        tool_call_params(VARIANT_ANNOTATED),
        "v1 tools/call (annotated embedded resource)",
    )
    .await;
}

#[tokio::test]
async fn embedded_resource_golden_tool_result_embedded_annotated_v2() {
    run_v2_leg(
        &golden_tool_result_embedded_annotated(),
        "tools/call",
        tool_call_params(VARIANT_ANNOTATED),
        "v2 tools/call (annotated embedded resource)",
    )
    .await;
}

// ===========================================================================
// Anti-drift — the frame literals and the item literals are one set of bytes.
// ===========================================================================

/// Each full-frame literal EMBEDS its item literal verbatim.
///
/// The v1 leg pins the frame and the v2 leg pins the item, so without this the
/// two legs could silently drift into asserting different shapes under one
/// fixture name. This is a pure literal check with no server, and it PASSES on
/// the unfixed tree.
#[test]
fn embedded_resource_golden_frames_embed_their_items() {
    let pairs: [(&str, &str, &str); 4] = [
        ("tool result", TOOL_RESULT_EMBEDDED, ITEM_EMBEDDED_TEXT),
        (
            "prompt message",
            PROMPT_MESSAGE_EMBEDDED,
            ITEM_EMBEDDED_TEXT,
        ),
        (
            "resources/read binary",
            RESOURCES_READ_BINARY,
            ITEM_RESOURCES_READ_BINARY,
        ),
        (
            "annotated tool result",
            TOOL_RESULT_EMBEDDED_ANNOTATED,
            ITEM_TOOL_RESULT_EMBEDDED_ANNOTATED,
        ),
    ];

    for (label, frame, item) in pairs {
        assert!(
            frame.contains(item),
            "the `{label}` frame literal must contain its item literal verbatim; \
             frame was {frame} and item was {item}"
        );
    }
}

// ===========================================================================
// Anti-vacuity — the leak guard itself.
// ===========================================================================

/// [`flat_embedded_resource_guard`] REJECTS each malformed shape AND ACCEPTS the
/// three legitimate ones.
///
/// This is the negative control D-04 requires. Without it the guard is vacuous in
/// both directions: before 118.1-03 it fires on everything the fixture server
/// emits, and after it should fire on nothing — so no real round trip ever
/// demonstrates that it DISCRIMINATES. The acceptance half is not decoration: a
/// guard that rejected everything would satisfy every rejection case below while
/// failing every real fixture for the wrong reason.
///
/// This test PASSES on the unfixed tree, and it is required to.
#[test]
fn embedded_resource_golden_flat_guard_is_load_bearing() {
    // --- ACCEPT ------------------------------------------------------------
    // The spec-nested shape 118.1-03 will emit.
    const CLEAN_NESTED: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"resource","resource":{"uri":"emb://x","mimeType":"text/plain","text":"t"}}],"isError":false}}"#;
    // A frame with no embedded resource at all.
    const NO_RESOURCE: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"t"}],"isError":false}}"#;
    // The FLAT `ReadResourceResult.contents` projection D-01 keeps. It carries no
    // `type` tag, so the guard must not mistake it for a broken `EmbeddedResource`
    // — if it did, the guard would forbid the very shape the spec requires there.
    const FLAT_CONTENTS: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"contents":[{"uri":"emb://x","mimeType":"image/png","blob":"AAA="}]}}"#;

    // --- REJECT ------------------------------------------------------------
    // (Declared here, before the first statement, because `clippy::pedantic`'s
    // `items_after_statements` is an error under this repo's `-D warnings` gate.)
    //
    // The G-1 defect itself: a flat `uri` at content level.
    const FLAT_AT_CONTENT_LEVEL: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"resource","uri":"emb://x","text":"t","mimeType":"text/plain"}],"isError":false}}"#;
    // Tagged `resource` with neither a nested `resource` nor a flat `uri`.
    const NEITHER_SHAPE: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"resource","mimeType":"text/plain"}],"isError":false}}"#;
    // `annotations` misplaced INSIDE `resource` (schema.ts:1741 puts it outside).
    const ANNOTATIONS_INSIDE: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"resource","resource":{"uri":"emb://x","text":"t","annotations":{"priority":0.5}}}],"isError":false}}"#;
    // The SAME flat defect, SSE-framed. Without this case the guard's SSE branch
    // would be exercised only by the v2 legs, i.e. only where it is expected to
    // fire anyway — so a broken unwrap would read as "no leak found".
    const SSE_FLAT: &str = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"content\":[{\"type\":\"resource\",\"uri\":\"emb://x\",\"text\":\"t\"}],\"isError\":false}}\n\n";
    // A body the guard cannot parse must be an Err, never a silent Ok.
    const UNPARSEABLE: &str = "not json at all";

    for (label, frame) in [
        ("the spec-nested shape", CLEAN_NESTED),
        ("a frame with no embedded resource", NO_RESOURCE),
        ("the flat ReadResourceResult projection", FLAT_CONTENTS),
    ] {
        flat_embedded_resource_guard(frame).unwrap_or_else(|leak| {
            panic!(
                "the guard must ACCEPT {label} — a guard that rejects everything would \
                 satisfy the rejection cases while proving nothing: {leak}"
            )
        });
    }

    let offences: [(&str, &str, &str); 5] = [
        ("flat `uri` at content level", FLAT_AT_CONTENT_LEVEL, "uri"),
        ("neither flat nor nested", NEITHER_SHAPE, "resource"),
        ("annotations inside `resource`", ANNOTATIONS_INSIDE, "1741"),
        ("SSE-framed flat shape", SSE_FLAT, "uri"),
        ("an unparseable body", UNPARSEABLE, "could not parse"),
    ];

    for (label, frame, must_name) in offences {
        let message = flat_embedded_resource_guard(frame).expect_err(&format!(
            "the guard must REJECT {label}; it returned Ok for {frame}"
        ));
        assert!(
            message.contains(must_name),
            "the rejection for {label} must name `{must_name}` so a future reader knows \
             WHAT was wrong, got: {message}"
        );
    }
}
