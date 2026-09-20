//! Phase 115-07 (SCHM-03): **on-the-wire proof that every `2026-07-28`
//! `CacheableResult` carries `ttlMs` and `cacheScope`, and that a `2025-11-25`
//! response never does.**
//!
//! 115-06 proved the projection at unit level over a synthetic
//! `JSONRPCResponse`. This file proves the WHOLE path — the dispatcher builds
//! the result, serde serializes it, `inject_v2_result_envelope` projects it, the
//! HTTP transport writes it — because that is the only level at which "every v2
//! list/read response carries both fields" is a checkable claim.
//!
//! # The six cacheable methods, not five
//!
//! SCHM-03's requirement text and `115-CONTEXT.md` both say "five". The pinned
//! `2026-07-28` schema has SIX: `DiscoverResult` extends `CacheableResult` too,
//! and `server/discover` is the FIRST call a v2 client makes. A suite with
//! exactly five method tests is the shape of that defect, so the sixth gets a
//! dedicated, separately-named test rather than a loop entry — see
//! [`v2_caching_hints_discover_is_the_sixth_cacheable_result`].
//!
//! # Why every v2 assertion starts with an era witness
//!
//! `inject_v2_result_envelope` (`src/server/core.rs:1637`) adds `resultType`
//! ONLY when the resolved era is `Era::V2`. Its presence in a response is
//! therefore in-band, SERVER-MINTED proof that the dispatcher really resolved
//! v2 — not a restatement of what the test intended. A "v2" test that skips it
//! can be silently running as v1 with every downstream assertion still passing,
//! which is precisely the defect the cross-AI review found in the pre-review
//! plan set. [`assert_v2_era_witness`] runs FIRST in every v2 test here, and
//! [`v2_caching_hints_the_v2_era_witness_is_load_bearing`] proves the witness
//! itself discriminates.
//!
//! **Grepping for the witness:** the wire spelling lives in ONE place, the
//! [`RESULT_TYPE_KEY`] constant, exactly as `tests/common/duplex.rs` keeps it —
//! so a reviewer counting era witnesses should grep `assert_v2_era_witness` /
//! `assert_no_v2_era_witness` (one per era-sensitive test), not the literal
//! `resultType`. Inlining the literal at every call site would put eight copies
//! of a wire spelling in this file, which is the drift this repo has already
//! been bitten by (`tests/common/v2.rs:673-681`, a hand-copied header encoder
//! that silently diverged from the shipped one).
//!
//! # Why every test name begins with `v2_caching_hints_`
//!
//! `115-RESEARCH.md` § Pitfall 4 MEASURED that `cargo nextest run -E
//! 'test(/stem/)'` against a file whose test names lack the file stem selects
//! ZERO tests and exits 0 — a plan can be "verified" having run nothing, and
//! that exact form appears in `114-16-PLAN.md`. Prefixing every name means both
//! `binary(v2_caching_hints)` and `test(/v2_caching_hints/)` select this file.
//!
//! # Both native dispatchers
//!
//! Pitfall 6 (twin-site drift) is a recurring defect class in this repo: the
//! high-level `Server` has its own dispatch and its own injection call at
//! `src/server/mod.rs:1723`, distinct from `ServerCore`'s at
//! `src/server/core.rs:3404`. The HTTP half of this file exercises `Server`;
//! the `server_core_*` half exercises `ServerCore` in-process. See the
//! `server_core` module's docs for the MEASURED bound on which methods the
//! in-process half can reach at all.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

// The era-aware in-process duplex seam, included per-crate exactly as its own
// module docs prescribe. Declared at the top level (not inside the `server_core`
// module that consumes it) because `#[path]` on a nested module resolves
// relative to the ENCLOSING module's directory, which for an inline module here
// would be `tests/v2_caching_hints/`.
#[path = "common/duplex.rs"]
mod duplex;

use async_trait::async_trait;
use common::v2::{
    post, spawn_stateless_config, teardown, v1_body, v2_body, v2_headers, Resp, V1, V2,
};
use pmcp::server::typed_tool::TypedTool;
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::types::protocol::error_codes::{INVALID_REQUEST, METHOD_NOT_FOUND};
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::CacheScope;
use pmcp::types::{
    Content, GetPromptResult, ListResourcesResult, PromptInfo, ReadResourceResult, ResourceInfo,
};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::collections::HashMap;

// ===========================================================================
// Wire spellings.
// ===========================================================================

/// The v2 result-envelope discriminator, and this file's era witness.
///
/// pmcp's own `crate::types::mrtr::RESULT_TYPE_KEY` is `pub(crate)`, so an
/// integration-test crate cannot read it. It is asserted on, never emitted, so
/// a drift shows up as a failing witness rather than a wrong request.
const RESULT_TYPE_KEY: &str = "resultType";

/// The `CacheableResult.ttlMs` wire spelling — camelCase, per the schema.
const TTL_MS_KEY: &str = "ttlMs";

/// The `CacheableResult.cacheScope` wire spelling — camelCase, per the schema.
const CACHE_SCOPE_KEY: &str = "cacheScope";

/// The SDK-supplied `ttlMs` default (D-08).
///
/// SOURCED from the crate, not restated: `DEFAULT_TTL_MS` is a `pub` item of
/// `src/types/caching.rs` re-exported at `pmcp::types::DEFAULT_TTL_MS`, which is
/// exactly the path `examples/s52_v2_caching_hints.rs` imports. Copying the
/// literal `0` here would let this suite keep asserting the old default if the
/// SDK ever changed it — the opposite of what a wire-conformance suite is for.
const DEFAULT_TTL_MS: u64 = pmcp::types::DEFAULT_TTL_MS;

/// The SDK-supplied `cacheScope` default (D-08): the value that cannot leak
/// across authorization contexts.
///
/// Derived by SERIALIZING [`CacheScope::default()`] rather than by typing the
/// string, for the same reason `project_caching_hints` injects it that way: the
/// assertion and the enum cannot drift apart.
fn default_cache_scope() -> String {
    serde_json::to_value(CacheScope::default())
        .expect("a unit enum always serializes")
        .as_str()
        .expect("CacheScope serializes to a JSON string")
        .to_string()
}

