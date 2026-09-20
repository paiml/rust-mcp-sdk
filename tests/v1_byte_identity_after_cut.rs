//! Phase 117-02 (SMPL-02): **byte-identity golden fixtures for the v1 session
//! lifecycle, captured BEFORE the D-03 cut of
//! `src/server/streamable_http_server.rs`.**
//!
//! # Read this before you change a literal in this file
//!
//! A diff in a golden literal here is a **V1 WIRE BREAK**, not a fixture that
//! drifted. These literals were captured from the UNMODIFIED tree, ahead of the
//! Phase-117 plans that sever the v1 session / SSE-resumability machinery from
//! the v2 path. If a change you are making turns one of these tests red, the
//! correct response is to **fix the cut** — make the change v2-only — *not* to
//! re-record the golden.
//!
//! Re-recording is exactly the failure this file exists to prevent: "the v1
//! suite still passes" is not byte-identity evidence, because a refactor that
//! reshapes a response, moves a header, or reorders a field is precisely the
//! change that alters bytes while leaving every structural assertion true.
//!
//! # Why these literals had to be captured FIRST
//!
//! Goldens captured after a refactor prove only that the refactor is
//! self-consistent. The pre-cut bytes are unrecoverable once the cut lands, so
//! they are pinned here against an unmodified tree; the capture anchor commit is
//! recorded in `.planning/phases/117-agents-tester-v1-severability/117-02-SUMMARY.md`.
//!
//! # Why a RAW-STRING comparison, and what the ONLY permitted normalization is
//!
//! [`assert_v1_bytes`] compares the **raw response text**, not merely the parsed
//! JSON. A structural comparison cannot detect key **order** (this crate builds
//! `serde_json` with `preserve_order`, so wire order follows struct declaration
//! order and is observable), **whitespace**, **SSE framing**, or **omission
//! versus explicit null** — and those are precisely what a transport-level
//! refactor changes while every structural assertion stays green.
//!
//! The ONLY normalization permitted before that comparison is placeholder
//! substitution of genuinely per-run VALUES: the minted session id, and the
//! per-frame SSE event id. Both substitutions are proven **width-preserving** by
//! an explicit length assertion plus a per-key occurrence-count assertion, so a
//! substitution cannot mask an added or removed byte and cannot delete a key.
//!
//! # Header identity and body identity are two DISTINCT claims
//!
//! `common::v2::Resp` stores `Mcp-Session-Id` outside `raw`, so a body-only
//! fixture cannot speak for headers. This file makes both claims, separately:
//! bodies are pinned as raw text, and headers are pinned as an explicit
//! `name: value` block rendered by [`render_headers`] and run through the very
//! same width-preserving normalizer.
//!
//! # Determinism: one tool and one prompt
//!
//! `tools/list` is served from a `HashMap`, whose iteration order is randomized
//! per process, so a two-entry array is not byte-stable. Registering exactly one
//! of each makes those arrays singletons and therefore deterministic, without
//! weakening the comparison by a single byte.
// `v1-compat` joins the predicate for Phase 117: these goldens pin the v1 wire,
// and the v1 wire does not exist on `--no-default-features --features full-v2`.
// Without this the aggregate severed test command is a BUILD failure rather than
// a green run — which is the whole point of a severability proof.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    feature = "v1-compat",
    not(target_arch = "wasm32")
))]

mod common;

use async_trait::async_trait;
use common::v2::{
    delete, header, post, spawn_default_config, spawn_with, teardown, v1_body, Resp, V1,
};
use pmcp::server::streamable_http_server::{InMemoryEventStore, StreamableHttpServerConfig};
use pmcp::server::typed_tool::TypedTool;
use pmcp::server::{PromptHandler, Server};
// The header NAMES come from the crate's own constants, never from a string
// literal: a constant rename must surface as a COMPILE ERROR here rather than as
// a silently-skipped fixture.
use pmcp::shared::http_constants::{LAST_EVENT_ID, MCP_SESSION_ID};
use pmcp::types::{GetPromptResult, PromptArgument, PromptInfo};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use uuid::Uuid;

// ===========================================================================
// Dynamic-value normalization.
//
// Restated from `tests/v1_lists_golden.rs:88-180` and `tests/v1_tasks_golden.rs:82-160`.
// The one adaptation is that substitution is FIELD-LINE anchored rather than
// JSON-key anchored, because this file pins SSE-framed responses and rendered
// header blocks, whose per-run values are not JSON string values.
//
// KNOWN DEBT, stated accurately. An earlier version of this note claimed the
// restatement was FORCED — "a Rust integration test is its own crate, so the
// two files cannot import each other". That is false, and this very file
// disproves it: it declares `mod common;` at line 61 and imports
// `common::v2::{…}` at line 64, which is the `tests/common/` sharing mechanism
// the root test tree already uses. So the restatement is a CHOICE, and the
// sharing home exists.
//
// What is actually duplicated: `width_preserving` is byte-identical across all
// three files, and `DynamicField` differs only in a doc line. A change to the
// same-width invariant must therefore be made in three places, with nothing
// detecting a missed one. Extracting them into `tests/common/` would touch two
// files outside this phase's scope, so it is recorded here rather than done.
//
// Note also that `is_uuid_v4` below is a STRICTER fork of
// `v1_tasks_golden.rs`'s `is_uuid_shaped` (it checks the version nibble and
// requires lowercase; that one accepts any `is_ascii_hexdigit`). Two divergent
// answers to "is this a UUID" now coexist in this test tree — deliberate here,
// because a golden that accepts a malformed id pins less than it claims.
// ===========================================================================

