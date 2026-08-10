//! Example: the DUAL-VERSION conformance TARGET (Phase 118, CONF-01).
//!
//! Run this server with:
//! ```bash
//! cargo run --example s54_v2_dual_conformance --features full
//! ```
//!
//! Optionally pass a bind address:
//! `cargo run --example s54_v2_dual_conformance --features full -- 127.0.0.1:9000`.
//!
//! # What this demonstrates
//!
//! - **One process, two eras.** The accept-list carries BOTH `2025-11-25` and
//!   `2026-07-28`, so the era is negotiated PER REQUEST rather than chosen once
//!   at startup. That is the whole milestone claim: ONE binary serves both.
//! - **A real session path.** The transport is built with the STATEFUL default
//!   `StreamableHttpServerConfig::default()`, so v1 clients get a live
//!   `Mcp-Session-Id` while v2 requests stay session-free.
//! - **A fixture surface dictated by an external referee.** Every tool,
//!   resource URI and prompt name below is named by the official
//!   `@modelcontextprotocol/conformance` suite. They are NOT free-form
//!   examples: renaming one silently breaks the scenario that consumes it.
//!
//! # This is a measurement target, not a style guide
//!
//! The official suite is run against this ONE process twice (D-04/D-06):
//!
//! ```bash
//! conformance server --url http://127.0.0.1:8149/ \
//!   --requirements 2025-11-25 -o target/conformance-results/2025-11-25
//!
//! conformance server --url http://127.0.0.1:8149/ \
//!   --requirements 2026-07-28 -o target/conformance-results/2026-07-28
//! ```
//!
//! The suite binary is the pinned one in `conformance/` — see
//! `conformance/README.md` for the pin, the Node >= 22 floor and the
//! `--ignore-scripts` rule. Results go under `target/` because `.gitignore`
//! already covers it; a top-level results directory is NOT ignored and would
//! dirty the worktree on every run.
//!
//! # What the two runs prove
//!
//! Both requirement sets are served by ONE process, started once from the built
//! binary at `target/debug/examples/s54_v2_dual_conformance` and never
//! restarted between them — that single unchanged PID is the whole of the
//! CONF-01 claim, because two processes would prove "pmcp can serve v1" and
//! "pmcp can serve v2" as two separate facts rather than as one binary doing
//! both. Measured against
//! `@modelcontextprotocol/conformance@0.2.0-alpha.11`:
//!
//! | Requirement set | Scenarios | Result | Scored scenarios failing |
//! |---|---|---|---|
//! | `2025-11-25` | 33 (30 scored) | 51 passed, 15 failed | 11 |
//! | `2026-07-28` | 50 (37 scored) | 124 passed, 54 failed | 7 |
//!
//! Both runs exit 1, for the reasons declared below. What one process DOES
//! demonstrate, measured rather than asserted: the whole v2 MRTR surface (all
//! 14 `input-required-result-*` scenarios green, including the `prompts/get`
//! one that proves MRTR is not a `tools/call` feature), the v2 caching, session
//! and DNS-rebinding scenarios, and — after the v2 run, against the same live
//! process — a re-run of the v1 `server-session-lifecycle` scenario still
//! passing 3/3, so no cross-era state bleed was observed.
//!
//! # The Tasks extension is deliberately ABSENT (D-14)
//!
//! The suite's task-bearing fixture tool and the ten `tasks-*` scenarios are
//! `not_scored` at BOTH revisions, so implementing them would add surface
//! without adding evidence. Their absence is a decision, not an omission.
//!
//! # DECLARED NON-CONFORMANCE — read this before citing the example (D-21)
//!
//! Neither requirement set exits 0, and that is a MEASURED, DECLARED outcome
//! rather than an unfinished one. Phase 118 decision D-21 scopes the claim to
//! what genuinely passes and states the rest in writing:
//! `.planning/phases/118-conformance-against-the-official-suite/118-CONFORMANCE-GAPS.md`.
//! No `--expected-failures` baseline, no allowlist and no known-fail file is
//! used anywhere in this phase — the suppression the phase forbids stays
//! forbidden, so the numbers below are the real ones.
//!
//! The scored 2026-07-28 failures that this example CANNOT fix, because every
//! one of them lives in `src/` rather than in a fixture, with the gap they
//! trace to:
//!
//! | Scored scenario | Gap | What `src/` does |
//! |---|---|---|
//! | `tools-call-embedded-resource`, `tools-call-mixed-content`, `prompts-get-embedded-resource` | G-1 | `Content::Resource` serialises FLAT; the spec's `EmbeddedResource` nests under `resource` |
//! | `resources-read-binary` | G-2 | no blob-bearing resource-contents variant exists |
//! | `tools-call-with-progress` | G-3 | `notification_tx` is set only in `Server::run()`, which `StreamableHttpServer` never calls |
//! | `completion-complete` | G-4 | `completion/complete` is a catch-all arm returning `{}` |
//! | `server-stateless` (`HttpServerMethodNotFound404ping`) | G-5 | `ping` is served under v2 instead of being retired — and see the sharper finding below |
//! | `server-stateless` (`ServerImplementsDiscover`, `ServerUnsupportedVersionError`) | G-7 (new) | `server/discover` emits `protocolVersion`, never the `supportedVersions` array the spec mandates — `grep -rn supportedVersions src/` finds nothing |
//! | `server-stateless` (3x `RequestMetaInvalid`, `HttpServerMetaInvalid400`) | G-6 (new) | a missing / malformed `_meta` answers `-32020`, not the `-32602` + HTTP 400 the spec requires; a missing `clientCapabilities` is not rejected at all |
//! | `server-stateless` (`HttpServerHeaderMismatch400`) | G-8 (new) | a header/`_meta` protocol-version disagreement answers `-32022`, not `-32020` |
//!
//! Everything else that fails is a MISSING FIXTURE, and fixtures are exactly
//! what this file is for. Do not cite this example as "pmcp passes the official
//! suite"; cite it for the dual-era claim, which one process does demonstrate.
//!
//! ## The removed-method retirement is WEAKER than the suite's score suggests
//!
//! v2 removes five RPCs: `initialize`, `ping`, `logging/setLevel`,
//! `resources/subscribe` and `resources/unsubscribe`. The suite scores four of
//! the five as passing. Probed directly with WELL-FORMED params, only two are
//! genuinely retired:
//!
//! | Method under v2 | Suite's probe (`params` = `_meta` only) | Well-formed `params` |
//! |---|---|---|
//! | `initialize` | 404 + `-32601` | **HTTP 200, served** — and answers `protocolVersion: "2025-11-25"` |
//! | `ping` | HTTP 200, served | HTTP 200, served |
//! | `logging/setLevel` | 404 + `-32601` | **HTTP 200, served** |
//! | `resources/subscribe` | 404 + `-32601` | 404 + `-32601` (genuinely retired) |
//! | `resources/unsubscribe` | 404 + `-32601` | 404 + `-32601` (genuinely retired) |
//!
//! `v2_retired_method_of` in `src/server/streamable_http_server.rs` matches
//! exactly `Subscribe` and `Unsubscribe` and nothing else. The other three
//! answer `-32601` to the suite only because its probe sends `params` carrying
//! `_meta` alone, which does not deserialize into `InitializeRequest` or
//! `SetLoggingLevel` — a PARSE failure that happens to produce the required
//! code. Two of the four "passes" therefore pass for the wrong reason, and the
//! `docs/v1-sunset-policy.md` tension about a v2 server still answering
//! `initialize` is CONFIRMED at the wire rather than merely suspected.
//!
//! # Divergence from `s47_v2_stateless_mrtr`: `PMCP_REQUEST_STATE_KEY`
//!
//! `s47` deliberately leaves that variable UNSET so a reader sees the SDK's
//! startup WARNING about a per-process fallback key. A conformance target must
//! not emit a warning as part of its contract — a `WARN` line in the transcript
//! is indistinguishable from a real defect when a run is reviewed later. So
//! this example reads the variable itself and, when it is absent, prints one
//! explicit line telling the operator to set it. The value is NEVER echoed.
//!
//! Phase 119's DOCS-06 cites this example rather than adding a second one.

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::shared::http_constants::{ACCEPT_STREAMABLE, MCP_PROTOCOL_VERSION};
use pmcp::types::capabilities::{
    ClientCapabilities, CompletionCapabilities, LoggingCapabilities, PromptCapabilities,
    ResourceCapabilities, ServerCapabilities, ToolCapabilities,
};
use pmcp::types::elicitation::ElicitRequestParams;
use pmcp::types::mrtr::{InputRequest, InputRequests, InputResponse, MrtrSignal};
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::types::sampling::{CreateMessageParams, SamplingMessage, SamplingMessageContent};
use pmcp::types::{
    CallToolResult, Content, GetPromptResult, ListResourcesResult, PromptArgument, PromptInfo,
    PromptMessage, ReadResourceResult, ResourceInfo, Role, ToolInfo,
};
use pmcp::{PromptHandler, RequestHandlerExtra, ResourceHandler, Server};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

