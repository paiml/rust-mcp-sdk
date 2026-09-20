//! Phase 115-02 (SCHM-03, D-11 / D-13): **byte-identity golden fixtures for the
//! five v1 list/read responses, captured BEFORE any caching-hint field exists.**
//!
//! # Read this before you change a literal in this file
//!
//! A diff in a golden literal here is a **v1 WIRE BREAK**, not a fixture that
//! drifted. These literals were captured from the unmodified tree on
//! 2026-08-01, ahead of plan 115-05 — the plan that adds `ttlMs` and
//! `cacheScope` to `ListToolsResult` and its siblings. If a change you are
//! making turns one of these tests red, the correct response is to make your
//! change **v2-only** — *not* to re-record the golden. Re-recording is exactly
//! the failure D-13 exists to prevent: "the v1 suite still passes" is not
//! byte-identity evidence, because a serde-level reshape is precisely the change
//! that alters bytes while leaving every structural assertion true.
//!
//! # Why a RAW-STRING comparison, and what the ONLY permitted normalization is
//!
//! [`assert_v1_bytes`] compares the **raw response text**, not merely the parsed
//! JSON. A structural comparison of parsed JSON cannot detect
//!
//! * key **order** (this crate builds `serde_json` with `preserve_order` at
//!   `Cargo.toml:55`, so wire order follows struct declaration order and is
//!   observable),
//! * **whitespace**, or
//! * **omission versus explicit null** (`"ttlMs":null` versus no `ttlMs` key at
//!   all),
//!
//! and those three are precisely what a serde-level reshape changes while every
//! structural assertion stays green. CONTEXT.md rejected absence-assertions for
//! this phase for the same reason: an absence assertion proves only the fields
//! you thought to name, and would miss collateral drift in the same response.
//!
//! The ONLY normalization permitted before that comparison is placeholder
//! substitution of genuinely time- or randomness-dependent VALUES. None of the
//! five responses pinned here carries one, so [`NO_DYNAMICS`] is empty and every
//! fixture is compared verbatim, byte for byte.
//!
//! # What these five fixtures are for
//!
//! SCHM-03 adds `ttlMs` / `cacheScope` to the six `CacheableResult` extenders.
//! D-11 says those hints are era-gated OFF on v1, so a v1 response must stay
//! **byte-identical**; D-13 says the pre-change bytes are unrecoverable once the
//! fields land, so they must be captured first. These five fixtures are that
//! capture, taken over real loopback HTTP from a server that is deliberately
//! **not** v2-opted-in — the exact configuration D-11 freezes. D-11 also makes
//! v1 byte-identity the severability precedent for Phases 116-119, so this file
//! outlives Phase 115.
//!
//! # Determinism: why one tool and one prompt, but two resources
//!
//! `tools/list` is served from `HashMap<String, ToolInfo>::values()`
//! (`src/server/mod.rs:1894`) and `prompts/list` from
//! `HashMap<String, Arc<dyn PromptHandler>>::iter()` (`src/server/mod.rs:2234`).
//! `std::collections::HashMap` randomizes its iteration order per process, so a
//! TWO-entry `tools` or `prompts` array is not byte-stable across runs — measured,
//! not assumed. Registering exactly one of each makes those arrays singletons and
//! therefore deterministic, without weakening the comparison by a single byte.
//! Multi-entry array coverage is not lost: `resources/list` is served straight
//! from [`PinnedResources`]'s own fixed `Vec`, so it pins a two-entry array whose
//! order this file owns.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use async_trait::async_trait;
use common::v2::{post, spawn_stateless_config, v1_body, Resp};
use pmcp::server::typed_tool::TypedTool;
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::types::{
    CacheScope, Content, GetPromptResult, ListResourcesResult, PromptArgument, PromptInfo,
    ReadResourceResult, ResourceInfo,
};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

// ===========================================================================
// Dynamic-value normalization.
// ===========================================================================

/// A response value that cannot be pinned because it is minted per run.
///
/// `token` replaces the value in the CANONICAL normalization (the one compared
/// against the golden literal). In the SAME-WIDTH normalization the token is
/// padded with `#` to the value's own byte width, so the normalized string is
/// exactly as long as the raw one — the check that proves the substitution
/// neither adds nor removes bytes and, in particular, never deletes a key.
///
/// Restated from `tests/v1_tasks_golden.rs:82-92` rather than shared: a Rust
/// integration test is its own crate, so the two files cannot import each other.
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