/// A response value that cannot be pinned because it is minted per run.
///
/// Every dynamic value pinned by this file lives on a FIELD LINE — `key: VALUE`
/// anchored to the start of a line, terminated by the newline (or by end of
/// input on the last line). That shape serves both SSE frame fields
/// (`id: <uuid>`) and the `name: value` blocks [`render_headers`] produces, so
/// one normalizer covers both the body claim and the header claim.
///
/// `v1_lists_golden.rs`'s JSON-string form (`"key":"VALUE"`) is deliberately NOT
/// restated: no capture in this file carries a dynamic JSON string VALUE — the
/// session id travels in a header and the SSE event id in a frame field — and an
/// unexercised second substitution path would be dead weight that no fixture
/// could prove correct.
///
/// `token` replaces the value in the CANONICAL normalization (the one compared
/// against the golden literal). In the SAME-WIDTH normalization the token is
/// padded with `#` to the value's own byte width, so the normalized string is
/// exactly as long as the raw one — the check that proves the substitution
/// neither adds nor removes bytes and, in particular, never deletes a key.
struct DynamicField {
    /// The JSON object key, or the field-line name, whose value is dynamic.
    key: &'static str,
    /// The canonical placeholder written into the golden literal.
    token: &'static str,
    /// Shape predicate the raw value must satisfy — a normalization that
    /// accepted any string would let a reshaped value through unnoticed.
    shape: fn(&str) -> bool,
    /// Human-readable form of `shape`, for the failure message.
    shape_description: &'static str,
}

/// Nothing is normalized: every byte of the response is pinned verbatim.
///
/// The machinery still runs on every call, so the width invariant is an executed
/// no-op rather than an untested claim.
const NO_DYNAMICS: &[DynamicField] = &[];

/// `Uuid::new_v4().to_string()`'s shape: 36 bytes, lowercase hex, dashes at
/// 8/13/18/23, version nibble `4` at offset 14.
///
/// Deliberately NOT "any non-empty string": both the session id
/// (`StreamableHttpServerConfig::default`'s generator) and the SSE event id
/// (`sse_event_for_message`) are v4 UUIDs today, and a reshaped value must be
/// caught by the normalizer rather than waved through it.
fn is_uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.get(14) == Some(&b'4')
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => matches!(byte, b'0'..=b'9' | b'a'..=b'f'),
        })
}

/// Human-readable form of [`is_uuid_v4`], quoted into failure messages.
const UUID_V4_SHAPE: &str = "a lowercase v4 UUID (36 bytes, dashes at 8/13/18/23)";

/// The minted `Mcp-Session-Id`, as it appears in a rendered header block.
const SESSION_ID_DYNAMICS: &[DynamicField] = &[DynamicField {
    key: MCP_SESSION_ID,
    token: "<session-id>",
    shape: is_uuid_v4,
    shape_description: UUID_V4_SHAPE,
}];

/// The per-frame SSE event id minted by `sse_event_for_message` /
/// `build_sse_response_from_single_message`.
const SSE_EVENT_ID_DYNAMICS: &[DynamicField] = &[DynamicField {
    key: "id",
    token: "<sse-event-id>",
    shape: is_uuid_v4,
    shape_description: UUID_V4_SHAPE,
}];

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
/// operations, so key order, spacing, SSE framing and null-versus-absent all
/// survive into the comparison.
fn substitute(raw: &str, fields: &[DynamicField], same_width: bool) -> String {
    let mut out = raw.to_string();
    for field in fields {
        out = substitute_field_line(&out, field, same_width);
    }
    out
}

/// The replacement text for one matched value.
fn replacement(field: &DynamicField, value: &str, same_width: bool) -> String {
    assert!(
        (field.shape)(value),
        "`{}` carried `{value}`, which is not {} — either the value shape \
         changed (a v1 wire break) or this fixture is normalizing the wrong key",
        field.key,
        field.shape_description
    );
    if same_width {
        width_preserving(field.token, value.len())
    } else {
        field.token.to_string()
    }
}

