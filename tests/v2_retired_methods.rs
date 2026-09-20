//! Phase 118.1 plan 05 (CONF-05, gap G-5): every RPC the MCP `2026-07-28` core
//! schema RETIRES answers `404` + `-32601` on v2, and still answers on v1.
//!
//! The retirement set is exactly five methods — `initialize`, `ping`,
//! `logging/setLevel`, `resources/subscribe`, `resources/unsubscribe`. All five
//! are ABSENT from the vendored `schema/vendored/core-2026-07-28/schema.ts`
//! method inventory, and the pinned official conformance suite probes exactly
//! these five, requiring HTTP `404` plus JSON-RPC `-32601` for each.
//!
//! # THIS FILE EXISTS BECAUSE TWO OF THE SUITE'S FOUR CHECKS PASS FOR THE WRONG REASON
//!
//! Read this before using the suite to validate anything about G-5.
//!
//! The suite sends `params: {_meta: <meta>}` and nothing else. `InitializeParams`
//! REQUIRES `protocolVersion`, `capabilities` and `clientInfo`;
//! `logging/setLevel` REQUIRES `level`. Both bodies therefore fail TYPED PARSING
//! before dispatch is ever reached, and the transport's raw-level recovery
//! (`map_unparsed_body_for_v2`) answers an unparseable v2 body with `-32601`/`404`
//! as a documented KNOWN LIMITATION. So the suite's `initialize` and
//! `logging/setLevel` checks are GREEN on a server that retires neither: they are
//! observing a params-parse failure that coincidentally carries the same code and
//! status the retirement would.
//!
//! Consequences, both of which are load-bearing:
//!
//! 1. **The suite's `initialize` / `logging/setLevel` checks MUST NOT be used to
//!    validate this fix.** They were already passing before it and will still pass
//!    if it is reverted. Cite `binary(v2_retired_methods)` instead.
//! 2. **A variant-keyed fix would look green in unit tests and change nothing on
//!    the wire.** A body that never deserialized has no `ClientRequest` variant to
//!    match, so a predicate over `&Request` can only ever see the requests that
//!    ALREADY parsed. The retirement is therefore keyed on the METHOD NAME STRING.
//!
//! Every request below carries WELL-FORMED, TYPED-PARSEABLE params for exactly
//! that reason: it is the shape a variant-keyed fix serves and a string-keyed one
//! refuses, so it is the only shape that can tell the two apart.
//!
//! Test reliability doctrine (carried from `tests/v2_stateless_http.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN via [`common::v2::teardown`]
//! (drop sockets, `abort()`, then `await` — a bare `abort()` produced
//! intermittent nextest `LEAK` noise).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, post, spawn_default_config, teardown, v2_body, v2_headers_for,
    META_PROTOCOL_VERSION, V1, V2,
};
use pmcp::types::protocol::error_codes::METHOD_NOT_FOUND;
use serde_json::{json, Value};

/// The resource URI the two `resources/*` bodies name.
///
/// A real URI, not a placeholder: on the v1 leg these requests are SERVED, and a
/// subscribe against a resource the fixture does not expose would be measuring
/// the wrong refusal.
const WATCHED_URI: &str = "mem://greeting";

/// The five retired methods, each with WELL-FORMED params and a DISTINCT
/// JSON-RPC id.
///
/// The ids differ per method so a mis-echoed id cannot be masked by a neighbour's
/// value, and one of them is a STRING so the `HttpServerErrorJsonrpcId` check is
/// exercised across both id shapes rather than only the numeric one.
///
/// `initialize` offers [`V1`] rather than [`V2`] in its params on purpose. The
/// ERA signal lives in `params._meta` (and in the `MCP-Protocol-Version` header),
/// never in `initialize`'s own `protocolVersion` field — so the same params are
/// usable verbatim on BOTH legs, which is what makes the v1 control a genuine
/// control rather than a differently-shaped request.
fn retired_methods() -> Vec<(&'static str, Value, Value)> {
    vec![
        (
            "initialize",
            json!(101),
            json!({
                "protocolVersion": V1,
                "capabilities": {},
                "clientInfo": { "name": "pmcp-retirement-probe", "version": "0.0.0" },
            }),
        ),
        ("ping", json!(102), json!({})),
        (
            "logging/setLevel",
            json!("retire-103"),
            json!({ "level": "debug" }),
        ),
        (
            "resources/subscribe",
            json!(104),
            json!({ "uri": WATCHED_URI }),
        ),
        (
            "resources/unsubscribe",
            json!(105),
            json!({ "uri": WATCHED_URI }),
        ),
    ]
}