/// None of the five list/read responses mints an id or a timestamp: every value
/// on the wire is one this file's own fixture server chose. So NOTHING is
/// normalized here and these goldens are pinned verbatim, byte for byte.
///
/// The machinery above is kept anyway, and still runs on every call, so the
/// width invariant is an executed no-op rather than an untested claim — and so
/// that a future fixture which *does* need normalization has the instrument
/// already in place instead of reaching for a relaxed comparison.
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
             changed (a v1 wire break) or this fixture is normalizing the wrong key",
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
// The assertion helper.
// ===========================================================================

/// What `_meta` must look like on this frame.
///
/// Every fixture in this file is [`MetaExpectation::Absent`]. The enum exists
/// rather than a bare bool because `ReadResourceResult` genuinely HAS a `_meta`
/// field (`src/types/resources.rs:369-388`, `skip_serializing_if`), so "absent"
/// is a checked property of the captured bytes here, not a structural
/// impossibility. `tests/v1_tasks_golden.rs` carries a second variant for its
/// create envelope; no response pinned here mints one.
enum MetaExpectation {
    /// `_meta` must not appear anywhere in the raw response.
    Absent,
}

/// The failure text the raw-byte comparison carries.
///
/// Factored out so the `assert_eq!` invocation stays on one line: this is the
/// assertion a reviewer greps for when asking "does this file actually compare
/// bytes, or only parsed JSON?", and a macro split across four lines by
/// `rustfmt` answers that question much less clearly.
fn wire_break_message(raw: &str) -> String {
    format!(
        "v1 list/read wire bytes changed. This is a V1 WIRE BREAK, not a stale fixture — \
         make the change v2-only instead of re-recording the golden. Raw response was: {raw}"
    )
}

/// One pinned v1 response.
struct V1Golden<'a> {
    /// The JSON-RPC request id the frame must echo.
    id: i64,
    /// The full frame, byte for byte, after canonical normalization.
    raw: &'a str,
    /// The same frame's `result` payload, for a readable structural failure
    /// message. Every fixture in this file is a success frame; the tasks golden's
    /// `Frame` enum exists there because it pins an error frame too, and there is
    /// no error fixture here to justify carrying the variant.
    result: Value,
    /// Values normalized before comparison (see [`DynamicField`]).
    dynamics: &'a [DynamicField],
    /// The `_meta` rule for this frame.
    meta: MetaExpectation,
}

/// Assert `raw` is byte-identical to `golden` once dynamic values are replaced.
///
/// Four things happen, in this order:
///
/// 1. **Width invariant.** A same-width substitution must leave the length
///    unchanged and every dynamic key's occurrence count unchanged. This is what
///    makes "the normalization never deletes a key" a checked property rather
///    than a comment.
/// 2. **RAW-STRING comparison** against the canonical golden — the load-bearing
///    assertion, and the only one that sees key order, spacing and
///    omission-versus-null.
/// 3. **Structural comparison** of the parsed frame. Note this crate's
///    `serde_json::Map` is an `IndexMap`, whose `PartialEq` is
///    order-INDEPENDENT, so step 3 is genuinely structural — it exists for the
///    readable message, and step 2 is what carries ordering.
/// 4. **v2 leak guards** ([`v1_leak_guard`]): none of `resultType`,
///    `serverInfo`, `ttlMs` or `cacheScope` may appear on a v1 wire, plus the
///    `_meta` rule.
fn assert_v1_bytes(raw: &str, golden: &V1Golden<'_>) {
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
            "the substitution changed how often `{}` appears; it must replace \
             VALUES only and never delete a key: {raw}",
            field.key
        );
    }

    let normalized = substitute(raw, golden.dynamics, false);
    assert_eq!(normalized, golden.raw, "{}", wire_break_message(raw));

    let parsed: Value = serde_json::from_str(&normalized).expect("v1 response must be valid JSON");
    let expected = json!({ "jsonrpc": "2.0", "id": golden.id, "result": golden.result });
    assert_eq!(
        parsed, expected,
        "the full JSON-RPC frame (jsonrpc + id + result) must match the golden"
    );

    v1_leak_guard(raw).unwrap_or_else(|leak| panic!("{leak}"));
    assert_meta(raw, &golden.meta);
}

/// The Phase-112 v2 response-envelope keys. `inject_v2_result_envelope`
/// (`src/server/core.rs:1561`) returns early on any era that is not `V2`, so
/// either of these on a v1 wire means that early return was bypassed.
const V2_ENVELOPE_KEYS: [&str; 2] = ["resultType", "serverInfo"];