// ===========================================================================
// The fixture surface, and which scenario consumes each entry.
//
// Every name below is DICTATED by the official suite. The scenario named in
// the right-hand column is the one that breaks if the entry is renamed or
// dropped, so this table is the blast radius of any edit here.
//
//   test://static-text             -> resources-read-text
//   test://static-binary           -> resources-read-binary
//   test://example-resource        -> resources-list (a plain listed entry)
//   test://embedded-resource       -> tools-call-embedded-resource
//   test://mixed-content-resource  -> tools-call-mixed-content
//   test://template/{id}/data      -> resources-templates-read
//   test://watched-resource        -> resources-subscribe / resources-unsubscribe
//
//   test_prompt                        -> (listed only; no scored scenario reads it)
//   test_simple_prompt                 -> prompts-get-simple
//   test_prompt_with_arguments         -> prompts-get-with-args, completion-complete
//   test_prompt_with_embedded_resource -> prompts-get-embedded-resource
//   test_prompt_with_image             -> prompts-get-with-image
//
// `resources-list` validates STRUCTURE across every listed entry, so each one
// declares a name, a description and a mimeType — RESEARCH measured tools-list
// failing against s47 purely for a missing `description`, and the same class of
// structural gap applies here.
// ===========================================================================

/// A 1x1 red-pixel PNG, base64. The suite asks only for "a minimal test image".
const TINY_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

/// The URI template the suite reads through with `{id}` substituted.
///
/// Spelled as `test://template/{id}/data` in the suite's requirements; matched
/// here by prefix + suffix because pmcp's [`ResourceHandler`] has no template
/// seam (see the SDK-gap note in this plan's SUMMARY).
const TEMPLATE_PREFIX: &str = "test://template/";
/// The suffix that closes the template's single path parameter.
const TEMPLATE_SUFFIX: &str = "/data";

/// A minimal 44-byte WAV header with no samples, base64. The suite asks only
/// for "a minimal test audio file".
const TINY_WAV_BASE64: &str = "UklGRiQAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQAAAAA=";

/// Every prompt name the suite names, in `prompts/list` order.
const PROMPT_NAMES: [&str; 6] = [
    "test_prompt",
    "test_simple_prompt",
    "test_prompt_with_arguments",
    "test_prompt_with_embedded_resource",
    "test_prompt_with_image",
    // `input-required-result-non-tool-request` — the scenario that proves MRTR
    // is UNIVERSAL rather than a tools/call feature, by driving it through
    // prompts/get.
    "test_input_required_result_prompt",
];

/// Every tool name the suite names.
///
/// The first fifteen are the surface Phase 118's plan enumerates. The last two
/// were added after MEASURING `conformance list --server --requirements
/// 2025-11-25`: `elicitation-sep1034-defaults` and `elicitation-sep1330-enums`
/// are SCORED at 2025-11-25 and name their own tools, which the plan's list
/// omitted. Registering them costs nothing and makes those two scenarios
/// reachable rather than failing on a missing tool.
///
/// The suite's task-bearing fixture tool and every other Tasks-extension tool
/// are deliberately ABSENT (D-14) — they are `not_scored` at both revisions, so
/// implementing them would add surface without adding evidence.
const TOOL_NAMES: [&str; 30] = [
    "test_simple_text",
    "test_image_content",
    "test_audio_content",
    "test_embedded_resource",
    "test_multiple_content_types",
    "test_error_handling",
    "test_tool_with_progress",
    "test_tool_with_logging",
    "test_logging_tool",
    "test_complete",
    "test_missing_capability",
    "test_headers",
    "test_custom_headers",
    "test_sampling",
    "test_elicitation",
    "test_elicitation_sep1034_defaults",
    "test_elicitation_sep1330_enums",
    // --- The 2026-07-28 MRTR surface (SEP-2322). ---
    "test_input_required_result_elicitation",
    "test_input_required_result_sampling",
    "test_input_required_result_list_roots",
    "test_input_required_result_multi_round",
    "test_input_required_result_multiple_inputs",
    "test_input_required_result_request_state",
    "test_input_required_result_tampered_state",
    "test_input_required_result_capabilities",
    "test_mrtr_echo_state",
    "test_mrtr_no_state",
    "test_mrtr_unrelated",
    "test_mrtr_no_result_type",
    // The `server-stateless` diagnostic tool. Not in the plan's list — found by
    // reading the scenario's own failure text, which names it verbatim
    // ("server does not list the diagnostic tool 'test_streaming_elicitation'
    // in tools/list, so the response stream could not be exercised").
    "test_streaming_elicitation",
];

/// Where the server binds when `argv[1]` is absent.
///
/// 8149 is chosen to avoid every port an in-repo example or script already
/// holds: `t04`/`t05` bind 8080/8081 in `scripts/test_examples_with_tester.sh`
/// and `s47_v2_stateless_mrtr` defaults to 8147. A stale process from a
/// previous run answering the suite is a Tampering threat (T-118-17), so the
/// bind failure below is reported rather than unwrapped.
const DEFAULT_ADDR: &str = "127.0.0.1:8149";

/// The environment variable carrying the AEAD key for MRTR `requestState`.
///
/// Read for PRESENCE only — the value is never logged, printed or echoed
/// (T-118-15).
const REQUEST_STATE_KEY_VAR: &str = "PMCP_REQUEST_STATE_KEY";

/// Serves every `test://` URI the 2025-11-25 scored set names.
///
/// One handler covers both `list` and `read` because [`ResourceHandler`] is a
/// single trait with both methods; the template URI is matched by prefix in
/// `read` rather than being listed, which is exactly what
/// `resources-templates-read` exercises (it reads `test://template/123/data`
/// directly and asserts the `123` reaches the body).
struct ConformanceResources;

impl ConformanceResources {
    /// Split `test://template/<id>/data` into its `<id>`, or `None`.
    fn template_id(uri: &str) -> Option<&str> {
        uri.strip_prefix(TEMPLATE_PREFIX)?
            .strip_suffix(TEMPLATE_SUFFIX)
    }
}

