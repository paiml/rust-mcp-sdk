//! **No cross-caller task visibility**, proven over a REAL socket, per method
//! (Phase 114, plan 15 — TASK-05).
//!
//! `tests/v2_tasks_owner_binding.rs` (114-09) owns the ordered REFUSAL chain: which
//! of five conditions answers first, and what each one says. This file owns the
//! claim that chain exists to support — that two authenticated callers on one
//! server cannot reach each other's tasks — and it owns it on all THREE v2 `tasks/*`
//! methods, with a per-method negative control behind each.
//!
//! # Why a live socket, and why not a unit matrix over the identity table
//!
//! 114-D-09 rejects the unit matrix explicitly: *"that is precisely the shape
//! 113-31 caught as insufficient — the tests that would have failed did not
//! exist."* 113-31 measured a case where two of four mandated capability opt-ins
//! were exercised only by `#[cfg(test)]` unit tests and never over a socket; the
//! asymmetry was invisible in a green suite. So every refusal here is measured on
//! bytes that crossed a loopback TCP connection through the real
//! `StreamableHttpServer`, with two DIFFERENT bearer principals arriving as two
//! DIFFERENT subjects.
//!
//! # Indistinguishability is MEASURED, never asserted by inspection
//!
//! `PROJECT.md`'s standing no-info-leak decision is `NotFound`, never
//! `OwnerMismatch`. A test that hard-codes the expected sentence proves only that
//! *this* test and *that* code agree on a string. So each of tests 1–3 fires the
//! SAME method a second time against an id that genuinely does not exist, in the
//! same test, against the same server, and asserts the two answers are equal. A
//! divergence introduced anywhere — a different code, a different message, an extra
//! `data` payload — fails the equality rather than a literal.
//!
//! # A refusal that still performed the write would pass a code-only assertion
//!
//! Every refusal test re-reads the task AS ITS OWNER afterwards and asserts nothing
//! moved. For `tasks/cancel` that is the load-bearing half: an implementation that
//! answered `-32602` and cancelled anyway is a complete cross-caller compromise and
//! is invisible to an assertion on the response alone.
//!
//! # The properties
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `v2_cross_caller_tasks_get_is_not_found` | B cannot READ A's task |
//! | 2 | `v2_cross_caller_tasks_update_is_not_found` | B cannot FEED A's paused task |
//! | 3 | `v2_cross_caller_tasks_cancel_is_not_found` | B cannot CANCEL A's task |
//! | 4 | `v2_owner_isolation_holds_for_a_second_task_of_the_same_shape` | BOTH directions: each reads its own, neither the other's |
//! | 5 | `v2_a_guessed_task_id_is_not_found` | a never-minted id is refused identically, on all three methods |
//! | 6 | `task_ids_are_unguessable` | the entropy / non-sequential / non-derived PROPERTIES the spec states |
//! | 7 | `v1_local_and_v2_anonymous_buckets_are_disjoint` | one server, two eras, two anonymous spellings, two buckets |
//! | 8 | `a_no_auth_provider_server_shares_one_v2_bucket` | D-07's accepted caveat, ASSERTED rather than implied |
//!
//! Test 4 exists because firing only the refusal direction cannot distinguish
//! "isolated" from "broken for everyone" — a fail-closed implementation that
//! refused EVERY caller would satisfy tests 1-3 completely. Test 8 exists
//! because a documented, accepted weakness that no test states is a weakness a
//! future reader rediscovers as a bug.

#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    header, post, spawn_tasks_server_with_store, teardown, v1_body, v2_body_with_client_extensions,
    v2_headers, AuthPosture, Resp, PAUSING_TOOL_NAME, PAUSING_TOOL_REQUEST_KEY, TASKS_TOOL_NAME,
};
use pmcp::server::task_store::{InMemoryTaskStore, StoreConfig, TaskStore};
use pmcp::testing::ANONYMOUS_PRINCIPAL;
use pmcp::types::capabilities::TASKS_EXTENSION_KEY;
use pmcp::types::protocol::error_codes::INVALID_PARAMS;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::net::SocketAddr;

// ===========================================================================
// Principals and fixtures.
// ===========================================================================

/// The principal that OWNS every task in tests 1–5.
const OWNER_A: &str = "alice";

/// The OTHER principal. It authenticates successfully — it is a legitimate
/// caller of this server — and it holds A's task id. That is the whole threat
/// model of an IDOR: the attacker is a valid user with a guessed or leaked
/// identifier, not an outsider.
const OWNER_B: &str = "mallory";

/// The v1 anonymous owner bucket, frozen by D-10.
///
/// Spelled here rather than imported because `V1_UNAUTHENTICATED_OWNER` is
/// `pub(crate)` in `src/server/task_dispatch.rs` and this crate's public testing
/// seam deliberately does not re-export it. Test 7 asserts the DISJOINTNESS of
/// the two buckets, and a disjointness claim needs both spellings written down;
/// if this literal ever stops matching production, test 7 fails by observing the
/// v1 caller cannot read its OWN task, which is the loudest possible signal.
const V1_LOCAL_OWNER: &str = "local";

/// A well-formed task id that was never minted by any store in this suite.
///
/// A fixed literal rather than a freshly generated value: a literal cannot
/// collide with a minted id by construction, and it keeps every refusal in this
/// file byte-for-byte reproducible.
const NEVER_MINTED: &str = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";

/// The `Authorization` header for `subject`.
fn bearer(subject: &str) -> Vec<(String, String)> {
    vec![header("authorization", &format!("Bearer {subject}"))]
}