// ===========================================================================
// Assertion helpers.
// ===========================================================================

/// Borrow the `result` object of a 200 response, panicking with the raw text.
fn result_of<'a>(response: &'a Resp, ctx: &str) -> &'a Value {
    assert_eq!(
        response.status, 200,
        "{ctx}: expected HTTP 200, raw response was: {}",
        response.raw
    );
    response.body.get("result").unwrap_or_else(|| {
        panic!(
            "{ctx}: the response carries no `result` at all, raw response was: {}",
            response.raw
        )
    })
}

/// Assert the dispatcher actually resolved `Era::V2` for this request.
///
/// **Call this FIRST in every v2 test.** Without it the test proves NOTHING
/// about v2: `inject_v2_result_envelope` (`src/server/core.rs`) writes the
/// envelope — `resultType` included — ONLY inside its `Some(Era::V2)` branch, so
/// the same request against a server that never opted in is served as v1 and the
/// assertion that follows would be measuring the wrong era's behaviour. (The
/// function does NOT early-return on a non-v2 era: the caching-hint projection
/// above the gate runs on both. Only the envelope half is v2-gated.)
fn assert_v2_era_witness(response: &Resp, ctx: &str) {
    let result = result_of(response, ctx);
    assert!(
        result.get(RESULT_TYPE_KEY).is_some(),
        "{ctx}: no `{RESULT_TYPE_KEY}` in the result, so the dispatcher did NOT resolve Era::V2 \
         for this request. Every caching-hint assertion after this line would be measuring the \
         v1 path under a v2 test name. Check the fixture's \
         `with_supported_protocol_versions` opt-in and the request's `_meta` \
         protocol-version signal. Raw response was: {}",
        response.raw
    );
}

/// Assert the dispatcher resolved v1: the mirror of [`assert_v2_era_witness`].
///
/// Absence of `resultType` is proof of v1 for the same reason its presence is
/// proof of v2 — the v2 envelope injector is that key's only writer.
fn assert_no_v2_era_witness(response: &Resp, ctx: &str) {
    let result = result_of(response, ctx);
    assert!(
        result.get(RESULT_TYPE_KEY).is_none(),
        "{ctx}: found `{RESULT_TYPE_KEY}` in the result, so the dispatcher resolved Era::V2 for a \
         request that was supposed to be served as v1. Raw response was: {}",
        response.raw
    );
}

/// Assert both hints are present on the wire with the SAFE SDK defaults.
///
/// Three assertions, deliberately: the two parsed values (D-08 — the default is
/// the inert, non-leaking one) and the two RAW key spellings (D-07 — both keys
/// are required on the v2 projection). The raw check is not redundant: a
/// struct-level `rename_all` regression that emitted `ttl_ms` / `cache_scope`
/// is invisible to a parsed-value assertion that looks the keys up by their
/// camelCase names, because the lookup would simply return `None` and the
/// message would blame the projection rather than the rename.
fn assert_default_hints(response: &Resp, ctx: &str) {
    assert_hints(response, ctx, DEFAULT_TTL_MS, &default_cache_scope());
}

/// [`assert_default_hints`] for a handler-chosen pair of values.
fn assert_hints(response: &Resp, ctx: &str, ttl_ms: u64, cache_scope: &str) {
    let result = result_of(response, ctx);
    let sdk_default_scope = default_cache_scope();

    assert_eq!(
        result.get(TTL_MS_KEY),
        Some(&json!(ttl_ms)),
        "{ctx}: D-07 makes `{TTL_MS_KEY}` REQUIRED on every v2 `CacheableResult`, and D-08 fixes \
         the SDK default at {DEFAULT_TTL_MS} (immediately stale, which asserts nothing about \
         cacheability). Expected {ttl_ms}. Raw response was: {}",
        response.raw
    );
    assert_eq!(
        result.get(CACHE_SCOPE_KEY),
        Some(&json!(cache_scope)),
        "{ctx}: D-07 makes `{CACHE_SCOPE_KEY}` REQUIRED on every v2 `CacheableResult`, and D-08 \
         fixes the SDK default at `{sdk_default_scope}` — marking an un-considered response \
         `public` authorizes a shared gateway to serve one caller's body to another caller \
         holding a different access token. Expected `{cache_scope}`. Raw response was: {}",
        response.raw
    );

    assert!(
        response.raw.contains(r#""ttlMs""#),
        "{ctx}: the RAW wire must spell the key `ttlMs` (camelCase). A struct-level \
         `rename_all` regression emitting `ttl_ms` is invisible to a parsed-value assertion. \
         Raw response was: {}",
        response.raw
    );
    assert!(
        response.raw.contains(r#""cacheScope""#),
        "{ctx}: the RAW wire must spell the key `cacheScope` (camelCase). A struct-level \
         `rename_all` regression emitting `cache_scope` is invisible to a parsed-value \
         assertion. Raw response was: {}",
        response.raw
    );
}

/// The name of the first caching hint found on `wire`, if any.
///
/// # Why this is a function returning `Option`, not an inline `assert!`
///
/// The same reason `tests/v1_lists_golden.rs:309` factors its `v1_leak_guard`
/// out: an absence assertion that is never itself exercised is
/// indistinguishable from one wired to the wrong string or one that can never
/// fire, and the moment anybody would find out is the moment it was supposed to
/// catch a real leak. Returning the key lets
/// [`v2_caching_hints_the_no_hints_guard_is_load_bearing`] drive synthetic
/// leaking wires through the SAME predicate the real assertions use — no
/// `catch_unwind`, no test-only duplicate.
///
/// The check is on the RAW text rather than on the parsed `result`, because a
/// hint that leaked into a nested object (`result._meta`, a `contents` element)
/// is still a hint on a wire that must not carry one.
fn leaked_hint_key(wire: &str) -> Option<&'static str> {
    [TTL_MS_KEY, CACHE_SCOPE_KEY]
        .into_iter()
        .find(|key| wire.contains(key))
}

/// Assert NEITHER hint key appears anywhere in `wire`.
///
/// Shared by the HTTP half ([`assert_no_hints`]) and the in-process
/// `ServerCore` half, so the two dispatchers are held to one predicate rather
/// than two that could drift.
fn assert_no_hints_in(wire: &str, ctx: &str) {
    assert!(
        leaked_hint_key(wire).is_none(),
        "{ctx}: the response carries the SCHM-03 caching hint \
         `{}` where it must carry neither. D-11 era-gates the hints OFF on v1, and a v1 \
         response carrying a v2 field breaks this milestone's severability story: Phases \
         116-119 all rest on v1 responses staying byte-identical. Fix the projection — never \
         relax this assertion. Wire was: {wire}",
        leaked_hint_key(wire).unwrap_or("<none>")
    );
}

/// [`assert_no_hints_in`] over an HTTP response's raw text.
fn assert_no_hints(response: &Resp, ctx: &str) {
    assert_no_hints_in(&response.raw, ctx);
}

/// Anti-vacuity for [`leaked_hint_key`]: it must DISCRIMINATE, not reject
/// everything.
///
/// Every `assert_no_hints` call in this file passes on a response that
/// genuinely carries no hint, so all of them are equally satisfied by a guard
/// that works and by one that returns `None` unconditionally. This drives
/// synthetic wires that DO carry each key, and a clean one that carries neither
/// — because a guard that rejected EVERYTHING would satisfy the leak cases
/// perfectly while failing every real fixture for the wrong reason.
/// Discrimination is the property under test, not rejection.
#[test]
fn v2_caching_hints_the_no_hints_guard_is_load_bearing() {
    const CLEAN: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"contents":[],"nextCursor":"c"}}"#;
    assert_eq!(
        leaked_hint_key(CLEAN),
        None,
        "a clean wire must PASS the guard — one that rejects everything would satisfy the leak \
         cases below while proving nothing"
    );

    for (key, wire) in [
        (
            TTL_MS_KEY,
            r#"{"jsonrpc":"2.0","id":1,"result":{"contents":[],"ttlMs":0}}"#,
        ),
        (
            CACHE_SCOPE_KEY,
            r#"{"jsonrpc":"2.0","id":1,"result":{"contents":[],"cacheScope":"private"}}"#,
        ),
    ] {
        assert_eq!(
            leaked_hint_key(wire),
            Some(key),
            "the guard must REJECT a wire carrying `{key}` and must NAME it, so a future reader \
             knows which field leaked"
        );
    }
}

