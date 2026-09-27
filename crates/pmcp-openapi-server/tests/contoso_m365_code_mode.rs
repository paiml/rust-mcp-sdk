//! Phase 90.2 Plan 03 (P902-CODEMODE + P902-FIXTURE) — the Contoso M365 demo's
//! HERO QUERY, proven deterministically and offline.
//!
//! This is the `execute_code`-style Code Mode workflow the demo advertises:
//! "which customers bought more than 100 in the last 3 months?". It is authored
//! as a Rust test (not a flat YAML replay step) on purpose — `execute_code`
//! requires an HMAC approval token that a flat scenario cannot thread (RQ-2), so
//! the test drives [`JsCodeExecutor`] DIRECTLY over the SAME token-threaded
//! [`HttpCodeExecutor`] the `ExecuteCodeHandler::PerRequestHttp` path constructs
//! per request (the seam mirrored verbatim from `tests/oauth_passthrough_e2e.rs`).
//!
//! ## "Via the tools" reconciliation (P902-CODEMODE)
//!
//! P902-CODEMODE frames the query as composed over the `get_customer` /
//! `get_customer_orders` tools. Those connector tools are thin literal mappers
//! over a SINGLE Microsoft Graph range-read op
//! (`/drives/.../workbook/worksheets/{sheet}/range(address='...')?$select=values`)
//! — they do `customer_id -> row -> A{row}:D{row}` address arithmetic in JS and
//! perform NO aggregation. The deterministic headline aggregation therefore reads
//! the **SAME Graph range-read API the tools expose** (identical endpoint shape,
//! identical `$select=values` projection, identical forwarded user bearer), but
//! over the all-rows block addresses (`all_customers_address` / `all_orders_address`
//! from the canonical workbook) so the cross-row join + filter + sum is computable
//! in a single pass. Reading the whole-table block rather than per-id ranges is
//! REQUIRED for the cross-row aggregation and stays within the single existing
//! range-read op — no new Graph op, no scope expansion, the locked tool interface
//! is unchanged.
//!
//! ## Determinism (no wall-clock)
//!
//! Every input is LOADED from the canonical
//! `tests/fixtures/contoso-m365-workbook.json` (Plan 01) and never re-typed: the
//! pinned reference date (`ref_date` -> `const REF`), the pinned lower bound
//! (`window_start` -> `const START`), the Customers/Orders rows (-> the wiremock
//! response bodies), and the asserted result (`expected_headline_set`). The
//! HEADLINE script uses NO `Date` builtin — the trailing-3-month lower bound is
//! derived with explicit pure-integer month subtraction (year-rollover-safe) and
//! the script asserts its own `computedStart === START`, so the rollover math is
//! provably correct against the pinned literal and the asserted set cannot rot
//! over calendar time.
//!
//! Run with: `cargo test -p pmcp-openapi-server --features openapi-code-mode \
//! --test contoso_m365_code_mode -- --test-threads=1`. Test fns are
//! `contoso_m365_code_mode_`-prefixed so the positional verify filter resolves
//! (Plan 01 verify-filter lesson).

#![cfg(feature = "openapi-code-mode")]

use pmcp_server_toolkit::code_mode::{
    request_executor_from_extra, CodeExecutor, ExecutionConfig, HttpCodeExecutor, JsCodeExecutor,
};
use pmcp_server_toolkit::http::auth::{create_passthrough_auth_provider, AuthConfig};

use pmcp::server::auth::AuthContext;
use serde_json::{json, Value};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// Executor seam — copied VERBATIM from tests/oauth_passthrough_e2e.rs (the
// `ExecuteCodeHandler::PerRequestHttp` seam; keep byte-identical so this proves
// the same token-threaded executor the toolkit handlers use).
// ============================================================================

/// Build a passthrough `HttpCodeExecutor` (no construction-time token — the
/// per-request token arrives via `apply`'s `inbound_token`) over `base_url`.
fn passthrough_exec(base_url: String, required: bool) -> HttpCodeExecutor {
    let auth = create_passthrough_auth_provider(
        &AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required,
        },
        None,
    )
    .expect("passthrough auth provider");
    HttpCodeExecutor::new(reqwest::Client::new(), base_url, auth)
}