#[async_trait]
impl ResourceHandler for ConformanceResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        // The template arm first: it is a PREFIX match, not an equality match.
        if let Some(id) = Self::template_id(uri) {
            // The suite asserts the substituted id appears in the body, so the
            // id is interpolated rather than echoed from a fixed string.
            let text = serde_json::json!({
                "id": id,
                "templateTest": true,
                "data": format!("Data for ID: {id}"),
            })
            .to_string();
            return Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                text,
                "application/json",
            )]));
        }

        let contents = match uri {
            "test://static-text" => vec![Content::resource_with_text(
                uri,
                "This is the content of the static text resource.",
                "text/plain",
            )],
            // NOTE: the suite requires `{uri, mimeType, blob}` here. pmcp's
            // `Content` enum has no blob-bearing resource variant, so this arm
            // cannot express the required shape — see the SDK-gap note in this
            // plan's SUMMARY. The closest expressible value is emitted so the
            // resource still exists and still reads.
            "test://static-binary" => vec![Content::image(TINY_PNG_BASE64, "image/png")],
            "test://example-resource" => vec![Content::resource_with_text(
                uri,
                "This is an example resource for testing.",
                "text/plain",
            )],
            "test://embedded-resource" => vec![Content::resource_with_text(
                uri,
                "This is an embedded resource content.",
                "text/plain",
            )],
            "test://mixed-content-resource" => vec![Content::resource_with_text(
                uri,
                r#"{"test":"data","value":123}"#,
                "application/json",
            )],
            "test://watched-resource" => vec![Content::resource_with_text(
                uri,
                "This is the watched resource's current content.",
                "text/plain",
            )],
            unknown => {
                return Err(pmcp::Error::validation(format!(
                    "unknown resource URI: {unknown}"
                )))
            },
        };

        Ok(ReadResourceResult::new(contents))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        // `resources/list` returns DIRECT resources only — the template URI is
        // deliberately absent, because the suite distinguishes the two and a
        // template listed here would be a structural error.
        let listed = [
            (
                "test://static-text",
                "Static Text Resource",
                "A plain-text resource read by resources-read-text",
                "text/plain",
            ),
            (
                "test://static-binary",
                "Static Binary Resource",
                "A binary resource read by resources-read-binary",
                "image/png",
            ),
            (
                "test://example-resource",
                "Example Resource",
                "A general-purpose example resource",
                "text/plain",
            ),
            (
                "test://embedded-resource",
                "Embedded Resource",
                "The resource test_embedded_resource embeds",
                "text/plain",
            ),
            (
                "test://mixed-content-resource",
                "Mixed Content Resource",
                "The resource test_multiple_content_types embeds",
                "application/json",
            ),
            (
                "test://watched-resource",
                "Watched Resource",
                "The resource resources/subscribe and /unsubscribe target",
                "text/plain",
            ),
        ];

        let resources = listed
            .into_iter()
            .map(|(uri, name, description, mime_type)| {
                let mut info = ResourceInfo::new(uri, name);
                info.description = Some(description.to_string());
                info.mime_type = Some(mime_type.to_string());
                info
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}

/// A prompt whose messages are a fixed list, built per call from its arguments.
///
/// One struct backs all five named prompts: they differ only in their metadata
/// and in how their messages are assembled, so a single dispatch on the name
/// keeps the five contracts side by side and readable.
struct ConformancePrompt {
    /// The suite-dictated prompt name. Load-bearing — see the table above.
    name: &'static str,
}

#[async_trait]
impl PromptHandler for ConformancePrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        // MRTR is not a `tools/call` feature: `prompts/get` and
        // `resources/read` are equally eligible, and this prompt is the fixture
        // that proves it. Same authoring seam as the tools — the signal rides
        // on the result's `_meta` and the SDK seals it.
        if self.name == "test_input_required_result_prompt" {
            return input_required_prompt(&extra);
        }
        let messages = match self.name {
            "test_prompt" => vec![PromptMessage::new(
                Role::User,
                Content::text("This is a test prompt."),
            )],
            "test_simple_prompt" => vec![PromptMessage::new(
                Role::User,
                Content::text("This is a simple prompt for testing."),
            )],
            "test_prompt_with_arguments" => {
                // Both arguments are declared REQUIRED in metadata, so a
                // missing one is a client error rather than something to
                // paper over with a default.
                let arg1 = args.get("arg1").ok_or_else(|| {
                    pmcp::Error::validation("test_prompt_with_arguments requires arg1")
                })?;
                let arg2 = args.get("arg2").ok_or_else(|| {
                    pmcp::Error::validation("test_prompt_with_arguments requires arg2")
                })?;
                vec![PromptMessage::new(
                    Role::User,
                    Content::text(format!(
                        "Prompt with arguments: arg1='{arg1}', arg2='{arg2}'"
                    )),
                )]
            },
            "test_prompt_with_embedded_resource" => {
                // The URI comes from the CALLER, so the embedded block echoes
                // whatever it named — that is the contract the scenario checks.
                let uri = args.get("resourceUri").ok_or_else(|| {
                    pmcp::Error::validation(
                        "test_prompt_with_embedded_resource requires resourceUri",
                    )
                })?;
                vec![
                    PromptMessage::new(
                        Role::User,
                        Content::resource_with_text(
                            uri.clone(),
                            "Embedded resource content for testing.",
                            "text/plain",
                        ),
                    ),
                    PromptMessage::new(
                        Role::User,
                        Content::text("Please process the embedded resource above."),
                    ),
                ]
            },
            "test_prompt_with_image" => vec![
                PromptMessage::new(Role::User, Content::image(TINY_PNG_BASE64, "image/png")),
                PromptMessage::new(Role::User, Content::text("Please analyze the image above.")),
            ],
            other => {
                return Err(pmcp::Error::validation(format!("unknown prompt: {other}")));
            },
        };

        Ok(GetPromptResult::new(messages, None))
    }

    fn metadata(&self) -> Option<PromptInfo> {
        // `prompts-list` validates that every entry carries a name and a
        // description, so no prompt may fall back to the empty default.
        let (description, arguments) = match self.name {
            "test_prompt" => ("A basic test prompt", vec![]),
            "test_simple_prompt" => ("A simple prompt with no arguments", vec![]),
            "test_prompt_with_arguments" => (
                "A prompt that takes two required string arguments",
                vec![
                    required_argument("arg1", "First test argument"),
                    required_argument("arg2", "Second test argument"),
                ],
            ),
            "test_prompt_with_embedded_resource" => (
                "A prompt that embeds the resource named by its argument",
                vec![required_argument(
                    "resourceUri",
                    "URI of the resource to embed",
                )],
            ),
            "test_prompt_with_image" => ("A prompt that returns an image content block", vec![]),
            "test_input_required_result_prompt" => (
                "A prompt that asks the user for context before it can be rendered",
                vec![],
            ),
            _ => ("", vec![]),
        };

        let mut info = PromptInfo::new(self.name).with_description(description);
        if !arguments.is_empty() {
            info = info.with_arguments(arguments);
        }
        Some(info)
    }
}