// ===========================================================================
// Fixture: a server whose handlers set NO caching hint at all.
// ===========================================================================

/// The one URI [`HintFreeResources::read`] serves.
const HINT_FREE_URI: &str = "hints://free/one.txt";

/// A resource handler that expresses NO caching preference.
///
/// Every hint on the wire in the six default tests below is therefore
/// SDK-supplied, which is exactly what D-08 is about.
///
/// # There is no `resources/templates/list` hook to be hint-free about
///
/// MEASURED (and already recorded by 115-02): `ResourceHandler` declares only
/// `read` and `list` (`src/server/mod.rs:368-382`). Both native dispatchers
/// return `resource_templates: vec![]` unconditionally
/// (`src/server/mod.rs:2498` and `src/server/core.rs:1015`), so a handler
/// cannot influence that result at all — neither its entries nor its hints.
/// That makes `resources/templates/list` the THINNEST of the six results and
/// therefore the one where an injected `ttlMs` / `cacheScope` is most
/// conspicuous, which is why it still gets its own test.
struct HintFreeResources;

#[async_trait]
impl ResourceHandler for HintFreeResources {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        Ok(ReadResourceResult::new(vec![Content::resource_with_text(
            uri,
            "a hint-free resource body",
            "text/plain",
        )]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo::new(HINT_FREE_URI, "one").with_mime_type("text/plain"),
            ResourceInfo::new("hints://free/two.txt", "two").with_mime_type("text/plain"),
        ]))
    }
}

/// A trivial tool, so `tools/list` has real entries to list and `tools/call`
/// has a real dispatch target for the non-cacheable control.
fn fixture_tool(name: &'static str) -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(name, json!({ "type": "object" }), |_args: Value, _extra| {
        Box::pin(async { Ok(json!({ "ok": true })) })
    })
    .with_description("a hint-free fixture tool")
}

/// A trivial prompt, so `prompts/list` has real entries to list.
struct FixturePrompt(&'static str);

#[async_trait]
impl PromptHandler for FixturePrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(vec![], None))
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo::new(self.0).with_description("a hint-free fixture prompt"))
    }
}

/// The names the first fixture tool and prompt register under.
const TOOL_ALPHA: &str = "hint_free_alpha";
const PROMPT_ONE: &str = "hint_free_one";

/// The v2-OPTED-IN, hint-free fixture server.
///
/// The opt-in uses the SAME mechanism `tests/common/v2.rs:298` uses — the
/// builder's `with_supported_protocol_versions` extended with BOTH [`V1`] and
/// [`V2`], both sourced from pmcp's own constants. There is deliberately no
/// second opt-in path invented here: without this call
/// `resolve_ingress_protocol_context` short-circuits before it ever reads
/// `_meta` (D-04) and every v2 assertion in this file would be vacuous.
fn hint_free_server() -> Server {
    hint_free_builder(true)
}

/// The NOT-opted-in twin, identical in every other respect.
///
/// Used only by [`v2_caching_hints_the_v2_era_witness_is_load_bearing`], where
/// the two servers differ by exactly one builder call so the ERA is the only
/// variable.
fn not_opted_in_server() -> Server {
    hint_free_builder(false)
}

fn hint_free_builder(opt_in_v2: bool) -> Server {
    let mut builder = Server::builder().name("v2-caching-hints").version("1.0.0");
    if opt_in_v2 {
        builder = builder.with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ]);
    }
    builder
        .tool(TOOL_ALPHA, fixture_tool(TOOL_ALPHA))
        .tool("hint_free_beta", fixture_tool("hint_free_beta"))
        .prompt(PROMPT_ONE, FixturePrompt(PROMPT_ONE))
        .prompt("hint_free_two", FixturePrompt("hint_free_two"))
        .resources(HintFreeResources)
        .build()
        .expect("the hint-free caching fixture server builds")
}