/// The Phase-115 SCHM-03 caching-hint keys, added to this guard in plan 115-02 —
/// deliberately BEFORE the fields that would emit them exist.
const V2_CACHING_HINT_KEYS: [&str; 2] = ["ttlMs", "cacheScope"];

/// Reject any v2-only key found on a v1 wire, returning a message naming it.
///
/// # Why this is a function returning `Result`, not an inline `assert!`
///
/// Two of the four keys it checks — [`V2_CACHING_HINT_KEYS`] — cannot appear on
/// ANY wire today, because 115-05 has not yet added them to the six
/// `CacheableResult` extenders. Asserting their absence is therefore vacuous in
/// this plan and stays vacuous until wave 4 lands 115-06's era-gated projection.
/// A vacuous assertion that is never itself exercised is indistinguishable from a
/// mis-wired one, and the moment anybody would find out is the moment it was
/// supposed to catch a real leak. Returning a `Result` lets
/// [`v1_lists_golden_leak_guard_is_load_bearing`] call the guard directly on
/// synthetic leaking frames — no `catch_unwind`, no test-only duplicate of the
/// predicate — so the guard is proven to FIRE on each key and to ACCEPT a clean
/// frame, today, before it has any real work to do.
fn v1_leak_guard(raw: &str) -> Result<(), String> {
    for key in V2_ENVELOPE_KEYS {
        if raw.contains(key) {
            return Err(format!(
                "v1 raw carries the v2 response-envelope key `{key}`. The envelope is \
                 injected only for `Era::V2` (`src/server/core.rs:1561`), so a v1 wire \
                 carrying it means the era gate was bypassed. Raw response was: {raw}"
            ));
        }
    }
    for key in V2_CACHING_HINT_KEYS {
        if raw.contains(key) {
            return Err(format!(
                "v1 raw carries the SCHM-03 caching hint `{key}`. D-11 era-gates the \
                 caching hints OFF on v1, and a v1 response carrying a v2 field breaks \
                 this milestone's severability story: Phases 116-119 all rest on v1 \
                 responses staying byte-identical, so a leak here is not a cosmetic \
                 diff. Emit the hint from the v2 egress projection only — do NOT relax \
                 this guard to make a v1 response accept it. Raw response was: {raw}"
            ));
        }
    }
    Ok(())
}

fn assert_meta(raw: &str, expectation: &MetaExpectation) {
    match expectation {
        MetaExpectation::Absent => assert!(
            !raw.contains("_meta"),
            "this v1 list/read response must carry no _meta: {raw}"
        ),
    }
}

// ===========================================================================
// Fixtures: the pinned server.
// ===========================================================================

/// The one URI [`PinnedResources::read`] serves.
const PINNED_URI: &str = "pin://fixture/one.txt";

/// The single pinned tool. One, not two, for the reason in the module doc:
/// `tools/list` iterates a `HashMap`, so two entries would not be byte-stable.
fn pinned_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "pin_lookup",
        json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        }),
        |_args: Value, _extra| Box::pin(async { Ok(json!({ "hits": 0 })) }),
    )
    .with_description("a fixed tool whose v1 wire shape is pinned by this file")
}

/// The single pinned prompt, carrying one required argument so the nested
/// `arguments` array's key order is pinned too.
struct PinnedPrompt;

#[async_trait]
impl PromptHandler for PinnedPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(vec![], None))
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(
            PromptInfo::new("pin_summarize")
                .with_description("a fixed prompt whose v1 wire shape is pinned by this file")
                .with_arguments(vec![PromptArgument::new("topic")
                    .with_description("the subject to summarize")
                    .required()]),
        )
    }
}

/// A resource handler whose `list` and `read` results are fixed literals.
///
/// This is the one place in the fixture where a MULTI-entry array is byte-stable,
/// because the `Vec` order is this file's own rather than a `HashMap`'s.
struct PinnedResources;

#[async_trait]
impl ResourceHandler for PinnedResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        assert_eq!(
            uri, PINNED_URI,
            "the fixture only ever reads its one pinned URI"
        );
        Ok(ReadResourceResult::new(vec![Content::resource_with_text(
            PINNED_URI,
            "pinned resource body",
            "text/plain",
        )]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo::new(PINNED_URI, "one")
                .with_description("the first pinned resource")
                .with_mime_type("text/plain"),
            ResourceInfo::new("pin://fixture/two.txt", "two")
                .with_description("the second pinned resource")
                .with_mime_type("text/plain"),
        ]))
    }
}

