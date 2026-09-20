//! `tasks/update` DELIVERY semantics (Phase 114, plan 14 — TASK-02).
//!
//! The sibling of `tests/v2_tasks_update_routing.rs`, and they divide the method
//! cleanly: that file proves the five ordered GATES answer in the right order,
//! this file proves what happens AFTER all five have passed — bounds, the
//! kind-directed decode, the transition, and the acknowledgement.
//!
//! # Every test here drives RAW FRAMES, and that is not a stylistic choice
//!
//! A `pmcp::Client` builds its `inputResponses` from the very `inputRequests` the
//! server sent it, so it is INCAPABLE of producing a mismatched answer. A suite
//! written against the client could never have caught D-113-O — where an
//! `ElicitResult`-shaped value was silently reclassified as `Sampling` because the
//! two structurally overlap, the handler's `Elicitation` arm never matched, the
//! operation re-elicited **sixteen times**, and it died on a misleading error.
//! Reaching that bug class requires sending bytes a conformant client would never
//! send, which means a raw frame. (113-27 established this discipline; the
//! measured proof it is load-bearing is negative control NC-1 in
//! `114-14-SUMMARY.md`.)
//!
//! # The properties
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `tasks_update_completes_the_outstanding_set` | a complete delivery moves `input_required` -> `working` |
//! | 2 | `tasks_update_partial_set_stays_input_required` | a partial delivery PERSISTS and the task stays paused |
//! | 3 | `tasks_update_ignores_a_key_that_was_never_issued` | an unrecorded key is ignored, not an error |
//! | 4 | `tasks_update_ignores_an_already_answered_key` | an answered key is ignored and NOT re-accepted |
//! | 5 | `tasks_update_kind_directed_accepts_an_elicitation_answer_under_an_elicitation_key` | the D-113-O positive case |
//! | 6 | `tasks_update_kind_directed_refuses_a_sampling_shape_under_an_elicitation_key` | the D-113-O negative case |
//! | 7-10 | `tasks_update_bounds_fire_before_the_decode_*` | one per bound: the BOUND error wins over an also-undecodable value |
//! | 11 | `tasks_update_never_runs_the_untagged_decoder_on_ingress` | the raw-map boundary, proven from two sides |
//! | 12 | `tasks_update_cas_first_writer_wins` | two concurrent deliveries: one wins, one sees the conflict |
//! | 13-15 | `tasks_update_on_a_*_task_is_refused` | `completed` / `failed` / `cancelled` cannot be fed |
//! | 16 | `tasks_update_ack_is_empty` | `UpdateTaskResult = Result`: no task fields |
//! | 17 | `tasks_update_for_another_owner_is_not_found` | the oracle-free `-32602`, byte-identical to an absent id |
//!
//! There are FOUR bounds tests, not five. `MAX_REQUEST_STATE_LEN` bounds the MRTR
//! continuation token and `tasks/update` carries no token, so a fifth test would
//! be asserting a bound this route correctly does not enforce.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    post, spawn_tasks_server_with_store, teardown, v2_body_with_client_extensions, v2_headers,
    AuthPosture, Resp, TASKS_TOOL_NAME,
};
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::testing::{
    MAX_INPUT_RESPONSES, MAX_INPUT_RESPONSES_TOTAL_BYTES, MAX_INPUT_RESPONSE_BYTES,
    MAX_INPUT_RESPONSE_DEPTH,
};
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::mrtr::InputRequests;
use pmcp::types::tasks::TaskStatus;
use serde_json::{json, Map, Value};
use std::net::SocketAddr;
use std::sync::Arc;

/// The wire method under test. The crate's own constant is `pub(crate)`.
const TASKS_UPDATE: &str = "tasks/update";

/// The principal every request in this suite binds to unless stated otherwise.
const OWNER: &str = "alice";

/// A DIFFERENT principal, for the cross-owner test.
const OTHER_OWNER: &str = "mallory";

// ===========================================================================
// Fixtures.
// ===========================================================================

/// A paused task fixture: the address, the store handle and the task's id.
struct Paused {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    store: Arc<InMemoryTaskStore>,
    task_id: String,
}