// ===========================================================================
// Fixture: a server whose handlers DO set caching hints.
// ===========================================================================

/// The one URI [`HintedResources::read`] serves.
const HINTED_URI: &str = "hints://set/one.txt";

/// The `ttlMs` the handler sets on its `ListResourcesResult`.
const LIST_TTL_MS: u64 = 300_000;

/// The `ttlMs` the handler sets on its `ReadResourceResult`.
///
/// Deliberately DIFFERENT from [`LIST_TTL_MS`], and paired with a different
/// scope, so a projection bug that carried one result's hints onto the other
/// cannot pass by coincidence.
const READ_TTL_MS: u64 = 60_000;

/// A resource handler that expresses a REAL caching preference on both of its
/// results, through the 115-05 builders.
///
/// This is what makes those builders meaningful: without a fixture that
/// genuinely opts in, "the projection preserves a handler-set value" and "the
/// projection strips a handler-set value on v1" are both unfalsifiable.
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
        .with_ttl_ms(READ_TTL_MS)
        .with_cache_scope(CacheScope::Private))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![
            ResourceInfo::new(HINTED_URI, "one").with_mime_type("text/plain")
        ])
        .with_ttl_ms(LIST_TTL_MS)
        .with_cache_scope(CacheScope::Public))
    }
}

/// The v2-OPTED-IN server whose `ResourceHandler` sets real hints.
fn hinted_server() -> Server {
    Server::builder()
        .name("v2-caching-hints-set")
        .version("1.0.0")
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .resources(HintedResources)
        .build()
        .expect("the handler-set caching fixture server builds")
}

// ===========================================================================
// Round trips.
// ===========================================================================

/// Spawn `server` over real loopback HTTP, POST one request, shut down.
///
/// `spawn_stateless_config` carries `enable_json_response: true`, so
/// [`Resp::raw`] IS the JSON-RPC frame rather than an SSE-framed copy of it —
/// which is what makes the raw camelCase key assertions above read the actual
/// wire bytes. Teardown goes through the shared harness's one order (drop
/// sockets → `abort()` → `await`, D-113-T); `()` is passed for the sockets
/// because every request here goes through the harness's pooled client.
async fn round_trip(server: Server, headers: &[(String, String)], body: &str) -> Resp {
    let (addr, handle) = spawn_stateless_config(server).await;
    let response = post(addr, headers, body).await;
    teardown(handle, ()).await;
    response
}

/// One v2 request against the hint-free fixture server.
async fn v2_round_trip(method: &str, name: &str, id: i64, params: Value) -> Resp {
    round_trip(
        hint_free_server(),
        &v2_headers(method, name),
        &v2_body(method, json!(id), params),
    )
    .await
}

/// One v1 request against the hint-free fixture server: no v2 headers, no
/// reserved `_meta`, which is exactly what a real v1 client sends.
async fn v1_round_trip(method: &str, id: i64, params: Value) -> Resp {
    round_trip(hint_free_server(), &[], &v1_body(method, json!(id), params)).await
}

// ===========================================================================
// The six cacheable methods on v2, with the SAFE defaults.
// ===========================================================================

#[tokio::test]
async fn v2_caching_hints_tools_list_carries_the_defaults() {
    let response = v2_round_trip("tools/list", "", 1, json!({})).await;

    assert_v2_era_witness(&response, "v2 tools/list");
    assert_default_hints(&response, "v2 tools/list");
}

#[tokio::test]
async fn v2_caching_hints_prompts_list_carries_the_defaults() {
    let response = v2_round_trip("prompts/list", "", 2, json!({})).await;

    assert_v2_era_witness(&response, "v2 prompts/list");
    assert_default_hints(&response, "v2 prompts/list");
}

#[tokio::test]
async fn v2_caching_hints_resources_list_carries_the_defaults() {
    let response = v2_round_trip("resources/list", "", 3, json!({})).await;

    assert_v2_era_witness(&response, "v2 resources/list");
    assert_default_hints(&response, "v2 resources/list");
}

/// The thinnest of the six results: both dispatchers hardcode
/// `resource_templates: vec![]`, so everything else in this response is
/// SDK-supplied and an injected hint is maximally conspicuous.
#[tokio::test]
async fn v2_caching_hints_resources_templates_list_carries_the_defaults() {
    let response = v2_round_trip("resources/templates/list", "", 4, json!({})).await;

    assert_v2_era_witness(&response, "v2 resources/templates/list");
    assert_default_hints(&response, "v2 resources/templates/list");
}

#[tokio::test]
async fn v2_caching_hints_resources_read_carries_the_defaults() {
    let response = v2_round_trip(
        "resources/read",
        HINT_FREE_URI,
        5,
        json!({ "uri": HINT_FREE_URI }),
    )
    .await;

    assert_v2_era_witness(&response, "v2 resources/read");
    assert_default_hints(&response, "v2 resources/read");
}

/// **The sixth `CacheableResult` extender — the one the requirement text does
/// not count.**
///
/// SCHM-03's requirement text and `115-CONTEXT.md` both say "five list/read
/// results". The pinned `2026-07-28` schema declares SIX: `DiscoverResult`
/// extends `CacheableResult` alongside `ListToolsResult`, `ListPromptsResult`,
/// `ListResourcesResult`, `ListResourceTemplatesResult` and
/// `ReadResourceResult`. `tests/v2_core_schema_facts.rs` asserts that set
/// against the vendored artifact, and `115-RESEARCH.md` § Finding 5 measured it.
///
/// This test is deliberately NOT folded into the five above, and is named for
/// the discrepancy, so a future reader who notices the count disagreeing with
/// the requirement finds the answer here rather than re-deriving it. Excluding
/// `server/discover` would ship a knowingly non-conformant v2 discover — the
/// FIRST call a v2 client makes.
///
/// `server/discover` also reaches the projection by a different route from the
/// other five: it rides the crate-private internal-request path
/// (`Server::handle_discover` → `core::build_discover_response`), NOT the
/// `ClientRequest` dispatch, so its hints come from the `Cacheable::Yes` named
/// at `src/server/core.rs:1935` rather than from `request_is_cacheable`. Two
/// routes, one projection point — this test is what proves they agree.
#[tokio::test]
async fn v2_caching_hints_discover_is_the_sixth_cacheable_result() {
    let response = v2_round_trip("server/discover", "", 6, json!({})).await;

    assert_v2_era_witness(&response, "v2 server/discover");
    assert_default_hints(&response, "v2 server/discover");
}