/// A `RequestHandlerExtra` carrying the captured inbound `Authorization` header
/// (mirrors assemble.rs's `TokenCaptureAuthProvider` capture).
fn extra_with_token(token: Option<&str>) -> pmcp::RequestHandlerExtra {
    let ctx = AuthContext {
        subject: "proxy-authenticated".to_string(),
        scopes: vec![],
        claims: std::collections::HashMap::new(),
        token: token.map(str::to_string),
        client_id: None,
        expires_at: None,
        authenticated: token.is_some(),
    };
    pmcp::RequestHandlerExtra::default().with_auth_context(Some(ctx))
}

/// The Code-Mode `execute_code` per-request executor: exactly what
/// `ExecuteCodeHandler::PerRequestHttp` builds — a `JsCodeExecutor` over the
/// token-threaded `HttpCodeExecutor`.
fn code_mode_executor(
    base: &HttpCodeExecutor,
    extra: &pmcp::RequestHandlerExtra,
) -> JsCodeExecutor<HttpCodeExecutor> {
    let http = request_executor_from_extra(base, extra);
    JsCodeExecutor::new(http, ExecutionConfig::default())
}

// ============================================================================
// Canonical dataset — LOADED from tests/fixtures/contoso-m365-workbook.json
// (Plan 01, the single source of truth). REF, window_start, the row values, the
// block addresses, AND the expected set all come from here — none re-typed.
// ============================================================================

/// THE canonical workbook json, embedded at build time so the test loads (never
/// re-types) the rows/dates/addresses/expected-set.
const WORKBOOK_JSON: &str = include_str!("fixtures/contoso-m365-workbook.json");

/// Build the Microsoft-Graph `workbookRange` response body the Code Mode read
/// expects: `{ "values": [[customer_id, name, segment, region], ...] }` — built
/// from the LOADED canonical Customers rows (header excluded; the script joins on
/// the data cells only).
fn customers_values_body(wb: &Value) -> Value {
    let rows: Vec<Value> = wb["customers"]
        .as_array()
        .expect("customers array")
        .iter()
        .map(|c| {
            json!([
                c["customer_id"].as_str().unwrap(),
                c["name"].as_str().unwrap(),
                c["segment"].as_str().unwrap(),
                c["region"].as_str().unwrap(),
            ])
        })
        .collect();
    json!({ "values": rows })
}

/// Build the Graph `workbookRange` response body for Orders from the LOADED
/// canonical Orders rows: `{ "values": [[order_id, customer_id, order_date, amount], ...] }`.
fn orders_values_body(wb: &Value) -> Value {
    let rows: Vec<Value> = wb["orders"]
        .as_array()
        .expect("orders array")
        .iter()
        .map(|o| {
            json!([
                o["order_id"].as_str().unwrap(),
                o["customer_id"].as_str().unwrap(),
                o["order_date"].as_str().unwrap(),
                o["amount"].as_f64().unwrap(),
            ])
        })
        .collect();
    json!({ "values": rows })
}

/// The shared month-rollover-safe `computedStart` derivation, authored once and
/// reused by both the headline script and the rollover-proof script. NO `Date`
/// builtin: parse REF's `YYYY-MM-DD` via `parseInt` on fixed substrings, subtract
/// 3 months with explicit year rollover (the `< 1` case wraps `m2 += 12` /
/// `y2 -= 1` expressed without mutation via ternaries), zero-pad back to
/// `YYYY-MM-DD` by string concat. `REF`/`START` are PINNED literals injected from
/// the canonical workbook json — never wall-clock `now`.
///
/// The SWC JS subset (Phase 90 Plan 05 + RESEARCH Pitfall 1) forbids reassignment,
/// `++`/`--`, `while`, and `throw`, so every binding is a `const` and the rollover
/// branch is a ternary — fully declarative.
fn rollover_prelude(ref_date: &str, window_start: &str) -> String {
    format!(
        r#"const REF = "{ref_date}";
const START = "{window_start}";

const y = parseInt(REF.slice(0, 4), 10);
const m = parseInt(REF.slice(5, 7), 10);
const d = parseInt(REF.slice(8, 10), 10);
// trailing-3-month lower bound with explicit year rollover (m - 3 < 1 wraps the
// month by +12 and the year by -1). Numbers are integer-formatted via toFixed(0)
// to avoid float string artifacts (e.g. "2.0"); the day component is unchanged by
// a whole-month shift so it is taken verbatim from REF's YYYY-MM-DD substring.
const m2 = m - 3 < 1 ? m - 3 + 12 : m - 3;
const y2 = m - 3 < 1 ? y - 1 : y;
const mm = m2 < 10 ? "0" + m2.toFixed(0) : m2.toFixed(0);
const dd = REF.slice(8, 10);
const computedStart = y2.toFixed(0) + "-" + mm + "-" + dd;
// Comparable integer encodings YYYYMMDD for the inclusive window bounds. The
// engine's relational operators (>=, <=) coerce operands to numbers, so date
// comparison is done on these integer encodings, not on the ISO strings.
const refNum = y * 10000 + m * 100 + d;
const startNum = y2 * 10000 + m2 * 100 + d;
"#
    )
}