/// Build an [`InputRequests`] map from the wire JSON a handler writes.
///
/// Deliberately routed through the PRODUCTION `Deserialize` rather than
/// hand-constructed: `InputRequest` is adjacently tagged on `method`/`params`, and
/// a fixture built by struct literal would not prove the spelling a real handler
/// emits is the spelling the record holds.
fn input_requests(value: Value) -> InputRequests {
    serde_json::from_value(value).expect("the inputRequests fixture is well formed")
}

/// One `roots/list` request under the key `roots`.
fn roots_only() -> InputRequests {
    input_requests(json!({ "roots": { "method": "roots/list" } }))
}

/// One `elicitation/create` request under the key `form`.
fn elicitation_only() -> InputRequests {
    input_requests(json!({
        "form": {
            "method": "elicitation/create",
            "params": { "message": "which city?", "requestedSchema": { "type": "object" } },
        }
    }))
}

/// TWO outstanding requests, so a PARTIAL delivery is expressible at all.
fn roots_and_elicitation() -> InputRequests {
    let mut requests = roots_only();
    requests.extend(elicitation_only());
    requests
}

/// A valid `ListRootsResult` — the only shape that decodes under a `roots/list`
/// key.
fn roots_answer() -> Value {
    json!({ "roots": [] })
}

/// A valid `ElicitResult`: `action` is its one required field.
fn elicitation_answer() -> Value {
    json!({ "action": "accept", "content": { "city": "Berlin" } })
}

/// A value that is a `CreateMessageResult` and CANNOT be anything else.
///
/// # This fixture was MEASURED, and a future reader must not "fix" it
///
/// `ElicitResult` carries no `deny_unknown_fields` and its `content` is
/// `Option<HashMap<String, Value>>`, so an object carrying `action`, `content` AND
/// `model` IS a valid `ElicitResult` — using that as the "wrong shape" would make
/// the negative test assert nothing, because the value would legitimately decode
/// under an elicitation key. Dropping `action` is what makes it exclusively a
/// `CreateMessageResult`: that type requires `content` (a tagged `Content`) plus
/// `model`, `ListRootsResult` requires `roots`, and `ElicitResult` requires
/// `action`.
///
/// It is therefore ALSO the exact input the untagged decoder classifies as
/// `Sampling` — the D-113-O reclassification — which is what test 11 uses it for.
fn sampling_shaped_answer() -> Value {
    json!({ "content": { "type": "text", "text": "sampled" }, "model": "test-model" })
}

/// A value that decodes as NONE of the three result shapes.
///
/// Used by the bounds tests: each of them sends a payload that violates a bound
/// AND carries a value the decoder would refuse, so "the bounds error wins" is a
/// statement about ORDER rather than about which of two refusals happened to be
/// the only one available.
fn undecodable_answer() -> Value {
    json!({ "nothing": "matches this" })
}

fn auth(subject: &str) -> Vec<(String, String)> {
    vec![("authorization".to_string(), format!("Bearer {subject}"))]
}

/// A v2 request that DECLARES the tasks extension, from `subject`.
async fn declaring(
    addr: SocketAddr,
    subject: &str,
    method: &str,
    name: &str,
    id: i64,
    params: Value,
) -> Resp {
    let mut headers = v2_headers(method, name);
    headers.extend(auth(subject));
    let body = v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]);
    post(addr, &headers, &body).await
}

/// Send one `tasks/update` carrying `input_responses` verbatim.
async fn update(addr: SocketAddr, subject: &str, task_id: &str, id: i64, responses: Value) -> Resp {
    declaring(
        addr,
        subject,
        TASKS_UPDATE,
        task_id,
        id,
        json!({ "taskId": task_id, "inputResponses": responses }),
    )
    .await
}

/// Spawn a tasks server, mint a REAL task over the wire, and pause it on
/// `requests`.
///
/// The task is created by a genuine v2 `tools/call`, so the id under test is the
/// store-minted one a client would actually hold. The pause is applied through
/// [`TaskStore::record_input_requests`] because the harness's client-reachable
/// pausing tool records exactly one `roots/list` entry, and half this suite needs
/// a specific KIND or a TWO-entry outstanding set — neither of which a fixed tool
/// fixture can express.
async fn paused_on(requests: InputRequests) -> Paused {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Optional).await;
    let created = declaring(
        addr,
        OWNER,
        "tools/call",
        TASKS_TOOL_NAME,
        1,
        json!({ "name": TASKS_TOOL_NAME, "arguments": {} }),
    )
    .await;
    let task_id = created.body["result"]["taskId"]
        .as_str()
        .unwrap_or_else(|| panic!("a declaring v2 tools/call mints a task: {}", created.raw))
        .to_string();
    store
        .record_input_requests(&task_id, OWNER, requests)
        .await
        .expect("the freshly created task accepts a first round of input requests");
    Paused {
        addr,
        handle,
        store,
        task_id,
    }
}