// ===========================================================================
// Fail-closed: a method that is NOT a `CacheableResult` gains neither key.
// ===========================================================================

/// The fail-closed direction for `request_is_cacheable`
/// (`src/server/core.rs:1700`).
///
/// `tools/call` returns a `CallToolResult`, which does NOT extend
/// `CacheableResult` in the `2026-07-28` schema, so it must gain neither key
/// (D-07). The `resultType` assertion is what makes this meaningful: it proves
/// the request really was served as v2, so the absence of the hints is a
/// decision by the classifier rather than a side effect of never having reached
/// the v2 path at all.
#[tokio::test]
async fn v2_caching_hints_non_cacheable_methods_gain_neither_key() {
    let response = v2_round_trip(
        "tools/call",
        TOOL_ALPHA,
        7,
        json!({ "name": TOOL_ALPHA, "arguments": {} }),
    )
    .await;

    assert_v2_era_witness(&response, "v2 tools/call (non-cacheable)");
    assert_no_hints(
        &response,
        "v2 tools/call is not a CacheableResult (D-07), so it must gain neither key",
    );
}

// ===========================================================================
// The v1 contrast, across all six methods.
// ===========================================================================

/// The same six methods on v1 carry NEITHER key.
///
/// Driven against the OPTED-IN fixture with a plain v1 body — no v2 headers, no
/// reserved `_meta` — so the server is capable of v2 and the REQUEST is what
/// selects the era. That is a strictly stronger contrast than pointing a v1
/// body at a v1-only server, which could pass for want of the capability rather
/// than because the era gate works.
///
/// `server/discover` is a v2-only method, so its v1 answer is `-32601`
/// method-not-found (D-10) rather than a result. It is included anyway and
/// asserted as such: "the v1 response carries neither hint" is true of it for a
/// different and equally load-bearing reason, and a future change that started
/// serving discover on v1 would be caught here.
#[tokio::test]
async fn v2_caching_hints_v1_methods_gain_neither_key() {
    for (id, method, params) in [
        (11_i64, "tools/list", json!({})),
        (12, "prompts/list", json!({})),
        (13, "resources/list", json!({})),
        (14, "resources/templates/list", json!({})),
        (15, "resources/read", json!({ "uri": HINT_FREE_URI })),
    ] {
        let response = v1_round_trip(method, id, params).await;
        let ctx = format!("v1 {method}");
        assert_no_v2_era_witness(&response, &ctx);
        assert_no_hints(&response, &ctx);
    }

    let discover = v1_round_trip("server/discover", 16, json!({})).await;
    assert_eq!(
        discover.body["error"]["code"], METHOD_NOT_FOUND,
        "server/discover is v2-only (D-10); a v1 request must be method-not-found, raw: {}",
        discover.raw
    );
    assert_no_hints(&discover, "v1 server/discover");
}

// ===========================================================================
// Anti-vacuity: the era witness must discriminate.
// ===========================================================================

/// Run the SAME method against the SAME server twice, changing only the era the
/// request signals.
///
/// Without this, every "v2" assertion in this file could be silently running as
/// v1 and nothing would say so. The v2 half must produce `resultType` AND both
/// hints; the v1 half must produce neither, because the projection's non-v2 arm
/// STRIPS rather than ensures (D-11). One server, one method, one variable.
#[tokio::test]
async fn v2_caching_hints_the_v2_era_witness_is_load_bearing() {
    let as_v2 = round_trip(
        hint_free_server(),
        &v2_headers("tools/list", ""),
        &v2_body("tools/list", json!(21), json!({})),
    )
    .await;
    assert_v2_era_witness(&as_v2, "opted-in server, v2-signalling tools/list");
    assert_default_hints(&as_v2, "opted-in server, v2-signalling tools/list");

    let as_v1 = round_trip(
        hint_free_server(),
        &[],
        &v1_body("tools/list", json!(22), json!({})),
    )
    .await;
    assert_no_v2_era_witness(&as_v1, "the SAME opted-in server, v1-signalling tools/list");
    assert_no_hints(
        &as_v1,
        "the SAME opted-in server serves a v1-signalling request as v1, so the projection STRIPS",
    );
}

/// **MEASURED (115-07), and the reason the contrast above is request-shaped
/// rather than server-shaped: over HTTP a non-opted-in server REFUSES a v2
/// request outright.**
///
/// `tests/structured_tool_output.rs`'s in-process twin of this anti-vacuity
/// check sends an identical `Era::V2` request to a core that never opted in and
/// gets a v1-SERVED 200 back — silently, which is what makes the era witness
/// load-bearing on that route. The HTTP transport does not behave that way: the
/// version gate answers `400` with `-32600 "Unsupported protocol version"`
/// before dispatch is reached at all, so the "v2 request silently served as v1"
/// failure mode is structurally unreachable here.
///
/// That is a STRONGER guarantee, not a missing test, and it is asserted rather
/// than assumed so a future transport change that started serving such a
/// request (as v1, hints stripped, no error) shows up as a failure here instead
/// of quietly weakening every HTTP era test in the phase.
#[tokio::test]
async fn v2_caching_hints_a_non_opted_in_server_refuses_a_v2_request_over_http() {
    let refused = round_trip(
        not_opted_in_server(),
        &v2_headers("tools/list", ""),
        &v2_body("tools/list", json!(23), json!({})),
    )
    .await;

    assert_eq!(
        refused.status, 400,
        "a non-opted-in server must REFUSE a v2 request at the HTTP boundary, raw: {}",
        refused.raw
    );
    assert_eq!(
        refused.body["error"]["code"], INVALID_REQUEST,
        "the refusal is the transport's unsupported-protocol-version gate, raw: {}",
        refused.raw
    );
    assert!(
        refused.body["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Unsupported protocol version")),
        "the refusal must name the version gate rather than some other -32600, raw: {}",
        refused.raw
    );
    assert!(
        refused.body.get("result").is_none(),
        "a refusal carries no result, so it cannot carry a projected hint either, raw: {}",
        refused.raw
    );
    assert_no_hints(&refused, "a refused v2 request");
}