/// The HEADLINE Code Mode script — the demo's hero query, engine-accurate for the
/// SWC JS subset: NO `Date` builtin, every `api.get` path bound to a `const`
/// before use, fully functional aggregation (`map`/`filter`/`reduce`/`sort`, no
/// mutation). Reads the SAME Graph range-read API the `get_customer` /
/// `get_customer_orders` tools expose (identical endpoint + `$select=values`),
/// over the all-rows block addresses for a single-pass cross-row aggregation.
/// REF/START/addresses are injected from the LOADED canonical json (never re-typed).
fn headline_script(
    ref_date: &str,
    window_start: &str,
    customers_addr: &str,
    orders_addr: &str,
) -> String {
    let prelude = rollover_prelude(ref_date, window_start);
    format!(
        r#"{prelude}
// Read the WHOLE Customers + Orders blocks via the SAME Graph range-read API the
// tools expose. The path is passed as a string literal directly to api.get (the
// engine requires a string/template literal there), then .values is bound to a const.
const customersResp = await api.get("/drives/CONTOSO_DRIVE/items/CUSTOMERS_ITEM/workbook/worksheets/Customers/range(address='{customers_addr}')?$select=values");
const customerRows = customersResp.values;

const ordersResp = await api.get("/drives/CONTOSO_DRIVE/items/ORDERS_ITEM/workbook/worksheets/Orders/range(address='{orders_addr}')?$select=values");
const orderRows = ordersResp.values;

// Known customer ids (join key) — column 0 of each Customers data row.
const knownIds = customerRows.map(crow => crow[0]);

// Join Orders -> Customers on customer_id; filter order_date into the inclusive
// trailing-3-month window [startNum, refNum] (each order date encoded YYYYMMDD;
// inclusive of startNum so the boundary order dated exactly window_start classifies
// IN); sum amount per customer; keep totals strictly > 100. Fully declarative:
// a per-id nested filter+reduce, no mutation.
const matched = knownIds.filter(cid => orderRows.filter(orow => orow[1] === cid && parseInt(orow[2].slice(0, 4), 10) * 10000 + parseInt(orow[2].slice(5, 7), 10) * 100 + parseInt(orow[2].slice(8, 10), 10) >= startNum && parseInt(orow[2].slice(0, 4), 10) * 10000 + parseInt(orow[2].slice(5, 7), 10) * 100 + parseInt(orow[2].slice(8, 10), 10) <= refNum).reduce((acc, orow) => acc + orow[3], 0) > 100);
const result = matched.sort();
return result;
"#
    )
}

/// A tiny companion script that returns the derived `computedStart` so the Rust
/// test can assert the month-rollover math equals the pinned `window_start`
/// EXPLICITLY (in addition to the set-equality proof in the headline test).
fn rollover_proof_script(ref_date: &str, window_start: &str) -> String {
    let prelude = rollover_prelude(ref_date, window_start);
    format!("{prelude}\nconst result = computedStart;\nreturn result;\n")
}

// ============================================================================
// The hero query, proven deterministic + offline + token-threaded.
// ============================================================================