/// A v2 request that DECLARES the tasks extension, optionally authenticated.
///
/// `subject` is `Option` because tests 7 and 8 run against a server with NO auth
/// provider, where a credential is meaningless and its presence must not change
/// the outcome.
async fn declaring(
    addr: SocketAddr,
    subject: Option<&str>,
    method: &str,
    name: &str,
    id: i64,
    params: Value,
) -> Resp {
    let mut headers = v2_headers(method, name);
    if let Some(subject) = subject {
        headers.extend(bearer(subject));
    }
    let body = v2_body_with_client_extensions(method, json!(id), params, &[TASKS_EXTENSION_KEY]);
    post(addr, &headers, &body).await
}

async fn tasks_get(addr: SocketAddr, subject: Option<&str>, task_id: &str, id: i64) -> Resp {
    declaring(
        addr,
        subject,
        "tasks/get",
        task_id,
        id,
        json!({ "taskId": task_id }),
    )
    .await
}

async fn tasks_cancel(addr: SocketAddr, subject: Option<&str>, task_id: &str, id: i64) -> Resp {
    declaring(
        addr,
        subject,
        "tasks/cancel",
        task_id,
        id,
        json!({ "taskId": task_id }),
    )
    .await
}

/// Send a `tasks/update` carrying `responses` verbatim.
///
/// The map is a parameter because the refusal and the absent-id control must
/// carry the IDENTICAL payload — otherwise a difference in the answer could be
/// attributed to the payload rather than to the id.
async fn tasks_update(
    addr: SocketAddr,
    subject: Option<&str>,
    task_id: &str,
    id: i64,
    responses: Value,
) -> Resp {
    declaring(
        addr,
        subject,
        "tasks/update",
        task_id,
        id,
        json!({ "taskId": task_id, "inputResponses": responses }),
    )
    .await
}

/// The only value that decodes under the pausing tool's recorded `roots/list`
/// key — a `ListRootsResult`.
fn roots_answer() -> Value {
    json!({ PAUSING_TOOL_REQUEST_KEY: { "roots": [] } })
}

/// Mint a task through a REAL v2 `tools/call` and return its FLAT create result.
///
/// The whole result rather than just the id: `taskId`, `status`, `createdAt` and
/// `lastUpdatedAt` together are the baseline [`assert_record_untouched`] compares
/// against, and taking them from the create response means no extra read is
/// needed before the refusal fires.
async fn create_task(addr: SocketAddr, subject: Option<&str>, tool: &str, id: i64) -> Value {
    let created = declaring(
        addr,
        subject,
        "tools/call",
        tool,
        id,
        json!({ "name": tool, "arguments": {} }),
    )
    .await;
    created
        .body
        .get("result")
        .filter(|result| result.get("taskId").is_some())
        .unwrap_or_else(|| {
            panic!(
                "a declaring v2 tools/call on `{tool}` mints a flat task handle: {}",
                created.raw
            )
        })
        .clone()
}

fn task_id_of(create_result: &Value) -> String {
    create_result["taskId"]
        .as_str()
        .expect("a flat v2 create result carries a top-level taskId")
        .to_string()
}

/// The `result` object of a success response, or a panic naming the error.
fn result_of(response: &Resp) -> &Value {
    response.body.get("result").unwrap_or_else(|| {
        panic!("expected a success result, got {}", response.raw);
    })
}

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

// ===========================================================================
// The shared refusal assertion — five independent facts.
// ===========================================================================

/// Words a wrong-owner refusal must never contain.
///
/// Each is a spelling of the fact the owner-prefixed lookup exists to withhold:
/// that the id resolved to a record belonging to someone else. `forbidden` is
/// included because an HTTP-shaped instinct reaches for it, and it is exactly the
/// word that turns a refusal into an existence oracle.
const LEAKY_WORDS: [&str; 3] = ["owner", "mismatch", "forbidden"];

/// Assert `refused` is the oracle-free not-found answer, MEASURED against
/// `absent` — the SAME method's answer, on the SAME server, for an id that
/// genuinely does not exist.
///
/// Five independent facts, because "B was refused" has five observable spellings
/// and a regression could break any one of them alone:
///
/// 1. the code is the v2 task-not-found code `-32602` (114-11's mapping),
/// 2. the code is the same one an absent id earns,
/// 3. the MESSAGE is byte-identical to the absent id's — measured, not inspected,
/// 4. neither the message nor the raw frame names an owner, a mismatch, a
///    prohibition, or the OTHER caller's subject,
/// 5. the frame carries no fragment of the task: no `result`, no `status`, no
///    `createdAt`, no `taskId`.
///
/// Fact 3 is the anti-oracle guarantee. Facts 1–2 alone are satisfied by an
/// implementation that answers `-32602 "task belongs to another owner"`, which
/// tells an attacker its guessed id is REAL.
fn assert_indistinguishable_not_found(
    method: &str,
    refused: &Resp,
    absent: &Resp,
    other_subject: &str,
) {
    assert_eq!(
        code_of(refused),
        i64::from(INVALID_PARAMS),
        "{method} for another caller's task is the v2 task-not-found code: {}",
        refused.raw
    );
    assert_eq!(
        code_of(absent),
        i64::from(INVALID_PARAMS),
        "{method} for a genuinely absent id must reach the same code, or the equality \
         below would compare two unrelated failures: {}",
        absent.raw
    );
    assert_eq!(
        message_of(refused),
        message_of(absent),
        "{method}: a wrong-owner refusal must be INDISTINGUISHABLE from an absent id. \
         Both messages are computed in this test from the same server, so this is a \
         measurement, not a literal. wrong-owner: {} / absent: {}",
        refused.raw,
        absent.raw
    );
    assert_eq!(
        error_of(refused).get("data"),
        error_of(absent).get("data"),
        "{method}: an `error.data` payload present on one and not the other is an oracle \
         even when the messages match: {}",
        refused.raw
    );

    let lowered = refused.raw.to_lowercase();
    for word in LEAKY_WORDS {
        assert!(
            !lowered.contains(word),
            "{method}: the refusal must not contain `{word}` — naming the reason confirms \
             the id exists, which is the one fact the owner-scoped lookup withholds: {}",
            refused.raw
        );
    }
    assert!(
        !lowered.contains(&other_subject.to_lowercase()),
        "{method}: the refusal must not name `{other_subject}`, the task's real owner: {}",
        refused.raw
    );

    assert!(
        refused.body["result"].is_null(),
        "{method}: a refusal carries no result: {}",
        refused.raw
    );
    for fragment in [
        "\"status\"",
        "\"createdAt\"",
        "\"lastUpdatedAt\"",
        "\"taskId\"",
    ] {
        assert!(
            !refused.raw.contains(fragment),
            "{method}: no fragment of the refused task may appear on the wire, and \
             {fragment} did: {}",
            refused.raw
        );
    }
}