// ===========================================================================
// Handler-set hints: preserved on v2, STRIPPED on v1.
// ===========================================================================

/// A handler-set hint reaches the v2 wire UNMODIFIED.
///
/// This is what makes the 115-05 builders meaningful, and it is the direct
/// on-the-wire proof that the projection uses `or_insert` semantics rather than
/// overwriting: an SDK that stamped its defaults over every result would pass
/// every default test above and fail here.
///
/// The two results carry deliberately DIFFERENT pairs — `resources/list` gets
/// 300000/`public`, `resources/read` gets 60000/`private` — so a bug that
/// carried one result's hints onto the other cannot pass by coincidence.
#[tokio::test]
async fn v2_caching_hints_handler_set_values_reach_the_wire_unmodified() {
    let list = round_trip(
        hinted_server(),
        &v2_headers("resources/list", ""),
        &v2_body("resources/list", json!(31), json!({})),
    )
    .await;
    assert_v2_era_witness(&list, "v2 resources/list, handler-set");
    assert_hints(
        &list,
        "v2 resources/list, handler-set",
        LIST_TTL_MS,
        "public",
    );

    let read = round_trip(
        hinted_server(),
        &v2_headers("resources/read", HINTED_URI),
        &v2_body("resources/read", json!(32), json!({ "uri": HINTED_URI })),
    )
    .await;
    assert_v2_era_witness(&read, "v2 resources/read, handler-set");
    assert_hints(
        &read,
        "v2 resources/read, handler-set",
        READ_TTL_MS,
        "private",
    );

    // The RAW pairs, spelled exactly as they must appear on the wire.
    //
    // Rust source cannot write the bare integers `300000` / `60000` — separators
    // are mandatory under `clippy::unreadable_literal`, which is pedantic and
    // NOT allow-listed by `make lint` — so these string literals are where the
    // un-separated wire form is pinned. They are not redundant with the parsed
    // assertions above: they prove each value reaches the wire as a JSON
    // INTEGER, not as `3e5`, `300000.0` or `"300000"`, which a `serde_json`
    // number-representation change could alter while `json!(300_000)` equality
    // still held.
    assert!(
        list.raw.contains(r#""ttlMs":300000"#) && list.raw.contains(r#""cacheScope":"public""#),
        "the handler-set list pair must reach the v2 wire verbatim, raw: {}",
        list.raw
    );
    assert!(
        read.raw.contains(r#""ttlMs":60000"#) && read.raw.contains(r#""cacheScope":"private""#),
        "the handler-set read pair must reach the v2 wire verbatim, raw: {}",
        read.raw
    );
}

/// **The single most important test in this plan.**
///
/// It is the only HTTP-level place where the STRIP half of the projection is
/// exercised end to end against a handler that genuinely opted in. Every other
/// v1 assertion in this file is over a handler that set nothing, where "no key
/// on the wire" is equally consistent with a projection that strips and one
/// that merely never adds. Here the handler DID set both hints, on both
/// results, and the v1 wire must still carry neither.
///
/// A v1 response carrying a v2 field breaks D-11 and this milestone's
/// severability story: Phases 116-119 all rest on the v1 layer staying cleanly
/// removable, which means v1 bytes staying v1 bytes. **The remedy for a failure
/// here is to fix the projection — never to relax this assertion.**
#[tokio::test]
async fn v2_caching_hints_v1_strips_handler_set_values() {
    for (id, method, params) in [
        (33_i64, "resources/list", json!({})),
        (34, "resources/read", json!({ "uri": HINTED_URI })),
    ] {
        let response = round_trip(hinted_server(), &[], &v1_body(method, json!(id), params)).await;
        let ctx = format!("v1 {method} against a handler that SET both hints");

        assert_no_v2_era_witness(&response, &ctx);
        assert_no_hints(&response, &ctx);
    }
}

// ===========================================================================
// Twin-dispatcher parity: the in-process `ServerCore` half.
// ===========================================================================

/// `ServerCore` in-process, driven through the era-aware duplex seam.
///
/// # Why only `resources/read`
///
/// MEASURED during the 2026-08-01 replan and re-verified here.
/// `ServerCore::handle_request` resolves the era via
/// `resolve_ingress_protocol_context` (`src/server/core.rs:4038`), which needs
/// BOTH the server's accept-list to be v2-opted-in AND a per-request signal read
/// by `extract_request_meta_value` (`src/server/core.rs:3997`). That extractor
/// matches EXHAUSTIVELY and returns the `_meta` object for exactly three
/// [`ClientRequest`](pmcp::types::ClientRequest) variants — `CallTool`,
/// `GetPrompt` and `ReadResource` — and `None` for every other variant,
/// INCLUDING `ListTools`, `ListPrompts`, `ListResources` and
/// `ListResourceTemplates`.
///
/// So of the six `CacheableResult` methods, only `resources/read` can reach
/// `Era::V2` through the in-process typed route at all. `server/discover` is a
/// seventh problem: it rides the crate-private internal-request path, not the
/// `ClientRequest` dispatch, so it has no in-process entry point here either.
///
/// This is a documented semver decision, not a defect — see
/// [`v2_caching_hints_list_methods_cannot_reach_v2_through_the_typed_dispatch_route`],
/// which asserts the bound so a future reader does not "fix" it. The four list
/// methods get their v2 coverage over HTTP, above, via
/// `Server::resolve_raw_meta_protocol_context`, which reads the RAW body and has
/// FULL method coverage.
mod server_core {
    use super::duplex::{
        assert_no_v2_witness, assert_v2_witness, call_tool_request, initialize_via_core,
        raw_via_core, read_resource_request, result_object, v2_accept_list,
    };
    use super::{
        assert_no_hints_in, default_cache_scope, fixture_tool, HintFreeResources, HintedResources,
        CACHE_SCOPE_KEY, DEFAULT_TTL_MS, HINTED_URI, HINT_FREE_URI, READ_TTL_MS, TOOL_ALPHA,
        TTL_MS_KEY, V2,
    };
    use pmcp::server::builder::ServerCoreBuilder;
    use pmcp::server::core::ProtocolHandler;
    use pmcp::types::jsonrpc::JSONRPCResponse;
    use pmcp::types::protocol::error_codes::V1_TASK_PENDING;
    use pmcp::types::protocol::Era;
    use pmcp::types::{ClientRequest, Request};
    use serde_json::json;
    use std::sync::Arc;

    /// A v2-OPTED-IN core carrying the hint-free resource handler and one tool.
    fn hint_free_core() -> Arc<dyn ProtocolHandler> {
        Arc::new(
            ServerCoreBuilder::new()
                .name("v2-caching-hints-core")
                .version("1.0.0")
                .with_supported_protocol_versions(v2_accept_list())
                .tool(TOOL_ALPHA, fixture_tool(TOOL_ALPHA))
                .resources(HintFreeResources)
                .build()
                .expect("the hint-free caching fixture core builds"),
        )
    }

    /// A v2-OPTED-IN core whose resource handler SETS both hints.
    fn hinted_core() -> Arc<dyn ProtocolHandler> {
        hinted_core_builder(true)
    }

    /// The NOT-opted-in twin of [`hinted_core`], for the v1 strip half.
    fn v1_hinted_core() -> Arc<dyn ProtocolHandler> {
        hinted_core_builder(false)
    }

    fn hinted_core_builder(opt_in_v2: bool) -> Arc<dyn ProtocolHandler> {
        let mut builder = ServerCoreBuilder::new()
            .name("v2-caching-hints-set-core")
            .version("1.0.0");
        if opt_in_v2 {
            builder = builder.with_supported_protocol_versions(v2_accept_list());
        }
        Arc::new(
            builder
                .resources(HintedResources)
                .build()
                .expect("the handler-set caching fixture core builds"),
        )
    }

    /// Assert a raw in-process response carries both hints with `ttl_ms` /
    /// `cache_scope`.
    ///
    /// The `ServerCore` twin of the HTTP `assert_hints`. It asserts on the
    /// SERIALIZED response as well as the parsed result for the same reason:
    /// a `rename_all` regression emitting `ttl_ms` is invisible to a lookup by
    /// camelCase name.
    fn assert_hints(response: &JSONRPCResponse, ctx: &str, ttl_ms: u64, cache_scope: &str) {
        let result = result_object(response);
        assert_eq!(
            result.get(TTL_MS_KEY),
            Some(&json!(ttl_ms)),
            "{ctx}: D-07 makes `{TTL_MS_KEY}` REQUIRED on a v2 `CacheableResult`; expected \
             {ttl_ms}, result was: {result:?}"
        );
        assert_eq!(
            result.get(CACHE_SCOPE_KEY),
            Some(&json!(cache_scope)),
            "{ctx}: D-07 makes `{CACHE_SCOPE_KEY}` REQUIRED on a v2 `CacheableResult`; expected \
             `{cache_scope}`, result was: {result:?}"
        );

        let wire = serde_json::to_string(response).expect("response serializes");
        assert!(
            wire.contains(r#""ttlMs""#) && wire.contains(r#""cacheScope""#),
            "{ctx}: both keys must reach the wire in camelCase, got: {wire}"
        );
    }

    /// Assert NEITHER hint key appears anywhere in the serialized response.
    ///
    /// Delegates to the SAME `leaked_hint_key` predicate the HTTP half uses
    /// (via [`assert_no_hints_in`]), so the two dispatchers cannot be held to
    /// two subtly different definitions of "carries no hint". Its anti-vacuity
    /// proof is `v2_caching_hints_the_no_hints_guard_is_load_bearing`, at the
    /// top level of this file.
    fn assert_no_hints(response: &JSONRPCResponse, ctx: &str) {
        let wire = serde_json::to_string(response).expect("response serializes");
        assert_no_hints_in(&wire, ctx);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn v2_caching_hints_server_core_resources_read_v2_carries_the_defaults() {
        let response = raw_via_core(
            hint_free_core(),
            read_resource_request(HINT_FREE_URI, Era::V2),
        )
        .await;

        assert_v2_witness(&response, "ServerCore / v2 resources/read, hint-free");
        assert_hints(
            &response,
            "ServerCore / v2 resources/read, hint-free",
            DEFAULT_TTL_MS,
            &default_cache_scope(),
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn v2_caching_hints_server_core_resources_read_v2_preserves_handler_set_values() {
        let response =
            raw_via_core(hinted_core(), read_resource_request(HINTED_URI, Era::V2)).await;

        assert_v2_witness(&response, "ServerCore / v2 resources/read, handler-set");
        assert_hints(
            &response,
            "ServerCore / v2 resources/read, handler-set",
            READ_TTL_MS,
            "private",
        );
    }

    /// The `ServerCore` twin of `v2_caching_hints_v1_strips_handler_set_values`.
    ///
    /// A NON-opted-in core plus an `Era::V1` request, against the handler that
    /// genuinely set both hints. `ServerCore` gates a v1 request behind the
    /// `initialize` handshake (`v1_initialize_gate_applies`,
    /// `src/server/core.rs:4089`) — unlike a v2 request, which needs none — so
    /// the handshake runs first. That asymmetry is itself further evidence the
    /// era reached the dispatcher.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn v2_caching_hints_server_core_resources_read_v1_strips_handler_set_values() {
        let core = v1_hinted_core();
        initialize_via_core(&core).await;
        let response = raw_via_core(core, read_resource_request(HINTED_URI, Era::V1)).await;

        assert_no_v2_witness(&response, "ServerCore / v1 resources/read, handler-set");
        assert_no_hints(&response, "ServerCore / v1 resources/read, handler-set");
    }

    /// The `ServerCore` twin of the fail-closed `request_is_cacheable` control.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn v2_caching_hints_server_core_tools_call_gains_neither_key() {
        let response = raw_via_core(
            hint_free_core(),
            call_tool_request(TOOL_ALPHA, json!({}), Era::V2),
        )
        .await;

        assert_v2_witness(&response, "ServerCore / v2 tools/call");
        assert_no_hints(
            &response,
            "ServerCore / v2 tools/call is not a CacheableResult (D-07)",
        );
    }

    /// **The structural bound, asserted at the code rather than only in a plan.**
    ///
    /// An OPTED-IN core is sent a typed `ClientRequest::ListResources` whose
    /// `params` carry a `_meta` object with the v2 protocol-version key — the
    /// exact signal that makes `resources/read` resolve v2 two tests above — and
    /// the era signal is DROPPED. `ListResourcesRequest` has no `_meta` field
    /// and does not set `deny_unknown_fields`, so serde discards the key
    /// silently, and `extract_request_meta_value`'s exhaustive match
    /// (`src/server/core.rs:3997-4026`) returns `None` for the variant anyway.
    ///
    /// **This is a documented semver decision, not a bug.** The rustdoc at
    /// `src/server/core.rs:3971-3991` records the reason: adding a `pub` field
    /// to a constructible `pub` struct is a MAJOR semver break
    /// (`cargo semver-checks` `constructible_struct_adds_field`), which the
    /// additive-scoped v2.5 milestone will not take. The four list methods
    /// therefore get their v2 caching-hint coverage over HTTP, through
    /// `Server::resolve_raw_meta_protocol_context`, which reads the RAW body and
    /// has FULL method coverage — see the six per-method tests at the top of
    /// this file.
    ///
    /// Without this test a future reader would find an unexplained in-process
    /// gap and try to close it by widening a public request struct, taking a
    /// MAJOR break to fix something that is already covered on the transport
    /// that actually matters for v2.
    ///
    /// The pre-handshake half is the extra evidence: a request that had resolved
    /// v2 would be served immediately, because
    /// [`v1_initialize_gate_applies`](pmcp::server::core) returns `false` for
    /// `Some(Era::V2)`. Getting `-32002` instead proves the era resolution came
    /// out non-v2 BEFORE anything about caching was decided.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn v2_caching_hints_list_methods_cannot_reach_v2_through_the_typed_dispatch_route() {
        let core = hint_free_core();

        // First evidence: without a v1 handshake the request is refused, which a
        // genuinely-v2-resolved request would not be.
        let refused = raw_via_core(core.clone(), list_resources_signalling_v2()).await;
        let refusal = serde_json::to_string(&refused).expect("response serializes");
        assert!(
            refusal.contains(&V1_TASK_PENDING.to_string()),
            "an opted-in core must still demand the v1 handshake for a `resources/list` \
             carrying the v2 `_meta` signal — proof the signal was dropped. Got: {refusal}"
        );

        // Second evidence: after the handshake it is served, as v1, hints stripped.
        initialize_via_core(&core).await;
        let response = raw_via_core(core, list_resources_signalling_v2()).await;

        assert_no_v2_witness(
            &response,
            "opted-in ServerCore, `resources/list` carrying the v2 `_meta` signal",
        );
        assert_no_hints(
            &response,
            "a `resources/list` that resolved v1 despite signalling v2",
        );
    }

    /// A typed `resources/list` request whose `params._meta` carries the v2
    /// protocol-version key.
    ///
    /// Built INLINE here rather than added to `tests/common/duplex.rs`
    /// deliberately: a shared era-aware builder for a list method would look
    /// usable and is not one, so the shared seam carries only
    /// `read_resource_request` and this lives in the one test whose whole
    /// subject is that the signal does NOT survive. The name here is
    /// deliberately NOT the obvious `<method>_request` shape, so the `grep`
    /// detector guarding that seam keeps working.
    fn list_resources_signalling_v2() -> Request {
        signalling_v2("resources/list", json!({}))
    }

    /// Build a typed request for `method` whose `params._meta` carries the
    /// reserved v2 protocol-version key.
    ///
    /// The `_meta` block is spelled as a JSON literal — the ONE place in this
    /// file that does so — because the whole subject here is what happens to a
    /// wire-shaped signal at the typed boundary, and
    /// [`v2_caching_hints_server_core_the_dropped_signal_is_a_real_one`] proves
    /// this exact literal DOES resolve v2 on a `_meta`-bearing variant.
    fn signalling_v2(method: &str, params: serde_json::Value) -> Request {
        let mut params = params;
        params.as_object_mut().expect("params is an object").insert(
            "_meta".to_string(),
            json!({ "io.modelcontextprotocol/protocolVersion": V2 }),
        );
        let mut envelope = serde_json::Map::new();
        envelope.insert("method".to_string(), json!(method));
        envelope.insert("params".to_string(), params);
        let request: ClientRequest = serde_json::from_value(serde_json::Value::Object(envelope))
            .unwrap_or_else(|e| panic!("`{method}` deserializes into ClientRequest ({e})"));
        Request::Client(Box::new(request))
    }

    /// **Anti-vacuity for the bound test: the dropped signal is a REAL one.**
    ///
    /// The bound test above asserts a `resources/list` carrying this `_meta`
    /// block resolves v1. That would be equally true of a mis-spelled,
    /// mis-nested or empty signal — in which case the test would prove nothing
    /// about `extract_request_meta_value` and everything about a typo.
    ///
    /// Here the IDENTICAL [`signalling_v2`] literal is applied to
    /// `resources/read` — a `_meta`-BEARING variant — against the SAME opted-in
    /// core, and it resolves v2. Signal, server and route held constant; only
    /// the `ClientRequest` variant differs. That isolates the variant as the
    /// cause, which is exactly the claim the bound test makes.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn v2_caching_hints_server_core_the_dropped_signal_is_a_real_one() {
        let response = raw_via_core(
            hint_free_core(),
            signalling_v2("resources/read", json!({ "uri": HINT_FREE_URI })),
        )
        .await;

        assert_v2_witness(
            &response,
            "the SAME `_meta` literal on `resources/read`, a `_meta`-bearing variant",
        );
        assert_hints(
            &response,
            "the SAME `_meta` literal on `resources/read`",
            DEFAULT_TTL_MS,
            &default_cache_scope(),
        );
    }
}