// ===========================================================================
// Reading responses.
// ===========================================================================

fn error_of(response: &Resp) -> &Value {
    response
        .body
        .get("error")
        .unwrap_or_else(|| panic!("expected a JSON-RPC error, got {}", response.raw))
}

fn code_of(response: &Resp) -> i64 {
    error_of(response)["code"]
        .as_i64()
        .unwrap_or_else(|| panic!("an error carries a numeric code; got {}", response.raw))
}

fn message_of(response: &Resp) -> String {
    error_of(response)["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn assert_is_ack(response: &Resp) {
    assert!(
        response.body.get("error").is_none(),
        "expected an UpdateTaskResult acknowledgement, got {}",
        response.raw
    );
    assert!(
        response.body["result"].is_object(),
        "an acknowledgement is an object result: {}",
        response.raw
    );
}

mod codes {
    use pmcp::types::protocol::error_codes as ec;
    pub const INVALID_PARAMS: i64 = ec::INVALID_PARAMS as i64;
    pub const INTERNAL_ERROR: i64 = ec::INTERNAL_ERROR as i64;
}

/// The substring a KIND-DIRECTED refusal carries, and nothing else does.
///
/// The bounds tests assert its ABSENCE, which is how "the bound fired FIRST" is
/// distinguished from "the bound fired at all".
const KIND_REFUSAL_MARKER: &str = "is not a valid response to the";

/// Fetch a task's current status over the wire.
async fn status_of(addr: SocketAddr, subject: &str, task_id: &str, id: i64) -> String {
    let polled = declaring(
        addr,
        subject,
        "tasks/get",
        task_id,
        id,
        json!({ "taskId": task_id }),
    )
    .await;
    polled.body["result"]["status"]
        .as_str()
        .unwrap_or_else(|| panic!("a v2 tasks/get inlines a status: {}", polled.raw))
        .to_string()
}

// ===========================================================================
// 1-2. Complete vs partial delivery.
// ===========================================================================

/// A delivery that COMPLETES the outstanding set resumes the task.
#[tokio::test]
async fn tasks_update_completes_the_outstanding_set() {
    let fixture = paused_on(roots_only()).await;

    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 2).await,
        "input_required",
        "precondition: the fixture is genuinely paused"
    );

    let acked = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        3,
        json!({ "roots": roots_answer() }),
    )
    .await;
    assert_is_ack(&acked);

    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 4).await,
        "working",
        "a complete delivery transitions input_required -> working"
    );

    teardown(fixture.handle, ()).await;
}

/// A PARTIAL delivery persists its response and leaves the task paused.
///
/// Both halves are asserted. Asserting only the status would pass against an
/// implementation that answered the ack and dropped the payload on the floor —
/// the task would still be `input_required`, and the response would be gone.
///
/// The persistence half is read through
/// [`TaskStore::task_input_snapshot`] — the SAME owner-scoped accessor the route
/// itself uses — because the wire has no view of it: a v2 `tasks/get` inlines
/// `inputRequests` and never `inputResponses`.
#[tokio::test]
async fn tasks_update_partial_set_stays_input_required() {
    let fixture = paused_on(roots_and_elicitation()).await;

    let acked = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ "roots": roots_answer() }),
    )
    .await;
    assert_is_ack(&acked);

    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 3).await,
        "input_required",
        "one of two outstanding keys is not a complete set"
    );

    let snapshot = fixture
        .store
        .task_input_snapshot(&fixture.task_id, OWNER)
        .await
        .expect("the paused task has an input snapshot");
    assert!(
        snapshot.input_responses.contains_key("roots"),
        "the partial response must be PERSISTED, not merely acknowledged"
    );
    assert!(
        !snapshot.input_responses.contains_key("form"),
        "nothing was delivered under `form`"
    );

    teardown(fixture.handle, ()).await;
}

// ===========================================================================
// 3-4. Ignore semantics.
// ===========================================================================