/// `test_input_required_result_prompt`, both rounds.
///
/// Round 1 asks for context through an `input_required` result; round 2 renders
/// the prompt with whatever the client answered. The `_meta` field of
/// [`GetPromptResult`] is set directly, exactly as the tools set
/// `CallToolResult::_meta` — this is the one authoring seam, and `prompts/get`
/// is one of the three methods the spec lets an `InputRequiredResult` ride on.
fn input_required_prompt(extra: &RequestHandlerExtra) -> pmcp::Result<GetPromptResult> {
    if extra.mrtr_continuation().is_some() {
        let context =
            elicited_string(extra, "user_context", "context").unwrap_or_else(|| "none".to_string());
        return Ok(GetPromptResult::new(
            vec![PromptMessage::new(
                Role::User,
                Content::text(format!("Using the context you supplied: {context}")),
            )],
            Some("A prompt rendered from elicited context".to_string()),
        ));
    }

    let signal = MrtrSignal {
        input_requests: one_request(
            "user_context",
            elicit(
                "What context should the prompt use?",
                one_string_schema("context"),
            ),
        ),
        continuation: serde_json::json!({ "asked": "user_context" }),
    };
    let (key, value) = signal
        .into_meta_entry()
        .map_err(|error| pmcp::Error::internal(error.to_string()))?;
    let mut meta = serde_json::Map::new();
    meta.insert(key, value);

    let mut result = GetPromptResult::new(
        vec![PromptMessage::new(
            Role::User,
            Content::text("I need some context before I can render this prompt."),
        )],
        Some("Awaiting elicited context".to_string()),
    );
    result._meta = Some(meta);
    Ok(result)
}

/// A required prompt argument with a description, the shape `prompts/list`
/// validates.
fn required_argument(name: &str, description: &str) -> PromptArgument {
    PromptArgument::new(name)
        .with_description(description)
        .required()
}

/// Every fixture tool the 2025-11-25 scored set names.
///
/// One struct backs all of them: they differ only in their metadata and in the
/// exact `CallToolResult` they return, so keeping the seventeen contracts in a
/// single `match` puts them side by side where a reviewer can compare them
/// against the suite's requirement blocks.
struct ConformanceTool {
    /// The suite-dictated tool name. Load-bearing — renaming it breaks the
    /// scenario that calls it.
    name: &'static str,
}

#[async_trait]
impl pmcp::ToolHandler for ConformanceTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Every tool owns its full envelope, so the plain-`Value` path is never
        // the one that runs. `handle_output` below is the real entry point;
        // this exists only to satisfy the trait.
        Err(pmcp::Error::internal(format!(
            "{} is served through handle_output, not handle",
            self.name
        )))
    }

    async fn handle_output(
        &self,
        args: Value,
        extra: RequestHandlerExtra,
    ) -> pmcp::Result<pmcp::server::ToolOutput> {
        // `ToolOutput::Result` rather than `::Payload`: the suite compares the
        // `content` array element by element, and the `Payload` tail would
        // text-wrap the value into a single text block instead.
        let result = self.call(args, extra).await?;
        Ok(pmcp::server::ToolOutput::Result(result))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        // `tools-list` is SCORED at BOTH revisions and asserts that every tool
        // carries a name, a description AND an inputSchema — RESEARCH measured
        // it failing against s47 for a missing `description` alone. So no tool
        // may fall back to the empty default.
        let (description, schema) = match self.name {
            "test_simple_text" => ("Returns a single simple text content block", no_args()),
            "test_image_content" => ("Returns an image content block", no_args()),
            "test_audio_content" => ("Returns an audio content block", no_args()),
            "test_embedded_resource" => ("Returns an embedded-resource content block", no_args()),
            "test_multiple_content_types" => (
                "Returns several content blocks of different types",
                no_args(),
            ),
            "test_error_handling" => ("Always returns a tool result with isError: true", no_args()),
            "test_tool_with_progress" => (
                "Emits progress notifications against the request's progress token",
                no_args(),
            ),
            "test_tool_with_logging" => (
                "Emits notifications/message log records during the call",
                no_args(),
            ),
            "test_logging_tool" => ("The tool the logging scenarios drive", no_args()),
            "test_complete" => ("Backs the completion/complete endpoint", no_args()),
            "test_missing_capability" => (
                "Needs a client sampling capability, so an undeclared client is refused -32021",
                no_args(),
            ),
            "test_headers" => ("Echoes the request's standard HTTP headers back", no_args()),
            "test_custom_headers" => ("Echoes custom request headers back", no_args()),
            "test_sampling" => (
                "Requests sampling/createMessage from the client",
                one_required_string("prompt", "The prompt to send to the LLM"),
            ),
            "test_elicitation" => (
                "Requests elicitation/create from the client",
                one_required_string("message", "The message to show the user"),
            ),
            "test_elicitation_sep1034_defaults" => (
                "Requests elicitation with SEP-1034 default values for every primitive type",
                no_args(),
            ),
            "test_elicitation_sep1330_enums" => (
                "Requests elicitation with all five SEP-1330 enum variants",
                no_args(),
            ),
            "test_input_required_result_elicitation" => (
                "Asks for a name through an input_required elicitation, then greets it",
                no_args(),
            ),
            "test_input_required_result_sampling" => (
                "Asks the client's model a question through an input_required sampling request",
                no_args(),
            ),
            "test_input_required_result_list_roots" => (
                "Asks the client for its roots through an input_required roots/list request",
                no_args(),
            ),
            "test_input_required_result_multi_round" => (
                "Two input_required rounds with evolving requestState, then a complete result",
                no_args(),
            ),
            "test_input_required_result_multiple_inputs" => (
                "One input_required result carrying elicitation, sampling and roots/list at once",
                no_args(),
            ),
            "test_input_required_result_capabilities" => (
                "Asks only for inputs the client's declared capabilities can answer",
                no_args(),
            ),
            "test_streaming_elicitation" => (
                "Answers with a single input_required result, never an independent request",
                no_args(),
            ),
            "test_input_required_result_request_state" => (
                "Round-trips an AEAD-sealed requestState across one MRTR continuation",
                no_args(),
            ),
            "test_input_required_result_tampered_state" => (
                "Mints an integrity-protected requestState whose tampered resend must be rejected",
                no_args(),
            ),
            "test_mrtr_echo_state" => (
                "Test tool: triggers MRTR flow with requestState. Client must echo state back \
                 unchanged.",
                no_args(),
            ),
            "test_mrtr_no_state" => (
                "Test tool: triggers MRTR flow WITHOUT requestState. Client must NOT include \
                 requestState in retry.",
                no_args(),
            ),
            "test_mrtr_unrelated" => (
                "Test tool: simple tool called between MRTR rounds. Must NOT carry inputResponses \
                 or requestState from another tool.",
                no_args(),
            ),
            "test_mrtr_no_result_type" => (
                "Test tool: returns a result without resultType. Client must treat it as complete \
                 (default).",
                no_args(),
            ),
            _ => ("", no_args()),
        };
        Some(ToolInfo::new(
            self.name,
            Some(description.to_string()),
            schema,
        ))
    }
}