/// Assert nothing about the task moved between `before` and `after`.
///
/// `lastUpdatedAt` is the load-bearing field: every mutating path in
/// `InMemoryTaskStore` (`update_status`, `deliver_task_inputs`,
/// `record_input_requests`) rewrites it in the same write that changes the
/// record. Equal timestamps therefore measure "no write landed", which is
/// strictly stronger than "the status I happened to check is unchanged".
fn assert_record_untouched(before: &Value, after: &Value, context: &str) {
    for field in ["taskId", "status", "createdAt", "lastUpdatedAt"] {
        // Presence FIRST. Two absent fields compare equal, so a comparison over
        // a payload that stopped carrying them would pass while measuring
        // nothing — and the v2 create payload and the v2 `tasks/get` payload are
        // built by two different projections that could drift apart.
        assert!(
            before[field].is_string(),
            "{context}: the baseline payload must carry `{field}` or the comparison \
             below is vacuous: {before}"
        );
        assert!(
            after[field].is_string(),
            "{context}: the re-read payload must carry `{field}` or the comparison \
             below is vacuous: {after}"
        );
        assert_eq!(
            before[field], after[field],
            "{context}: `{field}` moved, so the refused call DID write to the record. \
             before: {before} / after: {after}"
        );
    }
}

// ===========================================================================
// 1 — B cannot READ A's task.
// ===========================================================================

/// A cross-caller `tasks/get` is refused, and the refusal is indistinguishable
/// from an absent id.
///
/// The refusal fires before A ever re-reads, so a fixture that leaked cannot be
/// mistaken for the property: the only successful read in this test happens
/// AFTER the refusal has been captured.
#[tokio::test]
async fn v2_cross_caller_tasks_get_is_not_found() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Required).await;
    let created = create_task(addr, Some(OWNER_A), TASKS_TOOL_NAME, 1).await;
    let task_id = task_id_of(&created);

    let refused = tasks_get(addr, Some(OWNER_B), &task_id, 2).await;
    let absent = tasks_get(addr, Some(OWNER_B), NEVER_MINTED, 3).await;
    // The owner's own read is the non-vacuity control AND the after-state.
    let owner_view = tasks_get(addr, Some(OWNER_A), &task_id, 4).await;
    teardown(handle, ()).await;

    assert_indistinguishable_not_found("tasks/get", &refused, &absent, OWNER_A);
    assert_eq!(
        result_of(&owner_view)["taskId"],
        json!(task_id),
        "the control: the SAME id resolves for its OWNER, so the refusal above is \
         attributable to the caller and not to a broken lookup: {}",
        owner_view.raw
    );
    assert_record_untouched(&created, result_of(&owner_view), "tasks/get");
}

// ===========================================================================
// 2 — B cannot FEED A's paused task.
// ===========================================================================

/// A cross-caller `tasks/update` is refused, delivers nothing, and the refusal is
/// indistinguishable from an absent id.
///
/// The task is PAUSED (`input_required` with one outstanding `roots/list` key)
/// rather than `working`, and that is a correctness requirement of the test, not
/// a decoration: `task_input_snapshot` answers `NotFound` for a task with no
/// recorded requests, so a `working` task would refuse EVERY caller and the
/// refusal would prove nothing about ownership. Against a paused task the same
/// payload from A succeeds — which is asserted here, last, as the non-vacuity
/// control.
///
/// The store is consulted directly after the refusal because the wire cannot show
/// what a `tasks/update` persisted: `inputResponses` never appears in a
/// `tasks/get` payload. `input_responses` being EMPTY is the direct measurement
/// that B's payload was not written.
#[tokio::test]
async fn v2_cross_caller_tasks_update_is_not_found() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::Required).await;
    let created = create_task(addr, Some(OWNER_A), PAUSING_TOOL_NAME, 1).await;
    let task_id = task_id_of(&created);

    let refused = tasks_update(addr, Some(OWNER_B), &task_id, 2, roots_answer()).await;
    let absent = tasks_update(addr, Some(OWNER_B), NEVER_MINTED, 3, roots_answer()).await;
    let after_refusal = tasks_get(addr, Some(OWNER_A), &task_id, 4).await;
    let snapshot = store
        .task_input_snapshot(&task_id, OWNER_A)
        .await
        .expect("the owner can snapshot its own paused task");
    // Non-vacuity, fired LAST: the identical payload from the OWNER lands.
    let accepted = tasks_update(addr, Some(OWNER_A), &task_id, 5, roots_answer()).await;
    let after_delivery = tasks_get(addr, Some(OWNER_A), &task_id, 6).await;
    teardown(handle, ()).await;

    assert_indistinguishable_not_found("tasks/update", &refused, &absent, OWNER_A);
    assert_eq!(
        result_of(&after_refusal)["status"],
        json!("input_required"),
        "the task must still be PAUSED: a refusal that resumed it would be a complete \
         cross-caller write: {}",
        after_refusal.raw
    );
    assert!(
        snapshot.input_responses.is_empty(),
        "B's payload must not have been persisted; the record held {:?}",
        snapshot.input_responses
    );
    assert_eq!(
        snapshot.outstanding().len(),
        1,
        "the outstanding set is untouched, so nothing was consumed on B's behalf"
    );
    assert!(
        accepted.body.get("error").is_none(),
        "the control: the SAME payload from the OWNER is acknowledged, so the refusal \
         above is attributable to the caller and not to the payload: {}",
        accepted.raw
    );
    assert_eq!(
        result_of(&after_delivery)["status"],
        json!("working"),
        "and the owner's delivery genuinely resumed the task — without this the refusal \
         above would be consistent with `tasks/update` being broken for everyone: {}",
        after_delivery.raw
    );
}