// ===========================================================================
// The v2 leg: all five are GONE.
// ===========================================================================

/// Each of the five answers `404` + `-32601` with the ORIGINAL id echoed.
///
/// Every method is PROBED before anything is asserted, and the failures are
/// reported TOGETHER. A per-iteration `assert!` would abort on the first
/// offender, so a reader of the RED run would learn that `initialize` is served
/// and nothing at all about the other four — which is precisely the fact this
/// plan needed in order to know which of the five were genuinely broken and which
/// were already passing for the wrong reason.
#[tokio::test]
async fn v2_retires_every_schema_removed_method() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;

    let mut offenders: Vec<String> = Vec::new();
    for (method, id, params) in retired_methods() {
        let response = post(
            addr,
            &v2_headers_for(method, &params),
            &v2_body(method, id.clone(), params.clone()),
        )
        .await;

        if response.status != 404 {
            offenders.push(format!(
                "{method}: HTTP {} (expected 404) with well-formed params {params}; raw: {}",
                response.status, response.raw
            ));
        }
        if response.body["error"]["code"] != METHOD_NOT_FOUND {
            offenders.push(format!(
                "{method}: error.code {} (expected METHOD_NOT_FOUND {METHOD_NOT_FOUND}); raw: {}",
                response.body["error"]["code"], response.raw
            ));
        }
        if response.body["id"] != id {
            offenders.push(format!(
                "{method}: echoed id {} (expected {id}) — the suite's HttpServerErrorJsonrpcId \
                 check; raw: {}",
                response.body["id"], response.raw
            ));
        }
        if response.body.get("result").is_some() {
            offenders.push(format!(
                "{method}: a retired method also carried a result; raw: {}",
                response.raw
            ));
        }
    }

    teardown(handle, ()).await;

    assert!(
        offenders.is_empty(),
        "FAILURE MODE: {} of the 2026-07-28 retirement checks did not hold.\n\
         CONSEQUENCE: a method the core schema REMOVED is still reachable on the v2 wire, so the \
         SDK serves a surface the era it claims to speak does not define.\n\
         WHAT TO DO: widen the v2 ingress retirement predicate; do NOT relax these assertions.\n\
         {}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// ANTI-VACUITY: the retirement is SCOPED, not a blanket v2 `404`.
///
/// Without this, a change that refused every v2 request would pass the test
/// above with full marks.
#[tokio::test]
async fn v2_still_serves_a_method_the_schema_keeps() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let params = json!({});
    let response = post(
        addr,
        &v2_headers_for("tools/list", &params),
        &v2_body("tools/list", json!(201), params),
    )
    .await;
    teardown(handle, ()).await;

    assert_eq!(
        response.status, 200,
        "tools/list survives in the 2026-07-28 schema and MUST still be served on v2; raw: {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_some(),
        "expected a v2 tools/list result, got: {}",
        response.raw
    );
}

// ===========================================================================
// The v1 control: all five still answer.
// ===========================================================================