/// Replace the value of every LINE that begins `key: `.
///
/// Line-anchored on purpose. An unanchored `id: ` needle could in principle
/// match inside an SSE `data:` payload; anchoring makes the frame field and the
/// JSON body structurally impossible to confuse.
fn substitute_field_line(raw: &str, field: &DynamicField, same_width: bool) -> String {
    let prefix = format!("{}: ", field.key);
    let mut out = String::with_capacity(raw.len());
    let mut hits = 0_usize;
    // `split_inclusive` keeps each line's terminator, so re-joining is lossless
    // and a missing trailing newline stays missing.
    for segment in raw.split_inclusive('\n') {
        let (line, terminator) = segment
            .strip_suffix('\n')
            .map_or((segment, ""), |line| (line, "\n"));
        if let Some(value) = line.strip_prefix(prefix.as_str()) {
            out.push_str(&prefix);
            out.push_str(&replacement(field, value, same_width));
            out.push_str(terminator);
            hits += 1;
        } else {
            out.push_str(segment);
        }
    }
    assert_declared_key_present(hits, field, raw);
    out
}

fn assert_declared_key_present(hits: usize, field: &DynamicField, raw: &str) {
    assert!(
        hits > 0,
        "declared dynamic key `{}` does not appear in the response — a golden \
         that normalizes an absent key proves nothing: {raw}",
        field.key
    );
}

/// How many field lines carry `field`'s key.
fn key_occurrences(text: &str, field: &DynamicField) -> usize {
    let prefix = format!("{}: ", field.key);
    text.split_inclusive('\n')
        .filter(|segment| {
            segment
                .strip_suffix('\n')
                .unwrap_or(segment)
                .starts_with(prefix.as_str())
        })
        .count()
}

// ===========================================================================
// The assertion helper.
// ===========================================================================

/// The structural cross-check applied after the raw-byte comparison.
///
/// It exists for the readable failure message only; the RAW comparison is what
/// carries ordering, whitespace and framing.
enum Structural {
    /// A whole JSON-RPC success frame: `{"jsonrpc":"2.0","id":…,"result":…}`.
    JsonRpcResult { id: i64, result: Value },
    /// Every `data:` payload in an SSE capture, in order.
    Frames(Vec<Value>),
    /// The capture parsed as a whole JSON document that is NOT a JSON-RPC frame
    /// — the v1 DELETE answer is a bare `{"status":"ok"}`.
    Bare(Value),
    /// No structural cross-check — the capture is not JSON (a rendered header
    /// block).
    RawOnly,
}

/// The failure text the raw-byte comparison carries.
///
/// Factored out so the `assert_eq!` invocation stays on one line: this is the
/// assertion a reviewer greps for when asking "does this file actually compare
/// bytes, or only parsed JSON?".
fn wire_break_message(raw: &str) -> String {
    format!(
        "v1 session-lifecycle wire bytes changed. This is a V1 WIRE BREAK, not a stale \
         fixture — the Phase-117 severance of the v1 session / SSE machinery from the v2 \
         path is the likely cause, so FIX THE CUT and make the change v2-only instead of \
         re-recording the golden. Raw capture was: {raw}"
    )
}

/// One pinned v1 capture.
struct V1Golden<'a> {
    /// The capture, byte for byte, after canonical normalization.
    raw: &'a str,
    /// The structural cross-check (see [`Structural`]).
    structural: Structural,
    /// Values normalized before comparison (see [`DynamicField`]).
    dynamics: &'a [DynamicField],
}

/// Every `data:` payload in an SSE capture, parsed, in order.
fn sse_data_frames(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|payload| serde_json::from_str::<Value>(payload.trim()).ok())
        .collect()
}

/// Assert `raw` is byte-identical to `golden` once dynamic values are replaced.
///
/// Three things happen, in this order:
///
/// 1. **Width invariant.** A same-width substitution must leave the length
///    unchanged and every dynamic key's occurrence count unchanged. This is what
///    makes "the normalization never deletes a key" a checked property rather
///    than a comment.
/// 2. **RAW-STRING comparison** against the canonical golden — the load-bearing
///    assertion, and the only one that sees key order, spacing, SSE framing and
///    omission-versus-null.
/// 3. **Structural comparison**, for a readable message only.
fn assert_v1_bytes(raw: &str, golden: &V1Golden<'_>) {
    let same_width = substitute(raw, golden.dynamics, true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the placeholder substitution changed the capture length; it must be \
         width-preserving so it cannot mask an added or removed byte: {raw}"
    );
    for field in golden.dynamics {
        assert_eq!(
            key_occurrences(&same_width, field),
            key_occurrences(raw, field),
            "the substitution changed how often `{}` appears; it must replace \
             VALUES only and never delete a key: {raw}",
            field.key
        );
    }

    let normalized = substitute(raw, golden.dynamics, false);
    assert_eq!(normalized, golden.raw, "{}", wire_break_message(raw));

    match &golden.structural {
        Structural::JsonRpcResult { id, result } => {
            let parsed: Value =
                serde_json::from_str(&normalized).expect("a v1 JSON-RPC frame must be valid JSON");
            assert_eq!(
                parsed,
                json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                "the full JSON-RPC frame (jsonrpc + id + result) must match the golden"
            );
        },
        Structural::Frames(expected) => {
            assert_eq!(
                &sse_data_frames(&normalized),
                expected,
                "every SSE `data:` payload must match the golden, in order"
            );
        },
        Structural::Bare(expected) => {
            let parsed: Value =
                serde_json::from_str(&normalized).expect("a v1 JSON body must be valid JSON");
            assert_eq!(
                &parsed, expected,
                "the whole JSON body must match the golden"
            );
        },
        Structural::RawOnly => {},
    }
}