/// A distinctive CLIENT-CHOSEN `inputResponses` key.
///
/// If it appears anywhere in the response bytes, the server echoed
/// attacker-controlled content into its own wire (T-114-69).
const UNSOLICITED: &str = "zzz-client-invented-key-9f3a";

/// A key the record never held is IGNORED, not refused.
///
/// The extension says a server SHOULD ignore a key that is not currently
/// outstanding. It is also the log-poisoning boundary: that key is CLIENT-chosen
/// by definition, so the response must not render it back (T-114-69).
#[tokio::test]
async fn tasks_update_ignores_a_key_that_was_never_issued() {
    let fixture = paused_on(roots_only()).await;

    let acked = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ UNSOLICITED: elicitation_answer() }),
    )
    .await;

    assert_is_ack(&acked);
    assert!(
        !acked.raw.contains(UNSOLICITED),
        "a client-chosen key must never be rendered back: {}",
        acked.raw
    );
    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 3).await,
        "input_required",
        "an ignored key answers nothing, so the task is unchanged"
    );

    teardown(fixture.handle, ()).await;
}

/// A key that was ALREADY answered is IGNORED and is not re-accepted.
///
/// Delivering under it a second time cannot overwrite the first answer — the
/// spec's "each key MUST NOT be reused after its response was delivered", which
/// is also the replay mitigation (T-114-71).
#[tokio::test]
async fn tasks_update_ignores_an_already_answered_key() {
    let fixture = paused_on(roots_and_elicitation()).await;

    assert_is_ack(
        &update(
            fixture.addr,
            OWNER,
            &fixture.task_id,
            2,
            json!({ "roots": roots_answer() }),
        )
        .await,
    );

    // A second, DIFFERENT answer under the same key.
    let replayed = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        3,
        json!({ "roots": { "roots": [{ "uri": "file:///injected", "name": "injected" }] } }),
    )
    .await;
    assert_is_ack(&replayed);

    let snapshot = fixture
        .store
        .task_input_snapshot(&fixture.task_id, OWNER)
        .await
        .expect("the paused task has an input snapshot");
    let persisted = serde_json::to_value(&snapshot.input_responses).expect("responses serialize");
    assert_eq!(
        persisted["roots"]["roots"],
        json!([]),
        "the FIRST answer stands; a replay may not overwrite it"
    );
    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 4).await,
        "input_required",
        "a replay accepts nothing, so it cannot complete the set either"
    );

    teardown(fixture.handle, ()).await;
}

// ===========================================================================
// 5-6. The D-113-O pair: kind direction, positive and negative.
// ===========================================================================

/// An `ElicitResult`-shaped value under an ELICITATION key types as elicitation
/// and completes the round.
///
/// The positive half of the pair. Without it, an implementation that refused
/// EVERYTHING would pass test 6 and be recorded as correct.
#[tokio::test]
async fn tasks_update_kind_directed_accepts_an_elicitation_answer_under_an_elicitation_key() {
    let fixture = paused_on(elicitation_only()).await;

    let acked = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ "form": elicitation_answer() }),
    )
    .await;
    assert_is_ack(&acked);

    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 3).await,
        "working",
        "the elicitation answer completed the outstanding set"
    );

    teardown(fixture.handle, ()).await;
}

/// A `CreateMessageResult`-shaped value under an ELICITATION key is REFUSED.
///
/// **This is D-113-O.** Under the untagged decoder this value is classified as
/// `Sampling` (most-specific-first: `ListRootsResult` needs `roots`,
/// `CreateMessageResult` needs `content` + `model` and matches), the server's
/// `Elicitation` arm never fires, and the round re-elicits forever with no error
/// raised anywhere. Under the kind-directed decoder the server refuses, because
/// the kind came from ITS OWN record and the value does not satisfy it.
///
/// See [`sampling_shaped_answer`] for why the fixture omits `action`, and why an
/// "obvious" three-key fixture would silently make this test vacuous.
#[tokio::test]
async fn tasks_update_kind_directed_refuses_a_sampling_shape_under_an_elicitation_key() {
    let fixture = paused_on(elicitation_only()).await;

    let refused = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ "form": sampling_shaped_answer() }),
    )
    .await;

    assert_eq!(
        code_of(&refused),
        codes::INVALID_PARAMS,
        "a value that cannot be the requested kind is refused: {}",
        refused.raw
    );
    let message = message_of(&refused);
    assert!(
        message.contains("form"),
        "the refusal names the SERVER-ASSIGNED key so it is actionable: {message}"
    );
    assert!(
        !refused.raw.contains("test-model"),
        "the refusal must NEVER render the value back: {}",
        refused.raw
    );
    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 3).await,
        "input_required",
        "a refused delivery changes nothing"
    );

    teardown(fixture.handle, ()).await;
}