// ===========================================================================
// 3 — B cannot CANCEL A's task.
// ===========================================================================

/// A cross-caller `tasks/cancel` is refused AND does not cancel.
///
/// This is the test where the response alone proves the least. `tasks/cancel`
/// answers an EMPTY acknowledgement on v2 and cancellation is cooperative and
/// eventually consistent, so an implementation that refused the caller with
/// `-32602` and cancelled the task anyway would satisfy every assertion about the
/// response bytes. The owner's re-read is therefore not a supporting detail — it
/// is the property.
#[tokio::test]
async fn v2_cross_caller_tasks_cancel_is_not_found() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Required).await;
    let created = create_task(addr, Some(OWNER_A), TASKS_TOOL_NAME, 1).await;
    let task_id = task_id_of(&created);

    let refused = tasks_cancel(addr, Some(OWNER_B), &task_id, 2).await;
    let absent = tasks_cancel(addr, Some(OWNER_B), NEVER_MINTED, 3).await;
    let after_refusal = tasks_get(addr, Some(OWNER_A), &task_id, 4).await;
    // Non-vacuity, fired LAST: the owner CAN cancel the very same task.
    let cancelled = tasks_cancel(addr, Some(OWNER_A), &task_id, 5).await;
    let after_cancel = tasks_get(addr, Some(OWNER_A), &task_id, 6).await;
    teardown(handle, ()).await;

    assert_indistinguishable_not_found("tasks/cancel", &refused, &absent, OWNER_A);
    assert_eq!(
        result_of(&after_refusal)["status"],
        json!("working"),
        "THE load-bearing assertion of this test: a refusal that still cancelled would \
         pass every assertion above it: {}",
        after_refusal.raw
    );
    assert_record_untouched(&created, result_of(&after_refusal), "tasks/cancel");
    assert!(
        cancelled.body.get("error").is_none(),
        "the control: the OWNER's cancel is acknowledged: {}",
        cancelled.raw
    );
    assert_eq!(
        result_of(&after_cancel)["status"],
        json!("cancelled"),
        "and it genuinely cancelled — without this the untouched status above would be \
         consistent with cancel being broken for everyone: {}",
        after_cancel.raw
    );
}

// ===========================================================================
// 4 — BOTH directions, on two tasks of the same shape.
// ===========================================================================

/// Two principals, two tasks, one server: each reads its OWN and neither reads
/// the other's.
///
/// Negative control A's whole point. Firing only the refusal direction cannot
/// distinguish "the tasks are isolated" from "`tasks/get` is broken for
/// everyone", and a fail-closed implementation that refused every caller would
/// satisfy tests 1–3 completely. Both refusals fire before either success.
#[tokio::test]
async fn v2_owner_isolation_holds_for_a_second_task_of_the_same_shape() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Required).await;
    let a_task = task_id_of(&create_task(addr, Some(OWNER_A), TASKS_TOOL_NAME, 1).await);
    let b_task = task_id_of(&create_task(addr, Some(OWNER_B), TASKS_TOOL_NAME, 2).await);

    assert_ne!(
        a_task, b_task,
        "two creates must mint two distinct ids, or the isolation claim is vacuous"
    );

    // Refusals first, in both directions.
    let a_reads_b = tasks_get(addr, Some(OWNER_A), &b_task, 3).await;
    let b_reads_a = tasks_get(addr, Some(OWNER_B), &a_task, 4).await;
    let a_absent = tasks_get(addr, Some(OWNER_A), NEVER_MINTED, 5).await;
    let b_absent = tasks_get(addr, Some(OWNER_B), NEVER_MINTED, 6).await;
    // Then the two successes.
    let a_reads_a = tasks_get(addr, Some(OWNER_A), &a_task, 7).await;
    let b_reads_b = tasks_get(addr, Some(OWNER_B), &b_task, 8).await;
    teardown(handle, ()).await;

    assert_indistinguishable_not_found("tasks/get (A->B)", &a_reads_b, &a_absent, OWNER_B);
    assert_indistinguishable_not_found("tasks/get (B->A)", &b_reads_a, &b_absent, OWNER_A);
    assert_eq!(
        result_of(&a_reads_a)["taskId"],
        json!(a_task),
        "A reads its OWN task: {}",
        a_reads_a.raw
    );
    assert_eq!(
        result_of(&b_reads_b)["taskId"],
        json!(b_task),
        "B reads its OWN task — so the refusals above are isolation, not an outage: {}",
        b_reads_b.raw
    );
}