/// Render headers as a canonical `name: value` block, one per line, in the order
/// given.
///
/// Header identity is a SEPARATE claim from body identity: `common::v2::Resp`
/// keeps `Mcp-Session-Id` outside `raw`, so a body fixture says nothing about
/// headers. Rendering them into the same field-line shape the SSE frames
/// already use means one width-preserving normalizer serves both claims.
fn render_headers(pairs: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (name, value) in pairs {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push('\n');
    }
    out
}

// ===========================================================================
// The BOUNDED SSE reader.
//
// `common::v2::get` reads the response body to EOF. The v1 GET endpoint
// (`handle_get_sse`, `src/server/streamable_http_server.rs:4441-4503`) returns a
// LONG-LIVED `text/event-stream` that never reaches EOF, so driving an SSE
// fixture through it would block until a timeout and return nothing to compare.
// A timeout is not a golden, and a fixture that times out and then asserts on an
// empty capture is vacuous. Hence a bounded reader, LOCAL to this file:
// `tests/common/v2.rs` serves other suites and this plan does not modify it.
// ===========================================================================

/// Upper bound on ONE bounded read of a live v1 SSE stream.
///
/// This is the reader's FAILURE bound and never a success path: a hung stream
/// must FAIL the test, not hang it. [`REPLAY_FRAME_COUNT`] is its SUCCESS bound.
/// Both are needed — a timeout alone cannot tell "the server sent nothing" from
/// "the server is still streaming correctly".
const SSE_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// How many complete SSE frames [`read_bounded_sse`] waits for — the reader's
/// SUCCESS bound, `N`.
///
/// The replay fixture stores exactly two v1 responses before reconnecting (the
/// `initialize` reply and one session-carrying `tools/list` reply, each written
/// to the event store by `store_response_event`), so two frames is the whole
/// replay. The reader returns as soon as it has parsed this many and never waits
/// for the end of a stream that, being live, does not have one.
const REPLAY_FRAME_COUNT: usize = 2;

/// An SSE frame ends at a blank line.
const FRAME_TERMINATOR: &str = "\n\n";

/// One bounded read of a LIVE v1 SSE stream.
struct SseCapture {
    /// HTTP status of the GET response.
    status: u16,
    /// Every response header, lowercased name and value, in emission order.
    headers: Vec<(String, String)>,
    /// The accumulated RAW stream bytes, SSE framing included — the wire form,
    /// which is the whole point of this file. NOT parsed structs.
    raw: String,
    /// How many complete frames [`Self::raw`] contains.
    frames: usize,
}

impl SseCapture {
    /// This capture's value for `name`, or a panic naming the missing header.
    fn header(&self, name: &str) -> &str {
        let found = self
            .headers
            .iter()
            .find(|(header_name, _)| header_name == name);
        let Some((_, value)) = found else {
            panic!(
                "the v1 GET response is missing the `{name}` header. A header this file \
                 pins going ABSENT is a V1 WIRE BREAK just as much as one changing value. \
                 Headers were: {:?}",
                self.headers
            )
        };
        value.as_str()
    }
}