impl ConformanceTool {
    /// Build the exact `CallToolResult` this tool's scenario expects.
    ///
    /// Each literal string below is quoted from that scenario's
    /// `**Server Implementation Requirements:**` block character for
    /// character — they are contract, not prose.
    async fn call(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<CallToolResult> {
        // The MRTR-shaped tools come first and are dispatched BY ROUND rather
        // than by name alone, so each tool's ask and its resume sit next to
        // their siblings instead of being buried in one arm. A tool that is not
        // MRTR-shaped falls through to the match below.
        if let Some(result) = self.mrtr_call(&extra) {
            return result;
        }
        match self.name {
            "test_simple_text" => Ok(CallToolResult::new(vec![Content::text(
                "This is a simple text response for testing.",
            )])),

            "test_image_content" => Ok(CallToolResult::new(vec![Content::image(
                TINY_PNG_BASE64,
                "image/png",
            )])),

            "test_audio_content" => Ok(CallToolResult::new(vec![Content::audio(
                TINY_WAV_BASE64,
                "audio/wav",
            )])),

            "test_embedded_resource" => Ok(CallToolResult::new(vec![Content::resource_with_text(
                "test://embedded-resource",
                "This is an embedded resource content.",
                "text/plain",
            )])),

            "test_multiple_content_types" => Ok(CallToolResult::new(vec![
                Content::text("Multiple content types test:"),
                Content::image(TINY_PNG_BASE64, "image/png"),
                Content::resource_with_text(
                    "test://mixed-content-resource",
                    r#"{"test":"data","value":123}"#,
                    "application/json",
                ),
            ])),

            // `isError: true` on a RESULT, not a JSON-RPC error: the scenario
            // asserts the tool-level error shape specifically.
            "test_error_handling" => Ok(CallToolResult::error(vec![Content::text(
                "This tool intentionally returns an error for testing",
            )])),

            "test_tool_with_progress" => {
                // The scenario requires at least three notifications with
                // non-decreasing progress, and the delays are what let a client
                // observe them as separate messages rather than one burst.
                for step in [0.0_f64, 50.0, 100.0] {
                    extra.report_progress(step, Some(100.0), None).await?;
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Ok(CallToolResult::new(vec![Content::text(
                    "Tool with progress completed",
                )]))
            },

            "test_tool_with_logging" | "test_logging_tool" => {
                // Three info-level records, spaced so a client can receive them
                // DURING the call rather than all at completion.
                for message in [
                    "Tool execution started",
                    "Tool processing data",
                    "Tool execution completed",
                ] {
                    tracing::info!("{message}");
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Ok(CallToolResult::new(vec![Content::text(
                    "Tool with logging completed",
                )]))
            },

            "test_complete" => Ok(CallToolResult::new(vec![Content::text(
                "Completion fixture tool",
            )])),

            "test_headers" | "test_custom_headers" => {
                // pmcp's streamable-HTTP transport does not surface inbound
                // request headers to a handler (no header plumbing into
                // `RequestHandlerExtra::extensions`), so this reports what it
                // CAN see rather than fabricating an echo. Recorded as an SDK
                // gap in this plan's SUMMARY; the scenarios that consume these
                // two are v2-only and belong to plan 118-05.
                let seen = extra.request_meta.clone().unwrap_or(Value::Null);
                Ok(CallToolResult::new(vec![Content::text(format!(
                    "{}: request _meta = {seen}",
                    self.name
                ))]))
            },

            "test_sampling" => {
                let prompt = args
                    .get("prompt")
                    .and_then(Value::as_str)
                    .ok_or_else(|| pmcp::Error::validation("test_sampling requires prompt"))?;
                // `extra.peer()` is the SDK's server->client back-channel. It is
                // `None` on this transport (see the SUMMARY's SDK-gap note), so
                // the refusal is explicit rather than a silent empty result.
                let peer = extra.peer().ok_or_else(|| {
                    pmcp::Error::internal(
                        "no server->client channel on this transport: sampling/createMessage \
                         cannot be issued",
                    )
                })?;
                let response = peer
                    .sample(sampling_params(prompt))
                    .await
                    .map_err(|error| pmcp::Error::internal(error.to_string()))?;
                let text = match &response.content {
                    Content::Text { text } => text.clone(),
                    other => format!("{other:?}"),
                };
                Ok(CallToolResult::new(vec![Content::text(format!(
                    "LLM response: {text}"
                ))]))
            },

            // All three elicitation tools hit the same wall: pmcp's
            // `PeerHandle` exposes `sample`, `sample_with_tools`, `list_roots`
            // and `progress_notify` — there is no `elicit`, and the
            // `ElicitationManager` that would issue one is not reachable from a
            // handler. Recorded as an SDK gap in this plan's SUMMARY.
            "test_elicitation"
            | "test_elicitation_sep1034_defaults"
            | "test_elicitation_sep1330_enums" => Err(pmcp::Error::internal(
                "no server->client channel on this transport: elicitation/create cannot be issued",
            )),

            // ---------------------------------------------------------------
            // The two `test_mrtr_*` tools that are NOT MRTR-shaped. Their
            // MRTR-shaped siblings, and every `test_input_required_result_*`
            // tool, live in `mrtr_ask` / `mrtr_resume` below.
            // ---------------------------------------------------------------

            // A plain complete result: the negative control for a client that
            // treats every v2 response as an MRTR step.
            "test_mrtr_unrelated" => Ok(CallToolResult::new(vec![Content::text("unrelated-ok")])),

            // DIVERGENCE, recorded rather than worked around: the suite's mock
            // omits `resultType` entirely. pmcp's v2 envelope always writes it
            // (`inject_v2_result_envelope` is its single writer), so this
            // returns a COMPLETE result — `resultType: "complete"` — which is
            // the closest expressible value and is what a client must treat as
            // the default anyway.
            "test_mrtr_no_result_type" => Ok(CallToolResult::new(vec![Content::text(
                "no-result-type-ok",
            )])),

            other => Err(pmcp::Error::validation(format!("unknown tool: {other}"))),
        }
    }

    /// The MRTR-shaped tools, dispatched BY ROUND rather than by name alone.
    ///
    /// `None` means "not an MRTR-shaped tool" and sends the caller back to the
    /// plain `match`. The round split is the point: [`mrtr_ask`](Self::mrtr_ask)
    /// is what a tool needs before it can answer, [`mrtr_resume`](Self::mrtr_resume)
    /// is what it does once the client HAS answered. Keeping them apart leaves
    /// each side a flat table a reviewer can hold beside the suite's
    /// `**Server Implementation Requirements:**` blocks.
    fn mrtr_call(&self, extra: &RequestHandlerExtra) -> Option<pmcp::Result<CallToolResult>> {
        match extra.mrtr_continuation() {
            Some(continuation) => self.mrtr_resume(continuation, extra),
            None => self.mrtr_ask(extra),
        }
    }

    /// Round 1: the `inputRequests` each MRTR-shaped tool needs answered, and
    /// the handler-owned continuation that lets it resume.
    ///
    /// The continuation is SEALED by the SDK and never published; the client
    /// sees only the opaque token.
    fn mrtr_ask(&self, extra: &RequestHandlerExtra) -> Option<pmcp::Result<CallToolResult>> {
        let (text, signal) = match self.name {
            "test_input_required_result_elicitation" => (
                "I need your name before I can greet you.",
                MrtrSignal {
                    input_requests: one_request(
                        "user_name",
                        elicit("What is your name?", one_string_schema("name")),
                    ),
                    continuation: serde_json::json!({ "asked": "user_name" }),
                },
            ),

            "test_input_required_result_sampling" => (
                "I need your model to answer a question first.",
                MrtrSignal {
                    input_requests: one_request(
                        "capital_question",
                        sample("What is the capital of France?", 100),
                    ),
                    continuation: serde_json::json!({ "asked": "capital_question" }),
                },
            ),

            "test_input_required_result_list_roots" => (
                "I need to know which roots your client exposes.",
                MrtrSignal {
                    input_requests: one_request("client_roots", InputRequest::ListRoots),
                    continuation: serde_json::json!({ "asked": "client_roots" }),
                },
            ),

            "test_input_required_result_multi_round" => (
                "Step 1 of 2.",
                MrtrSignal {
                    input_requests: one_request(
                        "step1",
                        elicit("Step 1: What is your name?", one_string_schema("name")),
                    ),
                    // The step number rides INSIDE the sealed continuation, so
                    // the client cannot skip ahead by editing the token — the
                    // AEAD verdict rejects any edit before dispatch.
                    continuation: serde_json::json!({ "step": 1 }),
                },
            ),

            "test_input_required_result_multiple_inputs" => (
                "I need three different answers before I can finish.",
                MrtrSignal {
                    input_requests: all_three_requests(),
                    continuation: serde_json::json!({ "asked": "all-three" }),
                },
            ),

            "test_input_required_result_request_state" => (
                "I need a confirmation before I can finish.",
                MrtrSignal {
                    input_requests: one_request(
                        "confirm",
                        elicit("Please confirm", one_bool_schema("ok")),
                    ),
                    continuation: serde_json::json!({ "stage": "awaiting-confirm" }),
                },
            ),

            "test_input_required_result_tampered_state" => (
                "I need a confirmation before I can finish.",
                MrtrSignal {
                    input_requests: one_request(
                        "confirm",
                        elicit("Please confirm", one_bool_schema("ok")),
                    ),
                    continuation: serde_json::json!({ "stage": "tamper-probe" }),
                },
            ),

            // The only tool that reads the client's declaration. Every OTHER
            // arm asks for what its scenario dictates and lets the SDK reject
            // an undeclared capability wholesale with -32021; this one is the
            // scenario that grades the server for asking narrowly in the first
            // place.
            "test_input_required_result_capabilities" => (
                "I need whatever inputs your client actually declared it can answer.",
                MrtrSignal {
                    input_requests: capability_scoped_requests(extra.client_capabilities()),
                    continuation: serde_json::json!({ "asked": "capability-scoped" }),
                },
            ),

            // `server-stateless` reads up to three frames off this call's
            // response stream and fails the server if any frame is an
            // independent JSON-RPC REQUEST. Answering with one `input_required`
            // result is exactly the v2 shape that requirement exists to force.
            "test_streaming_elicitation" => (
                "I need one confirmation, delivered as a result and not as a request.",
                MrtrSignal {
                    input_requests: one_request(
                        "stream_confirm",
                        elicit("Confirm to continue", one_bool_schema("confirmed")),
                    ),
                    continuation: serde_json::json!({ "asked": "stream_confirm" }),
                },
            ),

            // The negative case, and it is the SDK that says no. The tool asks
            // for `sampling/createMessage`; when the client declared no
            // sampling capability, `reject_undeclared_capabilities` refuses the
            // WHOLE result with -32021 and `data.requiredCapabilities` set to
            // `{"sampling": {}}` before any continuation is minted. The example
            // states a need; it does not implement the refusal.
            "test_missing_capability" => (
                "I need your model to answer before I can finish.",
                MrtrSignal {
                    input_requests: one_request(
                        "needs_sampling",
                        sample("Answer anything at all.", 16),
                    ),
                    continuation: serde_json::json!({ "asked": "needs_sampling" }),
                },
            ),

            "test_mrtr_echo_state" => (
                "Please confirm to continue",
                MrtrSignal {
                    input_requests: one_request("confirm", confirm_elicitation("Confirm?")),
                    continuation: serde_json::json!({ "probe": "echo_state" }),
                },
            ),

            "test_mrtr_no_state" => (
                "Please confirm to continue (no state test)",
                MrtrSignal {
                    input_requests: one_request(
                        "confirm",
                        confirm_elicitation("Confirm? (no state test)"),
                    ),
                    // DIVERGENCE, recorded rather than worked around: the
                    // suite's own mock omits `requestState` here, and pmcp
                    // cannot — `seal_input_required` writes `inputRequests` AND
                    // `requestState` unconditionally, and the reserved-field
                    // registry deletes any the egress did not mint. The
                    // continuation is therefore minimal, not absent.
                    continuation: Value::Null,
                },
            ),

            _ => return None,
        };
        Some(input_required(text, signal))
    }

    /// Round 2 (and 3): what each MRTR-shaped tool does once the client has
    /// answered.
    ///
    /// Reached ONLY with a continuation the server-owned AEAD codec
    /// authenticated, so `continuation` is server-minted and trusted. Everything
    /// read out of `inputResponses` is the opposite — it came off the wire and
    /// is validated exactly like a tool argument.
    fn mrtr_resume(
        &self,
        continuation: &Value,
        extra: &RequestHandlerExtra,
    ) -> Option<pmcp::Result<CallToolResult>> {
        // The one tool that answers a round with ANOTHER round.
        if self.name == "test_input_required_result_multi_round"
            && continuation.get("step").and_then(Value::as_u64) == Some(1)
        {
            return Some(input_required(
                "Step 2 of 2.",
                MrtrSignal {
                    input_requests: one_request(
                        "step2",
                        elicit(
                            "Step 2: What is your favorite color?",
                            one_string_schema("color"),
                        ),
                    ),
                    continuation: serde_json::json!({ "step": 2 }),
                },
            ));
        }

        let text = match self.name {
            "test_input_required_result_elicitation" => {
                format!("Hello, {}!", answered_name(extra, "user_name"))
            },
            "test_input_required_result_sampling" => format!(
                "The model answered: {}",
                sampled_text(extra, "capital_question")
                    .unwrap_or_else(|| "(no sampling response)".to_string())
            ),
            "test_input_required_result_list_roots" => {
                let roots = listed_roots(extra, "client_roots").unwrap_or_default();
                format!("The client reported {} root(s): {}", roots.len(), {
                    let joined = roots.join(", ");
                    if joined.is_empty() {
                        "(none)".to_string()
                    } else {
                        joined
                    }
                })
            },
            "test_input_required_result_multi_round" => format!(
                "Done: name={}, color={}",
                answered_name(extra, "step1"),
                elicited_string(extra, "step2", "color")
                    .unwrap_or_else(|| "(unanswered)".to_string())
            ),
            "test_input_required_result_multiple_inputs" => format!(
                "Collected: name={}, greeting={}, roots={}",
                answered_name(extra, "user_name"),
                sampled_text(extra, "greeting").unwrap_or_else(|| "(none)".to_string()),
                listed_roots(extra, "client_roots")
                    .unwrap_or_default()
                    .len()
            ),
            // The scenario requires the literal "state-ok" so a reader can tell
            // the state was received AND validated, not merely echoed.
            "test_input_required_result_request_state" => format!(
                "state-ok (stage={}, confirmed={})",
                continuation
                    .get("stage")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                elicited_bool(extra, "confirm", "ok").unwrap_or(false)
            ),
            // Reached ONLY when the resent token verified. A tampered token
            // never gets here: the SDK's ingress answers -32602 before dispatch,
            // which is the rejection this scenario grades. The example owns no
            // integrity check of its own — the scenario is testing pmcp.
            "test_input_required_result_tampered_state" => {
                "state-ok: the resent requestState verified".to_string()
            },
            "test_input_required_result_capabilities" => {
                "capability-scoped inputs received".to_string()
            },
            "test_streaming_elicitation" => "streaming-elicitation-ok".to_string(),
            "test_missing_capability" => {
                "the client declared the capability, so the call completed".to_string()
            },
            "test_mrtr_echo_state" => "echo-state-ok".to_string(),
            "test_mrtr_no_state" => "no-state-ok".to_string(),
            _ => return None,
        };
        Some(Ok(CallToolResult::new(vec![Content::text(text)])))
    }
}

// ===========================================================================
// The MRTR authoring helpers (SEP-2322).
//
// These exist so every `input_required` result in this file is built the SAME
// way, out of the SDK's own types, and so no arm is tempted to hand-write the
// envelope. `MrtrSignal::into_meta_entry` is the ONE seam: the dispatch layer
// takes the signal off `_meta`, seals `continuation` into the opaque
// `requestState` with the AEAD codec keyed by `PMCP_REQUEST_STATE_KEY`, writes
// `inputRequests`, and removes the internal key before serialization.
//
// The example adds NO cryptographic primitive of its own. It cannot: the codec
// lives in `src/server/request_state.rs` and is reached only through this seam.
//
// A note on which SDK type is which, because it is easy to reach for the wrong
// one. `pmcp::types::mrtr::InputRequiredResult` is the CLIENT-side parsed twin
// of what these tools produce — it exists so a caller RECEIVES an unfulfilled
// `input_required` result instead of an empty success, and a server handler
// never constructs it. The server-side authoring type is [`MrtrSignal`], and
// the two `input_required` fields are written by the SDK's own
// `seal_input_required`, never by a handler.
// ===========================================================================

/// Attach an [`MrtrSignal`] to a `CallToolResult` so the dispatch layer seals it.
///
/// The `_meta` field is set DIRECTLY rather than through
/// `RequestHandlerExtra::set_result_meta`, and that is load-bearing. Every tool
/// here is served through `handle_output` returning `ToolOutput::Result`, and
/// that verbatim arm returns BEFORE the dispatcher drains the handler's result
/// `_meta` slot (`src/server/mod.rs`: "the verbatim `ToolOutput::Result` arm
/// above returns earlier and owns its own `_meta`"). A signal set through
/// `set_result_meta` on this path is silently dropped, and the tool ships an
/// empty success for an operation it never completed.
fn input_required(text: &str, signal: MrtrSignal) -> pmcp::Result<CallToolResult> {
    let (key, value) = signal
        .into_meta_entry()
        .map_err(|error| pmcp::Error::internal(error.to_string()))?;
    let mut meta = serde_json::Map::new();
    meta.insert(key, value);
    let mut result = CallToolResult::new(vec![Content::text(text)]);
    result._meta = Some(meta);
    Ok(result)
}

/// An [`InputRequests`] map with exactly one entry.
fn one_request(key: &str, request: InputRequest) -> InputRequests {
    let mut requests = InputRequests::new();
    requests.insert(key.to_string(), request);
    requests
}

/// A form-mode `elicitation/create` input request.
fn elicit(message: &str, requested_schema: Value) -> InputRequest {
    InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
        message: message.to_string(),
        requested_schema,
    }))
}