// ===========================================================================
// 5 — a never-minted id, on all three methods.
// ===========================================================================

/// A well-formed id that was never minted is refused identically on every
/// method.
///
/// The oracle has two directions, and tests 1–3 close only one of them. They
/// prove a WRONG-OWNER id does not look different from an absent one; this
/// proves an ABSENT id does not look different from a real one — the same
/// refusal, with the caller's own real task standing by as the control that the
/// server is serving at all.
#[tokio::test]
async fn v2_a_guessed_task_id_is_not_found() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Required).await;
    let owned = task_id_of(&create_task(addr, Some(OWNER_A), TASKS_TOOL_NAME, 1).await);

    let guessed_get = tasks_get(addr, Some(OWNER_A), NEVER_MINTED, 2).await;
    let guessed_update = tasks_update(addr, Some(OWNER_A), NEVER_MINTED, 3, roots_answer()).await;
    let guessed_cancel = tasks_cancel(addr, Some(OWNER_A), NEVER_MINTED, 4).await;
    let real = tasks_get(addr, Some(OWNER_A), &owned, 5).await;
    teardown(handle, ()).await;

    for (method, response) in [
        ("tasks/get", &guessed_get),
        ("tasks/update", &guessed_update),
        ("tasks/cancel", &guessed_cancel),
    ] {
        assert_eq!(
            code_of(response),
            i64::from(INVALID_PARAMS),
            "{method} on a never-minted id is the same task-not-found code every other \
             unreachable id earns: {}",
            response.raw
        );
        assert!(
            !response.raw.contains(NEVER_MINTED),
            "{method} must not echo the guessed id back — an echo turns the log into an \
             attacker-chosen channel: {}",
            response.raw
        );
    }
    assert_eq!(
        result_of(&real)["taskId"],
        json!(owned),
        "the control: a REAL id owned by the same caller resolves, so the three refusals \
         above are about the id and not about the server: {}",
        real.raw
    );
}

// ===========================================================================
// 6 — the unguessability PROPERTIES, not a format lock.
// ===========================================================================

/// How many ids the entropy and prefix measurements draw.
///
/// The per-position measurement below is a LOWER bound that converges from
/// beneath: a position can only be seen to carry `k` distinct values once all `k`
/// have actually appeared. With 1024 draws the chance that any one of the
/// sixteen values is missing from any one of the free positions is under 1e-26,
/// so the bound is tight rather than merely valid.
const ENTROPY_SAMPLE: usize = 1024;

/// The number of bits the spec's "cannot enumerate or guess" clause requires.
///
/// 122 is not chosen to match today's encoding — it is the floor below which a
/// remote attacker's guessing budget becomes relevant at all.
const REQUIRED_ENTROPY_BITS: f64 = 122.0;

/// The most characters two ids minted back-to-back for the SAME owner may share
/// beyond the encoding's fixed literal prefix.
///
/// Calibrated in both directions. For a uniform 16-symbol alphabet the chance
/// that any of the 1023 adjacent pairs shares nine or more leading symbols is
/// about 1.5e-8, so a correct generator does not trip it. A timestamp-prefixed
/// encoding (`UUIDv7` and friends) shares roughly the top 38 bits — nine or ten
/// symbols — for ids minted within the same second, so an incorrect generator
/// does.
const MAX_SHARED_PREFIX: usize = 8;

/// A store whose per-owner cap is above [`ENTROPY_SAMPLE`].
///
/// `StoreConfig::default()` caps an owner at 100 live tasks, which is a sensible
/// production default and a sample size too small to measure the bound above.
fn sampling_store() -> InMemoryTaskStore {
    InMemoryTaskStore::with_config(StoreConfig {
        max_tasks_per_owner: ENTROPY_SAMPLE * 4,
        ..StoreConfig::default()
    })
}

async fn mint_ids(store: &InMemoryTaskStore, owner: &str, count: usize) -> Vec<String> {
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        let task = store
            .create(owner, None)
            .await
            .expect("the sampling store mints without hitting its per-owner cap");
        ids.push(task.task_id);
    }
    ids
}

/// A lower bound, in bits, on the entropy this ENCODING realizes.
///
/// For each character position, count the DISTINCT characters observed there
/// across the whole sample and sum `log2(distinct)`. A constant position — a
/// literal separator, a version marker, a fixed prefix — contributes exactly
/// zero. A position drawing uniformly from `k` symbols contributes `log2(k)`
/// once the sample has seen all `k`.
///
/// Deliberately format-agnostic: it makes no assumption about hex, about
/// separators, or about a version nibble. A base32 or base64 identifier carrying
/// the same entropy passes it unchanged, which is the point — locking a FORMAT
/// would block a future move to a stronger encoding for no security gain.
fn observed_entropy_bits(ids: &[String]) -> f64 {
    let rows: Vec<Vec<char>> = ids.iter().map(|id| id.chars().collect()).collect();
    let width = rows[0].len();
    (0..width)
        .map(|position| {
            let distinct: BTreeSet<char> = rows.iter().map(|row| row[position]).collect();
            (distinct.len() as f64).log2()
        })
        .sum()
}

/// The number of leading characters `left` and `right` share.
fn common_prefix_len(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(a, b)| a == b)
        .count()
}