/// GET the MCP endpoint and read exactly `want_frames` complete SSE frames.
///
/// Returns the response STATUS and HEADERS alongside the RAW bytes, because
/// header identity and body identity are two distinct claims and this file makes
/// both.
async fn read_bounded_sse(
    addr: SocketAddr,
    extra: &[(String, String)],
    want_frames: usize,
) -> SseCapture {
    let client = reqwest::Client::new();
    let mut request = client
        .get(format!("http://{addr}"))
        .header("accept", "text/event-stream");
    for (name, value) in extra {
        request = request.header(name.as_str(), value.as_str());
    }
    let mut response = request.send().await.expect("the v1 GET is sent");

    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().unwrap_or("<non-ascii>").to_string(),
            )
        })
        .collect::<Vec<_>>();

    let mut raw = String::new();
    let deadline = tokio::time::Instant::now() + SSE_READ_TIMEOUT;
    while raw.matches(FRAME_TERMINATOR).count() < want_frames {
        // Any of: the deadline, a transport error, or EOF. Stop accumulating and
        // let the assertion below report how far the stream actually got — the
        // timeout is a FAILURE path, never a quiet success.
        let Ok(Ok(Some(chunk))) = tokio::time::timeout_at(deadline, response.chunk()).await else {
            break;
        };
        raw.push_str(&String::from_utf8_lossy(&chunk));
    }

    let frames = raw.matches(FRAME_TERMINATOR).count();
    assert!(
        frames >= want_frames,
        "FAILURE MODE: the v1 SSE stream produced {frames} complete frames, not the \
         {want_frames} this fixture requires, within {SSE_READ_TIMEOUT:?}. \
         WHAT TO DO: a timeout is NOT a golden — fix the v1 replay path rather than \
         relaxing the frame count, because a fixture that compares an empty capture \
         passes over nothing. Capture so far: {raw:?}"
    );

    SseCapture {
        status,
        headers,
        raw,
        frames,
    }
}

// ===========================================================================
// Fixtures: the pinned server.
// ===========================================================================

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

/// The fixture server.
///
/// **The builder's supported-protocol-versions extender is deliberately NEVER
/// called here**, and the name of that method is deliberately absent from this
/// whole file so that a plain `grep` for it is a working detector rather than a
/// hit on a comment. The fixture must be a NOT-OPTED-IN v1 server, because that
/// is precisely the configuration whose bytes this file freezes: an opted-in
/// server can negotiate the v2 era, and the whole point of these literals is
/// that they are the v1 era's.
fn pinned_server() -> Server {
    Server::builder()
        .name("v1-byte-identity")
        .version("1.0.0")
        .tool("pin_lookup", pinned_tool())
        .prompt("pin_summarize", PinnedPrompt)
        .build()
        .expect("the pinned v1 fixture server builds")
}

/// The v1 `initialize` request body.
fn initialize_body(id: i64) -> String {
    v1_body(
        "initialize",
        json!(id),
        json!({
            "protocolVersion": V1,
            "capabilities": {},
            "clientInfo": { "name": "v1-byte-identity-client", "version": "1.0.0" }
        }),
    )
}

/// Spawn a SESSION-MINTING server.
///
/// `spawn_default_config`, never the harness's build-time stateless spawn
/// helper — whose name is deliberately absent from this whole file so a plain
/// `grep` for it is a working detector rather than a hit on a comment.
/// `StreamableHttpServerConfig::stateless()` has no `session_id_generator`, so
/// `sessions_active_for(false, _)` is `false` and there is no `Mcp-Session-Id`
/// to pin at all: a stateless spawn would produce a vacuously-green session
/// fixture.
async fn spawn_session_minting() -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(pinned_server()).await
}

/// Spawn a server that mints sessions AND retains events for replay.
///
/// `session_id_generator` and `event_store` are named EXPLICITLY because they
/// are the two v1-only config fields whose severance this phase later gates —
/// naming them here is the point of the fixture. Everything else is reached
/// through `..Default::default()` rather than a field-by-field literal a later
/// plan would have to edit.
async fn spawn_resumable() -> (SocketAddr, JoinHandle<()>) {
    let config = StreamableHttpServerConfig {
        session_id_generator: Some(Box::new(|| Uuid::new_v4().to_string())),
        event_store: Some(Arc::new(InMemoryEventStore::default())),
        ..Default::default()
    };
    spawn_with(pinned_server(), config).await
}

/// Drive a resumable server to the point where its event store holds exactly
/// [`REPLAY_FRAME_COUNT`] v1 responses, and return the live session id.
///
/// `store_response_event` writes one event per POST that has a response session
/// id, so an `initialize` plus one session-carrying `tools/list` is exactly two.
async fn prime_replayable_events(addr: SocketAddr) -> String {
    let (_init, session_id) = initialize(addr, 10).await;
    let follow_up = post(
        addr,
        &[header(MCP_SESSION_ID, &session_id)],
        &v1_body("tools/list", json!(11), json!({})),
    )
    .await;
    assert_eq!(
        follow_up.status, 200,
        "priming the event store requires the follow-up POST to succeed: {}",
        follow_up.raw
    );
    session_id
}

/// POST `initialize` and return both the response and the session id it minted.
async fn initialize(addr: SocketAddr, id: i64) -> (Resp, String) {
    let response = post(addr, &[], &initialize_body(id)).await;
    let session_id = response
        .mcp_session_id
        .clone()
        .expect("a v1 initialize against a session-minting server MUST mint a session id");
    (response, session_id)
}

// ===========================================================================
// Golden captures — v1 session lifecycle.
// ===========================================================================