/// The fixture server.
///
/// **The builder's supported-protocol-versions extender is deliberately NEVER
/// called here**, and the name of that method is deliberately absent from this
/// whole file so that a plain `grep` for it is a working detector rather than a
/// hit on a comment. The fixture must be a NOT-OPTED-IN v1 server — the default
/// accept-list — because that is precisely the configuration whose bytes D-11
/// freezes. Extending the accept-list to `2026-07-28` would change what this
/// file proves, since an opted-in server can negotiate the v2 era and the whole
/// point of these five literals is that they are the v1 era's.
fn pinned_server() -> Server {
    Server::builder()
        .name("v1-lists-golden")
        .version("1.0.0")
        .tool("pin_lookup", pinned_tool())
        .prompt("pin_summarize", PinnedPrompt)
        .resources(PinnedResources)
        .build()
        .expect("the pinned v1 fixture server builds")
}

/// Spawn over real loopback HTTP with `enable_json_response: true`, so the raw
/// response text IS the JSON-RPC frame rather than an SSE-framed copy of it.
/// The framing is not what this file pins; the frame is.
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_stateless_config(server).await
}

/// Shut the spawned server down through the shared harness's one teardown order
/// (drop sockets → `abort()` → `await`, D-113-T).
///
/// `()` is passed for the sockets because this file owns none of its own — every
/// request goes through the harness's pooled `reqwest` client.
async fn shutdown(handle: JoinHandle<()>) {
    common::v2::teardown(handle, ()).await;
}

/// A v1 list/read request body: no v2 headers, no reserved `_meta`.
fn lists_body(id: i64, method: &str, params: Value) -> String {
    v1_body(method, json!(id), params)
}

/// POST one v1 request against a freshly spawned pinned server.
async fn round_trip(id: i64, method: &str, params: Value) -> Resp {
    let (addr, handle) = spawn(pinned_server()).await;
    let response = post(addr, &[], &lists_body(id, method, params)).await;
    shutdown(handle).await;
    response
}

// ===========================================================================
// Golden bodies.
// ===========================================================================

/// Captured 2026-08-01 from a real loopback round trip. `ListToolsResult`'s two
/// fields are `tools` then `next_cursor`; the cursor is `skip_serializing_if`, so
/// its absence — not a `"nextCursor":null` — is part of what this literal pins.
const TOOLS_LIST: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"pin_lookup","description":"a fixed tool whose v1 wire shape is pinned by this file","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]}}"#;

/// `PromptArgument::required` is a plain `bool` with `#[serde(default)]` and no
/// `skip_serializing_if`, so it is ALWAYS on the wire — pinned here as `true`.
const PROMPTS_LIST: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"prompts":[{"name":"pin_summarize","description":"a fixed prompt whose v1 wire shape is pinned by this file","arguments":[{"name":"topic","description":"the subject to summarize","required":true}]}]}}"#;

/// The one MULTI-entry array in this file, and the only one whose element order
/// is a stable property of the system rather than of a `HashMap` seed.
const RESOURCES_LIST: &str = r#"{"jsonrpc":"2.0","id":3,"result":{"resources":[{"uri":"pin://fixture/one.txt","name":"one","description":"the first pinned resource","mimeType":"text/plain"},{"uri":"pin://fixture/two.txt","name":"two","description":"the second pinned resource","mimeType":"text/plain"}]}}"#;

/// **The array is empty because the SERVER hardcodes it empty**, not because the
/// fixture declined to register a template. Both dispatchers return
/// `resource_templates: vec![]` unconditionally (`src/server/mod.rs:2463` and
/// `src/server/core.rs:994`); there is no registration API to populate it. That
/// makes this the thinnest of the five results and therefore the one where an
/// injected `ttlMs` / `cacheScope` key is most conspicuous.
const RESOURCE_TEMPLATES_LIST: &str =
    r#"{"jsonrpc":"2.0","id":4,"result":{"resourceTemplates":[]}}"#;

/// Note the element key order: `uri`, `mimeType`, `text`. That is NOT the
/// declaration order of `Content::Resource` (`uri`, `text`, `mimeType`) — the
/// custom `resource_contents_serde` serializer at `src/types/resources.rs:358-362`
/// re-emits the fields and drops the `type` discriminator. A structural assert
/// could not see that; this literal does. `ReadResourceResult._meta` is
/// `skip_serializing_if`, and it is confirmed ABSENT here rather than assumed.
const RESOURCES_READ: &str = r#"{"jsonrpc":"2.0","id":5,"result":{"contents":[{"uri":"pin://fixture/one.txt","mimeType":"text/plain","text":"pinned resource body"}]}}"#;