// ===========================================================================
// 7-10. The four bounds, each proven to fire BEFORE the decode.
// ===========================================================================

/// A JSON value nested exactly `depth` levels deep.
fn nested(depth: usize) -> Value {
    let mut value = json!(1);
    for _ in 1..depth {
        value = json!({ "n": value });
    }
    value
}

/// An `inputResponses` map that is `entries` big, every value UNDECODABLE.
fn undecodable_filler(entries: usize) -> Map<String, Value> {
    (0..entries)
        .map(|i| (format!("pad-{i:04}"), undecodable_answer()))
        .collect()
}

/// Assert `response` is the BOUND refusal `bound_marker` describes, and NOT the
/// kind-directed one.
fn assert_bound_won(response: &Resp, bound_marker: &str) {
    assert_eq!(
        code_of(response),
        codes::INVALID_PARAMS,
        "a bounds violation is a structured -32602: {}",
        response.raw
    );
    let message = message_of(response);
    assert!(
        message.contains(bound_marker),
        "the refusal must name the bound `{bound_marker}` that was exceeded: {message}"
    );
    assert!(
        !message.contains(KIND_REFUSAL_MARKER),
        "the BOUND must fire before the decode; this is the decode's refusal: {message}"
    );
}

/// The entry-COUNT bound fires before the decode.
#[tokio::test]
async fn tasks_update_bounds_fire_before_the_decode_entry_count() {
    let fixture = paused_on(roots_only()).await;

    let mut responses = undecodable_filler(MAX_INPUT_RESPONSES);
    // A RECORDED key whose value is undecodable: without the bound, the decode
    // would refuse this entry, so a passing test proves the ORDER.
    responses.insert("roots".to_string(), undecodable_answer());
    assert!(responses.len() > MAX_INPUT_RESPONSES);

    let refused = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        Value::Object(responses),
    )
    .await;
    assert_bound_won(&refused, &format!("{MAX_INPUT_RESPONSES}-entry"));

    teardown(fixture.handle, ()).await;
}

/// The per-entry SIZE bound fires before the decode.
#[tokio::test]
async fn tasks_update_bounds_fire_before_the_decode_entry_bytes() {
    let fixture = paused_on(roots_only()).await;

    let oversized = json!({ "nothing": "x".repeat(MAX_INPUT_RESPONSE_BYTES + 1) });
    let refused = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ "roots": oversized }),
    )
    .await;
    assert_bound_won(&refused, &format!("{MAX_INPUT_RESPONSE_BYTES}-byte limit"));

    teardown(fixture.handle, ()).await;
}

/// The TOTAL-size bound fires before the decode.
///
/// Every entry here is individually UNDER the per-entry bound, which is the whole
/// reason the total bound exists: the many-medium-values shape the per-entry cap
/// alone lets through.
#[tokio::test]
async fn tasks_update_bounds_fire_before_the_decode_total_bytes() {
    let fixture = paused_on(roots_only()).await;

    let chunk = MAX_INPUT_RESPONSE_BYTES / 2;
    let entries = MAX_INPUT_RESPONSES_TOTAL_BYTES / chunk + 2;
    let mut responses: Map<String, Value> = (0..entries)
        .map(|i| {
            (
                format!("pad-{i:04}"),
                json!({ "nothing": "x".repeat(chunk) }),
            )
        })
        .collect();
    responses.insert("roots".to_string(), undecodable_answer());
    assert!(
        responses.len() <= MAX_INPUT_RESPONSES,
        "not the count bound"
    );

    let refused = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        Value::Object(responses),
    )
    .await;
    assert_bound_won(
        &refused,
        &format!("{MAX_INPUT_RESPONSES_TOTAL_BYTES}-byte total limit"),
    );

    teardown(fixture.handle, ()).await;
}