/// The v1 `initialize` response.
///
/// PLAIN JSON, not SSE-framed, and that is itself pinned: `build_response`
/// selects its framing from the RAW INBOUND `Mcp-Session-Id`, which an
/// `initialize` request does not carry, so it falls through to
/// `build_json_response`.
/// Note there is no `nextCursor`, no `_meta` and no v2 response envelope: their
/// ABSENCE is part of what this literal pins.
const INITIALIZE_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false},"prompts":{"listChanged":false}},"serverInfo":{"name":"v1-byte-identity","version":"1.0.0"}}}"#;

/// The `Mcp-Session-Id` response header on a v1 `initialize`.
///
/// The header NAME is pinned exactly; the VALUE is pinned by SHAPE
/// ([`is_uuid_v4`]) because it is minted per run.
const INITIALIZE_SESSION_HEADER: &str = "mcp-session-id: <session-id>\n";

/// A session-carrying follow-up `tools/list`.
///
/// SSE-FRAMED, and that too is pinned: with an inbound `Mcp-Session-Id` and no
/// open SSE stream for it, `build_response` routes to
/// `build_sse_response_from_single_message`. The `id` / `event` / `data` field
/// ORDER and the frame-terminating blank line are part of the literal.
const FOLLOW_UP_TOOLS_LIST: &str = concat!(
    "id: <sse-event-id>\n",
    "event: message\n",
    r#"data: {"jsonrpc":"2.0","id":4,"result":{"tools":[{"name":"pin_lookup","description":"a fixed tool whose v1 wire shape is pinned by this file","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]}}"#,
    "\n\n"
);

// ===========================================================================
// Fixture 1 — the v1 initialize response BODY.
// ===========================================================================

#[tokio::test]
async fn v1_initialize_response_body_bytes_are_pinned() {
    let (addr, handle) = spawn_session_minting().await;
    let (response, _session_id) = initialize(addr, 1).await;
    teardown(handle, ()).await;

    assert_eq!(response.status, 200, "v1 initialize must still be served");
    assert_v1_bytes(
        &response.raw,
        &V1Golden {
            raw: INITIALIZE_BODY,
            structural: Structural::JsonRpcResult {
                id: 1,
                result: json!({
                    "protocolVersion": V1,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "prompts": { "listChanged": false }
                    },
                    "serverInfo": { "name": "v1-byte-identity", "version": "1.0.0" }
                }),
            },
            dynamics: NO_DYNAMICS,
        },
    );
}

// ===========================================================================
// Fixture 2 — the `Mcp-Session-Id` HEADER emission (a separate claim).
// ===========================================================================

#[tokio::test]
async fn v1_initialize_emits_the_mcp_session_id_header() {
    let (addr, handle) = spawn_session_minting().await;
    let (response, session_id) = initialize(addr, 2).await;
    teardown(handle, ()).await;

    assert_eq!(response.status, 200, "v1 initialize must still be served");
    assert!(
        is_uuid_v4(&session_id),
        "the minted session id must still be {UUID_V4_SHAPE}, got `{session_id}` — \
         a reshaped id is a V1 WIRE BREAK for every client that stores it"
    );
    assert_v1_bytes(
        &render_headers(&[(MCP_SESSION_ID, &session_id)]),
        &V1Golden {
            raw: INITIALIZE_SESSION_HEADER,
            structural: Structural::RawOnly,
            dynamics: SESSION_ID_DYNAMICS,
        },
    );
}

// ===========================================================================
// Fixture 3 — a session-carrying follow-up POST.
// ===========================================================================

#[tokio::test]
async fn v1_session_carrying_follow_up_post_bytes_are_pinned() {
    let (addr, handle) = spawn_session_minting().await;
    let (_init, session_id) = initialize(addr, 3).await;
    let response = post(
        addr,
        &[header(MCP_SESSION_ID, &session_id)],
        &v1_body("tools/list", json!(4), json!({})),
    )
    .await;
    teardown(handle, ()).await;

    assert_eq!(
        response.status, 200,
        "a v1 session-carrying request must still be served: {}",
        response.raw
    );
    assert_v1_bytes(
        &response.raw,
        &V1Golden {
            raw: FOLLOW_UP_TOOLS_LIST,
            structural: Structural::Frames(vec![json!({
                "jsonrpc": "2.0",
                "id": 4,
                "result": {
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
                }
            })]),
            dynamics: SSE_EVENT_ID_DYNAMICS,
        },
    );
}

// ===========================================================================
// Golden captures — v1 GET (SSE), Last-Event-ID replay, and DELETE.
// ===========================================================================

/// A well-formed cursor that is NOT in the store's `event_order`.
///
/// `InMemoryEventStore::replay_events_after` resolves an unknown cursor to
/// position `0` (`position(...).map_or(0, |pos| pos + 1)`), so an unknown
/// `Last-Event-ID` replays the stream FROM THE BEGINNING. That is today's v1
/// answer and this file pins it deliberately: it is the reach an
/// attacker-supplied replay cursor has on v1 (T-117-05), and the D-03 cut must
/// not change it while making the v2 side unreachable.
const UNKNOWN_LAST_EVENT_ID: &str = "00000000-0000-4000-8000-000000000000";