/// A `sampling/createMessage` input request with one user message.
fn sample(prompt: &str, max_tokens: u32) -> InputRequest {
    InputRequest::Sampling(Box::new(
        sampling_params(prompt).with_max_tokens(max_tokens),
    ))
}

/// The elicitation schema for exactly one required string field.
fn one_string_schema(field: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": { field: { "type": "string" } },
        "required": [field],
    })
}

/// The elicitation schema for exactly one required boolean field.
fn one_bool_schema(field: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": { field: { "type": "boolean" } },
        "required": [field],
    })
}

/// The three-entry `inputRequests` map — elicitation, sampling and roots/list
/// in one result — that `input-required-result-multiple-input-requests` grades.
fn all_three_requests() -> InputRequests {
    let mut requests = InputRequests::new();
    requests.insert(
        "user_name".to_string(),
        elicit("What is your name?", one_string_schema("name")),
    );
    requests.insert("greeting".to_string(), sample("Generate a greeting", 50));
    requests.insert("client_roots".to_string(), InputRequest::ListRoots);
    requests
}

/// The `inputRequests` map narrowed to what `declared` says the client can
/// answer.
///
/// The spec's MUST NOT is that a server may not RELY on an undeclared
/// capability. pmcp already enforces the hard half — an `inputRequests` map
/// naming an undeclared capability is refused wholesale with `-32021` before a
/// continuation is minted — so this is the SOFT half: ask narrowly, so a client
/// that can answer something still gets asked for it.
///
/// The declaration is CLIENT-SUPPLIED and trivially forgeable. It says only what
/// the client can ANSWER, never what it is allowed to reach, and nothing here
/// makes an access decision from it.
fn capability_scoped_requests(declared: Option<&ClientCapabilities>) -> InputRequests {
    let mut requests = InputRequests::new();
    let Some(declared) = declared else {
        return requests;
    };
    if declared.elicitation.is_some() {
        requests.insert(
            "user_name".to_string(),
            elicit("What is your name?", one_string_schema("name")),
        );
    }
    if declared.sampling.is_some() {
        requests.insert("greeting".to_string(), sample("Generate a greeting", 50));
    }
    if declared.roots.is_some() {
        requests.insert("client_roots".to_string(), InputRequest::ListRoots);
    }
    requests
}