/// The fixed literal every id in `ids` begins with — length in characters.
///
/// The spec permits a constant prefix (`task_`, a tenant tag, a version marker);
/// what it forbids is structure BEYOND that literal which an attacker can
/// predict. Measuring the literal rather than assuming it is absent is what lets
/// the prefix assertion below survive a future encoding that adds one.
fn shared_literal_prefix(ids: &[String]) -> usize {
    ids.iter()
        .skip(1)
        .fold(ids[0].chars().count(), |shortest, id| {
            shortest.min(common_prefix_len(&ids[0], id))
        })
}

/// Interpret an id as a big integer, if its encoding admits one.
///
/// Returns `None` when the id is not a run of hex digits and separators. The
/// caller asserts the conversion SUCCEEDED, so a future encoding change cannot
/// make the adjacency check pass vacuously by silently returning `None`.
fn as_integer(id: &str) -> Option<u128> {
    let digits: String = id.chars().filter(|c| *c != '-').collect();
    if digits.len() != 32 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(&digits, 16).ok()
}

/// Minted task ids carry the PROPERTIES the extension requires: enough entropy
/// to be unguessable, no sequence, and no derivation from the owner or the
/// clock.
///
/// # This test deliberately does NOT assert a format
///
/// The spec requires ids "generated with sufficient entropy that a third party
/// cannot enumerate or guess them". It does not mandate any particular
/// identifier standard. Today's ids happen to be random 128-bit values rendered
/// as 8-4-4-4-12 hex, and that is recorded here as a supporting OBSERVATION —
/// but asserting it would freeze the encoding, and a future move to a longer or
/// cryptographically-stronger rendering would then fail a test for no security
/// reason. Every assertion below is on a property that any conformant encoding
/// satisfies.
///
/// # Why this is the whole enumeration defence
///
/// `tasks/list` is RETIRED on v2 (114-09 case 1 answers `-32601` without
/// consulting any backend), so there is no enumeration surface at all. What
/// remains is guessing, and owner-keying plus unguessability is the entirety of
/// the answer to it.
#[tokio::test]
async fn task_ids_are_unguessable() {
    let store = sampling_store();
    let ids = mint_ids(&store, OWNER_A, ENTROPY_SAMPLE).await;

    // The sample must be REPRESENTATIVE of what a client actually receives, or
    // it measures a code path no caller reaches.
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::Required).await;
    let over_the_wire = task_id_of(&create_task(addr, Some(OWNER_A), TASKS_TOOL_NAME, 1).await);
    teardown(handle, ()).await;

    let width = ids[0].chars().count();
    assert!(
        ids.iter().all(|id| id.chars().count() == width),
        "the per-position estimator below assumes a fixed width; a variable-length \
         encoding needs a different estimator and this assertion is where a reader \
         finds that out"
    );
    assert_eq!(
        over_the_wire.chars().count(),
        width,
        "an id minted over a real socket has the same shape as the sampled ones, so \
         the measurements below describe the ids a caller actually holds"
    );

    // --- 1. Entropy ------------------------------------------------------
    let bits = observed_entropy_bits(&ids);
    assert!(
        bits >= REQUIRED_ENTROPY_BITS,
        "minted ids realize only {bits:.1} bits of entropy across {ENTROPY_SAMPLE} \
         samples, below the {REQUIRED_ENTROPY_BITS} the unguessability requirement \
         needs. This is a LOWER bound computed from the sample, so a value under the \
         floor means the encoding genuinely cannot carry it"
    );

    // --- 2. Uniqueness and non-sequence ----------------------------------
    let unique: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "a repeated id is a collision, and a collision is a cross-caller read waiting \
         for the right two owners"
    );

    let mut sorted = ids.clone();
    sorted.sort();
    assert_ne!(
        sorted, ids,
        "the mint order must not BE the sort order: every monotonic generator — a \
         counter, a timestamp, a lexicographically-sortable identifier — produces ids \
         already in order, and one that holds for {ENTROPY_SAMPLE} draws is not chance"
    );

    let numbers: Vec<u128> = sorted
        .iter()
        .map(|id| {
            as_integer(id).unwrap_or_else(|| {
                panic!(
                    "the adjacency check needs a total order the encoding admits, and \
                     `{id}` is not one this helper knows how to read; teach it the new \
                     encoding rather than deleting the check"
                )
            })
        })
        .collect();
    for pair in numbers.windows(2) {
        assert_ne!(
            pair[1] - pair[0],
            1,
            "two minted ids are numerically ADJACENT ({:x} and {:x}); holding one would \
             hand an attacker its neighbour",
            pair[0],
            pair[1]
        );
    }

    // --- 3. Not derived from the owner or the clock ----------------------
    // Every id here was minted for the SAME owner, within the same second or
    // two, so any owner-derivation or timestamp prefix shows up as a long shared
    // prefix between consecutive mints.
    let literal = shared_literal_prefix(&ids);
    for pair in ids.windows(2) {
        let shared = common_prefix_len(&pair[0], &pair[1]).saturating_sub(literal);
        assert!(
            shared <= MAX_SHARED_PREFIX,
            "`{}` and `{}` were minted back-to-back for the same owner and share {shared} \
             characters beyond the {literal}-character fixed literal — that is the \
             signature of a timestamp or owner-derived prefix, which makes the \
             high-order part of an id predictable",
            pair[0],
            pair[1]
        );
    }
    assert!(
        ids.iter().all(|id| !id.contains(OWNER_A)),
        "no minted id may embed the owner it belongs to"
    );

    // The owner-derivation check needs a second owner to be non-vacuous: if ids
    // carried an owner-specific prefix, one owner's ids would share more with
    // each other than the population does.
    let other = mint_ids(&store, OWNER_B, 64).await;
    let mut population = ids.clone();
    population.extend(other.iter().cloned());
    assert_eq!(
        shared_literal_prefix(&other),
        shared_literal_prefix(&population),
        "one owner's ids share no more of a prefix than the whole population does, so \
         the id carries no owner tag"
    );
}