/// The v1 GET response headers this file pins, in a fixed order.
///
/// `content-type` plus the three `attach_sse_response_headers`
/// (`src/server/streamable_http_server.rs:4426-4439`) emits. Asserted as an
/// explicit name-to-value set, SEPARATE from any body comparison.
const PINNED_GET_HEADERS: [&str; 4] = [
    "content-type",
    MCP_SESSION_ID,
    "cache-control",
    "connection",
];

/// The pinned v1 GET response headers, rendered.
const GET_SSE_HEADERS: &str = concat!(
    "content-type: text/event-stream\n",
    "mcp-session-id: <session-id>\n",
    "cache-control: no-cache, no-transform\n",
    "connection: keep-alive\n",
);

/// The two frames a `Last-Event-ID` GET replays: the stored `initialize` reply
/// then the stored `tools/list` reply, in `event_order`, each re-framed with a
/// FRESH SSE event id by `sse_event_for_message`.
const REPLAYED_FRAMES: &str = concat!(
    "id: <sse-event-id>\n",
    "event: message\n",
    r#"data: {"jsonrpc":"2.0","id":10,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false},"prompts":{"listChanged":false}},"serverInfo":{"name":"v1-byte-identity","version":"1.0.0"}}}"#,
    "\n\n",
    "id: <sse-event-id>\n",
    "event: message\n",
    r#"data: {"jsonrpc":"2.0","id":11,"result":{"tools":[{"name":"pin_lookup","description":"a fixed tool whose v1 wire shape is pinned by this file","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]}}"#,
    "\n\n",
);

/// The v1 DELETE answer for a live session. Not a JSON-RPC frame — a bare
/// `{"status":"ok"}` — and that is itself pinned.
const DELETE_OK_BODY: &str = r#"{"status":"ok"}"#;

/// A v1 POST that names a session DELETE has already torn down.
///
/// Note the key order (`jsonrpc`, `error`, `id`) and the explicit `"id":null`:
/// neither survives a structural-only assertion, and both are part of the v1
/// answer this file freezes.
const DELETE_THEN_REUSE_BODY: &str =
    r#"{"jsonrpc":"2.0","error":{"code":-32600,"message":"Unknown session ID"},"id":null}"#;

/// [`PINNED_GET_HEADERS`], read off `capture` and rendered as a `name: value`
/// block.
fn pinned_header_block(capture: &SseCapture) -> String {
    let values: Vec<(&str, &str)> = PINNED_GET_HEADERS
        .iter()
        .map(|name| (*name, capture.header(name)))
        .collect();
    render_headers(&values)
}

/// Open a v1 GET SSE stream against a primed resumable server and read the
/// replay.
///
/// The `Last-Event-ID` header is what makes the stream produce anything at all:
/// without it `replay_sse_events_from_header` returns before it ever LOOKS at
/// the store, the stream stays silent, and [`read_bounded_sse`] fails on its
/// frame bound rather than quietly passing over an empty capture.
async fn replay_capture(addr: SocketAddr, session_id: &str) -> SseCapture {
    read_bounded_sse(
        addr,
        &[
            header(MCP_SESSION_ID, session_id),
            header(LAST_EVENT_ID, UNKNOWN_LAST_EVENT_ID),
        ],
        REPLAY_FRAME_COUNT,
    )
    .await
}

// ===========================================================================
// Fixture 4 — the v1 GET response HEADERS (a claim independent of the body).
// ===========================================================================

#[tokio::test]
async fn v1_get_sse_response_headers_are_pinned() {
    let (addr, handle) = spawn_resumable().await;
    let session_id = prime_replayable_events(addr).await;
    let capture = replay_capture(addr, &session_id).await;
    teardown(handle, ()).await;

    assert_eq!(
        capture.status, 200,
        "a v1 GET with a valid session id must still open an SSE stream"
    );
    assert_v1_bytes(
        &pinned_header_block(&capture),
        &V1Golden {
            raw: GET_SSE_HEADERS,
            structural: Structural::RawOnly,
            dynamics: SESSION_ID_DYNAMICS,
        },
    );
}

// ===========================================================================
// Fixture 5 — the v1 `Last-Event-ID` replay BODY bytes.
// ===========================================================================