// ===========================================================================
// Fixture 1 — tools/list.
// ===========================================================================

#[tokio::test]
async fn v1_lists_golden_tools_list() {
    let got = round_trip(1, "tools/list", json!({})).await;

    assert_eq!(got.status, 200, "v1 tools/list must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 1,
            raw: TOOLS_LIST,
            result: json!({
                "tools": [
                    {
                        "name": "pin_lookup",
                        "description": "a fixed tool whose v1 wire shape is pinned by this file",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "query": { "type": "string" } },
                            "required": ["query"]
                        }
                    }
                ]
            }),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 2 — prompts/list.
// ===========================================================================

#[tokio::test]
async fn v1_lists_golden_prompts_list() {
    let got = round_trip(2, "prompts/list", json!({})).await;

    assert_eq!(got.status, 200, "v1 prompts/list must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 2,
            raw: PROMPTS_LIST,
            result: json!({
                "prompts": [
                    {
                        "name": "pin_summarize",
                        "description": "a fixed prompt whose v1 wire shape is pinned by this file",
                        "arguments": [
                            {
                                "name": "topic",
                                "description": "the subject to summarize",
                                "required": true
                            }
                        ]
                    }
                ]
            }),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 3 — resources/list.
// ===========================================================================

#[tokio::test]
async fn v1_lists_golden_resources_list() {
    let got = round_trip(3, "resources/list", json!({})).await;

    assert_eq!(got.status, 200, "v1 resources/list must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 3,
            raw: RESOURCES_LIST,
            result: json!({
                "resources": [
                    {
                        "uri": PINNED_URI,
                        "name": "one",
                        "description": "the first pinned resource",
                        "mimeType": "text/plain"
                    },
                    {
                        "uri": "pin://fixture/two.txt",
                        "name": "two",
                        "description": "the second pinned resource",
                        "mimeType": "text/plain"
                    }
                ]
            }),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 4 — resources/templates/list.
// ===========================================================================

#[tokio::test]
async fn v1_lists_golden_resource_templates_list() {
    let got = round_trip(4, "resources/templates/list", json!({})).await;

    assert_eq!(
        got.status, 200,
        "v1 resources/templates/list must still be served"
    );
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 4,
            raw: RESOURCE_TEMPLATES_LIST,
            result: json!({ "resourceTemplates": [] }),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 5 — resources/read.
// ===========================================================================

#[tokio::test]
async fn v1_lists_golden_resources_read() {
    let got = round_trip(5, "resources/read", json!({ "uri": PINNED_URI })).await;

    assert_eq!(got.status, 200, "v1 resources/read must still be served");
    assert_v1_bytes(
        &got.raw,
        &V1Golden {
            id: 5,
            raw: RESOURCES_READ,
            result: json!({
                "contents": [
                    {
                        "uri": PINNED_URI,
                        "mimeType": "text/plain",
                        "text": "pinned resource body"
                    }
                ]
            }),
            dynamics: NO_DYNAMICS,
            meta: MetaExpectation::Absent,
        },
    );
}

// ===========================================================================
// Fixture 6 — the leak guard against a handler that GENUINELY opted in.
// ===========================================================================

/// A resource handler that SETS both SCHM-03 caching hints on both results.
///
/// Deliberately separate from [`PinnedResources`], which must stay hint-free so
/// the five golden literals above keep pinning the bytes they were captured
/// from.
struct HintedResources;

#[async_trait]
impl ResourceHandler for HintedResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        Ok(ReadResourceResult::new(vec![Content::resource_with_text(
            uri,
            "a hinted resource body",
            "text/plain",
        )])
        .with_ttl_ms(60_000)
        .with_cache_scope(CacheScope::Private))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo::new(PINNED_URI, "one").with_mime_type("text/plain")
        ])
        .with_ttl_ms(300_000)
        .with_cache_scope(CacheScope::Public))
    }
}

/// The v1 fixture server whose resource handler SETS both caching hints.
///
/// Like [`pinned_server`] it is deliberately NOT v2-opted-in — the builder's
/// supported-protocol-versions extender is never called, and its name is absent
/// from this whole file so a plain `grep` for it stays a working detector.
fn hinted_v1_server() -> Server {
    Server::builder()
        .name("v1-lists-golden-hinted")
        .version("1.0.0")
        .resources(HintedResources)
        .build()
        .expect("the hinted v1 fixture server builds")
}