// ===========================================================================
// 7 — one server, two eras, two anonymous spellings, two BUCKETS.
// ===========================================================================

/// The path, relative to the crate root, of the `pmcp-tasks` predicate this test
/// makes a statement about.
const GENERIC_STORE_SOURCE: &str = "crates/pmcp-tasks/src/store/generic.rs";

/// The path of the `pmcp-tasks` key builder whose owner prefix IS the
/// disjointness.
const BACKEND_SOURCE: &str = "crates/pmcp-tasks/src/store/backend.rs";

fn read_workspace_source(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Complete a REAL v1 handshake and return the headers a v1 caller must carry.
///
/// D-114-J: the shared harness spawns with `StreamableHttpServerConfig::default()`,
/// which is STATEFUL on purpose, so a v1 caller has to negotiate and then carry
/// `Mcp-Session-Id` — otherwise it is answered `-32600`, which looks like a tasks
/// bug and is not one.
async fn v1_session_headers(addr: SocketAddr) -> Vec<(String, String)> {
    // On `--no-default-features --features full-v2` there is no session
    // machinery at all: `validate_non_init_session` is the null twin, so a v1
    // caller needs no handshake and carries no `Mcp-Session-Id`. Returning the
    // auth headers alone keeps every caller below RUNNING on the severed build,
    // instead of this whole file being gated away for one handshake.
    if !cfg!(feature = "v1-compat") {
        return Vec::new();
    }
    let initialized = post(
        addr,
        &[],
        &v1_body(
            "initialize",
            json!(0),
            json!({
                "protocolVersion": common::v2::V1,
                "capabilities": {},
                "clientInfo": { "name": "v1-client", "version": "0.0.0" }
            }),
        ),
    )
    .await;
    let session = initialized.mcp_session_id.unwrap_or_else(|| {
        panic!(
            "a stateful v1 handshake must mint a session id: {}",
            initialized.raw
        )
    });
    vec![header(
        pmcp::shared::http_constants::MCP_SESSION_ID,
        &session,
    )]
}

/// The v1 `"local"` bucket and the v2 anonymous bucket are DISJOINT storage
/// namespaces on one server.
///
/// # Two statements that are easy to confuse, and are asserted separately
///
/// * **Disjointness** is about the storage KEY. `pmcp-tasks` prefixes every key
///   with its owner (`make_key` -> `"{owner_id}:{task_id}"`), so `":<id>"` and
///   `"local:<id>"` are different keys and neither owner's read can reach the
///   other's record. The in-crate `InMemoryTaskStore` reaches the same outcome
///   through `validate_access`'s owner comparison rather than a key prefix, and
///   that is the store this live server runs, so the wire half below measures it
///   there.
/// * **`is_anonymous_owner`** is about whether an owner counts as anonymous for
///   the `allow_anonymous` refusal, and it treats `""` and `"local"`
///   IDENTICALLY. That is not a contradiction of the first statement: a
///   production backend refuses BOTH buckets by default while still keeping them
///   in separate namespaces.
///
/// The predicate half is asserted at the SOURCE because `pmcp-tasks` is not a
/// dependency of the `pmcp` crate in any profile — adding one to reach a private
/// helper would be a heavier change than the claim justifies, and this plan
/// touches no manifest. `crates/pmcp-tasks/tests/input_delivery.rs` (114-07)
/// owns the BEHAVIOURAL twin of the predicate claim from inside that crate;
/// this is its pmcp-side counterpart and does not restate it.
#[tokio::test]
async fn v1_local_and_v2_anonymous_buckets_are_disjoint() {
    let (addr, handle, store) = spawn_tasks_server_with_store(AuthPosture::None).await;

    // --- the v1 caller: owner "local" ------------------------------------
    let v1_headers = v1_session_headers(addr).await;
    let v1_created = post(
        addr,
        &v1_headers,
        &v1_body(
            "tools/call",
            json!(1),
            json!({ "name": TASKS_TOOL_NAME, "arguments": {}, "task": {} }),
        ),
    )
    .await;
    let v1_task = v1_created.body["result"]["task"]["taskId"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "a v1 create envelope nests under `task`: {}",
                v1_created.raw
            )
        })
        .to_string();

    // --- the v2 caller: owner ANONYMOUS_PRINCIPAL ------------------------
    let v2_task = task_id_of(&create_task(addr, None, TASKS_TOOL_NAME, 2).await);

    // Cross-era reads, both directions, BEFORE either same-era read.
    let v2_reads_v1 = tasks_get(addr, None, &v1_task, 3).await;
    let v1_reads_v2 = post(
        addr,
        &v1_headers,
        &v1_body("tasks/get", json!(4), json!({ "taskId": v2_task })),
    )
    .await;
    // The controls.
    let v2_reads_v2 = tasks_get(addr, None, &v2_task, 5).await;
    let v1_reads_v1 = post(
        addr,
        &v1_headers,
        &v1_body("tasks/get", json!(6), json!({ "taskId": v1_task })),
    )
    .await;
    teardown(handle, ()).await;

    assert_ne!(
        v1_task, v2_task,
        "the two eras minted two distinct ids, or the disjointness claim is vacuous"
    );
    assert!(
        v2_reads_v1.body.get("error").is_some(),
        "a v2 (anonymous-principal) caller must not reach the v1 `local` bucket: {}",
        v2_reads_v1.raw
    );
    assert!(
        v1_reads_v2.body.get("error").is_some(),
        "and a v1 (`local`) caller must not reach the v2 anonymous bucket: {}",
        v1_reads_v2.raw
    );
    assert_eq!(
        result_of(&v2_reads_v2)["taskId"],
        json!(v2_task),
        "the control: the v2 caller reads its OWN task: {}",
        v2_reads_v2.raw
    );
    assert_eq!(
        v1_reads_v1.body["result"]["task"]["taskId"],
        json!(v1_task),
        "the control: the v1 caller reads its OWN task, so the refusals above are \
         disjointness and not an outage: {}",
        v1_reads_v1.raw
    );

    // The same disjointness at the STORE, where the owner scoping actually
    // lives — the wire assertions above cannot distinguish "the owner differs"
    // from "the route refused for some other reason".
    assert!(
        store.get(&v1_task, ANONYMOUS_PRINCIPAL).await.is_err(),
        "the v1 task is not readable under the v2 anonymous owner"
    );
    assert!(
        store.get(&v2_task, V1_LOCAL_OWNER).await.is_err(),
        "the v2 task is not readable under the v1 `local` owner"
    );
    assert!(
        store.get(&v1_task, V1_LOCAL_OWNER).await.is_ok(),
        "and each IS readable under its own owner, so the two refusals above are about \
         the owner and not about the ids"
    );
    assert!(store.get(&v2_task, ANONYMOUS_PRINCIPAL).await.is_ok());
    assert_ne!(
        ANONYMOUS_PRINCIPAL, V1_LOCAL_OWNER,
        "the two buckets are two DIFFERENT owner strings; that is what makes their \
         storage namespaces disjoint"
    );

    // --- the SECOND, different statement ---------------------------------
    // `is_anonymous_owner` treats the two spellings IDENTICALLY. This is a claim
    // about the `allow_anonymous` refusal, NOT about namespaces, and conflating
    // the two is the mistake this block exists to prevent.
    let generic = read_workspace_source(GENERIC_STORE_SOURCE);
    assert!(
        generic.contains("fn is_anonymous_owner(owner_id: &str) -> bool {\n        owner_id.is_empty() || owner_id == DEFAULT_LOCAL_OWNER\n    }"),
        "`is_anonymous_owner` in {GENERIC_STORE_SOURCE} must keep treating the empty \
         owner and the `local` owner identically. If this predicate was split, the \
         `allow_anonymous: false` default no longer refuses both buckets and this \
         test's rustdoc has become wrong"
    );
    // …while the KEY builder is what keeps the namespaces apart.
    let backend = read_workspace_source(BACKEND_SOURCE);
    assert!(
        backend.contains(r#"format!("{owner_id}:{task_id}")"#),
        "`make_key` in {BACKEND_SOURCE} must keep prefixing by owner; dropping the \
         prefix collapses every owner's tasks into one namespace and the disjointness \
         asserted above stops holding on the production backends"
    );
}