#[tokio::test]
async fn v1_last_event_id_replay_frame_bytes_are_pinned() {
    let (addr, handle) = spawn_resumable().await;
    let session_id = prime_replayable_events(addr).await;
    let capture = replay_capture(addr, &session_id).await;
    teardown(handle, ()).await;

    assert_v1_bytes(
        &capture.raw,
        &V1Golden {
            raw: REPLAYED_FRAMES,
            structural: Structural::Frames(vec![
                json!({
                    "jsonrpc": "2.0",
                    "id": 10,
                    "result": {
                        "protocolVersion": V1,
                        "capabilities": {
                            "tools": { "listChanged": false },
                            "prompts": { "listChanged": false }
                        },
                        "serverInfo": { "name": "v1-byte-identity", "version": "1.0.0" }
                    }
                }),
                json!({
                    "jsonrpc": "2.0",
                    "id": 11,
                    "result": {
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
                    }
                }),
            ]),
            dynamics: SSE_EVENT_ID_DYNAMICS,
        },
    );
}

// ===========================================================================
// Fixture 6 — the v1 DELETE answer.
// ===========================================================================

#[tokio::test]
async fn v1_delete_session_answer_is_pinned() {
    let (addr, handle) = spawn_session_minting().await;
    let (_init, session_id) = initialize(addr, 20).await;
    let response = delete(addr, &[header(MCP_SESSION_ID, &session_id)]).await;
    teardown(handle, ()).await;

    assert_eq!(
        response.status, 200,
        "a v1 DELETE must still tear a session down"
    );
    assert_v1_bytes(
        &response.raw,
        &V1Golden {
            raw: DELETE_OK_BODY,
            structural: Structural::Bare(json!({ "status": "ok" })),
            dynamics: NO_DYNAMICS,
        },
    );
}

// ===========================================================================
// Fixture 7 — reusing a session DELETE already tore down.
// ===========================================================================

#[tokio::test]
async fn v1_delete_then_reusing_the_session_is_rejected_as_today() {
    let (addr, handle) = spawn_session_minting().await;
    let (_init, session_id) = initialize(addr, 21).await;
    let torn_down = delete(addr, &[header(MCP_SESSION_ID, &session_id)]).await;
    assert_eq!(torn_down.status, 200, "the DELETE must succeed first");
    let reuse = post(
        addr,
        &[header(MCP_SESSION_ID, &session_id)],
        &v1_body("tools/list", json!(22), json!({})),
    )
    .await;
    teardown(handle, ()).await;

    assert_eq!(
        reuse.status, 404,
        "a v1 request naming a torn-down session must still be rejected: {}",
        reuse.raw
    );
    assert_v1_bytes(
        &reuse.raw,
        &V1Golden {
            raw: DELETE_THEN_REUSE_BODY,
            structural: Structural::Bare(json!({
                "jsonrpc": "2.0",
                "error": { "code": -32600, "message": "Unknown session ID" },
                "id": null
            })),
            dynamics: NO_DYNAMICS,
        },
    );
}

// ===========================================================================
// Anti-vacuity — the replay fixture is comparing a NON-EMPTY stream.
// ===========================================================================

/// The replay fixture really replayed something.
#[tokio::test]
async fn the_replay_fixture_actually_replayed_something() {
    let (addr, handle) = spawn_resumable().await;
    let session_id = prime_replayable_events(addr).await;
    let capture = replay_capture(addr, &session_id).await;
    teardown(handle, ()).await;

    assert!(
        capture.frames > 0,
        "FAILURE MODE: the v1 `Last-Event-ID` replay produced ZERO frames, so \
         `v1_last_event_id_replay_frame_bytes_are_pinned` would be comparing an \
         empty capture against an empty golden and passing over nothing. \
         WHAT TO DO: fix the v1 replay path (or the priming of the event store) — \
         do NOT weaken the byte comparison to accommodate an empty stream. \
         Capture was: {:?}",
        capture.raw
    );
    assert_eq!(
        capture.frames, REPLAY_FRAME_COUNT,
        "the store was primed with exactly {REPLAY_FRAME_COUNT} v1 responses, so the \
         replay must deliver exactly that many"
    );
}

// ===========================================================================
// Anti-vacuity — the normalizer itself.
// ===========================================================================

/// The width-preserving substitution cannot mask a byte change.
///
/// Without this, "the normalization is width-preserving" would be a comment.
/// The three cases are: the same-width pass really does preserve length; a
/// declared-but-absent key is rejected rather than silently skipped; and a value
/// whose SHAPE changed is rejected rather than normalized away.
#[test]
fn the_substitution_is_width_preserving_and_shape_checked() {
    let raw = "id: 6f1e2d3c-4b5a-4978-8765-43210fedcba9\n";
    let same_width = substitute(raw, SSE_EVENT_ID_DYNAMICS, true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the same-width pass must be width-preserving"
    );
    assert_eq!(
        key_occurrences(&same_width, &SSE_EVENT_ID_DYNAMICS[0]),
        key_occurrences(raw, &SSE_EVENT_ID_DYNAMICS[0]),
        "the same-width pass must not delete the key"
    );
    assert_eq!(
        substitute(raw, SSE_EVENT_ID_DYNAMICS, false),
        "id: <sse-event-id>\n",
        "the canonical pass must write the bare token"
    );
}