/// Read a string field out of the client's answer to an elicitation entry.
fn elicited_string(extra: &RequestHandlerExtra, key: &str, field: &str) -> Option<String> {
    let InputResponse::Elicitation(result) = extra.input_responses()?.get(key)? else {
        return None;
    };
    result
        .content
        .as_ref()?
        .get(field)?
        .as_str()
        .map(str::to_string)
}

/// The `name` the client elicited under `key`, or a neutral placeholder.
fn answered_name(extra: &RequestHandlerExtra, key: &str) -> String {
    elicited_string(extra, key, "name").unwrap_or_else(|| "friend".to_string())
}

/// Read the text out of the client's answer to a sampling entry.
fn sampled_text(extra: &RequestHandlerExtra, key: &str) -> Option<String> {
    let InputResponse::Sampling(result) = extra.input_responses()?.get(key)? else {
        return None;
    };
    match &result.content {
        Content::Text { text } => Some(text.clone()),
        other => Some(format!("{other:?}")),
    }
}

/// Read the URIs out of the client's answer to a `roots/list` entry.
fn listed_roots(extra: &RequestHandlerExtra, key: &str) -> Option<Vec<String>> {
    let InputResponse::Roots(result) = extra.input_responses()?.get(key)? else {
        return None;
    };
    Some(result.roots.iter().map(|root| root.uri.clone()).collect())
}

/// The boolean-confirmation elicitation the `test_mrtr_*` tools share.
fn confirm_elicitation(prompt: &str) -> InputRequest {
    elicit(
        prompt,
        serde_json::json!({
            "type": "object",
            "properties": { "confirmed": { "type": "boolean", "description": prompt } },
        }),
    )
}

/// Read a boolean field out of the client's answer to an elicitation entry.
///
/// Every value here is CLIENT-SUPPLIED and is validated exactly like a tool
/// argument: a missing key, a declined elicitation or a wrong-shaped answer all
/// return `None` rather than a default that would look like a real answer.
fn elicited_bool(extra: &RequestHandlerExtra, key: &str, field: &str) -> Option<bool> {
    let InputResponse::Elicitation(result) = extra.input_responses()?.get(key)? else {
        return None;
    };
    result.content.as_ref()?.get(field)?.as_bool()
}

/// The sampling request shape the `tools-call-sampling` scenario specifies.
fn sampling_params(prompt: &str) -> CreateMessageParams {
    CreateMessageParams::new(vec![SamplingMessage::new(
        Role::User,
        SamplingMessageContent::from_content(Content::text(prompt)),
    )])
    .with_max_tokens(100)
}