// ===========================================================================
// 8 — D-07's accepted caveat, ASSERTED.
// ===========================================================================

/// On a server with NO auth provider, two v2 callers share ONE task bucket and
/// CAN see each other's tasks.
///
/// # This is accepted, documented behaviour — not a bug, and not a bug report
///
/// Row 3 of the v2 identity table maps a caller on an auth-provider-less server
/// onto `ANONYMOUS_PRINCIPAL`. There is no second identity to map it to: such a
/// server has no notion of caller identity at all, so "isolate the callers" is
/// not a thing it can do — it is a development / stdio affordance, and
/// `TaskDispatch::resolve_owner`'s own rustdoc says so in those words. The
/// fail-closed guarantee this phase makes is about AUTH-CONFIGURED deployments
/// (row 2), which tests 1–5 exercise.
///
/// The production backends bound this independently: `TaskSecurityConfig`
/// defaults `allow_anonymous` to `false`, so `pmcp-tasks`' `GenericTaskStore`
/// refuses the anonymous bucket outright unless an operator opts in. A
/// `DynamoDB`- or Redis-backed deployment therefore cannot reach the shape this
/// test asserts without a deliberate configuration change.
///
/// A test that states the accepted weakness is what stops a future reader from
/// "discovering" it as a vulnerability, filing it, and closing it with a change
/// that breaks every stdio server. If this test ever fails because the sharing
/// stopped, that is a DELIBERATE behaviour change and it needs its own plan —
/// not a fix to this file.
#[tokio::test]
async fn a_no_auth_provider_server_shares_one_v2_bucket() {
    let (addr, handle, _store) = spawn_tasks_server_with_store(AuthPosture::None).await;

    // Two callers presenting DIFFERENT credentials to a server that has no
    // provider to interpret them. Both bind ANONYMOUS_PRINCIPAL.
    let created = task_id_of(&create_task(addr, Some(OWNER_A), TASKS_TOOL_NAME, 1).await);
    let read_by_other = tasks_get(addr, Some(OWNER_B), &created, 2).await;
    let read_by_nobody = tasks_get(addr, None, &created, 3).await;
    teardown(handle, ()).await;

    assert_eq!(
        result_of(&read_by_other)["taskId"],
        json!(created),
        "ACCEPTED: with no auth provider there is one shared bucket, so a different \
         bearer reads the same task. See this test's rustdoc before changing it: {}",
        read_by_other.raw
    );
    assert_eq!(
        result_of(&read_by_nobody)["taskId"],
        json!(created),
        "and a caller presenting no credential at all reads it too — the bearer was \
         never interpreted, which is precisely why the bucket is shared: {}",
        read_by_nobody.raw
    );
}