/// The SAME five methods, the SAME params, on the SAME server — served on v1.
///
/// v1 CONTROL — gated behind `v1-compat` (Phase 117). Retirement is an ERA
/// property, so the whole claim is "gone on v2, present on v1"; on a
/// `--no-default-features --features full-v2` build there is no v1 half to
/// contrast against and the severance itself is the proof. Gated per-TEST so the
/// v2 assertions above keep RUNNING on the severed build.
#[cfg(feature = "v1-compat")]
#[tokio::test]
async fn v1_still_serves_every_retired_method() {
    use common::v2::v1_body;

    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let mut cases = retired_methods().into_iter();

    // `initialize` is itself one of the five, and on v1 it is also the ONLY way
    // to mint the session the other four need — so its own control assertion and
    // the session it establishes are the same round trip.
    let (method, id, params) = cases.next().expect("initialize is the first case");
    assert_eq!(method, "initialize", "the v1 control depends on this order");
    let init = post(addr, &[], &v1_body(method, id.clone(), params)).await;
    assert_eq!(
        init.status, 200,
        "v1 initialize must still be SERVED; raw: {}",
        init.raw
    );
    assert!(
        init.body["error"].is_null(),
        "v1 initialize must not be retired; raw: {}",
        init.raw
    );
    assert_eq!(init.body["id"], id, "v1 initialize echoes its id");
    let session = init
        .mcp_session_id
        .clone()
        .expect("a v1 initialize on a stateful server MUST mint a session id");

    let session_headers = vec![
        ("mcp-session-id".to_string(), session),
        ("mcp-protocol-version".to_string(), V1.to_string()),
    ];

    for (method, id, params) in cases {
        let response = post(addr, &session_headers, &v1_body(method, id, params)).await;
        assert_eq!(
            response.status, 200,
            "{method} is UNTOUCHED on v1 and must answer 200; raw: {}",
            response.raw
        );
        assert_ne!(
            response.body["error"]["code"], METHOD_NOT_FOUND,
            "{method} must NOT be retired on v1 — retirement is era-scoped; raw: {}",
            response.raw
        );
        assert!(
            response.body.get("result").is_some(),
            "{method}: expected a v1 result, got: {}",
            response.raw
        );
    }

    teardown(handle, ()).await;
}

/// The harness itself sends the shape this file claims it sends.
///
/// Cheap, and it is what stops the whole file silently becoming vacuous: if
/// `v2_body` ever stopped attaching `_meta`, or the `initialize` case lost a
/// required field, every assertion above would still be measuring SOMETHING — it
/// just would not be the well-formed-params case that distinguishes a
/// string-keyed fix from a variant-keyed one.
#[test]
fn every_probe_body_carries_well_formed_params_and_the_v2_era_signal() {
    for (method, id, params) in retired_methods() {
        let body: Value = serde_json::from_str(&v2_body(method, id.clone(), params.clone()))
            .expect("the harness emits JSON");
        assert_eq!(body["method"], json!(method));
        assert_eq!(body["id"], id);
        assert_eq!(
            body["params"]["_meta"][META_PROTOCOL_VERSION],
            json!(V2),
            "{method}: the body must carry the v2 era signal, or the request is a v1 request \
             wearing a v2 label"
        );
        for (key, value) in params.as_object().expect("params are an object") {
            assert_eq!(
                &body["params"][key], value,
                "{method}: the harness must preserve the well-formed param {key}"
            );
        }
    }
    // The two shapes the suite gets wrong, spelled out so a reader can see the
    // difference between this file and the suite at a glance.
    let initialize = retired_methods()
        .into_iter()
        .find(|(method, _, _)| *method == "initialize")
        .expect("initialize is a case")
        .2;
    for required in ["protocolVersion", "capabilities", "clientInfo"] {
        assert!(
            initialize.get(required).is_some(),
            "initialize must carry {required} — an InitializeParams missing it fails TYPED \
             PARSING and would reach -32601 through map_unparsed_body_for_v2 instead of through \
             the retirement, which is the exact false green this file exists to avoid"
        );
    }
    let set_level = retired_methods()
        .into_iter()
        .find(|(method, _, _)| *method == "logging/setLevel")
        .expect("logging/setLevel is a case")
        .2;
    assert_eq!(
        set_level["level"],
        json!("debug"),
        "logging/setLevel must carry a parseable level for the same reason"
    );
}