/// **This is the fixture that makes [`v1_leak_guard`] load-bearing on the wire.**
///
/// Until plan 115-05 the `ttlMs` / `cacheScope` half of that guard was VACUOUS:
/// the fields did not exist on any result type, so "the key is absent" was true
/// of a guard that works and equally true of one wired to the wrong string.
/// [`v1_lists_golden_leak_guard_is_load_bearing`] closed half of that gap by
/// driving synthetic frames through the predicate directly. This closes the
/// other half — a REAL v1 round trip against a handler that genuinely called
/// `with_ttl_ms` and `with_cache_scope`, where the only thing standing between
/// those values and the v1 wire is the era-gated projection itself.
///
/// No golden literal is pinned here on purpose. The five above pin the bytes of
/// the hint-FREE fixture, which is what D-13 required captured before the
/// fields landed; this sixth fixture uses a different server, so pinning its
/// bytes would be pinning something that was never captured pre-change. What it
/// asserts is the guard, which is the property D-11 actually needs.
#[tokio::test]
async fn v1_lists_golden_handler_set_hints_never_reach_the_v1_wire() {
    for (id, method, params) in [
        (6_i64, "resources/list", json!({})),
        (7, "resources/read", json!({ "uri": PINNED_URI })),
    ] {
        let (addr, handle) = spawn(hinted_v1_server()).await;
        let got = post(addr, &[], &lists_body(id, method, params)).await;
        shutdown(handle).await;

        assert_eq!(got.status, 200, "v1 {method} must still be served");
        v1_leak_guard(&got.raw).unwrap_or_else(|leak| {
            panic!(
                "the handler SET both hints and this is a v1 wire, so the era-gated \
                 projection must have stripped them: {leak}"
            )
        });
        assert_meta(&got.raw, &MetaExpectation::Absent);
    }
}

// ===========================================================================
// Anti-vacuity — the leak guard itself.
// ===========================================================================

/// [`v1_leak_guard`] fires on each of its four keys AND accepts a clean frame.
///
/// The five fixtures above pass their leak guard today for a reason that has
/// nothing to do with the guard being correct: `ttlMs` and `cacheScope` do not
/// exist on any result type yet, so "the key is absent" is true of a guard that
/// works and equally true of a guard that was wired to the wrong string, or that
/// returns `Ok(())` unconditionally. This test removes that ambiguity by driving
/// synthetic leaking frames through the guard directly.
///
/// The clean-frame case is the other half and is not decoration: a guard that
/// rejected EVERYTHING would satisfy the four leak cases perfectly while failing
/// every real fixture for the wrong reason. Discrimination is the property under
/// test, not rejection.
#[test]
fn v1_lists_golden_leak_guard_is_load_bearing() {
    const CLEAN: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[],"nextCursor":"c"}}"#;

    v1_leak_guard(CLEAN).expect(
        "a clean v1 frame must PASS the guard — a guard that rejects everything \
         would satisfy the leak cases below while proving nothing",
    );

    // (key, a synthetic raw frame carrying it, whether its branch must cite D-11)
    let leaks: [(&str, &str, bool); 4] = [
        (
            "ttlMs",
            r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[],"ttlMs":0}}"#,
            true,
        ),
        (
            "cacheScope",
            r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[],"cacheScope":"private"}}"#,
            true,
        ),
        (
            "resultType",
            r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[],"resultType":"complete"}}"#,
            false,
        ),
        (
            "serverInfo",
            r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[],"serverInfo":{"name":"x"}}}"#,
            false,
        ),
    ];

    for (key, frame, cites_d11) in leaks {
        let message = v1_leak_guard(frame).expect_err(&format!(
            "the guard must REJECT a v1 frame carrying `{key}`; it returned Ok for {frame}"
        ));
        assert!(
            message.contains(key),
            "the rejection message must NAME the offending key `{key}` so a future \
             reader knows which field leaked, got: {message}"
        );
        assert_eq!(
            message.contains("D-11"),
            cites_d11,
            "`{key}` must be reported by the {} branch, whose message {} cite D-11; got: {message}",
            if cites_d11 {
                "caching-hint"
            } else {
                "v2-envelope"
            },
            if cites_d11 { "must" } else { "must not" }
        );
    }
}