/// The nesting-DEPTH bound fires before the decode.
#[tokio::test]
async fn tasks_update_bounds_fire_before_the_decode_depth() {
    let fixture = paused_on(roots_only()).await;

    let refused = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ "roots": nested(MAX_INPUT_RESPONSE_DEPTH + 1) }),
    )
    .await;
    assert_bound_won(&refused, &format!("{MAX_INPUT_RESPONSE_DEPTH}-level"));

    teardown(fixture.handle, ()).await;
}

// ===========================================================================
// 11. The raw-map boundary, proven from BOTH sides.
// ===========================================================================

/// The route never runs the untagged decoder at ingress — proven by two requests
/// carrying the SAME value.
///
/// [`sampling_shaped_answer`] is decodable ONLY by the untagged path (as
/// `Sampling`) and is presented under a key the record holds as ELICITATION.
///
/// * **Over-bound**: the answer is the BOUNDS error. An implementation that
///   deserialized `inputResponses` into the typed `InputResponses` at parse time
///   would have run the untagged decoder before any bound could fire.
/// * **Within bounds**: the answer is the kind-directed REFUSAL — not a silent
///   reclassification into `Sampling`, and not a success.
///
/// Together the two pin the boundary: an implementation that typed the map up
/// front answers differently on at least one of them.
#[tokio::test]
async fn tasks_update_never_runs_the_untagged_decoder_on_ingress() {
    let fixture = paused_on(elicitation_only()).await;

    let mut over_bound = undecodable_filler(MAX_INPUT_RESPONSES);
    over_bound.insert("form".to_string(), sampling_shaped_answer());
    let bounded = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        Value::Object(over_bound),
    )
    .await;
    assert_bound_won(&bounded, &format!("{MAX_INPUT_RESPONSES}-entry"));

    let within = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        3,
        json!({ "form": sampling_shaped_answer() }),
    )
    .await;
    assert_eq!(
        code_of(&within),
        codes::INVALID_PARAMS,
        "within bounds, the kind-directed decode refuses: {}",
        within.raw
    );
    assert!(
        message_of(&within).contains(KIND_REFUSAL_MARKER),
        "and it is the KIND refusal, not a second bounds error: {}",
        within.raw
    );
    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 4).await,
        "input_required",
        "neither request may have resumed the task"
    );

    teardown(fixture.handle, ()).await;
}

// ===========================================================================
// 12. Concurrency.
// ===========================================================================

/// Two CONCURRENT deliveries against one paused task: the first wins and the
/// second sees the conflict.
///
/// The transition is ONE write inside the backend (`deliver_task_inputs` holds a
/// single entry guard across read/partition/write), so the loser cannot clobber
/// the winner. Its refusal is the state machine's: once the task is `working`,
/// `InputRequired -> Working` is no longer a legal transition for it to make.
///
/// The assertion is order-INDEPENDENT — it counts outcomes rather than naming
/// which request won — because "which of two concurrent requests arrives first"
/// is not a property this code controls, and a test that asserted it would be
/// asserting the scheduler.
#[tokio::test]
async fn tasks_update_cas_first_writer_wins() {
    let fixture = paused_on(roots_only()).await;

    let (first, second) = tokio::join!(
        update(
            fixture.addr,
            OWNER,
            &fixture.task_id,
            2,
            json!({ "roots": roots_answer() }),
        ),
        update(
            fixture.addr,
            OWNER,
            &fixture.task_id,
            3,
            json!({ "roots": { "roots": [{ "uri": "file:///second", "name": "second" }] } }),
        )
    );

    let acks = [&first, &second]
        .iter()
        .filter(|r| r.body.get("error").is_none())
        .count();
    assert_eq!(
        acks, 1,
        "exactly ONE of two concurrent deliveries may be accepted:\n{}\n{}",
        first.raw, second.raw
    );

    let loser = if first.body.get("error").is_some() {
        &first
    } else {
        &second
    };
    assert_eq!(
        code_of(loser),
        codes::INTERNAL_ERROR,
        "the loser sees the transition conflict: {}",
        loser.raw
    );

    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 4).await,
        "working",
        "the winner's transition stands"
    );

    teardown(fixture.handle, ()).await;
}

// ===========================================================================
// 13-15. Terminal tasks cannot be fed.
// ===========================================================================