#[tokio::test]
async fn contoso_m365_code_mode_headline_query_returns_deterministic_set() {
    // ---- LOAD the canonical dataset (no re-typed rows/dates/expected-set) ----
    let wb: Value = serde_json::from_str(WORKBOOK_JSON).expect("parse canonical workbook json");
    let ref_date = wb["ref_date"].as_str().expect("ref_date");
    let window_start = wb["window_start"].as_str().expect("window_start");
    let customers_addr = wb["all_customers_address"]
        .as_str()
        .expect("all_customers_address");
    let orders_addr = wb["all_orders_address"]
        .as_str()
        .expect("all_orders_address");
    let expected_set: Vec<String> = wb["expected_headline_set"]
        .as_array()
        .expect("expected_headline_set array")
        .iter()
        .map(|v| v.as_str().expect("id string").to_string())
        .collect();

    // ---- Graph backend: TWO range-read mocks (Customers + Orders), EACH ----
    // requiring the forwarded user bearer AND invoked exactly once (.expect(1) is
    // the request-count guard: a hardcoded result or a missing backend read fails).
    let server = MockServer::start().await;

    let customers_path =
        format!("/drives/CONTOSO_DRIVE/items/CUSTOMERS_ITEM/workbook/worksheets/Customers/range(address='{customers_addr}')");
    Mock::given(method("GET"))
        .and(path(customers_path))
        .and(header("authorization", "Bearer contoso-user-tok"))
        // The `$select` projection the script writes as a literal `?$select=values`
        // MUST arrive on the wire. `path()` ignores the query string entirely, so
        // without this matcher the projection could be silently dropped — or
        // mangled by the Phase-128 composed-path check — and the test would still
        // pass. It also pins the `?` narrowing from the consumer side: a regression
        // that refused or stripped an author-written query separator fails HERE.
        .and(query_param("$select", "values"))
        .respond_with(ResponseTemplate::new(200).set_body_json(customers_values_body(&wb)))
        .expect(1)
        .mount(&server)
        .await;

    let orders_path =
        format!("/drives/CONTOSO_DRIVE/items/ORDERS_ITEM/workbook/worksheets/Orders/range(address='{orders_addr}')");
    Mock::given(method("GET"))
        .and(path(orders_path))
        .and(header("authorization", "Bearer contoso-user-tok"))
        .and(query_param("$select", "values"))
        .respond_with(ResponseTemplate::new(200).set_body_json(orders_values_body(&wb)))
        .expect(1)
        .mount(&server)
        .await;

    // ---- Drive the token-threaded JsCodeExecutor (the execute_code seam) ----
    let base = passthrough_exec(server.uri(), true);
    let extra = extra_with_token(Some("Bearer contoso-user-tok"));
    let executor = code_mode_executor(&base, &extra);

    let script = headline_script(ref_date, window_start, customers_addr, orders_addr);
    let result = executor
        .execute(&script, None)
        .await
        .expect("headline query must run offline over the token-threaded executor and succeed");

    // ---- Assert the EXACT set LOADED from the canonical json (not a literal) ----
    assert_eq!(
        result,
        json!(expected_set),
        "headline set must equal the canonical expected_headline_set ({expected_set:?})"
    );

    // Boundary classification proof (per Plan 01): the boundary customer whose
    // single order is dated EXACTLY window_start is IN the set, and the
    // just-outside customer (order one day before window_start) is OUT. With the
    // canonical fixture this is C003 (IN) / C004 (OUT). The set-equality assertion
    // above already enforces this; restate it explicitly so a fixture edit that
    // breaks the boundary fails LOUDLY here too.
    assert!(
        expected_set.iter().any(|id| id == "C003"),
        "boundary customer C003 (order dated exactly window_start) must be IN"
    );
    assert!(
        !expected_set.iter().any(|id| id == "C004"),
        "just-outside customer C004 (order one day before window_start) must be OUT"
    );

    // The two .expect(1) mocks verify on drop that BOTH backend reads occurred
    // exactly once with the forwarded bearer — the computed set could not have come
    // from a hardcoded constant.
}

#[tokio::test]
async fn contoso_m365_code_mode_rollover_math_equals_pinned_window_start() {
    // Explicitly prove the month-rollover-safe trailing-3-month derivation: the
    // script's `computedStart` (parse REF -> subtract 3 months with year rollover
    // -> zero-pad) MUST equal the PINNED `window_start` from the canonical json.
    // This needs no backend (pure date math), so no mock/token is required — but we
    // still drive the SAME JsCodeExecutor seam. If the rollover math drifts, this
    // fails LOUDLY and the headline set in the test above also breaks.
    let wb: Value = serde_json::from_str(WORKBOOK_JSON).expect("parse canonical workbook json");
    let ref_date = wb["ref_date"].as_str().expect("ref_date");
    let window_start = wb["window_start"].as_str().expect("window_start");

    // Pure date math — the script makes no api.get call, so no MockServer is
    // needed; this dummy base URL is never dialed.
    let base = passthrough_exec("http://127.0.0.1:0".to_string(), false);
    let extra = extra_with_token(None);
    let executor = code_mode_executor(&base, &extra);

    let script = rollover_proof_script(ref_date, window_start);
    let result = executor
        .execute(&script, None)
        .await
        .expect("rollover-proof script must run offline and return computedStart");

    assert_eq!(
        result,
        json!(window_start),
        "month-rollover derivation (computedStart) must equal the pinned window_start {window_start}"
    );
}