/// The input schema for a tool that takes no arguments.
fn no_args() -> Value {
    serde_json::json!({ "type": "object", "properties": {} })
}

/// The input schema for a tool with exactly one required string argument.
fn one_required_string(name: &str, description: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": { name: { "type": "string", "description": description } },
        "required": [name],
    })
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // WARN is deliberately NOT part of this server's startup contract, so the
    // filter stays at INFO for the example's own target and pmcp's INFO lines.
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info,s54_v2_dual_conformance=info")
        .init();

    let requested: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string())
        .parse()?;

    // Presence check only. `std::env::var` returns the VALUE; it is dropped
    // immediately and never reaches a print or a log line.
    let request_state_key_present = std::env::var(REQUEST_STATE_KEY_VAR).is_ok();

    // The accept-list is the single call that opts this process into
    // 2026-07-28. Listing 2025-11-25 alongside it is what keeps v1 clients
    // working: the era is negotiated PER REQUEST, so one binary serves both.
    // Both strings come from pmcp's own constants — a retyped literal here
    // could drift from the crate without any compiler complaint.
    let mut builder = Server::builder()
        .name("s54-v2-dual-conformance")
        .version("1.0.0")
        .capabilities(conformance_capabilities())
        .with_supported_protocol_versions([
            ProtocolVersion(LATEST_PROTOCOL_VERSION.to_string()),
            ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
        ])
        .resources(ConformanceResources);

    for name in PROMPT_NAMES {
        builder = builder.prompt(name, ConformancePrompt { name });
    }
    for name in TOOL_NAMES {
        builder = builder.tool(name, ConformanceTool { name });
    }

    let server = builder.build()?;

    // `StreamableHttpServerConfig::default()`, NOT `::stateless()`.
    // `tests/common/v2.rs:371-385` states the rule: `stateless()` is a
    // BUILD-TIME config that removes the session machinery before a request is
    // ever seen, so it could never prove that the PER-REQUEST era gate is what
    // suppresses sessions on v2. The v1 session-lifecycle scenario also needs a
    // live session path to exercise.
    let http = StreamableHttpServer::with_config(
        requested,
        Arc::new(Mutex::new(server)),
        StreamableHttpServerConfig::default(),
    );

    // `start()` returns AFTER the socket is bound — that is the readiness
    // guarantee a driver script polls against.
    let (addr, server_handle) = match http.start().await {
        Ok(bound) => bound,
        Err(error) => {
            eprintln!("FATAL: could not bind {requested}: {error}");
            eprintln!(
                "  A previous run of this example may still hold the port. Check with \
                 `lsof -nP -iTCP:{} -sTCP:LISTEN` and stop it, or pass a free address as \
                 argv[1] (e.g. `-- 127.0.0.1:8155`).",
                requested.port()
            );
            std::process::exit(1);
        },
    };

    print_instructions(addr, request_state_key_present);

    // A server, not a one-shot script: run until signalled.
    server_handle.await?;
    Ok(())
}

/// The capability set the 2025-11-25 scored scenarios need to be REACHABLE.
///
/// A capability the server does not advertise makes its scenarios unreachable
/// rather than merely failing, which is a worse outcome: an unreachable
/// scenario reports an error that looks like a transport fault. `logging`
/// backs `logging-set-level`, `completions` backs `completion/complete`, and
/// `resources.subscribe` backs `resources-subscribe` / `resources-unsubscribe`.
fn conformance_capabilities() -> ServerCapabilities {
    // `ServerCapabilities` is `#[non_exhaustive]`, so it is built by mutating a
    // `Default` rather than with a struct literal.
    let mut capabilities = ServerCapabilities::default();
    capabilities.tools = Some(ToolCapabilities {
        list_changed: Some(false),
    });
    capabilities.prompts = Some(PromptCapabilities {
        list_changed: Some(false),
    });
    capabilities.resources = Some(ResourceCapabilities {
        subscribe: Some(true),
        list_changed: Some(false),
    });
    capabilities.logging = Some(LoggingCapabilities::default());
    capabilities.completions = Some(CompletionCapabilities::default());
    capabilities
}

/// Print the bound address, both suite invocations, and copy-pasteable `curl`
/// lines for each era.
///
/// Header names come from pmcp's own constants rather than retyped strings, so
/// a rename in the crate shows up here as a compile error instead of a stale
/// instruction.
fn print_instructions(addr: SocketAddr, request_state_key_present: bool) {
    println!();
    println!("=============================================================");
    println!("  DUAL-VERSION CONFORMANCE TARGET (Phase 118, CONF-01)");
    println!("=============================================================");
    println!("  Listening on : {addr}");
    println!("  Endpoint     : http://{addr}");
    println!(
        "  Versions     : {LATEST_PROTOCOL_VERSION} (v1) and {PROTOCOL_VERSION_2026_07_28} (v2)"
    );
    println!("  Sessions     : LIVE (StreamableHttpServerConfig::default)");
    println!("  Tasks ext    : absent by decision (D-14)");
    if !request_state_key_present {
        println!();
        println!("  NOTE: {REQUEST_STATE_KEY_VAR} is not set, so MRTR continuations are");
        println!("  sealed with a per-process key. Single-instance runs are fine; set the");
        println!("  variable (32 bytes, hex or base64) for anything load-balanced. CI sets");
        println!("  it. The value is never printed by this example.");
    }
    println!("-------------------------------------------------------------");
    println!("  RUN THE OFFICIAL SUITE — one process, two requirement sets:");
    println!();
    println!("    conformance server --url http://{addr}/ \\");
    println!("      --requirements 2025-11-25 -o target/conformance-results/2025-11-25");
    println!();
    println!("    conformance server --url http://{addr}/ \\");
    println!("      --requirements 2026-07-28 -o target/conformance-results/2026-07-28");
    println!();
    println!("  The suite binary is the PINNED one — see conformance/README.md.");
    println!("-------------------------------------------------------------");
    println!("  v1 ({LATEST_PROTOCOL_VERSION}) — a plain tools/list POST:");
    println!();
    println!("    curl -sS http://{addr} \\");
    println!("      -H 'content-type: application/json' \\");
    println!("      -H 'accept: {ACCEPT_STREAMABLE}' \\");
    println!("      -H '{MCP_PROTOCOL_VERSION}: {LATEST_PROTOCOL_VERSION}' \\");
    println!("      -d '{}'", v1_tools_list_body());
    println!();
    println!("-------------------------------------------------------------");
    println!("  v2 ({PROTOCOL_VERSION_2026_07_28}) — the SAME endpoint, no handshake:");
    println!();
    println!("    curl -sS http://{addr} \\");
    println!("      -H 'content-type: application/json' \\");
    println!("      -H 'accept: {ACCEPT_STREAMABLE}' \\");
    println!("      -H '{MCP_PROTOCOL_VERSION}: {PROTOCOL_VERSION_2026_07_28}' \\");
    println!("      -d '{}'", v2_tools_list_body());
    println!("=============================================================");
    println!();
    println!("Press Ctrl+C to stop the server");
}

/// A v1 `tools/list` body — era declared by the HTTP header alone.
fn v1_tools_list_body() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {},
    })
    .to_string()
}

/// A v2 `tools/list` body — the era ALSO travels in `params._meta`, which is
/// what makes the very first byte a client sends a real request rather than an
/// `initialize` handshake.
fn v2_tools_list_body() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {
            "_meta": {
                pmcp::testing::META_PROTOCOL_VERSION: PROTOCOL_VERSION_2026_07_28,
            },
        },
    })
    .to_string()
}