/// Drive a paused task to `status` and assert a delivery against it is refused.
async fn assert_terminal_status_refuses_delivery(status: TaskStatus) {
    let fixture = paused_on(roots_only()).await;
    fixture
        .store
        .update_status(&fixture.task_id, OWNER, status, None)
        .await
        .unwrap_or_else(|e| panic!("input_required -> {status} is a legal transition: {e}"));

    let refused = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ "roots": roots_answer() }),
    )
    .await;

    assert_eq!(
        code_of(&refused),
        codes::INTERNAL_ERROR,
        "a {status} task cannot be fed: {}",
        refused.raw
    );
    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 3).await,
        status.to_string(),
        "the refused delivery left the terminal status alone"
    );

    teardown(fixture.handle, ()).await;
}

#[tokio::test]
async fn tasks_update_on_a_completed_task_is_refused() {
    assert_terminal_status_refuses_delivery(TaskStatus::Completed).await;
}

#[tokio::test]
async fn tasks_update_on_a_failed_task_is_refused() {
    assert_terminal_status_refuses_delivery(TaskStatus::Failed).await;
}

#[tokio::test]
async fn tasks_update_on_a_cancelled_task_is_refused() {
    assert_terminal_status_refuses_delivery(TaskStatus::Cancelled).await;
}

// ===========================================================================
// 16-17. The acknowledgement, and the cross-owner refusal.
// ===========================================================================

/// `UpdateTaskResult = Result`: the ack carries NO task fields.
///
/// `resultType: "complete"` is present because the ENVELOPE writes it —
/// `own_reserved_result_fields` owns that key — not because this route did. Every
/// task-shaped key is asserted absent individually, so a regression says WHICH
/// field leaked rather than only that the shape changed.
#[tokio::test]
async fn tasks_update_ack_is_empty() {
    let fixture = paused_on(roots_only()).await;

    let acked = update(
        fixture.addr,
        OWNER,
        &fixture.task_id,
        2,
        json!({ "roots": roots_answer() }),
    )
    .await;
    assert_is_ack(&acked);

    assert_eq!(
        acked.body["result"]["resultType"], "complete",
        "the extension requires resultType complete on an update ack: {}",
        acked.raw
    );
    for leaked in [
        "task",
        "taskId",
        "status",
        "createdAt",
        "lastUpdatedAt",
        "ttlMs",
        "pollIntervalMs",
        "inputRequests",
        "inputResponses",
        "result",
        "error",
    ] {
        assert!(
            acked.body["result"].get(leaked).is_none(),
            "an empty acknowledgement must not carry `{leaked}`: {}",
            acked.raw
        );
    }

    teardown(fixture.handle, ()).await;
}

/// Feeding ANOTHER owner's task is not-found — the same answer, byte for byte, as
/// an id that does not exist.
///
/// The owner is bound from the identity table and never read from `params`, so
/// there is nothing on the request a caller could set to reach someone else's
/// task (T-114-73). The message must be oracle-FREE: identical for absent and for
/// wrong-owner, naming neither the id nor any owner. A message that differed
/// between the two would re-open exactly the existence oracle the owner-prefixed
/// key design closes.
#[tokio::test]
async fn tasks_update_for_another_owner_is_not_found() {
    let fixture = paused_on(roots_only()).await;

    let cross_owner = update(
        fixture.addr,
        OTHER_OWNER,
        &fixture.task_id,
        2,
        json!({ "roots": roots_answer() }),
    )
    .await;
    let absent = update(
        fixture.addr,
        OTHER_OWNER,
        "task-that-never-existed-4b1c",
        3,
        json!({ "roots": roots_answer() }),
    )
    .await;

    assert_eq!(
        code_of(&cross_owner),
        codes::INVALID_PARAMS,
        "a task another owner holds is not found: {}",
        cross_owner.raw
    );
    assert_eq!(
        message_of(&cross_owner),
        message_of(&absent),
        "wrong-owner and absent must be INDISTINGUISHABLE"
    );
    assert!(
        !cross_owner.raw.contains(&fixture.task_id),
        "the refusal must not echo the requested id: {}",
        cross_owner.raw
    );
    for owner in [OWNER, OTHER_OWNER] {
        assert!(
            !cross_owner.raw.contains(owner),
            "the refusal must render no owner information: {}",
            cross_owner.raw
        );
    }

    assert_eq!(
        status_of(fixture.addr, OWNER, &fixture.task_id, 4).await,
        "input_required",
        "the real owner's task is untouched"
    );

    teardown(fixture.handle, ()).await;
}
